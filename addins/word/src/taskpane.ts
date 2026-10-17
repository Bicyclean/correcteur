// Add-in Word : panneau latéral. Moteur en WebAssembly, document Word par
// Office.js ; hors de Word, mode démonstration sur une zone de texte.

import { EditorDocument } from "../../../web/commun/document";
import { startPanel } from "../../../web/commun/panneau";
import { type Checker } from "../../../web/commun/types";
import { Engine } from "./engine";
import { WordDocument } from "./word";

const DEMO_TEXT = [
  "Il à mangé du pain , puis il ce lave les mains.",
  "Quelle belle journée! Nous partons maintenat a cause de la pluie.",
  "Je ne sais pas d'ou il vient... Mon frère - le plus jeune - arrive demain.",
].join("\n");

async function fetchText(url: string): Promise<string> {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${url} : ${r.status}`);
  return r.text();
}

async function loadChecker(): Promise<Checker> {
  const index = JSON.parse(await fetchText("data/index.json")) as string[];
  const files: Record<string, string> = {};
  await Promise.all(index.map(async (f) => (files[f] = await fetchText(`data/${f}`))));
  const wasm = await (await fetch("correcteur.wasm")).arrayBuffer();
  const engine = await Engine.load(wasm, files);
  return {
    forms: engine.forms,
    check: async (text, options) => engine.check(text, options),
    conjugate: async (verb) => engine.conjugate(verb),
    synonyms: async (word) => engine.synonyms(word),
    readability: async (text) => engine.readability(text),
  };
}

function start(inWord: boolean): void {
  const root = document.getElementById("app") as HTMLElement;
  if (inWord) {
    void startPanel({ root, doc: new WordDocument(), loadChecker, host: "word", analyzeOnReady: false });
    return;
  }
  const area = document.getElementById("demo-text") as HTMLTextAreaElement;
  area.value = DEMO_TEXT;
  (document.getElementById("demo") as HTMLElement).hidden = false;
  document.body.classList.add("is-demo");
  const doc = new EditorDocument(area);
  void startPanel({ root, doc, loadChecker, host: "word", analyzeOnReady: true, undo: () => doc.undo() });
}

// Hors de Word (navigateur), Office.js est absent ou `host` est nul : mode démonstration.
if (typeof Office !== "undefined") {
  Office.onReady((info) => start(info.host === Office.HostType.Word));
} else {
  start(false);
}
