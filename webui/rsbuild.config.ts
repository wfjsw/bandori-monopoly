import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";

// `npm run dev` proxies the API, the SSE streams and the game data to the Rust
// server (port 8080; `BM_API` overrides the target, e.g. a branch's own server
// on :8090). `npm run build` writes dist/, which the server can serve.
const api = process.env.BM_API ?? "http://127.0.0.1:8080";
export default defineConfig({
  plugins: [pluginReact()],
  source: { entry: { index: "./src/index.tsx" } },
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
