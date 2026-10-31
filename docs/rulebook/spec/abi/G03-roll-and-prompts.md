# G03 — Roll pipeline stages and prompt option actions

Status: **draft for review, design only** (2026-10-31 revision of the
2026-10-28 draft). No source is edited by this document.
Worktree `/home/monopoly/web/.claude/worktrees/g03-design-review`, branch
`worktree-g03-design-review`. Fits **on top of ABI v108** (G04's landing:
plan frames, the window fold and `void_walk`); it does not redo G04.
Group: **G03**. Family: the roll transaction, its stage ladder, roll
attribution/history, the reroll/pick surface and roll-bound prompts.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
re-read there and is identified by the card id beside it. The row ids are the
triage's sheet rows (they are not a local file's line numbers — `cards.json`
has 185 lines of one card each, and the review CSV's line numbers are a third
thing; the BACKLOG section B carries the same row ids as this document). Where
the draft's `current`/`expected` disagree with master, master wins and the
entry says so — that is what happened to item 15's move half.

Reading order: `docs/CARDS.md` (filter/event split, the `DiceRollAfter`
exception), `docs/PIPELINE-AUDIT.md` §1 and `docs/SETTLE-STAGES.md` §7 (where
the existing roll stages sit), `docs/GUARDS.md` §4.2b-4.2c (the window
vocabulary this group extends), and G04 §4.3 (the plan-window fold this group
raises into).

## 0. Review record

This revision came out of a full review of the 2026-10-28 draft. The review
re-read every cited site against master and re-derived the band arithmetic
from the groups landing before this one. What the draft got wrong:

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header still named the old worktree/branch and base v104, and claimed G06's v104 snapshot is relevant. | Current worktree; base v108 (G04); G06's snapshot is not in this path. |
| R2 | The header table called item 13 `please-choose-opt1-inert` (sheet row 750) and item 15 rows 671-674, 1455; the row ids were never reconciled. `CROSS-TESTS.md`'s `G03` is an unrelated refund cross-test. | Item slugs keep the BACKLOG's; the doc says in one line that its rows are sheet rows and `CROSS-TESTS` G03 is a different numbering. |
| R3 | The ladder in I2 omitted `diceRollPlan` and `diceRollBefore`, so "no path skips a stage" was false for the two stages that were already raised. | §5.2 gives the full ordered ladder; Plan is Move-only and Before is every cast. |
| R4 | The ladder ran `DiceRollAfter` for Effect rolls. Every existing After listener is Move-shaped; starry's second arm would begin attacking on effect rolls, and detour's NEXT_ROLL would be consumed by one. | **After and Result are Move-only** (as today); Effect rolls get Cast/Before/Add/Value/Settled. The cards that want per-cast behaviour gate on `roll_purpose`. |
| R5 | `payload::RollAddValue` was "the plain i32 shape", but `超燃甩头`'s additive −1d6 needs a `recorded_roll` peek. | The Add payload is the same `RollValue` shape as Value (with `purpose`, `rerolled`, `recorded_roll`, `resolve`). |
| R6 | Only `no_expectation` and `moca_half` were migrated. **`event:超燃甩头`** (`rules/events/src/heat_head.rs:28-30,66-90`) subtracts a die at `DiceRollValue` and **`event:Forbidden Moca`** (`forbidden_moca.rs:22,47-60`) halves every face there; both would fold in the wrong order and both apply outside Move rolls. | Both are in the group: heat head moves to `DiceRollAdd`, Forbidden Moca stays `DiceRollValue`; both gain `roll_purpose == 1`. |
| R7 | `moca_half` was described as a `DiceRollValue` filter, but the entry is an `On::Action(Action::DiceRollAfter)` (`moca_half.rs:53`). | It becomes a real `Filter::DiceRollValue`; the Action body and `set_move_roll` go. |
| R8 | `taiki_even_if` was told to drop its manual push because "the cast is already recorded", but §5.3 committed `casts` only at commit — the row is not there yet during the Result window. Also the live `turn_rolls` is **not** merely turn-wide: nothing on the live path pushes it (`Cx::roll` and `card_do_move_roll` never do; only the uncalled `World::do_move_roll`/`roll_silent` do), so the card currently compares against an empty vector. | The cast row is written to `roll_log` **at cast time**, before any window; item 3 says both facts. |
| R9 | `cast()` (5.1) held `&mut RollCtx` borrowed from `self.rolls` across `self.raise`, which cannot compile as written, and a raise can open a child transaction. | §5.1 fixes the borrow discipline: pop the innermost transaction, work on the owned value, push it back after the raise. |
| R10 | A recast raised only `diceRollBefore`. Misaki counters `DiceRollResult`, so the fire reroll she gates on would still never open a Result window with `CHARGE`, and the outer result link's source was already stamped. | A recast of a windowed Move raises its **own** `diceRollBefore` **and** `diceRollResult`, both stamped with the transaction source. |
| R11 | `roll_source` precedence "Charge > Skill > Card > NONE" was stated, but the wire codes are NONE 0 / CHARGE 1 / CARD 2 / SKILL 3, so numeric order is not the stated order. | `RollSource::rank()` gives the stated order explicitly; only a higher-ranked source replaces the transaction's. |
| R12 | `fixed_roll` (karaoke, kept dice) was not in the design at all. A fixed face would be cast again at the main move, re-running Add/Value and raising a second Cast/Settled for one die. | A fixed face is a transaction with **no cast**: Before/Add/Value/[After/Result]/Settle only. `fixed_roll` carries its `source`. A preview roll (`ctx::roll` on a Move with no open move) is Cast+Before only, committed raw. |
| R13 | A standalone Move roll had one meaning. Doll Garden needs a fully settled victim move roll, karaoke needs raw candidates, and a roll inside an open move window must join the outer transaction. | Three explicit shapes: **join** (open Move transaction), **preview** (`ctx::roll(Move)` top level — raw), **standalone** (new `ctx::roll_move` — full ladder, own transaction even inside a window). |
| R14 | The marker of "settle again" was a boolean `tainted`, and the resettle re-folded the whole face. `layer_keeping` the old face would halve twice. | Each cast stores `settled: Option<i32>`; the settle folds Add+Value **once per cast** and caches the result. A window write to the running face clears the cache (treated as a new raw value); choosing an already-settled cast reuses its cached face. |
| R15 | `no_expectation`'s "non-turn-start" half was not implemented: the design only gated the floor on purpose. | The shave needs `trigger::step() != 1` (the turn-start stage) in addition to `roll_purpose` for the floor/no-settle half; a turn-start effect roll is not shaved. |
| R16 | Item 15 said "nothing reads either slot" and "the move always settles". `resolveOverride` **is** consumed (`play.rs:1902,1960-1966,2161`); only `stageEnd` is dead, and a blocked move settles nowhere today (`m.resolve = false`). | The move half is a rename/clarification of a live mechanism; only the turn-end half is new. §2.15 sets the real baseline. |
| R17 | The README table's G03 Op row was `—`, but the design adds nine Kani imports (eight roll ops plus the trigger payload read). | G03 takes **398–406**; G06 onwards shifts nine (§8). |
| R18 | §7 said `RECORD_VERSION` stays 3; master is **4** (`record.rs:48`). §12.4 said SAVE stays at 13 in the same document whose §7 bumps it to 14. | `RECORD_VERSION` stays 4; SAVE is 14 everywhere (§7, §2.15). |
| R19 | `RollCtx`/`RollPayload` mixed `chosen: usize` with a per-cast `chosen: bool`, and `RollId` was a newtype never used as one. | `RollTx`/`RollCast`/`RollPayload` are named once; `chosen` is an index, `unused` a face; no newtype. |
| R20 | The draft said a recast "never re-runs `DiceRollAfter`" but the Stage enum and I6 both implied a per-cast settle, so the terminal-Settled rule and the boost consumption were under-specified. | §3 I3/I6 state it exactly: Settled fires once per transaction at the chosen cast; a re-chosen cast reuses its cached face and never raises a second Settled. |
| R21 | `ask_tile_pick` clamped to 8 under Kani while `ask_pick` already uses 16 and Hagumi's option count is unbounded (2 + one per gain this turn). | Bound 16, matching `ask_pick`; the fallback and the `Ask::tile` "none" fallback are documented (§4.6). |
| R22 | The `roll_reroll` wrapper hid the pick's answer shape, and the prompt was called once; the design never said what the answer log stores. | The pick is a `PromptKind::Tile` answer (an **index**, stable because the options are rebuilt in cast order); the choose index is derived from it and logged by the stamp. |
| R23 | §5.7 designed `MoveCtx::sync_plan`, duplicating G04 §4.3's window fold that lands first. | §5.7 shrinks to "call G04's fold after every raise"; item 7 is re-dispositioned to G04. |
| R24 | The test table pointed at `rules/tests/tests/gap_money.rs` for a roll test and named files that do not exist. | One new `rules/tests/tests/g03_rolls.rs`; engine-level tests stay where the suites are. |

## 1. Items

| # | slug | sev | rows | disposition |
|---|------|-----|------|-------------|
| 1 | `starry-value-stage` | low | 1262 | B: `roll_reroll`; the rerolled face settles |
| 2 | `talent-effect-rolls-unpaid` | medium | 179, 1224 | B: reward on `DiceRollCast` |
| 3 | `taki-per-player-rolls` | low | 97, 1159 | B: cast log + `rolls_of` |
| 4 | `taki-extreme-reroll` | low | 1160 | **A** — the producer already honours `extreme` (`play.rs:1645-1667`) |
| 5 | `want-human-boost-vs-reroll` | high | 150, 1002 | B: `DiceRollAdd` + settle-time consume |
| 6 | `want-human-crystal-on-reroll` | low | 1169 | B: the check moves to `DiceRollCast` |
| 7 | `detour-plan-writes-lost` (roll half; G04's slug) | high | 973 | **Hand to G04** §4.3; G03 supplies the raise point |
| 8 | `hagumi-per-roll-options-abi` | medium | 242, 1187, 249 | B: `ask_tile_index` (the pair dedupe landed in A) |
| 9 | `misaki-fire-roll-signal` | high | 292, 954, 294, 298, 1006 | B: source on the recast's Result window; the pre becomes `roll_source == 1` |
| 10 | `moca-roll-post-additive-stage` | medium | 62 | B: additive pass before the final settlement |
| 11 | `noexpect-roll-scope` | high | 596, 598, 1367, 1479 | B: ladder on every cast; purpose + step scoping |
| 12 | `layer-keep-keep-point` | high | 892, 893, 1379 | B: `unused` on `DiceRollSettled` |
| 13 | `please-choose-opt1-inert` | medium | 750 | **A** (ABI-04); the prompt half only (§5.6) |
| 14 | `dg-move-dice` | medium | 1053 | B: `roll_move` (a real Move cast for the victim) |
| 15 | `own-stage-answer` | high | 671, 672, 673, 674, 1455 | B: the turn-end half; the move half already reads `resolveOverride` |

Group coverage against `BACKLOG.md` section B: `roll-stage-effect-rolls`
(items 2, 10, 11, 14), `roll-history-per-player` (3, 4),
`reroll-post-pick-signal` (1, 5, 6, 8, 12), `please-choose-option1` (13) and
`own-stage-answer` (15). Item 7 rides G04's `plan-live-sync`; items 5 and 6
share one card and one surface; items 1 and 12 share the recast; items 10, 11
and the two events share the ladder.

Two collateral card changes belong to no slug but are required for the ladder
to be correct: `event:超燃甩头` and `event:Forbidden Moca` (§6.10, §6.11).

## 2. Problem, per item

Each entry states the verbatim fragment, the current site, what the book
requires, and the root cause(s). The root causes are named **R1-R5** and
defined after the list.

### 2.1 `starry-value-stage` — Mor:蝴蝶飞舞的星月夜, row 1262

> 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」

`starry_night.rs:162` refuses unless the move is main, then `:199-203`
rerolls through `ctx::do_move_roll` (`= roll_direct(…, Move)`,
`ctx.rs:3426-3428`). That export sums the plan's tables and returns the raw
face (`play.rs:2480-2486`), and the enclosing `diceRollAfter` raise later just
reads the value back (`play.rs:1504-1506`). The rerolled face therefore never
passes `diceRollValue`, so 「最终结算」 reshapes (0.5倍速) are lost for exactly
the reroll the card is allowed to make.

Expected: the rerolled face is settled again through the ladder. **Root
fix:** a recast that joins the move transaction and a settle cache (R3, R5).

### 2.2 `talent-effect-rolls-unpaid` — HHW:运动的天赋, rows 179, 1224

> 「此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。」

The reward rides `Action::DiceRollAfter` and the engine raises it only in the
two move dispatches (`play.rs:932, 1504`). `HostRequest::Roll`
(`wasm_rules.rs:4078-4105`) raises only `diceRollBefore`; `roll_direct`
(`hostfns.rs:78-85`) raises nothing. Effect rolls never pay, and neither does
a non-main forced walk. The card's own guard narrows it further
(`move.main`, `sports_talent.rs:27`), and its comment (`:23-26`) says so.

Expected: every real die cast pays. **Root fix:** a per-cast `diceRollCast`
Action raised by the one cast point (R1).

### 2.3 `taki-per-player-rolls` — CRYCHIC:（立希）即便比不上..., rows 97, 1159

> 「你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。」

`ctx::turn_rolls()` reads `TurnCtx::turn_rolls` (`ctx.rs:3709`), which has no
roller — and, worse, nothing on the live path writes it: the only pushes are
`World::do_move_roll`/`roll_silent` (`ops.rs:534,575`), whose `Run` wrappers
have no caller; `Cx::roll` (`cx.rs:1422-1448`) and `card_do_move_roll`
(`play.rs:2480-2486`) never push. The card's `seen` therefore starts empty and
`seen.push(before.abs())` (`taki_even_if.rs:45-46`) is the only entry.

Expected: every cast, with its roller, and only the owner's faces count.
**Root fix:** a cast log written at cast time and a per-roller reader (R2).

### 2.4 `taki-extreme-reroll` — same card, row 1160 — DEMOTE TO A

> 「进行一次重骰」

`card_do_move_roll` → `roll_tables` honours `TurnCtx::extreme`
(`play.rs:1645-1667`). The producer is already correct. **Action:** keep one
pinned test; the recast must use the same producer (§6.4), no ABI item.

### 2.5 `want-human-boost-vs-reroll` — CRYCHIC:想要成为人类, rows 150, 1002

> 「…并使你下次的移动掷骰结果额外增加20-X」

The boost is added at `DiceRollAfter` and the slot is consumed there
(`want_human.rs:110-131`). A later `diceRollResult` reroll replaces the face
and nothing re-applies the consumed boost. Expected: the settled result still
carries +20-X. **Root fix:** express the boost as a `DiceRollAdd` filter that
the settle re-folds, and move the one-shot consume to `DiceRollSettled` (R5).

### 2.6 `want-human-crystal-on-reroll` — same card, row 1169

> 「每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶。」

The check runs at `diceRollAfter` only (`want_human.rs:93-107`); the reroll
lands later. Expected: every move cast is checked, rerolls included.
**Root fix:** `DiceRollCast` with `roll_purpose == 1` (R1).

### 2.7 `detour-plan-writes-lost` — AG:回家的路上绕个道, row 973 — HAND TO G04

> 「进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的（2）效果）」

`counteract_with_plan` writes `ctx::plan::*` from inside the `diceRollResult`
window (`detour.rs:288-292`), but the dispatch cloned the move before the raise
and reads only `t.value` back (`play.rs:1508-1532`). G04 §4.3 owns the plan
frame and the window fold. G03 owns only the raise point: the transaction is
the thing the fold hangs from. **G03 does not design the fold.**

### 2.8 `hagumi-per-roll-options-abi` — HHW:（育美）, rows 242, 1187, 249

> 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。」

*The pair dedupe landed in the A pass:* options are `(tile, roll)` pairs,
deduped on the pair and answered by index (`hagumi_marks.rs:76-101`).
What remains (B) is that the prompt is a generic `Choice` with `Msg` options:
the client cannot highlight the tile. **Root fix:** a tile-bound option ask
that still answers an index (§4.6).

### 2.9 `misaki-fire-roll-signal` — HHW:（美咲）, rows 292, 954, 294, 298, 1006

> 「[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡…」

The pre requires `move.tag('fireRoll') != 0`; the only tag writer is a test
fixture (`rules/fixtures/test-cards/src/lib.rs:589-613`). The real fire reroll
is `layer.rs:76` (`ctx::roll(…, Move, CHARGE)`); it stamps `roll_source` on a
`diceRollBefore` link only, never a `diceRollResult`, and the move's own
result raise does not stamp the source (`play.rs:1508`). So the card reads 0
in every real game.

Expected: the fire-funded recast opens the result window with `CHARGE`.
**Root fix:** transaction source plus a Result raise on the recast (R1, R4).

### 2.10 `moca-roll-post-additive-stage` — AG:（摩卡）0.5倍速, row 62

> 「此卡在场时，你的移动掷骰的最终结算/2（向上取整）」

The only final stage is `DiceRollValue`, which `moca_half` does not use — it
is an `On::Action(Action::DiceRollAfter)` (`:53`) that rewrites the face
(`:134`). `PP:不要背负期待`'s −2 is a `DiceRollValue` filter, so the two
compose by placement order (7 → 4 → 2 by the halves, or 7 → 5 → 3 depending
which sits first). `OPEN-ROLLRESHAPE-01` records it. **Root fix:** an additive
stage that always precedes the final settlement (§3 I2).

### 2.11 `noexpect-roll-scope` — PP:不要背负期待, rows 596, 598, 1367, 1479

> 「非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果最小为0，如果为0则此次移动不[结算]）」

`DiceRollValue` is raised only inside the move dispatches, so no effect roll
and no card-driven move reroll is shaved (`no_expectation.rs:32-38`). Expected:
every non-turn-start cast loses 2, the 0-floor/no-settle clause being Move-only.
**Root fix:** the ladder on every cast, purpose for the floor, `trigger::step()`
for 「回合开始时」 (R3).

### 2.12 `layer-keep-keep-point` — RAS:（和奏瑞依）寄于指尖的执念, rows 892, 893, 1379

> 「可打出此卡并保留（写下）未被选择的另一个骰点」

Three holes: the fire reroll's window resolves before 择其一; a card placed in
that window is not in the enclosing `DiceRollAfter` walk already in flight; and
no payload ever carries the unchosen face (`layer_keep.rs:107-117, 131-139`).
The `layerUnused` slot has no writer. Expected: the unchosen face reaches the
card after the pick. **Root fix:** the transaction keeps every cast, the recast
records `unused`, and a post-settlement observer delivers both faces (§3 I5).

### 2.13 `please-choose-opt1-inert` — RAS:PLEASE CHOOSE, row 750 — DEMOTE TO A

> 「（1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」

`pick == 0` logs a TODO and returns (`please_choose.rs:83-95`). ABI-04 owns the
play. G03 fixes only the shared prompt half: option (1) is offered only when a
legal hand card exists (§5.6).

### 2.14 `dg-move-dice` — Mujica:人偶的箱庭, row 1053

> 「进行一次移动掷骰并移动对应步数（不[触发结算]）」

The card rolls a generic `ctx::roll(p, 1, 20, Effect)` for each victim
(`doll_garden.rs:84`). That ignores the move-dice tables, the `DiceRollAfter`
hooks and the victim as actor. Expected: it is the victim's **move** roll, not
the main move, with the victim as `actor` and roller. **Root fix:** a
standalone Move import (`ctx::roll_move`), a synthesized non-main move view,
and the victim as the trigger's seat (R3).

### 2.15 `own-stage-answer` — R:选择自己的舞台, rows 671-674, 1455

> 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算」

The move half **already works**: the counter writes the `resolveOverride`
player slot (`own_stage.rs:87`) and `take_resolve_override` consumes it into
the move at the walk and teleport tails (`play.rs:1902,1960-1966,2161`). The
turn-end half writes `stageEnd` (`:80`) and nothing reads it; a blocked move
also settles nowhere today, because the move gate clears `resolve`
(`play.rs:1540-1548`) and `after_walk` returns early (`play.rs:2261`).

Expected: "yes" on a blocked stay/stun settles at the end of the turn; "no"
skips; leaving the card unplayed keeps the book's turn-end settle. **Root
fix:** one generic answer verb for each half and a turn-end settle runner
(§4.7, §5.8).

### 2.16 Shared root causes

1. **No roll object (R1).** A cast is a bare `i32` through a raise; there is
   nowhere to record the roller, purpose, source, casts or the unchosen face,
   so every observer re-derives facts from the enclosing move.
2. **No cast log (R2).** `turn_rolls` has no roller and is not even written on
   the live path.
3. **Cast and settlement are one event (R3).** `do_move_roll`/`roll_direct`
   are raw, `diceRollValue` is one reshape stage, and a reroll discards the
   settled face. There is no stage separation, no re-settlement and no
   universal ladder.
4. **Stage order is placement order (R4).** Additive and multiplicative
   reshapes share `DiceRollValue` and fold by field order.
5. **The roll has no name (R5).** A roll prompt is a tile prompt or a generic
   choice; the unchosen face is unaddressable.

## 3. Capability as an invariant

**A roll is a transaction with a purpose, a roller, a source and every cast.
Every cast runs one fixed ladder; the additive and final-settlement passes run
once, on the chosen face, after every window. A reroll is a new cast in the
same transaction that exposes the unchosen face. A roll prompt is bound to
that transaction and answers an option index.**

Invariants the implementation must hold (each gets a test or a Kani
obligation, §9):

* **I1 Purpose.** Every cast carries a `RollPurpose` and a `roll_source`. The
  purpose selects the producer and the Move-only clauses; nothing infers
  "move" from `sides == 0`.
* **I2 Ladder.** A cast runs `Cast → diceRollCast → diceRollBefore →
  diceRollAdd → diceRollValue → [diceRollAfter → diceRollResult] → Settle →
  diceRollSettled`. The bracketed pair is Move-only. `diceRollPlan` runs once
  per *move* before the cast, never for a reroll or an effect roll. No path
  skips a listed stage because of who called it.
* **I3 Settlement once, last.** Add+Value fold **once per cast**, on the
  chosen face, after the last window. The result is cached on the cast; a
  chosen cast that was settled before is not re-folded. A window write to the
  running face clears the cache and is treated as a new raw value.
  `DiceRollSettled` fires exactly once per transaction, at the terminal cast,
  after the fold.
* **I4 Attribution.** The roller is on the transaction and on every cast. A
  filter keyed to "your roll" sees only the roller's casts; per-roller history
  is a projection of the cast log.
* **I5 Choice.** A prompt over rolls carries `(tile?, face, label)` per cast
  and answers an **index**. Each distinct cast is one option. A recast exposes
  `chosen` and `unused`; `unused` is the cast among the latest pair that the
  choose did not pick.
* **I6 Accounting.** Every real die cast is logged once with its roller at
  cast time. A fixed face is not a cast and raises no `diceRollCast`, but it
  still gets a log row (marked fixed), because it is a face the player's move
  uses. Every transaction that settles raises exactly one `diceRollSettled`.
* **I7 Totality.** Every import exists on wasm, native and Kani (§5.9); no
  import decides; filters stay pure (`refuse_if_filter`).

## 4. Surface

### 4.1 Stages, kinds and names

New kinds (`crates/game-kinds/src/lib.rs`; ids 123-125 are this group's README
band, after G08's 114-121 and G02's 122). 110 (`paySplitMulBefore`) is the
current maximum. Retired ids (16-20, 22, 94, 104) stay untouched.
`diceRollValue` (102) keeps its id and wire name and is **redefined** as the
final-settlement (multiplicative) pass.

| id | wire | class | filter | hook-only | meaning |
|---:|---|---|---|---|---|
| 123 | `diceRollCast` | Action | no | yes | one real cast is on the table, raw. Payload: transaction, `value` = raw face, source, purpose. |
| 124 | `diceRollSettled` | Action | no | yes | the transaction's terminal cast finished settling. Payload: transaction, `value` = `final_face`, `rerolled`, `chosen`, `unused`, purpose. |
| 125 | `diceRollAdd` | Filter (value) | yes | yes | the additive pass, before the final settlement. `RollValue` payload. |

`diceRollCast` and `diceRollSettled` are hook-only: they are observations, not
counter windows. `diceRollBefore`/`diceRollResult` stay the only chain windows.

Naming (each word carries one meaning, and none collides with G02/G04):

* **`RollTx`** — the transaction (not `RollCtx`; `ctx` is already a guest
  module and a `Cx` is the engine).
* **`RollCast`** — one cast. `raw`, `settled`, `source`, `fixed`.
* **`RollStage`** — `Plan | Cast | Before | Add | Value | After | Result |
  Settle | Done`.
* `Match.rolls: Vec<RollTx>` — the stack; `Match.roll_seq: i32` — the next id.
  `parent`, `windowed`, `dirty`, `fixed` are the flags.
* Guest verbs: `roll_move`, `roll_recast`, `roll_choose`, `roll_reroll`,
  `roll_unused`, `roll_faces`, `roll_current`, `ask_tile_index`. A preview is
  the existing `ctx::roll(…, Move, …)` with no open move (§4.5).

### 4.2 `RollTx` (`crates/game-core/src/engine/roll.rs`)

```rust
#[derive(Clone, Serialize, Deserialize)]
pub struct RollCast {
    pub raw: i32,
    /// Add+Value applied once. Cleared by a later write to the face.
    pub settled: Option<i32>,
    pub source: i32,
    pub fixed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollStage { Plan, Cast, Before, Add, Value, After, Result, Settle, Done }

#[derive(Clone, Serialize, Deserialize)]
pub struct RollTx {
    pub id: i32,
    pub roller: i32,
    pub purpose: i32,              // abi::RollPurpose wire code
    pub count: i32,                // Effect producer only
    pub sides: i32,                // Effect producer only
    pub source: i32,               // highest-ranked source seen
    pub stage: RollStage,
    pub casts: Vec<RollCast>,
    pub chosen: usize,
    /// The face the latest choose did not pick; -1 before a choose.
    pub unused: i32,
    /// `diceRollAfter`/`diceRollResult` are part of this transaction's ladder:
    /// true for the main move and for `roll_move`, false for a preview (it
    /// returns before) and for an Effect roll.
    pub windowed: bool,
    pub parent: i32,
    /// The **current** cast is a fixed face (karaoke / a kept die): no die is
    /// thrown. Mirrors `casts[chosen].fixed` for the ladder.
    pub fixed: bool,
    /// The running face was written after its settle; the settle re-folds.
    pub dirty: bool,
}
```

`Match` gains `roll_seq: i32` (`#[serde(default)]`) and `rolls: Vec<RollTx>`
(`#[serde(default)]`). Helpers: `open`, `join`, `cast`, `fold_add`,
`fold_value`, `after`, `result`, `choose`, `set_unused`, `settle`, `commit`,
`find`, and the pure predicates `can_join`, `depth`.

`fixed_roll` changes shape: `Option<FixedFace { value: i32, source: i32 }>`
(`TurnCtx`, `#[serde(default)]`); `ctx::set_fixed_roll(v)` keeps `NONE`, and a
new `ctx::set_fixed_roll_from(v, source)` is used by the kept-die path.

### 4.3 `abi.rs` additions

```rust
/// Why a die is being cast. Wire codes are frozen i32s.
#[repr(i32)]
pub enum RollPurpose { Effect = 0, Move = 1 }        // unchanged; comment updated

/// The roll a `DiceRoll*` trigger belongs to, returned by
/// `trigger::roll()`. Read-only; one decode per access.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RollPayload {
    pub id: i32,
    pub purpose: i32,
    pub source: i32,
    pub rerolled: bool,
    /// Index into `casts`; -1 outside a roll.
    pub chosen: i32,
    /// The face the latest choose did not pick; -1 when none.
    pub unused: i32,
    pub casts: Vec<RollCastPayload>,
    /// The settled face at `DiceRollSettled`, `None` before it.
    pub final_face: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RollCastPayload { pub value: i32, pub source: i32 }
```

`payload::RollValue` (the `DiceRollAdd` and `DiceRollValue` payload) gains:

```rust
pub struct RollValue {
    pub value: i32,             // running face (read/write)
    pub purpose: i32,
    pub rerolled: bool,
    pub recorded_roll: Option<i32>, // heat head's peeked d6
    pub resolve: Option<bool>,  // Move only; false pins the settle off
}
```

`PromptKind` is unchanged. `ask_tile_index` reuses `PromptKind::Tile` with one
`PromptOption::Tile { tile, label }` per option and answers the option index;
the client's tile highlight and the existing answer-log shape are kept, and
repeated tiles are legal.

### 4.4 Imports (`ruleset_*`)

| import | args | returns | pause | why it is not a decision |
|---|---|---|---|---|
| `ruleset_roll_recast` | `(roll_id, source)` | raw face | Sentinel (raises the recast's Before/Result when windowed) | throws dice only |
| `ruleset_roll_move` | `(player_id, source)` | settled face | Sentinel (full Move ladder) | a move roll, no decision |
| `ruleset_roll_choose` | `(roll_id, index)` | — | none | writes the chosen index and `unused`, clamped |
| `ruleset_roll_current` | `()` | tx id or `-1` | none | read |
| `ruleset_roll_face` | `(roll_id, index)` | face or `-1` | none | read |
| `ruleset_roll_unused` | `(roll_id)` | face or `-1` | none | read |
| `ruleset_roll_rerolled` | `(roll_id)` | `0/1` | none | read |
| `ruleset_rolls_of` | `(player_id, buf, cap)` | `n` | none | read; postcard `Vec<i32>` |
| `ruleset_trig_roll` | `(buf, cap)` | `n` | none | read; postcard `Option<RollPayload>` |

`ruleset_roll` keeps its v103 signature. `ruleset_roll_direct` gains `source`
as a 5th arg and never raises (a windowless cast still needs its source for
item 9 and for `diceRollSettled`).

`ruleset_roll` semantics:

* **join** — an open Move transaction exists at any depth: cast raw, raise
  `diceRollBefore` on the face, and, because the stage is `After`/`Result`,
  raise `diceRollResult` too. No Add/Value/After: the outer commit settles the
  chosen face.
* **preview** — purpose `Move`, no open Move transaction: open a transaction,
  cast, raise `diceRollBefore`, commit raw. No Add/Value/Settled. Karaoke's
  candidates are previews; the chosen face is pinned and settled by the real
  move.
* **Effect** — always opens a child transaction and runs the whole Effect
  ladder (Cast/Before/Add/Value/Settle), even inside a move.

`ruleset_roll_move` always opens its own transaction, even inside a window,
and runs the whole Move ladder including Result.

### 4.5 `ctx` wrappers (sugar before the ABI)

```rust
/// The innermost open transaction, if any.
pub fn roll_current() -> Option<i32>;

/// Every face `roll` has cast, in cast order.
pub fn roll_faces(roll: i32) -> Vec<i32>;

/// The face the player did not choose (the latest recast), if any.
pub fn roll_unused(roll: i32) -> Option<i32>;

/// Recast `roll`; the enclosing transaction settles the chosen face.
pub fn roll_recast(roll: i32, source: i32) -> Result<i32, Prompt>;

/// Pick a face `roll` has drawn. Clamped; sets `unused`.
pub fn roll_choose(roll: i32, index: i32);

/// Cast a new face on the current roll, let the player pick old/new, and
/// choose it; the enclosing transaction settles the chosen face.
pub fn roll_reroll(
    player_id: i32, title: &Msg, text: &Msg,
    keep_old: &Msg, keep_new: &Msg, source: i32,
) -> Result<RollReroll, Prompt>;
pub struct RollReroll { pub chosen: i32, pub unused: Option<i32> };

/// A standalone move roll for `player_id` (Doll Garden's victim).
pub fn roll_move(player_id: i32, source: i32) -> Result<i32, Prompt>;
```

`roll_reroll` is `roll_current` → `roll_recast(roll, source)` → `ask_pick(keep_old,
keep_new)` → `roll_choose`. `roll_current() == None` opens a fresh preview.

`ctx::roll_direct` gains the `source` parameter; `ctx::do_move_roll` keeps its
one-arg signature and passes `roll_source::CARD`, so the existing call sites do
not change.

### 4.6 The option ask

```rust
pub struct TileLabel { pub tile: i32, pub label: Msg }

/// One option per tile-labelled item, answered by option index.
pub fn ask_tile_index(
    player_id: i32, title: &Msg, text: &Msg, options: &[TileLabel],
) -> Result<usize, Prompt>;
```

Pushes `opt_tile(tile, label)` per option and calls
`ask_raw(PromptKind::Tile, …)`; the answer is clamped to the option range.
`PromptKind::Tile` answers an **index** (that is why `ask_tile` has to map it
to a tile, `ctx.rs:2082-2110`); the answer log carries the index and the
options are rebuilt in the same order on replay, so it is stable.

The host builds `Ask::tile` for `PromptKind::Tile` (`wasm_rules.rs:7496-7521`);
its timeout fallback is `tiles.len()`, which the clamp turns into the **last**
option. The wrapper's docs say so. Under Kani the list is bounded at 16, the
same cap as `ask_pick` (`ctx.rs:2198`), because Hagumi's option count is
`2 + gains this turn`.

### 4.7 The turn-end settle answer

```rust
/// 「本次移动…是否触发结算」 -- the answering seat's current move.
pub fn set_move_settle_answer(yes: bool);
pub fn move_settle_answer() -> bool;

/// 「（或无法移动的回合结束时）」 -- the answering seat's blocked turn.
pub fn set_turn_end_settle_answer(yes: bool);
pub fn turn_end_settle_answer() -> i8;   // 0 none, 1 settle, 2 skip
```

The move half writes the existing `resolveOverride` slot (`1`/`0`), which
`take_resolve_override` already consumes; nothing new is checked. The turn-end
half writes `TurnCtx.blocked_settle` (`i8`, `#[serde(default)]`, 0 = none).

`blocked_settle` is engine-stamped in the blocked branch itself
(`play.rs:1548-1553`, `immobile`): that branch already zeroes the roll/steps
and clears the landing settle (`m.resolve = false`), and now also sets
`blocked_settle = 1` when the seat is live. It runs *after* the roll ladder, so
the round still casts and settles normally and `diceRollSettled` still fires —
only the tile settle is withheld. Leaving the card unplayed therefore settles
at turn end. `2` skips. It is cleared after the turn-end runner and rebuilt
with the turn.

## 5. Engine wiring

### 5.1 One cast point and the borrow discipline

`crates/game-core/src/engine/roll.rs` owns the producer:

```rust
/// `r` is the caller's transaction, already popped out of `self.rolls`.
fn cast(&mut self, r: &mut RollTx) -> Flow<i32> {
    let value = match RollPurpose::from_i32(r.purpose) {
        RollPurpose::Move => self.roll_tables(r.roller),
        RollPurpose::Effect => self.roll_n(r.roller, r.count, r.sides),
    };
    r.casts.push(RollCast { raw: value, settled: None, source: r.source, fixed: false });
    r.chosen = r.casts.len() - 1;
    r.unused = -1;
    r.stage = RollStage::Cast;
    let t = raise!(self, "diceRollCast", r.roller, @move_for(r), value = value)?;
    // `diceRollCast` may write the raw face; read the write-back
    r.casts[r.chosen].raw = t.value.max(0);
    Ok(r.casts[r.chosen].raw)
}
```

`rolls` is a `Match` field, so a method holding `&mut RollTx` *into the
vector* cannot also call `self.raise`. Every stage therefore takes the
innermost transaction **out** (`let mut r = self.rolls.pop().expect(...)`),
passes `&mut r` around, pushes it back after the raise, and re-checks
`self.rolls.len()` (a raise may have opened and popped a child). `@move_for(r)`
is a synthesized `Move` for a Move transaction and no `@m` for an Effect (see
5.4).

`Cx::roll`/`roll_tables`/`World::do_move_roll` become thin calls into these
producers. `roll_tables`' `extreme` branch and `Cx::roll`'s per-face extreme
branch move here, so a move roll and an effect roll can never diverge.
`Play::card_do_move_roll`/`card_roll` become the guest entry shims.

### 5.2 The ladder

```
run_cast(shape):
  shape = join | preview | standalone | effect
  open or join a RollTx        (join only: Move purpose + an open Move tx)
  if fixed:
      push RollCast{raw: fixed.value, fixed: true}; log the row; no Cast raise
  else:
      cast()                                      -> raw face, logged (I6)
  raise "diceRollBefore"                          (face on payload; source stamped)
  if preview:  commit raw; pop; return            (no Add/Value/Settled)
  fold "diceRollAdd"                              (filter; RollValue)
  fold "diceRollValue"                            (filter; RollValue; floor 0)
  if purpose == Move:
      raise "diceRollAfter"                       (Move only)
      if windowed: raise "diceRollResult"
  if the chosen cast is dirty:
      clear dirty; fold it again (cache overwritten)
  push r; r.stage = Settle
  raise "diceRollSettled" (final_face, rerolled, chosen, unused, purpose)
  pop r
  if the chosen cast is dirty:                 (a Settled listener wrote it)
      clear dirty; fold it again; no second Settled
  r.stage = Done; the record keeps final_face
  pop (child / top level) or return to the parent (join)
```

The fold runs only for a cast whose `settled` is `None` or that is `dirty`; a
re-chosen cast with a cached face is not folded again, and `dirty` is cleared
on the fold so at most two folds happen per cast (a Settled write cannot
loop). `dirty` is set by a write to the running face in `diceRollBefore`,
`diceRollAfter`, `diceRollResult` or a Settled listener, and by `choose` when it
picks a different cast. The commit's dirty re-fold is skipped when the fold
already ran after the last write.

`diceRollPlan` is raised by the move dispatches (`main_move` /
`card_move_seeded`), once, between opening the transaction and the cast — not
by `run_cast`, which also serves rerolls and effect rolls. `roll_move` runs it
too (the victim's move is planned the same way); a reroll never does. This makes the main move's `diceRollBefore`
post-cast: the window is named for the settlement, not the throw (today it
fires with the `-1` sentinel at `play.rs:1448`, before `diceRollPlan`).

`r` is pushed back for the `diceRollSettled` raise so `roll_face(id, …)`,
`roll_unused(id)` and `find` resolve it; `roll_current()` inside the window
returns the settling transaction (nothing joins a transaction already in
`Settle`). The write-back of a Settled listener sets `dirty` like any other
face write, and the commit folds once more; the settle therefore never loops
past the two-fold cap.

`diceRollAdd` and `diceRollValue` fold through the existing filter channel
(`counteract_orig` step (3)); both are hook-only. `raise!` write-back of the
trigger value is how a filter's return reaches the face (the v79 mechanism,
`payload.rs:306-341`, unchanged).

For a join the cast is appended to the outer transaction and the outer commit
settles whichever cast is chosen. For a preview the cast is committed raw under
its own (child) transaction, so a later card move can pin it; it is still
logged, and it still runs `diceRollBefore` on its own face.

### 5.3 The cast log

Every real cast (and every fixed face) appends
`RollRecord { roller, value, purpose, source, fixed }` to
`TurnCtx.roll_log: Vec<RollRecord>` (`#[serde(default)]`) **at cast time**,
before any window, so the current face is visible during `diceRollResult`.
`rolls_of(p)` filters by `roller`. `turn_rolls: Vec<i32>` stays and receives
the same face, so old cards and readers keep working; `turn_rolls` is now
actually populated (R2). `roll_log` is turn-scoped and cleared with the turn.

SAVE decision in §7.

### 5.4 Attribution and the trigger view

`CoreTrigger` gains `roll: Option<i32>`. `bridge_trigger`
(`wasm_rules.rs:7347-7405`) fills `Trigger.roll: Option<TriggerRoll>` from the
transaction and extends `is_roll_kind` so `mv.roll` is carried for
`Cast`/`After`/`Settled` too, not only Before/Result:

```rust
pub struct TriggerRoll {
    pub id: i32, pub purpose: i32, pub source: i32,
    pub rerolled: bool, pub chosen: i32, pub unused: i32,
    pub casts: Vec<(i32, i32)>,          // (value, source)
    pub final_face: Option<i32>,
}
```

`trigger::roll() -> Option<RollPayload>` is the typed read (one
`trig_roll` buffer import); the conveniences `roll_id`, `roll_purpose`,
`roll_rerolled`, `roll_unused` are guest-side wrappers over it. The CEL
vocabulary gains `roll_purpose`, `roll_rerolled` and `roll_unused`
(`cond_pre.rs` + `cond/src/schema.rs` + `vocab.rs`).

`move_*` readers keep working. A Move transaction synthesizes the move view
from it: `kind = Walk`, `main = false` for a standalone (Doll Garden's victim)
or a preview, `main = true` only on the turn's own main move, and
`resolve = false` throughout because the transaction's caller owns the settle.
An Effect transaction carries no move view.

`source` precedence is an explicit `RollSource::rank()`:
`Charge (3) > Skill (2) > Card (1) > NONE (0)`. A recast replaces the
transaction's source only when it is higher, and both the recast's Before and
Result raises stamp the transaction source, so item 9 sees `CHARGE`.

### 5.5 The option ask

`ask_tile_index` pushes `opt_tile(tile, label)` per option and calls
`ask_raw(PromptKind::Tile, …)`, then clamps. The host already resolves `Tile`
to an index in `ask_tile` (`ctx.rs:2082-2110`); the wrapper simply does not map
the index back to a tile. Hagumi builds one option per distinct `(tile, roll)`
pair.

### 5.6 Forced play (item 13, demoted)

Only the prompt surface is fixed here; the owner stays ABI-04: filter the hand
by `prop::DESIGNATES` and a `pre` that `cond_pre::admits` against the playing
seat, offer option (1) only when the list is non-empty, and on pick 0 call the
ABI-04 play verb. No roll-pipeline involvement.

### 5.7 Plan sync (item 7)

G04 §4.3 pushes a plan snapshot around Before…Result and folds the delta after
the last raise. This design's ladder casts first and may raise Result more than
once (each reroll); both raise kinds sit inside G04's snapshot, so nothing new
is needed. The only G03 requirement is that every roll window is raised through
`raise!` inside the live move frame. A preview roll runs outside any frame and
cannot shape the plan.

### 5.8 The turn-end settle runner

`end_turn` (`play.rs:737`) already raises `endTurnBefore`; the runner goes
immediately before `turnEndBefore` (`:742`) and only for a live seat whose
`blocked_settle == 1`, then clears it:

```
if blocked_settle == 1 && !out { settle_here(); }   // the seat's own tile
blocked_settle = 0;
```

`settle_here` is the ordinary landing settle with `resolve = true`, so its
stages, `tileResolved`, G10's outcome facts and the `[停留]` clauses all run as
for any settle; the mover is already at its tile (the blocked branch zeroed the
walk), so nothing moves. `turnEndBefore`/`turnEnd` body order is unchanged.

### 5.9 Kani model

`rules/kani/src/host.rs` gains a bounded transaction model: a roll stack of
depth 2, at most 4 casts, symbolic faces from the same producers, symbolic
answers.

```
roll_recast   => raise the source if higher; push a symbolic cast; raise
                 Before/Result when windowed; return its face
roll_move     => open a top-level Move tx, symbolic face, return settled
roll_choose   => chosen = index.clamp(0, casts.len()-1); unused = other of the pair
roll_face     => casts.get(i).copied().unwrap_or(-1)
roll_unused   => unused
roll_rerolled => casts.len() > 1
roll_current  => innermost open id or -1
rolls_of      => the model log filtered by roller, postcard-encoded
trig_roll     => postcard Option<RollPayload> from the model trigger
```

Every op is appended to `rules/kani/build.rs` (the import allowlist) and to the
per-group `proofs/*.rs` op tables; `sys_kani.rs` mirrors them. `ask_tile_index`
uses the existing symbolic-range answer pattern (`kani::any` + `assume`), with
the same 16-option cap as `ask_pick`.

## 6. Card usage — before / after

### 6.1 `starry_night.rs:199-203`

```rust
// before
let new_roll = ctx::do_move_roll(player_id)?;
trigger::set_move_roll(new_roll);
// after
let r = ctx::roll_reroll(player_id, &title, &ask, &keep_old, &keep_new,
                         roll_source::CARD)?;
// r.chosen is the settled face; no set_move_roll
```

The guard at `:162` stays. The card no longer knows the reroll is raw.

### 6.2 `sports_talent.rs:27,91-93,143-147`

Move the reward entry from `Action::DiceRollAfter("… move.main", …)` to
`Action::DiceRollCast("actor == owner && card.placed && roll_purpose == 1", …)`
and drop the in-loop `reward()` from `reroll_to_ten` (every recast raises Cast,
so the entry pays it). `reroll_to_ten` itself is unchanged: it stays on
`DiceRollAfter`, which is Move-only, and its fixed-face check on `roll_plan` is
unchanged too.

### 6.3 `taki_even_if.rs:43-47,68,82`

`ctx::turn_rolls()` → `ctx::rolls_of(player_id)`; drop the manual
`seen.push(before.abs())` (the cast log already has it, at cast time).

### 6.4 `taki_even_if.rs`

No body change except the verb: taki rerolls until satisfied, with no pick
prompt, so it uses `roll_recast` (which chooses the new cast). `roll_recast`
reuses the shared producer, so `extreme` is inherited. A caller that keeps the bare `do_move_roll` form still gets a
settled result: `roll_direct` stays windowless but its face is written back
through `set_move_roll`, which marks the cast dirty, and the enclosing commit
re-folds it. `roll_reroll` is preferred because it also runs the recast's own
`diceRollBefore`.

### 6.5 `want_human.rs:93-131`

The ≤X crystal check moves to a `DiceRollCast("actor == owner && card.placed
&& roll_purpose == 1", …)` body. The armed boost becomes

```rust
On::Filter(Filter::DiceRollAdd(
  "actor == owner && card.placed && roll_purpose == 1 && slot('want_human_boost') > 0",
  None, boost_add))
fn boost_add(p: i32, v: RollValue) -> RollValue {
    RollValue { value: v.value + (ctx::slot(p, SLOT_BOOST) - 1).max(0), ..v }
}
```

The slot is consumed and the abnormal snapshot taken at `DiceRollSettled` with
`roll_purpose == 1`.

### 6.6 `want_human.rs:118-131`

The boost consume and snapshot move out of the After body into the Settled
body; the log stays.

### 6.7 `detour.rs`

No body change. `counteract_with_plan` keeps writing `ctx::plan::*`; G04 §4.3
delivers them. The `NEXT_ROLL` latch keeps its one-shot consume at After.

### 6.8 `hagumi_marks.rs:76-101`

```rust
let mut options: Vec<TileLabel> = Vec::new();
for … { let t = (r - 1).rem_euclid(n);
        if options.iter().any(|o| o.tile == t && o.label == label(r)) { continue; }
        options.push(TileLabel { tile: t, label: Msg::new(key!("…")).i("n", r) }); }
let pick = ctx::ask_tile_index(player_id, &title, &text, &options)?;
let chosen = /* option order is the (tile, roll) order */;
```

The dedupe key stays the `(tile, roll)` pair, so two rolls on one tile remain
two options.

### 6.9 `misaki_card.rs:17-32`

The pre becomes `"actor == owner && roll_source == 1 && between(owner) > 0"`;
the `move.tag('fireRoll')` read and the TODO go. The body is unchanged; the
fire recast's Result window carries `CHARGE`.

### 6.10 `moca_half.rs:52-53,107-136`

The entry becomes `On::Filter(Filter::DiceRollValue("actor == owner &&
card.placed && roll_purpose == 1", None, roll_half))` returning a `RollValue`;
the Action body, the `set_move_roll` and the TODO go. OPEN-ROLLRESHAPE-01
resolves to "additive first, then this".

### 6.11 `no_expectation.rs:32-38,121-141`

The entry moves from `Filter::DiceRollValue` to
`Filter::DiceRollAdd("card.placed && actor == owner && trigger::step() != 1", …)`;
`roll_shave` gates the 0-floor/no-settle half on `roll_purpose == 1` and drops
the floor for effect rolls (`cut = roll - 2`, unbounded below). The TODO goes.

### 6.12 `rules/events/src/heat_head.rs:28-30,66-90`

`sub_die_filter` moves from `DiceRollValue` to `DiceRollAdd` with
`roll_purpose == 1`; it already reads `recorded_roll`, which the Add payload
now carries. Phase 2 subtraction therefore lands before any half.

### 6.13 `rules/events/src/forbidden_moca.rs:22,47-60`

`halve` stays on `DiceRollValue` and gains `roll_purpose == 1`; the empty pre
stays otherwise (all seats' move rolls).

### 6.14 `layer_keep.rs:27,107-148`

The `roll_after` entry changes from `Action::DiceRollAfter` to
`Action::DiceRollSettled` with `actor == owner && card.placed && roll_purpose
== 1 && roll_unused >= 0`; `counter_fire` records nothing itself, the body
reads `roll_unused` and banks it. `use_die` calls
`set_fixed_roll_from(die, roll_source::CHARGE)`.

### 6.15 `please_choose.rs:74-95`

Filter the hand by `prop::DESIGNATES` plus `cond_pre::admits` against the
playing seat, offer option (1) only when the list is non-empty, and call the
ABI-04 play verb on pick 0. The option-(1) label names the card.

### 6.16 `doll_garden.rs:84`

```rust
let r = ctx::roll_move(p, roll_source::CARD)?.max(0);
```

The victim is the roller/actor, the trigger has a non-main move view, and the
card still drives the walk with `plan::set_steps(r)` / `set_resolve(false)`.
The victim's roll reads the turn player's plan tables and raises
`diceRollPlan` against it, so a plan filter guarded on the roll `actor` skips
the victim (Q8); the roll runs outside a move frame, so its Plan writes cannot
fold and only the faces are used.

### 6.17 `own_stage.rs:68-90`

```rust
if matches!(abnormal_kind(), Some(Stay) | Some(Stun)) {
    ctx::set_turn_end_settle_answer(yes);
} else {
    ctx::set_move_settle_answer(yes);
}
```

The slot keys leave the card. The TODO keeps only the [不可阻挡] walk-through
question (§11 Q7).

### 6.18 `karaoke.rs:39-112`

No structural change: the card already passes `RollPurpose::Move` and
`roll_source::CARD`, which now makes each candidate a **preview** (raw, no
`diceRollAdd`/`diceRollValue`/Result/Settled). A preview still runs
`diceRollBefore` and still logs its cast, so a per-cast shave applies to the
candidates and 立希 counts them — which is right, each is a real die throw.
The pin becomes `set_fixed_roll_from(chosen, roll_source::CARD)`, and the
`TODO(规则书)` at `:101-104` narrows: dice effects landing after the card now
do apply — the fixed face gets Add and Value at the real move (§5.2) — and
only `set_base_dice`'s own clearing remains card-side.

## 7. Save / record / replay

* **SAVE_VERSION 13 → 14.** The roll stack is save-visible: a body can pause on
  a prompt with a transaction open, and the save must restore `rolls` (casts,
  chosen, unused, source, stage), `roll_seq`, `roll_log` and `fixed_roll`'s
  new source. Loading stays exact-version. Every field is `#[serde(default)]`
  for shape tolerance; `turn_rolls` still round-trips. A wrapper treats
  `roll_current() == None` as "open fresh", which is also what a resumed body
  with a dropped stack needs.
* **RECORD_VERSION 4 unchanged.** Cast faces come from the serialized RNG
  stream and prompt answers are already recorded; no new record field. Roll
  ids are per-run and never written to a card slot.
* **Replay.** A recast replays as the same RNG draw; the transaction is rebuilt
  by the re-run and only the answers (`roll_choose` index, `ask_tile_index`
  index) come from the log. Archived bundles are unaffected by the additive
  kinds, but `diceRollValue` (102) changes **stage** — a listener on it sees a
  different face and timing, and After no longer sees a settled face. That is
  the reason this group bumps the version at all.

## 8. Versioning

`ABI_VERSION 108 → 109` (sixth group in the README order; take the next free
number if the order moves). Wire break, named in the doc comment below the
constant and above the newest note:

```
/// v109: **roll transactions and settlement**. A roll is a `RollTx` carrying
///    roller / purpose / source / every cast; every cast runs
///    `diceRollCast` (new hook-only Action 123) -> `diceRollBefore` ->
///    `diceRollAdd` (new hook-only filter 125, the additive pass) ->
///    `diceRollValue` (now the final-settlement pass, id/wire unchanged) ->
///    `diceRollAfter` -> `diceRollResult` -> `diceRollSettled` (new hook-only
///    Action 124). `diceRollAfter`/`diceRollResult` stay Move-only.
///    `ruleset_roll` joins an open Move transaction or runs a preview;
///    `ruleset_roll_direct` gains `source`; new `ruleset_roll_move`.
///    New imports `roll_recast`, `roll_move`, `roll_choose`, `roll_current`,
///    `roll_face`, `roll_unused`, `roll_rerolled`, `rolls_of`, `trig_roll`;
///    guest wrappers `roll_reroll` / `ask_tile_index` /
///    `set_move_settle_answer` / `set_turn_end_settle_answer`. `Trigger.roll`
///    payload added; `roll_source` reaches Result and Settled; `fixed_roll`
///    carries its source. SAVE_VERSION 13 -> 14. RECORD_VERSION unchanged.
```

`ManifestOn` is unchanged. `kind_table_pin.rs`, the `KINDS` count/tests in
`crates/card-sdk/src/kinds.rs` (Filter 27→28, Action gains 2), `on_parse.rs`
(`BodyShape::Roll` for `DiceRollAdd`), the `FILTERS`/`HOOKS` pin lists in
`game-kinds`, `from_wire`'s Kani arms and the CEL vocabulary table are updated
with the three kinds.

**Kani `Op` band.** Base 340. G01 341-348, G08 349-368, G02 369-377 and G04
378-397 are ahead of this group. G03 takes **398-406** (nine ops: the seven
roll reads/writes plus `roll_move` and `trig_roll`); G06 shifts to **407-426**,
G05+G13 to **427-436**, G07 to **437-446**. The README table carries these
rows (§12).

Source-only, no bump: `RollValue` gains `purpose`/`rerolled`, the `RollSource`
rank, the cast-log projection, and every card edit.

## 9. Tests and Kani obligations

One black-box file `rules/tests/tests/g03_rolls.rs` unless noted. Discrepancies
that remain open are `#[ignore]` with the row id in the reason.

| # | Test | Asserts |
|---|---|---|
| 1 | `g03_starry_reroll_reaches_the_settlement` | 0.5倍速 placed, first 7, reroll 7 ⇒ settled 4; walk 4 |
| 2 | `g03_athletic_pays_on_every_cast` | an effect roll pays; a forced walk pays; the main-only guard is gone |
| 3 | `g03_taki_counts_only_the_owners_casts` | an opponent's effect roll does not block; the log has the roller |
| 4 | `g03_taki_reroll_settles_at_the_armed_extreme` | extreme max + reroll ⇒ the maximum face |
| 5 | `g03_want_human_boost_survives_a_reroll` | 7 + (20−X), reroll 3 ⇒ 3 + boost |
| 6 | `g03_want_human_banks_every_low_cast` | a low first face and a high reroll bank one crystal each |
| 7 | (G04) `g04_detour_counter_plan_writes_reach_the_walk` | reverse/start rewrite changes the path |
| 8 | `g03_hagumi_offers_one_option_per_cast` | two faces on one tile stay two options; each maps to its owner |
| 9 | `g03_misaki_only_after_a_fire_cast` | a plain roll refuses; the fire recast's Result carries `roll_source == 1` |
| 10 | `g03_half_speed_settles_after_the_shave` | 7 with −2 and the half ⇒ 3; the reverse placement order gives the same |
| 11 | `g03_no_expectation_shaves_effect_rolls` | effect rolls lose 2 with no floor; a Move 0 sets `resolve = false`; a turn-start roll is untouched |
| 12 | `g03_layer_keep_banks_the_unchosen_face` | fire reroll, pick old ⇒ `unused` is the new face, one die banked, `use_die` offers it |
| 13 | `g03_please_choose_opt1_offers_only_with_a_targeting_card` | option (1) absent with an empty hand; the pick plays via ABI-04 |
| 14 | `g03_doll_garden_victim_roll_takes_the_victim_view` | no expectation shaves the victim's roll; a victim After hook runs; `move.main` false |
| 15 | `g03_own_stage_no_suppresses_the_landing_settle` / `…_yes_on_a_blocked_stay_settles_at_turn_end` / `…_no_skips` / `…_default_settles` | exactly one settle overall, and only where the answer says (yes ⇒ at `end_turn`, no ⇒ nowhere, default ⇒ at the landing) |

Engine-level: `crates/game-rules/tests/kind_table_pin.rs` (123-125),
`crates/game-rules/tests/live_match.rs` (order Cast → Before → Add → Value →
After → Result → Settled; `roll_source` on Result; `unused` on Settled),
`rules/tests/tests/split_hook_entries.rs` (no expectation on Add),
`rules/kani/tests/on_parse.rs`, and a prompt test that `ask_tile_index` keeps
duplicate tiles and answers an index.

**Kani obligations** (proofs in `rules/kani/src/proofs/*.rs`, specs from the
quotes only; protocol-level where the ladder is engine-side):

* `talent_pays_exactly_once_per_cast_iff`
* `want_human_boost_is_added_iff_armed_and_move`
* `want_human_banks_one_crystal_iff_cast_le_x`
* `taki_counts_only_its_own_casts` (model log + `rolls_of`)
* `hagumi_each_option_maps_to_one_distinct_cast`
* `layer_keep_banks_exactly_the_unchosen_face`
* `no_expectation_shaves_iff_not_turn_start` (Move floor; effect unbounded)
* `moca_halves_iff_move_and_after_the_add_pass` — `(7, −2) → 3`
* `forbidden_moca_halves_every_move_cast` and
  `heat_head_subtracts_before_the_half`
* stub-totality: every new `Op` has an arm; a test asserts the build list and
  the enum agree.

## 10. Alternatives

1. **One `DiceRollValue` with `stage: Add | Final` on the payload.** Fewer
   kinds, but every match/CEL pre must switch and the two passes have
   different contracts (Add unbounded below, Final floors). Rejected.
2. **No automatic re-settle; an explicit `ctx::roll_settle(roll)`.** Every
   reroll card must remember; a forgetful card reproduces this group's bug.
   Rejected.
3. **Return a handle from `ctx::roll` and break the ~49 call sites.** Cleaner
   than `roll_current`, but v103 just landed the `Result<i32, Prompt>` shape.
   Rejected for this batch.
4. **A new `PromptKind::Options` answering an index.** Explicit, but needs a
   client kind, wire string and log kind. `Tile` already answers an index and
   its UI highlights the tile. Rejected.
5. **Keep `turn_rolls` and add a parallel roller vector.** Two vectors desync;
   the cast log with one row per cast is a single source. Chosen.
6. **Make the main move's `diceRollBefore` post-cast for every call path.**
   Chosen: the current main-move `-1` sentinel is the only path without a
   face, and the window's name says "before the settlement", not "before the
   die". A counter can now set the face it just reacted to.

## 11. Open questions

1. **Signed walks.** A signed (reverse) walk's `unused` is signed while
   `taki_even_if` compares unsigned. The design publishes raw; the card takes
   `abs()`. Confirm.
2. **Is a reroll 「掷骰」 for every "each roll" clause?** 运动的天赋 says
   「每次掷骰」 and every real cast raises Cast, so yes. 想要成为人类 says
   「每当你的移动掷骰」 — the design also checks every cast; if a ruling says
   only the final face counts, that card moves to Settled and the ladder is
   unchanged.
3. **Does 「结果至少为10」 (运动的天赋) read the raw die or the settled
   face?** The design compares at Cast (raw), so 0.5倍速's half does not feed
   the reroll threshold. A ruling either way changes only that card.
4. **`roll_source` precedence** when a skill recast follows a card recast.
   Rank order is Charge > Skill > Card; the book names charges only, so the
   rest is a judgement.
5. **A recast inside `diceRollAdd`/`diceRollValue`** (a pure filter cannot
   recast today; `roll_recast` is a mutating ctx call). Is a filter-side
   recast ever wanted?
6. **`rolls_of` scope.** Turn-scoped to match 「本回合内」; confirm no card
   needs match-scoped history.
7. **The own-stage branch is the abnormal kind, not "the move was actually
   blocked".** A [停留] walked through under 「不可阻挡」 takes the turn-end
   half and double-settles (once at the landing, once at turn end) if "yes".
   Keep the kind branch, or test blocked-ness?
8. **Doll Garden's plan source.** A victim's Move roll sums `turn.plan`, which
   the turn player shaped, not the victim's. Is that the victim's dice table,
   or should the plan be rebuilt from the victim?
9. **`fixed_roll` one-shot.** Today it survives the move and could shape a
   later card move. Make it one-shot at the move tail?

## 12. Reconciliation with other groups

- **G05/G13 (after).** `ask_tile_index` is the same `PromptKind::Tile` answer
  shape; a press/pick prompt may reuse it.
- **G07 (after).** The targeting window reads the roll payload for misaki's
  span; nothing new.
- **G06 (after).** G06 owns the abnormal/effect window ordering and carries
  item 15's ordering half (1.13/1.14). G03 raises the Result window the
  counteraction machinery already serves.
- **G12 (after).** `dice_immune` is about hand plays, not casts; no
  interaction. G12's `move_activation` stamps the move, not the roll.
- **G10 (after).** Settle facts are orthogonal; a settle opened at turn end
  (§5.8) raises the ordinary settle stages under G10's outcome flags.
- **G04 (before).** Owns the plan frame, the window snapshot/fold and
  `void_walk`; item 7 is theirs. G03 guarantees Result is raised inside their
  frame.
- **G02 (before).** Owns the settle stack; unchanged. The `RollValue.resolve`
  field is the plan write, not a settle fact.
- **G01/G08 (before).** Their trigger payload and zone writer carry
  `Trigger.roll`; the `is_roll_kind` list in `bridge_trigger` is the only
  shared line.
- **README.** The band table gains G03's Op row (398-406) and shifts G06,
  G05+G13 and G07; the G03 rows and the `G03 ↔ G04` coupling are added, and
  the `G03 ↔ G11` note stays.
- **`docs/CARDS.md`.** The stage list becomes Plan → Cast → Before → Add →
  Value → After → Result; the "Value is immediately before After" sentence
  and the `DiceRollAfter` exception wording are updated (After is Move-only
  and no longer sees a settled face).
- **`docs/GUARDS.md`.** §4.2 gains `roll_purpose` / `roll_rerolled` /
  `roll_unused` / `roll.chosen` / `roll.final_face`, the neutral defaults
  outside a roll, and the per-kind meaningful-vars rows for the three kinds.
