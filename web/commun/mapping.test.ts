import assert from "node:assert/strict";
import { test } from "node:test";
import { applyToText, escapeSearch, locate, replacement, searchRank } from "./mapping";

test("mot simple", () => {
  const t = locate("Le chein dort.", { start: 3, end: 8 })!;
  assert.equal(t.snippet, "chein");
  assert.equal(t.occurrence, 0);
  assert.equal(replacement(t, "chien"), "chien");
});

test("espace seule : extrait élargi aux caractères visibles", () => {
  const text = "Vous venez ?";
  const t = locate(text, { start: 10, end: 11 })!;
  assert.equal(t.snippet, "z ?");
  assert.equal(t.prefix, "z");
  assert.equal(t.suffix, "?");
  assert.equal(replacement(t, " "), "z ?");
});

test("plusieurs occurrences : rang de la bonne", () => {
  const text = "Il à faim, il à soif.";
  const t = locate(text, { start: 14, end: 15 })!;
  assert.equal(t.snippet, "à");
  assert.equal(t.occurrence, 1);
});

test("occurrence chevauchante : extrait élargi", () => {
  // « aa » en position 1 de « aaa » est sauté par une recherche sans chevauchement.
  assert.equal(searchRank("aaa", "aa", 1), null);
  const t = locate("xaaay", { start: 2, end: 4 })!;
  assert.equal(searchRank("xaaay", t.snippet, t.start), t.occurrence);
  assert.ok(t.snippet.length > 2);
});

test("extrait vide et bornes", () => {
  assert.equal(locate("abc", { start: 2, end: 5 }), null);
  const t = locate("abc", { start: 3, end: 3 })!;
  assert.equal(t.snippet, "c");
});

test("paire de substitution jamais coupée", () => {
  const text = "😀 !";
  const t = locate(text, { start: 2, end: 3 })!;
  assert.equal(t.snippet, "😀 !");
});

test("échappement de la recherche Word", () => {
  assert.equal(escapeSearch("a^b"), "a^^b");
});

test("application au texte", () => {
  assert.equal(applyToText("Le chein dort.", { start: 3, end: 8 }, "chien"), "Le chien dort.");
});
