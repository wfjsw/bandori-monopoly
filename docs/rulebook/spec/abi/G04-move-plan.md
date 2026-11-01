# G04 — Move plan, walk sync and forced moves

Status: **draft for review, design only** (2026-11-01 revision of the
2026-10-28 draft). No source is edited by this document.
Worktree `/home/monopoly/web/.claude/worktrees/g04-design-review`, branch
`worktree-g04-design-review`. Fits **on top of ABI v107** (G01 instance
identity and G08 zone lifecycle landed; G02 pay pipeline landed — this
document consumes its factor-scope API — G02 §4.5/§5.5, used here in §3.3 and
§4.2). It does not redo G03's roll
pipeline: `dg-move-dice` and `misaki-fire-roll-signal` stay with G03 and this
document consumes their surface where the move plan meets a roll window.

Group: **G04**. Family: the movement plan, its ownership and sealing, forced
and main moves, the walk's shape reads, the movement trigger payload and board
geometry.

Ground truth: `docs/rulebook/cards.json` and the live skill sheet. Every
fragment below was re-read from `cards.json` on 2026-11-01 against the card id
named beside it; the `cards.json` line is given with each quote because the
file has one card per line and no structured id.

The seven consolidated slugs of `docs/rulebook/BACKLOG.md` §B, with their
severity and their review rows (sheet rows, `card-review-2026-10-27.csv`; they
are not line numbers, and several name neighbouring cards rather than the
consolidated slug's own):

| consolidated slug | sev | rows | handled at |
|---|---|---|---|
| `want-to-grab-board-axis` | medium | 107, 1003 | item 1, §5.12 |
| `plan-shape-scope` | high | 131,132,288,290,338,339,340,560,835,963,964,1038,1064,1095,1361,1483 | items 2-4, 10, 16, §5.12 |
| `charity-rolled-flag` | medium | 209,1215,1216 | item 5, §5.12 |
| `plan-live-sync` | high | 203,973,1025,1182 | items 6, 9, §5.12 |
| `effect-move-forced-path` | high | 129,200,291,293,295,431,1183,1184,1286 | items 7, 11, 12, 14, 15, §5.12 |
| `trigger-walk-payload` | high | 292,294,298,954,1006,1007,1101 | items 8, 12, 13, §5.12 |
| `nested-teleport-tail` | medium | 693 | item 17 |

The code work is split finer than the slugs, into seventeen units, numbered
below and in §5:

| # | slug | sev |
|---|---|---|
| 1 | `want_to_grab-clause3-teleport` | medium |
| 2 | `pure-wings-stale-steps` | high |
| 3 | `pure-wings-stale-direction` | medium |
| 4 | `pure-wings-pay-factor-leak` | medium |
| 5 | `charity-dice-move-gate` | medium |
| 6 | `charity-extra-steps-sync` | medium |
| 7 | `sakiko-followers-card-move` | medium |
| 8 | `detour-plan-writes-lost` (merged `detour-false-move-blocked-gate`) | high |
| 9 | `detour-parity-unused` | medium |
| 10 | `saki-move-leaks-plan-shape` | high |
| 11 | `misaki-teleport` | high |
| 12 | `misaki-planned-start` | medium |
| 13 | `misaki-effective-steps` | medium |
| 14 | `dest-forced-hop` | high |
| 15 | `plan-set-forced` | medium |
| 16 | `dg-plan-reset` | medium |
| 17 | `lisa-bond-nested-teleport-window` | medium |

The consolidated `plan-shape-scope` also contains four pay-factor slugs
(`sakiko-pay-factor-composition`, `repaint-pay-factor`, `taki-pay-factor-leak`,
`rana-pay-factor-multiply`). Their semantics are now **G02's** (§4.5,
`PayFactorScope`), and G02 has landed. This document supplies the plan half the
composition reads (`ctx::plan::move_pay_factor()`), the per-frame default, and
the plan-reset half of the leak.

One item is **demoted to A** (§1.2 item 8): the read already exists and only
the test changes. Items 2 and 4 are already right on master for the
*freshness* cases the 2026-10-30 patch covers; they stay because the frame is
their structural answer and their tests pin the contract. The rest need this
design.

Reading order: `docs/EVENTS.md` (event moves are never the main move),
`docs/rulebook/SETTLE-STAGES.md` §7 (the move tail), `docs/ACTIVATION.md` §2 (the guest
body and its replay), `docs/GUARDS.md` §4.2b-4.2c (the window vocabulary), and
G02 §4.5/§5.5 (the factor scopes this document feeds). G03 §5.7 and its §10
(item 7) name the plan-window fold as this group's.

---

## 0. Review record

This revision came out of a full review of the 2026-10-28 draft. The review
re-read every cited site against master `a19097db`, re-derived the band
arithmetic from the groups that landed before it, and checked the draft against
the landed G01/G02 designs. What the draft got wrong or that master changed
under it:

| # | Draft defect / drift | Correction |
|---|---|---|
| R1 | The header named `worktree-card-rulebook-review`, base v104 and "no commit"; it predates G01/G02 landing and the README's renumbering. | Current worktree and branch; base ABI 107; G02's factor scopes are consumed, not re-designed. |
| R2 | Line citations were against v104 and had drifted 100–400 lines; several points (e.g. `want_to_grab.rs:118-127`, `happy_lucky.rs:37-67`) named the wrong file or range. | Every citation re-read on 2026-11-01 against `a19097db`. `happy_lucky.rs` is `card-hhw`, geometry at `:28-117`. |
| R3 | The draft assumed a bare turn plan with manual exits. Master now has the 2026-10-30 interim patch: `steps_fresh` / `stop_at_fresh` / `pay_fresh` / `reverse_fresh` / `resolve_fresh`, `drop_stale_plan_shape`, `drop_plan_fresh`, `play_drove_move` and `drop_spent_plan_shape` (`move_ctx.rs:176-196`, `play.rs:2116-2216`). | The frames **replace** the marks in the same landing; a frame and a fresh mark doing the same job is a bug farm. §4.1/§4.2 say which functions are deleted. |
| R4 | The draft's `pay_factor` story predates G02. It had no `next_move_pay_factor`, no `move_open`, and treated the plan factor as a simple field. | The plan factor is the `ThisMove` slot of G02 §5.5 (`move_open` true while a frame is live); the frame seed folds `next_move_pay_factor` and resets it; the move tail's existing clear moves into `end_plan`. G04 adds the getter only. |
| R5 | `TriggerMove` was quoted at `rules.rs:52-78` without `is_main_move`; the draft put the payload chapter before checking the current payload. | `TriggerMove` carries `is_main_move` through the bridge (`wasm_rules.rs:7783-7796`). The payload chapter adds `rolled` / `effective` beside it, and `bridge_trigger` mirrors both. |
| R6 | The draft described `forced` as host-only and `is_main_move` as derived nowhere. | `forced` and `is_main_move` are already `MoveCtx` fields and `run_planned_move` computes `is_main_move = is_turn && !plan.forced` (`play.rs:904`). The missing half is the guest setter and the `main_override` split. |
| R7 | Item 1 cited `TileData` as `data.rs:83-93`, correct, but the board data was assumed to be where coordinates belong without checking the record. `display.json` is documented as carrying a `cell` but does not. | `grid_cell` goes on `TileData` / `board.json` with `skip_serializing_if` (§3.6); the record already carries `MatchSetup::board: Option<Vec<TileData>>` (`record.rs:369`). |
| R8 | The Op band was `360-373` from the pre-amendment table. The 2026-10-31 recount (G01 341–346, G08 349–368) and G02's landing (369–377) moved G04 to **378–397**. | G04 uses **378–391** and leaves 392–397 free. |
| R9 | §6 said `RECORD_VERSION` stays 3. Master is **4** (`record.rs:48`). | Stays 4. Inline `grid_cell` on the recorded board is serde-skipped when absent. |
| R10 | The host arm was to "return the inner value", but the gate is called **before** `run_planned_move` in a host copy and returns that verdict, so no frame is live for a Play-time call. | §4.3 splits it: the gate keeps its call site and returns 0 on refusal; the refusal cleanup writes through `plan_mut()` (frame when the call sits inside a move, staging otherwise) and clears `teleport_to`, `void_walk` and resets `kind`. The allow path returns the run's bool. |
| R11 | `next_steps` lived in `hidden[i]` and was taken at the roll; the draft had the replacement teleport read the raw face, so a buffed misaki endpoint was still wrong. | `next_steps` is copied into the frame from `hidden[i]` at the Main seed, before any window, and `m.roll` is clamped to `effective_steps()`; the slot is consumed there. |
| R12 | `plan_scope` was four flat fields (one `staging_before`, one `plan_before`, one `window_before`). A body raised from inside another body (a card played from a hook) overwrote the outer snapshot. | `plan_scope` carries a stack of `PlanCommand` and a move counter; windows have their own stack (§3.3). |
| R13 | The draft kept `take_plan_teleport` out of the design; master has `take_plan_teleport` / `land_plan_teleport` (`play.rs:2111,2221`) used at every pass stage. | Both migrate to `plan()`; a window-written destination is taken from the frame and cleared one-shot. |
| R14 | Item 7 said followers replay only in `main_move`; the draft's sparse note did not say that a card main move is exactly the path that misses. | Confirmed on `a19097db`: the replay exists only in `main_move` (`play.rs:1702-1735`). The design moves the block into the one shared tail (`replay_followers`) and keeps its test. |
| R15 | Item 11 (`misaki`) overlapped G03's pre-rewrite (`roll_source`) but the draft did not state the split. | G03 owns the condition and the fire-cast source; G04 owns `set_cancelled` staying link-only plus the replacement body (§5.7). |
| R16 | The draft's `void_walk` branches did not say where the abnormal gate sits relative to the replacement. | The gate runs first; on refusal the outer walk resumes with `kind = Walk`, no teleport, and `void_walk` cleared (§4.4). |
| R17 | Parity canonicalization was stated only in passing, but Himari writes `2` while detour writes `0`. | `set_parity` normalizes `2 → 0` in `ops.rs`, so both encodings and the save path agree (§3.4). |
| R18 | `teleport_as` was a single self-scoping shape; a main teleport needs a Main frame, or a refused teleport restores the wrong plan. | `teleport_as` gains the frame kind from its existing `is_main_move` argument (§4.2). |
| R19 | The draft's Kani `arm_card_move` model echoed constant 1 and did not model the slot. | `arm_card_move` models `replacement_taken`; every new op joins the model or the zero group (§4.10). |
| R20 | The draft cited `sakiko_lead` as the follower item but the card's write is now `multiply_pay_factor(500, NextMove)` (G02), so the old prose about `set_pay_factor(500)` was stale. | §5.4 says no body change and cites the landed G02 call. |
| R21 | The draft's per-slug "rows" column was a guess: nearly every number named a neighbour card (item 1 cited rows of 商店街的救世主 and 不要背负期待; item 11 cited 海铃 and 睦). | Every row re-resolved against `card-review-2026-10-27.csv`; the table is now keyed by the BACKLOG's consolidated slugs and §5.12 disposes of every row the items do not name. |
| R22 | Several new identifiers were abbreviated: `main_as`, `plan_teleport_to`, `StartWhy`/`Why`, `PlanKind::Main`/`Side`, `axis_nearest`, `TileData.cell`, `tail_only_depth`, and `cur`/`dir`/`n` parameters. | Spelled out to `main_override`, `planned_teleport`, `StartReason`/`Reason`, `MainMove`/`SideMove`, `nearest_tile_on_axis`/`farthest_tile_on_axis`, `grid_cell`, `in_teleport_tail`, `current`/`direction`/`tile_count`. The `ruleset_trig_*` / `ruleset_move_*` / `ctx::plan::*` prefixes and `pay_factor` keep their existing spelling because they match landed imports. |

The review made no change to the item list or severities.

---

## 1. Problem

### 1.1 Shared root causes

The movement plan is one `TurnCtx::plan` (`world.rs:153-160`) created with the
turn and rebuilt only at `next_turn`. Every move — the main move, a card's own
move, a forced move, a nested move inside a settle — reads and writes it. The
tree works around that with per-field "fresh" marks and a hand-maintained
restore at only some exits:

- `drop_stale_plan_shape` clears `steps`, `stop_at`, both payment factors,
  `reverse` and `settles` at a move entry unless the current activation marked
  them fresh (`play.rs:2136-2167`); `drop_plan_fresh` clears the marks at the
  tail; `drop_spent_plan_shape` drops a Play body's writes when it drove a move
  (`play.rs:2200-2216`). Five booleans, two lifecycle functions and a
  `play_drove_move` latch encode one rule: *who owns this write*.
- Everything the marks do not cover leaks: `parity`, `start`, `kind`,
  `teleport_to`, `followers`, `extra_steps`, `settle_tile`, `more_steps`.
- `run_planned_move` mem-swaps the plan only when `move_depth > 0`
  (`play.rs:941-954`), so a nested move restores but a top-level card move
  leaves its shape on the turn.
- The walk re-reads a hand-picked subset each step (`stop_at`, `settles`:
  `play.rs:2018-2023`), not `parity`, `extra_steps` or `reverse`.
- One-shot fields (`teleport_to`, `start`, `by_skill`) are cleared by hand at
  every exit (`play.rs:913-916, 1077-1080, 1684-1687`); a refusal path that
  forgets one shapes an unrelated later move.
- `MoveCtx` has `forced` (`move_ctx.rs:86-89`) but only the `event:*` host arm
  writes it (`wasm_rules.rs:4019-4021`); a guest cannot, so every card-driven
  forced move takes the main-move path.
- The roll ladder clones the move before the window and folds back only the
  face and a couple of pulled fields (`play.rs:1577-1678`), so plan writes made
  in `diceRollResult` (detour, misaki) never reach the walk.
- The walk derives its next tile from the direction alone
  (`play.rs:1956`); `Move.parity` is written by Himari and detour and read by
  nobody.
- The follower replay sits only in `main_move` (`play.rs:1702-1735`), so a card
  main move never replays it.
- A nested teleport is a bare position write with no tail
  (`play.rs:875-886`).
- Board geometry is a private table in one card (`happy_lucky.rs:28-117`),
  duplicated from the client's `FOLD` (`Ring.tsx:31-38`).

These are not seventeen independent bugs. They are six defects in how a move
plan is owned, read, sealed and handed back:

- **R1 — no command ownership.** Shape leaks between moves (items 2, 3, 4, 10,
  16).
- **R2 — no intent split between "this move" and "the next main move".** Writes
  cannot be classified, so every "clear it afterwards" is a manual patch
  (items 1, 3, 4, 10, 16).
- **R3 — forcedness and mainness are conflated.** `forced` means "never the
  main move", while the book's forced walks of the turn player are that
  player's main move when they choose themselves (items 11, 14, 15).
- **R4 — the roll window cannot rewrite the move.** Only the face and `settles`
  survive the raise (items 8, 11, 12, 13).
- **R5 — the walk ignores plan dimensions.** Parity is dead (item 9);
  `extra_steps` is not synced (item 6); the movement origin is stamped too late
  (item 12); provenance `rolled` does not exist (item 5).
- **R6 — the move tail is not shared.** The follower replay lives in one body
  only (item 7); nested teleports raise no tail (item 17).

### 1.2 Per item

Each entry quotes the fragment verbatim with its `cards.json` line, names the
consolidated slug it serves, states what the tree does today, what the book
requires, and the root fix. "Demote to A" means the capability exists; only the guard/test changes.
"Fixed on master" means the interim patch already produces the right behaviour;
the item keeps its test and gets a structural home.

**1. `want_to_grab-clause3-teleport` — CRYCHIC:想要抓住..., consolidated `want-to-grab-board-axis`**
(`rules/cards/card-crychic/src/want_to_grab.rs`, cards.json:24)

> 「（3）你的下回合开始前，若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。」

The `TurnStart` hook retires unconditionally and never reads the `PASSER`
latch (`want_to_grab.rs:115-120`). The only grid ray search in the tree is the
private `CELLS` + `farthest` in `happy_lucky.rs:28-117`, fixed to 60 tiles;
`TileData` has no coordinate (`data.rs:83-93`). Expected: at the owner's turn
start with nobody passed (`PASSER == NEVER`), offer the nearest tile on each of
the four axes, teleport-and-settle there as the main move, and retire the card.
**Root fix:** board geometry in the board data plus nearest/farthest axis
queries (§3.6), then the card branch (§5.1).

**2. `pure-wings-stale-steps` — Mor:纯真振翅, consolidated `plan-shape-scope`**
(`rules/cards/card-morfonica/src/pure_wings.rs`, cards.json:53)

> 「传送到移动方向20格后（不触发结算），立刻进行移动掷骰」

The card clears the destination and calls `run_planned_move`, expecting a
fresh roll. `card_move_seeded` rolls only when `plan.steps.is_none()`
(`play.rs:1006`), and `set_steps` clamps `-1` to `Some(0)`
(`move_ctx.rs:303-307`), so there is no way back to `None`. An earlier card's
count silently turns the roll into a fixed walk. Expected: a dice roll from the
teleport destination. **Root fix:** `plan::clear_steps` and a frame whose
default is the post-teleport state (R1 + R2). *Fixed on master* by
`drop_stale_plan_shape` for step counts written by an earlier move; the frame
is the structural answer and covers the same-move replay case.

**3. `pure-wings-stale-direction` — Mor:纯真振翅, consolidated `plan-shape-scope`**

> 「传送到移动方向20格后」

`ctx::plan::dir()` reads `world.turn.plan` (`ops.rs:284-286`), which a prior
side move shares. `saki_move.rs:53` leaves `reverse = true` behind, so the jump
goes the wrong way. Expected: the moving player's own direction for *this*
move. **Root fix:** per-command frame seeded from the plan as it stood at body
entry; no card change after R1. *Fixed on master* for an earlier move's write
(`reverse_fresh`); the frame also covers the same-move replay.

**4. `pure-wings-pay-factor-leak` — Mor:纯真振翅, consolidated `plan-shape-scope`**

> 「立刻进行移动掷骰」

The landing settle reads `turn.plan.pay_factor` through
`scale_settle_payment`; a prior 祥，移动's `set_pay_factor(500)` halves the
rent. No getter exists to observe the inherited value. Expected: the default
factor for this walk's settle. **Root fix:** frame default = `1.0` (G02 §5.5's
`ThisMove` slot), `next_move_pay_factor` folded at the seed, and the
`plan_pay_factor` getter (R1). *Fixed on master* by G02's tail reset and
`drop_stale_plan_shape`; the frame supplies the same guarantee structurally.

**5. `charity-dice-move-gate` — HHW:爱心义演, consolidated `charity-rolled-flag`**
(`rules/cards/card-hhw/src/charity_show.rs`, cards.json:38)

> 「你若进行掷骰移动」

The `PassTile` condition gates on `move.is_main_move`
(`charity_show.rs:38`). A roll-less card main walk (尽力后的收获, 迷宫标) fires
it, and a forced dice move (an event walk) does not; `TriggerMove` has no dice
provenance. Expected: gate on "a die was rolled for this walk". **Root fix:** a
`rolled` discriminator on the move payload and CEL (`move.rolled`), set from how
the walk got its length (R5).

**6. `charity-extra-steps-sync` — HHW:爱心义演, consolidated `plan-live-sync`**

> 「使你的总移动数+2」

`ctx::plan::set_extra_steps` writes only `TurnCtx::plan.extra_steps`
(`ops.rs:68-70`); the running walk is a separate `Move` and never re-reads it,
so the +2 is a no-op on the move that earned it. The card keeps a running prop
and overwrites 上原绯玛丽's value because `set_extra_steps` assigns
(`charity_show.rs:139-145`). Expected: +2 extends the walk it is earned on and
composes. **Root fix:** the walk syncs `extra_steps` from the plan each step;
`plan::add_extra_steps` composes (R5).

**7. `sakiko-followers-card-move` — CRYCHIC:（祥子）带领着大家, consolidated `effect-move-forced-path`**
(`rules/cards/card-crychic/src/sakiko_lead.rs`, cards.json:30)

> 「使你的下次主要移动结果对那些玩家一起执行」

`plan.followers` is taken and replayed only in `main_move`
(`play.rs:1702-1735`). A main move produced by a card
(`run_planned_move`, 「视为你的主要移动」) never replays them, so a card main
move leaves the recorded players standing; the TODO at `sakiko_lead.rs:49-50`
describes exactly this. Expected: the recorded followers walk whenever the next
main move runs, however it runs. **Root fix:** move the block into the one
shared tail (`replay_followers`, R6) so a third path cannot miss it; remove the
TODO. Test kept.

**8. `detour-false-move-blocked-gate` — AG:回家的路上绕个道 — DEMOTE TO A**
(`rules/cards/card-ag/src/detour.rs`, cards.json:8)

> 「(1)此卡可作为反击使用」

The counter guard is `ctx::cant_move_code(player_id) != 3`
(`detour.rs:71`). `cant_move_code` already returns 2 when the main move is
spent, so 「already moved」 is accepted and only [停留] (3) is refused. No ABI
change. **Action:** add the regression test
`g04_detour_counter_opens_after_the_main_move_latched` and remove the TODO. The
abnormal-gate opt-out (STA-02, rules.txt 51) is a separate backlog item.

**9. `detour-parity-unused` — AG:回家的路上绕个道 / Himari (2), consolidated `trigger-walk-payload`**
(`rules/cards/card-ag/src/detour.rs`, `rules/skills/skill-personas/src/himari.rs`,
cards.json:8; the Himari fragment is the initial Afterglow (2))

> 「进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的（2）效果）」
> 「指定你的此次移动仅在单/双数格上进行」

`detour.rs:219,224` writes `plan::set_parity(1)` / `set_parity(0)` and
`himari.rs:72` writes `1` / `2`; the walk derives `next` from `m.dir()` alone
(`play.rs:1956`) and never reads `m.parity`. Expected: each step goes to the
nearest tile of the requested parity. **Root fix:** a parity step function in
the walk; canonical parity `-1`/`0`/`1` with `2` normalized to `0` (R5).

**10. `saki-move-leaks-plan-shape` — Mujica:祥，移动, consolidated
`plan-shape-scope` + `effect-move-forced-path`**
(`rules/cards/card-mujica/src/saki_move.rs`, cards.json:69)

> 「强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半」

`set_steps(3)` / `set_reverse` / `set_settles` / `set_pay_factor(500)` write
the shared turn plan (`saki_move.rs:53-56`); the writes are only partly dropped
by the fresh-mark patch at the body's end. Targeting another player makes the
caster's own later main move walk 3 tiles in the card's direction, ignoring the
dice, and settle at half price. Expected: the shape is scoped to the forced
walk. **Root fix:** a side frame whose shape is restored when the command ends
(R1 + R2), plus `set_forced` so another player's walk never touches the main
slot.

**11. `misaki-teleport` — HHW:（美咲）, consolidated `effect-move-forced-path` +
`trigger-walk-payload`**
(`rules/cards/card-hhw/src/misaki_card.rs`, cards.json:43)

> 「[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动）」

`trigger::set_cancelled()` only negates the link; the roll site reads `t.value`
and walks the face. `run_planned_move` then takes the `main_moved` early return
(`play.rs:905-918`), logs `main_move_used` and clears `teleport_to`, so there is
no teleport at all. Expected: the rolled walk is voided and replaced by the
teleport, settling there as the same main move. **Root fix:** a `void_walk`
handoff consumed at the roll site plus `main_override` (R3 + R4).

**12. `misaki-planned-start` — HHW:（美咲）, consolidated `trigger-walk-payload`**

> 「位于你的移动起点与预定移动终点之间」

`between()` uses `ctx::player_pos(player_id)` (`misaki_card.rs:49`). A
`set_start` override (your_light / detour / …) only lands when the walk starts,
after the `diceRollResult` window, and the trigger carries no start. Expected:
the move's actual start. **Root fix:** stamp `Move.from` from the planned start
before the roll windows (the payload and `trigger::move_from()` already exist,
`ctx.rs:3338`) and let `between()` read it (R4).

**13. `misaki-effective-steps` — HHW:（美咲）, consolidated `plan-live-sync` +
`trigger-walk-payload`**

> 「预定移动终点」

The span uses the raw face (`|roll|`, `misaki_card.rs:41`), while
`next_steps` (普通与理所当然) rewrites the steps after the window
(`play.rs:1625-1627`). Candidates are wrongly included or missed. Expected: the
endpoint the move will actually take. **Root fix:** carry `next_steps` into the
plan frame before the windows and expose the effective length as
`trigger::move_effective_steps()` (R4).

**14. `dest-forced-hop` — CRYCHIC:一起演奏音乐的命运共同体, consolidated
`effect-move-forced-path`**
(`rules/cards/card-crychic/src/fate_together.rs`, cards.json:17)

> 「每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。」

The hop calls `run_planned_move` (`fate_together.rs:106-119`), which treats it
as the main move: refused once the main move is spent, consumed otherwise. Its
bool is the abnormal-gate answer, not whether the move ran, so a spent main move
still reads as a hit. Expected: an effect move that is never the main move and
never refused by the slot. **Root fix:** `plan::set_forced` and
`run_planned_move` returning 「the move ran」 (R3).

**15. `plan-set-forced` — Mujica:无法将视线移开 / Mujica:人偶的箱庭,
consolidated `effect-move-forced-path`** (`rules/cards/card-mujica/src/cant_look_away.rs`,
`rules/cards/card-mujica/src/doll_garden.rs`, cards.json:66, 63)

> 「使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算]…若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动」

> 「你强制移动其他玩家本次移动掷骰数之和。」

Today `forced` is unreachable from the guest, so no walk carries the marker,
and a caster who already moved is refused when they pick themselves though the
book makes that walk their main move. The 「视为其主要移动」 clause has no
counterpart at all. Expected: the victims are forced moves; the self-pick is
the caster's main move. **Root fix:** `plan::set_forced` + `plan::set_main_override`
(R3) — one per clause.

**16. `dg-plan-reset` — Mujica:人偶的箱庭, consolidated `plan-shape-scope`**

> 「你强制移动其他玩家本次移动掷骰数之和。」

Every victim walks with the shared plan (`doll_garden.rs:78-80`), so a stale
`stop_at`, `pay_factor` or `reverse` from an earlier walk leaks in; each
victim's `settles = false` also leaks to the next victim and the summoner's sum
move. Expected: each forced walk starts clean. **Root fix:** every card move
gets its own frame; the prior command's shape is restored (R1).

**17. `lisa-bond-nested-teleport-window` — R:必然的联系（莉莎）, consolidated
`nested-teleport-tail`**
(`rules/cards/card-roselia/src/lisa_bond.rs`, cards.json:136)

> 「当你使用技能进行传送后」

`card_teleport` at `move_depth > 0` is a bare position write with no tail
(`play.rs:875-886`); `ChainKind::MoveAfter` never fires for a teleport nested in
a settle, so the card's window never opens. Expected: the window opens after any
skill teleport, nested ones included. **Root fix:** a tail-only nested teleport
for skill-sourced jumps (R6).

---

## 2. Capability as an invariant

**I1 — Command ownership.** Every move runs on a plan frame owned by the
command (or engine step) that started it (`World::begin_plan` / `end_plan`). The
frame is seeded at begin from the plan as it stood at command entry and never
writes the staging plan directly.

**I2 — One-shot command shape.** Nothing a move writes outlives it. An
outermost command that drove a move restores the staging plan to its entry
value when it ends; one that drove none keeps its writes, because those are the
next main move's staging. A write made *inside* a move lands on that move's
frame.

**I3 — Forcedness is orthogonal to mainness.** `forced` marks an effect-forced
move: it runs even when the main slot is latched and does not itself consume the
slot. Therefore

```
is_main_move = is_turn && (!forced || main_override)
```

Only `is_main_move` moves latch `main_moved`, drive the stage machine and
replay followers.

**I4 — Provenance is first-class.** `rolled` is true exactly when the walk's
length came from a cast die (not a `steps` override, not `next_steps`, not a
teleport); `effective` is the length the move will actually take; `from` is
stamped from the planned start before any roll window.

**I5 — Walk dimensions survive the whole walk.** `steps`, `clear_steps`,
`reverse`, `parity`, `start`, `teleport_to`, `extra_steps`, `stop_at` written
before or during `moveBefore` reach the walk; clearing is explicit
(`clear_steps`, `set_teleport_to(-1)`), never inferred.

**I6 — Parity is a walk rule.** When parity is `0`/`1`, each step moves to the
nearest tile ahead of that parity; skipped tiles are not passed or settled.

**I7 — Replacement is a void, not a second move.** A mid-roll teleport marked
`main_override` voids the pending walk (`void_walk`) and runs immediately, from inside
the window, as the same move with one tail; the roll site only skips its own
walk.

**I8 — The tail is shared.** Every completed move raises
`moveAfter`/`moveResolved`; every `is_main_move` move replays its followers in
the recorded order after the mover settles.

**I9 — Geometry is data.** Axis nearest/farthest read board coordinates from
the board's tile rows; no card carries a private board map.

**I10 — Refusal is clean.** A slot refusal, an abnormal-gate refusal or a
start-time cancellation leaves the staging plan exactly as it stood at the
attempt; a replacement refusal leaves the outer walk armed as a walk.

---

## 3. Surface

### 3.1 Layering recap

Each new import is added at every layer, or the build fails:

1. `crates/card-sdk/src/ctx.rs` — public wrapper (unit or typed).
2. `crates/card-sdk/src/ctx/sys.rs` — `extern` under the
   `wasm32`/`native-rules` attribute pair, link name `ruleset_*`.
3. `crates/card-sdk/src/ctx/sys_kani.rs` — `kani_host::call_*` stub.
4. `crates/card-sdk/src/kani_host.rs` — `Op` variants (README band 378-397;
   this group uses **378-391**).
5. `rules/kani/src/host.rs` — arm (stub or model) and every affected card
   proof's `host_subset!`.
6. `crates/game-rules/src/hostfns.rs` — `HostCtx` implementation.
7. `crates/game-rules/src/native_shims.rs` — trait method + `op_*` +
   `#[no_mangle] ruleset_*`.
8. `crates/game-rules/src/host.rs` — `func_wrap`.
9. `crates/game-rules/src/world.rs` — default `HostCtx` implementation.
10. `crates/game-rules/src/wasm_rules.rs` — live `HostCtx` implementation.

Nothing in this surface makes a player decision. `ask_tile` stays in the card.

### 3.2 Plan frames (engine, new)

`crates/game-core/src/engine/move_ctx.rs`:

```rust
// `PlanKind` is re-exported from `game_kinds::move_plan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanFrame {
    pub plan: MoveCtx,
    /// The staging plan at the command's entry; the restore target of a side
    /// or refused frame.
    pub saved: MoveCtx,
    pub kind: PlanKind,
    pub owner: usize,
}
```

`crates/game-core/src/engine/world.rs` (`TurnCtx`):

```rust
#[serde(default)]
pub plan_stack: Vec<PlanFrame>,
#[serde(default)]
pub plan_scope: PlanScope,
```

`World` also gains, saved beside the scope:

```rust
/// The rollers whose move dice windows are open, innermost last
/// (main-as-vs-walk handoff).
#[serde(default)]
pub move_roll_windows: Vec<usize>,
```

`MoveCtx` gains, all `#[serde(default)]`:

```rust
/// The main move's stored override, copied from `hidden[i]` at the seed.
pub next_steps: Option<i32>,
/// This move is main even though it is forced (「视为你的主要移动」).
pub main_override: bool,
/// This teleport replaces the pending walk instead of following it.
pub void_walk: bool,
/// A die was cast for this walk.
pub rolled: bool,
```

`next_steps` is engine-set from `hidden[i]` at a Main frame seed and never a
guest write; the staging plan leaves it `None`.

The pure decisions live in a heap-free shared module,
`crates/game-kinds/src/move_plan.rs`, so the engine and the Kani proofs run the
same code:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind { #[default] MainMove, SideMove }
/// Every field a plan can shape. The seed/restore match is exhaustive over it.
pub enum PlanField { Steps, Reverse, Parity, Kind, TeleportTo, Start, StartReason,
                     StopAt, SettleTile, SettleRedirect, Resolve, PayFactor,
                     RentFactor, Followers,
                     ExtraSteps, NextSteps, MinimumRoll, Signed, Base, Dice,
                     MoreSteps, NoBuy, CanBuild, BuildAnywhere,
                     SettleAsAgent, Tags, BySkill, Forced, MainOverride, VoidWalk,
                     Rolled, Reason, Roller, Cancelled }
pub enum PlanLifetime { Turn, NextMainMove, Command }
pub fn lifetime_of(f: PlanField) -> PlanLifetime;
/// `Turn` or `NextMainMove` -- a staging field, not one move's shape.
pub fn is_staging(f: PlanField) -> bool;
pub fn is_main_move(is_turn_player: bool, forced: bool, main_override: bool) -> bool;
/// True iff a `run_planned_move` call replaces a walk instead of starting a move.
pub fn replacement_taken(window: Option<usize>, owner: usize, main_override: bool,
                         void_walk: bool, teleport: bool) -> bool;
/// True iff the roll site still walks.
pub fn walk_runs(teleport: bool, cancelled: bool, void_walk: bool) -> bool;
pub fn effective_steps(steps: Option<i32>, override_steps: Option<i32>, roll: i32) -> i32;
pub fn next_parity_tile(current: i32, direction: i32, parity: i32, tile_count: i32) -> i32;
pub fn nearest_tile_on_axis(grid_cells: &[Option<[i32; 2]>], tile: usize, axis: i32) -> Option<usize>;
pub fn farthest_tile_on_axis(grid_cells: &[Option<[i32; 2]>], tile: usize, axis: i32) -> Option<usize>;
```

`is_staging` is the machine-checked form of the lifetime rule. With frames the
runtime restore no longer reads it field by field; it is the Kani obligation
that keeps a new `Command` field from being classified as staging (or the other
way round) and therefore keeps `staging_seed` honest.

`World` accessors, replacing the 137 direct `turn.plan` accesses:

```rust
pub fn plan(&self) -> &MoveCtx;              // top frame, else turn.plan
pub fn plan_mut(&mut self) -> &mut MoveCtx;  // top frame, else turn.plan
pub fn begin_plan(&mut self, owner: usize, kind: PlanKind, seed: MoveCtx);
pub fn end_plan(&mut self, ran: bool);
pub fn enter_plan_command(&mut self, resumed: bool);
pub fn leave_plan_command(&mut self);
```

`move_depth` stays as it is: it counts nested movement for the teleport-tail
rule (§4.7), not plan scoping. The frame depth is `plan_stack.len()`; the body
nesting is `plan_scope.commands.len()`.

`begin_plan` seeds the frame:

- It takes the innermost `PlanCommand`, or `None` for an engine step.
- `saved` is the command's `staging_before`, or the current staging clone when
  there is no command.
- `kind` is `MainMove` iff `is_main_move` was computed true.
- At frame depth 0 an **allocation** (`seed`) is built by the caller:
  - `MainMove` — the whole current plan target (a command's writes are staging and
    shape its own main move).
  - `SideMove` — `MoveCtx::staging_seed(plan_before, staging, target)`: for every
    `Command` field the value the command changed relative to `plan_before`,
    and for every `Turn`/`NextMainMove` field the staging value (those stay for the
    next main). `staging_seed` iterates `PlanField` and consults
    `is_staging`, so a new field cannot be forgotten. With no command scope (an
    inline host rule) the seed is the whole staging.
- At frame depth > 0 the frame is `SideMove` and `saved` is unused; the seed is the
  delta from the innermost command's `plan_before` to the current frame, or the
  whole current frame when there is no command.
- Every depth-0 seed folds `next_move_pay_factor` into `pay_factor` and resets
  the slot to 1000 (G02 §5.5). A nested seed does not fold it again.
- A `MainMove` seed takes `hidden[owner].next_steps` into `next_steps` and clears
  the hidden slot.
- `move_open` is set true while any frame is live.

`end_plan` pops and applies the restore table:

| frame | condition | `turn.plan` after pop |
|---|---|---|
| `SideMove`, frame depth 0 | — | `saved` |
| `MainMove`, frame depth 0 | ran | `Default` |
| `MainMove`, frame depth 0 | refused | `saved` |
| any, frame depth > 0 | — | unchanged (the nested frame is discarded) |

Restoring at the *pop* (not at command end) is what lets one command contain
several moves (人偶的箱庭's victims): each side move drops the writes it was
shaped with, and a write placed after a move survives as the next move's shape.
`move_open` clears when the stack empties.

`plan_scope` is the command stack:

```rust
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanScope {
    pub commands: Vec<PlanCommand>,
    pub moves: u32,
    /// The frame contents when the current roll window opened, innermost last.
    #[serde(default)] pub window_before: Vec<MoveCtx>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanCommand {
    pub staging_before: MoveCtx,
    /// The plan target when the body entered (a frame clone, or staging).
    pub plan_before: MoveCtx,
    pub moves_at_entry: u32,
}
```

`enter_plan_command(false)` pushes
`{ staging.clone(), plan().clone(), scope.moves }`. The resume entry (the
`NeedHost` / `Suspended` continuation in `apply`) calls it with `resumed: true`
and does not push: the paused world still holds the command, and a body that
pauses twice must not stack two snapshots. `leave_plan_command` pops and, when
`scope.commands` is now empty **and**
`scope.moves > ` the popped `moves_at_entry`, restores staging to
`staging_before` and drops the writes the replayed body made after its last
move. A command that drove no move keeps its writes (they are the next main
move's staging). A nested leave touches nothing; the outer command decides.
Every `run_planned_move` attempt (ran or refused) bumps `scope.moves`, except a
replacement, which belongs to the outer move.

The frame depth under a command is what makes the intent split work: an
outermost Play body's writes are staging and seed the move it drives; a
`moveBefore` counteraction's writes are on the live frame and die with it; a
post-move replay write lands on staging and the command's leave drops it.

### 3.3 New imports

`Op` band 378-391; `README` row.

| id | raw import | wrapper | signature (wire) | lifetime | kind |
|---|---|---|---|---|---|
| 378 | `ruleset_set_forced` | `plan::set_forced(bool)` | `(on: i32)` | command | new |
| 379 | `ruleset_set_main_override` | `plan::set_main_override(bool)` | `(on: i32)` | command | new |
| 380 | `ruleset_void_walk` | `plan::void_walk()` | `()` | command | new |
| 381 | `ruleset_clear_steps` | `plan::clear_steps()` | `()` | command | new |
| 382 | `ruleset_add_extra_steps` | `plan::add_extra_steps(i32)` | `(n: i32)` | command | new |
| 383 | `ruleset_move_planned_teleport` | `plan::planned_teleport() -> i32` | `() -> i32` | read | new |
| 384 | `ruleset_move_pay_factor` | `plan::move_pay_factor() -> i32` | `() -> i32` (milli) | read | new |
| 385 | `ruleset_move_rent_factor` | `plan::rent_factor() -> i32` | `() -> i32` (milli) | read | new |
| 386 | `ruleset_move_effective_steps` | `plan::effective_steps() -> i32` | `() -> i32` | read | new |
| 387 | `ruleset_move_forced` | `plan::forced() -> bool` | `() -> i32` (0/1) | read | new |
| 388 | `ruleset_trig_move_rolled` | `trigger::move_rolled() -> bool` | `() -> i32` (0/1) | read | new |
| 389 | `ruleset_trig_move_effective_steps` | `trigger::move_effective_steps() -> i32` | `() -> i32` | read | new |
| 390 | `ruleset_nearest_tile_on_axis` | `nearest_tile_on_axis(tile, axis) -> i32` | `(tile: i32, axis: i32) -> i32` | pure | new |
| 391 | `ruleset_farthest_tile_on_axis` | `farthest_tile_on_axis(tile, axis) -> i32` | `(tile: i32, axis: i32) -> i32` | pure | new |

`trigger::move_from()` already exists (`ctx.rs:3338`, import
`ruleset_trig_move_from`) and is not re-added.

Extended, wire-compatible:

- `set_parity` (`ruleset_set_parity`) — canonicalizes `2` to `0`: `-1` any,
  `0` even, `1` odd. Non-canonical values are clamped to `-1` inside
  `ops.rs::set_parity`, so every caller and the save path see one encoding.

`crates/card-sdk/src/abi.rs` adds:

```rust
pub mod axis {
    pub const UP: i32 = 0;
    pub const DOWN: i32 = 1;
    pub const LEFT: i32 = 2;
    pub const RIGHT: i32 = 3;
}
```

Semantics of the new writes:

- `set_forced(on)` — writes `plan.forced`. A forced move of the turn player is
  not the main move unless `set_main_override(true)`.
- `set_main_override(on)` — writes `plan.main_override`. `is_main_move` is then
  `is_turn && (!forced || main_override)`. In a move's own roll window, a
  `run_planned_move` with `main_override` and `void_walk` replaces the pending walk in
  place (§4.4).
- `void_walk()` — writes `plan.void_walk`. With `main_override`, inside the move's own
  roll window, it makes the next `run_planned_move` replace the pending walk
  with the planned teleport immediately (§4.4). Without `main_override`, or outside
  the window, the roll site just suppresses the walk: staged at Play it
  suppresses the next main's walk (pair it with a `teleport_to` when a teleport
  main is meant). A no-op for a teleport command.
- `clear_steps()` — `steps = None`; the next `run_planned_move` rolls. The roll
  keeps its sign.
- `add_extra_steps(n)` — `plan.extra_steps += n` (signed), composing with other
  writers. `set_extra_steps` remains the absolute form.

Getters:

- `planned_teleport()` — the planned destination, `-1` when none.
- `move_pay_factor()` — the **plan half** only, milli (`1000` = ×1). The
  composed scoped value is G02's `ctx::plan::pay_factor()`; callers never add
  the two.
- `rent_factor()` — milli.
- `effective_steps()` — `steps`, else the frame's `next_steps`, else `roll`.
- `forced()` — the flag, as 0/1.
- `trig_move_rolled()` — was the *triggering* move a rolled walk? 0 when no
  move.
- `trig_move_effective_steps()` — the triggering move's effective length, 0
  when no move.

Axis codes: `abi::axis` `0` = up, `1` = down, `2` = left, `3` = right (the
`happy_lucky.rs` 1..4 order minus one). `nearest_tile_on_axis(t, axis)` is the
closest tile strictly along that ray from `t`, or `-1`; `farthest_tile_on_axis`
is the farthest one, or `-1`. Both exclude `t` itself and are board-only.

### 3.4 Trigger payload

`TriggerMove` (`rules.rs:52-78`) gains:

```rust
/// A die was cast for this walk (a `steps` walk and a teleport are 0; a
/// `next_steps` override still casts, so it stays 1).
pub rolled: bool,
/// The length the move will actually take (`effective_steps`).
pub effective: i32,
```

`MoveCtx` (`Move` is an alias of it) gains the matching reader
`fn effective_steps(&self) -> i32 { self.steps.unwrap_or(self.roll) }` (the
frame's `next_steps` is already mirrored into `roll`), and
`move_plan::effective_steps` stays the pure form for the Kani proof. `with_move`
(`rules.rs:397-416`) stamps `rolled: m.rolled` and `effective: m.effective_steps()`; `bridge_trigger` rebuilds the `TriggerMove` from the
trigger (`wasm_rules.rs:7789-7796`) and carries both through. The guest `abi::Trigger`
payload and its accessors gain the pair. Cond gains `move.rolled` (Bool) and
`move.effective` (Int) in `cond/src/view.rs`, `cond/src/view_impl.rs`,
`cond/src/vocab.rs`, and `cond-ir/src/vars.rs` (`MoveRolled`, `MoveEffective`).
`move.rolled` is the charity condition; `move.effective` is the misaki span
length.

`rolled` is set true at the entry of the roll branch — before an override is
applied, because the cast still happened — and by any reroll inside the same
snapshot; a fixed-step walk never enters the branch and leaves it false, and
the replacement branch clears it for a teleport. `effective` on a
`passTile`/`pass`/`moveAfter` trigger is the planned total; on a roll window it
is the face or the pending `next_steps`, whichever will be used.

### 3.5 Card-shape writes through the frame

`ops.rs`'s plan setters write `self.plan_mut()` instead of `self.turn.plan`.
The engine step callers (`PlayCtx::main_move`, `card_move_seeded`,
`settle_tile_at`) open and close frames; a card body reaches `plan_mut()`
through the live frame. `HostCtx::move_plan` returns `self.plan().clone()`.

The new setters are ordinary plan-write handlers in `hostfns.rs`, so
`Run::stamp_skill_source` (`wasm_rules.rs:393-402`) tags them like the existing
ones; `set_forced`/`set_main_override`/`void_walk`/`clear_steps`/`add_extra_steps`
need no special case. A refused `Move` clears the tag (§4.2).

### 3.6 Board geometry (data)

`TileData` gains:

```rust
/// Board grid cell `[column, row]` for axis queries. `None` on boards without a grid.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub grid_cell: Option<[i32; 2]>,
```

`data/board.json` rows carry `grid_cell` from the drawn board's `pathXY` fold (the
existing `CELLS` table and `Ring.tsx`'s `FOLD` are the same 60 pairs and agree;
the board file is the authority, the client fold is its renderer). The
`skip_serializing_if` keeps a coordinate-less board byte-identical in a record,
so old records and old board copies are unchanged.

`GameData` gets:

```rust
/// Built once at load from the tile rows; index-aligned with `tiles`.
grid_cells: Vec<Option<[i32; 2]>>,
pub fn nearest_tile_on_axis(&self, tile: i32, axis: i32) -> i32;
pub fn farthest_tile_on_axis(&self, tile: i32, axis: i32) -> i32;
```

The cache keeps the query allocation-free, so `nearest_tile_on_axis` /
`farthest_tile_on_axis` take a plain `&[Option<[i32; 2]>]`.

Both return `-1` when coordinates are absent, the axis invalid, or no tile lies
on the ray. They are pure and board-only; `happy_lucky`'s private table is
deleted once the data is filled.

### 3.7 Not added

- No `plan_save`/`plan_restore` pair: the frame does it.
- No parity-free `next_tile` export: parity is a walk rule.
- No `set_pay_factor_mul`: G02's `multiply_pay_factor` is it.
- No `move_from` import: it exists.
- No `set_next_steps` change: the slot stays on `hidden[i]` and is consumed
  into the frame.
- No import decides anything. `ask_tile` stays a guest prompt.
- None of the additions is state sugar: every one is a direct engine op; there
  is no desugaring layer to update.

---

## 4. Engine wiring

### 4.1 Command scope and the deleted patch

`drive_inner_body` / `drive_inner_body_from` (`wasm_rules.rs:3184, 3198`) wrap
the guest call with `cx.enter_plan_command(resumed)` / `leave_plan_command()`
(the resume arm of `apply`, `wasm_rules.rs:2840-2884`, is the only `resumed: true` caller). Entering snapshots the command; leaving drops it and, at the
outermost level, applies I2. A pause keeps `commands` in the saved world and the
resumed body does not take a new snapshot: the replayed body rewrites the same
values the first pass wrote.

Deleted with this landing:

- `steps_fresh`, `stop_at_fresh`, `pay_fresh`, `reverse_fresh`, `resolve_fresh`
  (`move_ctx.rs:176-196`) and every write of them.
- `drop_stale_plan_shape`, `drop_plan_fresh`, `drop_spent_plan_shape`
  (`play.rs:2116-2216`) and the `play_drove_move` latch field
  (`world.rs:189-197`) plus its set/restore in `use_skill`, `draw_event` and
  `run_planned_move`.
- The `move_depth > 0` mem-swap and `drop_plan_fresh` call in
  `run_planned_move` (`play.rs:937-954`).

`seed_turn_plan` becomes `adopt_plan(&mut self, m: &Move)`: it clears
`plan_mut().settle_redirect` and writes the runtime progress fields of the top
frame from `m`. The "shared plan" readers (`scale_settle_payment`,
`trigger::move_from` fallbacks, `take_plan_teleport`) migrate to `plan()`.

### 4.2 Creation and sealing

`main_move` (`play.rs:1518`):

1. Refuse if `main_moved` (unchanged); the one-shot start is already staging
   and the command leaves it there.
2. `begin_plan(i, MainMove, seed)` — before `diceRollBefore`. The seed is the
   current staging clone.
3. Stamp `m.from = m.start.map_or(pos, |t| t.0)` and `m.to = m.from`.
4. If the frame carries `next_steps`, seed `m.roll` with it *before* the
   windows, and re-apply it after each `diceRollResult` raise (the raise returns
   `t.value`, which would otherwise clobber the override). The window therefore
   sees the length the move will take, and `trig_move_effective_steps()` is
   right for 美咲.
5. Run the roll ladder and `rollPlan`; window writes are on the frame (§4.3).
   After the last raise, read the frame back into `m` for the shape the walk
   reads (`teleport_to`, `steps`, `start`, `parity`, factors, `settles`, …).
6. `walk`; a replacement has already run the teleport and raised its tail, so
   only the completion remains.
7. `end_plan(m.started)`.

`run_planned_move` (`play.rs:899`) computes the slot policy:

```
let forced = plan.forced;
let main_override = plan.main_override;
let is_main_move = move_plan::is_main_move(is_turn, forced, main_override);
let owner = self.plan_stack.last().map_or(i, |f| f.owner);
let replace = move_plan::replacement_taken(w.move_roll_windows.last().copied(),
                                          owner, main_override, plan.void_walk,
                                          plan.teleport_to.is_some());
if is_main_move && main_moved && !replace { refuse; return false }
if is_main_move && !main_moved { main_moved = true }
if replace { return self.replace_main_move(i, plan) }   // §4.4
```

Otherwise it opens a frame (`MainMove` for `is_main_move`, `SideMove` otherwise)
through `begin_plan`, stamps `from`, and runs the shared body: the
`diceRollBefore` stage is the turn move's alone, the rest (a `rollPlan` cast
when there is no step count, the ladder, the walk, the tail) is common.
`end_plan(ran)` seals it. The return is **"the move ran"**: false on the slot
refusal, false on the gate refusal, false on a start-time cancellation, true
otherwise.

`HostRequest::Move` (`wasm_rules.rs:4011-4041`) keeps the gate call site:

```rust
let allowed = self.abnormal_gate(cx, player_id, AbKind(5), call.player_id())?;
if allowed {
    return Ok(cx.run_planned_move(sys_id, plan)? as i32);
}
// refused: clear every one-shot this attempt wrote. `plan_mut` is the frame
// when the call sits inside a move (misaki), staging otherwise.
let mut w = cx.world_copy();
let p = w.plan_mut();
p.teleport_to = None;
p.void_walk = false;
p.start = None;
p.start_reason.clear();
p.by_skill = false;   // `stamp_skill_source` set it on every write above
p.kind = MoveKind::Walk;
cx.swap_world(w);
Ok(0)
```

Today the arm clears only `teleport_to` (`wasm_rules.rs:4036-4038`) and leaves
`by_skill`, `start` and `start_reason` set; a refused forced move from a skill
therefore still tags, and re-seats, the next move. The engine's own refusal
already clears `teleport_to`/`start`/`start_reason` (`play.rs:913-918`); the
host copy is the one that forgot the rest.

`main_move` and `card_move_seeded` share one `run_move_body` helper so the
tail, followers and bookkeeping cannot diverge.

`skip_main_move` stays as it is: it is the turn-flow latch `MatchState.skip_move`,
not a plan field, and no frame clears or restores it. `void_walk` is narrower —
it skips one move's walk, not the move phase.

`teleport_as` opens a frame of its own kind when no frame is live
(`plan_stack` empty): `MainMove` when its `is_main_move` argument is set, `SideMove`
otherwise, seeded by `staging_seed` from the innermost command, and pops after.
When a
frame is live the nearest caller owns one (`replace_main_move` §4.4, follower
replay §4.6, the main/side move frame) and `teleport_as` uses it.
`card_teleport` becomes:

- `move_depth == 0` — `teleport_as(…, settles: false, is_main_move: false)`
  (which scopes itself).
- `move_depth > 0` and `by_skill` — position write, then `teleport_tail_only`
  (§4.7).
- `move_depth > 0`, not `by_skill` — the existing bare write.

`HostRequest::TeleportAs` relies on the self-scoping; its temporary `settles`
write and the save/restore around it (`wasm_rules.rs:4081-4098`) collapse into a
`plan_mut()` write before the call (the frame seed carries it; the post-window
re-read uses `plan()`). The arm keeps returning `Ok(1)` — it is the guest
request, not a run whose refusal the body branches on; `replace_main_move`
(§4.4) calls `PlayCtx::teleport_as` directly, so it sees the bool.
`HostRequest::Move`'s `main_moved` write is the host's own; a replacement never
passes through it (the outer move already latched the flag).

`settle_tile_at` (`play.rs:1181`) is the third top-level move entry: it opens a
Side frame from the plan clone `card_settle_move` built, runs the settle move,
and pops, so a nested settle's counter writes do not shape the outer move.
`settle_redirect` stays engine-set on the moved clone.

### 4.3 The roll window

Both entry points push `i` onto `w.move_roll_windows` and `frame.clone()` onto
`plan_scope.window_before` around `diceRollBefore`…`diceRollResult`, and pop
both after the last raise. A nested move with its own roll inside a counter
window gets its own snapshot instead of clobbering the outer one. The fold is
`fold_plan_delta(&mut m, window_before.last().unwrap(), &frame)` before the
pop. This is what makes a detour counter and a misaki teleport land.

`fold_plan_delta` (`play.rs:108-180`) changes from "copy when changed and
non-empty" to "copy when changed": the `is_some()` guards on `teleport_to`,
`steps` and `start` and the `>= 0` guards on `parity` and `settle_tile` all go,
so a window can *clear* a field (`teleport_to`, `steps`, `parity`) as well as
set it. `start` copies `start_reason` with it in either direction. Add
`main_override` and `void_walk`.

Clearing is what makes the fold usable both ways: a `void_walk` written in
`diceRollBefore` and then withdrawn (a gate refusal) is folded back as `false`,
and a card that clears an inherited parity gets a plain walk instead of the
staging filter.

G03 (after this group) keeps both raises inside this snapshot and adds none
outside it: the main move raises `diceRollBefore` *after* casting, a reroll
re-raises `diceRollBefore`+`diceRollResult` inside the same snapshot, and a
preview roll (no open move, 去唱卡拉ok吧's candidates) has no frame to fold
into. G03 item 7 (`detour-plan-writes-lost`) is therefore this group's, not
G03's.

Then:

- If `m.void_walk && m.teleport_to.is_some()`: the replacement already ran and
  raised its tail from inside the window (§ below). The caller frame was written
  back with `kind = Teleport`, `teleport_to = Some(landing)`, `path = [landing]`,
  `to = landing`, `total = 1`, `remaining = 0`, `cancelled = !ran`. Copy `kind`,
  `path`, `to`, `total`, `remaining` and `cancelled` onto `m` (the fold carries
  no progress fields; `teleport_to` comes from the fold) and set
  `m.rolled = false`. The call condition below skips the walk. A cancelled
  replacement has no tail either.
- Else if `m.teleport_to.is_some()`: a bare window write (no `void_walk`); run
  `teleport_as(i, to, m.settles, m.reason.clone(), m.is_main_move)` here, set
  `m.kind = Teleport`, `m.rolled = false`, `m.total = 1`, `m.remaining = 0`,
  `m.path = [to]` and `m.cancelled = !ran` (`m.to` comes from the player after
  the call). The tail is `teleport_as`'s; the body does not wait on it.
- Else if `m.void_walk`: a staged void with no teleport target; nothing runs.
  Set `m.kind = Teleport`, `m.total = 0`, `m.remaining = 0` so the summary
  records no walk and no `lastWalk`.
- Otherwise the call condition below walks `m`.

The call site is:

```rust
if !self.out(i) && move_plan::walk_runs(m.teleport_to.is_some(), m.cancelled,
                                        m.void_walk) {
    self.walk(&mut m)?;
}
```

so a set `void_walk`, a set `teleport_to`, a
cancellation and an out mover each skip the walk exactly once (a staged void
with no target is the third branch); the ordinary pre-roll teleport never
reaches this point (its branch returns before it).

`m.roll` is already the effective length (§4.2 step 4), so a teleport in the
window sees the same endpoint the walk would have used and the post-raise
`m.roll = t.value` assignment keeps the override when there is one.

### 4.4 Replacement

A `run_planned_move` issued from a move's own roll window
(`move_roll_windows.last() == Some(player)`, the frame owner is `player`, the
passed plan has `main_override && void_walk && teleport_to.is_some()`) does not open a
second move: `fn replace_main_move(&mut self, i: usize, plan: Move) -> Flow<bool>`. The check runs **before** the `main_moved` refusal, because the
outer main already latched it.

1. `begin_plan(i, SideMove, staging_seed(command.plan_before, staging, target))`.
   The command's delta already carries the teleport writes, so no seed is built
   for them; `teleport_as` then sees a live frame and does not add its own. The
   `by_skill` from the passed plan is written to the frame.
2. `let ran = teleport_as(i, to, plan.settles, plan.reason.clone(), true)`.
   `to` is the captured `plan.teleport_to`; `landing` for the write-back is
   `st.players[i].pos` afterwards (a `moveBefore` counter may retarget it).
   The Side frame scopes the `moveBefore` counter's writes.
3. `end_plan(ran)`, then write `kind`, `teleport_to = Some(landing)`, `path`,
   `to`, `total`, `remaining` and `cancelled = !ran` onto the caller's frame as
   above; the move's one-shot cleanup clears `teleport_to` later; return `ran`.
   No `main_moved`/step latch, no follower replay, no `lastWalk`/`main_steps`:
   the outer move does all three.

The outer move skips only its walk; completion is otherwise the ordinary
teleport path, so there is exactly one tail and one settle. The body resumes
after the teleport has resolved, which keeps the card's 「随后…支付」 order.

A gate refusal before `replace_main_move` clears `void_walk`, `teleport_to` and
resets `kind = Walk` through `plan_mut()` (§4.2), so the outer move walks the
roll normally instead of taking the void branch with no teleport.

### 4.5 Walk invariants

`walk_inner_depth` (`play.rs:1812`):

- `next` becomes `next_parity_tile(cur, m.dir(), m.parity, n)` (the loop's
  existing `cur`/`n` locals).
- At the top of every iteration, sync `m.extra_steps = self.plan().extra_steps`.
  `planned` and `remaining` then follow it.
- `rolled` stays as the roll branch set it; a `steps` walk never entered it.
- `total`/`remaining`/`path` count parity *landings*; skipped tiles raise no
  `passBefore`/`passTile`/`pass`.
- `stop_at` is compared against the chosen tile. A `stop_at` tile of the other
  parity is not reachable while parity is set (open question §10.3).
- `from` is stamped before the windows; the `start` branch moves the piece
  there and re-stamps `from` only when a `moveBefore` counteraction rewrote
  `start`.

```rust
/// The next tile in `direction`: the immediate neighbour when `parity < 0`, else
/// the nearest tile whose index parity matches.
fn next_parity_tile(current: i32, direction: i32, parity: i32, tile_count: i32) -> i32
```

The step loop's plan reads (`stop_at`, `settles`) keep working through
`plan()`; the frame's writes are already the walk's shape until the pop.

### 4.6 Shared tail and followers

`run_move_body` ends with `move_after` → `after_walk` → `move_resolved` for
walks and `teleport_as_depth` for teleports. After the movement completes and
before the completion bookkeeping:

```rust
if is_main_move && !m.cancelled { self.replay_followers(&m)?; }
```

`replay_followers` is the block currently at `play.rs:1702-1735`, moved with one
addition: `take` the followers, skip on
cancelled/out/self, then run each survivor under its own Side frame seeded from
`m` (`begin_plan`/`end_plan` around the clone+walk, `end_plan` on every return
path), so a follower's `moveBefore` counter writes die with that follower. The
clone still clears identity/progress and the follower list; walk or teleport as
today. The factor seed is now the frame's `pay_factor`/`rent_factor`, not two
direct plan writes. It runs for card-driven main moves too.

### 4.7 Nested skill teleport tail

```rust
/// Raise the movement tail for a teleport nested inside another move's stage
/// (a skill jump in a settle). Position is written by the caller; this raises
/// `moveAfter` then `moveResolved` on a synthetic teleport Move with
/// `kind = Teleport`, `by_skill = true`, `from`, `to`.
fn teleport_tail_only(&mut self, i: usize, from: i32, to: i32) -> Flow<()>
```

The tail runs under a `SideMove` frame seeded from the plan (as §4.6), so a
`moveAfter` counter's writes die with the nested jump. An `in_teleport_tail`
guard (one bool) prevents re-entering `teleport_tail_only` from a tail hook.
`lisa_bond` needs no change: its existing `MoveAfter` /
`move.kind == Teleport && move.by_skill` filter starts matching.

### 4.8 Geometry

`ops.rs` gains the two query wrappers over `GameData`, `hostfns.rs` the
`HostCtx` methods, and `happy_lucky`'s private table is deleted once the data is
filled. `GameData::nearest/farthest_tile_on_axis` scan the `grid_cells` cache along the axis and pick
the minimal/maximal offset; only strict ray members qualify, and the whole
answer is `-1` when the cache is empty (a board without `grid_cell`).

### 4.9 Host pauses and the guest copy

`HostCtx::move_plan` (`wasm_rules.rs:2660`) returns `self.plan().clone()`.
`HostCtx::move_*` getters likewise read `plan()`. `adopt_turn_policy`
(`cx.rs:867`) replaces its hand-picked plan copies (`settles`, `pay_factor`,
`rent_factor`, `can_build`, `no_buy`, `tags`, `:895-907`) with the whole
staging `plan`, `plan_stack`, `plan_scope` and `move_roll_windows`; the
`next_move_pay_factor` / `next_settle_pay_factor` copies stay. The routine is
about the same move, so nothing else crosses the pause, and the progress fields
stay on the `Move` untouched. Because the adopted stack is the guest copy's, an
interrupted engine call's own frame is dropped and opened again by the retry,
exactly like the guest body's replay. `World::plan()` is used by
`scale_settle_payment` and every other settle reader.

### 4.10 Kani model

- `MoveSnap` (`rules/kani/src/world.rs:240-282`) gains `rolled` and
  `effective`; `NONE` (`:264-274`) uses `rolled: false, effective: 0`.
- `arm_card_move` is today a constant `1` (`rules/kani/src/host.rs:1482-1486`,
  wired at `:868`) and `Op::teleport_as` a bare position write with no
  `main_moved` flag (`:842-850`). Both change:
  - the kani world gains `main_moved`, `forced`, `main_override`, `void_walk`,
    `teleport` and `window`;
  - `arm_card_move` answers:

    ```rust
    if is_main_move && main_moved
        && !(window && main_override && void_walk && teleport)
    { 0 } else { 1 }
    ```

    (the pure `replacement_taken`, inlined), and a `1` answer with
    `is_main_move` latches `main_moved`;
  - `teleport_as` sets `main_moved` when its `is_main_move` argument is set.
- Plan setters (`set_forced`, `set_main_override`, `void_walk`, `clear_steps`,
  `add_extra_steps`) join the protocol stub arm (`host.rs:1036-1072`);
  `clear_steps` also writes the model's `steps`; getters join the zero-return
  group. `trig_move_rolled`/`_effective` read the snap. `nearest/farthest_tile_on_axis` return
  `-1` unless the model board is enabled.
- The new names are appended after `rent_quote` (`kani_host.rs:388`, Op ids
  378-391) and to `ARRAY_OPS` beside it (`rules/kani/build.rs:317`).
- The pure-model proofs live in `rules/kani/src/proofs/move_plan.rs` (declared
  in `proofs/mod.rs`) over `game_kinds::move_plan`; `rules/kani/Cargo.toml`
  adds `game-kinds` (already transitive through `card-sdk`) as an explicit
  dependency.
- Every card proof that reaches a new import adds it to its `host_subset!`
  (`rules/kani/src/proofs/{ag,crychic,hhw,mujica,roselia,morfonica,sumimi}.rs`);
  `rules/kani/build.rs` allowlists the names.
- `set_parity`'s canonicalization is mirrored in the Kani `set_parity` arm
  (`2 → 0`).
- `arm_card_move`'s old constant-1 is replaced so the fate/misaki proofs can
  distinguish refused from ran.

---

## 5. Card usage — before / after

### 5.1 `want_to_grab` (item 1)

Before: `on_turn_start` retires unconditionally; clause (3) is a TODO
(`want_to_grab.rs:115-120`).

After:

```rust
fn on_turn_start(player_id: i32) -> card_sdk::Asked {
    if ctx::state::get(player_id, PASSER) == NEVER {
        let pos = ctx::player_pos(player_id);
        let mut tiles = Vec::new();
        use card_sdk::abi::axis;
        for a in [axis::UP, axis::DOWN, axis::LEFT, axis::RIGHT] {
            let t = ctx::nearest_tile_on_axis(pos, a);
            if t >= 0 { tiles.push(t); }
        }
        if !tiles.is_empty() {
            let to = ctx::ask_tile(
                player_id,
                &Msg::new(key!("want_to_grab_title")),
                &Msg::new(key!("want_to_grab_ask")),
                &tiles,
            )?;
            ctx::plan::set_teleport_to(to);   // also sets kind = Teleport
            ctx::plan::set_settles(true);
            ctx::run_planned_move(player_id);
        }
    }
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::state::set(player_id, PASSER, NEVER);
    Ok(())
}
```

The query already excludes the origin and returns `-1` off-board, so the
candidate list is at most four and the pick stays a guest prompt.
`set_teleport_to` derives `kind` (§3.3). The `run_planned_move` is the turn
start's main move: it opens a Main frame, latches the slot and settles.
`set_kind` is therefore redundant.

### 5.2 `pure_wings` (items 2-4)

Before: relies on the fresh marks for the count and the factor; carries three
TODOs (`pure_wings.rs:28-30, 38-40, 45-47`). The body already clears
`teleport_to` and re-arms the walk (`:50-54`), but the comment leaning on
`drop_stale_plan_shape` for `steps` stays an assumption.

After:

```rust
let dir = ctx::plan::dir();               // the body's own frame; nothing else
let to = ctx::tile_steps_ahead(player_id, 20 * dir);
if to < 0 { return Ok(()); }
ctx::plan::set_settles(false);
ctx::teleport_to(player_id, to);          // the jump (no tail frame)
if ctx::player_out(player_id) { return Ok(()); }
ctx::plan::clear_steps();                 // roll from the destination
ctx::plan::set_kind(card_sdk::abi::MoveKind::Walk);
ctx::plan::set_teleport_to(-1);
ctx::plan::set_settles(true);
ctx::run_planned_move(player_id);
```

`dir()` needs no change: the frame was seeded from the command, so the card's
own writes are the only delta. `clear_steps` replaces the "`steps` is `None` on
a fresh plan" assumption with an explicit clear, and the factor is the frame
default. The `MoveKind` import moves in with the other imports.

### 5.3 `charity_show` (items 5-6)

Before: condition `… && move.kind != Teleport && move.is_main_move && tile.id >= 0`
(`charity_show.rs:38`); body keeps a running absolute count in
`PROP_EXTRA` and writes it through `set_extra_steps` (`:139-147`).

After: condition `… && move.kind != Teleport && move.rolled && tile.id >= 0`;
body calls `ctx::plan::add_extra_steps(2)`. `PROP_EXTRA` and its clamp are
deleted (the frame's relative sum is the count); the first-pass marker stays.

### 5.4 `sakiko_lead` (item 7, fixed)

No body change: `multiply_pay_factor(500, PayFactorScope::NextMove)` is already
the landed G02 call (`sakiko_lead.rs:58`). Remove the TODO. The shared tail
makes the recorded followers walk on the card main move too; the half still
reaches them because `NextMove` is folded into the Main frame seed.

### 5.5 `detour` (items 8-9)

Guard unchanged (demoted to A). Parity already written; after the walk change
it works. Remove both walk TODOs and the lost-writes TODO. `2` is
canonicalized by `set_parity`, so the even branch may keep `0`:

```rust
// even
ctx::plan::set_parity(0);
```

### 5.6 `saki_move` (item 10)

Add before the existing plan block:

```rust
ctx::plan::set_forced(true);
ctx::plan::set_main_override(hit == player_id);
```

Rewrite the comment at `:49-51` (it says `forced` is event-only), remove the
shape-leak TODO. Targeting yourself keeps `main_override` true and consumes the
slot; targeting someone else never touches it. `set_pay_factor`
may become `multiply_pay_factor(500, ThisMove)` for composition consistency
(G02 owns the semantics; the half is otherwise identical).

### 5.7 `misaki_card` (items 11-13)

G03 changes the pre to `roll_source == 1` and keeps the body; G04 changes the
body only. The body already writes `set_cancelled`, `set_kind(Teleport)`,
`set_teleport_to(to)` and `set_settles(true)` (`misaki_card.rs:82-87`); two
lines and one call site change:

```rust
trigger::set_cancelled();                       // the link only
ctx::plan::set_kind(MoveKind::Teleport);
ctx::plan::set_teleport_to(to);
ctx::plan::set_settles(true);
ctx::plan::set_main_override(true);
ctx::plan::void_walk();
let _ran = ctx::run_planned_move(player_id);    // replaces the walk now
```

`between()` uses `trigger::move_from()` (item 12) and
`trigger::move_effective_steps().abs()` (item 13). The teleport has already
settled when `run_planned_move` returns, so the log names `to` rather than
`player_pos` (the settle itself may move the player again; the chosen
destination is the stable fact). The run's bool guards the fire-pot payout and
the log: on a gate refusal the outer walk is armed again, so the body pays
anyway (the fire pot is spent on play, before the move) and logs the walk's
outcome instead.

### 5.8 `fate_together` (item 14)

Before: `if !ctx::run_planned_move(player_id) { hit = 0 }`, with the hop
mistakenly main (`fate_together.rs:112-119`).

After:

```rust
ctx::plan::set_forced(true);   // an effect move, never the main move
ctx::plan::set_steps(i);
ctx::plan::set_settles(false);
if !ctx::run_planned_move(player_id) { hit = 0; return Ok(()); }
```

The bool now means "ran", so a spent main move is not a false hit.

### 5.9 `cant_look_away` / `doll_garden` (items 15-16)

`cant_look_away` sets the flags before its existing `set_steps` block
(`cant_look_away.rs:97-105`):

```rust
ctx::plan::set_forced(true);
ctx::plan::set_main_override(counter_user == player_id);  // 「选择了使用者自己」
```

`doll_garden` sets `set_forced(true)` before every victim walk (never
`main_override`; a victim is not the turn player) and `set_forced(true)` +
`set_main_override(true)` before both sum walks — the zero-sum branch (`:96-100`) and
the real one (`:114-116`), since both are 「视为你本回合的主要移动」. Each
victim runs on its own side frame; the sum walk is a Main frame seeded from the
clean staging, so the victims' shape cannot leak into it. Remove both TODOs.

### 5.10 `lisa_bond` (item 17)

No body change. The existing `MoveAfter` / `move.kind == Teleport && move.by_skill`
filter starts matching nested skill teleports; remove the TODO.

### 5.11 `happy_lucky` (geometry)

No rule change: `CELLS` / `cell` / `farthest` helpers are deleted and the 60-tile guard
goes (the query answers `-1` on any board). The card's `num = r % 4` is 1/2/3/0
for 上/下/左/右; the axis code is `(num + 3) % 4` (0..3 = up/down/left/right), so
the call becomes `ctx::farthest_tile_on_axis(p, (num + 3) % 4)` and keeps its
`far >= 0` fallback to the current tile.

### 5.12 Remaining group rows

The consolidated rows also carry card-level findings that no item above names.
They ride this group's mechanisms where they touch them and are otherwise
small card fixes; every one is closed in the same batch.

| row | card | finding | disposition |
|---|---|---|---|
| 107 | `tomoe_savior` | bought-back deed is always un-mortgaged (`purchase.rs:346-349`) — the sheet wants 抵押 | re-verified already right; close with a test, remove the TODO |
| 129 | `himari_plus_one` | the `None` branch of the roll condition is unreachable | delete the branch |
| 131, 132 | `himari_plus_one` | trigger half and condition carry bare quotes, not `// 规则书: 「…」` | citation fix |
| 200 | `cant_look_away` | play entry reads 拥有者 where the cell says [使用者] | key the handler on the caster (the play trigger's `player_id`) |
| 203 | `cant_look_away` | retired `skillUsed` spelling in the link declaration | `ActionKind::CardUsed` |
| 209 | `cant_look_away` | counteract clause quoted without the marker | citation fix |
| 288 | `umiri_card` | an `extra` attachment is a band-skill card on the holder's field but the discard filter skips it | drop the `extra` exclusion |
| 290 | `umiri_card` | discard ordered after the draw; `set_dest` only parks the instance | discard first, then draw |
| 292, 294 | `mortis_instinct` | empty `On::Counteract` link list — the card never registers a kind bit | links `[ChainKind::Card]` |
| 293, 295 | `mortis_instinct` | `run()` is a log call: no play history, no case filter, no execution; the module summary is present-tense | **G06** `mortis-copy-links` |
| 291, 298 | `mortis_instinct` | byte drift in the header quote; 任意 added to a citation | citation fix |
| 338, 339 | `elegant_shout` | trigger registration and turn-end binding carry no citation | citation fix |
| 340 | `elegant_shout` | window opens for a self-caused out-of-turn draw | exclude `target == actor` |
| 431 | `mutsumi_never` | 「然后弃一张卡」 silently skipped with an empty hand | log the skip (`log.discard_skipped`) |
| 560 | `kokoro_circle` | `transfer` leaves `p.tile` unset, so `pay`/`paid` carry tile `-1` | pass the tile through G02's pay command |
| 835 | `ordinary` | `next_steps` is consumed by any rolled card move, not only a main one | this group: the slot moves into the Main frame seed (§4.2) |
| 954 | `chisato_mask` | `cancelled_designations` is play-scoped and survives the reshuffle | scope it to the activation like the plan writes, and clear at `leave_plan_command` |
| 963, 964 | `eve_bushido` | handler keys on 拥有者; a tie returns before `try_resonance` | caster key; move `try_resonance` above the tie return |
| 973 | `hina_sound` | the one [共鸣] branch that never calls `try_resonance` | add the call before the branch |
| 1003 | `no_expectation` | the shave registers only on `DiceRollValue`; effect rolls raise only `diceRollBefore` | **G03** (effect-roll stage ladder) |
| 1006 | `no_expectation` | `SLOT_STACKS` is written and read but never consumed or reset | **G02** pay stack accounting |
| 1007 | `no_expectation` | 「生效2次」 lifetime (until next payment or turn end?) unruled | **G02** / open ruling |
| 1025 | `again_shine` | `can_record` refuses when no unrecorded owned colour is left | card condition fix |
| 1038 | `strong_flower` | a [不可阻挡] player must be able to decline a [强制停下] | **G12** forced-vs-immune ordering |
| 1064 | `trainee_guide` | `cards_in` is a private guest read, so the reveal is invisible | log the revealed set |
| 1095 | `star_beat` | `play` grants the slot but sets no `Dest`; the card goes to the graveyard | set `Dest::Field` |
| 1101 | `star_beat` | `can_build_on` claims affordability and the wrong target set | split the condition |
| 1182 | `saaya_sky` | shared latch and lost-fire divergence, no TODO | card fix + TODO(规则书) |
| 1183 | `rimi_choco` | `cant_play` refuses a [停留] player the whole card | gate on `cant_move_code != 3` |
| 1184 | `rimi_choco` | raw `set` clobbers an existing expiry | `state_set` |
| 1215, 1216 | `exist` | used-marker set at declaration, not by the redirect hook; missing citation | move the mark into the hook; citation |
| 1286 | `hey_kids` | the exactly-one-side check is absent | add the count test |
| 1361 | `fire_bird` | 1.5× stacking is unruled | **G02** composition; `multiply_pay_factor(1500, ThisMove)` once ruled |
| 1483 | `ringing_bloom` | 「房屋数」 read from the counted (virtual) value | read `ctx::houses_of` (standing), not `rent_houses_of` |

---

## 6. Save, record, replay

- New `MoveCtx`, `TurnCtx`, `World` and `PlanScope` fields use
  `#[serde(default)]`. `MoveCtx` has no struct-level default, so every new
  field must carry the attribute or old plans fail to parse.
- An empty stack, scope and window list are exactly an old save: no frame to
  restore, staging behaves as before. An empty stack has no `plan()` fallback
  problem (`turn.plan`), no `move_open`, and no roll-window replacement.
- **SAVE_VERSION stays 13** (G02 landed it; this group adds no further bump).
  The new save fields are additive and defaulted.
- The record stores actions, prompts and engine stamps, not plan internals; the
  frame is reconstructed by replaying commands. `TileData.grid_cell` is
  `skip_serializing_if` and absent from every existing record; a board row
  written with a grid cell replays on the archived bundle of the build that wrote
  it. **RECORD_VERSION stays 4.**
- Host pauses serialize the stack and scope (§4.9), so a pause inside a move
  resumes with the same frame. A save taken mid-move round-trips the frame; the
  walk's `Move` and the plan frame agree on `extra_steps` because both are saved
  and the step syncs from the frame.
- Old records replay on their archived engine bundle and ABI. New records
  embed the ABI-108 engine stamp.
- `next_turn` drops the stack and scope, empties `move_roll_windows` and clears
  `move_open`; a turn never starts with a live frame or an open window.

## 7. Versioning

`crates/card-sdk/src/abi.rs`:

```rust
// wire-breaking: the exact host/module version check rejects stale wasm.
pub const ABI_VERSION: i32 = 108;   // was 107 (fifth group in the landing order)
/// v108: **command-scoped move plans and walk control**. The turn plan is a
///    staging buffer; every move runs on a frame owned by its command
///    (`World::begin_plan` / `end_plan`) and is dropped when the command ends.
///    New imports: `ruleset_set_forced` (378), `ruleset_set_main_override` (379),
///    `ruleset_void_walk` (380), `ruleset_clear_steps` (381),
///    `ruleset_add_extra_steps` (382), `ruleset_move_planned_teleport` (383),
///    `ruleset_move_pay_factor` (384), `ruleset_move_rent_factor` (385),
///    `ruleset_move_effective_steps` (386), `ruleset_move_forced` (387),
///    `ruleset_trig_move_rolled` (388),
///    `ruleset_trig_move_effective_steps` (389),
///    `ruleset_nearest_tile_on_axis` (390), `ruleset_farthest_tile_on_axis` (391).
///    `MoveCtx.forced` is the `Forced` flag -- a forced move of the turn player
///    is still the main move when `main_override` is set -- no longer "never the
///    main move". `set_parity` canonicalizes `2` to `0` (even). The trigger move
///    payload gains `rolled` and `effective`, surfaced as `move.rolled` /
///    `move.effective`. SAVE_VERSION / RECORD_VERSION unchanged.
```

The bump is a wire break on two counts: a new import set (the module-load
equality refuses old wasm) and two new trigger fields in the postcard payload.
Every card wasm and the Kani host rebuild together.

## 8. Tests and Kani obligations

### 8.1 Engine (game-core)

`crates/game-core/src/engine/play/tests.rs` and `move_ctx.rs`:

| name | asserts |
|---|---|
| `g04_side_move_restores_staging` | a side command's `steps`/`reverse`/`pay_factor` are gone at command end |
| `g04_refused_main_keeps_staging` | a refused card main move leaves a staged teleport/factor intact |
| `g04_main_consumes_staging` | a Main move latches the slot and clears staging |
| `g04_forced_never_refused` | a forced move of the turn player runs with the slot spent and does not consume it |
| `g04_main_override_consumes_when_free` | a forced `main_override` move latches a free slot |
| `g04_void_walk_replaces_the_walk` | the walk's pass events do not raise; the teleport tail does |
| `g04_replacement_settles_before_the_body_resumes` | the teleport's settle and tail precede the body's `pay` |
| `g04_gate_refusal_restores_the_walk` | a refused replacement walks the roll with `kind = Walk` and no teleport |
| `g04_effective_steps_prefers_next_steps` | `next_steps` is visible in the `diceRollResult` window and used by the walk |
| `g04_from_is_stamped_before_the_roll` | `trigger::move_from()` is the planned start inside the window |
| `g04_extra_steps_add_is_relative` | two `add_extra_steps` calls compose; the walk lengthens mid-step |
| `g04_nested_frames_restore_in_order` | a body raised inside a body does not overwrite the outer snapshot |

### 8.2 Card (rules/tests/tests)

One per slug, in the file named beside it. Cross-command flows that no single
card owns (Play-time staging vs a side move in the same activation, the
replacement's body resuming after the settle, a window write on a
`moveBefore` counter) go in a new `g04_move.rs`.

| # | slug | test | file |
|---|---|---|---|
| 1 | want_to_grab | `g04_want_to_grab_nearest_axis_teleport` | `crychic.rs` |
| 2 | pure-wings-stale-steps | `g04_pure_wings_rolls_from_teleport_destination` | `morfonica.rs` |
| 3 | pure-wings-stale-direction | `g04_pure_wings_ignores_a_prior_reverse` | `morfonica.rs` |
| 4 | pure-wings-pay-factor-leak | `g04_pure_wings_settles_at_full_price_after_saki_move` | `morfonica.rs` |
| 5 | charity-dice-move-gate | `g04_charity_gate_ignores_roll_less_main_moves_and_takes_forced_dice_moves` | `hhw.rs` |
| 6 | charity-extra-steps-sync | `g04_charity_plus_two_extends_the_earning_walk` | `hhw.rs` |
| 7 | sakiko-followers-card-move | `g04_sakiko_followers_replay_on_a_card_main_move` | `crychic.rs` |
| 8 | detour gate | `g04_detour_counter_opens_after_the_main_move_latched` | `ag.rs` |
| 9 | detour-parity | `g04_detour_parity_lands_on_odd_or_even_tiles` | `ag.rs` |
| 10 | saki leak | `g04_saki_move_does_not_shape_the_caster_main_move` | `mujica.rs` |
| 11 | misaki-teleport | `g04_misaki_teleport_is_main_move_voids_the_rolled_walk` | `hhw.rs` |
| 12 | misaki-planned-start | `g04_misaki_span_uses_the_planned_start` | `hhw.rs` |
| 13 | misaki-effective-steps | `g04_misaki_span_uses_next_steps` | `hhw.rs` |
| 14 | dest-forced-hop | `g04_fate_hop_does_not_consume_or_require_the_main_move` | `crychic.rs` |
| 15 | plan-set-forced | `g04_forced_walk_of_the_turn_player_is_main_only_with_main_override` | `mujica.rs` |
| 16 | dg-plan-reset | `g04_doll_garden_victims_do_not_leak_shape` | `mujica.rs` |
| 17 | lisa-bond | `g04_lisa_bond_opens_after_a_nested_skill_teleport` | `roselia.rs` |

Positive and negative pairs where the rule has a choice (13: with and without
`next_steps`; 15: self vs other; 9: odd vs even vs reverse; 11: gate allowed
vs gate refused).

### 8.3 Kani obligations

Harnesses in `rules/kani/src/proofs/move_plan.rs` over the production
functions in `game-kinds`, each an iff proof with a cover and a mutant:

- `move_plan::command_fields_are_never_staging` — exhaustive over `PlanField`:
  `is_staging` is false for every `Command` field.
- `move_plan::main_frame_keeps_turn_fields` — a Main frame seeded from staging
  carries `base`/`dice`/`min_roll`/`signed`.
- `move_plan::side_seal_drops_command_fields` — after a side command's seal,
  staging no longer carries `steps`/`reverse`/`parity`/`stop_at`.
- `move_plan::side_seal_keeps_next_main_fields` — staging
  `teleport_to`/`start`/`pay_factor`/`followers` survive a side command.
- `move_plan::main_seal_consumes_next_main_fields` — a Main move leaves a
  default staging plan.
- `move_plan::parity_step_is_nearest_qualifying_tile` — `next_parity_tile` is
  the smallest `d ≥ 1` with the requested parity.
- `move_plan::parity_step_never_returns_start` — for `n > 1` the step moves.
- `move_plan::parity_step_stays_in_range` — result in `0..n`.
- `move_plan::effective_steps_order` — `steps` > `next_steps` > `roll`.
- `move_plan::nearest_tile_on_axis_is_on_ray` / `nearest_tile_on_axis_is_nearest` — same
  column/row, minimal distance, never the start.
- `move_plan::replacement_taken_exactly` — `replacement_taken` iff
  `window == Some(owner) && main_override && void_walk && teleport`.
- `move_plan::walk_runs_exactly` — `walk_runs` iff no teleport target, not
  cancelled and not `void_walk` (so a replacement and a pre-roll teleport both
  skip the walk exactly once).

Per item: 1 → axis proofs; 5 → `MoveSnap.rolled` through `arm_card_move`;
9 → the parity proofs; 11 → the replacement/walk proofs plus the misaki card
proof; 13 → `effective_steps_order`; 14/15 → `arm_card_move`; 16 → the seal
proofs.

`arm_card_move` proof: `refused iff is_main_move && main_moved && !replacement_taken`.

## 9. Alternatives

**A1 — Field lifetimes on the sticky plan, no frames.** Classify each
`MoveCtx` field as Turn / NextMainMove / Command and reset Command fields after
every move. Simple to state, but `pay_factor` is both Command (saki) and
NextMainMove (sakiko), `teleport_to` is both NextMainMove (kanon) and Command (misaki),
and the split needs exceptions per writer. The frame encodes the intent once;
the field list does not. This is the shipped patch in miniature: it is what
`steps_fresh` and kin are, and it already misses nine fields.

**A2 — Save/restore getters in the card.** Add `plan_save() -> PlanHandle` /
`plan_restore(handle)` and make every body restore its shape. No engine
scoping. Gives up the entire class fix (a forgetful card re-leaks) and every
card pays boilerplate; it also cannot express "the staging teleport belongs to
the next main" when a side move intervenes.

**A3 — Fresh `MoveCtx` for every `run_planned_move`, ignore staging.** Kills
leaks outright but loses Play-time shaping (`change_world` dice, detour base
dice, kanon teleport) unless a second, explicit next-main channel is added, and
every body that pre-shapes its own move is rewritten to pass a full move.

**A4 — Defer the window teleport to the roll site.** The body only records the
teleport; the roll site runs it after the last roll window. One frame and one
tail, but the body resumes before the teleport settles, so misaki's
「随后…支付」 pays before the destination resolves and a settle that reads the
board sees pre-teleport positions. A bare window write with no body waiting on
it keeps exactly this path (§4.3, second branch); a card that acts after the
teleport uses the immediate replacement, at the cost of the nested call rules in
§4.4.

## 10. Open questions

1. **Turn-start gate for (1).** If the [停留] layer this card just granted is
   still present at the owner's next turn start, the move gate is already shut
   (`play.rs:772-793` sets `skip_move`) and `run_planned_move`'s main move is
   refused. Does (1)'s teleport still happen? The book does not say. The
   implementation lets the gate answer, like every other main move
   (`docs/STATUS-HOSTS.md` §3.4).
2. **Parity pass events.** Does a parity walk "pass" the skipped tiles? We
   choose no (only landings pass). A reading of 「仅在…上进行」 as continuous
   movement would raise `pass` on every ring tile between landings.
3. **Parity vs `stop_at`/`settle_tile`.** A stop tile of the other parity is
   unreachable while parity is set. Should the walk stop there anyway?
4. **`next_steps` on a card main move.** Today only `main_move` consumes it.
   This design consumes it for every Main frame; confirm that a card-driven
   main move (「视为主要移动」) counts for a `next_steps` buff.
5. **Follower replay on a teleport main move.** Followers carry `kind` and
   `teleport_to`, so they teleport. Confirm this is intended (sakiko plus a
   teleport main is plausible).
6. **`main_override` outside a roll window.** A forced move of the turn player marked
   `main_override` outside any window still takes the slot check: it runs when the
   slot is free and is refused when spent. Is that the right reading of
   「代替主要移动」, or should a forced `main_override` move always run?
7. **Grid source.** The board file carries `grid_cell`; the client's `FOLD` stays
   its renderer. Should the client read `grid_cell` from the data instead of keeping
   a second copy? (No rule depends on the answer.)
8. **Detour's `0` vs Himari's `2`.** `set_parity` canonicalizes both, but a
   card-side migration to `0` for even is cheap; do it in the batch.
9. **`BySkill` on a nested tail.** `teleport_tail_only` inherits `by_skill`
   only when the host set it. A card body that itself calls `teleport_to` inside
   a settle without the host's skill prefix would not open lisa's window; is
   that the right scope for 「使用技能进行传送」?
10. **Forced move of a non-turn player and `set_forced`.** Both set the flag on
    a plan that is already `SideMove`; confirm the card list has no case that also
    needs `main_override` for an off-turn player.

## 11. Reconciliation with other groups

- **G02 (before).** Owns the factor scopes, `multiply_pay_factor`, `pay_factor`
  and the settle half. This group feeds the plan half: `move_pay_factor()`, the
  frame default, the `NextMove` fold and the tail clear. `move_open` is now
  「a frame is live」.
- **G03 (after).** Owns the roll ladder, `roll_source`, `dg-move-dice`,
  `misaki-fire-roll-signal` and the Result raise. This group owns the frame,
  the window snapshot/fold, `void_walk` and item 8's fold. G03's preview roll
  runs outside any frame.
- **G05/G13.** The press surface and invoke entry use the same command scope;
  a press body that drives a move is an activation like a Play body.
- **G07.** The target window reads `trigger::move_from()` /
  `move_effective_steps()` for range and span.
- **G12.** `cant_move_code_for_activation` reads the blocked predicate this
  group's `main_override`/`forced` split leaves orthogonal; the immunity gate keeps
  its ordering against the move activation.
- **G10.** Settle facts are orthogonal; the replacement's one settle raises the
  ordinary stages under G10's outcome flags.

## 12. README and doc edits

- README band table: G04 stays 107→108, no kinds, SAVE stays 13; the row's Op
  cell is narrowed from 378–397 to **378–391**, leaving 392–397 free for a
  later landing-order change. The coupling note `G04 ↔ G12` already exists.
- `docs/EVENTS.md`: the host `event:` stamp remains, but the file currently
  says nothing about forcedness (`:16-27` only quotes 「效果立刻生效」). Add the
  paragraph the engine doc-comment already assumes (`play.rs:893-898`): an event
  move is forced, never the main move, and a guest may now set the same flag.
- `docs/rulebook/SETTLE-STAGES.md` §7: no stage order change; note the nested
  `MoveAfter` tail and the parity landing rule.
- `docs/CARDS.md`: the roll stage list is G03's; add `move.rolled` /
  `move.effective` beside the `move.*` condition spellings (`:176`) and in the
  payload table.
