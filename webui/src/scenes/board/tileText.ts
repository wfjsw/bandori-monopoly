// Pure winner-picking for the deed popup's tile text (`docs/TILES.md`
// 「Tile text」). The view carries, per board tile, the speaking rule
// instances in body-precedence order; this picks the effective note of each
// slot. Run the unit test with:
//   node --test webui/src/scenes/board/tileText.test.ts

import type { TileTextEntry } from "../../core/types.ts";

/**
 * The winning key per slot from a tile's speaker list.
 *
 * - Inner list is body-precedence order: replacements first (e.g.
 *   `event:协助CiRCLE重建`), base `tile:*` last.
 * - Winner per slot = **first** entry with that `slot`.
 * - `key === ""` suppresses (render nothing) -- it still wins the slot, so a
 *   later base cannot re-claim it.
 * - `""` / missing → `null` (the slot is absent from the result).
 *
 * Returns `{}` when `entries` is missing (StubRules / old engine).
 */
export function winningTileText(
  entries: TileTextEntry[] | undefined,
): { settle?: string; pass?: string } {
  const out: { settle?: string; pass?: string } = {};
  if (!entries) return out;
  for (const slot of ["settle", "pass"] as const) {
    const e = entries.find((x) => x.slot === slot);
    // First claim wins; empty key suppresses rather than falling through.
    if (e && e.key !== "") out[slot] = e.key;
  }
  return out;
}