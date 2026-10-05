import { defineConfig } from "@rsbuild/core";
import { pluginReact } from "@rsbuild/plugin-react";

// `npm run dev` proxies the API, the SSE streams and the game data to the Rust
// server (port 8080). `npm run build` writes dist/, which the server can serve.
export default defineConfig({
  plugins: [pluginReact()],
  source: { entry: { index: "./src/index.tsx" } },
  html: { template: "./index.html", title: "BanG Dream 大富翁" },
  server: {
    port: 5173,
    compress: false, // keep SSE frames flowing through the proxy
    historyApiFallback: true, // History API routes: /menu, /play/solo, ...
    proxy: {
      "/api": { target: "http://127.0.0.1:8080", changeOrigin: true },
      "/data": { target: "http://127.0.0.1:8080", changeOrigin: true },
    },
  },
  output: { dataUriLimit: 0, cssModules: { localIdentName: "[name]__[local]--[hash:base64:4]" } },
});
