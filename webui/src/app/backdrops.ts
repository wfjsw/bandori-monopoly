// The scene backdrops, pre-blurred at build time by `plugins/pluginBlurQuery`
// (the `?blur` import query: sigma, saturation and a pre-blur downscale). The
// plates in `Stage` are plain images; no CSS `filter: blur()` runs at runtime.

import common from "../../public/assets/img/scene/bg_common.webp?sigma=14&sat=1.05&scale=0.5&blur";
import live from "../../public/assets/img/scene/bg_live.webp?sigma=14&sat=1.05&scale=0.5&blur";
import band from "../../public/assets/img/scene/bg_band.webp?sigma=14&sat=1.05&scale=0.5&blur";

const BLURRED: Record<string, string> = { bg_common: common, bg_live: live, bg_band: band };

/** URL of the blurred backdrop `name` (a `scene.img` name), or "" for an unknown one. */
export function backdropUrl(name: string): string {
  return BLURRED[name] ?? "";
}
