// Static server for a built Hatch (web/dist) in local tests, with the deployment's config.json
// taken from a file:   node web/e2e/serve.mjs <dist> <config.json> [port]
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";

const [dist, configFile, port = "5173"] = process.argv.slice(2);
const types = {
  ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
  ".json": "application/json", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png",
  ".woff": "font/woff", ".woff2": "font/woff2", ".ttf": "font/ttf", ".sig": "application/octet-stream",
};

createServer(async (req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^(\.\.[/\\])+/, "");
  const file = path === "/config.json" ? configFile : join(dist, path === "/" ? "index.html" : path);
  try {
    const body = await readFile(file);
    res.writeHead(200, { "content-type": types[extname(file)] ?? "application/octet-stream", "cache-control": "no-cache" });
    res.end(body);
  } catch {
    res.writeHead(404);
    res.end();
  }
}).listen(Number(port), "127.0.0.1", () => console.log(`serving ${dist} on http://localhost:${port}/`));
