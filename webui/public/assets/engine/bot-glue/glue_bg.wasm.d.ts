/* tslint:disable */
/* eslint-disable */
export const memory: WebAssembly.Memory;
export const data_files: () => [number, number];
export const decide: (a: number, b: number, c: number, d: number) => [number, number, number, number];
export const info: () => [number, number];
export const invalidate: (a: number, b: number) => [number, number, number];
export const load_data: (a: number, b: number) => [number, number];
export const ponder: (a: number, b: number, c: number, d: number) => [number, number, number, number];
export const reset_cache: () => void;
export const ruleset_add: (a: number, b: number) => [number, number];
export const ruleset_build: () => [number, number, number];
export const ruleset_precompiled: (a: number, b: number) => [number, number];
export const seed_for_thread: (a: number, b: number) => number;
export const set_search: (a: number, b: number) => [number, number];
export const __wbindgen_malloc: (a: number, b: number) => number;
export const __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
export const __wbindgen_externrefs: WebAssembly.Table;
export const __wbindgen_free: (a: number, b: number, c: number) => void;
export const __externref_table_dealloc: (a: number) => void;
export const __wbindgen_start: () => void;
