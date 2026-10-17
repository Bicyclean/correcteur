// Panneau du correcteur, commun à l'add-in Word et à l'application de bureau.
//
// Le panneau construit toute son interface dans l'élément fourni : en-tête,
// onglets Corrections / Outils / Réglages, pied de page et notifications.
// L'hôte fournit l'accès au document et le moteur (WebAssembly ou natif).

import { type DocumentAccess } from "./document";
import { type IconName, icon, logo } from "./icones";
import { type Target, applyToText, locate } from "./mapping";
import { loadSettings, saveSettings } from "./settings";
import { type Category, type CheckOptions, type Checker, type Conjugation, type Issue, type Readability, type Synonyms } from "./types";

export interface Found {
  /** Identifiant stable tant que le texte autour de l'erreur ne change pas. */
  key: string;
  paragraph: number;
  text: string;
  issue: Issue;
  target: Target | null;
}

export interface PanelSetup {
  /** Élément qui reçoit le panneau. */
  root: HTMLElement;
  doc: DocumentAccess;
  /** Charge le moteur (WebAssembly ou natif). */
  loadChecker: () => Promise<Checker>;
  /** « word » : bouton Analyser ; « bureau » : analyse pendant la frappe. */
  host: "word" | "bureau";
  analyzeOnReady: boolean;
  /** Après chaque analyse, avec les problèmes affichés. */
  onAnalyzed?: (found: Found[]) => void;
  /** Quand un problème devient actif (ou plus aucun). */
  onActivate?: (found: Found | null) => void;
  /** Annule la dernière modification du document (bureau). */
  undo?: () => void;
}

const CATEGORIES: { id: Category; label: string; desc: string }[] = [
  { id: "orthographe", label: "Orthographe", desc: "Mots inconnus, fautes de frappe" },
  { id: "homophone", label: "Homophones", desc: "a/à, ou/où, ce/se, on/ont, sa/ça…" },
  { id: "grammaire", label: "Grammaire", desc: "Accords, participes, négation" },
  { id: "typographie", label: "Typographie", desc: "Espaces, guillemets, apostrophes, tirets" },
  { id: "style", label: "Style", desc: "Répétitions et phrases longues : des conseils, pas des fautes" },
];
const label = (c: Category) => CATEGORIES.find((x) => x.id === c)?.label ?? c;

type Tab = "corrections" | "outils" | "reglages";

const state = {
  setup: null as PanelSetup | null,
  engine: null as Checker | null,
  options: loadSettings(),
  found: [] as Found[],
  ignored: new Set<string>(),
  filter: "tout" as Category | "tout",
  active: null as string | null,
  busy: false,
  again: false,
  phase: "loading" as "loading" | "ready" | "error",
  error: "",
  analyzed: false,
  words: 0,
  readability: null as Readability | null,
  /** Libellé des règles vues, pour la liste des règles désactivées. */
  ruleNames: new Map<string, string>(),
  tool: "conjugaison" as "conjugaison" | "synonymes",
};

const ui = {} as {
  root: HTMLElement;
  analyze: HTMLButtonElement | null;
  tabs: Record<Tab, HTMLButtonElement>;
  sections: Record<Tab, HTMLElement>;
  badge: HTMLElement;
  toast: HTMLElement;
  toolResult: HTMLElement;
  toolInput: HTMLInputElement;
};

// ---------------------------------------------------------------------------
// Petits outils DOM

type Child = Node | string | null | undefined | false;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> & Record<string, unknown> = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(props)) {
    if (v === undefined || v === null) continue;
    if (k.startsWith("aria-") || k.startsWith("data-") || k === "role") node.setAttribute(k, String(v));
    else (node as unknown as Record<string, unknown>)[k] = v;
  }
  node.append(...(children.filter(Boolean) as (Node | string)[]));
  return node;
}

function button(
  text: string,
  opts: { variant?: "primary" | "ghost"; size?: "sm"; iconName?: IconName; onClick: (e: MouseEvent) => void; title?: string; ariaLabel?: string },
): HTMLButtonElement {
  const cls = ["cr-btn", opts.variant && `cr-btn--${opts.variant}`, opts.size && `cr-btn--${opts.size}`, !text && "cr-btn--icon"]
    .filter(Boolean)
    .join(" ");
  const b = el("button", { type: "button", className: cls, title: opts.title, "aria-label": opts.ariaLabel }, opts.iconName ? icon(opts.iconName, opts.size ? 14 : 16) : null, text);
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    opts.onClick(e);
  });
  return b;
}

function switchControl(checked: boolean, labelledBy: string, onChange: (v: boolean) => void): HTMLButtonElement {
  const b = el("button", { type: "button", className: "cr-switch", role: "switch", "aria-checked": String(checked), "aria-labelledby": labelledBy });
  b.addEventListener("click", () => {
    const v = b.getAttribute("aria-checked") !== "true";
    b.setAttribute("aria-checked", String(v));
    onChange(v);
  });
  return b;
}

let idCounter = 0;
const uid = (p: string) => `${p}-${++idCounter}`;

/** Description d'une chaîne pour les lecteurs d'écran (espaces nommées). */
function spoken(s: string): string {
  if (s === "") return "rien";
  return s
    .replace(/ /g, " espace fine insécable ")
    .replace(/ /g, " espace insécable ")
    .replace(/^ $/, "une espace")
    .trim();
}

/** Contenu visible d'une suggestion : les espaces spéciales deviennent des repères lisibles. */
function visibleText(s: string): Node[] {
  const out: Node[] = [];
  let buf = "";
  const flush = () => {
    if (buf) out.push(document.createTextNode(buf));
    buf = "";
  };
  for (const ch of s) {
    const mark = ch === " " ? ["fine", "espace fine insécable"] : ch === " " ? ["insécable", "espace insécable"] : ch === " " && s.trim() === "" ? ["espace", "espace"] : null;
    if (mark) {
      flush();
      out.push(el("span", { className: "cr-space", title: mark[1] }, mark[0]));
    } else buf += ch;
  }
  flush();
  return out;
}

// ---------------------------------------------------------------------------
// Notifications

let toastTimer: ReturnType<typeof setTimeout> | undefined;
export function notify(message: string, action?: { label: string; run: () => void }): void {
  const t = ui.toast;
  if (!t) return;
  t.replaceChildren(el("span", {}, message));
  if (action) {
    t.append(
      button(action.label, {
        size: "sm",
        onClick: () => {
          t.classList.remove("is-visible");
          action.run();
        },
      }),
    );
  }
  t.classList.add("is-visible");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("is-visible"), action ? 6000 : 3000);
}

// ---------------------------------------------------------------------------
// Analyse

export async function analyze(): Promise<void> {
  const { engine } = state;
  const doc = state.setup?.doc;
  if (!engine || !doc) return;
  if (state.busy) {
    state.again = true;
    return;
  }
  state.busy = true;
  ui.analyze?.setAttribute("disabled", "");
  try {
    const paragraphs = await doc.readParagraphs();
    const found: Found[] = [];
    const seen = new Map<string, number>();
    for (const [paragraph, text] of paragraphs.entries()) {
      if (!text.trim()) continue;
      for (const issue of await engine.check(text, state.options)) {
        const context = text.slice(Math.max(0, issue.start - 20), issue.end + 20);
        const base = `${issue.rule}|${context}`;
        const n = (seen.get(base) ?? 0) + 1;
        seen.set(base, n);
        state.ruleNames.set(issue.rule, issue.message.replace(/ :.*$| « .*$/, "").replace(/[.]$/, ""));
        found.push({ key: `${base}|${n}`, paragraph, text, issue, target: locate(text, issue) });
      }
    }
    const r = await engine.readability(paragraphs.join("\n"));
    state.found = found;
    state.analyzed = true;
    state.words = r.words;
    state.readability = r.words >= 30 ? r : null;
    if (state.active && !found.some((f) => f.key === state.active)) state.active = null;
    renderCorrections();
    state.setup?.onAnalyzed?.(visible());
  } catch (e) {
    notify(`Analyse impossible : ${(e as Error).message}`);
  } finally {
    state.busy = false;
    ui.analyze?.removeAttribute("disabled");
  }
  if (state.again) {
    state.again = false;
    await analyze();
  }
}

/** Problèmes affichés (non ignorés), dans l'ordre du document. */
export function visible(): Found[] {
  return state.found.filter((f) => !state.ignored.has(f.key) && (state.filter === "tout" || f.issue.category === state.filter));
}

function allVisible(): Found[] {
  return state.found.filter((f) => !state.ignored.has(f.key));
}

// ---------------------------------------------------------------------------
// Onglet Corrections

function renderCorrections(): void {
  const section = ui.sections.corrections;
  const items = allVisible();
  ui.badge.textContent = String(items.length);
  ui.badge.toggleAttribute("data-zero", items.length === 0);

  if (state.phase === "loading") {
    section.replaceChildren(
      el("p", { className: "cr-summary-meta", role: "status" }, "Chargement du dictionnaire…"),
      el("div", { className: "cr-skeleton", "aria-hidden": "true" }, el("div"), el("div"), el("div")),
    );
    return;
  }
  if (state.phase === "error") {
    section.replaceChildren(
      stateBlock("error", "alert", "Le correcteur n'a pas pu démarrer", state.error, button("Réessayer", { variant: "primary", iconName: "refresh", onClick: () => location.reload() })),
    );
    return;
  }

  const nodes: Node[] = [];
  // Résumé
  const total = items.length;
  // Un document vide n'est pas « sans problème » : il n'a encore rien à vérifier.
  const empty = !state.analyzed || state.words === 0;
  const summary = el(
    "div",
    { className: "cr-summary" },
    el("span", { className: "cr-summary-main", role: "status" }, empty ? "Prêt" : total === 0 ? "Aucun problème" : `${total} suggestion${total > 1 ? "s" : ""}`),
    empty ? null : el("span", { className: "cr-summary-meta" }, `${state.words} mot${state.words > 1 ? "s" : ""}`),
    state.readability ? readabilityPill(state.readability) : null,
  );
  nodes.push(summary);

  // Filtres
  if (total > 0) {
    const filters = el("div", { className: "cr-filters", role: "radiogroup", "aria-label": "Filtrer par catégorie" });
    const chip = (id: Category | "tout", text: string, count: number) => {
      const b = el(
        "button",
        { type: "button", className: "cr-chip", role: "radio", "aria-checked": String(state.filter === id), "data-category": id === "tout" ? undefined : id },
        id === "tout" ? null : el("span", { className: "cr-dot" }),
        text,
        el("span", { className: "cr-count" }, String(count)),
      );
      b.addEventListener("click", () => {
        state.filter = id;
        renderCorrections();
        state.setup?.onAnalyzed?.(visible());
      });
      filters.append(b);
    };
    chip("tout", "Tout", total);
    for (const c of CATEGORIES) {
      const n = items.filter((f) => f.issue.category === c.id).length;
      if (n > 0) chip(c.id, c.label, n);
    }
    if (state.filter !== "tout" && !items.some((f) => f.issue.category === state.filter)) state.filter = "tout";
    nodes.push(filters);
  }

  const shown = visible();
  // Correction groupée : seulement la typographie, aux corrections sans ambiguïté.
  const bulk = shown.filter((f) => f.issue.category === "typographie" && f.issue.suggestions.length > 0 && f.target);
  if (state.filter === "typographie" && bulk.length > 1) {
    nodes.push(
      el(
        "div",
        { className: "cr-bulk" },
        el("span", {}, `${bulk.length} corrections typographiques`),
        button("Tout appliquer", { variant: "primary", size: "sm", iconName: "check", onClick: () => void applyAll(bulk) }),
      ),
    );
  }

  if (empty) {
    const hint =
      state.setup?.host === "word"
        ? state.analyzed
          ? "Le document est vide. Écrivez du texte, puis cliquez sur « Analyser »."
          : "Cliquez sur « Analyser » pour vérifier le document."
        : "Les suggestions apparaîtront ici dès que vous écrirez.";
    nodes.push(stateBlock("neutral", "sparkle", "Prêt à corriger", hint));
  } else if (shown.length === 0) {
    nodes.push(
      stateBlock(
        "ok",
        "circleCheck",
        total === 0 ? "Rien à signaler" : "Aucun problème dans cette catégorie",
        total === 0 ? "Le texte ne présente aucun problème détecté par les règles actuelles." : "Choisissez « Tout » pour voir les autres suggestions.",
      ),
    );
  } else {
    const list = el("div", { className: "cr-list", role: "list", "aria-label": "Problèmes détectés" });
    for (const f of shown) list.append(card(f));
    list.addEventListener("keydown", onListKey);
    nodes.push(list);
  }
  ui.sections.corrections.replaceChildren(...nodes);
}

function readabilityPill(r: Readability): HTMLElement {
  const score = Math.round(r.score);
  return el(
    "span",
    { className: "cr-pill", title: `Indice de lisibilité de Kandel et Moles : ${score} sur 100 (${r.level}). ${r.sentences} phrases, ${(r.words / Math.max(1, r.sentences)).toFixed(1)} mots par phrase.` },
    "Lisibilité",
    el("span", { className: "cr-gauge", "aria-hidden": "true" }, el("span", { style: `width:${score}%` } as Record<string, unknown>)),
    `${score} · ${r.level}`,
  );
}

function stateBlock(kind: "ok" | "error" | "neutral", iconName: IconName, title: string, text: string, action?: Node): HTMLElement {
  return el("div", { className: `cr-state cr-state--${kind}` }, el("div", { className: "cr-state-icon" }, icon(iconName, 22)), el("h3", {}, title), el("p", {}, text), action ?? null);
}

function card(f: Found): HTMLElement {
  const { issue, text } = f;
  const wrong = text.slice(issue.start, issue.end);
  const active = state.active === f.key;
  const node = el("article", {
    className: "cr-card",
    role: "listitem",
    tabIndex: 0,
    "data-category": issue.category,
    "data-key": f.key,
    "aria-current": active ? "true" : undefined,
    "aria-label": `${label(issue.category)} : ${issue.message}`,
  });

  const top = el("div", { className: "cr-card-top" }, el("span", { className: "cr-tag" }, el("span", { className: "cr-dot" }), label(issue.category)));
  const msg = el("p", { className: "cr-msg" }, issue.message);

  const before = text.slice(Math.max(0, issue.start - 40), issue.start);
  const after = text.slice(issue.end, issue.end + 40);
  const excerpt = el(
    "p",
    { className: "cr-excerpt" },
    (issue.start > 40 ? "…" : "") + before,
    el("mark", {}, ...(wrong.trim() ? [wrong] : visibleText(wrong.replace(/ /g, " ") || "‸"))),
    after + (issue.end + 40 < text.length ? "…" : ""),
  );

  const suggestions = el("div", { className: "cr-suggestions" });
  if (issue.suggestions.length === 0) {
    suggestions.append(el("span", { className: "cr-none" }, "Pas de correction automatique : à revoir."));
  }
  issue.suggestions.forEach((s, i) => {
    const del = s === "";
    const b = el(
      "button",
      {
        type: "button",
        className: ["cr-sugg", i === 0 && "cr-sugg--first", del && "cr-sugg--delete"].filter(Boolean).join(" "),
        "aria-label": del ? `Supprimer « ${spoken(wrong)} »` : `Remplacer « ${spoken(wrong)} » par « ${spoken(s)} »`,
      },
      ...(del ? [icon("x", 14), "Supprimer"] : visibleText(s)),
    );
    b.addEventListener("click", (e) => {
      e.stopPropagation();
      void apply(f, s);
    });
    suggestions.append(b);
  });

  const actions = el(
    "div",
    { className: "cr-actions" },
    button("Ignorer", { variant: "ghost", size: "sm", iconName: "x", onClick: () => ignore(f) }),
    issue.rule === "ORTH_MOT_INCONNU"
      ? button("Ajouter au dictionnaire", { variant: "ghost", size: "sm", iconName: "plus", onClick: () => addWord(wrong) })
      : button("Désactiver cette règle", { variant: "ghost", size: "sm", iconName: "sliders", onClick: () => disableRule(issue.rule) }),
  );
  const details = el("div", { className: "cr-details" }, el("p", { className: "cr-why" }, issue.explanation), actions);

  node.append(top, msg, excerpt, suggestions, details);
  node.addEventListener("click", () => void activate(f, { reveal: true }));
  node.addEventListener("focus", () => {
    if (state.active !== f.key) void activate(f, { reveal: false });
  });
  return node;
}

function cardFor(key: string): HTMLElement | null {
  return ui.sections.corrections.querySelector(`.cr-card[data-key="${CSS.escape(key)}"]`);
}

/** Rend un problème actif : carte dépliée, passage montré dans le document. */
export async function activate(f: Found | null, opts: { reveal: boolean; focus?: boolean } = { reveal: false }): Promise<void> {
  state.active = f?.key ?? null;
  for (const c of ui.sections.corrections.querySelectorAll<HTMLElement>(".cr-card")) {
    if (c.dataset.key === state.active) c.setAttribute("aria-current", "true");
    else c.removeAttribute("aria-current");
  }
  if (!f) {
    state.setup?.onActivate?.(null);
    return;
  }
  const c = cardFor(f.key);
  c?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  if (opts.focus) c?.focus({ preventScroll: true });
  state.setup?.onActivate?.(f);
  if (opts.reveal && f.target && state.setup) {
    const r = await state.setup.doc.select(f.paragraph, f.text, f.target);
    if (r !== "ok") report(r);
  }
}

/** Problème suivant (+1) ou précédent (-1). */
export function step(delta: number, reveal = true): void {
  const items = visible();
  if (items.length === 0) return;
  const i = items.findIndex((f) => f.key === state.active);
  const next = items[(i + delta + items.length) % items.length] ?? items[0];
  void activate(next, { reveal, focus: !reveal });
}

/** Rend actif le problème qui contient la position donnée (clic dans l'éditeur). */
export function activateAt(paragraph: number, offset: number): void {
  const f = visible().find((x) => x.paragraph === paragraph && offset >= x.issue.start && offset <= x.issue.end);
  if (f && f.key !== state.active) {
    showTab("corrections");
    void activate(f, { reveal: false });
  }
}

function onListKey(e: KeyboardEvent): void {
  const target = e.target as HTMLElement;
  if (!target.classList.contains("cr-card")) return;
  const f = state.found.find((x) => x.key === target.dataset.key);
  if (!f) return;
  switch (e.key) {
    case "ArrowDown":
    case "ArrowUp":
      e.preventDefault();
      step(e.key === "ArrowDown" ? 1 : -1, false);
      break;
    case "Enter":
      e.preventDefault();
      if (f.issue.suggestions.length > 0) void apply(f, f.issue.suggestions[0]);
      break;
    case "Delete":
    case "Backspace":
      e.preventDefault();
      ignore(f);
      break;
    case "Escape":
      void activate(null);
      break;
  }
}

/** Carte suivante à activer après le retrait de `f`. */
function neighbour(f: Found): string | null {
  const items = visible();
  const i = items.findIndex((x) => x.key === f.key);
  return (items[i + 1] ?? items[i - 1])?.key ?? null;
}

async function apply(f: Found, suggestion: string): Promise<void> {
  const doc = state.setup?.doc;
  if (!doc) return;
  if (!f.target) {
    notify("Ce passage ne peut pas être modifié automatiquement.");
    return;
  }
  const next = neighbour(f);
  const r = await doc.replace(f.paragraph, f.text, f.target, suggestion);
  if (r !== "ok") {
    report(r);
    await analyze();
    return;
  }
  const undo = state.setup?.undo;
  notify("Correction appliquée", undo ? { label: "Annuler", run: () => { undo(); void analyze(); } } : undefined);
  await analyze();
  // Garder le fil : activer la carte qui suivait.
  const again = state.found.find((x) => x.key === next) ?? visible()[0];
  if (again) void activate(again, { reveal: false, focus: document.activeElement?.closest?.(".cr-panel") != null });
}

async function applyAll(items: Found[]): Promise<void> {
  const doc = state.setup?.doc;
  if (!doc) return;
  const paragraphs = await doc.readParagraphs();
  let done = 0;
  // Par paragraphe, de la fin vers le début : les positions antérieures restent valables.
  const byParagraph = new Map<number, Found[]>();
  for (const f of items) byParagraph.set(f.paragraph, [...(byParagraph.get(f.paragraph) ?? []), f]);
  for (const [p, list] of byParagraph) {
    let text = paragraphs[p];
    if (text !== list[0].text) continue;
    for (const f of [...list].sort((a, b) => b.issue.start - a.issue.start)) {
      const s = f.issue.suggestions[0];
      const target = locate(text, f.issue);
      if (s === undefined || !target) continue;
      if ((await doc.replace(p, text, target, s)) !== "ok") break;
      text = applyToText(text, f.issue, s);
      done++;
    }
  }
  notify(`${done} correction${done > 1 ? "s" : ""} appliquée${done > 1 ? "s" : ""}`);
  state.filter = "tout";
  await analyze();
}

function ignore(f: Found): void {
  const next = neighbour(f);
  const c = cardFor(f.key);
  const done = () => {
    state.ignored.add(f.key);
    renderCorrections();
    state.setup?.onAnalyzed?.(visible());
    const n = state.found.find((x) => x.key === next);
    if (n) void activate(n, { reveal: false, focus: true });
  };
  if (c) {
    c.classList.add("is-leaving");
    setTimeout(done, 160);
  } else done();
  notify("Problème ignoré", {
    label: "Annuler",
    run: () => {
      state.ignored.delete(f.key);
      renderCorrections();
      state.setup?.onAnalyzed?.(visible());
    },
  });
}

function addWord(word: string): void {
  if (!state.options.userWords.includes(word)) state.options.userWords.push(word);
  saveSettings(state.options);
  notify(`« ${word} » ajouté au dictionnaire`, {
    label: "Annuler",
    run: () => {
      state.options.userWords = state.options.userWords.filter((w) => w !== word);
      saveSettings(state.options);
      renderSettings();
      void analyze();
    },
  });
  renderSettings();
  void analyze();
}

function disableRule(rule: string): void {
  if (!state.options.disabledRules.includes(rule)) state.options.disabledRules.push(rule);
  saveSettings(state.options);
  notify("Règle désactivée", {
    label: "Annuler",
    run: () => {
      state.options.disabledRules = state.options.disabledRules.filter((r) => r !== rule);
      saveSettings(state.options);
      renderSettings();
      void analyze();
    },
  });
  renderSettings();
  void analyze();
}

function report(r: "ok" | "changed" | "notfound"): void {
  if (r === "changed") notify("Le texte a changé : analyse mise à jour.");
  if (r === "notfound") notify("Passage introuvable dans le document.");
}

// ---------------------------------------------------------------------------
// Onglet Outils

function renderTools(): void {
  const section = ui.sections.outils;
  const seg = el("div", { className: "cr-segmented", role: "group", "aria-label": "Outil" });
  for (const [id, text] of [["conjugaison", "Conjugaison"], ["synonymes", "Synonymes"]] as const) {
    const b = el("button", { type: "button", "aria-pressed": String(state.tool === id) }, text);
    b.addEventListener("click", () => {
      state.tool = id;
      renderTools();
      ui.toolInput.focus();
    });
    seg.append(b);
  }
  const conj = state.tool === "conjugaison";
  const input = el("input", {
    className: "cr-input",
    type: "search",
    placeholder: conj ? "Verbe : venir, se souvenir, mangeons…" : "Mot, ou sélection du document",
    autocomplete: "off",
    spellcheck: false,
    "aria-label": conj ? "Verbe à conjuguer" : "Mot dont chercher les synonymes",
  });
  const form = el("form", { className: "cr-search", role: "search" }, icon("search"), input, el("button", { type: "submit", className: "cr-btn cr-btn--primary" }, conj ? "Conjuguer" : "Chercher"));
  const result = el("div", { "aria-live": "polite" });
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    void runTool();
  });
  ui.toolInput = input;
  ui.toolResult = result;
  const hint = el(
    "p",
    { className: "cr-hint" },
    conj
      ? "Tableau complet : temps simples et composés, pronominaux, impératif, participes."
      : "Synonymes et antonymes. Laissé vide, le champ prend le mot sélectionné ; cliquez un mot pour remplacer la sélection.",
  );
  section.replaceChildren(seg, form, hint, result);
}

async function runTool(): Promise<void> {
  const engine = state.engine;
  if (!engine) return;
  let query = ui.toolInput.value.trim();
  if (!query && state.tool === "synonymes" && state.setup) {
    query = (await state.setup.doc.selectedText()).trim();
    ui.toolInput.value = query;
  }
  if (!query) {
    ui.toolInput.focus();
    return;
  }
  if (state.tool === "conjugaison") {
    const c = await engine.conjugate(query);
    ui.toolResult.replaceChildren(c ? conjugationView(c) : stateBlock("neutral", "info", "Verbe inconnu", `« ${query} » n'est pas reconnu comme un verbe.`));
  } else {
    const s = await engine.synonyms(query);
    ui.toolResult.replaceChildren(s ? synonymsView(s) : stateBlock("neutral", "info", "Aucun résultat", `Pas de synonyme ni d'antonyme connu pour « ${query} ».`));
  }
}

let currentMode = "Indicatif";
function conjugationView(c: Conjugation): HTMLElement {
  const meta = el(
    "div",
    { className: "cr-result-meta" },
    el("span", { className: "cr-pill" }, `Auxiliaire ${c.auxiliary}`),
    el("span", { className: "cr-pill" }, `Modèle : ${c.model}`),
    c.pronominal ? el("span", { className: "cr-pill" }, "Pronominal") : null,
    c.guessed ? el("span", { className: "cr-pill", title: "Verbe absent du dictionnaire : conjugué sur le modèle du 1er groupe." }, "Modèle déduit") : null,
  );
  if (!c.modes.some((m) => m.name === currentMode)) currentMode = c.modes[0]?.name ?? "";
  const modes = el("div", { className: "cr-modes", role: "radiogroup", "aria-label": "Mode" });
  const grid = el("div", { className: "cr-tenses" });
  const show = () => {
    for (const b of modes.querySelectorAll("button")) b.setAttribute("aria-checked", String(b.textContent === currentMode));
    const mode = c.modes.find((m) => m.name === currentMode);
    grid.replaceChildren(...(mode?.tenses ?? []).map((t) => el("section", { className: "cr-tense" }, el("h4", {}, t.name), el("ul", {}, ...t.lines.map((l) => el("li", {}, l))))));
  };
  for (const m of c.modes) {
    const b = el("button", { type: "button", className: "cr-chip", role: "radio", "aria-checked": "false" }, m.name);
    b.addEventListener("click", () => {
      currentMode = m.name;
      show();
    });
    modes.append(b);
  }
  show();
  return el("div", {}, el("div", { className: "cr-result-head" }, el("h3", {}, c.infinitive), meta), modes, grid);
}

function synonymsView(s: Synonyms): HTMLElement {
  const groups = (title: string, list: Synonyms["groups"]) =>
    list.map((g) => {
      const words = el("div", { className: "cr-words" });
      for (const w of g.words) {
        const b = el("button", { type: "button", className: "cr-word", "aria-label": `Remplacer la sélection par « ${w} »` }, w);
        b.addEventListener("click", async () => {
          await state.setup?.doc.replaceSelection(w);
          notify(`Remplacé par « ${w} »`, state.setup?.undo ? { label: "Annuler", run: () => state.setup?.undo?.() } : undefined);
          void analyze();
        });
        words.append(b);
      }
      return el("div", { className: "cr-group" }, el("h4", { className: "cr-group-title" }, title, el("span", {}, g.category)), words);
    });
  return el("div", {}, el("div", { className: "cr-result-head" }, el("h3", {}, s.entry)), ...groups("Synonymes", s.groups), ...groups("Antonymes", s.antonyms));
}

// ---------------------------------------------------------------------------
// Onglet Réglages

function save(): void {
  saveSettings(state.options);
  void analyze();
}

function row(title: string, desc: string | null, control: Node, extra?: Node): HTMLElement {
  const id = uid("lbl");
  // Nom explicite : certains lecteurs d'écran résolvent mal aria-labelledby.
  if (control instanceof HTMLElement) {
    control.removeAttribute("aria-labelledby");
    if (!control.getAttribute("aria-label")) control.setAttribute("aria-label", desc ? `${title} — ${desc}` : title);
  }
  return el("div", { className: "cr-row" }, el("div", { className: "cr-row-text" }, el("div", { className: "cr-row-label", id }, extra ?? null, title), desc ? el("div", { className: "cr-row-desc" }, desc) : null), control);
}

function group(title: string, ...rows: Node[]): HTMLElement {
  return el("section", { className: "cr-settings-group" }, el("h3", {}, title), el("div", { className: "cr-settings-box" }, ...rows));
}

function boolRow(title: string, desc: string, key: "narrowNbsp" | "typographicApostrophe" | "frenchQuotes" | "ellipsisCharacter" | "checkProperNouns"): HTMLElement {
  return row(
    title,
    desc,
    switchControl(state.options[key], "", (v) => {
      state.options[key] = v;
      save();
    }),
  );
}

function numberRow(title: string, desc: string, key: "longSentenceWords" | "repetitionWindow", min: number, max: number): HTMLElement {
  const input = el("input", { className: "cr-number", type: "number", min: String(min), max: String(max), value: String(state.options[key]) });
  input.addEventListener("change", () => {
    const n = Number(input.value);
    if (Number.isInteger(n) && n >= min && n <= max) {
      state.options[key] = n;
      save();
    } else input.value = String(state.options[key]);
  });
  return row(title, desc, input);
}

function renderSettings(): void {
  const o = state.options;
  const categories = CATEGORIES.map((c) =>
    row(
      c.label,
      c.desc,
      switchControl(!o.disabledCategories.includes(c.id), "", (v) => {
        const set = new Set(o.disabledCategories);
        if (v) set.delete(c.id);
        else set.add(c.id);
        o.disabledCategories = [...set];
        save();
      }),
      el("span", { className: "cr-dot", "data-category": c.id }),
    ),
  );

  // Dictionnaire personnel
  const dictRows: Node[] = o.userWords.length
    ? o.userWords.map((w) =>
        row(w, null, button("", { variant: "ghost", size: "sm", iconName: "trash", ariaLabel: `Retirer « ${w} »`, title: "Retirer", onClick: () => { o.userWords = o.userWords.filter((x) => x !== w); save(); renderSettings(); } })),
      )
    : [el("div", { className: "cr-empty-row" }, "Aucun mot. Les mots ajoutés ne sont plus signalés comme inconnus.")];
  const add = el("input", { className: "cr-input", placeholder: "Ajouter un mot", "aria-label": "Mot à ajouter au dictionnaire" });
  const addForm = el("form", { className: "cr-add" }, add, el("button", { type: "submit", className: "cr-btn" }, "Ajouter"));
  addForm.addEventListener("submit", (e) => {
    e.preventDefault();
    const w = add.value.trim();
    if (w && !o.userWords.includes(w)) {
      o.userWords.push(w);
      save();
      renderSettings();
    }
  });

  // Règles désactivées
  const ruleRows: Node[] = o.disabledRules.length
    ? o.disabledRules.map((r) =>
        row(state.ruleNames.get(r) ?? r, r, button("Réactiver", { size: "sm", onClick: () => { o.disabledRules = o.disabledRules.filter((x) => x !== r); save(); renderSettings(); } })),
      )
    : [el("div", { className: "cr-empty-row" }, "Aucune. « Désactiver cette règle » sur une suggestion l'ajoute ici.")];

  ui.sections.reglages.replaceChildren(
    group("Vérifications", ...categories),
    group(
      "Typographie",
      boolRow("Espace fine avant ; ! ?", "Sinon, une espace insécable normale est proposée. Les deux sont acceptées.", "narrowNbsp"),
      boolRow("Apostrophe typographique", "Propose ’ à la place de l'apostrophe droite '.", "typographicApostrophe"),
      boolRow("Guillemets français", "Propose « » à la place des guillemets droits ou anglais.", "frenchQuotes"),
      boolRow("Points de suspension", "Propose le caractère … à la place de trois points.", "ellipsisCharacter"),
    ),
    group(
      "Style",
      numberRow("Phrase longue au-delà de", "Nombre de mots (catégorie Style).", "longSentenceWords", 10, 200),
      numberRow("Répétition à moins de", "Nombre de mots entre deux emplois du même mot.", "repetitionWindow", 2, 500),
    ),
    group("Orthographe", boolRow("Vérifier les noms propres", "Les mots à majuscule en milieu de phrase sont sinon ignorés.", "checkProperNouns")),
    group("Dictionnaire personnel", ...dictRows, addForm),
    group("Règles désactivées", ...ruleRows),
    el(
      "section",
      { className: "cr-settings-group cr-about" },
      el("h3", {}, "À propos"),
      el("p", {}, "Correcteur libre du français, sous licence GPL-3.0. L'analyse est faite sur cet ordinateur : aucun texte n'est envoyé."),
      el("p", {}, "Données : Lexique 3.83 et Wiktionnaire (CC BY-SA 4.0), thésaurus de Dicollecte (LGPL)."),
    ),
  );
}

// ---------------------------------------------------------------------------
// Coquille

function showTab(tab: Tab): void {
  for (const t of Object.keys(ui.tabs) as Tab[]) {
    const on = t === tab;
    ui.tabs[t].setAttribute("aria-selected", String(on));
    ui.tabs[t].tabIndex = on ? 0 : -1;
    ui.sections[t].hidden = !on;
  }
}

function buildShell(setup: PanelSetup): void {
  const root = setup.root;
  root.classList.add("cr-panel");
  ui.root = root;

  const head = el("header", { className: "cr-head" }, el("div", { className: "cr-brand" }, logo(22), "Correcteur"), el("div", { className: "cr-spacer" }));
  ui.analyze = null;
  if (setup.host === "word") {
    ui.analyze = button("Analyser", { variant: "primary", iconName: "refresh", onClick: () => void analyze() });
    ui.analyze.disabled = true;
    head.append(ui.analyze);
  }

  const tablist = el("div", { className: "cr-tabs", role: "tablist", "aria-label": "Sections" });
  ui.badge = el("span", { className: "cr-badge", "data-zero": "" }, "0");
  const defs: [Tab, string, IconName][] = [
    ["corrections", "Corrections", "list"],
    ["outils", "Outils", "book"],
    ["reglages", "Réglages", "sliders"],
  ];
  ui.tabs = {} as Record<Tab, HTMLButtonElement>;
  ui.sections = {} as Record<Tab, HTMLElement>;
  const body = el("div", { className: "cr-body" });
  for (const [id, text, ic] of defs) {
    const tabId = uid("tab");
    const panelId = uid("panel");
    const t = el("button", { type: "button", className: "cr-tab", role: "tab", id: tabId, "aria-controls": panelId }, icon(ic, 15), text, id === "corrections" ? ui.badge : null);
    t.addEventListener("click", () => showTab(id));
    t.addEventListener("keydown", (e) => {
      if (e.key !== "ArrowRight" && e.key !== "ArrowLeft") return;
      const order = defs.map((d) => d[0]);
      const next = order[(order.indexOf(id) + (e.key === "ArrowRight" ? 1 : order.length - 1)) % order.length];
      showTab(next);
      ui.tabs[next].focus();
    });
    ui.tabs[id] = t;
    tablist.append(t);
    const s = el("section", { className: "cr-section", role: "tabpanel", id: panelId, "aria-labelledby": tabId });
    ui.sections[id] = s;
    body.append(s);
  }

  ui.toast = el("div", { className: "cr-toast", role: "status", "aria-live": "polite" });
  const foot = el("footer", { className: "cr-foot" }, icon("lock", 13), "Analyse locale : le texte ne quitte pas votre ordinateur.");
  root.replaceChildren(head, tablist, body, foot, ui.toast);
  showTab("corrections");
}

export async function startPanel(setup: PanelSetup): Promise<void> {
  state.setup = setup;
  buildShell(setup);
  renderCorrections();
  renderTools();
  renderSettings();
  try {
    state.engine = await setup.loadChecker();
    state.phase = "ready";
    ui.analyze?.removeAttribute("disabled");
    renderCorrections();
    if (setup.analyzeOnReady) await analyze();
  } catch (e) {
    state.phase = "error";
    state.error = (e as Error).message;
    renderCorrections();
  }
}

/** Options courantes (lecture seule, pour l'hôte). */
export function currentOptions(): Readonly<CheckOptions> {
  return state.options;
}
