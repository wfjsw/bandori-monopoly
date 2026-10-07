# Guard prefilter — design (2026-10-07)

Status: design-only. Lands **after purchasing** (docs/PURCHASE.md) — same host/ABI
files, and purchasing takes the next `ABI_VERSION` bump (40 → 41; prefilter is 42).
No code is changed by this doc.

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

So a guard **never re-checks the kind** (the category filter already did) and
**never re-checks a clause its condition states**. A guard whose whole body
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
| window | `kind`, `actor`, `target`, `tile`, `value`, `step`, `by`, `pay_is_rent`, `move.roll`, `move.kind`, `move.remaining`, `roll_source`, `abnormal`, `chain.count/kind[i]/hits(seat)/from[i]` | `Trigger` + effect-link list (same data `effect::*` imports read, ctx.rs:2218-2264) |
| window | `turn_player`, `turn_key` | `Cx` / `TurnCtx` |
| candidate | `owner` (= `player_id` arg), `owner.money/fire/crystals/hand/pos/out/stay/stun/exile/no_hand`, `owner.character`, `owner.band`, `owner.tiles` (count) | `World`/`Player` |
| candidate | `card.id`, `card.placed`, `card.cp`, `slot(name)`, `tok(kind)`, `tile_named(name) -> id` (resolved at load) | `World` field instances + marks |

Not in the schema (residual, stays in the wasm guard): geometry, list builders,
`gains_this_turn` / `targeted_count` / `price_tag` / `grade_of`, `skill_blocked`
semantics beyond a `blocked(band)` int mirror.

### 4.3 ABI / authoring surface

* Manifest: `ManifestOn { kind, triggers, pre: Option<String> }` (postcard,
  abi.rs:1446). No `exact` flag: a fully expressed guard is simply removed, so
  "condition only" is `pre: Some(..)` + no guard.
* `On::*` gains a trailing `pre: &'static str` (`""` = none), or a const
  `.pre(..)` builder; ~490 declaration sites. Sugar consts `pre::MINE`
  (`actor == owner`) replaces the 118 `mine` guards outright; the 19 `always`
  guards are deleted. The condition sits beside the (now residual) guard fn, and
  each clause lives in exactly one of the two.
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
| G1 | `cel` dep (features off) + wasm32 spike re-verify; schema/Context from Cx/World/Trigger; compile at `RulesetBuilder::build` | ~400 | load-time errors; int-only lint |
| G2 | single `admits()` entry; evaluate at §4.4 (1)–(5); counters for skipped probes | ~250 | no direct guard calls left; no behaviour change with `pre` absent |
| G3 | migration audit (`legacy_*` equivalence) + fuzz + replay corpus + `precheck` | ~300 | rb suite green under audit |
| G4 | authoring pass: **move** clauses from guards into conditions and delete the kind checks (category filter owns them); 50 counteract + 62 `cant_play` + 35 skill gates; `mine` → `pre::MINE`, `always` guards deleted | ~150 sites | audit clean on corpus, then `legacy_*` removed |
| G5 | optional: lower the atom grammar to a match-ladder fast path / dep-free browser build (option C); re-run `bot_cost` | ~200 | BOT.md §5 numbers updated |

Ordering note: G0–G2 must rebase over purchasing's host.rs/wasm_rules.rs edits;
G3 is the merge gate for G4.