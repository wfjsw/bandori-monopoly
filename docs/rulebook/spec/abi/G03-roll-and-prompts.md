# ABI design G03 — Roll pipeline and prompt options

**Status: draft for user review. Design only — no source edits, no commit.**
Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`. Fits **on top of ABI v104** (the v103 windowed
`ctx::roll` / `roll_direct`, `RollPurpose`, `asked()` pause, and G06's v104
turn snapshot are already on disk); it does not redo either.

Closed slugs and their review rows:

| # | slug | sev | rows |
|---|------|-----|------|
| 1 | `starry-value-stage` | low | 1262 |
| 2 | `talent-effect-rolls-unpaid` | medium | 179, 1224 |
| 3 | `taki-per-player-rolls` | low | 97, 1159 |
| 4 | `taki-extreme-reroll` | low | 1160 |
| 5 | `want-human-boost-vs-reroll` | high | 150, 1002 |
| 6 | `want-human-crystal-on-reroll` | low | 1169 |
| 7 | `detour-plan-writes-lost` (G04's slug; the roll half) | high | 973 |
| 8 | `hagumi-per-roll-options-abi` | medium | 242, 1187, 249 |
| 9 | `misaki-fire-roll-signal` | high | 292, 954, 294, 298, 1006 |
| 10 | `moca-roll-post-additive-stage` | medium | 62 |
| 11 | `noexpect-roll-scope` | high | 596, 598, 1367, 1479 |
| 12 | `layer-keep-keep-point` | high | 892, 893, 1379 |
| 13 | `please-choose-opt1-inert` | medium | 750 |
| 14 | `dg-move-dice` | medium | 1053 |
| 15 | `own-stage-answer` | high | 671, 672, 673, 674, 1455 |

Ground truth: `docs/rulebook/cards.json`. Every fragment below was re-read from
that file on 2026-10-28 against the card id named beside it.

---

## 2. Problem

### 2.1 Per item

Each entry states the verbatim fragment, the row ids, what the tree does today,
what the book requires, and the root fix. "Demote to A" means: the mechanism
already exists in the tree and only the card/test changes.

**1. `starry-value-stage` — Mor:蝴蝶飞舞的星月夜, rows 1262**

> 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」

`rules/cards/card-morfonica/src/starry_night.rs:199-203` rerolls through
`ctx::do_move_roll` (= v103 `roll_direct(…, RollPurpose::Move)`). That export
sums the plan's tables and returns the **raw** face; the enclosing
`diceRollAfter` raise later just reads the written value back. The rerolled
face therefore never passes `diceRollValue`, so 「最终结算」 reshapes (摩卡
0.5倍速) are lost for exactly the reroll the card is allowed to make.

Expected: the rerolled face is settled again through the value ladder.
**Root fix (B):** recast + automatic re-settlement (4.1, 5.2).

**2. `talent-effect-rolls-unpaid` — HHW:运动的天赋, rows 179, 1224**

> 「此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。」

The reward rides `Action::DiceRollAfter`, and the engine raises
`diceRollAfter` only in the two move dispatches (`play.rs:900, 1443`).
`HostRequest::Roll` (`wasm_rules.rs:4044-4067`) raises only `diceRollBefore`;
`roll_direct` raises nothing. Effect rolls never pay, and neither does a
non-main forced walk. The card's own guard narrows it further
(`move.main`). The source comment at `sports_talent.rs:23-26` already says so.

Expected: every roll pays, whatever cast it. **Root fix (B):** a per-cast
`diceRollCast` Action raised by the one cast point (4.1).

**3. `taki-per-player-rolls` — CRYCHIC:（立希）即便比不上..., rows 97, 1159**

> 「你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。」

`ctx::turn_rolls()` reads `TurnCtx::turn_rolls: Vec<i32>` — turn-wide, with no
roller. An opponent's effect roll blocks the owner's reroll
(`taki_even_if.rs:43-45`).

Expected: only the owner's own faces count. **Root fix (B):** a roll record
carrying the roller + a per-roller reader (5.3), leaving the legacy vector in
place for older saves.

**4. `taki-extreme-reroll` — same card, row 1160**

> 「进行一次重骰」

`card_do_move_roll` (`play.rs:2263-2269`) and `roll_tables`
(`play.rs:1572-1595`) both honour `TurnCtx::extreme` today — the producer path
is already correct after v103. **Demote to A:** keep one pinned test; no ABI
change beyond the recast sharing the same cast function (5.2).

**5. `want-human-boost-vs-reroll` — CRYCHIC:想要成为人类, rows 150, 1002**

> 「…并使你下次的移动掷骰结果额外增加20-X」

The boost is added at `DiceRollAfter` (`want_human.rs:121-124`) and the slot is
consumed there. A later `diceRollResult` reroll replaces the face; nothing
re-applies the consumed boost. Expected: the settled result still carries
+20-X. **Root fix (B):** express the boost as an additive filter
(`DiceRollAdd`) that the resettle re-folds, plus a post-settlement observer
for the one-shot consumption (4.2).

**6. `want-human-crystal-on-reroll` — same card, row 1169**

> 「每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶。」

Check runs at `diceRollAfter` only (`want_human.rs:93-107`); the reroll lands
later at `diceRollResult`. Expected: the settled ≤X face banks its crystal.
**Root fix (B):** the same post-settlement `diceRollSettled` point as item 5.

**7. `detour-plan-writes-lost` — AG:回家的路上绕个道, row 973**

> 「进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的（2）效果）」

`counteract_with_plan` writes `ctx::plan::*` from inside the `diceRollResult`
window (`detour.rs:266-281`), but the dispatch already cloned the move and
reads only `t.value` / the plan's `resolve` back (`play.rs:1447-1471`). Every
plan mutation (`reverse`, `start`, `parity`, `teleport_to` from other cards)
is dropped before the walk. Expected: window plan writes reach the walk.
**Root fix (B):** fold the plan delta into the move after every mid-roll raise
(5.7) — the same helper `walk_inner_depth` already uses at moveBefore.

**8. `hagumi-per-roll-options-abi` — HHW:（育美）, rows 242, 1187, 249**

> 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。」

*Already fixed in this branch's A pass:* the options are now `(tile, roll)`
pairs deduped on the pair, answered by **option index** through `ask_pick`
(`hagumi_marks.rs:76-99`), so two rolls on one tile stay two options and the
chosen roll survives. What remains (B): the prompt carries a `tile`/`roll`
message per option so the client can highlight the tile, instead of a bare
option list — the `ask_tile_pick` reader of 4.5. The card body after that is
unchanged in substance. **Root fix (B):** `ask_tile_pick` (4.5).

**9. `misaki-fire-roll-signal` — HHW:（美咲）, rows 292, 954, 294, 298, 1006**

> 「[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡…」

The pre requires `move.tag('fireRoll') != 0`; no shipped producer writes that
tag (`misaki_card.rs:25-30`), and `roll_source` is stamped only on
`diceRollBefore` (`wasm_rules.rs:4062-4065`), never on `diceRollResult`.
`layer` recasts with `roll_source::CHARGE`, but the outer move roll's
result window still reads `NONE`. Expected: the card is playable only after a
fire-pot-funded move roll. **Root fix (B):** the roll carries a source
throughout the transaction and the result window reads it (4.4, 5.4); the pre
becomes `roll_source == 1`.

**10. `moca-roll-post-additive-stage` — AG:（摩卡）0.5倍速, row 62**

> 「此卡在场时，你的移动掷骰的最终结算/2（向上取整）」

The only reshape stage is `DiceRollValue`, folded in placement order, so
`PP:不要背负期待`'s additive −2 and this half commute by accident of
placement (7 → 4 → 2). 「最终结算」 means *after* the additive pass:
7 → 5 → 3. Filed as OPEN-ROLLRESHAPE-01, which this design closes by ordering.
**Root fix (B):** the additive stage (`DiceRollAdd`) always precedes the final
settlement stage (`DiceRollValue`) (4.2).

**11. `noexpect-roll-scope` — PP:不要背负期待, rows 596, 598, 1367, 1479**

> 「非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果最小为0，如果为0则此次移动不[结算]）」

`DiceRollValue` is raised only inside the engine's move dispatch; `ctx::roll`
(effect rolls) raises only `diceRollBefore` and `roll_direct` raises nothing,
so no effect roll and no card-driven reroll is shaved
(`no_expectation.rs:31-38`). Expected: every non-turn-start roll loses 2, with
the 0-floor / no-settle clause applying to moves only. **Root fix (B):** the
ladder runs on every cast and the payload carries the purpose (4.4, 5.4).

**12. `layer-keep-keep-point` — RAS:（和奏瑞依）寄于指尖的执念, rows 892, 893, 1379**

> 「可打出此卡并保留（写下）未被选择的另一个骰点」

Three holes: the fire reroll's window resolves before 择其一; a card placed in
that window is not in the enclosing `DiceRollAfter` walk already in flight;
and no payload ever carries the unchosen face (`layer_keep.rs:107-117,
131-139`). The card's `layerUnused` slot has no writer. Expected: the
unchosen face reaches the card after the pick. **Root fix (B):** the
transaction keeps every cast, the reroll records `unused`, and a
post-settlement observer delivers both faces (4.3, 5.2, 5.4).

**13. `please-choose-opt1-inert` — RAS:PLEASE CHOOSE, row 750**

> 「（1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」

`pick == 0` logs a TODO and returns (`please_choose.rs:76-88`). **Demote to
A**, owned by BACKLOG ABI-04 — not a roll-pipeline change. G03 fixes only the
shared prompt half: the option set is a real option list and option (1) is
offered only when a legal hand card exists (5.6).

**14. `dg-move-dice` — Mujica:人偶的箱庭, row 1053**

> 「进行一次移动掷骰并移动对应步数（不[触发结算]）」

The card rolls a generic `ctx::roll(p, 1, 20, Effect)` for each victim
(`doll_garden.rs:84`). That ignores the victim's move-dice modifiers,
`DiceRollAfter` hooks and the plan's dice table. Expected: it is the victim's
**move** roll, still not the main move (no main-move latch), with the victim
as `actor`. **Root fix (B):** `RollPurpose::Move` with an explicit roller and
a synthetic non-main move view on the trigger (5.4, 6.4).

### 2.2 Shared root causes

1. **No roll object.** A roll is a bare `i32` passed through a raise. There is
   nowhere to record its roller, purpose, source, casts or `unused` face, so
   every observer re-derives facts from the enclosing move (items 1, 2, 3, 5,
   6, 9, 11, 12, 14).
2. **Cast and settlement are the same event.** `do_move_roll` /
   `roll_direct` are raw, `diceRollValue` is a single reshape stage, and a
   reroll silently discards the settled face. There is no stage separation and
   no re-settlement (items 1, 5, 6, 10, 11).
3. **Stage order is placement order.** Additive and multiplicative reshapes
   share `DiceRollValue` and fold by field order (item 10).
4. **The dispatch only reads `t.value` back.** Mid-roll plan writes die (item
   7).
5. **The prompt is a tile prompt even when the subject is a roll.** The answer
   is a tile id and the option list is deduped by tile (item 8).

---

## 3. Capability as an invariant

**A roll is a transaction with a purpose, a roller, a source and a record of
every cast. Every cast runs the same fixed stage ladder; the final settlement
runs exactly once on the chosen face, after every additive modifier; a reroll
is a new cast that re-settles the chosen face and exposes the unchosen one.
Every roll prompt is bound to a roll and answers an option index.**

Invariants the implementation must hold (each gets a test or a Kani
obligation, §9):

* **I1 Purpose.** Every cast has a `RollPurpose` (`Effect` | `Move`) and a
  `roll_source`. The purpose selects the producer and the move-only clauses;
  nothing infers "move" from `sides == 0` (v103 already removed the hack).
* **I2 Ladder.** A cast runs `Cast → DiceRollCast → DiceRollAdd →
  DiceRollValue → DiceRollAfter → [DiceRollResult] → Settle → DiceRollSettled`.
  No path skips a stage because of who called it. `Effect` runs the same
  ladder; only the `DiceRollResult` window is Move-only.
* **I3 Settlement once, last.** `DiceRollValue` runs exactly once per chosen
  face and always after the whole `DiceRollAdd` fold. A recast re-runs
  `DiceRollAdd` + `DiceRollValue` and never re-runs a state-consuming stage
  (`DiceRollAfter`).
* **I4 Attribution.** The roller is recorded on the transaction and on every
  cast. Filters keyed to "your roll" see only the owner's casts; per-roller
  history is a projection of the record, never the whole turn.
* **I5 Choice.** A prompt over rolls carries `(tile?, value, label)` per cast
  and answers an option **index**. Each cast is exactly one option; identical
  casts may dedupe. A reroll exposes `chosen` and `unused`.
* **I6 Accounting.** Every cast is logged once with its roller. Every roll
  that reaches a terminal cast raises exactly one `DiceRollSettled`.
* **I7 Totality.** Every import exists in the wasm, native and Kani hosts
  (§5.8); no import asks a player; filters stay pure (`refuse_if_filter`).

---

## 4. Surface

### 4.1 Stage kinds (new; `crates/game-kinds/src/lib.rs`)

| id | wire | class | filter | hook-only | meaning |
|---:|---|---|---|---|---|
| 123 | `diceRollCast` | Action | no | yes | one cast is on the table, raw. Payload: the roll, value = raw face, source. |
| 124 | `diceRollSettled` | Action | no | yes | the terminal cast finished settling. Payload: the roll, `value` = settled face, `rerolled`, `chosen`, `unused` (`-1` when none), purpose. |
| 125 | `diceRollAdd` | Filter (value) | yes | yes | the additive pass. Reads + writes the running face; payload reader sees purpose / rerolled. |

110 (`paySplitMulBefore`) is the current maximum; 123..=125 are this group's
README band (111-122 go to earlier-landing groups). Retired
ids (16-20, 22, 94, 104) stay untouched. `diceRollValue` (102) is **redefined
in place** as the final-settlement (multiplicative) stage; its numeric id and
wire name do not move.

### 4.2 `RollCtx` (new, `crates/game-core/src/engine/roll.rs`)

```rust
/// Monotonic per-match id; -1 = none. Never persisted by a card.
pub struct RollId(i32);

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollStage { Plan, Cast, Add, Value, After, Window, Settle, Done }

/// One cast. `value` is raw until the settle pass writes `settled`.
#[derive(Clone, Serialize, Deserialize)]
pub struct RollCast { pub value: i32, pub settled: Option<i32>, pub source: i32 }

#[derive(Clone, Serialize, Deserialize)]
pub struct RollCtx {
    pub id: i32,
    pub roller: i32,
    pub purpose: i32,          // abi::RollPurpose wire code
    pub count: i32,            // Effect producer only
    pub sides: i32,            // Effect producer only
    pub stage: RollStage,
    pub casts: Vec<RollCast>,
    pub chosen: usize,
    pub source: i32,           // highest-precedence source seen (Charge > Skill > Card > NONE)
    pub rerolled: bool,
    pub window: bool,          // false = windowless (roll_direct / main-move pre-roll)
    pub parent: i32,           // -1 = top level
    pub pre_raised: bool,      // the Move dispatch already raised diceRollBefore
    /// A recast or a stage **after** the value pass wrote the face (recast /
    /// After / Window / Settled / `roll_choose`). The first cast and the
    /// Add/Value folds do not set it; the settle re-run keys on it.
    pub tainted: bool,
}
```

`Match` gains `rolls: Vec<RollCtx>` (a stack; `#[serde(default)]`). Helpers:
`begin`, `join`, `cast`, `fold_add`, `fold_value`, `after`, `window`,
`resettle`, `settle`, `commit`, `find`, plus pure predicates `can_join`,
`depth`.

### 4.3 `abi.rs` additions

```rust
/// Why a die is being cast. Wire codes are frozen i32s.
#[repr(i32)]
pub enum RollPurpose { Effect = 0, Move = 1 }        // unchanged

/// The roll a `DiceRoll*` trigger belongs to. Returned by
/// `ctx::trigger::roll()`, the same typed-reader shape as `trigger::buy()`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RollPayload {
    pub id: i32,
    pub purpose: i32,
    pub source: i32,
    pub rerolled: bool,
    pub chosen: i32,
    /// One per cast, in cast order; exactly one has `chosen == true`.
    pub casts: Vec<RollCastPayload>,
    /// The unchosen face of the latest recast, `-1` when none.
    pub unused: i32,
    pub final_face: Option<i32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RollCastPayload { pub value: i32, pub source: i32, pub chosen: bool }
```

`payload::RollValue` gains:

```rust
pub struct RollValue {
    pub value: i32,             // running face (read/write)
    pub purpose: i32,           // RollPurpose wire code
    pub rerolled: bool,
    pub recorded_roll: Option<i32>,
    pub resolve: Option<bool>,  // move only; settles to false at 0
}
```

`payload::RollAddValue` is the plain `i32` value shape, but its cross-target
write is unchanged (`set_move_roll`).

`PromptKind` is **unchanged**. `ask_tile_pick` (§4.5) reuses
`PromptKind::Tile` with one `PromptOption::Tile { tile, label }` per option
and answers the option index. That keeps the answer-log shape (a single
`i32`) and the client's tile-highlight UI; duplicates in the tile column are
legal.

### 4.4 Imports (new; `ruleset_*`)

| import | args | returns | pause | why it is not a decision |
|---|---|---|---|---|
| `ruleset_roll_recast` | `(roll_id, source)` | raw face | Sentinel (it raises the recast's `diceRollBefore` when `window`) | casts dice only |
| `ruleset_roll_choose` | `(roll_id, index)` | — | none | writes the chosen index, clamped |
| `ruleset_roll_current` | `()` | roll id or `-1` | none | read |
| `ruleset_roll_face` | `(roll_id, index)` | face or `-1` | none | read |
| `ruleset_roll_unused` | `(roll_id)` | face or `-1` | none | read |
| `ruleset_roll_rerolled` | `(roll_id)` | `0/1` | none | read |
| `ruleset_rolls_of` | `(player_id, buf, cap)` | `n` | none | read; postcard `Vec<i32>` |

`ruleset_roll` keeps its v103 signature (it already takes `source`).
`ruleset_roll_direct` gains `source` as a new 5th arg (wire call-shape change;
a windowless cast still needs its source for item 9 and for
`diceRollSettled`). Both are host imports the guest wrappers call, see §8.

`ruleset_roll` semantics: **top level** (no joinable Move transaction) opens,
runs the whole ladder and commits; **joinable** (`purpose == Move`, an open
Move transaction) casts raw, raises `diceRollBefore` on the raw face, and
returns it. `ruleset_roll_direct` is the same without `diceRollBefore`.
A nested `Effect` always opens a child transaction (it settles itself).

### 4.5 `ctx` wrappers (guest side; sugar before the ABI)

```rust
/// The innermost open roll, if any.
pub fn roll_current() -> Option<i32>;

/// Results of every cast of `roll`, in cast order.
pub fn roll_faces(roll: i32) -> Vec<i32>;

/// The face the player did not choose (the latest recast), if any.
pub fn roll_unused(roll: i32) -> Option<i32>;

/// Recast `roll` without staging; the enclosing transaction settles it.
/// `source` is raised onto the transaction if it outranks the current one.
pub fn roll_recast(roll: i32, source: i32) -> Result<i32, Prompt>;

/// Pick one of the faces `roll` has drawn. Clamped to the cast range.
pub fn roll_choose(roll: i32, index: i32);

/// Cast a new face for the current roll, let the player pick old/new, and
/// settle the chosen face (the Add/Value pass runs once more).
/// `None` current roll = a fresh `roll(Move)` (defensive).
pub fn roll_reroll(
    player_id: i32,
    title: &Msg,
    text: &Msg,
    keep_old: &Msg,
    keep_new: &Msg,
    source: i32,
) -> Result<RollReroll, Prompt>;

pub struct RollReroll { pub chosen: i32, pub unused: Option<i32> };

pub struct TileOption { pub tile: i32, pub label: Msg }
/// Option-index tile ask: one option per roll (or per tile), duplicates kept.
/// Bound to 8 options under Kani.
pub fn ask_tile_pick(
    player_id: i32, title: &Msg, text: &Msg, options: &[TileOption],
) -> Result<usize, Prompt>;
```

`roll_reroll` is the whole card-facing reroll: `roll_current` →
`roll_recast(roll, source)` → `ask_pick(keep_old, keep_new)` → `roll_choose`.
No new import is spent on the prompt; `source` is only passed through.

`ctx::roll_direct` gains the same `source` parameter; `ctx::do_move_roll`
keeps its one-arg signature and passes `roll_source::CARD` (every caller is a
card or skill body), so the ~8 call sites in `rules/` are untouched.

---

## 5. Engine wiring

### 5.1 One cast point

`crates/game-core/src/engine/roll.rs` owns **both** producers:

```rust
fn cast(&mut self, r: &mut RollCtx) -> i32 {
    let value = match RollPurpose::from_i32(r.purpose) {
        RollPurpose::Move => self.roll_tables_for(r.roller), // plan tables + extreme
        RollPurpose::Effect => self.roll_n(r.roller, r.count, r.sides),
    };
    r.casts.push(RollCast { value, settled: None, source: r.source });
    r.chosen = r.casts.len() - 1;
    r.stage = RollStage::Cast;
    raise!(self, "diceRollCast", r.roller, @r r, value = value)?;   // write-back honoured
    value
}
```

`Cx::roll` / `roll_tables` (cx.rs:1360) become thin calls into this producer,
so an effect roll and a move roll can never diverge again. `extreme` handling
from `roll_tables` (play.rs:1578-1591) and `ops.rs::do_move_roll` (506-522)
moves here; `card_do_move_roll` keeps an `extreme`-honouring wrapper.

### 5.2 Ladder (`Match::run_roll`)

```
run_roll(purpose, roller, source, window):
  open or join a RollCtx            (join only: purpose == Move && open Move)
  cast()                            -> raw face; I6 records (roller, value)
  if add applies:  raise "diceRollAdd"  (filter fold; value write-back)
  raise "diceRollValue"             (filter fold; value write-back)
  raise "diceRollAfter"             (Action; may recast/choose)
  if window && purpose == Move: raise "diceRollResult" (source on the payload)
  if tainted: add -> value again   (the settle pass; I3)
  commit: r.stage = Settle; raise "diceRollSettled" (final_face, rerolled,
          chosen, unused, purpose); r.stage = Done
  pop (toplevel) / return to parent (join)
```

`tainted` is set by a recast, by any face write in `diceRollAfter` /
`diceRollResult`, by `roll_choose`, and by a write in `diceRollSettled` — not
by the first cast and not by the Add/Value folds themselves. The settle pass
is capped at 2 passes: a `diceRollSettled` write that changes the face runs
one more Add+Value and is then final (prevents a Settled ↔ resettle loop).

`DiceRollAdd` and `DiceRollValue` are folded through the existing filter
channel (`counteract_orig` step (3)); both are hook-only, so no hand window
opens. `raise!` write-back of the trigger value is how a filter's return
reaches the face (the v79 mechanism, `payload.rs:331-341`, unchanged).

### 5.3 The record

`RollCtx.casts` is committed to `TurnCtx.roll_log: Vec<RollRecord>`:

```rust
#[serde(default)]
pub struct RollRecord { pub roller: i32, pub value: i32, pub purpose: i32, pub source: i32 }
```

`turn_rolls: Vec<i32>` stays, populated identically, so old saves and
`ctx::turn_rolls()` keep working. `rolls_of(p)` reads `roll_log` filtered by
`roller == p`; empty on a save written before this field (documented:
pre-v13 saves have no attribution). `#[serde(default)]`, SAVE_VERSION decision
in §7.

### 5.4 Attribution and the trigger view

`CoreTrigger` gains `roll: Option<RollId>`. `bridge_trigger`
(`wasm_rules.rs:7270-7306`) fills `Trigger.roll: Option<TriggerRoll>` from the
transaction:

```rust
pub struct TriggerRoll {
    pub id: i32, pub purpose: i32, pub source: i32,
    pub rerolled: bool, pub chosen: i32, pub unused: i32,
    pub casts: Vec<(i32, i32)>,          // (value, source)
}
```

Trigger readers (`ctx::trigger`): `roll() -> Option<RollPayload>` (the typed
view, same shape as `trigger::buy()`), plus the conveniences `roll_id`,
`roll_purpose`, `roll_rerolled`, `roll_unused`, all reads. The CEL vocabulary
gains `roll_purpose` / `roll_rerolled` / `roll_unused` (`cond_pre.rs` +
`cond/src/schema.rs` + `vocab.rs`), so a pre can gate on them. `move_*`
readers keep working: a `Move` roll synthesizes the move view from the
transaction (roller = the trigger's `player_id`, `kind = Walk`, `main =
false`, `resolve = false` for a card-driven `Move` roll) so existing
`move.main` / `move.tag` pre-conditions do not silently go false. The
main-move dispatch sets `main = true` on its own transaction.

`source` precedence: a recast's source replaces the transaction's when it is
higher (`Charge` > `Skill` > `Card` > `NONE`). The `diceRollResult` and
`diceRollSettled` raises stamp the transaction's source, so item 9 sees
`CHARGE`.

### 5.5 The option ask

`ask_tile_pick` pushes `opt_tile(tile, label)` per option and calls
`ask(PromptKind::Tile, …)`, then clamps the answer to the option range. The
host already resolves `Tile` to an option index in `ask_tile`
(`ctx.rs:2061-2070`); the new wrapper simply does not map index → tile.
Hagumi builds one option per **roll**, deduping only identical
`(tile, value)` pairs.

### 5.6 Forced play (item 13, demoted)

Only the surface is fixed here; the owner stays ABI-04: `prop::TARGETING`
(+ `prop::NORMAL` for the hand filter) on the `CardDef`, and
`ctx::play_card_targeted(player_id, card, target)` — `play_card` plus a forced
`t.target` for the play's target window. No roll pipeline involvement.

### 5.7 Plan sync after a mid-roll raise

Extract the `fold_plan_delta` call (`play.rs:1654-1670`) into
`MoveCtx::sync_plan(&mut self, before: &Move, plan: &Move) -> bool` and call it
after every mid-roll raise in `main_move` and `card_move_seeded`, before the
resolve / min-roll / walk sequence. `diceRollPlan` already re-clones the plan;
After/Result gain the same treatment. This closes item 7 and every future
"write the plan from a roll hook" card.

### 5.8 Kani model

`rules/kani/src/host.rs` gains a bounded `RollCtx` model (max 4 casts,
symbolic faces from the same producers, symbolic answers):

```rust
roll_recast   => raise the source if higher, push a symbolic cast, return its face
roll_choose   => chosen = index.clamp(0, casts.len() - 1)
roll_face     => casts.get(i).copied().unwrap_or(-1)
roll_unused   => latest unchosen cast or -1
roll_rerolled => casts.len() > 1
roll_current  => innermost open id or -1
rolls_of      => the model log filtered by roller, postcard-encoded
```

Every new op is listed in `rules/kani/build.rs` (the import allowlist) and
re-exported through the per-group `proofs/*.rs` op tables. `ask_tile_pick`'s
symbolic answer uses the existing `kani::any` range-assume pattern.

---

## 6. Card usage — before / after

**1. starry_night.rs**

```rust
// before
let new_roll = ctx::do_move_roll(player_id)?;
trigger::set_move_roll(new_roll);
// after
let r = ctx::roll_reroll(player_id, &title, &ask, &keep_old, &keep_new,
                         roll_source::CARD)?;
// r.chosen is already the settled face; no set_move_roll
```
Guard stays: owner, main, unfixed. The card no longer needs to know the reroll
is raw.

**2. sports_talent.rs** — move the reward entry from
`Action::DiceRollAfter("actor == owner && card.placed && move.main", …)` to
`Action::DiceRollCast("actor == owner && card.placed", …)`; drop the in-loop
`reward()` from `reroll_to_ten`, since every recast now raises
`diceRollCast`. `roll_plan`/`reroll_to_ten` move to a `roll_purpose == 1`
gate (the reroll-to-10 clause is move-only).

**3. taki_even_if.rs** — `ctx::turn_rolls()` → `ctx::rolls_of(player_id)`;
drop the manual `seen.push(before.abs())` (the cast is already recorded) and
the `abs()` mapping stays for signed walks.

**4. taki_even_if.rs** — no body change; `roll_reroll` inherits the extreme
through the shared producer.

**5. want_human.rs** — the armed boost becomes

```rust
On::Filter(Filter::DiceRollAdd(
    "actor == owner && card.placed && roll_purpose == 1 && slot('want_human_boost') > 0", None,
    boost_add))
fn boost_add(_p: i32, v: i32) -> i32 { v + (ctx::slot(p, SLOT_BOOST) - 1).max(0) }
```
The slot is consumed at `DiceRollSettled` (final) instead of `DiceRollAfter`;
the abnormal snapshot also moves there.

**6. want_human.rs** — the ≤X crystal check moves from
`TriggerKind::DiceRollAfter` to `TriggerKind::DiceRollSettled` with
`final_face`, `roll_purpose == 1`, `actor == owner`.

**7. detour.rs** — no body change. `counteract_with_plan` keeps writing
`ctx::plan::*`; §5.7 delivers them. The `NEXT_ROLL` latch keeps its existing
one-shot consume.

**8. hagumi_marks.rs**

```rust
// before: Vec<i32> tiles, dedupe by tile, ask_tile, lookup by tile
// after:
let mut seen: Vec<(i32, i32)> = Vec::new();          // (tile, roll) identity
let mut options: Vec<TileOption> = Vec::new();
for … { let t = (r - 1).rem_euclid(n);
        if seen.contains(&(t, r)) { continue; }
        seen.push((t, r));
        options.push(TileOption { tile: t, label: Msg::new(key!("…")).i("n", r) }); }
let pick = ctx::ask_tile_pick(player_id, &title, &text, &options)?;
let chosen = seen[pick].1;
```

**9. misaki_card.rs** — the pre becomes
`"actor == owner && roll_source == 1 && between(owner) > 0"`; the tag read
(`move.tag('fireRoll')`) is dropped. `raises_effort` needs no change.

**10. moca_half.rs** — the entry changes kind:
`On::Filter(Filter::DiceRollAdd(…, roll_half))` stays `DiceRollValue` for the
half; the pre switches `move.main` → `roll_purpose == 1` (a Move recast and a
non-main forced move are both 「你的移动掷骰」). The TODO and the
OPEN-ROLLRESHAPE-01 comment resolve to "additive first, then this".

**11. no_expectation.rs** — the entry moves from `Filter::DiceRollValue` to
`Filter::DiceRollAdd("card.placed", …, roll_shave)`; `roll_shave` gates on
`roll_purpose == 1` for the 0-floor/no-settle half and drops the floor for
effect rolls (`cut = roll - 2`, unbounded below). Ordering is now the ladder,
so the TODO disappears.

**12. layer_keep.rs** — the `roll_after` entry changes kind from
`Action::DiceRollAfter` to `Action::DiceRollSettled` with the pre
`actor == owner && card.placed && roll_unused >= 0 && roll_purpose == 1`
(the fire reroll is a Move recast); `counter_fire` records nothing itself, the
body reads `roll_unused` and banks it into `SLOT_DICE`. `use_die` keeps
`ask_pick` and `set_fixed_roll`, now with `roll_choose` available if it ever
needs to set the face directly.

**13. please_choose.rs** — filter the hand by `prop::TARGETING` + admits and
offer option (1) only when the list is non-empty; on pick 0 call
`ctx::play_card_targeted(other, card, player_id)`.

**14. doll_garden.rs** — `ctx::roll(p, 0, 0, RollPurpose::Move,
roll_source::CARD)` replaces the `Effect` 1d20; the victim is the roller in
the trigger, `main = false`, and the card still drives the walk with
`plan::set_steps(r)` / `set_resolve(false)`.

---

## 7. Save / record / replay

* **SAVE_VERSION 13 → 14** (second SAVE bump behind G08; take the next free one). The open-roll stack is save-visible state: a
  body can pause on a prompt with a roll open, and the save must restore
  `rolls` (including `casts` / `chosen` / `source` / `unused`). Loading stays
  exact-version (`engine/mod.rs`: `version != SAVE_VERSION` is refused), so a
  v12 save is not restored into v104; `#[serde(default)]` on the new fields is
  there for shape tolerance (an at-rest save may legitimately have an empty
  stack), and `turn_rolls` remains so the legacy aggregate still round-trips.
  Every wrapper treats `roll_current() == None` as "open a fresh transaction",
  which is also what a resumed body with a dropped stack needs.
* **RECORD_VERSION 3 unchanged.** Cast faces come from the serialized RNG
  stream (`docs/REPLAY.md`: both streams travel in the record), and prompt
  answers are already recorded. No new record field. `RollId` is per-run only
  and is never written to a record or a card save slot.
* **Replay.** A recast replays as the same RNG draw. The transaction's
  `casts` vector is rebuilt by the re-run; only the answers (`roll_choose`
  index, `ask_tile_pick` index) come from the log. Archived engine bundles are
  unaffected: kinds 123-125 are additive, and 102 keeps its id and wire name, but its **stage changes** from the value
point to the settlement point -- a listener registered on it sees a different
face and timing. That is the reason this group bumps the version at all.

## 8. Versioning

`ABI_VERSION 108 → 109` (fifth group in the README landing order; take the next
free number if the order moves). Wire break, named in the doc comment
immediately below the constant and above the newest note:

```
/// v109: **roll transaction + settlement stages**. A roll is a `RollCtx`
///    carrying roller / purpose / source / every cast; every cast runs
///    `diceRollCast` -> `diceRollAdd` (new filter 125, the additive pass) ->
///    `diceRollValue` (now the final-settlement pass, id/wire unchanged) ->
///    `diceRollAfter` -> `diceRollResult` -> `diceRollSettled` (new Action
///    124). `diceRollCast` is new Action 123. `ruleset_roll` joins an open
///    Move transaction instead of always casting standalone; `ruleset_roll_direct`
///    gains the `source` arg. New imports `roll_recast`, `roll_choose`,
///    `roll_current`, `roll_face`, `roll_unused`, `roll_rerolled`,
///    `rolls_of`; guest wrappers `ctx::roll_reroll` / `ask_tile_pick`.
///    `PromptKind::Tile` answers the option index (unchanged shape) and its
///    options may repeat a tile. `Trigger.roll` payload added; `roll_source`
///    now reaches `diceRollResult`. SAVE_VERSION 13 -> 14 (roll stack is
///    saved). RECORD_VERSION unchanged.
```

`ManifestOn` is unchanged (no new `OnKind`). `kind_table_pin.rs`,
`kinds.rs`'s `KINDS` count, `on_parse.rs` and the CEL vocabulary table are
updated with the three kinds.

## 9. Tests (per item) and Kani obligations

Black-box tests take faces from `t.dice(&[…])` and assert positions / money /
slots.

| item | test |
|---|---|
| 1 | `rules/tests/tests/morfonica.rs::starry_reroll_reaches_the_final_settlement_stage` — 0.5倍速 placed, first face 7, reroll face 7 ⇒ settled 4; walk length 4 |
| 2 | `hhw.rs::athletic_pays_on_effect_rolls_and_non_main_moves` — effect roll pays; forced walk pays; main-only reward removed |
| 3 | `crychic.rs::taki_counts_only_the_owners_rolls` — opponent effect roll does not block |
| 4 | `crychic.rs::taki_reroll_settles_at_the_armed_extreme` |
| 5 | `crychic.rs::want_human_boost_survives_a_reroll` — 7 + (20−X) then reroll 3 ⇒ settled 3 + boost |
| 6 | `crychic.rs::want_human_banks_a_rerolled_low_face` |
| 7 | `ag.rs::detour_counter_plan_writes_reach_the_walk` — reverse/start/teleport rewrite changes the walked path |
| 8 | `hhw.rs::hagumi_offers_one_option_per_roll_even_on_one_tile` — ≤60 and >60 faces on one tile both reachable |
| 9 | `hhw.rs::misaki_only_after_a_fire_funded_move_roll` — plain roll refuses, fire reroll admits; `roll_source` on the result payload |
| 10 | `gap_money.rs::g02_half_speed_plus_expectations_floor_on_a_rent` — retarget to 7 → 5 → 3 (landing moves by one) |
| 11 | `pp.rs::no_expectation_shaves_effect_rolls_and_card_rerolls`; move floor stays 0 and sets `resolve = false` |
| 12 | `ras.rs::layer_keep_banks_the_unchosen_face_after_the_pick` — fire reroll, pick old ⇒ `unused` = new face, one entry banked, `use_die` offers it later |
| 13 | `ras.rs::please_choose_opt1_plays_a_targeting_card_at_the_counteractor` |
| 14 | `mujica.rs::doll_garden_victim_roll_takes_the_victim_modifiers` — no_expectation shaves the victim's roll; a victim's `DiceRollAfter` hook runs |

ABI-level: `crates/game-rules/tests/kind_table_pin.rs` (123-125),
`crates/game-rules/tests/live_match.rs` (ladder order: Cast → Add → Value → After
→ Result → Settled; `roll_source` on Result; `unused` on Settled),
`rules/tests/tests/split_hook_entries.rs` (no_expectation now `DiceRollAdd`),
`rules/kani/tests/on_parse.rs` (`BodyShape::Roll` for `DiceRollAdd`),
`crates/card-sdk/src/kinds.rs` (KINDS count 27 → 28).

**Kani obligations** (proofs in `rules/kani/src/proofs/*.rs`, specs from the
quotes only; PROTOCOL-LEVEL where the ladder is engine-side):

* `talent_pays_exactly_once_per_cast_iff` — one reward per `diceRollCast`,
  none on a non-cast.
* `want_human_boost_is_added_iff_armed` and
  `want_human_banks_one_crystal_iff_final_face_le_x`.
* `taki_counts_only_its_own_casts` (model log + `rolls_of` stub).
* `hagumi_each_option_maps_to_one_distinct_roll` — option count == distinct
  casts; `ask_tile_pick` clamp keeps the index in range.
* `layer_keep_banks_exactly_the_unchosen_face` — `unused` ≠ `chosen`; the
  slot receives the former.
* `no_expectation_shaves_iff_purpose_is_move` (and the effect-roll branch
  unbounded below).
* `moca_halves_iff_move_and_after_the_add_pass` — spec pairs
  `(7, −2) → 3`.
* stub-totality proof: every new `Op` has an arm (the `kani_host.rs` match is
  exhaustive, so the compiler carries this; a test asserts the build list and
  `Op` enum agree).

## 10. Alternatives

1. **One `DiceRollValue` with `stage: Add | Final` on the payload**, instead
   of a second kind. Fewer kinds, but every `match` and CEL pre must switch on
   the stage, and the two stages have different purity contracts (Add is
   unbounded below, Final clamps). Two kinds keep the fixed order in the
   engine instead of in card text. Rejected.
2. **No automatic re-settle; an explicit `ctx::roll_settle(roll)` the reroll
   card must call.** Simpler engine, but every reroll card must remember to
   settle (items 1, 5, 6, 12 all silently regress), and a card that forgets
   produces the exact bug this group closes. Rejected.
3. **Return a roll handle from `ctx::roll` and break every caller.** Cleaner
   than `roll_current()`, but ~30 call sites change and v103 just landed the
   `Result<i32, Prompt>` shape. Rejected for this batch.
4. **New `PromptKind::Options` answering an index.** Explicit, but needs a
   client kind, a wire string and an answer-log kind. `Tile` already answers
   an index and its UI fits the roll-to-tile options. Rejected.
5. **Keep `turn_rolls` and add a parallel `turn_rollers: Vec<i32>`.**
   Cheap, but two vectors can desync; `roll_log` with one row per cast is a
   single source. Chosen.

## 11. Open questions

1. **`DiceRollSettled` placement in signed walks.** A signed (reverse) walk's
   `unused` face is signed while `chosen` is compared unsigned by
   `taki_even_if`. The design publishes both raw; the card does the `abs()`.
   Confirm.
2. **Is a reroll an 「掷骰」 for every "each roll" clause?** 运动的天赋 says
   「每次掷骰」 and the design fires one `diceRollCast` per cast, so yes. If a
   ruling says only the final face counts, sports moves to `DiceRollSettled`
   and the ladder is unchanged.
3. **`roll_source` precedence** when a skill-funded recast follows a
   card-funded one. The design keeps the highest (Charge > Skill > Card);
   the book names charges only, so this is a judgement.
4. **A recast inside `DiceRollAdd`/`DiceRollValue`** (a pure filter cannot
   recast today; `roll_recast` is a mutating ctx call). Is a filter-side
   recast ever wanted, or is `DiceRollAfter` always enough?
5. **`rolls_of` scope.** This design is turn-scoped to match
   `turn_rolls`. 「本回合内」 is the only clause; confirm no card needs a
   match-scoped history.
6. **Save-shape choice.** `roll_log` is a second vector next to
   `turn_rolls`. A later cleanup could migrate `turn_rolls` to a projection
   and bump SAVE_VERSION once; out of scope here.

---

## 12. Item 15 — `own-stage-answer` (rows 671, 672, 673, 674, 1455)

The fifth consolidated item of this group, missed by the first draft. It is not
a roll item; it rides here because both halves are prompt answers taken on an
abnormal-move window.

### 12.1 The card and its fragment

`rules/cards/card-roselia/src/own_stage.rs` (`R:选择自己的舞台`), sheet row 671:

> 选择自己的舞台：
> [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算

Rows 672–674, 1455 are the halves: `resolveOverride` and `stageEnd` are C#
`H.SetV`/`H._resolveOverride` slots that no current reader consumes.

### 12.2 The gap (today's tree)

The counteraction body branches on the abnormal kind and writes
`ctx::set_slot(player_id, "resolveOverride" | "stageEnd", …)`. Nothing reads
either slot:

* the move always settles (`after_walk` only consults `Move.resolve`, which no
  card answers), so "no" changes nothing;
* a blocked move settles at the square on landing today (a 0-step walk still
  runs `after_walk`), and no settle is scheduled for a blocked turn end, so
  the 「无法移动的回合结束时」 half has nowhere to land.

### 12.3 Capability

Two generic, answerable flags, both read by the engine after the window and
cleared with the thing they belong to:

```rust
// ctx, both valid inside the abnormal move's `effect`/`card` counteraction
// window; both are the *answering seat's* own move/turn.
pub fn move_answer_resolve(yes: bool);        // 「本次移动…是否触发结算」
pub fn turn_end_settle_answer(yes: bool);     // 「无法移动的回合结束时」 half
pub fn move_resolve() -> bool;                // read-back (tests, later cards)
pub fn turn_end_settle() -> i8;               // 0 none, 1 settle, 2 skip
```

`move_answer_resolve(false)` writes the pending `Move.resolve = false`
(`move_ctx.rs:116`), which `after_walk` already honours
(`play.rs:2048`); no new engine check. `turn_end_settle_answer` writes one new
`TurnCtx` byte.

Blocked-turn default: the engine sets `turn.blocked_settle = 1` when a stay/stun
abnormal ends a move with no displacement (the normal settle at the square is
cancelled for that move); leaving the card unplayed therefore keeps "settle at
the end of the turn". The card's "yes" also writes 1, its "no" writes 2, so a
blocked landing never both settles at the square and at turn end.

### 12.4 Engine wiring

* `TurnCtx.blocked_settle: i8` (`#[serde(default)]`, 0 = none) — rebuilt with
  the turn like every other `TurnCtx` field.
* In the walk tail, when the abnormal was stay/stun and the piece did not move:
  set `m.resolve = false` for the blocked landing and `turn.blocked_settle = 1`.
* `end_turn` (`play.rs:725`), before `turnEndBefore`: if
  `turn.blocked_settle == 1` and the seat is live, `self.settle(i, &Move-only
  frame)`; 2 skips. Clear to 0 after.
* `Move.resolve` is already saved with parked work; only `blocked_settle` is a
  new saved byte, and `TurnCtx` carries it with `serde(default)` — SAVE stays
  at this group's 13.

### 12.5 Card usage

```rust
// before
ctx::set_slot(player_id, "stageEnd", if yes { 1 } else { 2 });
ctx::set_slot(player_id, "resolveOverride", if yes { 1 } else { 0 });
// after
if matches!(abnormal_kind(), Some(Stay) | Some(Stun)) {
    // TODO(规则书): the kind alone decides the branch; a [停留] walked through
    //   under 「不可阻挡」 still lands here -- confirm with 12.7 Q2.
    ctx::turn_end_settle_answer(yes);
} else {
    ctx::move_answer_resolve(yes);
}
```

The engine sets the "move was blocked" default, so the card no longer depends
on the slot living long enough; the two slot keys leave the card.

### 12.6 Tests and Kani

* `g03_own_stage_no_suppresses_the_landing_settle` — abnormal teleport, card
  answers no: no `settleBefore`, position still changed.
* `g03_own_stage_yes_on_a_blocked_stay_settles_at_turn_end` — forced stay,
  answer yes: exactly one settle, at `end_turn`, on the square.
* `g03_own_stage_no_on_a_blocked_stay_skips_the_turn_end_settle`.
* `g03_own_stage_default_without_the_card_is_settle` — the engine's own
  default (1) runs one turn-end settle.
* Kani: `resolve_answer_false_raises_no_settle` over the model's walk tail.

### 12.7 Open questions

1. **Does "yes" on an unblocked move force a settle the engine would otherwise
   skip** (e.g. after an [除外]-gated landing)? Design: yes, it only writes the
   existing `resolve` flag.
2. **Is the branch the abnormal kind alone, or "the move was actually
   blocked"?** Design keeps the kind branch; a walk-through under
   「不可阻挡」 still takes the turn-end half.
3. **Turn-end settle order** against 「回合结束时额外进行一次[触发结算]」
   scheduled rules ops: the design puts it before `turnEndBefore`.
