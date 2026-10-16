// Panneau latéral : chargement du moteur, analyse du document, liste des problèmes.

import { type DocumentAccess, DemoDocument, WordDocument } from "./document";
import { type Category, type CheckOptions, Engine, type Issue } from "./engine";
import { type Target, locate } from "./mapping";
import { loadSettings, saveSettings } from "./settings";

interface Found {
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
];

const DEMO_TEXT = [
  "Il à mangé du pain , puis il ce lave les mains.",
  "Quelle belle journée! Nous partons maintenat a cause de la pluie.",
  "Je ne sais pas d'ou il vient... Mon frère - le plus jeune - arrive demain.",
].join("\n");

const state = {
  engine: null as Engine | null,
  doc: null as DocumentAccess | null,
  options: loadSettings(),
  found: [] as Found[],
  ignored: new Set<string>(),
  filter: "tout" as Category | "tout",
  busy: false,
  analyzed: false,
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

function setStatus(text: string, kind: "info" | "error" = "info"): void {
  const s = $("status");
  s.textContent = text;
  s.dataset.kind = kind;
}

/** Rend visibles les blancs d'un extrait (espace, insécable, fine). */
function showSpaces(s: string): string {
  return s.replace(/ /g, "␣").replace(/ /g, "⍽").replace(/ /g, "⍽");
}

async function fetchText(url: string): Promise<string> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${url} : ${r.status}`);
  return r.text();
}

async function loadEngine(): Promise<Engine> {
  setStatus("Chargement du moteur…");
  const index = JSON.parse(await fetchText("data/index.json")) as string[];
  const files: Record<string, string> = {};
  await Promise.all(index.map(async (f) => (files[f] = await fetchText(`data/${f}`))));
  const wasm = await (await fetch("correcteur.wasm")).arrayBuffer();
  return Engine.load(wasm, files);
}

async function analyze(): Promise<void> {
  const { engine, doc } = state;
  if (!engine || !doc || state.busy) return;
  state.busy = true;
  $("analyze").setAttribute("disabled", "");
  try {
    const t0 = performance.now();
    const paragraphs = await doc.readParagraphs();
    const found: Found[] = [];
    paragraphs.forEach((text, paragraph) => {
      if (!text.trim()) return;
      for (const issue of engine.check(text, state.options)) {
        const context = text.slice(Math.max(0, issue.start - 20), issue.end + 20);
        found.push({
          key: `${issue.rule}|${context}`,
          paragraph,
          text,
          issue,
          target: locate(text, issue),
        });
      }
    });
    state.found = found;
    state.analyzed = true;
    const words = paragraphs.join(" ").split(/\s+/).filter(Boolean).length;
    setStatus(`${words} mots analysés en ${Math.round(performance.now() - t0)} ms`);
    render();
  } catch (e) {
    setStatus(`Analyse impossible : ${(e as Error).message}`, "error");
  } finally {
    state.busy = false;
    $("analyze").removeAttribute("disabled");
  }
}

function visible(): Found[] {
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

async function reveal(f: Found): Promise<void> {
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

async function start(inWord: boolean): Promise<void> {
  if (inWord) {
    state.doc = new WordDocument();
  } else {
    const area = $<HTMLTextAreaElement>("demo-text");
    area.value = DEMO_TEXT;
    $("demo").hidden = false;
    state.doc = new DemoDocument(area);
  }
  bindOptions();
  $("analyze").onclick = () => void analyze();
  render();
  try {
    state.engine = await loadEngine();
    setStatus(`Prêt · lexique de ${state.engine.forms.toLocaleString("fr-FR")} formes`);
    $("analyze").removeAttribute("disabled");
    if (!inWord) await analyze();
  } catch (e) {
    setStatus(`Le moteur n'a pas pu être chargé : ${(e as Error).message}`, "error");
  }
}

// Hors de Word (navigateur), Office.js est absent ou `host` est nul : mode démonstration.
if (typeof Office !== "undefined") {
  Office.onReady((info) => void start(info.host === Office.HostType.Word));
} else {
  void start(false);
}
