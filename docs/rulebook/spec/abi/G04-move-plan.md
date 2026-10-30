# ABI design G04 — Move plan and walk control

**Status: draft for user review. Design only — no source edits, no commit.**
Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`. Fits **on top of ABI v104** (turn snapshot
restore). It does not redo G03's roll pipeline; `dg-move-dice` and
`misaki-fire-roll-signal` stay with G03, and this document consumes their
surface where the move plan is concerned.

Closed slugs and their review rows:

| # | slug | sev | rows |
|---|------|-----|------|
| 1 | `want_to_grab-clause3-teleport` | medium | 107, 1003 |
| 2 | `pure-wings-stale-steps` | high | 288 |
| 3 | `pure-wings-stale-direction` | medium | 290 |
| 4 | `pure-wings-pay-factor-leak` | medium | 1038 |
| 5 | `charity-dice-move-gate` | medium | 209, 1215, 1216 |
| 6 | `charity-extra-steps-sync` | medium | 203, 1025 |
| 7 | `sakiko-followers-card-move` | medium | 129 |
| 8 | `detour-plan-writes-lost` (merged `detour-false-move-blocked-gate`) | high | 225 |
| 9 | `detour-parity-unused` | medium | 1101 |
| 10 | `saki-move-leaks-plan-shape` | high | 338, 339, 963, 964, 340, 1064 |
| 11 | `misaki-teleport` | high | 291, 293, 295, 1183, 1184 |
| 12 | `misaki-planned-start` | medium | 1007 |
| 13 | `misaki-effective-steps` | medium | 1182 |
| 14 | `fate-forced-hop` | high | 200 |
| 15 | `plan-set-forced` | medium | 431, 1286 |
| 16 | `dg-plan-reset` | medium | 1095 |
| 17 | `lisa-bond-nested-teleport-window` | medium | 693 |

Ground truth: `docs/rulebook/cards.json` and the live skill sheet. Every
fragment below was re-read from `cards.json` on 2026-10-28 against the card id
named beside it.

The consolidated `plan-shape-scope` also contains four pay-factor slugs
(`sakiko-pay-factor-composition`, `repaint-pay-factor`, `taki-pay-factor-leak`,
`rana-pay-factor-multiply`). Their factor semantics are designed in **G02**
§2.3 / §4.5 (the `PayFactorScope` API); this document supplies the plan half
the composition reads and the plan-reset half of the leak.

One item is **demoted to A** (§1.2 item 8): the mechanism exists and only
the guard and its test change. The rest need this design.

---

## 1. Problem

### 1.1 Shared root causes

The movement plan is a single `TurnCtx::plan` (`world.rs:143-150`) that is
created once per turn and reset only at `next_turn`. Every move — the main
move, a card's own move, a forced move, a nested move inside a settle — reads
and writes that one object. The tree works around this:

- `card_move` swaps the plan out only when `move_depth > 0`
  (`play.rs:829-835`), so nested moves restore, but a *top-level* card move
  leaves its shape on the turn plan.
- The walk re-reads only a hand-picked subset of the plan each step
  (`resolve`, `stop_at`: `play.rs:1811-1817`), not `parity`, `steps`,
  `extra_steps`, or `reverse`.
- One-shot fields (`teleport_to`, `start`, `by_skill`) are cleared by hand at
  every exit (`play.rs:804-806, 938-940`), and a refusal path that forgets one
  shapes an unrelated later move.
- `forced` exists on `MoveCtx` (`move_ctx.rs:89-92`) but only the
  `event:*` host writes it (`wasm_rules.rs:3794`); a guest cannot.
- The roll window clones the move and pulls back only `roll`/`resolve`
  (`play.rs:1349-1478`), so plan writes made in `diceRollResult` (detour,
  misaki) are lost.
- The walk derives its next tile from the direction alone
  (`play.rs:1753`); `Move.parity` is written by Himari but never read.
- The follower replay lives only in `main_move` (`play.rs:1516-1536`).
- A nested teleport is a bare position write with no tail
  (`play.rs:765-774`).
- Board geometry is private to one card (`happy_lucky.rs:37-67`).

These are not 17 independent bugs. They are six defects in how a move plan is
owned, read, sealed and handed back:

- **R1 — no command ownership.** Shape leaks between moves (items 2, 3, 4, 10,
  16).
- **R2 — no intent split between "this move" and "the next main move".**
  Sticky writes cannot be classified; every "clear it afterwards" is a manual
  patch (items 1, 3, 4, 10, 16).
- **R3 — forcedness and mainness are conflated.** `forced` means "never the
  main move", while the book's forced walks of the turn player are that
  player's main move (items 11, 14, 15).
- **R4 — the roll window cannot rewrite the move.** Only two fields survive
  the raise (items 9, 11, 12, 13).
- **R5 — the walk ignores plan dimensions.** Parity is dead (item 9);
  `extra_steps` is not synced (item 6); the movement origin is stamped too
  late (item 12); provenance `rolled` does not exist (item 5).
- **R6 — the move tail is not shared.** Followers replay only on the
  engine main move (item 7); nested teleports raise no tail (item 17).

### 1.2 Per item

Each entry quotes the fragment verbatim, cites the row ids, states what the
tree does today, what the book requires, and the root fix. "Demote to A"
means: the capability already exists; only the guard/test changes.

**1. `want_to_grab-clause3-teleport` — CRYCHIC:想要抓住..., rows 107, 1003**
(`rules/cards/card-crychic/src/want_to_grab.rs`)

> 「你的下回合开始前，若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。」

The `TurnStart` hook always retires the card and never branches on the
`PASSER == NEVER` latch (`want_to_grab.rs:118-127`). The only grid ray search
in the tree is the private `CELLS` + `farthest` in
`happy_lucky.rs:37-67`, fixed to 60 tiles; `TileData` has no coordinates
(`data.rs:83-93`). Expected: at the owner's turn start with nobody passed,
offer the nearest tile on each of the four axes, teleport-and-settle there as
the main move, and retire the card. **Root fix:** board geometry in data plus
nearest/farthest axis queries (§3.5), then the card branch.

**2. `pure-wings-stale-steps` — Mor:纯真振翅, row 288**
(`rules/cards/card-morfonica/src/pure_wings.rs`)

> 「立刻进行移动掷骰」

The card clears the teleport destination and calls `card_move`, expecting a
fresh roll. `card_move_seeded` rolls only when `plan.steps.is_none()`
(`play.rs:876`), and `set_steps` clamps to `Some(0)` on `-1` so there is no
way back to `None` (`move_ctx.rs:277-282`). An earlier card's step count
silently turns the roll into a fixed walk. Expected: a dice roll from the
teleport destination. **Root fix:** `plan::clear_steps` and a frame whose
default is the post-teleport state (R1 + R2).

*Fixed (2026-10-30)*: a per-move value an earlier move left is dropped at the
next move's entry (`drop_stale_plan_shape`: `steps`, `stop_at`, `reverse`,
`resolve`, and the payment factors), so the roll branch runs again. A
`plan::set_*` write raises a per-field fresh mark, which the entry consumes
and which keeps a value written for *this* move; an activation body that drove
a move also drops its own trailing replay writes when it ends (an activation
that drove none is shaping a later move and keeps them). The frame rework
below is still the fuller answer (§4.3), this is the small one.

**3. `pure-wings-stale-direction` — Mor:纯真振翅, row 290**

> 「传送到移动方向20格后」

`ctx::plan::dir()` reads `world.turn.plan` (`ctx.rs` plan module;
`ops.rs:223-229`), which a prior side move shares. `saki_move` leaves
`reverse = true` behind, so the jump goes the wrong way. Expected: the moving
player's own direction for *this* move. **Root fix:** per-command frame with
the plan's default direction; no card change after R1.

**4. `pure-wings-pay-factor-leak` — Mor:纯真振翅, row 1038**

> 「立刻进行移动掷骰」

The landing settle reads `turn.plan.pay_factor` through
`scale_settle_payment`; a prior 祥，移动 left 500, so rent is halved with no
discount on this card. No getter exists to detect the inherited value.
Expected: the default factor for this walk's settle. **Root fix:** frame
default + `plan_pay_factor`/`plan_rent_factor` getters (R1).

**5. `charity-dice-move-gate` — HHW:爱心义演, rows 209, 1215, 1216**
(`rules/cards/card-hhw/src/charity_show.rs`)

> 「你若进行掷骰移动」

The `PassTile` pre gates on `move.main` (`charity_show.rs:48`). A roll-less
card main walk (尽力后的收获, 迷宫标) fires it, and a forced dice move (an
event walk) does not; `TriggerMove` has no dice provenance. Expected: gate on
"a die was rolled for this walk". **Root fix:** a `rolled` discriminator on the
move payload and CEL (`move.rolled`), set from how the walk got its steps
(R5).

**6. `charity-extra-steps-sync` — HHW:爱心义演, rows 203, 1025**

> 「使你的总移动数+2」

`ctx::plan::set_extra_steps` writes only `TurnCtx::plan.extra_steps`; the
running walk is a separate `Move` and never re-reads that field, so the +2 is
a no-op on the move that earned it. The card keeps a running prop and
overwrites `uika_imprisoned`'s value because `set_extra_steps` assigns.
Expected: +2 extends the walk it is earned on and composes. **Root fix:** the
walk syncs `extra_steps` from the plan each step; `plan::add_extra_steps`
composes (R5).

**7. `sakiko-followers-card-move` — CRYCHIC:（祥子）带领着大家, row 129**
(`rules/cards/card-crychic/src/sakiko_lead.rs`)

> 「使你的下次主要移动结果对那些玩家一起执行」

`plan.followers` is taken and replayed only in `main_move`
(`play.rs:1516-1536`). A main move produced by a card (`card_move`,
「视为你的主要移动」) never replays them (`sakiko_lead.rs:62-64`). Expected:
the recorded followers walk whenever the next main move runs, however it runs.
**Root fix:** the replay moves into the shared move tail used by every
`as_main` move (R6).

**8. `detour-false-move-blocked-gate` — AG:回家的路上绕个道, row 225 —
DEMOTE TO A**
(`rules/cards/card-ag/src/detour.rs`)

> 「(1)此卡可作为反击使用」

The counter guard is `ctx::cant_move_code(player_id) != 3`
(`detour.rs:71`). `cant_move_code` already exists and returns 2 when the main
move is spent, so "already moved" is accepted and only the [停留] gate (3) is
refused. No ABI change. **Action:** add the regression test
`g04_detour_counter_opens_after_the_main_move_latched` and remove the TODO.
The abnormal-gate opt-out (STA-02, `rules.txt` 51) is a separate backlog item.

**9. `detour-parity-unused` — AG:回家的路上绕个道 / Himari (2), row 1101**

> 「视为使用一次初始Afterglow角色的（2）效果」

`detour.rs:214-226` writes `plan::set_parity(1/0)`, and
`rules/skills/skill-personas/src/himari_step.rs:74-75` writes 1/2; the walk
loop derives `next` from `m.dir()` alone (`play.rs:1753`) and never reads
`m.parity`. The book's 「仅在单/双数格上进行」 lands only on the chosen
parity. Expected: each step goes to the nearest tile of the requested parity.
**Root fix:** a parity step function in the walk; canonical parity `-1/0/1`
(`2` normalized to `0`) (R5).

**10. `saki-move-leaks-plan-shape` — Mujica:祥，移动, rows 338, 339, 963, 964, 340, 1064**
(`rules/cards/card-mujica/src/saki_move.rs`)

> 「强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半」

`set_steps(3)` / `set_reverse` / `set_resolve` / `set_pay_factor(500)` write the
shared turn plan (`saki_move.rs:61-66`); `card_move` seeds it and never
restores it (`play.rs:853-860`). Targeting another player makes the caster's
own later main move walk 3 tiles in the card's direction, ignoring the dice,
and settle at half price. Expected: the shape is scoped to the forced walk.
**Root fix:** a side frame whose shape is restored when the command ends (R1 +
R2).

**11. `misaki-teleport` — HHW:（美咲）, rows 291, 293, 295, 1183, 1184**
(`rules/cards/card-hhw/src/misaki_card.rs`)

> 「消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动）」

`trigger::set_cancelled()` only negates the trigger link; the roll site reads
`t.value` and walks the face. `card_move` then takes the `main_moved` early
return, logs `main_move_used` and clears `teleport_to` (`play.rs:795-808`), so
there is no teleport at all. Expected: the rolled walk is voided and replaced
by the teleport, settling there as the same main move. **Root fix:** a
`void_walk` handoff consumed at the roll site plus `main_as` (R3 + R4).

**12. `misaki-planned-start` — HHW:（美咲）, row 1007**

> 「位于你的移动起点与预定移动终点之间」

`between()` uses `ctx::player_pos(player_id)` (`misaki_card.rs:52`). A
`set_start` override (your_light / detour / …) only lands when the walk
starts, after the `diceRollResult` window, and the trigger carries no start.
Expected: the move's actual start. **Root fix:** stamp `Move.from` from the
planned start before the roll windows and let `between()` read
`trigger::move_from()` (R4).

**13. `misaki-effective-steps` — HHW:（美咲）, row 1182**

> 「预定移动终点」

The span uses the raw face (`|roll|`, `misaki_card.rs:47`), while `next_steps`
(普通与理所当然) rewrites the steps after the window
(`play.rs:910-912`). Candidates are wrongly included or missed. Expected: the
endpoint the move will actually take. **Root fix:** carry `next_steps` into
the plan frame before the windows and expose `move.effective`
(`trigger::move_effective_steps`) (R4).

**14. `fate-forced-hop` — CRYCHIC:一起演奏音乐的命运共同体, row 200**
(`rules/cards/card-crychic/src/fate_together.rs`)

> 「每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。」

The hop calls `card_move` (`fate_together.rs:111-125`), which treats it as the
main move: refused once the main move is spent, consumed otherwise. Its bool is
the abnormal-gate answer, not whether the move ran, so a spent main move still
reads as a hit. Expected: an effect move that is never the main move and never
refused by the slot. **Root fix:** `plan::set_forced` and `card_move` returning
"the move ran" (R3).

**15. `plan-set-forced` — Mujica:无法将视线移开 / Mujica:人偶的箱庭, rows 431, 1286**
(`rules/cards/card-mujica/src/cant_look_away.rs`,
`rules/cards/card-mujica/src/doll_garden.rs`)

> 「使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算]」

> 「你强制移动其他玩家本次移动掷骰数之和。」

C# builds `MoveCtx { …, Forced = true }`; `MoveCtx.forced` exists but only the
event host writes it, so the flag is dropped and a forced walk of the turn
player takes (or is refused by) the main-move path. Expected: a guest-forced
move that runs regardless of the slot, and — for the C# `MainMoveAs` cards —
still the main move when the walked player is the turn player. **Root fix:**
`plan::set_forced` + `plan::set_main_as` (R3).

**16. `dg-plan-reset` — Mujica:人偶的箱庭, row 1095**

> 「你强制移动其他玩家本次移动掷骰数之和。」

Every victim walks with the shared plan (`doll_garden.rs:86-94`), so a stale
`stop_at`, `pay_factor`, or `reverse` from an earlier walk leaks in; each
victim's `resolve = false` also leaks to the next victim and the summoner's
sum move. Expected: each forced walk starts clean. **Root fix:** every card
move gets its own frame; the prior command's shape is restored (R1).

**17. `lisa-bond-nested-teleport-window` — R:必然的联系（莉莎）, row 693**
(`rules/cards/card-roselia/src/lisa_bond.rs`)

> 「当你使用技能进行传送后」

`card_teleport` at `move_depth > 0` is a bare position write, no tail
(`play.rs:765-774`); `ChainKind::MoveAfter` never fires for a teleport nested
in a settle, so the card's window never opens. Expected: the window opens
after any skill teleport, nested ones included. **Root fix:** a tail-only
nested teleport for skill-sourced jumps (R6).

---

## 2. Capability as an invariant

**I1 — Command ownership.** Every move runs on a plan frame owned by the
command (or engine step) that started it (`World::begin_plan` / `end_plan`).
The frame is seeded from the turn plan at begin and never writes it directly.

**I2 — One-shot command shape.** Nothing a move writes outlives it: a side
move's shape is restored when the move ends. A command that stages for the next
main keeps its writes, because a write outside a move (or after one returned)
lands on the staging plan, not on the move frame.

**I3 — Forcedness is orthogonal to mainness.** `forced` marks an effect-forced
move: it runs even when the main slot is latched and does not itself consume
the slot. `main_as` says "when the walked player is the turn player, this
forced move is that player's main move" (C# `MainMoveAs`). Therefore

```
as_main = is_turn && (!forced || main_as)
```

Only `as_main` moves latch `main_moved`, drive the stage machine, and replay
followers.

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
`main_as` voids the pending walk (`void_walk`) and runs immediately, from
inside the window, as the same move with one tail; the roll site only skips
its own walk.

**I8 — The tail is shared.** Every completed move raises
`moveAfter`/`moveResolved`; every `as_main` move replays its followers in the
recorded order after the mover settles.

**I9 — Geometry is data.** Axis nearest/farthest read board coordinates from
`GameData`; no card carries a private board map.

---

## 3. Surface

### 3.1 Layering recap

Each new import is added at every layer, or the build fails:

1. `crates/card-sdk/src/ctx.rs` — public wrapper (unit or typed).
2. `crates/card-sdk/src/ctx/sys.rs` — `extern` under the
   `wasm32`/`native-rules` attribute pair, link name `ruleset_*`.
3. `crates/card-sdk/src/ctx/sys_kani.rs` — `kani_host::call_*` stub.
4. `crates/card-sdk/src/kani_host.rs` — `Op` variants (README band 351-370; this group uses 351-364).
5. `rules/kani/src/host.rs` — arm (stub or model) and every affected card
   proof's `host_subset!`.
6. `crates/game-rules/src/hostfns.rs` — `HostCtx` implementation.
7. `crates/game-rules/src/native_shims.rs` — trait method + `op_*` +
   `#[no_mangle] ruleset_*`.
8. `crates/game-rules/src/host.rs` — `func_wrap`.
9. `crates/game-rules/src/world.rs` — default `HostCtx` implementation.
10. `crates/game-rules/src/wasm_rules.rs` — live `HostCtx` implementation.

Nothing in this surface makes a player decision. `ask_tile` stays in the card.

### 3.2 Plan scopes (new, engine-side)

`crates/game-core/src/engine/move_ctx.rs`:

```rust
// `PlanKind` is re-exported from `game_kinds::move_plan`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanFrame {
    pub plan: MoveCtx,
    /// The outermost command's `staging_before`; the restore target of a
    /// side or refused frame.
    pub saved: MoveCtx,
    pub kind: PlanKind,
    pub owner: usize,
    /// Did the move actually start?
    pub ran: bool,
}

impl MoveCtx {
    /// Steps the move will take: `steps`, else the main-move `next_steps`,
    /// else the roll. The roll window's `effective`.
    pub fn effective_steps(&self) -> i32;
    /// Is the walk's length a rolled die?
    pub fn is_rolled(&self) -> bool;
    /// A side seed: copy every field `current` changed from `before` (the
    /// command's shaping for this move), then clear `next_steps`. Iterates
    /// `PlanField`, so a new field cannot be forgotten.
    pub fn staging_seed(before: &MoveCtx, current: &MoveCtx) -> MoveCtx;
}
```

`crates/game-core/src/engine/world.rs` (`TurnCtx`):

```rust
#[serde(default)] pub plan_stack: Vec<PlanFrame>,
#[serde(default)] pub plan_scope: PlanScope,

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanScope {
    /// Driven guest-body nesting (a hook inside a body).
    pub command_depth: u32,
    /// The staging plan when the outermost body entered.
    pub staging_before: Option<MoveCtx>,
    /// The plan target when the body entered (frame or staging).
    pub plan_before: Option<MoveCtx>,
    /// The frame contents when the current roll window opened.
    /// One snapshot per open window, innermost last.
    #[serde(default)] pub window_before: Vec<MoveCtx>,
}
```

`World` also gains (saved like the scope):

```rust
/// The rollers whose move dice windows are open, innermost last
/// (main-as-vs-walk handoff).
#[serde(default)] pub move_roll_windows: Vec<usize>,
```

`MoveCtx` gains, all `#[serde(default)]`:

```rust
pub next_steps: Option<i32>, // the main move's stored override (frame seed)
pub main_as: bool,           // C# MainMoveAs: forced self walk = main
pub void_walk: bool,         // this teleport replaces the pending walk
pub rolled: bool,            // a die produced this walk's length
```

The pure decision rules live in a heap-free shared module,
`crates/game-kinds/src/move_plan.rs`, so the engine and the Kani proofs run the
same code:

```rust
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanKind { #[default] Main, Side }
/// Every field a plan can shape. The seal/restore match is exhaustive over it.
pub enum PlanField { Steps, Reverse, Parity, Kind, TeleportTo, Start, StartWhy,
                     StopAt, SettleTile, SettleRedirect, Resolve, PayFactor,
                     RentFactor, Followers,
                     ExtraSteps, NextSteps, MinRoll, Signed, Base, Dice,
                     MoreSteps, NoBuy, CanBuild, BuildAnywhere,
                     SettleAsAgent, Tags, BySkill, Forced, MainAs, VoidWalk,
                     Rolled, Why, Roller, Cancelled }
pub enum PlanLifetime { Turn, NextMain, Command }
pub fn lifetime_of(f: PlanField) -> PlanLifetime;
/// `Turn` or `NextMain` -- a staging field, not one move's shape.
pub fn is_staging(f: PlanField) -> bool;
pub fn as_main(is_turn: bool, forced: bool, main_as: bool) -> bool;
/// True iff a `card_move` call replaces a walk instead of starting a move.
pub fn replacement_taken(window: Option<usize>, owner: usize, main_as: bool,
                         void_walk: bool, teleport: bool) -> bool;
/// True iff the roll site still walks.
pub fn walk_runs(teleport: bool, cancelled: bool, void_walk: bool) -> bool;
pub fn effective_steps(steps: Option<i32>, next: Option<i32>, roll: i32) -> i32;
pub fn next_parity_tile(cur: i32, dir: i32, parity: i32, n: i32) -> i32;
pub fn axis_nearest(cells: &[Option<[i32; 2]>], tile: usize, axis: i32) -> Option<usize>;
pub fn axis_farthest(cells: &[Option<[i32; 2]>], tile: usize, axis: i32) -> Option<usize>;
```

`is_staging` is the machine-checked form of the lifetime rule: a staging
write of a `Turn` or `NextMain` field survives a side move untouched, while an
`as_main` move consumes it (the staging plan becomes `Default`); `Command`
fields belong to whichever move wrote them and never outlive it.

`World` accessors, replacing the 101 direct `turn.plan` accesses:

```rust
pub fn plan(&self) -> &MoveCtx;              // top frame, else turn.plan
pub fn plan_mut(&mut self) -> &mut MoveCtx;  // top frame, else turn.plan
pub fn begin_plan(&mut self, owner: usize, as_main: bool, seed: MoveCtx);
pub fn end_plan(&mut self, ran: bool);
pub fn enter_plan_command(&mut self);
pub fn leave_plan_command(&mut self);
```

`move_depth` stays as it is: it counts nested movement for the teleport-tail
rule (§4.6), not plan scoping. The frame depth below is `plan_stack.len()` (a
Play body is depth 0 until it starts a move); the body nesting is
`plan_scope.command_depth`.

`begin_plan` seeds the frame:

- `as_main`, depth 0 — `seed` is the guest-supplied plan (the whole staging
  state, already including the body's writes). `saved` is the command's
  `staging_before`; staging becomes `Default` only when the move starts.
- side, depth 0 — `seed` is `MoveCtx::staging_seed(plan_before, plan target)`:
  only what the command changed, with `next_steps` cleared (that override
  belongs to the next main move and stays in staging). `saved` is
  `staging_before`; staging is restored to it on pop, so the shape that seeded
  the move dies with it. With no command scope (`plan_before` is `None`, an
  inline host rule) the seed is the whole staging and `saved` is `staging`.
- any frame at depth > 0 — side; `saved` is unused (staging was not touched).
  A main frame at depth > 0 (a roll-window replacement or a nested turn-player
  move from a hook) behaves as a side frame; the enclosing move owns latching,
  followers and the summary.

`end_plan` pops. The restore table:

| frame | condition | `turn.plan` after pop |
|---|---|---|
| side, depth 0 | — | `saved` |
| main, depth 0 | ran | `Default` |
| main, depth 0 | refused | `saved` |
| any, depth > 0 | — | unchanged (the nested frame is discarded) |

Restoring at the *pop* (not at command end) is what lets one command contain
several moves (doll_garden's victims): each side move drops the writes it was
shaped with, and a write placed after a move survives as the next move's
shape. A command that runs no move keeps all its writes: they are the next
main move's staging. `leave_plan_command` only drops the scope;
`next_turn` rebuilds `TurnCtx` and so clears everything.

### 3.3 New imports

| id | raw import | wrapper | signature (wire) | lifetime | kind |
|---|---|---|---|---|---|
| 351 | `ruleset_set_forced` | `plan::set_forced(bool)` | `(on: i32)` | command | new |
| 352 | `ruleset_set_main_as` | `plan::set_main_as(bool)` | `(on: i32)` | command | new |
| 353 | `ruleset_void_walk` | `plan::void_walk()` | `()` | command | new |
| 354 | `ruleset_clear_steps` | `plan::clear_steps()` | `()` | command | new |
| 355 | `ruleset_add_extra_steps` | `plan::add_extra_steps(i32)` | `(n: i32)` | command | new |
| 356 | `ruleset_move_teleport_to` | `plan::teleport_to() -> i32` | `() -> i32` | read | new |
| 357 | `ruleset_move_pay_factor` | `plan::move_pay_factor() -> i32` | `() -> i32` (milli) | read | new |
| 358 | `ruleset_move_rent_factor` | `plan::rent_factor() -> i32` | `() -> i32` (milli) | read | new |
| 359 | `ruleset_move_effective_steps` | `plan::effective_steps() -> i32` | `() -> i32` | read | new |
| 360 | `ruleset_move_forced` | `plan::forced() -> bool` | `() -> i32` (0/1) | read | new |
| 361 | `ruleset_trig_move_rolled` | `trigger::move_rolled() -> bool` | `() -> i32` (0/1) | read | new |
| 362 | `ruleset_trig_move_effective_steps` | `trigger::move_effective_steps() -> i32` | `() -> i32` | read | new |
| 363 | `ruleset_tile_axis_nearest` | `tile_axis_nearest(tile, axis) -> i32` | `(tile: i32, axis: i32) -> i32` | pure | new |
| 364 | `ruleset_tile_axis_farthest` | `tile_axis_farthest(tile, axis) -> i32` | `(tile: i32, axis: i32) -> i32` | pure | new |

Extended, wire-compatible:

- `set_parity` (`ruleset_set_parity`) — canonicalizes `2` to `0`: `-1` any,
  `0` even, `1` odd. Non-canonical values are clamped to `-1`. The
  normalization is in `ops.rs::set_parity`, so every caller and the save path
  see one encoding.

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
  not the main move unless `set_main_as(true)`.
- `set_main_as(on)` — writes `plan.main_as`; `as_main = is_turn && (!forced ||
  main_as)`. In a move's own roll window for the same player, a `card_move`
  with `main_as` and `void_walk` replaces the pending walk in place (§4.3).
- `void_walk()` — writes `plan.void_walk`. With `main_as`, inside the move's
  own roll window, it makes the next `card_move` replace the pending walk with
  the planned teleport immediately (§4.3). Without `main_as`, or outside the
  window, the roll site just suppresses the walk: if `void_walk` is staged at
  Play it suppresses the next main's walk (pair it with a `teleport_to` when a
  teleport main is meant). A no-op for a teleport command.
- `clear_steps()` — `steps = None`; the next `card_move` rolls. The roll keeps
  its sign.
- `add_extra_steps(n)` — `plan.extra_steps += n` (signed), composing with
  other writers. `set_extra_steps` remains the absolute form.

Getters:

- `plan_teleport_to()` — the planned destination, `-1` when none.
- `plan_pay_factor()` / `plan_rent_factor()` — milli-units, `1000` = x1.
- `plan_effective_steps()` — `steps`, else the frame's `next_steps`, else
  `roll`.
- `plan_forced()` — the flag, as 0/1.
- `trig_move_rolled()` — was the *triggering* move a rolled walk? 0 when no
  move.
- `trig_move_effective_steps()` — the triggering move's `effective`, 0 when no
  move.

Axis codes: `abi::axis` `0` = up, `1` = down, `2` = left, `3` = right (the
`happy_lucky.rs` 1..4 order minus one). `tile_axis_nearest(t, axis)` is the
closest tile strictly along that ray from `t`, or `-1`; `tile_axis_farthest`
is the farthest one, or `-1`. Both exclude `t` itself.

### 3.4 Trigger payload

`TriggerMove` (`rules.rs:52-78`) gains:

```rust
/// A die produced this walk's length (a `steps` walk and a teleport are 0).
pub rolled: bool,
/// The length the move will actually take (`effective_steps`).
pub effective: i32,
```

`with_move` stamps both from the move (`rolled` and `effective_steps()`).
`bridge_trigger` copies both. The guest `abi::Trigger` payload and its
accessors gain the pair. Cond gains `move.rolled` (Bool) and `move.effective`
(Int) in `cond/src/view.rs`, `cond/src/view_impl.rs`, `cond/src/vocab.rs`, and
`cond-ir/src/vars.rs` (`MoveRolled`, `MoveEffective`). `move.rolled` is the
charity pre; `move.effective` is the misaki span length.

`rolled` is set true in the roll branch, just before the dice windows; a
fixed-step walk never enters that branch and leaves it false, and the
replacement branch clears it for a teleport. `effective` on a
`passTile`/`pass`/`moveAfter` trigger is the planned total; on a roll window
it is the face or the pending `next_steps`, whichever will be used.

### 3.5 Board geometry (data)

`TileData` gains:

```rust
/// Board cell `[col, row]` for axis queries. `None` on boards without a grid.
#[serde(default)] pub cell: Option<[i32; 2]>,
```

`board.json` rows carry `cell` from the drawn board's `pathXY` fold (the
existing `CELLS` table is the source). `GameData` gets:

```rust
pub fn tile_axis_nearest(&self, tile: i32, axis: i32) -> i32;
pub fn tile_axis_farthest(&self, tile: i32, axis: i32) -> i32;
```

Both return `-1` when coordinates are absent, the axis invalid, or no tile
lies on the ray. They are pure and board-only.

### 3.6 Not added

- No `plan_save`/`plan_restore` pair: the frame does it.
- No parity-free `next_tile` export: parity is a walk rule.
- No `set_pay_factor_mul`: the getter plus `set_pay_factor` is enough for the
  one card that needs multiplication (sakiko's double halves, open question
  §10).
- No import decides anything. `ask_tile` stays a guest prompt.
- None of the additions is state sugar: every one is a direct engine op, so
  there is no desugaring layer to update (`sugar translates before the ABI`
  does not apply).

---

## 4. Engine wiring

### 4.1 Command scope

`drive_inner_body` and `drive_inner_body_from` (`wasm_rules.rs:3050, 3064`)
wrap the guest call with `cx.enter_plan_command()` / `leave_plan_command()`.
Entering the outermost body snapshots `staging_before` and `plan_before`;
leaving only drops the scope. The table applies at each move's `end_plan`, while
the scope is live: the scope guards the **staging plan** only. When a move frame
is already live (a counteraction body raised from the move), its writes go to
that frame and survive the body, which is what each raise's fold reads. When no
frame is live, the body writes staging; a move started there seeds its frame
from those writes, and the frame's pop restores staging to `staging_before`,
dropping them with the move. Writes made after a pop (or by a body that runs no
move) hit staging directly and survive as the next main's shape. A pause keeps
`command_depth > 0` in the saved world, so the resumed body does not take a new
snapshot: the replayed body rewrites the same values the first pass wrote.

All `turn.plan` accesses in `ops.rs`, `play.rs`, `cx.rs`, the host
(`wasm_rules.rs`'s `by_skill` stamps and refusal cleanups) and `move_ctx.rs`
tests migrate to `plan()`/`plan_mut()`. The scope internals and save
round-trip keep `turn.plan` directly.

### 4.2 Creation and sealing

`main_move` (`play.rs:1349`):

1. Refuse if `main_moved` (unchanged).
2. `begin_plan(i, true, self.w.turn.plan.clone())` — before `diceRollBefore`.
3. Stamp `m.from = m.start.map_or(pos, |t| t.0)` and `m.to = m.from`.
4. Take `hidden[i].next_steps` into the frame (`next_steps`) and mirror it
   into `m` when as_main, before the windows.
5. Run the roll ladder and `rollPlan`; the window delta is folded into `m`
   after the last raise (§4.3), as `moveBefore` is folded when the walk
   starts.
6. `walk`; a roll-window replacement has already run the teleport and raised
   its tail, so only the completion remains.
7. `end_plan(m.started)`.

`card_move` (`play.rs:789`) recomputes the slot policy:

```
let forced = plan.forced;
let main_as = plan.main_as;
let as_main = is_turn && (!forced || main_as);
let frame_owner = self.plan_frame().map_or(i, |f| f.owner);
let replace = replacement_taken(w.move_roll_windows.last().copied(), frame_owner, main_as,
                                plan.void_walk, plan.teleport_to.is_some());
if as_main && main_moved && !replace { refuse; return false }
if as_main && !main_moved { main_moved = true }
if replace { return self.replace_main_move(i, plan) }   // §4.3
```

Otherwise it opens a frame (Main for `as_main`, Side otherwise) through
`begin_plan`, stamps `from` as above, and runs the shared body — the
`diceRollBefore` stage is the turn move's alone, the rest (`diceRollPlan` when
there is no step count, the ladder, walk, tail) is common. `end_plan` seals
it.
The return value is **"the move ran"**: false on the slot refusal, false on
the abnormal-gate refusal from the host, false on a start-time cancellation,
true otherwise. `HostRequest::Move` (`wasm_rules.rs:3785-3817`) returns the
inner `card_move` value when the gate allows the move and `0` when it refuses,
instead of always echoing the gate verdict.

`seed_turn_plan` and the `move_depth > 0` `mem::replace` workaround go away.
`main_move` and `card_move_seeded` keep one shared `run_move_body` helper so
the tail, followers and bookkeeping cannot diverge.

`skip_main_move` stays as it is: it is the turn-flow latch
`MatchState.skip_move`, not a plan field, and no frame clears or restores it.
`void_walk` is narrower -- it skips one move's walk, not the move phase. A card
that uses both keeps the two independent.

`teleport_as` keeps its own `Move` and tail. When no frame is live
(`plan_stack` empty) it opens a Side frame seeded by `MoveCtx::staging_seed`
and pops it after, so a top-level jump's `moveBefore` counter writes die with
the jump; when a frame is live the nearest caller owns one (`replace_main_move`
§4.3, follower replay §4.5, the main/side move frame) and `teleport_as` uses it.
`card_teleport` becomes:

- `move_depth == 0` — `teleport_as(…, resolve: false, main: false)` (which
  scopes itself).
- `move_depth > 0` and `by_skill` — position write, then `teleport_tail_only`
  (§4.6).
- `move_depth > 0`, not `by_skill` — the existing bare write.

`HostRequest::TeleportAs` likewise relies on that self-scoping when it is
top-level, and writes its temporary `resolve` through `plan_mut()`.

`card_settle_at` (`play.rs:1027`) is the third top-level move entry: it opens a
Side frame from the plan clone `card_settle_move` built, runs the settle move,
and pops, so a nested settle's counter writes do not shape the outer move.
`settle_redirect` stays engine-set on the moved clone.

### 4.3 The roll window

Both entry points push `i` onto `w.move_roll_windows` and
`frame.clone()` onto `plan_scope.window_before` around
`diceRollBefore`…`diceRollResult`, and pop both after the last raise. A nested
move with its own roll inside a counter window therefore gets its own snapshot
instead of clobbering the outer one. The fold is
`fold_plan_delta(&mut m, window_before.last().unwrap(), &frame)` before the
pop. This is what makes a detour counter and a misaki teleport land.

`fold_plan_delta` (`play.rs:108-180`) changes from "copy when changed and
non-empty" to "copy when changed", so `steps`, `start`, `stop_at`,
`settle_tile`, `teleport_to` and `parity` can be *cleared* by a window write.
Add `main_as` and `void_walk`.

Then:

- If `m.void_walk && m.teleport_to.is_some()`: the replacement already ran and
  raised its tail from inside the window (§ below). The caller frame was
  written back with `kind = Teleport`, `teleport_to = Some(landing)`,
  `path = [landing]`, `to = landing`, `total = 1`, `remaining = 0`,
  `cancelled = !ran`. Copy `kind`, `path`, `to`, `total`, `remaining` and
  `cancelled` onto `m` (the fold carries no progress fields; `teleport_to`
  itself comes from the fold) and set `m.rolled = false`. The call condition
  below skips the walk. A cancelled replacement has no tail either.
- Else if `m.teleport_to.is_some()`: a bare window write (no `void_walk`); run
  `teleport_as(i, to, m.resolve, m.why.clone(), m.main)` here, set
  `m.kind = Teleport`, `m.rolled = false`, `m.total = 1`, `m.remaining = 0`,
  `m.path = [to]` and `m.cancelled = !ran` (`m.to` comes from the player after
  the call). The tail is teleport_as's; the body does not wait on it.
- Else if `m.void_walk`: a staged void with no teleport target; nothing runs.
  Set `m.kind = Teleport`, `m.total = 0`, `m.remaining = 0` so the summary
  records no walk and no `lastWalk`.
- Otherwise the call condition below walks `m`.

The call site is `if !self.out(i) &&
move_plan::walk_runs(m.teleport_to.is_some(), m.cancelled,
m.void_walk) { self.walk(&mut m)? }`, so a set `void_walk`, a set
`teleport_to`, a cancellation and an out mover each skip the walk exactly once
(and a staged void with no target is the third branch); the ordinary pre-roll
teleport never reaches this point (its branch returns before it).

**Replacement call.** A `card_move` issued from a move's own roll window
(`move_roll_windows.last() == Some(player)`, the frame owner is `player`, the passed
plan has `main_as && void_walk && teleport_to.is_some()`) does not open a
second move: `fn replace_main_move(&mut self, i: usize, plan: Move) -> Flow<bool>`.
The check runs in `card_move` **before** the `main_moved` refusal, because the
outer main already latched it:

1. `begin_plan(i, false, seed)` — a Side frame seeded from the calling
   command's delta (so it inherits the teleport writes the body just made),
   with `by_skill` taken from the passed plan. `teleport_as` then sees a live
   frame and does not add its own.
2. `let ran = teleport_as(i, to, plan.resolve, plan.why.clone(), true)`.
   `to` is the captured `plan.teleport_to`; `landing` for the write-back is
   `st.players[i].pos` afterwards (a `moveBefore` counter may retarget it).
   `teleport_as` opens no frame of its
   own, so the Side frame scopes its `moveBefore` counter writes.
3. `end_plan(ran)`, then write `kind`, `teleport_to = Some(landing)`, `path`,
   `to`, `total`, `remaining` and `cancelled = !ran` onto the caller's frame
   as above; the move's one-shot cleanup clears `teleport_to` later; return
   `ran`. No `main_moved`/`step` latch, no follower replay, no
   `lastWalk`/`main_steps`: the outer move does all three.

The outer move skips only its walk; completion is otherwise the ordinary
teleport path, so there is exactly one tail and one settle. The body resumes
after the teleport has resolved, which keeps the card's 「随后…支付」 order.

If the abnormal gate refuses **before** `replace_main_move` (the host gate runs
first), its existing cleanup must write through `plan_mut()` — a frame is live,
`turn.plan` is staging — and clear `void_walk` and reset `kind = Walk` beside
`teleport_to`, so the outer move walks the roll normally instead of taking the
void branch with no teleport. The slot-refusal early return (which runs before
any frame opens) clears the same three fields through `plan_mut()`.

`next_steps` is consumed from `hidden[i]` at `begin_plan` for `as_main` and
mirrored into `m`, not applied after the window; `m.roll` is then clamped to
`m.effective_steps()` so a teleport in the window sees the same endpoint the
walk would have used.

### 4.4 Walk invariants

`walk_inner_depth` (`play.rs:1620`):

- `next` becomes `next_parity_tile(cur, m.dir(), m.parity, n)`.
- At the top of every iteration, sync `m.extra_steps =
  self.plan().extra_steps`. `planned` and `remaining` then follow it.
- `rolled` is set on the move in the roll branch when the face decides the
  length; a `steps`/`next_steps` walk leaves it false.
- `total`/`remaining`/`path` count parity *landings*; skipped tiles raise no
  `passBefore`/`passTile`/`pass`.
- `stop_at` is compared against the chosen tile. A `stop_at` tile of the
  other parity is not reachable while parity is set (open question §10).
- `from` is already stamped; the `start` branch moves the piece there and
  re-stamps `from` only when a `moveBefore` counteraction rewrote `start`.

```rust
/// The next tile in `dir`: the immediate neighbour when `parity < 0`, else
/// the nearest tile whose index parity matches.
fn next_parity_tile(cur: i32, dir: i32, parity: i32, n: i32) -> i32
```

### 4.5 Shared tail and followers

`run_move_body` ends with `move_after` → `after_walk` → `move_resolved` for
walks and `teleport_as_depth` for teleports. After the movement completes and
before the completion bookkeeping:

```rust
if as_main && !m.cancelled { self.replay_followers(&m)?; }
```

`replay_followers` is the block currently at `play.rs:1516-1536`, moved with
one addition: `take` the followers, skip on cancelled/out/self, then run each
survivor under its own Side frame seeded from `m` (`begin_plan`/`end_plan`
around the clone+walk, `end_plan` on every return path), so a follower's
`moveBefore` counter writes die with that follower. The clone still clears
identity/progress and the follower list; walk or teleport as today. It runs for
card-driven main moves too.

### 4.6 Nested skill teleport tail

```rust
/// Raise the movement tail for a teleport that is nested inside another
/// move's stage (a skill jump in a settle). Position is written by the
/// caller; this raises `moveAfter` then `moveResolved` on a synthetic
/// teleport Move with `kind = Teleport`, `by_skill = true`, `from`, `to`.
fn teleport_tail_only(&mut self, i: usize, from: i32, to: i32) -> Flow<()>
```

The tail runs under a Side frame seeded from the plan (as §4.5), so a
`moveAfter` counter's writes die with the nested jump. A `tail_only_depth`
guard (one bool) prevents re-entering `teleport_tail_only` from a tail hook. `lisa_bond` needs no change: its existing `MoveAfter` /
`move.kind == Teleport && move.by_skill` filter starts matching.

### 4.7 Geometry

`ops.rs` gains the two query wrappers over `GameData`, `hostfns.rs` the
`HostCtx` methods, and the happy_lucky private table is deleted once the data
is filled (fallback: if `cell` is absent, it keeps the local table).

### 4.8 Host pauses and the guest copy

`HostCtx::move_plan` (`wasm_rules.rs:2532`) returns `self.plan().clone()`.
`HostCtx::move_*` getters likewise read `plan()`. `adopt_turn_policy`
(`cx.rs:802`) replaces its plan-field copies with the whole `plan` staging,
`plan_stack` and `plan_scope`, plus `move_roll_windows` (the routine is about
the same move; nothing else crosses the pause). Because the adopted stack is
the guest copy's, an interrupted engine call's own frame is dropped and opened
again by the retry, exactly like the guest body's replay. `World::plan()` is
used by `scale_settle_payment` and every other settle reader.

### 4.9 Kani model

- `MoveSnap` (`rules/kani/src/world.rs:243-261`) gains `rolled` and
  `effective`; `NONE` uses `rolled: false, effective: 0`.
- The kani world gains `main_moved`, `forced`, `main_as`, `void_walk`,
  `teleport` and `window` for the `arm_card_move` policy:
  `if as_main && main_moved && !(window && main_as && void_walk && teleport)
  { 0 } else { 1 }` (the pure `replacement_taken`, inlined).
- Plan setters (`set_forced`, `set_main_as`, `void_walk`, `clear_steps`,
  `add_extra_steps`) join the protocol stub group (`host.rs:1022-1072`);
  getters join the zero-return group. `trig_move_rolled`/`_effective` read the
  snap. `tile_axis_*` return `-1` unless the model board is enabled.
- The pure-model proofs live in `rules/kani/src/proofs/move_plan.rs` (declared
  in `proofs/mod.rs`) over `game_kinds::move_plan`; `rules/kani/Cargo.toml`
  adds `game-kinds` (already transitive through `card-sdk`) as an explicit
  dependency.
- Every card proof that reaches a new import adds it to its `host_subset!`
  (`rules/kani/src/proofs/{ag,crychic,hhw,mujica,roselia,morfonica,sumimi}.rs`);
  `rules/kani/build.rs` allowlists the names.
- `arm_card_move`'s old constant-1 is replaced so the fate/misaki proofs can
  distinguish refused from ran.

---

## 5. Card usage — before / after

### 5.1 `want_to_grab` (item 1)

Before: `on_turn_start` retires unconditionally.

After:

```rust
if ctx::state::get(player_id, PASSER) == NEVER {
    let pos = ctx::player_pos(player_id);
    let mut tiles = Vec::new();
    for axis in 0..4 {
        let t = ctx::tile_axis_nearest(pos, axis);
        if t >= 0 && t != pos { tiles.push(t); }
    }
    if !tiles.is_empty() {
        let to = ctx::ask_tile(player_id, &title, &text, &tiles)?;
        ctx::plan::set_kind(MoveKind::Teleport);
        ctx::plan::set_teleport_to(to);
        ctx::plan::set_resolve(true);
        ctx::card_move(player_id);
    }
}
ctx::set_dest(ctx::Dest::Graveyard);
```

The four candidates were computed from the axis rays; the tile pick remains a
guest prompt.

### 5.2 `pure_wings` (items 2-4)

Before: relies on a fresh plan; carries three TODOs.

After:

```rust
ctx::teleport_to(player_id, to);          // the jump (no tail frame)
ctx::plan::clear_steps();                 // roll from the destination
ctx::plan::set_kind(MoveKind::Walk);
ctx::plan::set_teleport_to(-1);
ctx::plan::set_resolve(true);
if ctx::plan::move_pay_factor() != 1000 { /* log the inherited factor, optional */ }
ctx::card_move(player_id);
```

`dir()` needs no change: the frame's staging was clean at begin; the card's
own `set_*` writes are the only delta.

### 5.3 `charity_show` (items 5-6)

Before: pre `… && move.main && tile.id >= 0`; body tracks an absolute prop.

After: pre `… && move.kind != Teleport && move.rolled && tile.id >= 0`; body
calls `ctx::plan::add_extra_steps(2)` and keeps only the first-pass marker.
`move.main` is removed from the pre.

### 5.4 `sakiko_lead` (item 7)

No body change. Remove the TODO. The replay now happens in the shared tail
for a card-driven main move; `set_pay_factor(500)` staged at Play still
reaches it because the as_main frame seeds from the whole staging plan.

### 5.5 `detour` (items 8-9)

Guard unchanged (demoted to A). Parity already written; after the walk change
it works. Remove both walk TODOs. The Himari `2` is already canonicalized to
`0` by `set_parity`, so it may stay or be written as `0`:

```rust
// even
ctx::plan::set_parity(0);
```

### 5.6 `saki_move` (item 10)

Add before the existing plan block:

```rust
ctx::plan::set_forced(true);
ctx::plan::set_main_as(hit == player_id);
```

Remove the shape-leak TODO. Targeting yourself keeps `main_as` true and
consumes the slot; targeting someone else never touches the slot.

### 5.7 `misaki_card` (items 11-13)

Before: `trigger::set_cancelled()`, plan teleport writes, `card_move` (refused).

After:

```rust
trigger::set_cancelled();                       // the link only
ctx::plan::set_kind(MoveKind::Teleport);
ctx::plan::set_teleport_to(to);
ctx::plan::set_resolve(true);
ctx::plan::set_main_as(true);
ctx::plan::void_walk();
let ran = ctx::card_move(player_id);            // replaces the walk now
```

`between()` uses `trigger::move_from()` (item 12) and
`trigger::move_effective_steps().abs()` (item 13). The teleport has already
settled when `card_move` returns, so the log names `to` rather than
`player_pos` (the settle itself may move the player again; the chosen
destination is the stable fact). `ran` guards the fire-pot payout/log (if the
abnormal gate refused, nothing moved).

### 5.8 `fate_together` (item 14)

Before: `if !ctx::card_move(player_id) { hit = 0 }` with `card_move` mistakenly
main.

After:

```rust
ctx::plan::set_forced(true);   // an effect move, never the main move
ctx::plan::set_steps(i);
ctx::plan::set_resolve(false);
if !ctx::card_move(player_id) { hit = 0; return; }
```

The bool now means "ran", so a spent main move is not a false hit.

### 5.9 `cant_look_away` / `doll_garden` (items 15-16)

Both set the forced pair immediately before each `card_move` (inside the
victim loop and before the sum walk):

```rust
ctx::plan::set_forced(true);
ctx::plan::set_main_as(true);       // C# MainMoveAs
```

`doll_garden` runs every victim through `card_move`, each on its own side
frame; the final sum walk is a main frame seeded from the clean staging, so
the victims' shape cannot leak into it. Remove both TODOs.

### 5.10 `lisa_bond` (item 17)

No body change. The existing `MoveAfter` registration starts receiving nested
skill teleports; remove the TODO.

---

## 6. Save, record, replay

- New `MoveCtx` fields use `#[serde(default)]`, so a version-12 save without
  them loads. `MoveCtx` has no struct-level `default`; every new field must
  carry the attribute or old plans fail to parse.
- `TurnCtx.plan_stack`, `TurnCtx.plan_scope` and `World.move_roll_windows` are
  `#[serde(default)]`. An empty stack, scope and window list are exactly an
  old save: no frame to restore, staging behaves as before. **SAVE_VERSION
  stays 13** (G08 already moved it; this group adds no further bump).
- The record stores actions, prompts and engine stamps, not plan internals;
  the frame is reconstructed by replaying commands. **RECORD_VERSION stays 3.**
- Host pauses serialize the stack and scope (§4.8), so a pause inside a move
  resumes with the same frame. A save taken mid-move round-trips the frame;
  the walk's `Move` and the plan frame agree on `extra_steps` because both are
  saved and the step syncs from the frame.
- Old records replay on their archived engine bundle and ABI. New records
  embed the ABI-105 engine stamp.
- `next_turn` drops the stack and scope and empties `move_roll_windows`; a turn
  never starts with a live frame or an open window.

## 7. Versioning

`crates/card-sdk/src/abi.rs`:

```rust
pub const ABI_VERSION: i32 = 108;   // was 107 (fourth group)
/// v108: **command-scoped move plans and walk control**. The turn plan is a
///    staging buffer; every move runs on a frame owned by its command
///    (`World::begin_plan` / `end_plan`) and is dropped when the command ends.
///    New imports: `ruleset_set_forced` (351), `ruleset_set_main_as` (352),
///    `ruleset_void_walk` (353), `ruleset_clear_steps` (354),
///    `ruleset_add_extra_steps` (355), `ruleset_move_teleport_to` (356),
///    `ruleset_move_pay_factor` (357), `ruleset_move_rent_factor` (358),
///    `ruleset_move_effective_steps` (359), `ruleset_move_forced` (360),
///    `ruleset_trig_move_rolled` (361),
///    `ruleset_trig_move_effective_steps` (362),
///    `ruleset_tile_axis_nearest` (363), `ruleset_tile_axis_farthest` (364).
///    `MoveCtx.forced` is the C# `Forced` flag --
///    a forced move of the turn player is still the main move when
///    `main_as` is set -- no longer "never the main move". `set_parity`
///    canonicalizes `2` to `0` (even). The trigger move payload gains
///    `rolled` and `effective`, surfaced as `move.rolled` / `move.effective`.
///    SAVE_VERSION / RECORD_VERSION unchanged.
```

The bump is a wire break on two counts: a new import set (the module-load
exact equality refuses old wasm) and two new trigger fields in the postcard
payload. Every card wasm and the kani host rebuild together.

## 8. Tests and Kani obligations

### 8.1 Engine (game-core)

`crates/game-core/src/engine/play/tests.rs` and `move_ctx.rs`:

| name | asserts |
|---|---|
| `g04_side_move_restores_staging` | a side command's `steps`/`reverse`/`pay_factor` are gone at command end |
| `g04_refused_main_keeps_staging` | a refused card main move leaves a staged teleport/factor intact |
| `g04_main_consumes_staging` | an as_main move latches the slot and clears staging |
| `g04_forced_never_refused` | a forced move of the turn player runs with the slot spent and does not consume it |
| `g04_main_as_consumes_when_free` | a forced `main_as` move latches a free slot |
| `g04_void_walk_replaces_the_walk` | the walk's pass events do not raise; the teleport tail does |
| `g04_replacement_settles_before_the_body_resumes` | the teleport's settle and tail precede the body's `pay` |
| `g04_gate_refusal_restores_the_walk` | a refused replacement walks the roll with `kind = Walk` and no teleport |
| `g04_effective_steps_prefers_next_steps` | `next_steps` is visible in the `diceRollResult` window and used by the walk |
| `g04_from_is_stamped_before_the_roll` | `trigger::move_from()` is the planned start inside the window |
| `g04_extra_steps_add_is_relative` | two `add_extra_steps` calls compose; the walk lengthens mid-step |

### 8.2 Card (rules/tests/tests)

One per slug, in the file named beside it:

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
| 11 | misaki-teleport | `g04_misaki_teleport_as_main_voids_the_rolled_walk` | `hhw.rs` |
| 12 | misaki-planned-start | `g04_misaki_span_uses_the_planned_start` | `hhw.rs` |
| 13 | misaki-effective-steps | `g04_misaki_span_uses_next_steps` | `hhw.rs` |
| 14 | fate-forced-hop | `g04_fate_hop_does_not_consume_or_require_the_main_move` | `crychic.rs` |
| 15 | plan-set-forced | `g04_forced_walk_of_the_turn_player_is_main_only_with_main_as` | `mujica.rs` |
| 16 | dg-plan-reset | `g04_doll_garden_victims_do_not_leak_shape` | `mujica.rs` |
| 17 | lisa-bond | `g04_lisa_bond_opens_after_a_nested_skill_teleport` | `roselia.rs` |

Positive and negative pairs where the rule has a choice (13: with and without
`next_steps`; 15: self vs other; 9: odd vs even vs reverse).

### 8.3 Kani obligations

Harnesses in `rules/kani/src/proofs/move_plan.rs` over the production
functions in `game-kinds`, each an iff proof with a cover and a mutant:

- `move_plan::command_fields_are_never_staging` — exhaustive over `PlanField`:
  `is_staging` is false for every `Command` field.
- `move_plan::main_frame_keeps_turn_fields` — an as_main frame seeded from
  staging carries `base`/`dice`/`min_roll`/`signed`.
- `move_plan::side_seal_drops_command_fields` — after a side command's seal,
  staging no longer carries `steps`/`reverse`/`parity`/`stop_at`.
- `move_plan::side_seal_keeps_next_main_fields` — staging
  `teleport_to`/`start`/`pay_factor`/`followers` survive a side command.
- `move_plan::main_seal_consumes_next_main_fields` — an as_main move leaves a
  default staging plan.
- `move_plan::parity_step_is_nearest_qualifying_tile` — `next_parity_tile` is
  the smallest `d ≥ 1` with the requested parity.
- `move_plan::parity_step_never_returns_start` — for `n > 1` the step moves.
- `move_plan::parity_step_stays_in_range` — result in `0..n`.
- `move_plan::effective_steps_order` — `steps` > `next_steps` > `roll`.
- `move_plan::axis_nearest_is_on_ray` / `axis_nearest_is_nearest` — same
  column/row, minimal distance, never the start.
- `move_plan::replacement_taken_exactly` — `replacement_taken` iff
  `window == Some(owner) && main_as && void_walk && teleport`.
- `move_plan::walk_runs_exactly` — `walk_runs` iff no teleport target, not
  cancelled and not `void_walk` (so a replacement and a pre-roll teleport both
  skip the walk exactly once).

Per item: 1 → axis proofs; 5 → `MoveSnap.rolled` through `arm_card_move`;
9 → the parity proofs; 11 → the replacement/walk proofs plus the misaki card
proof; 13 → `effective_steps_order`; 14/15 → `arm_card_move`; 16 → the seal
proofs.

`arm_card_move` proof: `refused iff as_main && main_moved && !replacement_taken`.

## 9. Alternatives

**A1 — Field lifetimes on the sticky plan, no frames.** Classify each
`MoveCtx` field as Turn / NextMain / Command and reset Command fields after
every move. Simple to state, but `pay_factor` is both Command (saki) and
NextMain (sakiko), `teleport_to` is both NextMain (kanon) and Command
(misaki), and the split needs exceptions per writer. The frame encodes the
intent once; the field list does not.

**A2 — Save/restore getters in the card.** Add `plan_save() -> PlanHandle` /
`plan_restore(handle)` and make every body restore its shape. No engine
scoping. Gives up the entire class fix (a forgetful card re-leaks) and every
card pays boilerplate; it also cannot express "the staging teleport belongs to
the next main" when a side move intervenes.

**A3 — Fresh `MoveCtx` for every `card_move`, ignore staging.** Kills leaks
outright but loses Play-time shaping (`change_world` dice, detour base dice,
kanon teleport) unless a second, explicit next-main channel is added, and every
body that pre-shapes its own move is rewritten to pass a full move.

**A4 — Defer the window teleport to the roll site.** The body only records the
teleport; the roll site runs it after the last roll window. One frame and one
tail, but the body resumes before the teleport settles, so misaki's
「随后…支付」 pays before the destination resolves and a settle that reads the
board sees pre-teleport positions. A bare window write with no body waiting on
it keeps exactly this path (§4.3, second branch); a card that acts after the
teleport uses the immediate replacement, which pays in book order, at the cost
of the nested call rules in §4.3.

## 10. Open questions

1. **Turn-start gate for (1).** If the [停留] layer this card just granted is
   still present at the owner's next turn start, `card_move`'s main move may
   be blocked. Does (1)'s teleport still happen? The book does not say.
2. **Parity pass events.** Does a parity walk "pass" the skipped tiles? We
   choose no (only landings pass). A reading of 「仅在…上进行」 as continuous
   movement would raise `pass` on every ring tile between landings.
3. **Parity vs `stop_at`/`settle_tile`.** A stop tile of the other parity is
   unreachable while parity is set. Should the walk stop there anyway?
4. **`next_steps` on a card main move.** Today only `main_move` consumes it.
   This design consumes it for every `as_main` move; confirm that a
   card-driven main move (「视为主要移动」) counts for a `next_steps` buff.
5. **Follower replay on a teleport main move.** Followers carry `kind` and
   `teleport_to`, so they teleport. Confirm this is intended (sakiko plus a
   teleport main is plausible).
6. **`main_as` outside a roll window.** A forced move of the turn player
   marked `main_as` outside any window still takes the slot check: it runs
   when the slot is free and is refused when spent. Is that the right reading
   of `MainMoveAs`, or should a forced `main_as` move always run (and only the
   slot bookkeeping be skipped when spent)?
7. **Grid source.** `board.json` carries `cell`, or the ruleset manifest does.
   The memory note "data lives with code" points at the manifest; the boot
   board is still JSON today.
8. **Sakiko double halving.** Two `set_pay_factor(500)` calls overwrite rather
   than multiply (`sakiko_lead.rs:73-77`). A multiplicative import is in
   G02's pay scope; this design only adds the getter.
9. **`BySkill` on a nested tail.** `teleport_tail_only` inherits `by_skill`
   only when the host set it. A card body that itself calls `teleport_to`
   inside a settle without the host's skill prefix would not open lisa's
   window; is that the right scope for 「使用技能进行传送」?
