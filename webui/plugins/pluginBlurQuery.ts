// Rsbuild plugin: `import url from "./bg.webp?sigma=14&sat=1.05&scale=0.5&blur"`
// emits a blurred copy of the image as a hashed asset and resolves to its URL.
// The blur is done once at build time, so the browser composites a plain image
// instead of re-running a CSS `filter: blur()` over a viewport-sized layer.
//
// Query: `sigma` in source px (default 8), `sat` saturation multiplier,
// `scale` downscale factor applied before blurring (default 1).

import type { RsbuildPlugin } from "@rsbuild/core";

export const pluginBlurQuery = (): RsbuildPlugin => ({
  name: "plugin-blur-query",
  setup(api) {
    api.modifyBundlerChain((chain, { CHAIN_ID }) => {
      const loader = new URL("./blurLoader.mjs", import.meta.url).pathname;
      // The stock image rule must leave `?blur` imports to the rule below.
      chain.module.rule(CHAIN_ID.RULE.IMAGE).resourceQuery({ not: [/[?&]blur$/] });
      chain.module
        .rule("blur-query")
        .test(/\.(webp|png|jpe?g)$/i)
        .resourceQuery(/[?&]blur$/)
        .type("asset/resource")
        .generator({ filename: "static/image/[name].[contenthash:8].webp" })
        .use("blur")
        .loader(loader);
    });
  },
});
