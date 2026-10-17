// Application de bureau : éditeur avec erreurs soulignées, panneau commun,
// documents texte, correction du presse-papiers. Le moteur natif et les
// fichiers passent par les commandes Tauri définies dans src-tauri/src/main.rs.

import { EditorDocument } from "../../../web/commun/document";
import { icon } from "../../../web/commun/icones";
import { type Found, type HostSetting, activateAt, analyze, errorText, notify, startPanel, step } from "../../../web/commun/panneau";
import { type SettingsStore, localStore, sanitize } from "../../../web/commun/settings";
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
const doc = new EditorDocument(area);
const platform = navigator.platform || navigator.userAgent;
const isMac = /Mac|iPhone|iPad/.test(platform);
const isWindows = /Win/.test(platform);
const MOD = isMac ? "⌘" : "Ctrl";
const TRAY = isMac ? "la barre des menus" : "la zone de notification";

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

/** Réglages dans des fichiers ; ceux d'une version précédente (stockage local) sont repris. */
const fileStore: SettingsStore = {
  async load() {
    const saved = await invoke<unknown>("lire_reglages");
    if (saved !== null) return saved;
    const old = await localStore.load();
    if (old !== null) await invoke("ecrire_reglages", { reglages: sanitize(old) }).catch(() => undefined);
    return old;
  },
  save: (o) => invoke("ecrire_reglages", { reglages: o }),
};

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

fill("ouvrir", "folder", "Ouvrir");
fill("enregistrer", "save", "Enregistrer");
fill("coller", "clipboard", "Coller");
fill("copier", "copy", "Copier");
fill("copier-masquer", "send", "Copier et fermer", ...kbd(MOD, "↩"));
fill("w-coller", "clipboard", "Coller le presse-papiers");
fill("w-ouvrir", "folder", "Ouvrir un fichier");
$("ouvrir").title = `Ouvrir un fichier texte (${MOD}O)`;
$("enregistrer").title = `Enregistrer (${MOD}S)`;
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
  shortcut([MOD, "S"], "enregistrer"),
  shortcut([MOD, "Z"], "annuler"),
);

// ---------------------------------------------------------------------------
// Document courant

type Encoding = "utf-8" | "utf-8-bom" | "windows-1252";

/** Fichier associé au texte de l'éditeur. */
interface FileInfo {
  chemin: string;
  nom: string;
  crlf: boolean;
  encodage: Encoding;
}

interface OpenedJs extends FileInfo {
  texte: string;
}

interface SavedJs {
  chemin: string;
  nom: string;
  encodage: Encoding;
}

const current = {
  file: null as FileInfo | null,
  /** Texte tel qu'enregistré dans le fichier ; `null` si inconnu (à enregistrer). */
  saved: null as string | null,
};

function modified(): boolean {
  return current.file !== null && area.value !== current.saved;
}

let lastTitle = "";
function updateChrome(): void {
  const text = area.value;
  const words = text.split(/\s+/).filter(Boolean).length;
  $("stats").textContent = words === 0 ? "" : `${words.toLocaleString("fr-FR")} mot${words > 1 ? "s" : ""} · ${text.length.toLocaleString("fr-FR")} caractères`;
  $("welcome").hidden = text.length > 0 || current.file !== null;

  const name = current.file?.nom ?? "Sans titre";
  const changed = modified();
  $("doc-nom").textContent = name;
  $("doc-nom").title = current.file?.chemin ?? "Texte non enregistré dans un fichier";
  $("doc-etat").hidden = !changed;
  const title = `${name}${changed ? " (modifié)" : ""} — Correcteur`;
  if (title !== lastTitle) {
    lastTitle = title;
    void invoke("titre", { titre: title }).catch(() => undefined);
  }
}

// Brouillon : le texte en cours est retrouvé au lancement suivant, même si
// l'application a été quittée sans enregistrer.
interface Draft {
  texte: string;
  fichier: FileInfo | null;
  modifie: boolean;
}

let draftTimer: ReturnType<typeof setTimeout> | undefined;
function saveDraft(now = false): void {
  clearTimeout(draftTimer);
  const write = () => {
    const d: Draft = { texte: area.value, fichier: current.file, modifie: modified() };
    void invoke("ecrire_brouillon", { brouillon: JSON.stringify(d) }).catch(() => undefined);
  };
  if (now) write();
  else draftTimer = setTimeout(write, 400);
}
window.addEventListener("blur", () => saveDraft(true));
document.addEventListener("visibilitychange", () => saveDraft(true));

function isDraft(v: unknown): v is Draft {
  const d = v as Draft;
  return typeof d === "object" && d !== null && typeof d.texte === "string" && typeof d.modifie === "boolean" && (d.fichier === null || typeof d.fichier?.chemin === "string");
}

/** Reprend le brouillon ; renvoie un message à afficher une fois le panneau prêt. */
async function restoreDraft(): Promise<string | null> {
  const d = await invoke<unknown>("lire_brouillon").catch(() => null);
  if (!isDraft(d)) return null;
  if (d.fichier && !d.modifie) {
    // Fichier enregistré : le relire, il a pu changer entre-temps.
    try {
      const f = await invoke<OpenedJs | null>("ouvrir_document", { chemin: d.fichier.chemin });
      if (f) setDocument(f.texte, fileInfo(f), f.texte);
      return null;
    } catch {
      setDocument(d.texte, null, null);
      return `« ${d.fichier.nom} » est introuvable : son texte a été conservé, sans fichier associé.`;
    }
  }
  setDocument(d.texte, d.fichier, null);
  return null;
}

const fileInfo = (f: FileInfo): FileInfo => ({ chemin: f.chemin, nom: f.nom, crlf: f.crlf, encodage: f.encodage });

/** Remplace le texte de l'éditeur (ouverture, nouveau, presse-papiers). */
function setDocument(text: string, file: FileInfo | null, saved: string | null): void {
  area.value = text;
  current.file = file;
  current.saved = saved;
  area.focus();
  area.setSelectionRange(0, 0);
  area.scrollTop = 0;
  updateChrome();
  paint([]);
  saveDraft();
  void analyze();
}

/** Vrai si le texte courant peut être remplacé (rien à perdre, ou l'utilisateur accepte). */
async function confirmDiscard(): Promise<boolean> {
  if (!modified()) return true;
  return invoke<boolean>("confirmer", {
    titre: "Modifications non enregistrées",
    message: `« ${current.file?.nom} » contient des modifications qui n'ont pas été enregistrées. Les abandonner ?`,
    ok: "Abandonner les modifications",
  }).catch(() => false);
}

/**
 * Remplace le document après confirmation. Un texte sans fichier qui serait
 * perdu reste récupérable par « Annuler » dans la notification.
 */
async function replaceDocument(text: string, file: FileInfo | null, message: string | null, confirmed = false): Promise<void> {
  if (!confirmed && !(await confirmDiscard())) return;
  const previous = { text: area.value, file: current.file, saved: current.saved };
  setDocument(text, file, file ? text : null);
  if (!previous.file && previous.text.trim() && previous.text !== text) {
    const notice = message ?? (file ? `« ${file.nom} » ouvert` : "Nouveau texte");
    notify(notice, { label: "Annuler", run: () => setDocument(previous.text, previous.file, previous.saved) });
  } else if (message) {
    notify(message);
  }
}

async function openFile(path: string | null = null): Promise<void> {
  if (!(await confirmDiscard())) return;
  try {
    const f = await invoke<OpenedJs | null>("ouvrir_document", { chemin: path });
    if (f) await replaceDocument(f.texte, fileInfo(f), null, true);
  } catch (e) {
    notify(errorText(e));
  }
}

async function saveFile(saveAs: boolean): Promise<void> {
  const f = current.file;
  const text = area.value;
  try {
    const r = await invoke<SavedJs | null>("enregistrer_document", {
      chemin: saveAs || !f ? null : f.chemin,
      nom: f?.nom ?? null,
      texte: text,
      crlf: f?.crlf ?? isWindows,
      encodage: f?.encodage ?? "utf-8",
    });
    if (!r) return;
    current.file = { chemin: r.chemin, nom: r.nom, crlf: f?.crlf ?? isWindows, encodage: r.encodage };
    current.saved = text;
    updateChrome();
    saveDraft(true);
    notify(f && f.encodage !== r.encodage ? `Enregistré en UTF-8 : certains caractères n'existent pas dans l'encodage d'origine (${f.encodage}).` : `Enregistré : ${r.nom}`);
  } catch (e) {
    notify(errorText(e));
  }
}

const newDocument = () => void replaceDocument("", null, null);

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
  saveDraft();
  // Une correction appliquée par le panneau est réanalysée par le panneau lui-même.
  if (doc.programmatic) return;
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

async function paste(): Promise<void> {
  try {
    const text = await invoke<string>("lire_presse_papiers");
    if (!text.trim()) {
      notify("Le presse-papiers ne contient pas de texte.");
      return;
    }
    await replaceDocument(text, null, "Texte du presse-papiers chargé");
  } catch (e) {
    notify(`Presse-papiers illisible : ${errorText(e)}`);
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
    notify(`Copie impossible : ${errorText(e)}`);
  }
}

$("ouvrir").onclick = () => void openFile();
$("enregistrer").onclick = () => void saveFile(false);
$("coller").onclick = () => void paste();
$("w-coller").onclick = () => void paste();
$("w-ouvrir").onclick = () => void openFile();
$("w-exemple").onclick = () => void replaceDocument(EXAMPLE, null, null);
$("copier").onclick = () => void copy(false);
$("copier-masquer").onclick = () => void copy(true);

void tauri?.event.listen<string>("texte-presse-papiers", (e) => void replaceDocument(e.payload, null, "Texte du presse-papiers chargé"));
void tauri?.event.listen<string>("fichier-depose", (e) => void openFile(e.payload));
// Menu Fichier (macOS).
void tauri?.event.listen<string>("menu", (e) => {
  const actions: Record<string, () => void> = {
    nouveau: newDocument,
    ouvrir: () => void openFile(),
    enregistrer: () => void saveFile(false),
    "enregistrer-sous": () => void saveFile(true),
  };
  actions[e.payload]?.();
});

// ---------------------------------------------------------------------------
// Raccourcis de la fenêtre

document.addEventListener("keydown", (e) => {
  const withMod = isMac ? e.metaKey : e.ctrlKey;
  if (!withMod) {
    if (e.key === "Escape" && document.activeElement === area) void invoke("masquer").catch(() => undefined);
    return;
  }
  const key = e.key.toLowerCase();
  // Sous macOS, Nouveau, Ouvrir et Enregistrer passent par la barre de menus.
  const fileActions: Record<string, () => void> = isMac
    ? {}
    : { n: newDocument, o: () => void openFile(), s: () => void saveFile(e.shiftKey) };
  if (key === "enter") void copy(true);
  else if (key === "j") step(e.shiftKey ? -1 : 1, true);
  else if (fileActions[key]) fileActions[key]();
  else return;
  e.preventDefault();
});

// ---------------------------------------------------------------------------
// Démarrage

interface UpdateInfo {
  version: string;
  notes: string | null;
}

interface AppInfo {
  version: string;
  raccourci: boolean;
  demarrageAuto: boolean;
  dossierReglages: string;
  misesAJourAuto: boolean;
  miseAJour: UpdateInfo | null;
}

// ---------------------------------------------------------------------------
// Mises à jour : recherche sur demande, ou automatique si l'utilisateur l'a
// activée ; installation toujours sur confirmation.

/** Propose d'installer une version trouvée (la notification reste 20 s). */
function offerUpdate(u: UpdateInfo): void {
  notify(`La version ${u.version} est disponible.`, { label: "Installer", run: () => void installUpdate(u) }, 20000);
}

async function installUpdate(u: UpdateInfo): Promise<void> {
  // Le brouillon d'abord : l'application redémarre après l'installation.
  clearTimeout(draftTimer);
  const d: Draft = { texte: area.value, fichier: current.file, modifie: modified() };
  await invoke("ecrire_brouillon", { brouillon: JSON.stringify(d) }).catch(() => undefined);
  notify(`Téléchargement de la version ${u.version}… L'application redémarrera seule.`, undefined, 60000);
  try {
    await invoke("installer_mise_a_jour");
  } catch (e) {
    notify(errorText(e));
  }
}

async function checkForUpdate(): Promise<void> {
  notify("Recherche des mises à jour…");
  try {
    const u = await invoke<UpdateInfo | null>("chercher_mise_a_jour");
    if (u) offerUpdate(u);
    else notify("Correcteur est à jour.");
  } catch (e) {
    notify(errorText(e));
  }
}

void tauri?.event.listen<UpdateInfo>("mise-a-jour-disponible", (e) => offerUpdate(e.payload));

async function hostSettings(info: AppInfo | null): Promise<{ title: string; items: HostSetting[] }> {
  const items: HostSetting[] = [
    {
      kind: "switch",
      title: "Ouvrir à l'ouverture de session",
      desc: `Le correcteur démarre discrètement dans ${TRAY}, prêt pour ${MOD}⇧E.`,
      value: info?.demarrageAuto ?? false,
      onChange: (v) => invoke("regler_demarrage_auto", { actif: v }),
    },
    {
      kind: "switch",
      title: "Rechercher les mises à jour automatiquement",
      desc: "Au démarrage, une fois par jour, l'application consulte la page des versions sur GitHub. Aucun texte n'est envoyé ; l'installation demande votre accord.",
      value: info?.misesAJourAuto ?? false,
      onChange: (v) => invoke("regler_mises_a_jour_auto", { actif: v }),
    },
    {
      kind: "action",
      title: "Mises à jour",
      desc: info ? `Version installée : ${info.version}.` : "Rechercher une nouvelle version.",
      label: "Rechercher",
      run: checkForUpdate,
    },
    {
      kind: "action",
      title: "Dossier des réglages",
      desc: "Contient le dictionnaire personnel (dictionnaire.txt), à sauvegarder ou à copier sur un autre poste.",
      label: "Afficher",
      run: () => invoke("afficher_reglages"),
    },
  ];
  if (info && !info.raccourci) {
    items.unshift({
      kind: "note",
      title: `Raccourci ${MOD}⇧E indisponible`,
      desc: `Une autre application l'utilise déjà. Le menu de l'icône, dans ${TRAY}, propose « Corriger le presse-papiers ».`,
    });
  }
  return { title: "Application", items };
}

async function start(): Promise<void> {
  const info = await invoke<AppInfo>("infos_application").catch(() => null);
  const notice = await restoreDraft();
  updateChrome();
  await startPanel({
    root: $("app"),
    doc,
    loadChecker: checker,
    host: "bureau",
    analyzeOnReady: true,
    undo: () => doc.undo(),
    onAnalyzed: paint,
    onActivate,
    store: fileStore,
    hostSettings: await hostSettings(info),
    version: info?.version,
  });
  if (notice) notify(notice);
  // Trouvée par la recherche automatique avant que l'interface soit prête.
  else if (info?.miseAJour) offerUpdate(info.miseAJour);
}

void start();

// Le retour à la ligne change avec la largeur : les soulignements suivent.
window.addEventListener("resize", () => paint(lastFound));
