// Serveur de développement pour dist/.
// HTTPS si les certificats de développement Office existent
// (créés par `npx office-addin-dev-certs install`), HTTP sinon (aperçu dans un navigateur).
import fs from "node:fs";
import http from "node:http";
import https from "node:https";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dist = path.join(path.dirname(fileURLToPath(import.meta.url)), "dist");
const port = Number(process.env.PORT ?? 3000);
const types = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".tsv": "text/plain; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".md": "text/plain; charset=utf-8",
  ".map": "application/json",
  ".wasm": "application/wasm",
  ".png": "image/png",
  ".xml": "application/xml",
};

function handler(req, res) {
  const url = new URL(req.url, "http://localhost");
  const rel = decodeURIComponent(url.pathname === "/" ? "/taskpane.html" : url.pathname);
  const file = path.normalize(path.join(dist, rel));
  if (!file.startsWith(dist + path.sep) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    res.writeHead(404).end("introuvable");
    return;
  }
  res.writeHead(200, {
    "Content-Type": types[path.extname(file)] ?? "application/octet-stream",
    "Cache-Control": "no-store",
  });
  fs.createReadStream(file).pipe(res);
}

const certDir = path.join(os.homedir(), ".office-addin-dev-certs");
const cert = path.join(certDir, "localhost.crt");
const key = path.join(certDir, "localhost.key");
if (fs.existsSync(cert) && fs.existsSync(key)) {
  https.createServer({ cert: fs.readFileSync(cert), key: fs.readFileSync(key) }, handler).listen(port, "127.0.0.1");
  console.log(`https://localhost:${port}/taskpane.html`);
} else {
  http.createServer(handler).listen(port, "127.0.0.1");
  console.log(`http://localhost:${port}/taskpane.html (sans certificat : aperçu navigateur seulement, Word exige HTTPS)`);
}
