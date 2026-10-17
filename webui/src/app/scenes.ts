// Code-split scene modules, shared by the shell and the preloader.
//
// `React.lazy` suspends on a component's *first* render even when the chunk is
// already in the browser cache, so a scene dissolve would snapshot the empty
// Suspense fallback and the real scene would pop in after the fact. These
// resolve to a component as soon as their chunk is fetched -- `preload.ts` warms
// that same promise -- so by the time the shell swaps to a scene it renders
// synchronously and there is never a blank frame in the dissolve.

import { createElement, type ReactNode } from "react";

const LOADERS = {
  deck: () => import("../scenes/deck/Deck").then((m) => m.Deck),
  gallery: () => import("../scenes/gallery/Gallery").then((m) => m.Gallery),
  replay: () => import("../scenes/replay/Replays").then((m) => m.Replays),
  replayView: () => import("../scenes/replay/ReplayPlayer").then((m) => m.ReplayPlayer),
};

/** Scene kinds that live in their own chunk. */
export type ChunkName = keyof typeof LOADERS;

const resolved = new Map<ChunkName, () => ReactNode>();
const pending = new Map<ChunkName, Promise<void>>();

/** Fetch a scene's chunk. Resolves the same promise its `LazyScene` throws. */
export function loadSceneChunk(name: ChunkName): Promise<void> {
  let p = pending.get(name);
  if (!p) {
    p = LOADERS[name]().then((c) => {
      resolved.set(name, c as () => ReactNode);
    });
    pending.set(name, p);
  }
  return p;
}

/** A scene from `name`'s chunk. Synchronous once [`loadSceneChunk`] has resolved. */
export function LazyScene({ name }: { name: ChunkName }): ReactNode {
  const c = resolved.get(name);
  if (!c) throw loadSceneChunk(name);
  return createElement(c);
}