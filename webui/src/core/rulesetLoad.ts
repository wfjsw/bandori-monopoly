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
//
// The sequence is split in two so a live source switch can fetch everything
// before touching the live glue: `fetchRuleset` only reads and digests,
// `applyRuleset` stages the fetched bytes and builds. Nothing is replaced
// until `ruleset_build` succeeds; `ruleset_reset` drops modules a failed or
// superseded run staged.

export interface RulesetGlue {
  ruleset_add(bytes: Uint8Array): void;
  /** `ruleset_add` with a caller-computed SHA-256 (hex) of the same bytes. */
  ruleset_add_hashed?(bytes: Uint8Array, sha256: string): void;
  /** Drop modules staged by earlier `ruleset_add` calls (see `ruleset_reset`
   *  in the glue); the built set stays live until the next build. */
  ruleset_reset?(): void;
  ruleset_precompiled(bytes: Uint8Array): void;
  ruleset_build(): number;
}

export interface RulesetIndex {
  modules: { file: string }[];
  /** Precompiled guard conditions (docs/GUARDS.md §8.2); absent when the set
   *  declares no `pre` -- older bundles and cond-less rulesets stay valid. */
  conds?: { file: string } | null;
}

/** One fetched module and, when `crypto.subtle` exists, its digest. */
export interface RulesetModule {
  bytes: Uint8Array;
  sha256?: string;
}

/** A fully fetched ruleset, ready to stage without any network access. */
export interface RulesetBundle {
  modules: RulesetModule[];
  conds: Uint8Array | null;
}

/** SHA-256 (hex) of `bytes`, or undefined where `crypto.subtle` is missing
 *  (insecure context) -- the glue then computes its own. */
export async function digestHex(bytes: Uint8Array): Promise<string | undefined> {
  const subtle = globalThis.crypto?.subtle;
  if (!subtle) return undefined;
  const sum = new Uint8Array(await subtle.digest("SHA-256", bytes as BufferSource));
  return Array.from(sum, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Add one fetched module, with its native digest when the glue takes one. */
function addModule(glue: RulesetGlue, m: RulesetModule): void {
  // The native digest is ~30 ms cheaper per 0.5 MB module than the glue's
  // own SHA-256 (wasm).
  if (glue.ruleset_add_hashed && m.sha256) glue.ruleset_add_hashed(m.bytes, m.sha256);
  else glue.ruleset_add(m.bytes);
}

/**
 * Stage a fetched bundle into `glue` and build it. Any modules staged by a
 * previous run are dropped first, so a failed load cannot leak into the next.
 * `extra` modules are already part of `bundle.modules`. Returns the number of
 * modules loaded.
 */
export function applyRuleset(glue: RulesetGlue, bundle: RulesetBundle): number {
  glue.ruleset_reset?.();
  for (const m of bundle.modules) addModule(glue, m);
  if (bundle.conds) glue.ruleset_precompiled(bundle.conds);
  return glue.ruleset_build();
}

/**
 * Read `index.json`, every module it names, the precompiled-condition blob and
 * any `extra` modules (a user's custom solo ruleset). `read` returns the bytes
 * of a file under the rules directory (`/assets/rules/` in the page). Nothing
 * is written to a glue here. `onFile(done, total)` reports fetch progress once
 * the index has been parsed. Throws on any failure; nothing degrades
 * silently.
 */
export async function fetchRuleset(
  read: (rel: string) => Promise<Uint8Array>,
  extra: readonly Uint8Array[] = [],
  onFile?: (done: number, total: number) => void,
): Promise<RulesetBundle> {
  const indexText = new TextDecoder().decode(await read("index.json"));
  // An SPA dev server answers unknown paths with `index.html` and HTTP 200;
  // `JSON.parse` then throws a SyntaxError with no path in it. Say what failed.
  if (indexText.trimStart().startsWith("<")) {
    throw new Error(
      "index.json: got HTML (the ruleset was not built -- run tools/build-ruleset.mjs)",
    );
  }
  const index: RulesetIndex = JSON.parse(indexText);
  const total = index.modules.length + extra.length + (index.conds?.file ? 1 : 0);
  let done = 0;
  const tick = () => onFile?.(++done, total);
  const modules: RulesetModule[] = [];
  for (const m of index.modules) {
    const bytes = await read(m.file);
    modules.push({ bytes, sha256: await digestHex(bytes) });
    tick();
  }
  for (const bytes of extra) {
    modules.push({ bytes, sha256: await digestHex(bytes) });
    tick();
  }
  // The blob is fetched after the modules (it used to be the last input too);
  // its position in `applyRuleset` is what matters, not here.
  const conds = index.conds?.file ? await read(index.conds.file) : null;
  if (conds) tick();
  return { modules, conds };
}

/**
 * Fetch and apply a ruleset in one call: `index.json` -> modules -> `extra`
 * -> the precompiled-condition blob (`conds-*.bin`, docs/GUARDS.md §8.2) ->
 * `ruleset_build`. Returns the number of modules loaded. Kept for callers
 * (the node deploy gate, the solo / replay workers) that load straight into a
 * fresh glue.
 */
export async function loadRulesetInto(
  glue: RulesetGlue,
  read: (rel: string) => Promise<Uint8Array>,
  extra: readonly Uint8Array[] = [],
): Promise<number> {
  return applyRuleset(glue, await fetchRuleset(read, extra));
}
