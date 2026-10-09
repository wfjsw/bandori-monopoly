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
**C2 shipped** (early stopping + next-decision ponder; §3.4 "Early stopping",
§3.5 "What gets pondered", §5 "C2" has the numbers -- CPU/match −10 % on the
controlled `bot_cpu` pair, requests −35 %, and the StubRules strength A/B at
the C1 record).
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
  for a turn surface with no paid option left. Trivial prompts (≤1 option,
  mulligan) and everything else (roll, end, discard) are delegated to the
  heuristic — the engine's `aiAnswer` for prompts, an `ai_step`-shaped policy
  for the turn surface.
  * **Decline scope (post-bisect, 2026-10-08).** Decline is only on the menu
    when the turn would otherwise have **no** action (an unaffordable /
    gate-refused buy — the stall). A legal Buy/Build stays forced: the §5 C1
    bisect showed that letting the search pass on a legal buy costs ~5 wins
    on StubRules (`net_worth` is buy-neutral at the instant, so the tie goes
    to the bias, which leans Decline below `BUY_RESERVE`). See §5 C1.
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
* **Early stopping (time management, 2026-10-08; BOT-RESEARCH §5).** Stop the
  search before the budget when the best root action cannot be overtaken, so
  easy decisions cost a fraction of the budget and hard ones keep all of it.
  `SearchConfig::early_stop` (off by default so tests stay deterministic; on
  in `bot-service` / `bot-glue`, `--no-early-stop` / `set_search
  {"early_stop":false}` to disable). Rule, in [`early_stop_should_break`]:
  * **Visit-lead rule (primary).** The pick is by root visits, and every further
    iteration adds at most one visit to any single root edge -- so the
    runner-up can gain at most `iters_left` visits before the deadline. When
    `leader.visits − runner_up.visits > iters_left` the pick is settled and the
    loop breaks. `iters_left` is estimated from the **measured** per-iteration
    rate over the search loop and **inflated 1.3×** plus a +1 margin (an
    underestimated remainder would stop while a catch-up is still possible =
    a strength risk; an overestimate only costs a little CPU).
  * **Guards.** At least `min_iterations` (default 4) iterations, and every
    root action tried at least once -- otherwise an unexpanded action is not
    yet in the running and the "leader" is an artifact of expansion order. 4
    is below the real ruleset's ~4-iteration-per-thread floor (one wasmtime
    rollout is hundreds of ms), so the rule can fire there too; the
    all-tried guard is the real protection against an expansion-order
    artifact.
  * **Value-CI rule (optional, `early_stop_ci`, off by default).** Also stop
    when the leader's value mean's 95 % normal-approximation lower bound
    exceeds the runner-up's upper bound (`mean ± 1.96·se` with
    `se = sqrt((E[x²]−mean²)/n)` from the tracked second moment). Documented
    and left off: the visit-lead rule alone is what the strength A/B kept at
    parity (§5 C2).
  * **Determinism.** Wall-clock dependent by construction, so tests that pin
    seed+threads reproducibility run in the **iteration-count mode**
    (`SearchConfig::deterministic()` / `max_iterations` with a wide budget /
    `early_stop: false`, which is the default) -- the iteration cap binds and
    the result is a pure function of the seed and thread count
    (`crates/bot-core/tests/root_parallel.rs`). Root-parallel: **each thread
    early-stops on its own stats** (thread 0's stream is unchanged, so N=1 is
    bit-identical to the single search); the merged outcome reports
    `early_stopped` when any thread stopped early.
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
  `bot-service` keeps one such tree per `(room, seat)`. A speculative
  `ponder` (§3.5) instead keeps the **whole** explored tree -- the real turn
  has not chosen an action yet and a warm superset is what a later `decide`
  re-attaches to.

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
* **Ponder (B7, BOT-RESEARCH #5; next-decision target, 2026-10-08).**
  Optional `op: "ponder"` with the same view frame (no `prompt_id` needed):

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
  immediately. `bot-service --no-ponder` disables the whole path.

  **What gets pondered (2026-10-08).** An idle view has no searchable
  surface -- the B7 polls searched **0 of ~14 k** ponders
  (`target/scratch/profile/REPORT.md` §6). Instead of sending those, the
  drive only ponders when the seat's **next own decision is near**
  (`bot_core::view::next_turn_near`): it is already our turn (mid-routine /
  just began), the current player's turn is at 结束, or we are the next active
  seat in the ring. The service then searches the **predicted turn-start 运营
  surface** (`bot_core::view::predict_upcoming_view`) -- a `SeatView` with
  `turn = us`, `step = OPS`, the empty prompt, and exactly the turn-boundary
  fields `next_turn` resets (`landed = -1`, `bought` / `built` / `skip_move`
  cleared, `can_roll_here` on, `can_end_here` off) -- not the dead idle
  frame. Everything else (own hand / draw composition, every other seat's
  public record) is taken as-is. The prediction's decision key matches the
  real turn-start decision when nothing else about the public state changes
  before that turn begins; then the real `decide` answers `reused: true`
  with zero search. Otherwise the pondered tree is still a warm start for
  the seat's real search (tree reuse) and the CPU went to a real decision
  surface. A `ponder` whose prediction fails (`"noop": true`) is a cheap
  no-op -- no search, nothing cached. **One searching ponder per seat per
  public turn** (`(round, turn)` of the incoming state): the "next turn is
  near" gate alone still fires every 200 ms while it holds, and without this
  bound the speculative search would cost more CPU than the decide it might
  save; the rest of the idle probes are no-ops. **Server side**
  (`server/src/lib.rs::spawn_ponder`): the 200 ms idle probe sends
  `op: "ponder"` only when
  `next_turn_near`, non-blocking (its own tokio task, the tick never waits),
  one in flight per seat, cancelled / ignored when that seat's own decision
  arrives. **Browser** (`botDrive.ts::ponderUpcoming` / `session.ts`): the
  same gate and the same prediction inside `bot-glue`, one in flight per
  seat, rate-limited to 200 ms, budget `ponderBudgetMs` (a slice of the §3.6
  budget rules -- solo `cap/3` capped at 1 s, timed 300 ms).
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

Status: designed 2026-10-08 (user request). **S1 shipped** (params + plumbing + book format / loader / back-off lookup, empty book; §3.8 S1 gate). S2 (derivation) next.

**Why.** Outcomes are dominated by dice, and each character's real choices are few and recurring: which colour groups to buy, how much cash to hold, how hard to bid, when to play each card, when to press each skill, when to counteract. A small set of **tuned parameters per character** captures much of the advantage at near-zero runtime cost. It also improves everything built on the heuristic:
- the standard bot and 托管 play it directly;
- the ISMCTS rollouts use it as their policy (better rollouts → fewer needed);
- the search uses it as its progressive-bias prior (§3.4).

**Parameter set** (`game-core/src/strategy.rs` `StrategyParams`: a flat, versioned, serde struct of integers / milli-fractions; every field has a default equal to today's constant in `engine/ai.rs` / `bot-core` / `autopilot.ts`, so the default book changes nothing). The inventory S1 wired up:

| family | parameter | default | today's constant / effect when neutral |
|---|---|---|---|
| buying | `buy_reserve` | 2 000 | `BUY_RESERVE` |
| | `buy_reserve_mid` / `buy_reserve_late` | 2 000 / 2 000 | phase split from `phase_mid_round` (20) / `phase_late_round` (40); all equal = no phases |
| | `buy_group_weight[g]` | empty = 1 000 each | per-`TileData::group` milli weight on the price; 1 000 = price unchanged |
| | `set_complete_bonus_milli` | 0 | discount for a group-completing buy; 0 = off |
| | `max_price_ratio_milli` | 0 | price / cash cap; 0 = off |
| building | `build_reserve` | 3 500 | `BUILD_RESERVE` |
| | `build_group_weight[g]` | empty = 1 000 | per-group milli weight on the cost |
| | `target_houses[g]` | empty = ∞ | soft per-group house cap; absent = rulebook's only |
| auctions / force-buy | `force_buy_reserve` | 4 000 | `FORCE_BUY_RESERVE` |
| | `auction_worth_lo_milli` / `auction_worth_span_milli` | 600 / 700 | `0.6 + U(0, 0.7)` of the quoted worth |
| | `auction_cash_margin` | 1 000 | `min(worth, money − 1000)` |
| | `bid_step` / `bid_nudge_steps` | 100 / 3 | the `min + 100·U(0..2)` nudge (clamped to the auction's legal raise) |
| | `bid_frac_milli` | 750 | the search abstraction's `base · 3/4` bid level |
| card play | `play_card_chance_milli` | 700 | `PLAY_CARD_CHANCE` 0.7 (`700/1000.0 == 0.7` exactly) |
| | `max_plays_per_turn` | 2 | `MAX_PLAYS_PER_TURN` |
| | `play_card_reserve` | 2 000 | `bot_wants_play_card`'s `estCost` reserve (was `BUY_RESERVE`) |
| | `cards[id]` | sparse, default neutral | `play_weight_milli` 1 000 (0 = never), `hold_for_counteract` false, `min_round` 0 / `max_round` ∞, `min_cash_after_est` null (= the global reserve) |
| skills | `skills[id]` | sparse, default **never press** | `play_weight_milli` 0 (today's standard policy; 1 000 = always), round window; `min_fires` / `min_crystals` / `keep_markers` are tuner metadata in S1 |
| counteraction | `counteract_propensity_milli` | **600** | base [反击] declare propensity for an unlisted card (user ruling 2026-10-08, "bots must be able to counteract"; the old default was 0 = never). Drawn per offer from the match RNG like chaos's `CHAOS_COUNTER_CHANCE` |
| | `counteract[id]` | sparse; a listed card is its own spec | `propensity_milli` 0..=1000 (**0 = hold this card back**), plus `by_kind[window]` overrides. Unlisted cards take `counteract_propensity_milli` |
| mortgage / redeem | `mortgage_house_key_milli` / `mortgage_price_key_milli` | 1 000 / 1 000 | the `(houses>0, price, t)` sort key; 1 000/1 000 = that order exactly, 0 = ignore that component, negative = flip |
| | `redeem_reserve` | 4 000 | `REDEEM_RESERVE` |
| search priors | `prior_buy_yes_milli` … `prior_counter_declare_milli` | 800 / 200 / 800 / 200 / 300 / 400 / 300 / 400 / 150 / 900 / 700 / 500 | `action_priors`' f64s (0.8 / 0.2 / …), `milli/1000.0` |
| deck | `StrategyEntry.deck` | empty = any | link to the deck-book entry the params were tuned with (§3.7) |

Per-card and per-skill entries are sparse maps (absent = the default row). A
per-counteract entry is the card's own spec (0 = hold it back); a card with no
entry takes `counteract_propensity_milli`. Chaos is **not** parameterised: it
keeps `CHAOS_RESERVE` / `CHAOS_COUNTER_CHANCE` as literals.

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

**Shipped (S1, 2026-10-08).** `data/strategy_book.json` (optional -- absent or empty = behaviour unchanged), loaded by `GameData::load` into `GameData::strategy_book` and resolved by `strategy::for_seat` / `StrategyBook::resolve` (`crates/game-core/src/strategy.rs`) from `engine/ai.rs` (`Cx::strategy_of`), `bot-core`'s `heuristic_message_view` / `action_priors` / `HeuristicProvider` and web-glue's `strategy_for` (which `autopilot.ts`'s `AutopilotCtx.strategyFor` reads). Not in `data_sha256`, like `deck_book.json`. Levels are `char_bands` / `me` / `band`; a stale ruleset hash, policy or `params_version` ignores the whole book (warned once). An entry's `params` is a partial object merged over the defaults (`#[serde(default)]` per field); a deck-tagged entry only applies to that deck (or when the deck is unknown).

**S1 gate (2026-10-08).** With the shipped empty book: `cargo run -p game-core --release --example sim -- 50 4 200` counts identical to the pre-S1 baseline (avg rounds 196.3; end reasons `last 11 / settle 39`; `buy 2899`, `forcebuy 392`, `auction won 255`, `rent 19567`, `bankrupt 96`, …), and `cargo run -p game-rules --release --example ckpt_equiv -- 3 4 60` (real `WasmRules`, seeded games, `Match::save` hash at every turn boundary) is byte-identical before / after. Lookup tests: `crates/game-core/tests/strategy_book.rs` (back-off order, first-hit, stale hash / policy / `params_version` fallback, partial-params merge, deck-tagged entries, defaults == the old formulas incl. the auction float expression and the mortgage sort, and an in-memory book whose `buy_reserve` blocks every purchase -- the plumbing proof). `webui/src/game/autopilot.test.ts` pins that the policy reads the glue's params and asks `strategyFor` for the public key.

**Derivation (offline):** extend the D2 tool (`crates/game-rules/examples/deckbook.rs`) into a joint deck + strategy tuner:
- **Optimiser:** CMA-ES or coordinate / racing search over the per-character parameter vector (start with the ~10–20 highest-impact scalars: reserves, group weights, bid fraction, counteract propensity), then the sparse per-card weights for that character's deck.
- **Evaluation:** common random numbers, match score at a round cap (as D2), successive halving, fictitious-play rounds with opponents on the current book, held-out seeds for acceptance (must beat defaults by a margin).
- **Order:** band level first (≈10 bands, cheap), then characters. Popular characters first if budget-limited.
- **Cost:** real-ruleset games (~12 s each today). It gets much cheaper with the engine work (B1/B2, window optimisations, profile fixes). Schedule the full run after those.

**Phases:**

| phase | content | gate |
|---|---|---|
| S1 | `StrategyParams` + defaults equal to today's constants; plumb into `ai.rs`, bot-core rollout policy and priors, autopilot; book format + loader + back-off lookup (empty book) — **done** (shipped `data/strategy_book.json` is the empty placeholder; §3.8 gate) | byte-identical behaviour with the empty book (sim counts, seeded real-rules checkpoints); lookup tests |
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
| S1–S3 | strategy book (§3.8): params + plumbing (S1) — **done**; tuner + band trial (S2), full run (S3) | §3.8 phase table |

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

### Counteract window fast path (2026-10-08)

The live bot-service profile (`target/scratch/profile/REPORT.md`) put ~80 % of
bot CPU under `RulesBridge::counteract`: per trigger it built a world copy, a
CEL `WindowScope`, `can_counteract_now`, a `Trigger` and repeated `cant_play` /
guard probes -- **even when nobody could respond**, and again on every lap of
the ring. Two exact optimizations (docs/GUARDS.md §4.5, no behaviour change;
`examples/ckpt_equiv.rs` A/B is byte-identical on standard and chaos seeds and
the StubRules sim counts are unchanged):

1. **Valid-option-first.** Before any per-window machinery, a pre-scan asks
   whether any seat can respond (seat eligibility → per-card kind bitmask →
   compiled conditions against a window context built straight from the live
   world, no `Run` / `world_copy`). Zero survivors skips the whole ring. Offers
   run lazily: a G4-deleted guard (condition alone decides) is eligible with
   zero world copies; only a surviving entry that still carries a guard builds
   a `Run`. The CEL scope is built only if some candidate has a condition, and
   shared across the ring.
2. **Per-window processed set.** Within one ring (across laps) each
   `(seat, card)` probe's verdict is reused when the inputs it reads cannot
   have changed -- the ring's only world change is a declaration removing a
   card from a hand, so a verdict that reads no hand field is stable for the
   window. `cant_play` in `view_extra` is likewise memoised per distinct card
   id within one view, and `Ruleset::cant_play` skips the CEL scope entirely
   when the entry has no condition.

Measured with `bot-cost` (`cargo run -p game-rules --release --features
bot-cost --example bot_cost -- 4 4 120`); the "slow" column is the **same
binary** with `BGD_COUNTERACT_SLOW=1` (pre-scan never skips, processed set
bypassed, `declare_one` builds the old per-offer `win_run` + per-candidate
`Run`). 4 games × 4 standard bots, real `dist/cards`:

| | slow (old path) | fast (new path) |
|---|---|---|
| [反击] windows/game | 22 403 opened | **0 opened, 22 403 skipped** |
| counteract probes/game | (in `declare_one`) | 0 in this bench (see below) |
| world clones/game | 121 889 | 121 889 |
| ms/game (real ruleset) | 14 639 | **13 210** (−10 %) |
| inline rollout N=1 / N=2 / N=3 | 36.2 / 90.9 / 132.9 ms | 36.4 / 89.6 / 131.9 ms (noise) |

Every seat was an AI (`can_counteract_now` rejected AI seats -- C# parity), so
`declare_one` never ran in an all-bot bench: the 22 403 windows were raises
nobody could answer, and the win there was the pre-scan skipping the whole ring
(answer-tree `Trigger` clone, `Priority`, the per-visit loop). Two changes land
on top of that bench (both 2026-10-08): the AI exclusion is gone ("is a bot" is
not a rulebook reason; out / [除外] / `CannotPlay` still are), and standard's
default [反击] propensity moved from "never declare" to
`DEFAULT_COUNTERACT_PROPENSITY_MILLI` = 600‰ (user ruling *"bots must be able
to counteract"*). Same bench (4 g × 4 standard bots, real `dist/cards`):

| | AI seats excluded | bots offered, propensity 0 | bots offered, default 600‰ |
|---|---|---|---|
| [反击] windows/game | 0 opened, 22 403 skipped | 176 opened, 22 510 skipped | 141 opened, 22 263 skipped |
| counteract probes/game | 0 | 180 | 142 |
| **declared/game** | 0 | 0 | **5.2** |
| prompts/game | 355.2 | 365.0 | 356.5 |
| world clones/game | 121 889 | 123 845 | 124 595 |
| ms/game (real ruleset) | 15 060 | 17 052 | 14 029–15 558 (2 reps) |

The pre-scan still skips 99 % of raises: no seat (human or bot) holds a card
whose kind bitmask + compiled condition admits. The opens are its known
conservative gap (a condition survivor opens the ring; a card the residual
guard would then reject contributes no option, exactly as a quiet lap -- see
`docs/GUARDS.md` §4.5). **Standard bots declare 5.2 [反击] cards per game** at
the default 600‰ (each offer draws from the match RNG, first fired card wins),
and every game still finished normally (`end reasons {"settle": 4}`, the
120-round cap). The window / probe counts shift run to run because the
propensity draws move the RNG stream and therefore the game trajectories; wall
time is ±10 % between reps of the *same* binary. StubRules sim counts are
**bit-identical** across all three states (same prompts / events / rounds / end
reasons) -- `StubRules` has no hand-counteraction window
(`CardRules::counteract`'s default is a no-op), so bots cannot counteract
there; its ms/game moved 175 → 188–203 on a busy machine, under the 323
ms/game gate.

Against the **original tree** (before this change *and* the concurrent engine work) the
same bench showed the `can_counteract_now` world copy per visit going away too:

| | original tree | new tree |
|---|---|---|
| cx `world_copy`/game | 159 790 | **73 480–80 892** (−50 %) |
| total world clones/game | 193 258 | **107 405–121 889** (−37 %) |
| ms/game | 14 407 | 11 255–13 210 (−10–22 %) |

The `declare_one` machinery the live profile blamed (~80 % of bot CPU under
`counteract`) is the per-offer `win_run` world copy + `WindowScope` +
per-candidate `Run`/guard. In the mixed 1-human + 3-bot live case the human
*can* respond, so the pre-scan's condition stage and the lazy offer path (G4:
zero copies for a condition-only card) are what remove that cost. Exactness is
unchanged either way -- see the `ckpt_equiv` A/B below.

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

### C1: forced-buy refusal loop + trivial decisions (2026-10-08)

Two `bot_cpu` §6 findings, fixed together (they share the 结束 surface).

**Correctness: the forced-buy refusal loop.** At `turn/end` the action set had
no decline and `action::buyable` had no funds / eligibility gate, so an
unaffordable Buy was proposed, refused (`err.buy_poor` / `err.cannot_buy` /
`err.poor`), and the service replayed the cached refusal every 200 ms until
the turn bank expired — **45–54% of match time stalled**, live matches
included. Fixed by (a) `MatchState::can_buy_here` / `can_build_here` /
`can_roll_here` / `can_end_here` (the engine's own `why_not_act` predicates,
computed in `Match::state`), (b) `Action::Decline` (`act: "end"` at 结束)
always on the menu where the engine accepts `end`, and (c) defence in depth:
on an `act` refusal the drive drops the cached answer (`op: "invalidate"`)
and applies the heuristic immediately, never re-asking
(`server::apply_bot_answer`; `botDrive.ts` already fell back, now also drops
the worker cache). The 运营 `err.roll_first` residue was the public
`skip_move`'s stay/exile re-derivation disagreeing with the engine's latch
(unstoppable / mid-turn [停留] / [除外]); the heuristic picks roll vs end by
`can_roll_here` / `can_end_here` now.

**CPU: don't search trivial decisions.** `action::trivial_decision` answers
immediately (`heuristic: true`, `iterations: 0`) when the root has exactly one
legal action (forced) or when no candidate moves money / ownership / cards
(a pure decline / pass / skip menu). Buys, builds, force-buy offers, auctions
with a real bid, [反击] declarations, card plays, agent picks, tile choices and
mortgages always keep the full search. Lives in `Ismcts::search` and in both
`decide` paths (`bot-service`, `bot-glue`), so the browser bot gets it too.

**`bot_cpu`, 1 match, 30 rounds, 4 search threads, real `dist/cards`
(the §6 baseline in parentheses):**

| | before | after |
|---|---|---|
| stalled game time | **45–54%** | **0 s (0%)** |
| `act` refused | thousands (`err.buy_poor` / `err.cannot_buy` / `err.poor`) | **0** |
| searched decides with 1 legal action | 27–34% of searched CPU | **0 calls, 0 CPU-s** |
| game time / requests | — | 510 s / 6 956 |
| bot CPU / match | 735 s (4 threads) | 562 s |
| searched decides | 97% of bot CPU | 97.7% (122 calls, 4.5 CPU-s each) |
| idle ponders | 2.5% (0 of 14 540 searched) | 2.2% (0 of 6 658 searched) |
| iterations / searched decide | 8.6 | 9.9 |

The fast path absorbs every forced root (`one_action_calls: 0`); the 176
delegated decides of 298 are the roll / end / discard surfaces plus the
trivial ones. Wall time per decide still runs 131–146% of the budget (the
last iteration overruns) — unchanged, out of scope here. Inside one search
the split is now fork 0.4% / descent 9.7% / rollout 89.8% (§2's
counteract / `cant_play` / buy-quote costs; the engine-efficiency items in
§5's fix table are still open).

**Gate:** `cargo test -p bot-core -p bot-service -p server -p game-core` all
green (incl. `bot-core/tests/action_legality.rs` — an unaffordable tile
offers decline, not Buy; and `server/tests/bot_drive.rs` — a refused service
answer falls back without re-asking and the cache is dropped). Node
`botDrive` / `botPool` / `botBudget` 37 passing; `tsc --noEmit` clean;
`node tools/build-bot-glue.mjs` green.

**Strength bisect (StubRules, n = 100, 200 ms/decision, 4 search threads,
`ismcts_vs_bots -- 100 200 0 --threads 4`, same seeds, all cells sequential
on one machine).** Measurement knobs (`action::ab_on`, env, never set in
production): `BOT_STRENGTH_AB_NO_DECLINE`, `_NO_FASTPATH`, `_NO_LOWSTAKES`,
`_NO_GATES`. The baseline cell is a `git worktree` of `7d67911` (the last
commit before C1, which still has B7) built into its own `CARGO_TARGET_DIR`.

| cell | wins | rank | score | rounds | searched | refused | wall |
|---|---|---|---|---|---|---|---|
| a **shipped** (decline + gates + fast path) | 23 | 2.54 | 56 543 | 175 | 8 168 | 16 | 658 s |
| b fast path OFF entirely | 22 | 2.58 | 54 444 | 175 | 8 139 | 19 | 1 683 s |
| c LowStakes OFF (Forced on) | 23 | 2.56 | 55 842 | 176 | 8 154 | 13 | 651 s |
| **d decline OFF** | **27** | **2.41** | **65 751** | 180 | 3 663 | 10 | 468 s |
| e `can_*_here` gates OFF | 23 | 2.57 | 57 508 | 175 | 8 230 | 14 | 682 s |
| **f baseline `7d67911` (pre-C1)** | **28** | **2.41** | **63 264** | 178 | 3 828 | **217** | 804 s |
| **g narrow decline (shipped after bisect)** | **30** | **2.41** | **65 808** | 178 | 8 908 | 8 | 456 s |
| (B7 record, 10-07) | 30 | 2.44 | 64 261 | 178 | — | 8 % | — |

**Cause: the Decline option next to a legal Buy/Build.** d and f land on top
of each other (27–28 wins, rank 2.41, score 63–66 k) and on top of the B7
record; a, b, c and e all sit at 22–23. The mechanism: at 结束 with an
affordable buyable tile the root became `[Buy, Decline]`, and
[`eval::net_worth`] counts a deed at its price — a buy is worth-neutral at
the instant, and the 2-round rollout only sometimes sees the rent. The tie
goes to the progressive bias, which leans Decline whenever
`money − price < BUY_RESERVE` (2 000) — exactly the region where the old
bot was forced to buy. Fewer deeds → less rent → ~9 k less score. The
fast path (b), LowStakes (c) and the legality gates (e) are strength-neutral
(within the ±4 pp binomial SE at n = 100); the gates are what cut refusals
from f's 217 to a–e's 10–19.

**Shipped default after the bisect:** Decline is offered only when the turn
surface would otherwise have **no** action (the unaffordable / gate-refused
buy of the stall). A paid option the engine accepts stays forced — the
search cannot pass on a legal buy (that is the harmful half). The empty-menu
Decline is strength-neutral by construction (its answer is the same `end`
the heuristic gives) and keeps the stall fix and the
`unaffordable_tile_offers_decline_not_buy` contract; the re-measured default
(cell g) is **30 / 100, rank 2.41, score 65 808** — at the B7 record. If the
pass-on-a-legal-buy choice is wanted back for the real ruleset, the tuning
surfaces are `prior_buy_yes` / `prior_buy_no` (`docs/BOT.md` §3.8) or a
narrower Decline (e.g. only when `wants_buy` is false) — both are search
guidance, not legality.

### C2: early stopping + next-decision ponder (2026-10-08)

Two search-efficiency items (`docs/BOT-RESEARCH.md` §5 "time management" and
"pondering"; `target/scratch/profile/REPORT.md` §5 fix I). Both cut CPU
without touching play strength -- the A/B below is at parity with the C1
default.

**1. Early stopping (`bot-core::ismcts`).** The search stops before the
budget when the best root action cannot be overtaken. Rule and guards in
§3.4 "Early stopping"; `SearchConfig::early_stop` (off by default so the
iteration-count determinism mode is unchanged; on in `bot-service` /
`bot-glue`). Root-parallel: each thread stops on its own stats. Deterministic
given seed + threads whenever the iteration cap binds
(`crates/bot-core/tests/root_parallel.rs`).

**2. Next-decision ponder (server `spawn_ponder` + browser `ponderUpcoming`).**
Idle ponders used to send the seat's idle view, which has no searchable
surface -- 0 of ~14 540 ever searched (`REPORT.md` §6). Now:

* the drive only ponders when the seat's next own decision is **near**
  (`next_turn_near`: our turn / the current turn is ending / we are next in
  the ring);
* the service searches the **predicted turn-start 运营 view**
  (`predict_upcoming_view`) instead of the dead idle frame -- `playable` is
  reset to all-true on the prediction (the live `cant_play` answers "no" for
  every card while another seat acts, which is what made every speculative
  root look empty);
* **one searching ponder per seat per public turn**; the rest of the idle
  probes are cheap no-ops (1.4 ms parse + hash). Without that bound the
  "near" gate alone still fires every 200 ms and the speculative search would
  cost more CPU than the decide it might save;
* the result is cached by `decision_key` and the **whole** explored tree is
  kept per `(room, seat)`, so a real `decide` on the same key answers
  `reused: true` with zero search and any other decide starts from a warm
  tree.

**`bot_cpu`, 1 match, 30-round cap, 4 search threads, real `dist/cards`
(`ruleset_sha256 39fd3686…`), same seed 1.** The two "this binary" rows are
the clean comparison (identical code, only the knobs differ: `--no-early-stop
--ponder-all` = the pre-C2 search + the pre-C1 "ponder every idle probe"
traffic). The "C1 record" row is the §5 C1 number -- a **different game
trajectory** (the bot's answers differ, so the surfaces it faces differ),
given for scale, not as a controlled delta.

| | C1 record | this binary, old knobs | **this binary, shipped** |
|---|---|---|---|
| bot CPU / match | 562 s | 380 s | **342 s** |
| game time | 510 s | 444 s | 434 s |
| requests | 6 956 | 6 332 | **4 098** |
| searched decides | 122 | 99 | 84 |
| CPU / searched decide | 4.5 s | 3.77 s | 4.02 s |
| iterations / searched decide | 9.9 | 18.8 | 20.8 |
| idle ponders | 6 658 | 6 049 | **3 810** |
| of which searched | 0 | 20 | 9 |
| ponder CPU | ~12 s (2.2 %) | 37.7 s | **17.0 s (5.0 %)** |
| ponder hits (decides served from cache) | 0 | 0 | **0** |
| `act` refused | 0 | 0 | **0** |
| stalled game time | 0 s | 0 s | **0 s** |
| would fall back | — | 0 | **0** |

Reading:

* **CPU/match −10 % on the controlled pair (380 → 342 s), −39 % vs the C1
  record.** The request count is the bigger structural win: **−35 %** on the
  controlled pair (6 332 → 4 098), **−41 %** vs C1. Almost all of the removed
  traffic is idle probes that used to fire a `ponder` and now return without
  one (`next_turn_near` is false for most of another seat's turn).
* **Idle ponders −37 %** and their CPU **37.7 → 17.0 s**; the ones that
  remain are 4.5 ms parse + hash no-ops, with **one searching ponder per
  seat per turn** (9 in this match) on the predicted turn-start 运营 surface.
* **Ponder hits: 0.** Honest number. The predicted decision key matches the
  real one only when nothing about the public state changes before the turn
  begins -- and in a 4-player game another seat's turn always moves money,
  deeds or the event tail. The mechanism is in place and measured (the cache
  contract is pinned by `bot-service/tests/protocol.rs` and
  `server/tests/http.rs`), but on this ruleset the free-CPU window is better
  spent on the engine-efficiency items in §5's fix table (A/B/C) than on
  speculative search. The 9 speculative searches cost 21 CPU-s of the 17 s
  ponder bill and bought 0 cache hits; the **gate** is what pays for itself.
* **Early stopping is present but quiet on the real ruleset**: one thread
  completes only ~4 iterations per decision there (a wasmtime rollout is
  hundreds of ms), so the visit-lead rule fires mostly on blowouts. Its
  strength effect is the StubRules A/B below; its CPU effect here is inside
  the trajectory noise (84 searched × 4.02 s vs 99 × 3.77 s).
* **Stall / refusals stay 0** in every row -- the C1 correctness fixes are
  untouched.

**Strength A/B (StubRules, `ismcts_vs_bots -- 100 200 0 --threads 4`, n = 100,
same seeds, cells sequential on a quiet machine).** "old" = the C1 default
shape (no early stop, no ponder); "early-stop" = `--early-stop`;
"early-stop+ponder" = `--early-stop --ponder` (the harness models
production's `op: "ponder"`: after each of seat 0's answers it predicts the
next turn-start view, speculates on it, and answers a later decide from the
cache when the key matches). The C1 record (cell g) is **30 / 100, rank 2.41,
score 65 808**; the binomial SE on 26–30 wins at n = 100 is ~4.6 pp.

| cell | wins | rank | score | rounds | iters/decide | ms/decide | ponder searches / hits | wall |
|---|---|---|---|---|---|---|---|---|
| old | 29 | 2.43 | 64 976 | 177 | 37.1 | 53.3 | — | 488 s |
| **early-stop** | **30** | **2.41** | **66 314** | 178 | 39.8 | 48.7 | — | 452 s |
| early-stop + ponder | 26 | 2.44 | 62 332 | 177 | 39.5 | 48.9 | 4 / **0** | 449 s |
| early-stop + ponder (re-run) | 28 | 2.42 | 63 969 | 178 | 39.5 | 48.5 | 2 / **0** | 446 s |

Reading:

* **Early stopping does not cost strength.** The isolating cell is
  "early-stop": **30 / 100, rank 2.41** -- exactly the C1 record, and the
  best of the four runs. The visit-lead rule is strength-safe as shipped; no
  tightening was needed. The spread across all four runs is 26–30 wins, i.e.
  within one binomial SE of the record.
* **Wall per decision drops** 53.3 → 48.7 ms with early stop (−9 %), and the
  total search wall drops 464 → 434 s despite the cell facing more searched
  decisions (8 698 → 8 922) -- the rule is cutting the long tail of obvious
  decisions, which is the point.
* **"early-stop + ponder" is within noise of the record** (26 and 28 on the
  two runs vs 30 = 0.4–0.9 binomial SE; rank 2.42–2.44 vs 2.41). It differs
  from the isolating cell only by the harness's 2–4 speculative searches
  between decisions -- 0 cache hits on both runs -- so the gap is wall-clock
  budget jitter under the extra load, not the ponder path.
* **Ponder hits: 0 of 2–4** in the harness, matching the `bot_cpu` read: the
  predicted decision key almost never matches the real one, because another
  seat's turn always moves the public state. The mechanism is wired and
  measured; the win from the ponder change is the **removal of dead traffic**
  (the `bot_cpu` rows above), not speculative hits.

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
