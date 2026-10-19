#!/usr/bin/env node
// Prove the bot monolith (`bot-glue` with `native-rules`) has no `ruleset`
// wasm-import module -- the card rules are linked in as rlibs, so the host
// imports must not appear. Also report whether `wasmi` is linked (it currently
// is, but unused; the check is the import-module gate).
//
// Usage:
//   node tools/check-bot-monolith.mjs [path/to/glue_bg.wasm]
//
// Default path: webui/public/assets/engine/bot-glue/glue_bg.wasm

import { readFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath =
  process.argv[2] ?? join(ROOT, "webui", "public", "assets", "engine", "bot-glue", "glue_bg.wasm");

if (!existsSync(wasmPath)) {
  console.error(`check-bot-monolith: missing ${wasmPath} -- run tools/build-bot-glue.mjs`);
  process.exit(1);
}

const bytes = readFileSync(wasmPath);
const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);

function u32(off) {
  return view.getUint32(off, true);
}
function readLeb(off) {
  let n = 0;
  let shift = 0;
  for (;;) {
    const b = bytes[off++];
    n |= (b & 0x7f) << shift;
    if ((b & 0x80) === 0) break;
    shift += 7;
  }
  return [n, off];
}
function readName(off) {
  const [len, o] = readLeb(off);
  return [new TextDecoder().decode(bytes.subarray(o, o + len)), o + len];
}

// wasm magic + version
if (u32(0) !== 0x6d736100) {
  console.error("check-bot-monolith: not a wasm module");
  process.exit(1);
}

// Walk sections looking for the import section (id 2).
let off = 8;
let imports = [];
while (off < bytes.length) {
  const id = bytes[off++];
  const [size, o] = readLeb(off);
  off = o;
  const end = off + size;
  if (id === 2) {
    let [count, p] = readLeb(off);
    for (let i = 0; i < count; i++) {
      let mod, name;
      [mod, p] = readName(p);
      [name, p] = readName(p);
      const kind = bytes[p++];
      // skip the desc
      if (kind === 0) {
        [, p] = readLeb(p); // func type idx
      } else if (kind === 1) {
        p++; // valtype
        const [mut, q] = readLeb(p);
        p = q;
        void mut;
        // limits
        const flags = bytes[p++];
        [, p] = readLeb(p);
        if (flags & 1) [, p] = readLeb(p);
      } else if (kind === 2) {
        const flags = bytes[p++];
        [, p] = readLeb(p);
        if (flags & 1) [, p] = readLeb(p);
      } else if (kind === 3) {
        p++; // valtype
        [, p] = readLeb(p); // mut? actually global: valtype + mut
      }
      imports.push({ mod, name, kind });
    }
    break;
  }
  off = end;
}

const rulesetImports = imports.filter((i) => i.mod === "ruleset");
const mods = [...new Set(imports.map((i) => i.mod))].sort();
console.log(`check-bot-monolith: ${wasmPath}`);
console.log(`  bytes:            ${bytes.length}`);
console.log(`  import modules:   ${mods.length ? mods.join(", ") : "(none)"}`);
console.log(`  import count:     ${imports.length}`);
console.log(`  ruleset imports:  ${rulesetImports.length}`);

// wasmi: either not linked, or linked-but-unused. The import-module gate is
// the hard requirement; report wasmi strings so the size story is visible.
const text = Buffer.from(bytes).toString("latin1");
const wasmiHit = /wasmi/i.test(text);
console.log(`  wasmi strings:    ${wasmiHit ? "present (linked-but-unused)" : "absent"}`);

if (rulesetImports.length > 0) {
  console.error(
    "FAIL: monolith still imports from module `ruleset`:\n  " +
      rulesetImports.map((i) => `${i.mod}.${i.name}`).join(", ") +
      "\nThe card rules must be linked as rlibs (native-rules); a `ruleset` " +
      "import means the sandbox cfg seam leaked.",
  );
  process.exit(1);
}
console.log("OK: no `ruleset` import module (card rules are linked in).");