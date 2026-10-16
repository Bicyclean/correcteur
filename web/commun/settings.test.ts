import assert from "node:assert/strict";
import { test } from "node:test";
import { sanitize } from "./settings";
import { DEFAULT_OPTIONS } from "./types";

test("réglages corrompus : valeurs par défaut", () => {
  assert.deepEqual(sanitize(null), DEFAULT_OPTIONS);
  assert.deepEqual(sanitize("x"), DEFAULT_OPTIONS);
});

test("réglages : seules les valeurs valides sont reprises", () => {
  const o = sanitize({
    narrowNbsp: false,
    frenchQuotes: "non",
    userWords: ["chein", 3],
    disabledCategories: ["typographie", "inconnue"],
    maxSuggestions: 99,
  });
  assert.equal(o.narrowNbsp, false);
  assert.equal(o.frenchQuotes, true);
  assert.deepEqual(o.userWords, ["chein"]);
  assert.deepEqual(o.disabledCategories, ["typographie"]);
  assert.equal(o.maxSuggestions, DEFAULT_OPTIONS.maxSuggestions);
});
