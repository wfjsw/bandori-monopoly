# P0 findings — risk spikes

Date: 2026-10-03. These are the load-bearing facts discovered by the P0 spikes.
They change the architecture, so read this before touching `game-rules` or the WASM build.

## Toolchain

| Tool | Status |
|---|---|
| Rust | 1.96.1 (`x86_64-pc-windows-msvc`), `wasm32-unknown-unknown` target installed |
| LLVM | **installed but not on PATH** — `/c/Program Files/LLVM/bin` |
| clang | 18.1.7, `wasm32` target supported, produces valid WebAssembly objects |
| `wasm-ld.exe` / `llvm-ar.exe` | present (needed for vendored C) |
| Node / npm | 24.19.0 / 11.17.0 |
| MSVC | via Visual Studio (for native `cc` builds) |

Build env for wasm32 C compilation (the `cc` crate):

```sh
export CC_wasm32_unknown_unknown="C:/Program Files/LLVM/bin/clang.exe"
export AR_wasm32_unknown_unknown="C:/Program Files/LLVM/bin/llvm-ar.exe"
export CFLAGS_wasm32_unknown_unknown="--target=wasm32-unknown-unknown"
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER="C:/Program Files/LLVM/bin/wasm-ld.exe"
```

## Spike A — `mlua` + CEL on `wasm32-unknown-unknown`

### Lua cannot run on bare `wasm32-unknown-unknown`

`lua-src` 551.0.2 panics during its build script:

```
don't know how to build Lua for wasm32-unknown-unknown
  at lua-src-551.0.2/src/lib.rs:100
```

**Root cause (not a config problem):** Lua's error handling uses `setjmp`/`longjmp`.
Bare `wasm32-unknown-unknown` provides neither. `lua-src` only handles:

* `*-emscripten` → `-sSUPPORT_LONGJMP=wasm`
* `*-wasi` → `-mllvm -wasm-enable-sjlj` + `wasi-emulated-signal`
* everything else → error

So this is a hard limit of the target, not of LLVM or our setup.

### CEL is pure Rust and works everywhere

`cel-interpreter` / `cel-parser` have **no `build.rs` and no `cc`/`libc` dependency**.
`libcel_interpreter*.rlib` and `libcel_parser*.rlib` build cleanly for wasm32.

### Resulting architecture

| feature | native (server) | `wasm32-unknown-unknown` (browser) |
|---|---|---|
| `cel`  | yes | **yes** |
| `lua`  | yes | **no** |

* **Browser WASM** gets `game-core` + **CEL** — guards, formulas, hint pre-validation.
* **The Lua effect engine runs on the dedicated server only**, which is authoritative
  anyway.
* **Solo play = a 1-player room on the server.** The HTTP/SSE protocol is unchanged,
  so there is no separate solo path to maintain.
* If browser-side solo is later required, evaluate **`piccolo`** (v0.3.3,
  *"Stackless Lua VM implemented in pure Rust"*, 87k downloads) — the only viable
  pure-Rust Lua. Note it is `0.3.x` and "stackless", so coroutine semantics need
  re-checking against our `Step` protocol before committing.

**Do not** use mlua's `StdLib::ALL_SAFE` as a sandbox. It is `(1 << 30) - 1`, which
**still includes `io`, `os`, and `package`** — it only drops `debug` and `ffi`.
`game-rules` uses an explicit subset:

```rust
StdLib::COROUTINE | StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8
```

## CEL gotcha: strict typing

CEL will **not** implicitly mix `Int` and `Float`:

```
money * 1.5 + 200   ->  "Unsupported binary operator 'add': Float(1500.0), Int(200)"
```

The P4 context builder must normalise numeric literals to a single width, or card
formulas will fail at runtime. Covered by `cel_env::tests::cel_rejects_mixed_int_float_arithmetic`.

## Spike B — UnityPy asset extraction

`python tools/asset-pipe/extract.py --probe` → **PROBE OK**

| kind | sample | result |
|---|---|---|
| `Texture2D` | `Splash Screen Unity Logo` | 2048×1024 RGBA → 95,909-byte PNG, magic OK |
| `AudioClip` | `afterlive-SA-LK-1-001` | 465,452-byte WAV, `RIFF`/`WAVE` OK, `fmt_tag=1` (PCM) |

UnityPy decodes Vorbis and PCM clips in-memory to valid WAV, so the P1 pipeline
(WAV → Opus/Ogg) is straightforward.

## Verdict

Both P0 spikes **pass**, with one architectural amendment: **Lua is server-side only.**
The plan's documented fallback ("solo play works by hosting a 1-player room on the
server") is now the primary path rather than a contingency.

---

## Revision (same day): CEL and Lua dropped — one WASM module per card, run on wasmi

The Lua/CEL split above is **superseded**. What replaced it, and why:

### 1. Deterministic replay instead of coroutines

An analysis of all prompt sites in `MatchHost.cs` showed:

| | count |
|---|---|
| prompt sites whose options are known before the effect runs | 174 / 189 (92%) |
| prompts that read state the same effect already mutated | 0 |
| classes whose prompt options depend on mid-effect dice or loop state | 14 |
| classes with `CanReact`/`React` (another player interrupts) | 48 |

So "collect all inputs first" covers most but not all cards. **Replay** covers all of them:
run the effect against a copy of the world; at an unanswered prompt, abort and discard
the copy; when the answer arrives, re-run from the same snapshot (RNG included) with the
answer log. Dice re-roll identically. An effect with *k* prompts runs *k + 1* times
(max *k* in the original is 6). No runtime needs coroutines any more.

### 2. Runtime: wasmi

| candidate | result |
|---|---|
| `piccolo` 0.3.3 (pure-Rust Lua) | no release since 2024-06; fails on wasm32 via transitive `getrandom` 0.2 **and** 0.3; pulls in randomly seeded `ahash` (risk to iteration-order determinism). Rejected. |
| `wasmi` 2.0.0 | pure Rust, builds clean for wasm32, 24M downloads, active (2026-09). Fuel metering. **Chosen.** |

The same engine runs on the server (native) and in the browser (inside our wasm32 build).

### 3. Packaging: one `.wasm` per card

* Measured: one card per module = **18–20 KB**, almost all std runtime; each additional
  card in a shared module adds ~1–2 KB. Removing `format!` saved nothing — the floor is
  std's allocator/panic machinery. A `no_std` SDK would likely shrink it (not measured).
* Cross-card coupling: 63/184 cards create, copy or play other cards; 24 use shared
  `*Fx` helpers; 8 share `DecayCard`. Cross-card calls go through the host import
  `play_card(id, seat)` (ABI v2), which runs the other module nested in the same run:
  same world copy, same answer log, shared fuel, nesting limit 8.
* The ABI allows 1..N cards per module, so per-band or single-module packaging remains
  a build choice; the host loads any set of modules.
* Output is content-addressed: `dist/cards/<sha256>.wasm` + `index.json`
  (card id → module). The ruleset hash is order-independent over module hashes.

### Layout

```
rules/                      guest workspace (wasm32 only)
  card-sdk/                 ABI (shared with host) + ctx imports + bandori_card! macro
  cards/<slug>/             one crate per card -> one .wasm
  fixtures/test-cards/      test-only cards (cross-module, recursion)
crates/game-rules/          host: Ruleset/RulesetBuilder, replay, play_card nesting
crates/game-rules/examples/ruleset_index.rs   validate + hash + index
tools/build-ruleset.mjs      build all -> dist/cards, dist/fixtures
```

Tests: `cargo test -p game-rules` (after `tools/build-ruleset.mjs`) — 11 passing.
