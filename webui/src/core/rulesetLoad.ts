// The card-ruleset load sequence, shared by `data.ts` (the page) and
// `webui/src/game/ruleset.test.ts` (the node deploy gate) so the two cannot
// drift: the test must load the REAL built ruleset exactly the way the page
// does, or it proves nothing about the page.
//
// The glue is the browser build, `rules-cond` `runtime-only` (docs/GUARDS.md
// §8/§8.3): it cannot compile a CEL condition source. Every guarded entry's
// `pre` therefore ships as a precompiled blob (`conds-<sha256>.bin`, §8.2)
// named in `index.json`, and `ruleset_precompiled` feeds it in before
// `ruleset_build`. A card with a `pre` and no blob is a loud build error
// (`BadPre`) -- never silently treated as true.

export interface RulesetGlue {
  ruleset_add(bytes: Uint8Array): void;
  /** `ruleset_add` with a caller-computed SHA-256 (hex) of the same bytes. */
  ruleset_add_hashed?(bytes: Uint8Array, sha256: string): void;
  ruleset_precompiled(bytes: Uint8Array): void;
  ruleset_build(): number;
}

export interface RulesetIndex {
  modules: { file: string }[];
  /** Precompiled guard conditions (docs/GUARDS.md §8.2); absent when the set
   *  declares no `pre` -- older bundles and cond-less rulesets stay valid. */
  conds?: { file: string } | null;
}

/** Add one rules module, with the native digest when the glue and a secure
 *  context allow it (`crypto.subtle`). */
async function addModule(glue: RulesetGlue, bytes: Uint8Array): Promise<void> {
  // The native digest is ~30 ms cheaper per 0.5 MB module than the glue's
  // own SHA-256 (wasm). `crypto.subtle` needs a secure context; fall back.
  const subtle = globalThis.crypto?.subtle;
  if (glue.ruleset_add_hashed && subtle) {
    const sum = new Uint8Array(await subtle.digest("SHA-256", bytes as BufferSource));
    glue.ruleset_add_hashed(bytes, Array.from(sum, (b) => b.toString(16).padStart(2, "0")).join(""));
  } else {
    glue.ruleset_add(bytes);
  }
}

/**
 * Load `index.json`, every module and the precompiled-condition blob into
 * `glue`, then build. `read` returns the bytes of a file under the rules
 * directory (`/assets/rules/` in the page). `extra` modules (a user's custom
 * solo ruleset) are added after the index's own modules and before the build,
 * so they enter the same roster / rule set. Throws on any failure; nothing
 * here degrades silently. Returns the number of modules loaded (the index's
 * plus `extra`).
 */
export async function loadRulesetInto(
  glue: RulesetGlue,
  read: (rel: string) => Promise<Uint8Array>,
  extra: readonly Uint8Array[] = [],
): Promise<number> {
  const indexText = new TextDecoder().decode(await read("index.json"));
  // An SPA dev server answers unknown paths with `index.html` and HTTP 200;
  // `JSON.parse` then throws a SyntaxError with no path in it. Say what failed.
  if (indexText.trimStart().startsWith("<")) {
    throw new Error(
      "index.json: got HTML (the ruleset was not built -- run tools/build-ruleset.mjs)",
    );
  }
  const index: RulesetIndex = JSON.parse(indexText);
  for (const m of index.modules) {
    await addModule(glue, await read(m.file));
  }
  for (const bytes of extra) {
    await addModule(glue, bytes);
  }
  if (index.conds?.file) {
    glue.ruleset_precompiled(await read(index.conds.file));
  }
  return glue.ruleset_build();
}