# Advanced bot (ISMCTS) — design (2026-10-07)

Status: designed. B0 (measurement) is queued after the CP-marks work; B1–B3
wait for purchasing (docs/PURCHASE.md) because they touch the same engine and
host files.

Goal: a bot that plays well while seeing only what an ordinary player sees.
Method: Information Set Monte Carlo Tree Search over determinizations,
shallow rollouts with the heuristic bot (`engine/ai.rs`) as the rollout
policy, and a static evaluation at the horizon.

## 1. Rules

* **Information boundary.** The bot's only input is one seat's view: the same
  `MatchState` + own hand + own deck composition + prompt the client gets
  (`engine/mod.rs:464-536`). It never receives a `World`, a seed, or another
  seat's `aiAnswer`/`worth`. Hidden state is *sampled*, never read.
* **Advisory only.** The bot proposes an answer; the sandboxed engine
  (server worker or solo glue) stays authoritative. A bot build that drifts
  from the sandboxed rules makes the bot weaker, never corrupts a match.
* **No RNG coupling.** Search uses its own RNG and forked worlds only; it
  never touches the live `world.rng`, `live_rng`, `seq` or event counters.
  Recordings are unchanged because the answers are what get recorded.
* **Fallback.** Timeout, crash or a missing service → the heuristic answer
  (`aiAnswer` / `autopilot.ts`).

## 2. Architecture readiness (review 2026-10-07)

| requirement | state | evidence |
|---|---|---|
| fork | ready | `World: Clone` (`world.rs:329`), COW `EventTail` (`world.rs:215-247`); no wasm-side state |
| step | ready, private | `execute` / `Pending` / `Routine` are private (`mod.rs:119-125, 562-616`) |
| legal actions | ready | `why_not_act` (`mod.rs:1448-1555`), `cant_play`, prompt options; auction bids are numeric |
| randomness | ready | one xoshiro256** per `World` (`rng.rs:14-44`) |
| info boundary | ready | hand counts only, sorted own deck, per-seat `aiAnswer` |
| determinization | missing | hidden: opponent hands, every draw order, discards, event-deck order, RNG |
| speed | the risk | 1 sandbox run ≈ 0.57–0.70 ms (`ENGINE.md:364`); no full-game WasmRules number |
| server budget | tight | 4 stateless workers (`server/src/main.rs:29-31`), 5 s op timeout (`pool.rs:32`), choice prompts 1.2 s |
| browser | main thread, rules interpreted by wasmi | `web-glue/src/lib.rs:376-458`, `be_wasmi.rs` |

## 3. Design

### 3.1 Native rules build (`rules-native`)

Card rules are plain Rust crates; only the build makes them sandboxed
modules. The bot links them directly.

* The guest side already has a non-wasm mode: `card-sdk/src/native.rs`
  passes buffers as arena handles instead of wasm pointers. The 278
  `bandori` imports (`card-sdk/src/ctx.rs` `mod sys`) are tagged as wasm
  imports only on wasm32.
* **Host split.** The ~281 `func_wrap` closures in `game-rules/src/host.rs`
  become backend-neutral functions over `HostState` plus a `GuestMem` trait
  (wasm linear memory vs the native arena). The sandbox linker and the native
  backend both call them.
* **Native backend.** `#[no_mangle]` definitions of the `bandori` symbols
  dispatch to a thread-local host. The ruleset macros
  (`bandori_ruleset!` / `bandori_card!`, `card-sdk/src/rt.rs`) gain a native
  mode that emits a table (`RULESET: &[&[CardDef]]`) instead of the
  `bandori_on` / `bandori_manifest` exports, so every rule crate links into
  one binary without symbol clashes.
* **Panics.** Native server build: `panic = "unwind"` + `catch_unwind` at the
  entry → "this card is out", as the sandbox trap does today. wasm32 worker
  build: a panic aborts the worker; the client restarts it and falls back.
* **Drift check.** A test runs seeded games through `WasmRules` and the
  native backend and compares `save()` checkpoints.

### 3.2 Inline answers (simulation mode)

Today an `ask_*` halts and the run is replayed with the answer appended
(card runs and engine routines alike). In a simulation every decision belongs
to the bot, so `Cx` gets an answer-provider mode: an ask calls the
provider (tree policy, then rollout policy) and returns the answer at once.
No halt, no re-run, one forward pass per simulation. The live match keeps the
halt/replay model.

### 3.3 Copy-on-write world

Extend the `EventTail` technique: `Arc` chunks with `make_mut` for players,
`Hidden` piles, board tiles, field cards + props, and `TurnCtx`. A clone
then copies only what the step touches. Low value while a sandbox run costs
~0.6 ms; high value once 3.1 removes it, since the engine clones twice per
routine. Profile after 3.1.

### 3.4 `bot-core`

Target-independent crate:

* `determinize(view, rng) -> World`: opponent hands from their known deck
  composition (public picks / `deck`) minus every public sighting; shuffled
  draw piles and discards consistent with counts; event deck from the
  unseen remainder (top card if public); fresh RNG seed.
* ISMCTS (single-observer first), UCB over information sets, a fresh
  determinization per iteration.
* Action abstraction: auction bids as a few fractions of the quoted worth;
  mortgage subsets via the heuristic's order; prompts with trivial choices
  delegated to the heuristic.
* Horizon N rounds (start N = 2–3), then evaluation: net worth (cash +
  deeds + houses at value) + rent-expectation and set-completion terms,
  normalised by score weights.
* Budget: wall-clock per decision; anytime result.
* Searched decisions only: card plays, buy / build / force-buy, auctions,
  counteract chains, agent picks. Everything else uses the heuristic.

### 3.5 Server: `bot-service`

* A separate process (native build: engine + `rules-native` + `bot-core`),
  its own process count / CPU budget; the 4 room workers and the 20 Hz tick
  are untouched.
* Protocol: request `{room, seat, view, prompt, budget_ms, seed}` →
  `{answer}`. The server sends one seat's view only.
* The room asks when a searching bot's prompt opens. It also asks
  speculatively during other seats' turns, to cover 1.2 s choice prompts.
  On timeout it answers with `aiAnswer`.
* Bot level chosen at match creation next to mentality (standard / chaos /
  advanced).

### 3.6 Browser: worker bundle

* The same crates compiled to one wasm32 module run in a Web Worker:
  `postMessage(view, prompt, budget)` → decision. The rules compile into the
  module itself instead of running in wasmi, so card runs get the browser's
  JIT.
* Loaded lazily, only when the advanced 托管 policy is chosen. Phones get a
  smaller budget; the heuristic autopilot stays the default.
* The solo match itself keeps the sandboxed rules (recording parity).

### 3.7 Deck book (offline)

Bots pick decks as `deck::preset` (standard) or `deck::random` (chaos)
(`game-core/src/deck.rs:97-118`), blind to the table. The deck book is a
dictionary derived offline and loaded as data; **D1 is shipped** -- the
format, loader and back-off lookup exist and the standard bot / 托管 deck
pick goes through them, with the book itself still empty until D2.

* **Key = public information only.** At the deck phase every pick and the
  seat order are public; opponents' decks are not. Key:
  `(my character, my seat, opponents' characters in seat order)`.
* **Back-off.** The exact key space is far too large to fill (characters⁴
  × orders), so lookup falls back, first hit wins:
  1. exact `(me, seat, opponents in order)` — only for frequent tables;
  2. `(me, seat, multiset of opponents' bands)`;
  3. `(me, multiset of opponents' bands)`;
  4. `(me)`;
  5. `deck::preset`.

  An entry is stored only when it beats its back-off parent by a margin, so
  the file stays small (card ids → indices, compressed).
* **Derivation** (`game-rules` example / tool, real `WasmRules`, many cores,
  overnight):
  1. Candidates per character: preset, single-card add/drop ablations, then
     greedy local search (swap one card) from the best.
  2. Evaluation: games with the candidate in the target seat against the
     field's current book decks; metric = match score (`scoring.rs`
     weights) / placement, not binary win, for lower variance; common random
     numbers (same seeds across candidates).
  3. Racing / successive halving to drop weak candidates early.
  4. A few fictitious-play rounds: rebuild the field from the new book and
     re-evaluate, until the book stops changing.
* **Validity.** The book records the ruleset hash and the policy it was
  tuned for; a mismatch at load falls back to `preset` (stale cards / rule
  changes silently mis-tune decks otherwise). Regenerate on card changes.
* **Users.** Standard and advanced bots, and the 托管 deck pick. Chaos stays
  random. The advanced bot's determinizer uses the book as the prior for
  opponents' undrawn decks (bots exactly; humans approximately, later
  refined with deck stats from recorded matches).
* **Shipped (D1).** `data/deck_book.json` (optional -- absent or empty =
  behaviour unchanged), loaded by `GameData::load` into
  `GameData::deck_book` and looked up by `deck_book::suggest` from
  `engine/setup.rs` (standard bots) and web-glue's `deck_suggest` (托管,
  `autopilot.ts`'s `deckSuggest`). Header: `version`, `ruleset_sha256`
  (the same hash `EngineStamp` / `rules/index.json` carry), `policy`
  (`"standard"`), `generated_at`; entries per level as `exact` /
  `seat_bands` / `bands` / `me`, card ids as strings (index compression is
  a later format bump). A stale hash or policy mismatch ignores the whole
  book (logged once); an entry whose cards do not `deck::clean` down to 10
  falls through to the next level; lookup is pure (no RNG).
* **Cost** depends on B0: the eval needs the real card rules (StubRules
  cards do nothing), and ~1000 games per candidate for ±3% at 95%. Budget
  the derivation from the B0 ms/game.

## 4. Phases

| phase | content | gate |
|---|---|---|
| B0 | measure: sim with real `WasmRules` (ms/game, ms/round, runs/round, clone µs) | numbers in §5 |
| B1 | host split + `GuestMem`; macro table mode; `rules-native`; drift check | game-rules suite, drift test, sim parity |
| B2 | inline answers in `Cx` + host asks | identical outcomes vs halt/replay on seeded games |
| B3 | COW world chunks (if B1/B2 profiling shows clone cost) | sim time, save() equality |
| B4 | `bot-core`: determinizer, ISMCTS, bid abstraction, evaluation | determinizer consistency tests; win rate vs standard bot |
| B5 | `bot-service` + server protocol / budget / fallback + match option | server tests, latency under load |
| B6 | Web Worker bundle + advanced 托管 policy | bundle size, phone budget |
| D1 | deck book format + loader + back-off lookup; bots / 托管 use it — **done** (empty book until D2) | lookup tests; preset when missing / stale |
| D2 | offline derivation tool (candidates, CRN eval, racing, fictitious play) | book beats preset in a held-out sim |

## 5. Measurements

B0, 2026-10-07. AMD Ryzen 7 5800H (8C/16T, 3.2 GHz), 32 GB RAM, Windows 11,
release profile. Bench: `crates/game-rules/examples/bot_cost.rs` (4 standard
bots, seeds 0..n, 200-round cap, the shipped `dist/cards` ruleset). Counters
ride a `bot-cost` feature (compiled out by default): a few `AtomicU64`s in
`Cx::world_copy` and around `Ruleset::store` / `call_card`, plus a counting
`CardRules` wrapper in the example.

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p game-rules --release --features bot-cost --example bot_cost -- 10 4 200
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p game-rules --release --features bot-cost,wasmi-native --example bot_cost -- 3 4 200
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p game-core --release --example sim -- 50 4 200
```

**Full game** (n=50 StubRules / n=10 wasmtime / n=3 wasmi; sim agrees:
260.6 ms/game, 196.4 rounds).

| | StubRules | WasmRules wasmtime | WasmRules wasmi |
|---|---|---|---|
| ms/game | 251 | 14 957 | 39 792 |
| ms/round | 1.3 | 75.5 | 198 |
| rounds/game | 196 | 198 | 201 |
| prompts/game | 235 | 507 | 513 |

**Per game** (wasmtime): 55 900 module instantiations (110 per prompt; 49 900
`CardRules` entry calls of which 48 900 are `counteract`) and 245 600 `World`
clones (206 500 `cx.world_copy` + 39 000 host-boundary). Time share: fire-up
(fresh Store + instantiate) 17 % at 45 µs/run, guest call (incl. host imports)
1 % at 3 µs/run, engine shell 82 % — the [反击] window bookkeeping and the
halt/replay re-runs, not the wasm. wasmi flips the split: fire-up 57 %
(351 µs/run), guest 1 %, shell 42 %.

**World clone** (mid-game shape): 5.3–6.2 µs at rounds 10 / 50 / 150; ~1.7 KB
shallow, ~50–92 KiB compact encoding (the `EventTail` prefix is Arc-shared, so
a clone is refcount bumps plus the players / piles / board).

**Rollout** = fork a round-30 state (`save`/`restore`; a real bot would
`World::clone` at ~6 µs) + N rounds of bot play.

| N rounds | StubRules µs | /s | wasmtime µs | /s | wasmi µs | /s |
|---|---|---|---|---|---|---|
| 1 | 838 | 1190 | 44 700 | 22 | 114 000 | 9 |
| 2 | 1 080 | 920 | 129 000 | 8 | 295 000 | 3 |
| 3 | 1 280 | 780 | 182 000 | 5 | 377 000 | 3 |
| 5 | 1 740 | 570 | 321 000 | 3 | 649 000 | 2 |
| 10 | 3 000 | 330 | 652 000 | 2 | 1 500 000 | 1 |

**Conclusion.** At depth 2–3 a wasmtime rollout costs 0.13–0.18 s: **1 rollout
per 200 ms decision budget, 5–8 per 1 s** (wasmi: 2–3, and 1 per 200 ms only at
depth 1). Useless for ISMCTS as-is. The sandbox is not the main cost — guest
execution is 1 % — so B1 alone is *not* sufficient: it removes the 17 % fire-up
and still leaves ~60 ms/round (~120–180 ms per depth-2-3 rollout). The 82 %
shell is dominated by halt/replay amplification (507 prompts/game each re-run
their whole routine → 55 900 instantiations for ~1 000 logical effects) and the
48 900 `counteract` calls, so **B2 is the necessary lever**; B1 is the enabler
that makes B2 cheap, and B3 (clone is only ~7 % of a game) can wait. With
B1+B2 the floor is the StubRules row (0.8–1.3 ms at depth 1–3) plus real card
effects — a 2–5 ms depth-2-3 rollout gives 40–100 rollouts per 200 ms / 200–500
per 1 s, the range ISMCTS needs. Keep the horizon at 2–3 and the searched
decision set small until those numbers land.

## 6. Risks

1. The rollout cost may stay too high even native; the horizon and searched
   decision set are the levers.
2. Server wall-clock: 1.2 s choice prompts and the 5 s op timeout; the
   service must be budgeted and anytime.
3. Determinization bias: a sampler that ignores a public sighting quietly
   misleads the search. Needs consistency tests against the view.
4. Native vs wasm drift (usize width, panics); caught by the drift check,
   harmless to matches by §1.
