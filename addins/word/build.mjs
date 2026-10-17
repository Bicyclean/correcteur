// Construction de l'add-in dans dist/ :
//   node build.mjs                 module WebAssembly + panneau + données
//   node build.mjs --sans-wasm     sans recompiler le module (dist/correcteur.wasm doit exister)
//   node build.mjs --url URL       manifeste pointant vers l'adresse de publication
//   node build.mjs --tests         construit aussi les tests dans dist-tests/
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const dist = path.join(here, "dist");
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const urlArg = args.includes("--url") ? args[args.indexOf("--url") + 1] : null;

fs.mkdirSync(path.join(dist, "data"), { recursive: true });

// 1. Module WebAssembly.
const wasmOut = path.join(dist, "correcteur.wasm");
if (!flag("--sans-wasm")) {
  execFileSync("cargo", ["build", "--release", "--target", "wasm32-unknown-unknown", "-p", "correcteur-wasm"], {
    cwd: root,
    stdio: "inherit",
    env: wasmEnv(),
  });
  fs.copyFileSync(path.join(root, "target/wasm32-unknown-unknown/release/correcteur_wasm.wasm"), wasmOut);
} else if (!fs.existsSync(wasmOut)) {
  throw new Error("dist/correcteur.wasm absent : lancez d'abord `node build.mjs`");
}

// 2. Panneau.
await esbuild.build({
  entryPoints: [path.join(here, "src/taskpane.ts")],
  outfile: path.join(dist, "taskpane.js"),
  bundle: true,
  format: "esm",
  target: "es2020",
  minify: true,
  sourcemap: true,
  logLevel: "warning",
});
for (const f of ["taskpane.html", "aide.html"]) {
  fs.copyFileSync(path.join(here, "src", f), path.join(dist, f));
}
// Feuille commune, puis la mise en page propre au volet Word.
fs.writeFileSync(
  path.join(dist, "taskpane.css"),
  fs.readFileSync(path.join(root, "web/commun/panneau.css"), "utf8") + "\n" + fs.readFileSync(path.join(here, "src/word.css"), "utf8"),
);
fs.cpSync(path.join(here, "assets"), path.join(dist, "assets"), { recursive: true });

// 3. Manifeste.
let manifest = fs.readFileSync(path.join(here, "manifest.xml"), "utf8");
if (urlArg) manifest = manifest.replaceAll("https://localhost:3000", urlArg.replace(/\/$/, ""));
fs.writeFileSync(path.join(dist, "manifest.xml"), manifest);

// 4. Données : celles que lit le moteur, et leur liste.
// Toutes les données lues par le moteur (il ignore les fichiers qu'il n'utilise pas).
const dataFiles = ["regles", "conjugaison", "thesaurus", "lexique"].flatMap((dir) =>
  fs
    .readdirSync(path.join(root, "data", dir))
    // Dans lexique/, seuls les .tsv sont des lexiques (lectures-ecartees.txt est un journal).
    .filter((f) => (dir === "lexique" ? /\.tsv$/ : /\.(tsv|txt|dat)$/).test(f) && !f.startsWith("README"))
    .sort()
    .map((f) => `${dir}/${f}`),
);
for (const f of dataFiles) {
  const to = path.join(dist, "data", f);
  fs.mkdirSync(path.dirname(to), { recursive: true });
  fs.copyFileSync(path.join(root, "data", f), to);
}
fs.writeFileSync(path.join(dist, "data/index.json"), JSON.stringify(dataFiles, null, 1));
// Mentions de licence des données, à côté des données.
fs.copyFileSync(path.join(root, "data/lexique/LISEZMOI.md"), path.join(dist, "data/lexique/LISEZMOI.md"));
fs.copyFileSync(path.join(root, "data/thesaurus/README_thes_fr.txt"), path.join(dist, "data/thesaurus/README_thes_fr.txt"));

// 5. Tests (Node).
if (flag("--tests")) {
  const testDirs = [path.join(here, "test"), path.join(root, "web/commun")];
  const tests = testDirs.flatMap((d) =>
    fs.readdirSync(d).filter((f) => f.endsWith(".test.ts")).map((f) => path.join(d, f)),
  );
  const outdir = path.join(here, "dist-tests");
  // Repartir de zéro : un test supprimé ou déplacé ne doit pas continuer à tourner.
  fs.rmSync(outdir, { recursive: true, force: true });
  await esbuild.build({
    // Tous les tests au même niveau : dist-tests/<nom>.test.mjs
    entryPoints: Object.fromEntries(tests.map((t) => [path.basename(t, ".ts"), t])),
    outdir,
    outExtension: { ".js": ".mjs" },
    bundle: true,
    platform: "node",
    format: "esm",
    target: "node20",
    logLevel: "warning",
  });
}
console.log(`add-in construit dans ${path.relative(process.cwd(), dist) || "."}${urlArg ? ` pour ${urlArg}` : ""}`);

/** Environnement de compilation : préfère la chaîne rustup, qui a la cible wasm32. */
function wasmEnv() {
  const candidates = ["rustup", "/opt/homebrew/opt/rustup/bin/rustup", "/usr/local/opt/rustup/bin/rustup"];
  for (const rustup of candidates) {
    for (const extra of [[], ["--toolchain", "stable"]]) {
      try {
        const rustc = execFileSync(rustup, ["which", "rustc", ...extra], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
        return { ...process.env, PATH: `${path.dirname(rustc)}${path.delimiter}${process.env.PATH}` };
      } catch {
        // essayer la suite
      }
    }
  }
  return process.env;
}
