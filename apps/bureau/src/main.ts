// Application de bureau : éditeur avec erreurs soulignées, panneau commun,
// correction du presse-papiers. Le moteur natif est appelé par les commandes
// Tauri définies dans src-tauri/src/main.rs.

import { EditorDocument } from "../../../web/commun/document";
import { icon } from "../../../web/commun/icones";
import { type Found, activateAt, analyze, notify, startPanel, step } from "../../../web/commun/panneau";
import {
  type CheckOptions,
  type Checker,
  type Conjugation,
  type Issue,
  type Readability,
  type Synonyms,
} from "../../../web/commun/types";

interface TauriGlobal {
  core: { invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> };
  event: { listen<T>(event: string, handler: (e: { payload: T }) => void): Promise<() => void> };
}

const tauri = (window as unknown as { __TAURI__?: TauriGlobal }).__TAURI__;
const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const area = $<HTMLTextAreaElement>("texte");
const backdrop = $("backdrop");
const isMac = /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);
const MOD = isMac ? "⌘" : "Ctrl";

const EXAMPLE = [
  "Hier soir, les enfants on visité le musé du village. Ils étaient tous très content de découvrir les tableaux anciens!",
  "",
  "La guide nous a expliqué que le batiment a été construit au XVIIe siècle. Je sais pas si j'y retournerai, mais c'est une belle sortie pour toute la famille.",
  "",
  "Il à fallu partir trop tôt : le bus nous attendait devant la mairie a cause de la pluie.",
].join("\n");

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!tauri) return Promise.reject(new Error("Tauri indisponible (page ouverte hors de l'application)"));
  return tauri.core.invoke<T>(cmd, args);
}

const checker = async (): Promise<Checker> => ({
  forms: await invoke<number>("infos"),
  check: (text: string, options: CheckOptions) => invoke<Issue[]>("analyser", { texte: text, options }),
  conjugate: (verb: string) => invoke<Conjugation | null>("conjuguer", { verbe: verb }),
  synonyms: (word: string) => invoke<Synonyms | null>("synonymes", { mot: word }),
  readability: (text: string) => invoke<Readability>("lisibilite", { texte: text }),
});

// ---------------------------------------------------------------------------
// Boutons et textes d'interface

function kbd(...keys: string[]): HTMLElement[] {
  return keys.map((k) => Object.assign(document.createElement("kbd"), { textContent: k }));
}

function fill(id: string, iconName: Parameters<typeof icon>[0], text: string, ...extra: Node[]): void {
  const b = $(id);
  b.setAttribute("aria-label", text);
  const span = Object.assign(document.createElement("span"), { className: "label", textContent: text });
  b.replaceChildren(icon(iconName), span, ...extra);
}

fill("coller", "clipboard", "Coller");
fill("copier", "copy", "Copier");
fill("copier-masquer", "send", "Copier et fermer", ...kbd(MOD, "↩"));
fill("w-coller", "clipboard", "Coller le presse-papiers");
$("w-tip").replaceChildren("Depuis n'importe quelle application : copiez un texte puis ", ...kbd(MOD, "⇧", "E"), " pour l'ouvrir ici.");

const shortcut = (keys: string[], text: string) => {
  const s = document.createElement("span");
  s.append(...kbd(...keys), ` ${text}`);
  return s;
};
$("raccourcis").replaceChildren(
  shortcut([MOD, "⇧", "E"], "corriger le presse-papiers"),
  shortcut([MOD, "J"], "problème suivant"),
  shortcut([MOD, "↩"], "copier et fermer"),
  shortcut([MOD, "Z"], "annuler"),
);

// ---------------------------------------------------------------------------
// Statistiques et accueil

function updateChrome(): void {
  const text = area.value;
  const words = text.split(/\s+/).filter(Boolean).length;
  $("stats").textContent = words === 0 ? "Nouveau texte" : `${words.toLocaleString("fr-FR")} mot${words > 1 ? "s" : ""} · ${text.length.toLocaleString("fr-FR")} caractères`;
  $("welcome").hidden = text.length > 0;
}

// ---------------------------------------------------------------------------
// Soulignements : le calque reproduit le texte, les erreurs y sont marquées.

let lastFound: Found[] = [];
let activeKey: string | null = null;

function paragraphStarts(lines: string[]): number[] {
  const starts: number[] = [];
  let at = 0;
  for (const l of lines) {
    starts.push(at);
    at += l.length + 1;
  }
  return starts;
}

function paint(found: Found[]): void {
  lastFound = found;
  const text = area.value;
  const lines = text.split("\n");
  const starts = paragraphStarts(lines);
  // Une analyse antérieure à la dernière frappe ne correspond plus au texte.
  const spans = found
    .filter((f) => lines[f.paragraph] === f.text)
    .map((f) => ({
      key: f.key,
      start: starts[f.paragraph] + f.issue.start,
      end: starts[f.paragraph] + Math.max(f.issue.end, f.issue.start + 1),
      category: f.issue.category,
    }))
    .sort((a, b) => a.start - b.start);

  const nodes: Node[] = [];
  let pos = 0;
  for (const s of spans) {
    if (s.start < pos) continue; // chevauchement : garder le premier
    nodes.push(document.createTextNode(text.slice(pos, s.start)));
    const mark = document.createElement("mark");
    mark.dataset.category = s.category;
    mark.dataset.key = s.key;
    if (s.key === activeKey) mark.classList.add("is-active");
    mark.textContent = text.slice(s.start, Math.min(s.end, text.length)) || " ";
    nodes.push(mark);
    pos = s.end;
  }
  // Le saut de ligne final garde au calque la même hauteur que la zone de texte.
  nodes.push(document.createTextNode(text.slice(pos) + "\n"));
  backdrop.replaceChildren(...nodes);
  backdrop.scrollTop = area.scrollTop;
}

/** Problème actif : surligné, et amené dans la zone visible. */
function onActivate(f: Found | null): void {
  activeKey = f?.key ?? null;
  let target: HTMLElement | null = null;
  for (const m of backdrop.querySelectorAll<HTMLElement>("mark")) {
    const on = m.dataset.key === activeKey;
    m.classList.toggle("is-active", on);
    if (on) target = m;
  }
  if (target) {
    const top = target.offsetTop;
    const view = area.clientHeight;
    if (top < area.scrollTop + 40 || top > area.scrollTop + view - 80) {
      area.scrollTo({ top: Math.max(0, top - view / 3), behavior: "smooth" });
    }
  }
}

area.addEventListener("scroll", () => {
  backdrop.scrollTop = area.scrollTop;
});

// Analyse pendant la frappe, après une courte pause.
let timer: ReturnType<typeof setTimeout> | undefined;
area.addEventListener("input", () => {
  updateChrome();
  paint([]);
  clearTimeout(timer);
  timer = setTimeout(() => void analyze(), 350);
});

// Clic dans le texte : la carte du problème sous le curseur s'ouvre.
function caretToIssue(): void {
  const pos = area.selectionStart;
  if (area.selectionEnd !== pos) return;
  const before = area.value.slice(0, pos).split("\n");
  activateAt(before.length - 1, before[before.length - 1].length);
}
area.addEventListener("click", caretToIssue);
area.addEventListener("keyup", (e) => {
  if (e.key.startsWith("Arrow")) caretToIssue();
});

// ---------------------------------------------------------------------------
// Presse-papiers

function load(text: string): void {
  area.value = text;
  area.focus();
  area.setSelectionRange(0, 0);
  area.scrollTop = 0;
  updateChrome();
  paint([]);
  void analyze();
}

async function paste(): Promise<void> {
  try {
    const text = await invoke<string>("lire_presse_papiers");
    if (!text.trim()) {
      notify("Le presse-papiers ne contient pas de texte.");
      return;
    }
    load(text);
  } catch (e) {
    notify(`Presse-papiers illisible : ${(e as Error).message}`);
  }
}

async function copy(hide: boolean): Promise<void> {
  if (!area.value) {
    notify("Rien à copier : le texte est vide.");
    return;
  }
  try {
    await invoke(hide ? "copier_et_masquer" : "ecrire_presse_papiers", { texte: area.value });
    if (!hide) notify("Texte copié dans le presse-papiers.");
  } catch (e) {
    notify(`Copie impossible : ${(e as Error).message}`);
  }
}

$("coller").onclick = () => void paste();
$("w-coller").onclick = () => void paste();
$("w-exemple").onclick = () => load(EXAMPLE);
$("copier").onclick = () => void copy(false);
$("copier-masquer").onclick = () => void copy(true);

void tauri?.event.listen<string>("texte-presse-papiers", (e) => load(e.payload));

// ---------------------------------------------------------------------------
// Raccourcis de la fenêtre

document.addEventListener("keydown", (e) => {
  const withMod = isMac ? e.metaKey : e.ctrlKey;
  if (withMod && e.key === "Enter") {
    e.preventDefault();
    void copy(true);
  } else if (withMod && e.key.toLowerCase() === "j") {
    e.preventDefault();
    step(e.shiftKey ? -1 : 1, true);
  } else if (e.key === "Escape" && document.activeElement === area) {
    void invoke("masquer").catch(() => undefined);
  }
});

const doc = new EditorDocument(area);
updateChrome();
void startPanel({
  root: $("app"),
  doc,
  loadChecker: checker,
  host: "bureau",
  analyzeOnReady: true,
  undo: () => doc.undo(),
  onAnalyzed: paint,
  onActivate,
});

// Si le moteur répond avant la première peinture, les soulignements suivent.
window.addEventListener("resize", () => paint(lastFound));
