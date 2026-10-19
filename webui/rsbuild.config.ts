import { appendFileSync } from "node:fs";
import { defineConfig, type RequestHandler } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";
import { pluginBlurQuery } from "./plugins/pluginBlurQuery";

// `npm run dev` proxies the API, the SSE streams and the game data to the Rust
// server (port 8080; `BM_API` overrides the target, e.g. a branch's own server
// on :8090). `npm run build` writes dist/, which the server can serve.
const api = process.env.BM_API ?? "http://127.0.0.1:8080";

// `BM_VDIAG=<file>` (dev server only): force the viewport readout on
// (app/ViewportDiag) and append every reading a client posts to `/__vdiag` to
// <file>, one JSON line each -- for collecting a real handset's screen
// geometry from a dev server the phone reaches over the LAN.
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

export default defineConfig({
  plugins: [pluginReact(), pluginBlurQuery()],
  source: {
    entry: { index: "./src/index.tsx" },
    define: { "import.meta.env.BM_VDIAG": JSON.stringify(vdiagLog ? "1" : "") },
  },
  dev: vdiagLog ? { setupMiddlewares: [(middlewares) => middlewares.unshift(vdiagSink)] } : {},
  html: { template: "./index.html", title: "BanG Dream 大富翁" },
  server: {
    port: Number(process.env.BM_PORT ?? 5173),
    compress: false, // keep SSE frames flowing through the proxy
    historyApiFallback: true, // History API routes: /menu, /play/solo, ...
    proxy: {
      "/api": { target: api, changeOrigin: true },
      "/data": { target: api, changeOrigin: true },
    },
  },
  output: { dataUriLimit: 0, cssModules: { localIdentName: "[name]__[local]--[hash:base64:4]" } },
});
