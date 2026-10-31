import { appendFileSync, existsSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig, type RequestHandler } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";
import { pluginBlurQuery } from "./plugins/pluginBlurQuery";

// `npm run dev` proxies the API, the SSE streams and the game data to the Rust
// server (port 8080; `BM_API` overrides the target, e.g. a branch's own server
// on :8090). `npm run build` writes dist/, which the server can serve.
const api = process.env.BM_API ?? "http://127.0.0.1:8080";

// The profile server of this deployment (directory + uploads). Fixed at build
// time; empty = the page's own origin, which hosts the SPA.
const profile = process.env.BM_PROFILE ?? "";

// `BM_VDIAG=<file>` (dev server only): force the viewport readout on
// (app/ViewportDiag) and append every reading a client posts to `/__vdiag` to
// <file>, one JSON line each -- for collecting a real handset's screen
// geometry from a dev server the phone reaches over the LAN (`--host ::`).
const vdiagLog = process.env.BM_VDIAG;
const vdiagSink: RequestHandler = (req, res, next) => {
  if (req.method !== "POST" || !req.url?.startsWith("/__vdiag")) return next();
  let body = "";
  req.on("data", (chunk) => {
    if (body.length < 64_000) body += chunk;
  });
  req.on("end", () => {
    let reading: unknown = body;
    try {
      reading = JSON.parse(body);
    } catch {
      // Keep the raw text.
    }
    appendFileSync(vdiagLog!, JSON.stringify({ at: new Date().toISOString(), from: req.socket.remoteAddress, reading }) + "\n");
    res.statusCode = 204;
    res.end();
  });
};

// Dev only: the engine archive lives in `archive/engine/` + `data/engine-archive/`
// (an `npm run build` copies it into `dist/assets/engine/`, which is what the
// live site serves). Without this the dev server answers `/assets/engine/…`
// with a 403, so "download replay, include engine" fails on every development
// server with "no copy of engine bundle". Read-only; nothing is written.
const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ENGINE_STORES = [join(ROOT, "data/engine-archive"), join(ROOT, "data/engine-cache")];
// Hand-written assets of the same URL space (`replay-worker.js`, `bot-worker.js`,
// `bot-glue/`) live in the public dir; the SPA fallback must not answer them.
const ENGINE_PUBLIC = join(ROOT, "webui/public/assets/engine");
const ENGINE_MIME: Record<string, string> = {
  ".json": "application/json",
  ".js": "text/javascript",
  ".wasm": "application/wasm",
};

const engineSink: RequestHandler = (req, res, next) => {
  const path = (req.url ?? "").split("?")[0];
  const prefix = "/assets/engine/";
  if (req.method !== "GET" || !path.startsWith(prefix)) return next();
  // `/assets/engine/` itself, and a malformed escape, belong to the handlers
  // behind this one -- a miss here must fall through, never throw a 500.
  let rest: string;
  try {
    rest = decodeURIComponent(path.slice(prefix.length));
  } catch {
    return next();
  }
  if (!rest) return next();

  let file: string | null = null;
  if (rest === "index.json") {
    file = join(ROOT, "archive", "engine", "index.json");
  } else {
    const [head, ...tail] = rest.split("/").filter(Boolean);
    const isFile = (p: string) => existsSync(p) && statSync(p).isFile();
    if (!tail.length) {
      file = [...ENGINE_STORES, ENGINE_PUBLIC].map((s) => join(s, head)).find(isFile) ?? null;
    } else {
      const dirs = head === "modules" ? ENGINE_STORES.map((s) => join(s, "modules")) : [...ENGINE_STORES, ENGINE_PUBLIC].map((s) => join(s, head));
      file = dirs.map((d) => join(d, ...tail)).find(isFile) ?? null;
    }
  }
  if (!file || !existsSync(file)) {
    // Anything else (nested public assets) is the built-in static handler's.
    if (!rest.startsWith("index.json")) return next();
    res.statusCode = 404;
    res.end("no engine asset");
    return;
  }
  res.statusCode = 200;
  res.setHeader("content-type", ENGINE_MIME[extname(file)] ?? "application/octet-stream");
  res.end(readFileSync(file));
};

export default defineConfig({
  plugins: [pluginReact(), pluginBlurQuery()],
  source: {
    entry: { index: "./src/index.tsx" },
    define: {
      "import.meta.env.BM_VDIAG": JSON.stringify(vdiagLog ? "1" : ""),
      "import.meta.env.BM_PROFILE": JSON.stringify(profile),
    },
  },
  html: { template: "./index.html", title: "BanG Dream 大富翁" },
  server: {
    port: Number(process.env.BM_PORT ?? 5173),
    compress: false, // keep SSE frames flowing through the proxy
    historyApiFallback: true, // History API routes: /menu, /play/solo, ...
    proxy: {
      "/api": { target: api, changeOrigin: true },
      "/data": { target: api, changeOrigin: true },
      // Rule builds of the dev game server; without this the SPA fallback
      // answers them with HTML and the client wrongly "falls back" to the
      // bundled rules.
      "/builds": { target: api, changeOrigin: true },
    },
    // Registered here, ahead of the built-ins, so the history fallback never
    // answers the readout's POST.
    setup: (ctx) => {
      if (ctx.action !== "dev") return;
      ctx.server.middlewares.use(engineSink);
      if (vdiagLog) ctx.server.middlewares.use(vdiagSink);
    },
  },
  output: { dataUriLimit: 0, cssModules: { localIdentName: "[name]__[local]--[hash:base64:4]" } },
});
