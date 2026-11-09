// A tiny static server for the harness. COOP/COEP make the page cross-origin isolated, which
// gives performance.now() a finer resolution.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { dirname, extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));
const TYPES = { ".html": "text/html", ".mjs": "text/javascript", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json", ".png": "image/png" };

export function serve(port = 0, extraRoots = {}) {
  const server = createServer(async (req, res) => {
    try {
      const url = new URL(req.url, "http://localhost");
      const rel = normalize(decodeURIComponent(url.pathname)).replace(/^(\.\.[/\\])+/, "");
      const base = Object.entries(extraRoots).find(([prefix]) => rel.startsWith(prefix));
      const path = base ? join(base[1], rel.slice(base[0].length)) : join(root, rel);
      if (!base && !path.startsWith(root)) throw new Error("outside root");
      if (rel === "/favicon.ico") { res.writeHead(204); res.end(); return; }
      const body = await readFile(path);
      res.writeHead(200, {
        "content-type": TYPES[extname(path)] ?? "application/octet-stream",
        "cross-origin-opener-policy": "same-origin",
        "cross-origin-embedder-policy": "require-corp",
        "cross-origin-resource-policy": "same-origin",
      });
      res.end(body);
    } catch {
      res.writeHead(404);
      res.end("not found");
    }
  });
  return new Promise((resolve) => server.listen(port, "127.0.0.1", () => resolve(server)));
}
