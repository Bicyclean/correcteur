// Tests du module WebAssembly avec les vraies données, et simulation de
// l'application des suggestions telle que la fait l'add-in dans Word.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { DEFAULT_OPTIONS, Engine, encodeOptions } from "../src/engine";
import { applyToText, locate, replacement } from "../../../web/commun/mapping";

// dist-tests/engine.test.mjs → addins/word
const addin = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const dist = path.join(addin, "dist");
let engine: Engine;

before(async () => {
  const index = JSON.parse(fs.readFileSync(path.join(dist, "data/index.json"), "utf8")) as string[];
  const files: Record<string, string> = {};
  for (const f of index) files[f] = fs.readFileSync(path.join(dist, "data", f), "utf8");
  engine = await Engine.load(fs.readFileSync(path.join(dist, "correcteur.wasm")), files);
});

test("lexique complet chargé", () => {
  assert.ok(engine.forms > 100_000, `${engine.forms} formes`);
});

test("positions en UTF-16, utilisables telles quelles en JavaScript", () => {
  const text = "😀 Le chein dort.";
  const [issue] = engine.check(text);
  assert.equal(text.slice(issue.start, issue.end), "chein");
  assert.equal(issue.category, "orthographe");
  assert.equal(issue.suggestions[0], "chien");
});

test("options transmises au moteur", () => {
  assert.equal(engine.check("Le chein dort !").length, 2);
  const o = { ...DEFAULT_OPTIONS, userWords: ["chein"], disabledCategories: ["typographie" as const] };
  assert.equal(engine.check("Le chein dort !", o).length, 0);
  assert.ok(!encodeOptions({ ...DEFAULT_OPTIONS, userWords: ["a\nb"] }).includes("a\nb"));
});

test("erreur de format renvoyée en exception", async () => {
  await assert.rejects(Engine.load(fs.readFileSync(path.join(dist, "correcteur.wasm")), { "regles/regles.tsv": "x" }));
});

/** Recherche Word simulée : occurrences exactes, de gauche à droite, sans chevauchement. */
function wordSearch(text: string, snippet: string): number[] {
  const out: number[] = [];
  for (let i = text.indexOf(snippet); i !== -1; i = text.indexOf(snippet, i + snippet.length)) out.push(i);
  return out;
}

test("chaque suggestion du corpus s'applique au bon endroit via la recherche", () => {
  const root = path.resolve(addin, "../..");
  const dir = path.join(root, "corpus/fr/regles");
  let checked = 0;
  for (const f of fs.readdirSync(dir).filter((f) => f.endsWith(".tsv"))) {
    for (const line of fs.readFileSync(path.join(dir, f), "utf8").split("\n")) {
      if (!line.trim() || line.startsWith("#")) continue;
      const text = line
        .split("\t")[1]
        .replace(/⟦([^→⟧]*)(→[^⟧]*)?⟧/g, "$1")
        .replaceAll("{nbsp}", " ")
        .replaceAll("{fine}", " ");
      for (const issue of engine.check(text)) {
        const s = issue.suggestions[0];
        if (s === undefined) continue;
        const target = locate(text, issue);
        assert.ok(target, `${f} : aucun extrait pour « ${text} »`);
        const at = wordSearch(text, target.snippet)[target.occurrence];
        assert.equal(at, target.start, `${f} : mauvaise occurrence dans « ${text} »`);
        const viaWord = text.slice(0, at) + replacement(target, s) + text.slice(at + target.snippet.length);
        assert.equal(viaWord, applyToText(text, issue, s));
        checked++;
      }
    }
  }
  assert.ok(checked > 50, `${checked} suggestions vérifiées`);
});

test("conjugaison, synonymes et lisibilité", () => {
  const c = engine.conjugate("se souvenir");
  assert.ok(c);
  assert.equal(c.auxiliary, "être");
  assert.equal(c.modes[0].tenses[0].lines[0], "je me souviens");
  assert.equal(engine.conjugate("xyzir"), null);
  const s = engine.synonyms("maisons");
  assert.equal(s?.entry, "maison");
  assert.ok(s?.groups[0].words.includes("demeure"));
  const a = engine.synonyms("chaude");
  assert.equal(a?.entry, "chaud");
  assert.ok(a?.antonyms.some((g) => g.words.includes("froid")));
  const r = engine.readability("Le chat dort. Il rêve.");
  assert.equal(r.words, 5);
  assert.equal(r.sentences, 2);
  assert.ok(r.score > 60);
});

test("style désactivé par défaut, activable", () => {
  const text = "La maison est grande et la maison est ancienne.";
  assert.equal(engine.check(text).length, 0);
  const all = engine.check(text, { ...DEFAULT_OPTIONS, disabledCategories: [] });
  assert.equal(all[0]?.rule, "STYLE_REPETITION");
});
