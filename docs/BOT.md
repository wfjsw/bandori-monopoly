# Advanced bot (ISMCTS) — design (2026-10-07)

Status: B0 measured (§5), **B1 shipped** (host split + `rules-native` +
drift check; §5 has the numbers; one known sandbox/native divergence is
`#[ignore]`d and tracked below), **B2 shipped** (answer-provider mode +
inline card prompts + two-pass drive; §5 has the numbers and the one known
equivalence gap), **B4 shipped** (determinizer + ISMCTS core in
`crates/bot-core`, consistency tests green, StubRules harness smoke in §5),
**B5 shipped** (`crates/bot-service` + server protocol / budget / fallback +
the `advanced` match option; §5 has the real-ruleset latency numbers),
**B7 shipped** (BOT-RESEARCH #2/#3/#5: root-parallel ISMCTS, implicit-minimax
eval backup + progressive bias, tree reuse + the `ponder` request; §5 "B7"
has the numbers and the B4-iteration-regression triage; the server-side
`ponder` polls and the `budget + 1.5 s` deadline policy are wired too, §3.5),
**B6 shipped** (the browser worker bundle `crates/bot-glue` + the page's
root-parallel pool + the 进阶 solo / 托管 policy; §5 "B6" has the bundle size
and the 1-vs-4-worker iterations, and §3.6 documents the as-built split from
the rules-compiled-into-the-module idea -- rules still run in wasmi, see the
follow-up at the end of §3.6).
B3 waits on B2 profiling (the [反击] window bookkeeping is now the dominant
cost).

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

### 3.1 Native rules build (`rules-native`) — shipped (B1)

Card rules are plain Rust crates; only the build makes them sandboxed
modules. The bot links them directly.

* **Host split (as built).** The 293 `func_wrap` closures in
  `game-rules/src/host.rs` are now thin wrappers over
  `game-rules/src/hostfns.rs` — one plain function per import over a
  [`HostCtx`] trait. `HostCtx` supplies what differs between backends: guest
  memory (wasm linear memory vs `card_sdk::native`'s arena), fuel (store fuel
  vs a plain counter), and `call_entry` (a nested `Store` vs a thread-local
  host swap). Errors are a backend-neutral `HostErr::{NeedInput, Trap}`;
  each backend converts at its boundary. The sandbox linker is unchanged
  from the outside (`Ruleset::run` / `run_hook` / `can_counteract` /
  `cant_play` / `declares` / `card_props` keep their semantics).
* **Guest side.** `bandori_ruleset!` (`card-sdk/src/rt.rs`) emits
  `pub static RULESET: &[&[CardDef]]` on native instead of the
  `#[no_mangle]` `bandori_*` exports (those stay for wasm32), so every rule
  crate links into one binary. Each `mod sys` import carries
  `#[cfg_attr(not(target_arch = "wasm32"), link_name = "bandori_<name>")]`
  so the bare names cannot collide with libc (`log`, `draw`, `target`, ...).
  `crates/rules-native/src/symbols.rs` defines those 296 `#[no_mangle]`
  shims (`extern "C-unwind"`) over a thread-local `NativeHost`.
  `tests/abi_link_names.rs` fails loudly if a new import lacks either the
  attribute or the shim -- do not hand-rely on noticing a link error.
* **`crates/rules-native`.** `NativeModules` implements the same
  `CardModules` surface `Ruleset` does (`cards` / `card` / `card_props` /
  `declares` / `run` / `run_hook` / `can_counteract` / `cant_play`), so
  `WasmRules` generalises to `RulesBridge<M: CardModules>` and
  `NativeRules = RulesBridge<NativeModules>` is a drop-in `CardRules`.
  Manifest / kind lists / prefilters are read from the linked `CardDef`
  tables, identical to the wasm manifest's postcard form.
* **Panics.** `panic = "unwind"` (root workspace profile) + `catch_unwind`
  at every entry → `RuleError::Trap` ("this card is out"), the sandbox trap
  contract. The `extern "C-unwind"` shim ABI is what lets the panic cross
  the guest boundary at all; `extern "C"` aborts instead.
* **Drift check.** `crates/rules-native/tests/drift.rs` plays a seeded game
  through `WasmRules` and `rules-native` and compares `save()` checkpoints;
  `examples/drift_where.rs` prints the first semantic difference. Native is
  self-deterministic. See §5 (B1) for the known divergence.

### 3.2 Inline answers (simulation mode) — shipped (B2)

Today an `ask_*` halts and the run is replayed with the answer appended
(card runs and engine routines alike). In a simulation every decision belongs
to the bot, so `Cx` gets an answer-provider mode: an ask calls the
provider (tree policy, then rollout policy) and returns the answer at once.
The live match keeps the halt/replay model; the default (no provider) path is
byte-identical (checkpoints, sim counts — `cargo run -p game-core --release
--example sim -- 50 4 200` is 285 ms/game, counts unchanged).

**What landed.**

* [`AnswerProvider`] (`engine/cx.rs`): `fn answer(&mut self, ask: &Ask) ->
  Option<Answered>`. `Cx::ask` consults it after the replay log is exhausted;
  `None` keeps the halt so the tree can still branch on a prompt it cares
  about. [`HeuristicProvider`] is the rollout policy — the seat heuristic the
  engine's own bot schedule would have applied (`ask.ai` / `ai_picked` /
  `worth`, with the auction's bid loop resolved in one shot).
* `Match::{fork, set_provider, restore_value}`: a cheap in-memory fork (~6 µs
  `World` clone, vs the ~1 ms a save-JSON → restore round trip cost), a
  provider install, and an in-memory fork materialisation for the determinizer
  (no string round trip).
* **Card rules.** `game_rules::inline` is a thread-local [`InlineHost`] stack
  that `hostfns` consults at every pause point (`request_or_pause` /
  `prompt_or_pause`). The drive installs one around each guest call when the
  `Cx` has a provider. Both backends see it (the sandbox's `Caller` and
  `rules-native`'s `NativeHost` share `hostfns`).
* **Prompts inline, host requests through the loop.** A player prompt is
  answered as it happens, so it no longer costs a body re-run. A
  [`HostRequest`] still pauses and is applied by the drive's `NeedHost` arm
  against the live world before the body re-runs, and the drive then runs one
  **commit pass** with the collected answers pre-filled — so the final state is
  `guest_writes(live + host_effects)`, the guest's writes *on top* of the host
  effects, exactly as the replay model derives it. **Two guest runs per body,
  not *k*+1.**
* **Pauses are traps where the result is widely discarded.** `gain_fire` /
  `add_crystals` open a `HostRequest::Marker` window; their `Result` is
  discarded at ~35 call sites (`ctx::gain_fire(...);`). With the pause as the
  `EXIT_NEED_INPUT` sentinel those bodies caught the `Err(Prompt)` and kept
  going, which the replay model's `finish` boundary check traps ("card
  published a prompt and kept going" — the card goes out) while the inline
  model answered and the body completed. The pause is now a **trap** (tears
  the stack before the body can swallow it), so both models replay / answer the
  same body. `ctx::decay()` also propagates now instead of `unwrap_or(0)`.
  `crates/game-rules/tests/inline_provider.rs` pins "no body swallows a
  prompt".

**The one known equivalence gap.** The learn pass's body runs against a copy
of the live world taken *before* the `HostRequest`s it discovers, so a body
that queries state a host routine changed (money after a `pay`, say) sees the
pre-effect value and can ask different questions — and the overlay the engine
routine runs against (and therefore e.g. `paid.moved()`) can differ — from the
replay model's last pass (which runs on `live + host_effects`). The answer
plumbing itself is exact: `stub_rules_inline_matches_halt_replay_exactly`
pins identical checkpoints with no card effects at all. Closing the gap needs
a rebase of the guest's writes onto the updated live world after each
`HostRequest` (a write journal on `Run`), which is the next increment. The
test reports the gap per seed rather than assert through it.

**Not yet: the tree policy as a provider.** During tree descent the provider
is off and a prompt the tree branches on surfaces as `Advance::Decision`
(halt/replay, `MatchSim::inline` can disable the whole thing for comparison).
Making the tree policy *be* the provider (one forward pass even in descent) is
the next increment after the equivalence gap.

### 3.3 Copy-on-write world

Extend the `EventTail` technique: `Arc` chunks with `make_mut` for players,
`Hidden` piles, board tiles, field cards + props, and `TurnCtx`. A clone
then copies only what the step touches. Low value while a sandbox run costs
~0.6 ms; high value once 3.1 removes it, since the engine clones twice per
routine. Profile after 3.1.

### 3.4 `bot-core` (shipped, B4)

Target-independent crate `crates/bot-core`, runnable now against the built-in
rules (`StubRules`) and the public `Match` API. No engine edits.

* **Input** ([`view::SeatView`]): one seat's view only — the same frame
  web-glue's `SoloMatch::view` sends (`MatchState` + own hand + own sorted
  draw composition + the open prompt + `aiAnswer` / `playable`). Built from
  `Match::{state, hand_of, draw_of, view_extra}`; never a `World`, seed, or
  another seat's hand / deck order / `aiAnswer`.
* **Determinizer** ([`determinize::determinize`]): samples every hidden zone
  and materialises a playable fork through the engine's **save format**
  (`Saved` JSON, `SAVE_VERSION`) + the public `Match::restore`. What is
  sampled:
  * own hand — exact; own draw pile — order shuffled (composition is the
    view's sorted list); own discard — exact (discards are fully public in
    `MatchState`; only the future reshuffle order is hidden, and the fork's
    fresh RNG owns that);
  * opponent hand + draw — a split of (`deck::pool(character)` ∪
    `deck::derived(character)` − public sightings: their discard + their
    field + board fields they own), sizes fixed by the public counts;
  * event deck order — the unseen remainder of non-derived events, shuffled,
    with the public `event_top` face-down slots pinned on top in order;
    discard / active / removed respected;
  * fresh match-RNG seed and a fresh `live_rng` per fork.
  A fork reproduces the view **exactly** for the searching seat (same public
  state, same own hand / draw composition, hidden-zone counts equal, no card
  in two zones, opponent hands ⊆ allowed minus sightings) — pinned by
  `crates/bot-core/tests/determinize_consistency.rs` (risk 3).
* **ISMCTS** ([`ismcts::Ismcts`]): single-observer information-set MCTS,
  UCB over the searching seat's abstracted actions, a fresh determinization
  per iteration, tree keyed by the seat's public view (clocks stripped).
  Generic over [`sim::Simulator`]; [`sim::MatchSim`] drives real `Match`
  forks through `act` / `tick`. The searching seat is taken over in the fork
  (its `ai` flag is dropped in the save JSON before `restore`, so the engine
  never auto-plays it); the other seats are the engine's own bots.
* **Action abstraction** ([`action::Action`]): card plays, buy / build /
  force-buy offers, a few auction bid levels (min / min+100 / ¾·quote /
  quote / money−1000), counteract offers, agent / tile picks, mortgage
  subsets in the heuristic's order, and a **decline** (`act: "end"` at 结束)
  so the search can pass on a buy. Trivial prompts (≤1 option, mulligan) and
  everything else (roll, end, discard) are delegated to the heuristic — the
  engine's `aiAnswer` for prompts, an `ai_step`-shaped policy for the turn
  surface.
  * **Legality gate (2026-10-08).** Buy / Build / roll / end ride the engine's
    own `why_not_act` predicates, computed into the view as
    `MatchState::can_buy_here` / `can_build_here` / `can_roll_here` /
    `can_end_here` (buyable shape + the plan's no-buy flag + the quote's
    `eligible` gate + the funds; `why_not_build` + funds for a build; the live
    `skip_move` latch / `main_moved` / `over_hand` for roll and end). The
    abstraction never offers a command the engine would refuse — before this,
    an unaffordable Buy was proposed at 结束, the engine refused it
    (`err.buy_poor` / `err.cannot_buy` / `err.poor`), and the service replayed
    the cached refusal every 200 ms until the turn bank expired (45–54% of
    match time, `bot_cpu` §6). The public `skip_move` re-derives from stay /
    exile and can disagree with the end gate (unstoppable, mid-turn [停留],
    [除外]); trusting it sent `end` into `err.roll_first` — the heuristic's
    运营 branch now picks roll vs end by `can_roll_here` / `can_end_here`.
  * **Trivial decisions (2026-10-08).**
    [`action::trivial_decision`] skips the search when the root has exactly
    one legal action (it is forced) or when no candidate moves money,
    ownership or cards (a pure decline / auction pass / [反击] skip menu).
    The "all priors identical" alternative is deliberately not used on its
    own — two equal-prior card plays still differ tactically. Buys, builds,
    force-buy offers, auctions with a real bid, [反击] declarations, card
    plays, agent picks, tile choices and mortgages always keep the full
    search. Answered as `heuristic: true, iterations: 0`. This was 27–34% of
    searched bot CPU.
* **Rollout**: the engine's own heuristic bot (`ai.rs`) plays every seat to
  the horizon (2 rounds by default) — all seats in a fork are bots.
* **Evaluation** ([`eval::relative_worth`]): net worth (cash + deeds at
  value, mortgaged at half + houses at build cost) minus the field mean,
  normalised by start money + board value. Simple on purpose; rent /
  set-completion terms wait for B1+B2's rollout budget.
* **Anytime**: wall-clock budget per decision + iteration cap; at least one
  iteration always completes; the search's own RNG is seeded per decision
  (`DeterminizerRng`), never the match RNG. All work happens on forks.
* **Root-parallel search (B7, BOT-RESEARCH #2).**
  [`Ismcts::search_root_parallel`]: N independent searches of the same
  decision, one per thread, each with its own determinization stream
  (`seed_for_thread(seed, i)` — thread 0 keeps the request seed, so N=1 is
  bit-identical to the single-threaded search) and its own tree; root visit
  counts / value sums (and the trees) are merged at the deadline. No locks,
  no shared tree — Sephton et al. (WCCI 2014) find root parallel the
  efficient choice for ISMCTS and virtual loss unhelpful. Deterministic
  given the seed and thread count when the iteration cap binds
  (`crates/bot-core/tests/root_parallel.rs`).
* **Implicit-minimax eval + progressive bias (B7, BOT-RESEARCH #3).** Each
  edge keeps *two* statistics: the simulation outcome (terminal win/score
  from [`eval::terminal_value`], else the cutoff value) and the heuristic
  evaluation at the cutoff. The heuristic one is backed up implicit-minimax
  style (Lanctot et al. CIG 2014) — max along the searching seat's decision
  path, every node being that seat's — and mixed into selection by
  `eval_weight` α (default 0.3). The tree policy also gets a decaying
  progressive-bias bonus (Chaslot et al. 2008)
  `bias_weight · prior / sqrt(1 + visits)` (default `bias_weight` 0.4), the
  prior coming from the signals the view already carries (`aiAnswer` /
  `ai_picked`, the `wants_buy` / `wants_build` thresholds, `estCost`;
  [`action::action_priors`]). Heuristic-preferred actions are expanded
  first. Rollout horizon is configurable (`horizon_rounds`, default 2 —
  in the affordable 1–2 range). `SearchConfig::legacy()` /
  `bot-service --legacy` restores the pre-#3 selection (both weights 0) for
  A/B.
* **Tree reuse (B7, BOT-RESEARCH #5).** Nodes are keyed by the public view's
  information-set hash (`SeatView::decision_key`). After a decision,
  [`Ismcts::retain_after`] keeps the subtree under the action actually
  played (the node keys each iteration reached under that root action) and
  drops the siblings, so consecutive decisions of one seat start warm.
  `bot-service` keeps one such tree per `(room, seat)`.

#### Engine APIs B4 still needs

The view does not carry everything the engine's `Saved` format holds. B4
stubs the gaps (`crates/bot-core/src/saved.rs` documents each); they make
search *approximate*, never corrupt a match (§1). Exact signatures wanted —
all view-side, so the bot service (which only ever receives a view) can use
them; to be added once the purchasing migration stops touching
`game-core/src/engine/*`:

1. **`Match::view_routine_base(&self, member: i32) -> Option<serde_json::Value>`**
   — the public projection of the halted routine's restart base. Returns
   `None` when nothing is pending; otherwise
   `{ "routine": <Routine>, "snapshotState": <MatchState>,
      "snapshotHiddenSizes": [[hand, draw], …], "answers": [<Answered>],
      "turn": {…}, "scheduled": […], "nextEvent": i32, "askSeq": i32,
      "nextTurnPending": bool, "ringBonus": i32,
      "askAi": […], "askPicked": […], "askWorth": […] }`.
   Closes three stubs at once: the true `Pending.snapshot` (today B4 uses the
   partial world at the halt), the true `Pending.routine` (today
   `Routine::Ai(turn)`), and the `World` fields `MatchState` does not carry
   (`TurnCtx` extras, `scheduled`, `targeted`, `gains`, `ring_bonus`,
   `leftovers`, `next_turn_pending`, `ask_seq`). Without it, continuing a
   mid-prompt fork past `complete_prompt` re-runs the wrong routine from the
   wrong base — the root decision still evaluates fine, deeper play is fuzzy.
2. **`Ask::ai` / `Ask::ai_picked` / `Ask::worth` in the view** — per-seat
   heuristic answers and auction ceilings, computed at ask time. Only the
   own seat's entry is public today (`view_extra`); a fork's other seats fall
   back to the prompt fallback / a price-scaled ceiling. Wanted: either the
   full columns (they are engine-side heuristics, not hidden game state —
   `aiAnswer` already ships the own column), or a public
   `Match::ai_fill_for(prompt) -> serde_json::Value` recomputing them.
3. **`MatchMode` / `World::next_turn_pending` in the view** (minor) — the
   host `MatchMode` is not in `MatchState` (`st.mode` is its own int), and
   `next_turn_pending` is a `World` flag. B4 reconstructs the first from
   `st.mode` and stubs the second `false`; the engine's own `结束` surface
   stays live until `NextTurn` runs, which B4 papers over with a turn-end
   latch in the driver. A public `st.nextTurnPending` bit would let both the
   bot and the client's action bar agree on when 结束 is over.
4. **`deck::pool` counts for mid-game created copies** (minor) — a derived /
   created card that entered a hand without ever being public is not
   reconstructible from the view. B4 samples with replacement from the
   allowed set and reports the degradation (`SampleReport::short_seats`);
   a `cardCreated(card, seat)` log event would make it exact.

Not needed (already public): `Match::restore` + `SAVE_VERSION` (fork
materialisation), `Match::{state, hand_of, draw_of, view_extra, act, tick}`,
`deck::{pool, derived, preset}`, `purchase::quote_native`, the heuristic's
thresholds (`wants_buy` / `wants_build` / …).

### 3.5 Server: `bot-service` (shipped, B5)

* A separate process (`crates/bot-service`), its own process count / CPU
  budget; the 4 room workers and the 20 Hz tick are untouched. It loads the
  real ruleset the same way the server and `rules-worker` do (`data/` +
  `dist/cards` via `WasmRules`, `StubRules` fallback), so its simulator
  agrees with the live match.
* **Protocol** (the `rules-worker` transport: newline-delimited JSON on
  stdin/stdout, `id` echoed so replies can come back out of order — the
  service **pipelines**, one search thread per in-flight request):

  ```json
  {"id":7,"op":"decide","room":"R","seat":1,"view":{...},"prompt_id":12,
   "budget_ms":800,"seed":12345}
  {"id":7,"ok":true,"answer":{"act":"buy","value":5,"...":"..."},
   "iterations":16,"elapsed_ms":226,"heuristic":false}
  ```

  `view` is the **exact per-member frame the client gets** (the worker's
  `view` op: `MatchState` + own hand + own sorted draw + `you` / `playerId` +
  `aiAnswer` / `playable` / `estCost` + the open prompt inside `state`) —
  never a `World`, a match seed, or another seat's hidden information (§1).
  `seed` is the **search** RNG's, derived from the public decision identity,
  never the match's. Also `op: "ping"` / `"info"` / `"ponder"` (below).
  `budget_ms` is clamped to `[1, 5000]` in the service; the search is
  anytime and stops at the budget (the last iteration overruns it slightly).
  Searched surfaces come back as an abstracted action turned into a
  `NetMessage`; heuristic-delegated ones (roll / end / discard / trivial
  prompts) come back as the engine's own `aiAnswer` with `heuristic: true`,
  `iterations: 0`. Replies add `reused: true` when the answer came from the
  ponder cache (B7).
* **Ponder (B7, BOT-RESEARCH #5).** Optional `op: "ponder"` with the same
  view frame (no `prompt_id` needed):

  ```json
  {"id":8,"op":"ponder","room":"R","seat":1,"view":{...},"budget_ms":300,"seed":12345}
  {"id":8,"ok":true,"reused":false,"iterations":12,"elapsed_ms":301,
   "decisionKey":"a1b2c3d4e5f60718","answer":{"act":"buy","value":5}}
  ```

  Speculative search while the other seats act. The result is cached by
  `SeatView::decision_key()` (the public-view information-set hash, clocks
  stripped) for 60 s; a following `op: "decide"` whose view maps to the same
  key answers from the cache (`"reused": true`) without spending its budget.
  A `ponder` for a key that is already cached returns `"reused": true`
  immediately. `bot-service --no-ponder` disables the whole path. **Server
  side is wired** (B7 server round): while an advanced seat is idle (another
  seat's turn / prompt) the drive's 200 ms idle probe sends that seat's view
  as `op: "ponder"` — non-blocking (its own tokio task, the tick never waits),
  one in flight per seat, rate-limited by the probe cadence, and cancelled /
  ignored when that seat's own decision arrives (the real `decide` then hits
  the cache when the decision key matches).
* **Invalidate (2026-10-08).** `op: "invalidate"` with the reply's
  `decisionKey` drops that cached answer, so a refused one is never replayed:

  ```json
  {"id":9,"op":"invalidate","decisionKey":"a1b2c3d4e5f60718"}
  {"id":9,"ok":true,"dropped":true}
  ```

  Applied by the drive on an `act` refusal (below) and by the browser driver
  (`botDrive.ts` / `botPool.ts` → `bot-glue`'s `invalidate`).
* **Threads.** `--threads N` (env `BOT_THREADS`, default 4) request workers
  inside one process. `--search-threads N` (env `BOT_SEARCH_THREADS`,
  default = `--threads`): root-parallel searches **per decision** — one
  decision may use several threads, all stopping at the same per-request
  deadline (B7 / BOT-RESEARCH #2). The in-process backend the server's
  tests use defaults to 1 search thread so the room tick's process never
  fans out. `--data` / `--rules` like the match workers.
* **A/B flags.** `--bias-weight` (`BOT_BIAS_WEIGHT`, default 0.4),
  `--eval-weight` (`BOT_EVAL_WEIGHT`, default 0.3),
  `--implicit-minimax` (`BOT_IMPLICIT_MINIMAX`, default on),
  `--horizon` (`BOT_HORIZON`, default 2), `--no-reuse` (tree reuse off),
  `--legacy` (the pre-#3 search: no bias / eval mix / reuse / ponder, 1
  search thread). `op: "info"` reports the active settings.
* **Server side** (`server/src/botsvc.rs`). `--bot-service <path>` /
  `BM_BOT_SERVICE` (optional) spawns the child; `--bot-threads N` request
  workers, `--bot-search-threads N` root-parallel searches per decision
  (default = `--bot-threads`). Absent → logged once at startup and advanced
  bots play as standard (see below). The tick loop never waits on a search:
  each probe is its own tokio task, one in flight per seat, idle seats
  re-probed every 200 ms and busy ones every 50 ms (and the idle probe is
  what sends the `ponder` above). Budget `min(1000 ms, prompt time left −
  200 ms)`, 800 ms on the turn surface; **outer deadline `budget + 1.5 s`
  capped at 3 s** (`botsvc::ask_timeout`) — the §5 B7 policy, sized so a
  real-ruleset iteration (0.4–1.3 s at 1–4 search threads) finishing after
  the budget still lands inside the deadline and does **not** fall back.
  (The old `budget + 400 ms` capped at 1.5 s fired routinely on 4-thread
  searches; that was the fallback-rate bug this margin fixes.)
* **Who answers.** An Advanced seat is held by the engine (`ai` starts off)
  so the server is the one applying answers through the normal `act` path.
  Setup (ban / pick / deck) stays engine-side and runs the standard policy
  (`MatchPlayer::auto_setup`). On timeout / crash / bad reply the server
  applies the engine's own heuristic (`aiAnswer` / `heuristic_message_view`)
  for that decision and the match moves on. **On an `act` refusal of the
  service's answer the drive does not re-ask** (2026-10-08, `apply_bot_answer`):
  it drops the cached answer for that decision (`op: "invalidate"`) and applies
  the heuristic immediately, so a refused answer is never replayed every 200 ms
  until the turn bank expires (the forced-buy stall of `bot_cpu` §6). The
  browser driver (`botDrive.ts`) already did the fallback half; it now also
  drops the worker cache. The engine's turn clock is the last-resort safety net
  (a held seat is taken over after its bank runs out).
* **Option.** `advanced` is a bot level next to standard / chaos
  (`BotMentality::Advanced`, `POST /api/rooms/{id}/bots` `mentality`), for
  **online** rooms only. Inside the engine it is Standard — the heuristic
  plays it whenever the engine is the one driving the seat (Solo, or an
  online room with no service: the server rewrites the mentality to
  Standard before `Match::new`, so the seat is never parked on answers
  nobody will send). Browser 托管 / solo now have 进阶 too (B6, §3.6) — a Web
  Worker pool, not this service.
* **What the search sees.** Only the view frame (§1). Determinization
  samples every hidden zone; forks are materialised through the engine's
  save format and driven on copies. The live match's RNG is never touched —
  recordings are unchanged because the answers are what get recorded.

### 3.6 Browser: worker bundle — shipped (B6)

* **One wasm32 module per worker** (`crates/bot-glue`): `game-core` +
  `game-rules` (the **wasmi** backend, the same sandbox the page's `web-glue`
  runs) + `bot-core`, exposed via `wasm-bindgen`. The main page's glue is a
  separate crate and its bytes do not change -- the bot bundle is lazy-loaded
  only when an advanced seat / 进阶 托管 is in use (`ensureBotPool`).
  * Output: `webui/public/assets/engine/bot-glue/{glue.js,glue_bg.wasm}`,
    built by `tools/build-bot-glue.mjs` (same determinism rules as
    `tools/build-glue.mjs`; its own `CARGO_TARGET_DIR=target/` so a
    `target/coord` caller cannot redirect it).
  * Worker: `webui/public/assets/engine/bot-worker.js` -- a plain ES module
    (the `replay-worker.js` pattern), dynamically imports the glue and the
    shared ruleset-load sequence. The node gate runs the **same**
    `webui/src/core/rulesetLoad.ts` contract against the real built ruleset
    (`webui/src/game/botPool.test.ts`), so the inlined worker copy and the
    page's loader cannot drift.
* **API** (JSON strings, the `bot-service` wire shapes):
  * `decide(view_json, budget_ms, seed) -> {answer, iterations, elapsed_ms,
    heuristic, reused, decisionKey, rootStats}`.
  * `ponder(view_json, budget_ms, seed)` -- speculative, cached by
    `SeatView::decision_key`; a later `decide` on the same key answers
    `reused: true` (BOT-RESEARCH #5).
  * `rootStats[]` carries `{answer, visits, valueSum, mean, evalMax, evalSum,
    evalVisits}` per abstracted action, each `answer` already turned into the
    public `NetMessage` -- the page merges and picks.
  * `seed_for_thread(seed, i)` is exported so the page's derivation and
    `bot_core::seed_for_thread` stay bit-identical (pinned by
    `botBudget.test.ts`).
* **Information boundary** (`docs/BOT.md` §1): the worker receives ONE seat's
  view -- the same frame the page renders for that seat
  (`SoloMatch::view(member)` / the server's `match` frame). Never a `World`, a
  match seed, or another seat's hand / deck order / `aiAnswer`. The `seed` is
  the **search** RNG's, derived from the public decision identity
  (`decisionSeed(room, member, seq, promptId)`), never the match's.
* **Root-parallel across workers** (not inside one): `std::thread` is
  unavailable on wasm32 and each worker is already its own wasm instance +
  determinization stream. The page's pool is
  `clamp(navigator.hardwareConcurrency − 1, 1, 4)` workers (fewer on phones /
  low `navigator.deviceMemory`), each with `seed_for_thread(seed, i)`; the
  page merges the root statistics (`mergeRootStats`, a port of
  `bot_core::merge_stats`) and sends the winner through the ordinary `act`
  path. Tree reuse is per worker per seat (each worker keeps its own
  subtree); the ponder cache is per worker and the pool fires a `ponder` at
  every worker so any of them can answer a later `decide` from cache.
* **Budget (user, 2026-10-07): the browser may give the advanced bot much more
  time than the server, as long as it stays within the turn limit.**
  (`webui/src/game/botBudget.ts`, pinned by `botBudget.test.ts`.)
  * **Timed** (online 托管): `min(prompt clock, turn clock) − 500 ms`, spread
    over the turn's expected remaining decisions (`EXPECTED_DECISIONS_PER_TURN`
    = 3) so the first one does not eat the whole clock. Clamped to
    100 ms .. 12 s. Outer deadline `budget + 2 s` (one wasmi iteration
    overrun) -- the timeout fallback is the heuristic, and inside the deadline
    the search is anytime and returns whatever it has.
  * **Solo** (no deadline): a generous configurable cap, default **3 s** per
    searched decision (`bm.botSoloCapMs`, a slider in the solo setup screen,
    1–8 s). Play stays responsive; trivial prompts are answered instantly by
    the heuristic.
* **Who answers.** An Advanced seat is held by the engine (`Match::new` sets
  `ai: m.bot && m.mentality != Advanced` -- **including Solo now**, the old
  Solo exception is gone) so the driver is the one applying answers through
  the normal `act` path. Setup (ban / pick / deck) stays engine-side
  (`MatchPlayer::auto_setup`). Solo's driver is `SoloSession.driveAdvancedBots`
  (`webui/src/game/botDrive.ts`); 进阶 托管 is the same driver on the player's
  own seat. On worker error / timeout / a refused answer the existing bot
  policy (`autopilot.ts` `plan(..., "bot")` / the view's `aiAnswer`) answers
  and the match moves on -- a match never stalls (§1). A small "thinking…"
  pill shows while a search runs.
* **Recordings.** Bot answers are inputs, so replays stay exact. Pinned by
  `webui/src/game/botPool.test.ts::a solo match with an Advanced bot records
  and replays clean` (the `record.test` shape: drive via `act`, export
  `.bdrec`, replay, compare public state).
* **Lazy.** Nothing downloads until an advanced seat / 进阶 托管 is in use.
  The archive / replay path does **not** need the worker bundle -- replays
  never run the bot (`docs/REPLAY.md`).

**Follow-up: rules compiled into the module (not done here).** Today the
worker runs the same wasmi-interpreted card modules the page's `web-glue` runs
(~3–5× slower per BOT.md B0; §5 "B6" shows 3.3 iterations at 1 s / 1 worker on
the real ruleset). §3.6's original idea -- `game-rules` linked **natively** into
the wasm32 module so card runs get the browser's JIT -- needs:
1. `card-sdk`'s **native** mode on wasm32. Its `mod sys` imports are
   `cfg(target_arch = "wasm32")`-gated to the `bandori_*` wasm exports today;
   the native mode is `cfg_attr(not(target_arch = "wasm32"), link_name = ...)`.
   A third configuration ("wasm32 host, native guest") has to compile the
   guest as plain Rust and resolve the `bandori_*` shims inside the module.
2. The **rules-native drift** closed (`crates/rules-native/tests/drift.rs`'s
   known divergence, §5 B1). A bot that drifts from the sandbox is weaker
   (§1), but a *worker* whose rules disagree with the page's live engine makes
   the search's forks diverge from the match it is advising -- still advisory
   only, but the strength hit would be silent.
3. A wasm32 build of `rules-native`'s `HostCtx` (a plain `GuestMem` arena
   instead of a wasmi `Caller`) and the `catch_unwind` / `extern "C-unwind"`
   shim ABI checked on wasm32 (panic unwinding across the "guest" is all
   inside one module there, so the traps have to become plain `Result`s).

Not needed for the follow-up: the page's glue, the worker protocol, the pool,
the budget policy, the solo / 托管 drivers, or the tests -- all of that is
backend-agnostic and would keep working once `rules()` returns a native
`CardRules` instead of `WasmRules`.

### 3.7 Deck book (offline)

Bots pick decks as `deck::preset` (standard) or `deck::random` (chaos)
(`game-core/src/deck.rs:97-118`), blind to the table. The deck book is a
dictionary derived offline and loaded as data; **D1 is shipped** -- the
format, loader and back-off lookup exist and the standard bot / 托管 deck
pick goes through them. **D2 (the derivation tool) is shipped** -- see
"D2 tool" below; the shipped `data/deck_book.json` is still the empty
placeholder until the post-purchasing full run.

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
* **D2 tool.** `crates/game-rules/examples/deckbook.rs`. Offline derivation
  under the real `WasmRules` (`dist/cards`), wasmtime, one `WasmRules` per
  worker thread, deterministic in `--seed`, resumable via a JSON checkpoint
  under `target/scratch/deckbook/`.

  ```
  CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
    cargo run -p game-rules --release --example deckbook -- \
      --cap-rounds 40 --games-per-candidate 300 \
      --threads 16 --budget-minutes 0 --levels me \
      --margin 0 --seed 1 --fp-rounds 1 \
      --out data/deck_book.json --resume
  ```

  Flags: `--cap-rounds` (short capped games, scored at the cap with
  `scoring.rs` weights / `MatchPlayer::score`, not binary win), 
  `--games-per-candidate`, `--characters` (subset), `--threads`,
  `--budget-minutes` (0 = unlimited), `--levels` (`me`, `seat_bands`,
  `bands`, `exact`), `--margin`, `--seed`, `--out` (never defaults to
  `data/deck_book.json`), `--resume`, `--checkpoint`, `--fp-rounds`,
  `--greedy-rounds`, `--greedy-games`, `--rung0`, `--players`.

  Per character: `deck::preset` + single-card drop / add ablations over
  `deck::pool`, then greedy one-card swaps from the best. Every candidate is
  a complete legal deck (`deck::clean` / `deck::is_complete`). Evaluation
  uses common random numbers (same seed set across candidates) against the
  field's working book (preset on the first pass), with successive halving
  (`--rung0` up to `--games-per-candidate`) to drop weak candidates early;
  greedy neighbours are ranked on `--greedy-games` and only the winner /
  preset are topped up to the full count. An entry is emitted only when the
  winner beats its back-off parent (preset, for the `me` level) by
  `--margin`, and only the requested levels are written in the D1 format
  (header `ruleset_sha256` = the hash the tool actually ran). Fictitious
  play: after the pass, opponents' decks are rebuilt from the working book
  and each character's shortlist (winner + preset) re-evaluated,
  `--fp-rounds` times.

  Forcing a seat's deck goes through the public `act("deck")` +
  `member_left` pair (non-bot seats submit explicitly, then the engine takes
  the seat over). A first-class `Match::submit_deck(member, cards)` would be
  cleaner; not required.

* **Trial (2026-10-07, 20 min, 3 characters, cap 40, 24 games/candidate,
  greedy 1 round × 12 games, 12 threads).** 1 148 games, **12.1 s/game**
  average (spot checks 7.5 s at n = 42; opening / mulligan makes early
  rounds denser than the 75 ms/round B0 full-game mean). 户山香澄: 1 020
  games, best `swap:PPP:抓到了->通用:GREAT` 12 229 ± 1 318 vs preset
  10 876 ± 1 276 (n = 24 each, gain +1 353, EMIT). 美竹兰: 128 games
  before the budget, best `add:通用:@Tsugu ycm` 31 779 ± 4 353 vs preset
  28 296 ± 4 134 (n = 4, under-powered). 仓田真白 never started. Racing
  halved 21 → 11 → 7 → 5 ablations; the greedy field of 110 swaps ranked on
  12 games and topped the winner up to 24. Output is in
  `target/scratch/deckbook/` -- a trial must not overwrite
  `data/deck_book.json`.

* **Full-run estimate** (54 characters, `me` level, cap 40, wasmtime,
  16 threads, 12 s/game serial CPU): 300 games/candidate ≈ 24 k
  games/character (22 ablations × 1.4 ladder + 2 greedy rounds × 110
  swaps × 64 + final 2 × 300) → 1.3 M games → **~11 days wall**;
  1000 games/candidate ≈ 47 k games/character → 2.5 M games → **~22 days
  wall**. Cutting to 1 greedy round of 32 games halves that (~6 / ~13
  days). Held-out margin checks (a second seed set) add one more top-up
  pass. These are wasmtime-and-halt/replay numbers: B1+B2 (native rules,
  inline answers) should land near the StubRules row and make a full run an
  overnight job. The ruleset hash changes when the purchasing work lands,
  which invalidates any book -- the real run is scheduled after that.

* **Cost** depends on B0: the eval needs the real card rules (StubRules
  cards do nothing), and ~1000 games per candidate for ±3% at 95%. Budget
  the derivation from the measured ms/game above, not the 200-round B0
  number: capped games cost ~12 s each, not 75 ms/round × 40.

### 3.8 Strategy book (offline, per character)

Status: designed 2026-10-08 (user request). S1 started.

**Why.** Outcomes are dominated by dice, and each character's real choices are few and recurring: which colour groups to buy, how much cash to hold, how hard to bid, when to play each card, when to press each skill, when to counteract. A small set of **tuned parameters per character** captures much of the advantage at near-zero runtime cost. It also improves everything built on the heuristic:
- the standard bot and 托管 play it directly;
- the ISMCTS rollouts use it as their policy (better rollouts → fewer needed);
- the search uses it as its progressive-bias prior (§3.4).

**Parameter set** (`StrategyParams`: a flat, versioned, serde struct of integers / milli-fractions; every field has a default equal to today's constant in `engine/ai.rs` / `bot-core`, so the default book changes nothing):

| family | parameters (illustrative) |
|---|---|
| buying | per-colour-group priority weights; set-completion bonus; cash reserve by game phase (early / mid / late, by round); max price-to-cash ratio |
| building | per-group build priority; target houses per group; build reserve |
| auctions / force-buy | bid ceiling as a fraction of quoted worth, by group and phase; force-buy appetite |
| card play | per card: `play_weight`, `hold_for_counteract` flag, min/max round, min cash after `estCost` |
| skills | per skill entry: press threshold (e.g. fire / crystals in hand, phase), keep-reserve of markers |
| counteraction | per card: response propensity (0..1000 ‰) by window kind |
| mortgage / redeem | mortgage order weights; redeem cash threshold |
| deck | optional link to the deck-book entry the parameters were tuned with (§3.7) |

Per-card and per-skill entries are sparse maps (absent = default).

**Book** (`data/strategy_book.json`, versioned like the deck book):
- **Header:** `version`, `ruleset_sha256`, `policy` (standard), `generated_at`, `params_version`.
- **Entries keyed by public information only**, with back-off, first hit wins:
  1. `(character, opponents' bands multiset)`;
  2. `(character)`;
  3. `(band)`;
  4. defaults.
- **Validation:** a stale ruleset hash, policy or `params_version` ignores the book (warned once). Unknown fields are ignored; missing fields take defaults.
- **Not in `data_sha256`**, like `deck_book.json`, so replay stamps don't move with it.
- **Joint with the deck book:** character and deck are chosen together, so the derivation tunes them as a pair. An entry may name the deck it was tuned with; mismatched pairs fall back to the character's defaults.

**Consumers:**
- **Engine heuristic:** `engine/ai.rs` (standard mentality; chaos unchanged).
- **Search:** `bot-core` rollout policy + `action_priors`; `bot-service` and `bot-glue` load the book with the ruleset.
- **托管 autopilot:** `webui/src/game/autopilot.ts` gets the seat's resolved params from the glue (one source of truth, like `deck_suggest`).
- **Information boundary:** keys and parameters are public / static. Nothing reads hidden state.

**Derivation (offline):** extend the D2 tool (`crates/game-rules/examples/deckbook.rs`) into a joint deck + strategy tuner:
- **Optimiser:** CMA-ES or coordinate / racing search over the per-character parameter vector (start with the ~10–20 highest-impact scalars: reserves, group weights, bid fraction, counteract propensity), then the sparse per-card weights for that character's deck.
- **Evaluation:** common random numbers, match score at a round cap (as D2), successive halving, fictitious-play rounds with opponents on the current book, held-out seeds for acceptance (must beat defaults by a margin).
- **Order:** band level first (≈10 bands, cheap), then characters. Popular characters first if budget-limited.
- **Cost:** real-ruleset games (~12 s each today). It gets much cheaper with the engine work (B1/B2, window optimisations, profile fixes). Schedule the full run after those.

**Phases:**

| phase | content | gate |
|---|---|---|
| S1 | `StrategyParams` + defaults equal to today's constants; plumb into `ai.rs`, bot-core rollout policy and priors, autopilot; book format + loader + back-off lookup (empty book) | byte-identical behaviour with the empty book (sim counts, seeded real-rules checkpoints); lookup tests |
| S2 | derivation: extend `deckbook.rs` to tune params (+ joint deck); band-level trial run | tuned band params beat defaults on held-out seeds |
| S3 | full per-character run (after the engine speedups); ship `data/strategy_book.json` | held-out win rate / score vs defaults; advanced-bot strength vs standard with and without the book |

## 4. Phases

| phase | content | gate |
|---|---|---|
| B0 | measure: sim with real `WasmRules` (ms/game, ms/round, runs/round, clone µs) | numbers in §5 |
| B1 | host split + `HostCtx`; macro table mode; `rules-native`; drift check — **done** (suite green, native self-deterministic, one known divergence `#[ignore]`d; §5) | game-rules suite, drift test, sim parity |
| B2 | inline answers in `Cx` + host asks + two-pass drive — **done** (StubRules checkpoints identical; one known real-rules gap, §3.2) | identical outcomes vs halt/replay on seeded games |
| B3 | COW world chunks (if B1/B2 profiling shows clone cost) | sim time, save() equality |
| B4 | `bot-core`: determinizer, ISMCTS, bid abstraction, evaluation — **done** (StubRules smoke; engine-API gaps in §3.4) | determinizer consistency tests; win rate vs standard bot |
| B5 | `bot-service` + server protocol / budget / fallback + match option — **done** (server tests green; real-ruleset latency in §5) | server tests, latency under load |
| B7 | BOT-RESEARCH #2/#3/#5: root-parallel ISMCTS, implicit-minimax eval + progressive bias, tree reuse + `ponder` — **done** (§5 B7; server wiring of the ponder polls + the deadline policy is landed too, §3.5) | determinism / N=1 tests; iterations/decision 1 vs 4 threads; old-vs-new strength at fixed budget |
| B6 | Web Worker bundle + advanced 托管 policy + 进阶 solo bots — **done** (§3.6 as-built; §5 B6 has the bundle size and the 1-vs-4-worker iterations; rules still run in wasmi, follow-up at the end of §3.6) | bundle size, phone budget |
| D1 | deck book format + loader + back-off lookup; bots / 托管 use it — **done** (empty book until D2) | lookup tests; preset when missing / stale |
| D2 | offline derivation tool (candidates, CRN eval, racing, fictitious play) — **tool done**; book itself waits for the post-purchasing ruleset (§3.7 trial + estimate) | book beats preset in a held-out sim |
| S1–S3 | strategy book (§3.8): params + plumbing (S1), tuner + band trial (S2), full run (S3) | §3.8 phase table |

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

### B1: native rules (2026-10-07)

Same machine and profile as B0. One full bot-only game (4 standard bots,
seed 7, 200-round cap, the shipped `dist/cards` table), `rules-native` vs the
sandboxed `WasmRules`, release profile:

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run --release -p rules-native --example drift_where -- 7 200
```

| | WasmRules (wasmtime) | rules-native | |
|---|---|---|---|
| ms/game | 19 421 | 14 021 | **1.4x** |
| ms/round (196 rounds) | 99 | 71 | |

n = 1 per backend (the two runs are not bit-identical, see below, so this is
a rough read). Consistent with B0's prediction that B1 removes the ~17 %
fire-up and the guest-call share: the floor is still the engine shell
(halt/replay + the [反击] window bookkeeping), which is B2's job. Rollout
cost at depth 2-3 should now be ~0.10 s instead of ~0.15 s -- still far from
the 2-5 ms ISMCTS wants.

**Drift check.** `crates/rules-native/tests/drift.rs`:

* `native_is_deterministic` — green. Two native runs of the same seed
  produce identical `save()` checkpoints.
* `native_and_sandbox_agree_on_a_seeded_game` — **`#[ignore]`d**, known
  divergence. Setup agrees (`checkpoint 0` identical); from round 1 the
  native run draws one extra card and raises two extra prompts
  (`ask_seq` 7 vs 9, player 0's draw pile 14 vs 13 and differently ordered,
  `live_rng` advanced differently). `examples/drift_where -- 7 2` prints the
  full first-difference list. Candidates still open: a guard / hook
  dispatch difference in `NativeModules`, the native fuel counter being
  effectively unlimited (the sandbox stops runaways at `DEFAULT_FUEL`), or
  an in-flight card-rule change landing in `rules/*` after `dist/cards` was
  last built (the sandbox runs the snapshot, native compiles the sources).
  Per §1 this makes the bot weaker / different, never corrupts a match --
  but it must be closed before the native build is trusted for search.
* `tests/abi_link_names.rs` — green. Every `mod sys` import has both the
  native `link_name` and a `bandori_*` shim; a new import without them fails
  `cargo test -p rules-native` with the fix spelled out.

Suite: `cargo test -p game-core -p game-rules -p server -p bot-core -p
bot-service -p rules-cond` = 1048 passed / 0 failed / 96 ignored (baseline
~1041; the delta is the concurrent purchasing batch). The sandbox path is
byte-for-byte the same `Ruleset` as before the split -- the closures moved
into `hostfns.rs` and the wrappers forward.

**Wiring.** `bot-service` has a `native-rules` cargo feature (off by
default) that swaps its `CardRules` to `rules_native::native_rules` for the
search's simulations; `WasmRules` / `StubRules` fallback is unchanged when
the feature is off. `cargo test -p bot-service --features native-rules` is
green. Turn it on by default once the drift above is closed.

### B2: inline answers (2026-10-07)

Same machine and profile as B0. `crates/game-rules/examples/bot_cost.rs` now
prints an **inline** row per ruleset: the same mid-game round-30 state forked
with `Match::fork` and played with [`HeuristicProvider`] installed (every
prompt answered inline; two guest runs per card body instead of *k*+1), next
to the pre-B2 row (save/restore fork, halt/replay). Full games (2 × 4 bots,
cap 200) are the **old** path (the live match keeps halt/replay) and moved
only with the concurrent purchasing batch's card changes — not a B2 signal.

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p game-rules --release --features bot-cost --example bot_cost -- 2 4 200
```

**Rollout** (fork + N rounds of bot play from a round-30 state, mean over 5–60
iterations):

| N rounds | StubRules µs | inline | wasmtime µs | inline | inline /s |
|---|---|---|---|---|---|
| 1 | 804 | **258** | 60 816 | **49 742** | 20 |
| 2 | 1 032 | **470** | 173 647 | **130 959** | 8 |
| 3 | 1 285 | **700** | 240 227 | **188 448** | 5 |
| 5 | 1 735 | **1 322** | 427 339 | **329 307** | 3 |
| 10 | 2 968 | **2 297** | 859 230 | **617 865** | 2 |

**1.3–1.8×** on the real ruleset, **1.3–1.9×** on StubRules. The B0 target
(2–5 ms per depth-2-3 rollout) is **not** met: the two-pass drive removes the
prompt re-runs and one of the *k*+1 body re-runs, but the [反击] window
bookkeeping B0 measured alongside the halt/replay amplification is untouched
(56 000 `counteract` calls/game, 81 % engine shell) and every `gain_fire` /
`add_crystals` still costs one body re-run through the `HostRequest::Marker`
window. Closing the equivalence gap (§3.2) would let the commit pass drop
away for bodies whose host requests commute with their writes, and a cached
counteract index is the other lever (B3 territory).

**Default path untouched.** `cargo run -p game-core --release --example sim
-- 50 4 200` is 285 ms/game over 50 games (avg 196.3 rounds) — inside the
323 ms budget; prompt and event counts are the halt/replay model's.

**Equivalence.** `crates/game-rules/tests/inline_provider.rs`:
`stub_rules_inline_matches_halt_replay_exactly` — identical checkpoints for
3 seeded games (the engine-level provider is exact);
`real_rules_inline_matches_halt_replay` — no body swallows a prompt, and the
one known gap (§3.2) is reported per seed rather than asserted through.

### G3/G4 + cached counteraction index (2026-10-07)

BOT-RESEARCH.md #1 (the biggest single lever). Same bench as B0/B2
(`bot-cost`, 2 games × 4 standard bots, real `dist/cards`):

| | B0/B2 baseline | after |
|---|---|---|
| `counteract` entry calls/game | 48 900 | 59 350 (2-game sample) |
| module instantiations/game | 55 900 | **47 836** (−14 %) |
| inline rollout N=1 | 49 742 µs | **48 767 µs** |
| inline rollout N=2 | 130 959 µs | **125 304 µs** (−4 %) |
| inline rollout N=3 | 188 448 µs | 183 131 µs |
| StubRules inline N=2 | 470 µs | 511 µs |

`cargo run -p game-core --release --example sim -- 50 4 200` is **320.1
ms/game** (≤ 323 budget), counts identical (buy 2899 / forcebuy 392 /
auctions 255 / rent 19567 / bankrupt 96). The instantiation cut is the
per-card kind bitmask + the condition pre-filter (docs/GUARDS.md §10.3)
rejecting probes before any `Run`/`world_copy`/guard. The modest rollout cut
reflects that only 12/51 counteract entries carry a `pre` so far — moving the
remaining ~39 bodies (G4 batch 2) is what closes the 131 ms → 20–40 ms gap.

### G4 batch 2 + sim path fix (2026-10-07/08)

Same bench (`bot-cost`, 2 games × 4 standard bots, real `dist/cards`).
**The 320 ms sim reading was not a G3/G4 regression** — `game-core` does not
link `game-rules`, so the guard plumbing is not on the StubRules path at all.
Allocation profiling showed the sim loop is **53 % `Match::state()`**: `public_state()`
re-cloned the 80-event view window on every one of the sim's 21 700 per-game
polls (174 allocs/call). Fixed by caching the window in `EventTail` behind a
`Mutex` and making `MatchState::events` an `Arc<Vec<MatchEvent>>` (docs/GUARDS.md
§11.1). Idle-machine sim is now **~182–228 ms/game** (≤ 323, target ~270),
counts identical.

| | batch 1 | after batch 2 |
|---|---|---|
| entries with a condition | 128 | **139** |
| residual guard kept | 258 | 250 |
| guard deleted (`None`) | 342 | **350** |
| kind re-checks (precheck) | 34 | **0** |
| `legacy_*` audit copies | 128 | 137 |
| module instantiations/game | 47 836 | 47 836 (2-game sample) |
| inline rollout N=2 (wasmtime) | 125 304 µs | 133 595 µs (loaded machine) |
| StubRules inline N=2 | 511 µs | 490 µs |

What moved (batch 2): 8 counteract entries (`elegant_shout`, `taki_even_if`,
`marina_work`, `layer_keep`, `two_in_one`, `hold_hands_again`, `even_lost`,
`mana_champion` — plus `here_the_world`'s prefix) got their expressible clauses
into `pre` (`actor`/`by`/`target` relations, `move.roll`, `roll_source`,
`effect.hits`, `character_is`, `neighbor`, `value`) with `legacy_*` copies for
the audit. **All 34 kind re-checks the precheck flagged are stripped** (the
category list owns the kind; compound forms kept their residual clauses).
Play-gate `cant_play` money/fire thresholds are still in the gate bodies —
next batch.

**`slot()` read the wrong key.** `cond_pre::fill_candidate` looked the CEL
`slot('name')` up under `slot:<name>`, while the guest's `ctx::slot(player_id,
name)` reads the bare player-state key. Every `slot('lastWalk') > 0` condition
read 0 and silently closed the window (`rb_guards::ordinary_matches_only_abnormal`
caught it). Fixed in `cond_pre` (bare key + `SLOT_NAMES` now covers
`asUsualTurn`/`lastWalk`); `fill_candidate` always fills them.

The 20–40 ms depth-2 target is still open: at N=2 a wasmtime inline rollout is
~134 ms, and `bot_cost` attributes **88 % of wall to the engine shell** (the
[反击] window bookkeeping + halt/replay amplification), 10 % fire-up, 2 %
guest. The remaining counteract/play-gate clause moves cut probes further, but
the shell is the floor B2's write-journal rebase (§3.2) has to attack.

### B4 harness (2026-10-07)

`crates/bot-core/examples/ismcts_vs_bots.rs` — seat 0 = ISMCTS over
determinized `Match` forks (StubRules), seats 1–3 = the engine's standard
bots, 200-round cap, Solo, the shipped `data/` table. Same machine as B0,
release profile:

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p bot-core --release --example ismcts_vs_bots -- 6 200 6
```

| | |
|---|---|
| decisions | 127 over 6 games (~21/game — the searched set only) |
| decisions/sec | 4.2 (game playing + search, one thread) |
| iterations/decision at 200 ms | 16 |
| search wall-clock/decision | 226 ms (the last iteration overruns the budget slightly; probe setup excluded from the budget) |
| fallbacks (heuristic, 0 iterations) | 21 / 127 (17 %) |
| ISMCTS seat 0 | rank 2.33, score 73 875, wins 1/6 |
| standard-bot baseline seat 0 | rank 2.33, score 78 545, wins 2/6 |

Weak signal, as expected on StubRules (cards do nothing — the search can only
choose between buy / build / end shapes). The numbers that matter for B5/B6
are the **per-iteration** cost: 226 ms / 16 ≈ **14 ms per iteration** on
StubRules forks, dominated by materialising the fork (`save`-format JSON
serialize + `Match::restore` parse, ~10 ms) plus one `decision_key` state
hash, not by the rollout (2 rounds ≈ 1–2 ms per B0). A native
`World`-level fork (B3 / a `Match::fork_from` API) would cut that ~10×; with
the current materialisation 200 ms buys ~16 iterations, enough for a 2–4
action root but not deeper trees. Determinizer consistency: 3 tests green
(`cargo test -p bot-core`) — 12 seeds × random points × 4 seats × 4 samples
reproduce the view exactly.

### B5 service latency (2026-10-07)

`crates/bot-service/examples/latency.rs` -- real `WasmRules` (`dist/cards`,
1 module, `ruleset_sha256 d9b4d307…`), release profile, same machine as B0.
Parks a 4-seat match at one seat's real decisions (the exact frame the server
sends) and runs `decide` at the budgets the server actually uses.

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p bot-service --release --example latency -- \
    --data data --rules dist/cards --decisions 24 --budgets 200,1000
```

| | 200 ms budget | 1 000 ms budget |
|---|---|---|
| decisions parked | 24 | 24 |
| searched surfaces (cards / buy-build / auctions / prompts) | 3 | 3 |
| heuristic-delegated (roll / end / discard) | 21 | 21 |
| latency, searched (mean / max) | **244 / 277 ms** | **1 105 / 1 220 ms** |
| iterations, searched (mean) | **1.0** | **3.7** |
| latency, delegated | ~1–5 ms | ~1–5 ms |
| answer refused by the engine | 0 | 0 |

The searched latency overruns its budget by one iteration (the ISMCTS loop
completes the iteration it is in -- same shape as B4's 226 ms at 200 ms), so
the server's outer deadline is `budget + 400 ms` capped at 1.5 s. Iterations
match B0's prediction (1 rollout per 200 ms, 5–8 per 1 s on the full ruleset;
this ruleset is 1 module and the fork materialisation dominates, as in B4).
Most turn surfaces are roll / end and cost nothing -- the search only fires
where the abstraction has something to choose.

Protocol round-trip is pinned by `crates/bot-service/tests/protocol.rs` (7
tests); the server path by `crates/server/tests/http.rs` (advanced seats play
with the service, play as standard without it, and a timeout falls back
without stalling).

### B7: search acceleration (2026-10-07)

BOT-RESEARCH #2/#3/#5: root-parallel ISMCTS, implicit-minimax eval backup +
progressive bias, tree reuse + the `ponder` request. Same machine and
profile as B0. Harnesses: `crates/bot-core/examples/ismcts_vs_bots.rs`
(StubRules, `--threads` / `--horizon` / `--bias` / `--eval` / `--legacy` /
`--profile` / `--compare`) and
`crates/bot-service/examples/latency.rs` (real `dist/cards` WasmRules or
`--stub`, `--search-threads 1,4`, `--budgets 200,1000`, `--ponder`).

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p bot-core --release --example ismcts_vs_bots -- \
    --threads 4 --profile -- 8 200 0
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p bot-service --release --example latency -- \
    --data data --rules dist/cards --decisions 8 --budgets 200,1000 \
    --search-threads 1,4
```

**The B4/B5 iteration regression — cause found and fixed (in bot-core).**
B2/B5 reported 1.0 iterations/decision at 200 ms on the B4 harness (B4 had
measured 16). `--profile` attributes the iteration cost (StubRules, before
the fix): fork 2.6 ms, descent 1.4 ms, **rollout 57.7 ms**, eval 0.03 ms,
`decision_key` 0.23 ms — the rollout was ~100× the B2 inline number (0.47 ms
for 2 rounds) and alone ate the budget. Cause: an **auction prompt the seat
is already top bidder on**. The engine refuses any positive bid from the
current top bidder (`err.already_top_bid`, `engine/mod.rs` place_bid) and
anything under `bid + 100` (`err.bid_min`); `aiAnswer.worth` is a ceiling
that can sit at our own bid, so the heuristic re-bid it and was refused —
and the harness/rollout then `tick`ed, which let the other seats' bots
raise, making the surface staler still. Each rollout burned `max_ticks`
(4 000) ticks on the spin and each harness game 200 000 steps. Fixes
(both in `crates/bot-core`, no engine change):

1. auction actions: pass-only when the prompt's `bidder` is us; bid levels
   are round hundredths strictly over the live bid (`action.rs`);
2. `ai_answer_to_action` passes unless `worth` is a legal raise
   (`sim.rs`); a stale ceiling can no longer force a refused act;
3. a two-refusal bail in `MatchSim::advance_inner` (cut the rollout at
   `Advance::Horizon` instead of spinning) and in the harness.

After the fix (same machine, StubRules, 8 games): per-iteration fork 3.26
ms, descent 0.52 ms, **rollout 0.35 ms**, eval 0.05 ms, key 0.24 ms —
**4.5 ms/iteration**, i.e. the B4 14 ms materialisation cost is already
halved by the in-memory `restore_value` path and the rollout is back to the
B2 inline floor. Delegated-act refusals dropped 199 631 → 0 per game.
This is the whole regression: it is in bot-core's heuristic/abstraction
handling of auctions, not in the engine.

**Iterations per decision (StubRules, `ismcts_vs_bots`, 8 games per cell,
horizon 2):**

| budget | 1 thread | 4 threads | scale-up |
|---|---|---|---|
| 200 ms | **29.1** | **93.9** | 3.2× |
| 1 000 ms | **138.7** | **357.1** | 2.6× |

(The B4 target was 40–100+ at 200 ms; 1 thread now sits at 29 and 4 threads
at 94. Root-parallel is near-linear as the WCCI 2014 paper predicts; the
sub-linear tail is merge + the last-iteration overrun.)

**Iterations per decision (service path, `latency`, 8 searchable surfaces
per cell)** — StubRules (`--stub`) and the real `dist/cards` WasmRules
(`ruleset_sha256 259cfa15…`, 1 module):

| | StubRules 200 ms | StubRules 1 s | real 200 ms | real 1 s |
|---|---|---|---|---|
| 1 thread | **18.6** iters, 208 ms | **68.9**, 1 011 ms | **1.0**, 569 ms | **2.1**, 1 601 ms |
| 4 threads | **85.6** iters, 210 ms | **238.0**, 1 095 ms | **4.0**, 1 298 ms | **6.9**, 1 822 ms |
| scale-up | 4.6× | 3.5× | 4.0× | 3.3× |

(The StubRules service rows are lower than the harness rows above because
`latency` parks at cheap early-game surfaces and the harness averages a
whole game's worth of decisions; both are in the target range. The real
rows are the B0/B2 prediction still standing: one wasmtime rollout is
tens of ms and a fork is ~10 ms of JSON, so an iteration costs 0.4–1.3 s
until BOT-RESEARCH #1's counteract cache and #4's native fork land.
Root-parallel still multiplies that by the thread count.)

**Budget overrun note (real ruleset).** The search is anytime *between*
iterations and always completes the first one, so a real-rules iteration
(0.4 s solo, up to ~1.3 s when 4 searches contend) overruns `budget_ms` by
up to one iteration. At 200 ms that is 0.6–1.3 s wall. **The server's
deadlines are now rewired to this note's recommendation** (`server/src/botsvc.rs`:
outer deadline `budget + 1.5 s` capped at 3 s,
`ITERATION_OVERRUN_MS` / `ask_timeout`), so a 4-thread search finishing its
iteration after the budget lands inside the deadline and does not fall
back. `--bot-search-threads` (default = `--bot-threads`) is the deploy knob
for the fan-out; 1 is the conservative choice if the box is shared. The
StubRules / browser shapes (iterations ≤ 25 ms) fit any deadline at any
thread count. §5 "B7 server wiring" has the measured fallback rate.

**Strength at a fixed wall budget** (StubRules, seat 0 vs 3 standard bots,
200 ms/decision, n = 100 games per cell, `ismcts_vs_bots --compare`):
legacy = pre-#3 selection (no bias / eval mix / reuse), 1 search thread;
new = the shipped defaults (bias 0.4, α 0.3, implicit minimax, tree reuse),
also 1 thread — so this isolates BOT-RESEARCH #3:

| | legacy | new | delta |
|---|---|---|---|
| wins (seat 0, rank 1) | 17 / 100 | **26 / 100** | **+9** |
| mean score | 43 593 | **58 995** | **+15 402** |
| mean rank | 2.83 | **2.47** | **−0.36** |
| mean rounds | 158 | 178 | +20 |
| iterations/decision | 24.0 | 21.1 | −2.9 |
| refused search acts | 365 / 2 559 (14 %) | 304 / 3 894 (8 %) | −6 pp |

Binomial SE on 17–26 wins at n = 100 is ~4 pp, so the win delta is ~2σ; the
score and rank deltas are large. The iterations dip is the prior / dual-stat
bookkeeping costing ~12 % per iteration — the #3 flags buy more decision
quality than they cost in iterations. (StubRules is a weak absolute test —
cards do nothing, so this is mostly buy / build / end / auction shaping,
including the auction fix above. Machine shared with a concurrent build;
both cells ran sequentially under the same load.)

**4 threads, new settings, same budget** (n = 100, `ismcts_vs_bots --threads
4`) — the whole B7 stack:

| | legacy 1 thread | new 1 thread | new 4 threads |
|---|---|---|---|
| wins | 17 / 100 | 26 / 100 | **30 / 100** |
| mean score | 43 593 | 58 995 | **64 261** |
| mean rank | 2.83 | 2.47 | **2.44** |
| iterations/decision | 24.0 | 21.1 | **93.6** |

Root-parallel at the same wall budget costs nothing in latency (212 ms vs
210 ms per decision) and adds the second strength step. Against the
all-standard-bot baseline's seat-0 stand-in (rank 2.3–2.4 in the B4 / B5
tables) the advanced seat is now at or above par on StubRules, where the
only real decisions are buy / build / end / auction — the real-ruleset
strength read waits on the counteract-cache rollout cost (BOT-RESEARCH #1)
so a 200 ms budget can afford more than one real iteration.

**Ponder / tree reuse.** `crates/bot-service/tests/protocol.rs` pins the
cache: a `ponder` then a `decide` on the same view answers `reused: true`
without searching again (the reused decide returns in <10 ms vs the
pondered search's budget); a different information set misses. Tree reuse
(`retain_after`) keeps exactly the subtree under the played action —
pinned by `root_parallel.rs`. `rules-native` stays out of the measurements:
its drift test still has the known B1 divergence (`#[ignore]`d), so the
"native if green" gate is not met.

### B7 server wiring (2026-10-07)

The server-side half of B7: the idle-seat `ponder` polls and the deadline
policy §5 "Budget overrun note" recommended. Harness:
`crates/server/examples/bot_fallback.rs` (real `dist/cards` WasmRules,
`ruleset_sha256 596bfd89…`, every decision one seat faces — searched and
heuristic-delegated — scored under both deadline policies off the same
latencies). Same machine as B0, release profile, `target/coord`:

```
CARGO_TARGET_DIR=target/coord CARGO_INCREMENTAL=0 \
  cargo run -p server --release --example bot_fallback -- \
    --data data --rules dist/cards --decisions 20 --search-threads 4
```

| | shipped deadline `budget + 1.5 s` (cap 3 s) | old deadline `budget + 400 ms` (cap 1.5 s) |
|---|---|---|
| answered by the service | **20 / 20 (100 %)** | 17 / 20 (85 %) |
| fell back to the heuristic | **0 / 20 (0 %)** | 3 / 20 (**15 %**) |
| 1 search thread, n = 12 | 12 / 12 (100 %) | 10 / 12 (17 % fallback) |

Default budgets as the server sends them (800 ms turn surface, `min(1000 ms,
prompt time left − 200 ms)` on prompts), 4 search threads (= `--bot-threads`
default). Composition of the 20: 8 searched (mean 1 207 ms, max 1 346 ms vs
the 2 300–2 500 ms deadlines), 6 heuristic-delegated (roll / end / discard,
~1–5 ms), 6 cache reuses (`reused: true` — a refused `act` re-asks the same
information set and the service answers it from the decision cache, so the
cache pays off even without an idle ponder matching). With `--ponder` sent
for the decision view first, 10 / 16 come back `reused` and 0 fall back.

Idle ponders themselves are the speculative half: the drive's 200 ms idle
probe sends `op: "ponder"` for the seat's current view (non-blocking, one in
flight per seat, cancelled when the seat's own decision arrives,
`server/src/lib.rs::spawn_ponder`). An idle view has no searchable actions
(`action::surface` is `None`), so the service answers it in ~1–5 ms without
caching — the cache entry appears only when a **decision** view is pondered,
which is why the reuse numbers above come from re-asks and from the
`--ponder` harness rather than from live idle traffic. The wiring is what
`docs/BOT.md` §3.5 specifies; the same-key cache path is pinned by
`crates/server/tests/http.rs::a_decide_after_a_ponder_for_the_same_key_is_answered_from_cache`.

### D2 deck-book trial (2026-10-07)

`crates/game-rules/examples/deckbook.rs`, 40-round cap scored at the cap
(`scoring.rs` weights), real `WasmRules` / wasmtime, 12 threads, 20-minute
budget, 3 characters (`--characters 户山香澄,美竹兰,仓田真白`), 24
games/candidate, `--rung0 4`, 1 greedy round of 12 neighbour games,
`--fp-rounds 1`, `--levels me`, `--seed 1`. Output
`target/scratch/deckbook/` (not `data/deck_book.json`).

| | |
|---|---|
| games | 1 148 (户山香澄 1 020 + 美竹兰 128; 仓田真白 never started) |
| ms/game (40-round cap) | **12 082** mean of per-game wall; spot check 7 500 on n = 42 |
| parallel speedup | 7.5× on 8 threads (355 game-s / 47 s wall); ~10 cores busy on 12 |
| racing | ablations 21 → 11 → 7 → 5 over rungs 4/8/16/24 |
| 户山香澄 best | `swap:PPP:抓到了->通用:GREAT` 12 229 ± 1 318 (n = 24, rank 3.38) vs preset 10 876 ± 1 276 (n = 24) — gain +1 353, EMIT |
| 美竹兰 best | `add:通用:@Tsugu ycm` 31 779 ± 4 353 (n = 4) vs preset 28 296 ± 4 134 (n = 8) — under-powered, budget cut the top-up |

Score variance is large (± 1.3 k on a 12 k mean at n = 24), which is why the
real run wants 300–1 000 games per candidate and a held-out margin check.
Full-run wall estimates for 54 characters on 16 threads are in §3.7 (~11 /
~22 days at 300 / 1 000 games per candidate, 12 s/game, 2 greedy rounds).

### B6: browser worker bundle (2026-10-08)

Same machine as B0. The bundle is `crates/bot-glue` built by
`tools/build-bot-glue.mjs` into `webui/public/assets/engine/bot-glue/`; the
measurement harness is `tools/measure-bot-workers.mjs` (node `worker_threads`,
one `bot-glue` wasm instance each, the real built ruleset `dist/cards` /
`webui/public/assets/rules`, views taken from a live solo match at the
searched surfaces only). The main page's glue (`webui/src/wasm/glue_bg.wasm`)
is unchanged.

```
node tools/measure-bot-workers.mjs
```

**Bundle size** (lazy: downloaded only when an advanced seat / 进阶 托管 is in
use):

| | bytes | |
|---|---|---|
| `bot-glue/glue_bg.wasm` | **5 929 030** | 5.65 MiB |
| `bot-glue/glue.js` | 15 249 | 14.9 KiB |
| main page `glue_bg.wasm` | 6 585 567 | unchanged |

**Iterations per decision, real ruleset, wasmi** (3 searched surfaces per
cell, `budget` = the search budget each worker gets -- they run in parallel):

| workers | budget | iters/decision | wall ms (mean) | scale-up |
|---|---|---|---|---|
| 1 | 1 000 ms | **3.3** | 1 482 | 1.00x |
| 4 | 1 000 ms | **12.3** | 1 574 | **3.70x** |
| 1 | 3 000 ms | **6.3** | 3 249 | 1.00x |
| 4 | 3 000 ms | **22.3** | 3 643 | **3.53x** |

Root-parallel is near-linear, as on the server (§5 B7). The wall clock
overruns the budget by one iteration (the ISMCTS loop finishes the one it is
in) -- the same shape as B4/B5/B7, and the reason the outer deadline is
`budget + 2 s` (`botBudget.ts::outerDeadlineMs`). wasmi is the expected 3–5×
slower than the server's wasmtime (§3.6 follow-up): 3.3 iterations at 1 s
where the service's `latency` harness shows 2.1 on the real ruleset at the
same budget with 1 search thread is the same order of magnitude as the B0
wasmi/wasmtime split, not a regression.

**What landed** (the gate):

* `cargo test -p game-core -p game-rules -p bot-core -p bot-service -j 2`.
* Node: `botBudget.test.ts` (16), `botDrive.test.ts` (10),
  `botPool.test.ts` (11, including the real-ruleset `decide` range, the
  seat-view-only boundary, seed determinism, the ponder cache, and
  **a solo match with an Advanced bot records and replays clean**) plus the
  existing `ruleset` / `autopilot` / `record` / `engineBundle` suites --
  76 passing.
* `tsc --noEmit` clean; a temp-outDir rsbuild build is green (the main glue
  is 6 585.6 kB, unchanged).
* `webui/src/game/botBudget.ts` / `botPool.ts` / `botDrive.ts` are the page
  side; `webui/public/assets/engine/bot-worker.js` is the plain module
  worker; `crates/bot-glue` is the wasm32 bundle.

**Deploy notes.** `node tools/build-bot-glue.mjs` next to
`tools/build-glue.mjs` (the prebuild can call both); the output
`webui/public/assets/engine/bot-glue/` is a static asset directory the
worker imports -- it is NOT part of the engine bundle and the archive /
replay path does not need it (`docs/REPLAY.md`). The solo 想考时间 slider
persists as `bm.botSoloCapMs` (ms, default 3 000).

## 6. Risks

1. The rollout cost may stay too high even native; the horizon and searched
   decision set are the levers.
2. Server wall-clock: 1.2 s choice prompts and the 5 s op timeout; the
   service must be budgeted and anytime.
3. Determinization bias: a sampler that ignores a public sighting quietly
   misleads the search. Guarded by `bot-core`'s consistency tests (every fork
   reproduces the view exactly; opponent samples ⊆ allowed − sightings) —
   keep them green whenever the sampler changes.
4. Native vs wasm drift (usize width, panics); caught by the drift check,
   harmless to matches by §1.

**Workspace note (pre-existing).** `cargo test -p server -p rules-native`
(or any run that builds both) fails `LNK2019` on all 289 `bandori_*` symbols:
feature unification turns `card-sdk/guest` on for the server test binaries
(rules-native enables it), which compiles `ctx.rs` in, but the `#[no_mangle]`
shims live in `rules-native/src/symbols.rs` and only link into binaries that
reference `rules_native::`. Run the suites in groups: `cargo test -p game-core
-p game-rules -p rules-cond -p bot-core -p bot-service`, then `cargo test -p
server`, then `cargo test -p rules-native`. A real fix is a shim table in
`game-rules` (G5 territory).
