// The engine-bundle id recipe (`docs/REPLAY.md` §9) -- the JS twin of
// `game_core::record::bundle_id`. One place for every tool that has to
// compute a bundle id without the Rust engine in hand (the archive step);
// the engine itself computes the same id in `record::bundle_id`, and
// `tools/test-replay-archive.mjs` pins the two together through the glue.
//
//   bundle = sha256_hex("bdre-bundle-v1\n"
//                       "<glue_sha256>\n<ruleset_sha256>\n<data_sha256>\n"
//                       "<format>\n<save_version>\n<abi>\n")
//
// `glue_sha256` is sha256 over the glue's `glue.js` bytes followed by its
// `glue_bg.wasm` bytes, hex. An empty glue sha yields "" (unknown, never a
// wrong id).

import { createHash } from "node:crypto";

/** The version tag baked into every id; mirrors `BUNDLE_ID_VERSION`. */
export const BUNDLE_ID_VERSION = "bdre-bundle-v1";

/** sha256 hex over `glue.js` bytes followed by `glue_bg.wasm` bytes. */
export function glueSha256(glueJsBytes, glueWasmBytes) {
  return createHash("sha256").update(glueJsBytes).update(glueWasmBytes).digest("hex");
}

/** The engine-bundle id. `s` is an `EngineStamp`-shaped object
 *  (`{ruleset_sha256, data_sha256, format, save_version, abi}`). Returns ""
 *  when `glueSha` is empty. */
export function bundleId(glueSha, s) {
  const sha = String(glueSha || "").trim();
  if (!sha) return "";
  const parts = [
    BUNDLE_ID_VERSION,
    sha,
    s.ruleset_sha256 ?? "",
    s.data_sha256 ?? "",
    String(s.format ?? 0),
    String(s.save_version ?? 0),
    String(s.abi ?? 0),
  ];
  return createHash("sha256").update(parts.join("\n") + "\n", "utf8").digest("hex");
}

/** The stamp fields a bundle id covers -- the loader's fallback match key
 *  for records written before the `bundle` field existed. */
export function stampKey(s) {
  return [s.format, s.save_version, s.abi, s.ruleset_sha256, s.data_sha256].join("|");
}

/**
 * The bundle id a record actually names for this glue + stamp.
 *
 * `data_sha256` is `""` on every record this build writes (`docs/REPLAY.md`
 * §9.1 -- the board and rule params ride in `MatchSetup`, `game_core::
 * DATA_FILES` is empty). Archive entries sealed before that change carry a
 * hash of the boot tables in that slot, so their stored id is not the id a
 * record looks up. This is the id to alias onto the stored one.
 */
export function engineBundleId(glueSha, s) {
  return bundleId(glueSha, { ...s, data_sha256: "" });
}

/**
 * Give every indexed bundle the id a record actually names for it.
 *
 * Mutates `index.bundles` in place: for each entry whose stored id was minted
 * with a non-empty `data_sha256`, add `aliases[engineBundleId(...)] = entry.id`
 * so `exportPortable` / the replay loader resolve the record's lookup key to
 * the directory the bytes live under. Returns how many entries gained an
 * alias. Idempotent.
 */
export function healEmptyDataAliases(index) {
  let healed = 0;
  for (const e of index.bundles ?? []) {
    if (!e.glueSha256 || !e.stamp) continue;
    const want = engineBundleId(e.glueSha256, e.stamp);
    if (!want || want === e.id) continue;
    const aliases = e.aliases ?? {};
    if (aliases[want] === e.id) continue;
    aliases[want] = e.id;
    e.aliases = aliases;
    healed++;
  }
  return healed;
}
