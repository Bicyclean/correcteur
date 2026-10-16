// Construit l'interface de l'application dans dist/ (lancé par `tauri dev` et `tauri build`).
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import * as esbuild from "esbuild";

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, "../..");
const dist = path.join(here, "dist");
fs.mkdirSync(dist, { recursive: true });

await esbuild.build({
  entryPoints: [path.join(here, "src/main.ts")],
  outfile: path.join(dist, "main.js"),
  bundle: true,
  format: "esm",
  target: "es2020",
  minify: true,
  logLevel: "warning",
});
fs.copyFileSync(path.join(here, "src/index.html"), path.join(dist, "index.html"));
fs.copyFileSync(path.join(here, "src/bureau.css"), path.join(dist, "bureau.css"));
fs.copyFileSync(path.join(root, "web/commun/panneau.css"), path.join(dist, "panneau.css"));
fs.copyFileSync(path.join(here, "src-tauri/icons/32x32.png"), path.join(dist, "icone-32.png"));
console.log("interface construite dans dist/");
