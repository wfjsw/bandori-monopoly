// Scene preloading: fetch + decode a scene's images (and its code chunk)
// before the shell swaps to it, so the new scene paints complete instead of
// popping in piece by piece under the lifting fader.

import type { Route } from "../app/router";
import { backdropUrl } from "../app/backdrops";
import { sceneImg } from "./assets";

type Name = Route["name"];

const BACKDROP: Record<Name, string> = { menu: "bg_common", lobby: "bg_live", room: "bg_live", play: "bg_band", gallery: "bg_band", deck: "bg_band", replay: "bg_common", replayView: "bg_common" };

/** The scene images each screen paints on first frame (names in the asset manifest). */
const IMAGES: Partial<Record<Name, string[]>> = {
  menu: ["pic_story", "pic_single", "pic_group", "deco_star_stripes", "icon_fire", "icon_medal", "logo_game", "badge_new"],
  lobby: ["pic_single", "pic_group", "star5"],
  room: ["pic_group"],
  deck: ["icon_star", "star5", "card_back"],
  play: ["star5", "card_back"],
  gallery: ["card_back"],
};

/** Code chunks of the lazily loaded scenes. */
const CHUNKS: Partial<Record<Name, () => Promise<unknown>>> = {
  gallery: () => import("../scenes/gallery/Gallery"),
  deck: () => import("../scenes/deck/Deck"),
  replay: () => import("../scenes/replay/Replays"),
  replayView: () => import("../scenes/replay/ReplayPlayer"),
};

const done = new Map<string, Promise<void>>();

/** Fetch and decode one image; resolves (never rejects) once it is ready to paint. */
export function preloadImage(url: string): Promise<void> {
  if (!url) return Promise.resolve();
  let p = done.get(url);
  if (!p) {
    const img = new Image();
    img.decoding = "async";
    img.src = url;
    p = img.decode().catch(() => undefined);
    done.set(url, p);
  }
  return p;
}

const pending = new Map<Name, Promise<void>>();

/** Warm everything a route needs. Cached per route kind; safe to call often. */
export function preloadRoute(r: Route | Name): Promise<void> {
  const name = typeof r === "string" ? r : r.name;
  let p = pending.get(name);
  if (!p) {
    const urls = [backdropUrl(BACKDROP[name]), ...(IMAGES[name] ?? []).map(sceneImg)];
    p = Promise.all([...urls.map(preloadImage), CHUNKS[name]?.().catch(() => undefined)]).then(() => undefined);
    pending.set(name, p);
  }
  return p;
}

/** Resolve when `p` does, or after `ms` -- a slow network must not stall navigation. */
export function within(p: Promise<unknown>, ms: number): Promise<void> {
  return Promise.race([p.then(() => undefined), new Promise<void>((res) => setTimeout(res, ms))]);
}

const ORDER: Name[] = ["menu", "lobby", "deck", "play", "gallery", "replay"];

/** After boot, warm every screen in the background, one per idle slice. */
export function warmAllWhenIdle(): void {
  const idle = (cb: () => void) => {
    if ("requestIdleCallback" in window) window.requestIdleCallback(cb, { timeout: 3000 });
    else setTimeout(cb, 300);
  };
  const queue = [...ORDER];
  const step = () => {
    const n = queue.shift();
    if (!n) return;
    void preloadRoute(n).then(() => idle(step));
  };
  idle(step);
}
