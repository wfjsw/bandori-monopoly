/* tslint:disable */
/* eslint-disable */

/**
 * The file names `load_data` needs.
 */
export function data_files(): string;

/**
 * `{view, budget_ms, seed}` -> `{answer, iterations, elapsed_ms, heuristic,
 * reused, decisionKey, rootStats}`.
 *
 * `view` is the exact per-member frame the client gets (`docs/BOT.md` §1).
 * `seed` is the **search** RNG's (the page derives it from the public
 * decision identity and the worker index), never the match's.
 */
export function decide(view_json: string, budget_ms: number, seed: number): string;

/**
 * What this worker is running, for logs and tests.
 */
export function info(): string;

/**
 * Drop the cached answer for one decision key after the engine refused it
 * (`docs/BOT.md` §5 B6): `decision_key` is the reply's `decisionKey` (hex).
 */
export function invalidate(decision_key: string): boolean;

/**
 * `files_json`: `{"board.json": "<contents>", ...}` for every file in
 * `DATA_FILES`, plus the optional `deck_book.json`. Same shape as
 * `web-glue::load_data`.
 */
export function load_data(files_json: string): void;

/**
 * Speculative search while the other seats act (BOT-RESEARCH #5). Same view
 * frame as [`decide`]; the result is cached by the information-set key.
 */
export function ponder(view_json: string, budget_ms: number, seed: number): string;

/**
 * Drop every cached tree and pondered answer (tests / a new match).
 */
export function reset_cache(): void;

/**
 * Feed one card module (the bytes of `<sha>.wasm`); call `ruleset_build` after.
 */
export function ruleset_add(bytes: Uint8Array): void;

/**
 * Build the ruleset from the modules added so far. Returns the module count.
 */
export function ruleset_build(): number;

/**
 * Feed the precompiled-condition blob (`conds-<sha>.bin`, `docs/GUARDS.md`
 * §8.2). Required whenever any loaded card declares a `pre`.
 */
export function ruleset_precompiled(bytes: Uint8Array): void;

/**
 * Deterministic per-worker search seed (`bot_core::seed_for_thread`): worker
 * 0 keeps `seed` exactly (so a 1-worker pool is bit-identical to a plain
 * search), the others are split-mixed away from it. The page derives each
 * worker's `decide` seed with this so root-parallel across the pool matches
 * `Ismcts::search_root_parallel`.
 */
export function seed_for_thread(seed: number, thread: number): number;

/**
 * `{"bias_weight":0.4,"eval_weight":0.3,"implicit_minimax":true,
 *   "horizon_rounds":2,"reuse_trees":true,"accept_ponder":true,"cache_cap":256}`.
 * Missing fields keep their current value.
 */
export function set_search(opts_json: string): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly data_files: () => [number, number];
    readonly decide: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly info: () => [number, number];
    readonly invalidate: (a: number, b: number) => [number, number, number];
    readonly load_data: (a: number, b: number) => [number, number];
    readonly ponder: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly reset_cache: () => void;
    readonly ruleset_add: (a: number, b: number) => [number, number];
    readonly ruleset_build: () => [number, number, number];
    readonly ruleset_precompiled: (a: number, b: number) => [number, number];
    readonly seed_for_thread: (a: number, b: number) => number;
    readonly set_search: (a: number, b: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
