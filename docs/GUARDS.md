# Guard prefilter — design (2026-10-07)

Status: **G0 + G1 + G2 + G3 + G4 landed** (2026-10-07; §8 the G1 measurements
and the CEL fork, §9 the G0/G2 wiring, §10 the G3 audit + G4 authoring pass,
§11 batch 2 + workspace hygiene). ABI **45 → 46** (`On::Counteract`/`On::Hook`
guards become `Option<fn>` (`None` = G4-deleted), `ManifestOn` gains
`has_guard`/`has_legacy`, `CardDef::legacy` + `export::OP_LEGACY_GUARD` for the
audit). The **cached counteraction index** (BOT-RESEARCH.md #1) is in
`declare_one`: per-card kind bitmask + condition pre-filter before any
`Run`/`world_copy`.

Goal: a **serialized condition** per guarded entry, emitted in the manifest and
evaluated natively by the host, so the wasm guard is only instantiated when the
condition passes. The wasm guard stays as the secondary filter.

**Distinct responsibilities (user ruling 2026-10-07).** Each layer rejects what
only it can; nothing is rejected twice:

| layer | owns | evaluated by |
|---|---|---|
| 0. category | the trigger / chain kind (`On::*` kind lists, `declares()`) | host dispatch |
| 1. condition | everything expressible in the CEL schema (§4.2) | host, natively |
| 2. guard | only the residual the condition cannot express (§6) | wasm |

So a guard **does not re-check the kind** the category filter already checked
and **does not re-check a clause its condition states** — an authoring rule,
not a language restriction (conditions and guards may still read `kind`, e.g.
per-kind clauses on a multi-kind entry). A guard whose whole body
moves into its condition is deleted (`None`). A multi-kind entry may still read
the kind to *branch behaviour*, never to reject. Consequence: the condition is
no longer an optional hint — it is part of the decision and must be evaluated
at every call site of its guard (§4.4).

## 1. Why — measurement

`docs/BOT.md` §5 (4 standard bots, real ruleset, n=3–10): **55 900 module
instantiations/game**, 49 900 `CardRules` entry calls of which **48 900 are
`counteract` guard probes** (`can_counteract`, host.rs:615-641). Fire-up (fresh
Store + instantiate) is 45 µs/run = 17 % of 14 957 ms (wasmtime) and 351 µs/run
= **57 % of 39 792 ms (wasmi / browser)**. Each probe also builds a `Run` +
`cx.world_copy()` *before* the guard (wasm_rules.rs:2658-2678).

Existing prefilters, keep as layer 0: `declares()` u128 bitmask (host.rs:367-370,
493) and per-card `counteracts_to(kind)` from the manifest kind list
(wasm_rules.rs:2655). Everything below is the layer that replaces the remaining
per-card probes.

## 2. Inventory (rules/, 310 .rs)

| family | decls | distinct fns | entry | where |
|---|---|---|---|---|
| counteract guards | 51 | 50 (`can_counteract`×48, `can_counteract2`, `can_counter_fire`, `can_negate`) | `On::Counteract` | cards + `tae_police.rs:87` |
| hook guards | 250 | ~44 (`mine`×118, `always`×19, `crystals_changed_guard`×11, `is_placed`-only ≈30, …) | `On::Hook` | cards, skills, events, tiles |
| play gates | 97 of 201 `On::Play` (`None`×104) | `cant_play`×62 + `can_use`×19 + 16 one-offs | `On::Play(Some(..))` | cards |
| skill usability | (in play gates) | `can_use` family ×35 | `On::Play` | skills/ |
| gates / settle / rollplan | no guard fn (body only) | — | `On::Gate`/`Settle`/`RollPlan` | tiles, cards |

### 2.1 Recurring patterns (50 counteract bodies; hot path)

| pattern | guards | fully expressible | example |
|---|---|---|---|
| trigger kind | 44 | 44 (multi-kind arms stay as OR-lists) | `detour.rs:34` `kind == MoveRoll` |
| actor rel (self / other / any) | ~30 | 30 | `welcome_mujica.rs:67` `actor != owner` |
| target rel (self / other / exists) | ~13 | 13 | `cant_look_away.rs:21` `target == owner && actor != owner` |
| `by_card` rel | 6 | 6 | `riot.rs:21` `by != owner && effect.hits(owner)` |
| effect-chain `has(kind)` / `hits(seat)` | 15 | 15 (flat link list, host-readable) | `tritone.rs:23` `effect.has(Pay)` |
| move/roll fields (roll present, kind, tag, source, remaining) | 13 | 11 | `sayo_play.rs:21` `move.kind == Walk && move.roll != null` |
| value threshold | 9 | 9 | `vocal_too_hard.rs:18` `value >= 5000` |
| once-per-turn flag (`slot` vs `turn_key`) | 3 | 3 | `ran_as_usual.rs:25` `slot('asUsualTurn') != turn_key` |
| money / can_pay | 3 | 2 | `crimson_soul.rs:41` `money(owner) >= 500` |
| tile owner / named / houses / mortgaged | 9 | 6 | `hey_kids.rs:40` `tile.owner == owner && houses > 0` |
| player-exists (others / neighbor / on tile) | 6 | 3 | `hold_hands_again.rs:28` `actor == neighbor(owner,-1)` |
| character / band identity | 2 | 2 | `two_in_one.rs:28` `character_is(owner, …)` |
| geometry (`next_dist`, `on_path`, `within5`, `near`, `dist`) | 6 | 0 → prefix only | `meet_again.rs:51`, `repaint.rs:37` |
| derived accounting / lists (`gains_this_turn`, `targeted_count`, `price_tag`, `grade_of`, `buildable_targets`) | 7 | 0 → prefix only | `secret_rainbow.rs:112`, `hagumi_marks.rs:172` |

**FULL** (every body condition in the atom set): **~30/50 (60 %)**. **PARTIAL**
(cheap prefix — usually kind + actor + move/effect — is the dominant rejector):
**~20/50**. Two guards are *only* a prefix over an opaque residual
(`even_lost.rs:25`, `mana_champion.rs:18`).

### 2.2 Hook and play families

| pattern | decls | example |
|---|---|---|
| `mine` = `trigger.player_id == owner` | 118 | `skills/skill-bands/src/mygo.rs:34` |
| `mine` + `skill_blocked(band)` | 3 | `aya_with.rs:36` |
| `is_placed` only / + actor | ~30 | `charity_show.rs:92` |
| `crystals_changed_guard` (card_is(ID) ∧ crystals==0 ∧ value≤0) | 11 | `crimson_soul.rs:184` |
| threshold on fire / crystals / tokens / hand / state | ~40 | `tae_police.rs:87` `fire(owner) >= 4` |
| money threshold (play) | 7 | `gacha10.rs:17` `money < 1500` |
| `cant_move` / counts / tile-distance (play) | ~45 | `your_light.rs:16` (dist residual) |
| `always` (no-op prefilter) | 19 | events `embers.rs:26` |

Overall: **~75 % of guards fully expressible, ~20 % partial, ~5 % prefix-only**;
0 guards need "no pre at all" beyond `always`.

## 3. What dominates runtime

`declare_one` (wasm_rules.rs:2638-2681) probes every distinct hand card that
declares the answered kind, per seat visit, per chain link. Responders per
`ChainKind` (static): **Effect 18, MoveRoll 10**, Card 4, Roll 3, SettleBefore 3,
Pass 2, Paid 2, 1 each ×15. Runtime frequency (≈250 probes/round × 196 rounds):
every move roll, every payment/effect declaration, every settle — i.e. the
Effect/MoveRoll/Settle*/Card windows hold most of the 48 900 probes. The
actor-relation atom alone rejects ~3/4 of the probes on `actor == owner` guards
whenever the trigger belongs to another seat (13 of 50 counteract bodies lead
with it; 118 hook decls are exactly `mine`).

## 4. Condition language — CEL vs atoms

### 4.1 Options

| | A. closed atom set | B. CEL string (recommended) | C. CEL lowered to atoms |
|---|---|---|---|
| authoring | builder / const lists next to the guard | `pre: "actor == owner && move.roll < 6"` next to the guard | same as B |
| wire | postcard enum of atoms (~1–2 B/atom) | `Option<String>` in `ManifestOn` | atom IR, lowering at load |
| eval | match ladder, ~0.1–0.5 µs | tree-walk over parsed AST, est. **1–5 µs** | as A |
| deps / size | ~2 KB | `cel` 0.15.0 → antlr4rust + nom + serde (P0 spike: pure Rust, builds wasm32; **re-verify 0.15** after the crate rename) | host keeps `cel`; browser keeps only atoms |
| expressiveness | fixed vocabulary; `money >= tile_price/2` needs a bespoke atom | arithmetic, comparisons, `||` of arms; int-only mandate | anything that fits the atom grammar; build error otherwise |
| soundness | identical (authored necessary condition + audit) | identical | identical |

**Recommendation: B, CEL as first-class**, with C as a later optimisation if
measurement says eval or binary size matters. Rationale: the guards are already
small integer predicates — CEL re-expresses them without a vocabulary committee
(`tomoe_savior.rs:22` `money(owner) >= tile.price/2`, `hold_hands_again.rs:28`
`actor == neighbor(owner, -1)`, `secret_rainbow.rs:112` grade compares), eval is
two orders of magnitude below instantiate cost, and the per-window context (§4.2)
amortises the rest. Mandate **ints only** (docs/P0-FINDINGS.md:76-85: CEL does
not mix Int/Float — reject float literals at compile); keep optional features
(`regex`/`chrono`/`json`) **off**. Spike gate: `cel` 0.15.0 must build for
`wasm32-unknown-unknown` (web-glue runs the engine in the browser) before G1.

### 4.2 Variable schema (host-side, no guest)

Built **once per chain window / trigger** (`declare_one`, `run_hook`), reused by
every candidate; the owner overlay is per candidate.

| layer | variables (ints / string ids) | source |
|---|---|---|
| window | `kind`, `actor`, `target`, `tile`, `value`, `step`, `by`, `pay_is_rent`, `move.roll`, `move.kind`, `move.remaining`, `move.main`, `roll_source`, `abnormal`, `trigger_card`, `chain.count/kind[i]/hits(seat)/from[i]` | `Trigger` + effect-link list (same data `effect::*` imports read, ctx.rs:2218-2264) |
| window | `turn_player`, `turn_key` | `Cx` / `TurnCtx` |
| candidate | `owner` (= `player_id` arg), `owner.money/fire/crystals/hand/pos/out/stay/stun/exile/no_hand`, `owner.character`, `owner.band`, `owner.tiles` (count) | `World`/`Player` |
| candidate | `card.id`, `card.placed`, `card.cp`, `slot(name)`, `tok('name')`, `tile_named(name) -> id` (every board name registered; `-1` when unknown, matching the guest) | `World` field instances + marks |

Not in the schema (residual, stays in the wasm guard): geometry, list builders,
`gains_this_turn` / `targeted_count` / `price_tag` / `grade_of`, `skill_blocked`
semantics beyond a `blocked(band)` int mirror.

### 4.2b The condition vocabulary (one definition per name)

`rules-cond`'s `vocab.rs` is **the** definition of every condition name: one
`VOCAB` entry per name, from which the schema/lint's name lists, the CEL type
info, the binding and the evaluator registration are all derived. Adding a
name = one entry + a `CondView` accessor (and that accessor's host
implementations). No edits to `schema.rs` / `eval.rs` tables.

| field | what it is |
|---|---|
| `cel` | the CEL spelling (`actor`, `move.main`, `is_circle`) |
| `aliases` | other CEL spellings that flatten to the same `flat` (`chain.count` for `effect.count`, `trigger.kind` for `kind`) |
| `flat` | the flat identifier after the `root.field` rewrite (`move_main`) |
| `scope` | `Window` / `Candidate` / `Func { arity, cand }` |
| `ty` | `Int` / `Bool` / `OptInt` (binds `null` when `-1`) |
| `get` | scalar fetch: `fn(&dyn CondView) -> i64` |
| `fx` | function shape (`Fx::SeatInt` / `StrInt` / `IntHas` / `Neighbor` / …), including the hidden table the eager evaluator bakes |
| `doc` | one line, mirrored in this table |

Accessors go through [`crate::view::CondView`] (`rules-cond/src/view.rs`): a
read-only query trait covering the trigger fields, the player/tile/card
queries, `slot`/`tok` and the tile-kind predicates -- the same vocabulary the
card SDK exposes to guests (`card-sdk` `ctx::*` reads). A condition and a
guard therefore read the world through one surface.

Host views (all call the underlying store directly -- never `self.<same
method>`, which is trait-method recursion and a stack overflow):

* `rules_cond::SnapshotView` (`view_impl.rs`) -- the eager `WindowCtx` +
  `CandidateCtx` reference the tests and the precompiled-conds runtime use.
* `game-rules::cond_pre::SnapView<S: SnapSrc>` -- any `SnapSrc` (a
  `CardWorld`, or `rules-native`'s linked world) plus the candidate being
  probed. This is the generic host accessor implementation.
* `game-rules::wasm_rules::LiveSnap` -- the live `World` + `GameData` +
  `Trigger` (+ optional candidate), reading `self.world.*` / `self.data.*`.

**Eager vs lazy.** The evaluator **eagerly copies** the used names and the
function tables into the CEL context once per window (`bind_window`) and per
probe (`bind_candidate`, only `cond.used_vars()` / `used_fns()`). A true
lazy path -- every function closure fetching through a `&dyn CondView` at
call time -- was measured (`bench_cond`: `lazy dyn-CondView money(seat)` ≈
2.4 ns/call) and does **not** beat the ~1.2 µs/eval eager mean: a full eval
is dominated by the CEL AST walk, and a `Context<'static, 'static>` wants
owned values, so lazy still needs a thread-local side channel
(`eval::with_live_view`). Keep eager filling, now generated from VOCAB
through `CondView`. `WindowScope::new` (~30 µs) amortises over the
counteract pre-scan's ~200 probes per window.

`tests/cond_tests.rs::vocab_matches_schema_lists` keeps the table in step
with the lint (both sides derive from VOCAB, so this is a regression guard
for the projections and the CEL-stdlib list); `vocab_flats_are_unique`
guards the flat namespace.

**How to add a condition name**

1. Add a `CondView` accessor in `rules-cond/src/view.rs` (if it reads
   something new).
2. Add one `VOCAB` entry in `rules-cond/src/vocab.rs` (scalar: `get` points
   at the accessor; function: `fx` picks an existing `Fx` shape and its
   `fill`/`field` closure calls the accessor).
3. Implement the accessor on each host view: `SnapshotView`
   (`view_impl.rs`), `SnapView` (`game-rules/cond_pre.rs`), `LiveSnap`
   (`game-rules/wasm_rules.rs`).
4. If the name reads **new** world state, also extend `SnapSrc` /
   `fill_window` / `fill_candidate` (the eager snapshot mapping). A name
   over existing snapshot fields needs no fill edit.
5. Run `cargo test -p rules-cond` -- `vocab_matches_schema_lists` fails if
   the lint and the table disagree.

`schema.rs` and `eval.rs` are pure consumers of VOCAB: the lint lists, the
`root.field` rewrite (via `vocab::by_cel`), the variable bindings (via
`Name::get`) and the function registration (via `Name::fx`) all iterate the
table. Adding a name never edits their bodies.

### 4.3 ABI / authoring surface

* Manifest: `ManifestOn { kind, triggers, pre: Option<String> }` (postcard,
  abi.rs:1446). No `exact` flag: a fully expressed guard is simply removed, so
  "condition only" is `pre: Some(..)` + no guard.
* `On::*` gains a `pre: &'static str` (`""` = none) immediately before the
  residual guard -- the declaration reads category → condition → guard → body,
  matching the three layers above; or a const `.pre(..)` builder. ~490
  declaration sites. Sugar consts `pre::MINE` (`actor == owner`) replaces the
  118 `mine` guards outright; the 19 `always` guards are deleted. The condition
  sits beside the (now residual) guard fn, and each clause lives in exactly one
  of the two.
* Load time: `RulesetBuilder::build` parses + compiles every `pre` once
  (TypeRegistry with the §4.2 vars); parse/unknown-var/int-literal errors are
  `RuleError::BadPre` and fail the build-ruleset check.

### 4.4 Evaluation points

1. `declare_one` (wasm_rules.rs:2647-2680): after `counteracts_to`, **before**
   the `Run`/`world_copy` and `can_counteract` call. Build one window context
   per `top` trigger; evaluate per (card, seat). This is the 48 900-call path.
2. `run_hook` (host.rs:566-611) / `drive_inner` guard step: before
   `self.store(...)`; window context per trigger, owner = card's player.
3. `cant_play` (host.rs:648-671, wasm_rules.rs:3099-3136) and the skill gates
   riding `On::Play`: before the `Run` build. Lower frequency, same schema with
   `kind` absent.
4. `can_counteract_now` (wasm_rules.rs:2922) and the `declares()` bitmask stay
   untouched (layer 0).
5. **Every other path that reaches a guard** (UI `playable` / `why_not_act`,
   `ai_play`, any host request that asks another card's guard, the future
   `rules-native` bot build) goes through one host function
   `admits(entry, ctx) = pre(ctx) && guard(ctx)`. No caller may call a guard
   directly — with clauses removed from the guards, skipping the condition
   would admit what it should reject.

### 4.5 Window algorithm (2026-10-08) — valid-option-first + processed set

The [反击] window (`hand_counteractions` / `build_round` / `declare_one`,
wasm_rules.rs) runs in three stages. The exactness contract: **same offers, same
order, same prompts, same settlements** as the naive ring — only the work done
to decide "nobody can respond" shrinks. Verified by
`examples/ckpt_equiv.rs` A/B (`BGD_COUNTERACT_SLOW=1` disables both stages):
`Match::save()` checkpoints per turn are byte-identical on standard and chaos
seeds, and the StubRules sim counts are unchanged.

**Stage 0 — open the window only if someone can respond.** Before any per-window
machinery (the answer-tree `Trigger` clone, `Priority`, per-visit offers), a
pre-scan asks "could any seat answer this link at all?":

1. seat eligibility (`can_counteract_now`: out / exiled / stunned /
   no-hand — live state reads, **no** world copy; a bot seat is eligible,
   it is a player);
2. the per-card kind bitmask (`Ruleset::counteracts_to`) over each eligible
   seat's distinct hand ids;
3. the compiled condition against a window context built straight from the live
   world (`LiveSnap`: `fill_window` / `fill_candidate` with **no** `Run`, **no**
   `world_copy`). The CEL `WindowScope` is built only if some candidate actually
   declares a condition.

Zero survivors ⇒ the whole ring is skipped. That is every raise nobody answers
(«the common case»): no chain clone, no `Priority`, no per-visit offers, no
world copies, no guards. A single quiet lap would have closed the round
identically, so the skip is exact — and if nothing's condition admits at
window start, nothing can declare, so nothing can flip a condition mid-ring
(a ring's only world change is a declaration removing a card from a hand).

**Stage 1 — offer one seat (`declare_one`), lazily.** The bitmask scan runs
first with no world access at all; a hand with nothing that answers this kind
returns before any per-window machinery exists. Survivors then run their
condition against the ring's shared scope (built lazily, see stage 2) using
`LiveSnap`. A candidate whose condition admitted **and** whose residual wasm
guard is deleted (G4, `entry_guard_is_none`) is eligible with **zero** world
copies — no `Run`, no store, no instantiation. Only a surviving entry that
still carries a guard builds the throwaway `Run` (`probe_run`, one world copy)
and goes through `can_counteract_scoped` (the one shared `admits` gate).

**Stage 2 — the per-window processed set (`ProbeMemo`).** Within one ring
(one trigger's round, across its laps) each `(seat, card)` probe's verdict is
remembered. The ring's only world change is a declaration removing a card from
a hand (`build_round`), so:

* a verdict that reads **no** hand field is stable for the whole window —
  `cond_reads_hand` (the condition names `owner_hand` or calls `hand(p)`) and
  any residual wasm guard (it may read anything) are the hand-sensitive ones;
* only hand-sensitive verdicts are stamped with a `hand_gen` that bumps on
  every declaration; the rest are reused on later laps and on the re-offer
  after a seat declares;
* the CEL scope's `_hand` table is rebuilt only when a probe calls `hand(p)`
  and a declaration has moved the hands since it was built — `owner_hand` lives
  in the candidate overlay, which is rebuilt per probe from the live world, so
  a stale scope still answers it correctly.

"Resolved" (declared) cards leave the hand and are never re-scanned; a second
copy of the same id is a new offer (the `Distinct()` shape) and reuses the
memoised verdict when the inputs are unchanged. A seat that **passed** is
re-offered on a later lap (ruling 2026-10-07 «a later seat's declaration can
re-open earlier seats») — the processed set caches the *evaluation*, never the
prompt; the offer list is byte-identical to the naive ring's.

## 5. Soundness

Invariant: **category ∧ condition ∧ residual guard ≡ the old guard.** Since
clauses move out of the guards, a wrong condition now changes behaviour in both
directions (wrongly admits as well as wrongly rejects), so the check is
equivalence, not necessity.

1. **Migration audit** (`guard-audit` feature, G3–G4 only): the authoring pass
   keeps each card's pre-migration guard as `legacy_*` under the feature; on
   every probe the host evaluates both `legacy(ctx)` and
   `kind ∧ pre(ctx) ∧ guard(ctx)` and **panics** with card id + trigger dump on
   any mismatch. Run over the `tests/rb_*.rs` suite, the seeded
   `engine`/`mentality` tests, a fuzz sim (N random games), and the local
   `.bdrec` corpus. Once a card is clean, its `legacy_*` copy is deleted.
2. **`precheck` tool** (rules/ or game-rules example): loads the ruleset,
   reports per-entry condition/guard split, rejects parse errors / unknown vars
   / float literals, and flags guards that still read the kind for rejection
   (a `kind`/`trigger.kind` compare followed by `return false`) — the category
   filter owns that.
3. **Fail-closed at build**: an unparsable condition is a `build-ruleset`
   error, never "treat as true" — the clauses it carries are gone from the guard.
   An absent condition means "no clauses beyond the guard".
4. The rulebook suites (`rb_*`) stay the behavioural spec; they must stay
   green with the audit off after each card's legacy copy is removed.

## 6. Savings estimate

Assume the prefilter rejects ~55–70 % of the 48 900 counteract probes (actor
relation + `effect.has`/`hits` + move/value atoms; §2.1) and ~30 % of hook/gate
runs (`mine` on other seats' triggers).

| | today | after prefilter |
|---|---|---|
| instantiations/game | 55 900 | **~22 000–28 000** (−50–60 %) |
| wasmtime fire-up saved | — | ~29 000 × 45 µs ≈ **1.3 s** (≈9 % of 14.96 s) + ~29 000 `world_copy`/Run ≈ 0.2 s |
| wasmi / browser saved | — | ~29 000 × 351 µs ≈ **10 s** (≈25 % of 39.8 s) |
| CEL eval cost | — | 48 900 × ~2–5 µs + 1 context/window ≈ 0.2 s (noise) |

Engine-shell share (82 %, BOT.md §5) is only partially touched (the bookkeeping
around each probe still runs); this is a **direct fire-up cut**, complementary
to B2 (inline answers), which removes the halt/replay amplification.

**Not expressible** (wasm residual; listed in §2.1): `meet_again` next_dist,
`repaint` on_path, `misaki_card` between, `council_check` near, `haruhikage`
within5, `secret_rainbow` grades, `hagumi_marks` gains_this_turn, `centrifugal`
targeted_count, `now_sumimi` price_tag, `dream_return` targets/can_pay,
`hey_kids` buildable_targets, `no_breakup` repeated_digits, `tsugushi_monitor`
all-player token scan. Each still gets its kind/actor prefix.

## 7. Phases (after purchasing; sizes are LOC-ish)

| phase | content | size | gate |
|---|---|---|---|
| G0 | ABI bump (42): `ManifestOn.pre/exact`, `On` trailing field or `.pre()` builder, `rt.rs` serialise | ~200 | manifest round-trip test |
| G1 | **done** (2026-10-07, isolated `crates/rules-cond` — §8): `cel` 0.15.0 features off, wasm32 ok; `WindowCtx`/`CandidateCtx` + `compile`/`eval`; load-time errors; int-only lint; kind rejection | ~400 | load-time errors; int-only lint |
| G2 | single `admits()` entry; evaluate at §4.4 (1)–(5); counters for skipped probes | ~250 | no direct guard calls left; no behaviour change with `pre` absent |
| G3 | migration audit (`legacy_*` equivalence) + fuzz + replay corpus + `precheck` | ~300 | rb suite green under audit |
| G4 | authoring pass: **move** clauses from guards into conditions and delete the kind checks (category filter owns them); 50 counteract + 62 `cant_play` + 35 skill gates; `mine` → `pre::MINE`, `always` guards deleted | ~150 sites | audit clean on corpus, then `legacy_*` removed |
| G5 | optional: lower the atom grammar to a match-ladder fast path / dep-free browser build (option C); re-run `bot_cost` | ~200 | BOT.md §5 numbers updated |

Ordering note: G0–G2 must rebase over purchasing's host.rs/wasm_rules.rs edits;
G3 is the merge gate for G4.
## 8. G1 status (2026-10-07) — `crates/rules-cond`

Isolated module: workspace member `crates/rules-cond`, **no** dependency on
`game-core` / `card-sdk`, not referenced by host/ABI/`rules/card-sdk` (those
are being edited concurrently). The host fills the two pure-data snapshots in
G2.

### 8.1 The `cel` fork — `third_party/cel-rust`

`cel` 0.15.0 is vendored and patched in via the root `[patch.crates-io]`; see
**`third_party/cel-rust/FORK.md`** for the base commit, the diff summary and
the test commands. Two things matter here:

* **`parser` feature (default ON)** — antlr4rust and everything parse-time
  (`Program::compile`, `Env::compile`, the macro expander) behind one
  feature. `--no-default-features` is a pure interpreter: no antlr4rust in
  the binary. `nom` is only the `chrono` duration-literal parser, not part of
  the expression parser.
* **`Program::from_ast` / `wire::Compiled`** — evaluate a deserialized AST
  with the same interpreter and function registry as a compiled one.

### 8.2 The serialized compiled form

**Serde-derived native AST, postcard on the wire** (the same format as the
ruleset manifest). Not protobuf: the user ruled that compatibility with other
CEL implementations is not a goal (2026-10-07), and postcard of the native
AST is smaller and already in the dependency set.

| | |
|---|---|
| shape | `cel::wire::Compiled { version: u8, expression: IdedExpr, source: Option<String> }` |
| format version | first field = first byte of the blob. `Cond::from_bytes` rejects any other value with `CondError::WireVersion` — postcard is not self-describing, so a fork bump that changes the AST shape must bump `wire::FORMAT_VERSION` and every older blob fails loudly. No migration |
| on the wire | the expression tree only. Source offsets, macro bookkeeping and types are not carried (parse errors happen on the host; names resolve against the `Context` at runtime). Source *text* is optional — `to_bytes(false)` is the lean runtime-only form |
| host | `compile(src)` parses + lints + rewrites once, `Cond::to_bytes` caches the blob per guarded entry |
| browser | `Cond::from_bytes(blob)` + `eval` — no parser, no `compile` feature |

`rules-cond` features: default `compile + wire` (host); the browser build is
`--no-default-features --features runtime-only` (= `wire`).

**Shipping the compiled form.** `tools/build-ruleset.mjs` compiles every
shipped `pre` on the host (fail-closed) and publishes one postcard envelope,
`PrecompiledConds { version: u8, entries: Vec<{card, entry, blob}> }` (entries
sorted by `(card, entry)`), as the content-addressed `conds-<sha256>.bin`
listed in `dist/cards/index.json` (`"conds": {file, sha256, bytes, entries}`);
the set identity mixes each entry's blob hash into `Ruleset::sha256`, so
record stamps and bundle ids change with the conditions, not only with the
module bytes. The page (`webui/src/core/rulesetLoad.ts`, shared with the node
gate) fetches it with the modules and feeds it through
`RulesetBuilder::precompiled` before `ruleset_build`; `index.json` with no
`conds` entry means no entry has a condition, and older bundles keep working.
A `pre` with no blob on that path is a loud `BadPre` — never "treat as true" —
and a blob for an entry that declares no condition (or a blob that disagrees
with the host compile, checked whenever a native build is handed one) is
refused the same way. Server / `rules-worker` / `rules-native` keep compiling
the sources themselves; `cargo test -p game-rules --test pre_conditions` pins
the shipped blob to that compile.

### 8.3 Binary size impact (browser glue)

Throwaway cdylib examples, `wasm32-unknown-unknown`, vs `size_baseline`
(same export, no `cel`): `size_spike` (host path: parse + evaluate) and
`size_wire` (runtime-only: `Cond::from_bytes` + evaluate, built with
`--no-default-features --features runtime-only`).

| build | baseline | full (`compile`) | runtime-only (`wire`) | Δ full | Δ runtime-only |
|---|---|---|---|---|---|
| `opt-level=3`, `lto=thin` (workspace release) | 585 B | 1 955 900 B | 824 992 B | ≈ 1.96 MB | **≈ 0.82 MB** |
| `opt-level=s`, `lto=fat`, `strip=symbols` | 165 B | 1 042 661 B | 383 904 B | ≈ 1.04 MB | **≈ 0.38 MB** |

The runtime-only build is **60 % smaller** than shipping the parser: the
remaining weight is the interpreter, the serde/postcard decoder and the
minimal stdlib (below). Current `web_glue.wasm` is ≈ 6.36 MB, so shipping the
runtime-only evaluator is **+6 %** (vs +16–30 % for the full one). The host
side (wasmtime / server) is unaffected either way.

`Env::with_minimal_stdlib` (fork addition) registers only the scalar
conversions and string predicates the §4.2 schema needs — operators are
interpreted natively and need no registration — and is worth another 33–45 KB
(417 KB → 384 KB at `opt-level=s`). `size` of a list/map needs the full
`Env::stdlib` (`rules-cond` does not use it).

### 8.4 Eval and load cost (release, `bench_cond`)

Eval, `WindowScope` reused across candidates (n = 200 000/cell):

| condition | ns/eval (scope reused) |
|---|---|
| `actor == owner` | 548 |
| `actor != owner && owner.money >= 500` | 959 |
| `effect.hits(owner) && value >= 5000` | 951 |
| `tile.owner == owner && tile.houses > 0` | 822 |
| `move.kind == Walk && move.roll != null` | 673 |
| `effect.has(Pay)` | 706 |
| `slot('asUsualTurn') != turn_key` | 1 692 |
| `money(owner) >= 500` | 1 437 |
| **mean** | **974 ns** (0.97 µs) |

Scope rebuilt per call: 25.5 µs mean (26×). Well under the ~45 µs wasmi
instantiate (§1) — 46× at the mean, and a *rejecting* condition skips the
instantiate entirely.

Load path, per guarded entry (one-shot, load time):

| | |
|---|---|
| `compile(src)` (host, parse + lint + rewrite) | 33.7 µs |
| `Cond::from_bytes` (runtime-only, postcard + AST) | **1.7 µs** |
| `WindowScope::new` | 22.5 µs (once per trigger window, ~250/game) |
| postcard decode alone (67 B, release) | 0.76 µs |

The runtime-only path never parses: the browser decodes a 22–67 B blob and
evaluates. Eval of a loaded cond is the same code (1.07 µs end-to-end
`from_bytes` + `WindowScope::eval`).

### 8.5 Wire sizes (postcard, the §8.4 sample conditions)

| condition | source | lean (`to_bytes(false)`) | with source |
|---|---|---|---|
| `actor == owner` | 14 B | 27 B | 42 B |
| `actor != owner && owner.money >= 500` | 36 B | 64 B | 101 B |
| `effect.hits(owner) && value >= 5000` | 35 B | 56 B | 92 B |
| `tile.owner == owner && tile.houses > 0` | 38 B | 67 B | 106 B |
| `move.kind == Walk && move.roll != null` | 38 B | 63 B | 102 B |
| `effect.has(Pay)` | 15 B | 22 B | 38 B |
| `slot('asUsualTurn') != turn_key` | 31 B | 46 B | 78 B |
| `money(owner) >= 500` | 19 B | 34 B | 54 B |

The blob is 1.5–2× the source text (tree ids + node tags). Small enough that
the manifest stays postcard and the browser never sees a `String` of CEL.

### 8.6 What landed

* `WindowCtx` / `CandidateCtx` — pure data, no game-core types (structs, not a
  host trait: no vtable in the hot path, and "built once per window" is then a
  literal value the host fills from `Trigger` + effect-link list + `World`).
* `compile(src) -> Result<Cond, CondError>`: parse, unknown-var / unknown-fn
  check against the §4.2 schema, int-only lint (float / uint literals and
  `double()` rejected). `kind` (alias `trigger.kind`) is an ordinary window
  variable — no language limit (user, 2026-10-07): a multi-kind entry may need
  per-kind clauses. Not restating the declared category is an authoring
  discipline for review / `precheck` (a warning at most), not a compile error.
  Dotted roots flatten (`owner.money` → `owner_money`); `effect.has(x)` /
  `effect.hits(x)` rewrite to `chain_has` / `chain_hits`.
* `Cond::to_bytes` / `Cond::from_bytes` — the serialized compiled form (§8.2).
  `used_vars` / `used_fns` are recomputed from the tree on load; the wire
  carries only the (rewritten) AST and the optional source text.
* `Cond::eval(&WindowCtx, &CandidateCtx) -> bool`, plus `WindowScope` for the
  §4.4 (1) hot path. `eval_checked` surfaces type errors for the G3 audit.
* `slot` / `tok` are CEL functions (as in §4.2); `tile_named`, `money(p)`,
  `character_is`, `band_is`, `blocked`, `neighbor` likewise.
* Unit tests (`tests/cond_tests.rs`, 29 + 4 wire + doctest): parse errors,
  unknown vars, float / kind rejection, truth tables for the sample
  conditions, function lookups, scope-reuse equivalence, **postcard
  round-trip** (compile → bytes → `from_bytes` → eval equals direct eval,
  plus a loud `CondError::WireVersion` on a format bump). Benchmark:
  `examples/bench_cond.rs` (eval + load paths). Size spikes:
  `examples/{size_baseline,size_spike,size_wire}.rs`.
* **Not in G1 (G3):** the `precheck` lint helper that flags kind-based
  rejection inside *guards*. Conditions already reject kind at compile; the
  guard-side scan is a G3 item (§5.2). Note left in `rules-cond/src/lib.rs`.

## 9. G0 + G2 status (2026-10-07)

### 9.1 What landed

**G0 — surface.** `On::Counteract` / `On::Hook` / `On::Play` (the guarded
entries) carry a `pre: &'static str` condition immediately before the residual
guard (`On::Counteract(kinds, pre, guard, body)`, `On::Hook(kinds, pre, guard,
body)`, `On::Play(pre, gate, body)`); `""` = none. `On::pre(..)` is
the const builder and `card_sdk::pre::MINE` (`actor == owner`) the sugar.
`ManifestOn` gains `pre: Option<String>` (the CEL source) and `ABI_VERSION`
goes **44 → 45**. `rt::manifest` serialises it (postcard; never
`skip_serializing_if` — postcard is not self-describing). ~640 declaration
sites under `rules/` updated mechanically to the new arity.

**Ruleset build.** `RulesetBuilder::build` compiles every condition once
(`rules_cond::compile`, fail-closed → `RuleError::BadPre`) and stores the lean
compiled form (`Cond::to_bytes(false)`) in `Inner.pre`, parallel to
`cards[i].on[j]`. The same strings are read and compiled by
`crates/rules-native` from its linked `RULESET` table
(`On::condition()` → `CompiledPre::compile`). The browser glue builds
`game-rules` against `rules-cond` with `runtime-only` (no parser) and loads
the blob via `RulesetBuilder::precompiled` / `CompiledPre::from_bytes`; a
source-only `pre` on that path is a build error (fail-closed), so the compiled
form is what crosses.

**G2 — one `admits()`.** `game_rules::cond_pre` owns the single gate
`admits(entry, window, candidate) = cond(window, candidate) && guard(...)`,
plus `admits_gate` (the `On::Play` `Option<Msg>` shape) and `admits_pre`
(condition-only, for G4's deleted guards). `WindowCtx` is built once per
trigger / chain window (`cond_pre::fill_window`, `WindowScope`) and reused
across candidates; `CandidateCtx` is filled per (card, seat) from `World`.
Counters in `cond_pre::guard_cost` (`COND_EVALS` / `SKIPPED_BY_CONDITION` /
`GUARD_ASKED` / `COND_EVAL_NS`) are ready for G3/G4 measurement, same style as
`bot_cost`. The G3 `legacy_*` audit hook point is `cond_pre::legacy_audit`
(`guard-audit` feature; the call is already inside `admits`).

### 9.2 Guard call sites now routed through `admits()`

| site | file | shape |
|---|---|---|
| `Ruleset::can_counteract` / `can_counteract_scoped` | `crates/game-rules/src/host.rs` | `admits` (bool guard) |
| `Ruleset::run_hook` guard step | `crates/game-rules/src/host.rs` | `admits` (bool guard) |
| `Ruleset::cant_play` | `crates/game-rules/src/host.rs` | `admits_gate` (→ `err.play_pre`) |
| `card_replayable` nested `OP_GUARD` | `crates/game-rules/src/hostfns.rs` | `admits_gate` |
| `RulesBridge::run_hook` guard step | `crates/rules-native/src/lib.rs` | `admits` |
| `RulesBridge::can_counteract` | `crates/rules-native/src/lib.rs` | `admits` |
| `RulesBridge::cant_play` | `crates/rules-native/src/lib.rs` | `admits_gate` |

Every other path already goes through one of these:
`declare_one` (the 48 900-call counteract probe loop, `wasm_rules.rs`) calls
`can_counteract_scoped` with one `WindowScope` per offer window; the view's
`playable` flags (`engine/mod.rs` `view_extra`), `why_not_act`,
`ai_card_choice` / `ai_skill_choice` (`engine/ai.rs`) and `Cx::cant_play`
(`engine/play.rs`) all call `cant_play`. No caller invokes a guard directly.

Not routed (layer 0 / not a guard, by design):
`can_counteract_now` (§4.4 item 4), `declares()` / `counteracts_to(kind)`,
and `game_core::deck::cant_play` (deck legality, not the live-match gate).

The only remaining raw `OP_GUARD` calls are the ones *inside* those `admits`
closures (`host.rs`, `hostfns.rs`, `rules-native/src/lib.rs`) — that is the
guard half of `admits = cond && guard`, not a bypass.

### 9.3 Sizes

| | |
|---|---|
| `webui/src/wasm/glue_bg.wasm` before | 5 947 869 B |
| after (rules-cond `runtime-only` linked via game-rules) | **6 524 119 B** |
| Δ | **+576 250 B ≈ 0.55 MB** |
| §8.3 expectation (runtime-only evaluator alone) | ≈ +0.38 MB |

The delta is larger than the evaluator-only spike because it also carries the
`cond_pre` host plumbing (`fill_window` / `fill_candidate` / `admits`) and
concurrent `game-rules` edits landing in the same build; the evaluator share
matches §8.3's runtime-only figure. Host side (wasmtime / server) is
unaffected either way.

### 9.4 Files

* `rules/card-sdk/src/{lib.rs,rt.rs,abi.rs}` — `On` `pre` before the guard,
  `pre::MINE`, `On::pre()` builder, `ManifestOn.pre`, `ABI_VERSION = 45`.
* `crates/game-rules/src/cond_pre.rs` (new) — `CompiledPre`, `PrecompiledConds`
  (the shipped `conds-*.bin` envelope, §8.2), `admits` /
  `admits_gate` / `admits_pre`, `fill_window` / `fill_window_ambient` /
  `fill_candidate`, `guard_cost`, `legacy_audit` hook point.
* `crates/game-rules/src/{host.rs,hostfns.rs,wasm_rules.rs,lib.rs}` —
  `RuleError::BadPre`, `Inner.pre`, `RulesetBuilder::build` compile +
  `precompiled()`, `RulesHandle::pre`, the three guard methods through
  `admits`, `declare_one`'s shared `WindowScope`.
* `crates/rules-native/src/lib.rs` — same conditions from the linked table,
  the three guard methods through `admits`.
* Shipping the compiled form (G4 fix): `crates/game-rules/examples/ruleset_index.rs`
  (emits `conds-<sha>.bin` + the `conds` index entry),
  `crates/web-glue/src/lib.rs` (`ruleset_precompiled`, `ruleset_pre_eval`),
  `webui/src/core/rulesetLoad.ts` (shared load sequence),
  `webui/src/game/ruleset.test.ts` (the node gate),
  `tools/build-ruleset.mjs` / `archive-engine.mjs` / `rebuild-engine.mjs` /
  `webui/public/assets/engine/replay-worker.js` (carry the blob).
* `rules/fixtures/test-cards` — `TEST:preReject` / `preAccept` / `prePlay` /
  `preHook`.
* `crates/game-rules/tests/ruleset.rs` — `pre_*` fixtures tests (reject skips
  the guard, accept runs it, compile errors fail closed, runtime-only path
  matches the host).
* `webui/src/i18n/locales/{zh-CN,en}/game.json` — `err.play_pre`.

## 10. G3 + G4 status (2026-10-07)

### 10.1 G3 — migration audit + `precheck`

* **`cond_pre::legacy_audit`** (`guard-audit` feature) panics with card id +
  trigger dump on any mismatch between the entry's `legacy_*` copy and
  `kind ∧ pre(ctx) ∧ guard(ctx)` (equivalence, both directions). The host calls
  `export::OP_LEGACY_GUARD` (= 2) for every entry whose `ManifestOn::has_legacy`
  is set; `rt.rs` returns `-1` for "no legacy copy" and the check is skipped.
* Each migrated card keeps its pre-migration guard as `fn legacy_*` registered
  via `CardDef::legacy(&[(entry, fn)])`. Once a card's audit is clean the copy
  is deleted (next increment).
* **`precheck`** (`cargo run -p game-rules --example precheck`) loads the
  ruleset and reports the per-entry condition/guard split; flags guards that
  still reject on the trigger kind (the category owns that). Skips `legacy_*`
  bodies (they intentionally keep the pre-migration checks).
* `fill_window` now filters negative `move_roll` to `null` (matching the
  guest's `trigger::move_roll()`, which treats the pre-cast `value = -1`
  sentinel as "no face yet") — this was a G3 audit catch.

### 10.2 G4 — authoring pass (first batch)

Coverage after the first batch (precheck, 600 guarded entries):

| | count | share |
|---|---|---|
| entries with a condition (`pre`) | **128** | 21 % |
| residual guard kept | 258 | 43 % |
| guard deleted (`None`, condition-only or always) | **342** | 57 % |
| `legacy_*` audit copies | 128 | — |
| of which `Counteract` | 51 entries (17 with conditions) | |
| of which `Hook` | 349 entries | |
| of which `Play` | 200 entries | |

What moved:

* **`mine` → `pre::MINE`** (111 hook entries): the 118 `mine` guards
  (`trigger::player_id() == player_id`) became `card_sdk::pre::MINE` on the
  entry; where the guard was *only* `mine` it is deleted (`None`). Two files
  that also call `mine()` in-body keep a forwarding alias.
* **`always` deleted** (111 `Some(|_| true)`): the no-op prefilter is gone —
  the category + condition decide.
* **Counteract clause moves** (17 entries, `card-ag` + one per other band):
  kind re-checks stripped (the category list owns them); actor/target/by
  relations, value thresholds, `effect.has(Pay)`/`effect.has(Abnormal)`/
  `effect.hits(owner)`, `move.roll`/`move.kind` fields, the `slot != turn_key`
  once-per-turn flag all moved into `pre`. Residuals kept only for derived
  lists / geometry (`players_on`, `is_shop`, `abnormal_kind`, `card_is` —
  GUARDS.md §6).
* **Play gates** unchanged this batch (the `Some(cant_play)` shape already
  matched `On::Play`'s `Option`); their kind-free bodies stay in the gate.

### 10.3 Cached counteraction index (BOT-RESEARCH.md #1)

`declare_one` (`wasm_rules.rs`) now, per offer window:

1. builds one `WindowScope` (as before);
2. tests the per-card **kind bitmask** (`Ruleset::counteracts_to(card, kind)`,
   one shift of `Inner.counteract_mask`) — no entry-table scan, no `Run`;
3. evaluates the **condition** via `counteract_pre_allows` against the shared
   `win_run` (no per-candidate `world_copy`) — rejects most probes before any
   sandbox/native run;
4. only survivors build a `Run` + `world_copy` and ask the residual wasm guard.

Same for the hook dispatch (`Ruleset::hooks_to` bitmask) and every guard call
stays behind `admits` / `admits_pre`. `rules-native` mirrors the shape.

### 10.4 Measurements (AMD Ryzen 7 5800H, release, `bot-cost`)

`cargo run -p game-rules --release --features bot-cost --example bot_cost -- 2 4 200`
(2 games × 4 standard bots, real `dist/cards` ruleset):

| | B0/B2 baseline | after G3/G4 + index |
|---|---|---|
| `counteract` entry calls/game | 48 900 | 59 350 (2-game sample; game-length variance) |
| module instantiations/game | 55 900 | **47 836** (−14 %) |
| ms/game (wasmtime) | 14 957 | 28 183 (2-game sample, machine loaded) |
| inline rollout N=2 | 130 959 µs | **125 304 µs** (−4 %) |
| inline rollout N=1 | 49 742 µs | **48 767 µs** |
| StubRules inline N=2 | 470 µs | 511 µs |

The instantiation cut is the index + condition pre-filter working; the modest
rollout cut reflects that most counteract guards are **not yet** expressed as
conditions (only 12/51 counteract entries carry a `pre`). The 20–40 ms
depth-2 target (BOT-RESEARCH #1) needs the remaining ~39 counteract bodies
moved — the next batch. `cargo run -p game-core --release --example sim --
50 4 200` is **320.1 ms/game** (≤ 323 budget) with counts identical to the
required set (buy 2899 / forcebuy 392 / auctions 255 / rent 19567 / bankrupt 96).

### 10.5 Files (G3/G4)

* `rules/card-sdk/src/{lib.rs,abi.rs,rt.rs}` — `On::*` guard `Option<fn>`,
  `On::has_guard`/`no_guard`/`guard`, `CardDef::legacy`, `ManifestOn
  ::has_guard`/`has_legacy`, `export::OP_LEGACY_GUARD`, `ABI_VERSION = 46`.
* `crates/game-rules/src/cond_pre.rs` — `legacy_audit` (panic on mismatch),
  `move.roll` sentinel filter in `fill_window`.
* `crates/game-rules/src/host.rs` — `counteract_mask`/`hook_mask`,
  `counteracts_to`/`hooks_to`/`counteract_pre_allows`/`guard_is_none`/
  `legacy_probe`, `CardModules::{counteracts_to,counteract_pre_allows,pre}`.
* `crates/game-rules/src/wasm_rules.rs` — `declare_one` index + condition
  pre-filter before `Run`.
* `crates/game-rules/examples/precheck.rs` — the §5.2 lint.
* `crates/rules-native/src/lib.rs` — `has_guard`/`has_legacy`, `pre` on
  `CardModules`.
* `rules/**` — 111 `mine` → `pre::MINE` + `.legacy`, 111 `always` deleted,
  12 counteract clause moves with `legacy_can_counteract`.

### 10.6 Replay re-run cache (considered, deferred)

BOT-RESEARCH.md #1 also asks about caching `can_counteract` results across the
halt/replay re-runs of the same routine (the window is identical on replay --
same trigger, same world snapshot). The `WindowScope` is already reused per
offer window (the main win); a cross-rerun cache would key on a trigger
fingerprint + (seat, card) and is sound while the routine's snapshot is
unchanged. **Deferred**: the G4 condition pre-filter is the dominant rejector
and gets the same probes skipped without a cache's invalidation surface. Revisit
after G4 batch 2 with `bot_cost`'s `SKIPPED_BY_CONDITION` vs `GUARD_ASKED`
counters to see what a cache would still save.

## 11. Batch 2 + workspace hygiene (2026-10-07)

### 11.1 Sim regression — root cause (NOT the G2/G3/G4 plumbing)

`cargo run -p game-core --release --example sim -- 50 4 200` (StubRules) read
**320.1 ms/game** after G3/G4 + index vs ~270 before. The G2/G3/G4 plumbing
cannot be the cause: `game-core` does not depend on `game-rules` at all
(`cargo tree -p game-core`), so `admits()` / `cond_pre` / the legacy audit /
the counteract index are not on the StubRules path. The 320 reading was taken
under concurrent build load (RESUME.md: "Perf can't be measured reliably under
concurrent load"); an idle-machine reading of the same tree was **293 ms/game**.

Allocation-profiled (`examples/sim_prof.rs`, throwaway, deleted after): the sim
loop is **53 % `Match::state()`** (8.3 µs/call × 21 700 calls/game = 180 ms/game)
and only 22 % `tick()`. `state()` → `World::public_state()` deep-cloned the
**80-event view window** on every call (174 allocs/call, 84 of them the event
tail). The sim polls `state()` every 0.25 s tick just to read `prompt.id` and
`round`; the server calls it once per broadcast.

**Fix.** `EventTail` caches the `STATE_EVENTS` window behind a `Mutex`, rebuilt
only when the stream's `gen` changes (`push_back` / `back_mut` / `drain_front`);
`MatchState::events` is now `Arc<Vec<MatchEvent>>` so a poll between log lines is
a refcount bump. Serde output is unchanged (an `Arc<Vec<_>>` serialises as the
same JSON array). One call site (`bot-core` `determinize.rs`) moved from
`for e in &view.state.events` to `.iter()`.

Result: **sim 182 ms/game** (≤ 323 budget, target ~270), counts identical
(buy 2899 / forcebuy 392 / auctions 255 / rent 19567 / bankrupt 96). The
remaining loop cost is `tick()`'s routine bookkeeping and the sim's own
per-event `msg.to_string()` scan.

### 11.2 `LNK2019` on `bandori_*` — shim table moved to `game-rules`

`cargo test -p server -p rules-native` (any multi-crate run building both)
failed LNK2019 on 289 `bandori_*` symbols: feature unification turns
`card-sdk/guest` on for the server's copy of `card-sdk`, which compiles `ctx`'s
`#[link_name = "bandori_*"]` externs, while the `#[no_mangle]` shims lived in
`rules-native/src/symbols.rs` — a crate the server never links.

The shim table now lives in **`game-rules/src/native_shims.rs`** (the crate
every `card-sdk/guest` consumer links) as 296 `#[no_mangle]` wrappers over an
object-safe `HostOps` forwarding trait, blanket-implemented for every
`HostCtx`. `rules-native` installs its concrete `NativeHost` with
`native_shims::install_host` for the duration of a card call and recovers it
with `take_host` (fat-to-thin `Box` cast; this crate is the only installer).
Behaviour is unchanged: the bodies still call `crate::hostfns::*`, the same
functions the sandbox linker registers. `rules-native/tests/abi_link_names.rs`
now checks the new path. A single `cargo test` over all crates links.

### 11.3 `pre_*` fixture tests isolated

The `TEST:pre*` guard-condition tests assert on the **process-global**
`guard_cost` counters, which every other test in `ruleset.rs` bumps through
`can_counteract` / `run_hook` / `cant_play`. Under parallel test threads the
absolute assertions raced (and a panicking test poisoned the shared `Mutex`).
They now live in their own test binary (`tests/pre_conditions.rs`) with a
poison-tolerant lock; `tests/testworld.rs` is the shared `TestWorld` both
binaries include via `#[path]`.

### 11.4 `slot()` read the wrong key (audit catch)

`cond_pre::fill_candidate_extras` looked the CEL `slot('name')` up under a
`slot:<name>`-prefixed player-state key, while the guest's `ctx::slot(player_id,
name)` reads the **bare** key through `CardWorld::slot`. Every `slot('…')`
condition therefore read `0`: `slot('lastWalk') > 0` silently closed
`MyGO:普通与理所当然`'s counteraction window
(`rb_guards::ordinary_matches_only_abnormal`), and `slot('asUsualTurn') !=
turn_key` degenerated to `0 != turn_key`. The function also only scanned a
hardcoded `["asUsualTurn"]`. Fixed: the bare key (matching the guest),
`SLOT_NAMES` covers `asUsualTurn` + `lastWalk`, and `fill_candidate` always
fills them (the old `fill_candidate_extras` call site had been forgotten —
`slots` was always empty). Extend `SLOT_NAMES` when a new `slot('…')` shows up
in a `pre`.

### 11.5 G4 batch 2 coverage (2026-10-08)

| | batch 1 | batch 2 |
|---|---|---|
| entries with a condition (`pre`) | 128 | **139** |
| residual guard kept | 258 | 250 |
| guard deleted (`None`) | 342 | **350** |
| kind re-checks (precheck) | 34 | **0** |
| `legacy_*` audit copies | 128 | 137 |

Counteract entries migrated in batch 2 (expressible clauses into `pre`,
`legacy_*` kept for the audit): `elegant_shout`, `taki_even_if`, `marina_work`,
`layer_keep` (`roll_source == 1`), `two_in_one` (`character_is`), `hold_hands_again`
(`actor == neighbor(owner, -1)`), `even_lost` / `mana_champion`
(`by != owner && by >= 0 && effect.hits(owner)`), `here_the_world` (prefix
`actor != owner`; `player_out` stays as the residual). Plus the
`actor == owner && card.placed` move onto `here_the_world`'s two hook entries.

All 34 kind re-checks the precheck flagged are stripped: pure
`if kind != X { return }` blocks deleted (the `On::*` kind list owns them),
compound forms (`kind != X || other`) keep only `other`. Hook **bodies** with
`return Ok(())` early-outs are the same category-owned rejection and were
stripped the same way. `cargo run -p game-rules --example precheck` now prints
`(none)` and exits 0.

Still open for a later batch: the `cant_play` money/fire/crystal thresholds and
the `skill_blocked` / once-flag play gates (their *expression* can move into
`pre`; marker press checks stay as gates per the user's ruling), and the
geometry / derived-list residuals GUARDS.md §6 lists (`meet_again` `next_dist`,
`repaint` `on_path`, `secret_rainbow` grades, …).

### 11.6 Fuzz soak finding (2026-10-08) -- FIXED

`FUZZ_ITERS=2000 cargo test -p game-rules --test fuzz_interactions` (the
default suite runs 900 and is green) found **one** finding at
`seed=10174556463119430459` (iter 1517): `drain_prompts: prompts never
stopped` — a `choice` prompt storm around `cards:card-ppp.returns_title` /
`returns_which_band` (PPP:Returns' [持续]（2） turn-start band borrow). The
prompt loop caps at 400 iterations in the fuzz driver.

**Root cause (not Returns).** The fuzz `arrange` wrote `[晕眩]` through the
raw `Table::set_state` → `MatchPlayer::state_set("stun", 1)`, which named no
`StateVar::expires`; `tick_state` only wears a counter down when the item
says when it expires, so both seats stayed stunned forever. Every later turn
was skipped (`log.stunned_skip` → `end_turn`), and Returns' 「[拥有者]每回合
开始时选择一个其他存活玩家的团卡」 correctly raised one prompt per turn
start *before* the stun skip -- one prompt per skipped turn, forever. The
prompt history shows the tell: `turn` flipping 0↔1 every answer with
`stun=1(None)` on both seats.

**Fix (engine, `crates/game-core/src/state.rs`).** 规则书 [停留]/[晕眩]
「玩家的每回合结束时移除一层」 (and `stunStart`'s turn-start tick) is a
property of the *status*, not of whichever writer landed the layer:
`MatchPlayer::state_set` now stamps `default_expiry(key)` on any stay / stun
/ stunStart write whose item has no tick. An explicit non-default
`state_set_expires` still wins. The card rule needed no change -- the ask is
correct per 「每回合开始时」.

**Regression:** `rb_ppp::returns_turn_start_ask_survives_stun_skip`,
`rb_fuzz_found::returns_prompt_storm` (the seed), and
`state::tests::status_writes_carry_the_books_tick` /
`explicit_expiry_is_not_overridden`. Re-checked green at `FUZZ_ITERS=2000`
(only the known `money_ledger_gap` trips). **No `KNOWN_FINDINGS` entry**:
`drain_prompts: prompts never stopped` is an over-broad signature and the
root cause is fixed -- a future prompt storm is a new finding and must fail
the soak. See `docs/rulebook/TEST-FINDINGS.md`.
