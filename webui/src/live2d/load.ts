// Fetch + cache for one Cubism 2 model at a time (model.json + textures), plus
// the animation bundle (`idle.mtn`, reaction `.mtn` files, `physics.json`) that
// `catalog.json` lists per model.

import { parseMotion, type ParsedMotion } from "./motion";
import { parsePhysics, type PhysicsHairSpec } from "./physics";
import type { ModelJson } from "./types";

/** Where the converter dropped the compiled models (`webui/public/assets/live2d`). */
const BASE = "/assets/live2d";

export interface LoadedLive2D {
  id: string;
  model: ModelJson;
  /** One entry per `model.textures`, ready for `texImage2D`. */
  textures: TexImageSource[];
}

interface CacheEntry {
  id: string;
  data: LoadedLive2D;
  /** Frees the decoded texture memory of a dropped model. */
  dispose: () => void;
}

let cache: CacheEntry | null = null;
let inflight: { id: string; promise: Promise<LoadedLive2D> } | null = null;
/** Id of the most recently requested model; stale loads never enter the cache. */
let latestId = "";

function disposeTextures(data: LoadedLive2D): void {
  for (const t of data.textures) {
    if (typeof ImageBitmap !== "undefined" && t instanceof ImageBitmap) t.close();
  }
}

/** Drop the cached model (its texture bitmaps are closed). */
export function clearLive2DCache(): void {
  if (cache) {
    cache.dispose();
    cache = null;
  }
  inflight = null;
  latestId = "";
}

export function live2dModelUrl(id: string): string {
  return `${BASE}/${id}/model.json`;
}

function live2dTextureUrl(id: string, file: string): string {
  return `${BASE}/${id}/${file}`;
}

async function fetchJson(id: string): Promise<ModelJson> {
  const res = await fetch(live2dModelUrl(id));
  if (!res.ok) throw new Error(`model.json ${id}: HTTP ${res.status}`);
  const data: unknown = await res.json();
  if (
    typeof data !== "object" ||
    data === null ||
    !Array.isArray((data as ModelJson).meshes) ||
    !Array.isArray((data as ModelJson).deformers) ||
    !Array.isArray((data as ModelJson).params) ||
    !Array.isArray((data as ModelJson).textures)
  ) {
    throw new Error(`model.json ${id}: malformed`);
  }
  return data as ModelJson;
}

async function fetchTexture(url: string): Promise<TexImageSource> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const blob = await res.blob();
  if (typeof createImageBitmap === "function") return createImageBitmap(blob);
  const img = new Image();
  img.src = URL.createObjectURL(blob);
  try {
    await img.decode();
    return img;
  } finally {
    URL.revokeObjectURL(img.src);
  }
}

async function fetchModel(id: string): Promise<LoadedLive2D> {
  const model = await fetchJson(id);
  const textures: TexImageSource[] = [];
  for (const file of model.textures) {
    textures.push(await fetchTexture(live2dTextureUrl(id, file)));
  }
  return { id, model, textures };
}

/**
 * Load `webui/public/assets/live2d/<id>/model.json` and its textures, lazily.
 * Only the most recent model is kept: once a newer model is ready, the
 * previously cached one's texture bitmaps are closed.
 */
export function loadLive2D(id: string): Promise<LoadedLive2D> {
  if (cache && cache.id === id) return Promise.resolve(cache.data);
  if (inflight && inflight.id === id) return inflight.promise;

  latestId = id;
  const promise = fetchModel(id).then((data) => {
    if (inflight && inflight.id === id) inflight = null;
    // One model at a time: retire the previous cache entry when this one
    // (still the latest request) takes its place.
    if (id === latestId) {
      if (cache && cache.id !== id) {
        cache.dispose();
        cache = null;
      }
      if (!cache) {
        cache = { id, data, dispose: () => disposeTextures(data) };
      }
    }
    return data;
  });
  inflight = { id, promise };
  return promise;
}

// ------------------------------------------------------------------ animations

/** One `catalog.json` entry (the game's `Live2DPortrait.Entry`). */
export interface CatalogEntry {
  id: string;
  model: string;
  textures: string[];
  motion?: string;
  physics?: string;
  reactions?: string[];
  motionsFrom?: string;
  altOf?: string;
  label?: string;
}

export interface AnimBundle {
  idle: ParsedMotion | null;
  reactions: ParsedMotion[];
  physics: PhysicsHairSpec[] | null;
}

let catalog: Promise<Map<string, CatalogEntry>> | null = null;

/** `webui/public/assets/live2d/catalog.json`, one fetch (BOM-tolerant). */
export function loadLive2DCatalog(): Promise<Map<string, CatalogEntry>> {
  catalog ??= fetch(`${BASE}/catalog.json`)
    .then((r) => (r.ok ? r.text() : "{}"))
    .then((text) => {
      const data = JSON.parse(text.replace(/^﻿/, "")) as { models?: CatalogEntry[] };
      return new Map((data.models ?? []).map((m) => [m.id, m]));
    })
    .catch(() => new Map<string, CatalogEntry>());
  return catalog;
}

async function fetchText(url: string): Promise<string> {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return res.text();
}

/**
 * The motions + physics `catalog.json` lists for `id`: `idle.mtn`, the
 * reaction set and `physics.json`. Missing pieces degrade to null/empty (the
 * stand then just holds its default pose, like a model without a catalog row).
 */
export function loadLive2DAnim(id: string): Promise<AnimBundle> {
  return loadLive2DCatalog().then(async (catalogMap) => {
    const entry = catalogMap.get(id);
    const bundle: AnimBundle = { idle: null, reactions: [], physics: null };
    if (!entry) return bundle;
    const reactionPaths = entry.reactions ?? [];
    const [idleText, physicsText, reactionTexts] = await Promise.all([
      entry.motion ? fetchText(`${BASE}/${entry.motion}`).catch(() => null) : Promise.resolve(null),
      entry.physics ? fetchText(`${BASE}/${entry.physics}`).catch(() => null) : Promise.resolve(null),
      Promise.all(
        reactionPaths.map((p) =>
          fetchText(`${BASE}/${p}`)
            .then((t) => ({ p, t }))
            .catch(() => null),
        ),
      ),
    ]);
    if (idleText) {
      try {
        bundle.idle = parseMotion(idleText);
      } catch {
        bundle.idle = null;
      }
    }
    if (physicsText) {
      try {
        bundle.physics = parsePhysics(physicsText);
      } catch {
        bundle.physics = null;
      }
    }
    // Keep the catalog order; skip the ones that failed to load (LoadOneShot
    // does the same per reaction in the game).
    for (const hit of reactionTexts) {
      if (!hit) continue;
      try {
        bundle.reactions.push(parseMotion(hit.t));
      } catch {
        // ignore a malformed reaction file
      }
    }
    return bundle;
  });
}