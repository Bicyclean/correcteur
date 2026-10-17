// Panneau commun à l'add-in Word et à l'application de bureau : analyse du
// document paragraphe par paragraphe, liste des problèmes, réglages.
// La page doit contenir les éléments décrits dans `panneau.html` (status,
// analyze, filters, issues, options…).

import { type DocumentAccess } from "./document";
import { type Target, locate } from "./mapping";
import { loadSettings, saveSettings } from "./settings";
import { type Category, type CheckOptions, type Checker, type Conjugation, type Issue, type Synonyms } from "./types";

export interface Found {
  key: string;
  paragraph: number;
  text: string;
  issue: Issue;
  target: Target | null;
}

const CATEGORIES: { id: Category; label: string }[] = [
  { id: "orthographe", label: "Orthographe" },
  { id: "homophone", label: "Homophones" },
  { id: "grammaire", label: "Grammaire" },
  { id: "typographie", label: "Typographie" },
  { id: "style", label: "Style" },
];

const state = {
  engine: null as Checker | null,
  doc: null as DocumentAccess | null,
  options: loadSettings(),
  found: [] as Found[],
  ignored: new Set<string>(),
  filter: "tout" as Category | "tout",
  busy: false,
  /** Une analyse a été demandée pendant qu'une autre tournait. */
  again: false,
  analyzed: false,
  onAnalyzed: null as ((found: Found[]) => void) | null,
};

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> & { className?: string } = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = Object.assign(document.createElement(tag), props);
  node.append(...children);
  return node;
}

export function setStatus(text: string, kind: "info" | "error" = "info"): void {
  const s = $("status");
  s.textContent = text;
  s.dataset.kind = kind;
}

/** Rend visibles les blancs d'un extrait (espace, insécable, fine). */
function showSpaces(s: string): string {
  return s.replace(/ /g, "␣").replace(/ /g, "⍽").replace(/ /g, "⍽");
}

export async function analyze(): Promise<void> {
  const { engine, doc } = state;
  if (!engine || !doc) return;
  if (state.busy) {
    state.again = true;
    return;
  }
  state.busy = true;
  $("analyze").setAttribute("disabled", "");
  try {
    const t0 = performance.now();
    const paragraphs = await doc.readParagraphs();
    const found: Found[] = [];
    for (const [paragraph, text] of paragraphs.entries()) {
      if (!text.trim()) continue;
      for (const issue of await engine.check(text, state.options)) {
        const context = text.slice(Math.max(0, issue.start - 20), issue.end + 20);
        found.push({
          key: `${issue.rule}|${context}`,
          paragraph,
          text,
          issue,
          target: locate(text, issue),
        });
      }
    }
    state.found = found;
    state.analyzed = true;
    const ms = Math.round(performance.now() - t0);
    const r = await engine.readability(paragraphs.join("\n"));
    const score = r.words > 0 ? ` · lisibilité ${Math.round(r.score)} (${r.level})` : "";
    setStatus(`${r.words} mots${score} · ${ms} ms`);
    render();
    state.onAnalyzed?.(visible());
  } catch (e) {
    setStatus(`Analyse impossible : ${(e as Error).message}`, "error");
  } finally {
    state.busy = false;
    $("analyze").removeAttribute("disabled");
  }
  if (state.again) {
    state.again = false;
    await analyze();
  }
}

export function visible(): Found[] {
  return state.found.filter((f) => !state.ignored.has(f.key));
}

function render(): void {
  const items = visible();
  const chips = $("filters");
  chips.replaceChildren();
  const chip = (id: Category | "tout", label: string, count: number) => {
    const b = el("button", { className: "chip", type: "button" }, label, el("span", { className: "count" }, String(count)));
    b.setAttribute("aria-pressed", String(state.filter === id));
    if (id !== "tout") b.dataset.category = id;
    b.onclick = () => {
      state.filter = id;
      render();
    };
    chips.append(b);
  };
  chip("tout", "Tout", items.length);
  for (const c of CATEGORIES) {
    const n = items.filter((f) => f.issue.category === c.id).length;
    if (n > 0) chip(c.id, c.label, n);
  }

  const list = $("issues");
  list.replaceChildren();
  const shown = items.filter((f) => state.filter === "tout" || f.issue.category === state.filter);
  if (shown.length === 0) {
    list.append(
      el(
        "p",
        { className: "empty" },
        state.analyzed ? "Aucun problème détecté." : "Lancez l'analyse pour vérifier le document.",
      ),
    );
    return;
  }
  for (const f of shown) list.append(card(f));
}

function card(f: Found): HTMLElement {
  const { issue, text } = f;
  const before = text.slice(Math.max(0, issue.start - 30), issue.start);
  const wrong = text.slice(issue.start, issue.end);
  const after = text.slice(issue.end, issue.end + 30);
  const excerpt = el(
    "p",
    { className: "excerpt" },
    (issue.start > 30 ? "…" : "") + before,
    el("mark", {}, wrong.trim() ? wrong : showSpaces(wrong) || "‸"),
    after + (issue.end + 30 < text.length ? "…" : ""),
  );

  const suggestions = el("div", { className: "suggestions" });
  for (const s of issue.suggestions) {
    const label = s === "" ? "Supprimer" : s.trim() ? s.replace(/[  ]/g, "⍽") : showSpaces(s);
    const b = el("button", { className: "suggestion", type: "button" }, label);
    b.setAttribute("aria-label", s === "" ? "Supprimer" : `Remplacer par « ${label} »`);
    b.onclick = (e) => {
      e.stopPropagation();
      void apply(f, s);
    };
    suggestions.append(b);
  }

  const actions = el("div", { className: "actions" });
  const action = (label: string, fn: () => void) => {
    const b = el("button", { className: "link", type: "button" }, label);
    b.onclick = (e) => {
      e.stopPropagation();
      fn();
    };
    actions.append(b);
  };
  action("Ignorer", () => {
    state.ignored.add(f.key);
    render();
    state.onAnalyzed?.(visible());
  });
  if (issue.rule === "ORTH_MOT_INCONNU") {
    action("Ajouter au dictionnaire", () => {
      if (!state.options.userWords.includes(wrong)) state.options.userWords.push(wrong);
      saveSettings(state.options);
      renderDictionary();
      void analyze();
    });
  } else {
    action("Désactiver cette règle", () => {
      state.options.disabledRules.push(issue.rule);
      saveSettings(state.options);
      void analyze();
    });
  }

  const explanation = el("details", { className: "explanation" }, el("summary", {}, "Pourquoi ?"), el("p", {}, issue.explanation));
  explanation.onclick = (e) => e.stopPropagation();

  const node = el(
    "article",
    { className: "card", tabIndex: 0, title: "Afficher dans le document" },
    el("p", { className: "message" }, issue.message),
    excerpt,
    suggestions,
    explanation,
    actions,
  );
  node.dataset.category = issue.category;
  const select = () => void reveal(f);
  node.onclick = select;
  node.onkeydown = (e) => {
    if (e.key === "Enter" && e.target === node) select();
  };
  return node;
}

export async function reveal(f: Found): Promise<void> {
  if (!state.doc || !f.target) return;
  const r = await state.doc.select(f.paragraph, f.text, f.target);
  report(r);
}

async function apply(f: Found, suggestion: string): Promise<void> {
  if (!state.doc) return;
  if (!f.target) {
    setStatus("Ce passage ne peut pas être modifié automatiquement.", "error");
    return;
  }
  const r = await state.doc.replace(f.paragraph, f.text, f.target, suggestion);
  report(r);
  await analyze();
}

function report(r: "ok" | "changed" | "notfound"): void {
  if (r === "changed") setStatus("Le texte a changé depuis l'analyse : relancez-la.", "error");
  if (r === "notfound") setStatus("Passage introuvable dans le document.", "error");
}

function bindOptions(): void {
  const boxes: [string, keyof CheckOptions][] = [
    ["opt-fine", "narrowNbsp"],
    ["opt-apostrophe", "typographicApostrophe"],
    ["opt-guillemets", "frenchQuotes"],
    ["opt-points", "ellipsisCharacter"],
    ["opt-noms", "checkProperNouns"],
  ];
  for (const [id, key] of boxes) {
    const box = $<HTMLInputElement>(id);
    box.checked = state.options[key] as boolean;
    box.onchange = () => {
      (state.options[key] as boolean) = box.checked;
      saveSettings(state.options);
      void analyze();
    };
  }
  const cats = $("opt-categories");
  for (const c of CATEGORIES) {
    const box = el("input", { type: "checkbox", checked: !state.options.disabledCategories.includes(c.id) });
    box.onchange = () => {
      const set = new Set(state.options.disabledCategories);
      if (box.checked) set.delete(c.id);
      else set.add(c.id);
      state.options.disabledCategories = [...set];
      saveSettings(state.options);
      void analyze();
    };
    cats.append(el("label", {}, box, ` ${c.label}`));
  }
  const phrase = document.getElementById("opt-phrase") as HTMLInputElement | null;
  if (phrase) {
    phrase.value = String(state.options.longSentenceWords);
    phrase.onchange = () => {
      const n = Number(phrase.value);
      if (Number.isInteger(n) && n >= 10 && n <= 200) {
        state.options.longSentenceWords = n;
        saveSettings(state.options);
        void analyze();
      }
    };
  }
  $("opt-reset-rules").onclick = () => {
    state.options.disabledRules = [];
    saveSettings(state.options);
    void analyze();
  };
  renderDictionary();
}

function renderDictionary(): void {
  const list = $("dictionary");
  list.replaceChildren();
  if (state.options.userWords.length === 0) {
    list.append(el("li", { className: "muted" }, "Aucun mot."));
    return;
  }
  for (const w of state.options.userWords) {
    const remove = el("button", { className: "link", type: "button" }, "Retirer");
    remove.onclick = () => {
      state.options.userWords = state.options.userWords.filter((x) => x !== w);
      saveSettings(state.options);
      renderDictionary();
      void analyze();
    };
    list.append(el("li", {}, el("span", {}, w), remove));
  }
}

// ---------------------------------------------------------------------------
// Outils : conjugaison et synonymes

function showConjugation(c: Conjugation): HTMLElement {
  const head = el(
    "p",
    { className: "tool-head" },
    el("strong", {}, c.infinitive),
    ` — modèle « ${c.model} », auxiliaire ${c.auxiliary}`,
    c.guessed ? " (verbe inconnu : modèle déduit de la terminaison)" : "",
  );
  const modes = c.modes.map((m) =>
    el(
      "details",
      { className: "mode", open: m.name === "Indicatif" },
      el("summary", {}, m.name),
      ...m.tenses.map((t) =>
        el("div", { className: "tense" }, el("h4", {}, t.name), el("ul", {}, ...t.lines.map((l) => el("li", {}, l)))),
      ),
    ),
  );
  return el("div", { className: "conjugation" }, head, ...modes);
}

function showSynonyms(s: Synonyms): HTMLElement {
  const block = (title: string, groups: Synonyms["groups"]) => {
    if (groups.length === 0) return [];
    return [
      el("h4", { className: "tool-section" }, title),
      ...groups.map((g) => {
        const list = el("div", { className: "suggestions" });
        for (const w of g.words) {
          const b = el("button", { className: "suggestion", type: "button", title: "Remplacer la sélection" }, w);
          b.setAttribute("aria-label", `Remplacer la sélection par « ${w} »`);
          b.onclick = async () => {
            await state.doc?.replaceSelection(w);
            void analyze();
          };
          list.append(b);
        }
        return el("div", { className: "synonym-group" }, el("p", { className: "muted" }, g.category), list);
      }),
    ];
  };
  return el(
    "div",
    {},
    el("p", { className: "tool-head" }, el("strong", {}, s.entry)),
    ...block("Synonymes", s.groups),
    ...block("Antonymes", s.antonyms),
  );
}

function bindTools(): void {
  const out = document.getElementById("outil-resultat");
  const conj = document.getElementById("form-conjuguer") as HTMLFormElement | null;
  const syn = document.getElementById("form-synonymes") as HTMLFormElement | null;
  if (!out || !conj || !syn) return;
  const show = (node: Node) => out.replaceChildren(node);
  conj.onsubmit = async (e) => {
    e.preventDefault();
    const verb = $<HTMLInputElement>("verbe").value.trim();
    if (!verb || !state.engine) return;
    const c = await state.engine.conjugate(verb);
    show(c ? showConjugation(c) : el("p", { className: "muted" }, `« ${verb} » : verbe inconnu.`));
  };
  const word = $<HTMLInputElement>("mot");
  // Champ vide : on part de la sélection du document.
  word.onfocus = async () => {
    if (!word.value && state.doc) word.value = (await state.doc.selectedText()).trim();
  };
  syn.onsubmit = async (e) => {
    e.preventDefault();
    let w = word.value.trim();
    if (!w && state.doc) w = (await state.doc.selectedText()).trim();
    if (!w || !state.engine) return;
    word.value = w;
    const s = await state.engine.synonyms(w);
    show(s ? showSynonyms(s) : el("p", { className: "muted" }, `Aucun synonyme ni antonyme connu pour « ${w} ».`));
  };
}

export interface PanelSetup {
  doc: DocumentAccess;
  /** Charge le moteur (WebAssembly ou natif). */
  loadChecker: () => Promise<Checker>;
  /** Analyser dès que le moteur est prêt. */
  analyzeOnReady: boolean;
  /** Appelé après chaque analyse avec les problèmes affichés. */
  onAnalyzed?: (found: Found[]) => void;
}

export async function startPanel(setup: PanelSetup): Promise<void> {
  state.doc = setup.doc;
  state.onAnalyzed = setup.onAnalyzed ?? null;
  bindOptions();
  bindTools();
  $("analyze").onclick = () => void analyze();
  render();
  try {
    setStatus("Chargement du moteur…");
    state.engine = await setup.loadChecker();
    setStatus(`Prêt · lexique de ${state.engine.forms.toLocaleString("fr-FR")} formes`);
    $("analyze").removeAttribute("disabled");
    if (setup.analyzeOnReady) await analyze();
  } catch (e) {
    setStatus(`Le moteur n'a pas pu être chargé : ${(e as Error).message}`, "error");
  }
}
