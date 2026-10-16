// Garde-fous sur le manifeste, que Word refuse en bloc au moindre écart.
// La validation complète se fait avec `npx office-addin-manifest validate dist/manifest.xml`.
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const manifest = fs.readFileSync(
  path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../manifest.xml"),
  "utf8",
);

test("aucun double tiret dans les commentaires XML", () => {
  for (const [, body] of manifest.matchAll(/<!--([\s\S]*?)-->/g)) {
    assert.ok(!body.includes("--"), `commentaire invalide : ${body.trim().slice(0, 60)}`);
  }
});

test("version du manifeste au moins 1.0", () => {
  const v = /<Version>(\d+)\.\d+\.\d+\.\d+<\/Version>/.exec(manifest);
  assert.ok(v, "élément Version absent ou mal formé");
  assert.ok(Number(v[1]) >= 1, `version ${v[0]}`);
});

test("chaque ressource référencée est définie", () => {
  const defined = new Set([...manifest.matchAll(/<bt:\w+ id="([^"]+)"/g)].map((m) => m[1]));
  for (const [, id] of manifest.matchAll(/resid="([^"]+)"/g)) {
    assert.ok(defined.has(id), `ressource « ${id} » non définie`);
    assert.ok(id.length <= 32, `identifiant de ressource trop long : ${id}`);
  }
});
