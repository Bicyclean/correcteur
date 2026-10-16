// Application de bureau : éditeur avec erreurs surlignées, panneau commun,
// correction du presse-papiers. Le moteur natif est appelé par les commandes
// Tauri définies dans src-tauri/src/main.rs.

import { EditorDocument } from "../../../web/commun/document";
import { type Found, analyze, reveal, startPanel, setStatus, visible } from "../../../web/commun/panneau";
import { type CheckOptions, type Checker, type Issue } from "../../../web/commun/types";

interface TauriGlobal {
  core: { invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> };
  event: { listen<T>(event: string, handler: (e: { payload: T }) => void): Promise<() => void> };
}

const tauri = (window as unknown as { __TAURI__?: TauriGlobal }).__TAURI__;
const area = document.getElementById("texte") as HTMLTextAreaElement;
const backdrop = document.getElementById("backdrop") as HTMLElement;
const isMac = navigator.userAgent.includes("Mac");
const mod = isMac ? "⌘" : "Ctrl";

function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!tauri) return Promise.reject(new Error("Tauri indisponible (page ouverte hors de l'application)"));
  return tauri.core.invoke<T>(cmd, args);
}

const checker = async (): Promise<Checker> => ({
  forms: await invoke<number>("infos"),
  check: (text: string, options: CheckOptions) => invoke<Issue[]>("analyser", { texte: text, options }),
});

// ---------------------------------------------------------------------------
// Surlignage : le calque reproduit le texte, les erreurs y sont marquées.

function paint(found: Found[]): void {
  const text = area.value;
  const lines = text.split("\n");
  const starts: number[] = [];
  let at = 0;
  for (const l of lines) {
    starts.push(at);
    at += l.length + 1;
  }
  // Une analyse antérieure à la dernière frappe ne correspond plus au texte.
  const current = found.filter((f) => lines[f.paragraph] === f.text);
  const spans = current
    .map((f) => ({
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
    mark.textContent = text.slice(s.start, Math.min(s.end, text.length)) || " ";
    nodes.push(mark);
    pos = s.end;
  }
  // Le saut de ligne final garde au calque la même hauteur que la zone de texte.
  nodes.push(document.createTextNode(text.slice(pos) + "\n"));
  backdrop.replaceChildren(...nodes);
  backdrop.scrollTop = area.scrollTop;
}

area.addEventListener("scroll", () => {
  backdrop.scrollTop = area.scrollTop;
});

// Analyse automatique après une courte pause de frappe.
let timer: ReturnType<typeof setTimeout> | undefined;
area.addEventListener("input", () => {
  paint([]);
  clearTimeout(timer);
  timer = setTimeout(() => void analyze(), 400);
});

// ---------------------------------------------------------------------------
// Presse-papiers

function load(text: string): void {
  area.value = text;
  area.focus();
  area.setSelectionRange(0, 0);
  paint([]);
  void analyze();
}

async function copy(hide: boolean): Promise<void> {
  try {
    await invoke(hide ? "copier_et_masquer" : "ecrire_presse_papiers", { texte: area.value });
    if (!hide) setStatus("Texte copié dans le presse-papiers.");
  } catch (e) {
    setStatus(`Copie impossible : ${(e as Error).message}`, "error");
  }
}

(document.getElementById("coller") as HTMLButtonElement).onclick = async () => {
  try {
    load(await invoke<string>("lire_presse_papiers"));
  } catch (e) {
    setStatus(`Presse-papiers illisible : ${(e as Error).message}`, "error");
  }
};
(document.getElementById("copier") as HTMLButtonElement).onclick = () => void copy(false);
(document.getElementById("copier-masquer") as HTMLButtonElement).onclick = () => void copy(true);

void tauri?.event.listen<string>("texte-presse-papiers", (e) => load(e.payload));

// ---------------------------------------------------------------------------
// Raccourcis dans la fenêtre

let current = -1;
document.addEventListener("keydown", (e) => {
  const withMod = isMac ? e.metaKey : e.ctrlKey;
  if (withMod && e.key === "Enter") {
    e.preventDefault();
    void copy(true);
  } else if (withMod && e.key.toLowerCase() === "j") {
    // Problème suivant (Maj : précédent), sélectionné dans le texte.
    e.preventDefault();
    const items = visible();
    if (items.length === 0) return;
    current = (current + (e.shiftKey ? -1 : 1) + items.length) % items.length;
    void reveal(items[current]);
  } else if (e.key === "Escape" && document.activeElement === area) {
    void invoke("masquer");
  }
});

(document.getElementById("raccourcis") as HTMLElement).innerHTML = [
  `<kbd>${mod}</kbd><kbd>⇧</kbd><kbd>E</kbd> corriger le presse-papiers, depuis n'importe quelle application`,
  `<kbd>${mod}</kbd><kbd>↩</kbd> copier et fermer`,
  `<kbd>${mod}</kbd><kbd>J</kbd> problème suivant`,
].join(" · ");

void startPanel({
  doc: new EditorDocument(area),
  loadChecker: checker,
  analyzeOnReady: true,
  onAnalyzed: (found) => {
    current = -1;
    paint(found);
  },
});
