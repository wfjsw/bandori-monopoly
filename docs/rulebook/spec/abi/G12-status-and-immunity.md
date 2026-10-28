# ABI design G12 — Status, blocked and immunity gates

**Status:** draft for user review · design only, no source touched.
**Worktree:** `.claude/worktrees/card-rulebook-review` (branch `worktree-card-rulebook-review`).
**Group:** G12, family "Status, blocked and immunity gates".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below is verbatim from it.

Terminology: **blocked-ness** = whether a landed abnormal actually holds this
turn's move, a *derived* predicate. **Kind** = the `AbKind` wire code
(1 stay / 2 stun / 3 exile / 4 teleport / 5 forced / 6 stop / 7 reverse).
The whole group exists because the engine confuses the two, and because the
ruleset's `<status>` key is invisible to the engine on three paths.

Status hosts live in `rules/status`; the engine keeps only generic verbs
(`docs/STATUS-HOSTS.md`). The engine must neither name `stay` nor switch on a
kind.

**Closed slugs and rows:** all 5 items are covered.

| # | slug | severity | rows |
|---|---|---|---|
| 1 | `move-gate-replay-read` | high | 225, 988, 1498 |
| 2 | `unstoppable-blocked-read` | medium | 403, 404, 1035, 1454 |
| 3 | `major-abnormal-scope` | medium | 1098, 1171 |
| 4 | `immunity-causer` | medium | 608, 1412 |
| 5 | `dice-immunity` | medium | 710, 1462 |

**No item demotes to A.** Every item needs at least one engine-side fact the
current ABI cannot express (a persisted activation/span fact, a ruleset gate
kind, a causer-aware immunity reader, or a turn dice flag). The three
card-body fixes already in this worktree (tsugu, detour, shop_friends) stand as
the *consumer* side of item 1, not as a replacement for it.

---

## 1. Problem, per item

### 1.1 `move-gate-replay-read` (rows 225, 988, 1498)

Book (`通用:@Tsugu ycm`) — the only fragment that needs the main move at all:

> @Tsugu ycm：
> [手]：
> 投掷3d10并根据结果依次进行以下操作：
> 1. 结果至少为22则抽1张卡；
> 2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；
> 3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；
> 4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。

Only clause (2) (and its (4) twin) rewrites the main move. Today the card
gates the **whole play** through `cant_play`:

```rust
// HEAD before this worktree's fix: rules/cards/card-general/src/tsugu_ycm.rs
fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)   // → ctx::cant_move_code(player_id) != 0
}
```

so a spent main move or a [停留] latch refuses the roll, the draw, the discount
and the gain too. The worktree fix drops `cant_play` and guards only the (2)
arm in the body (`tsugu_ycm.rs:116-124`), but it has to lean on a hand-rolled
replay marker because **no engine read survives a body re-run**:

```rust
let moved_here = ctx::player_pos(player_id) == station
    || ctx::slot(player_id, MOVED) == ctx::turn_key();   // MOVED = "tsugu_ycm_moved"
if moved_here || rules_status::move_blocked(player_id).is_none() { … }
```

`ctx::cant_move_code` (`crates/card-sdk/src/ctx.rs:1905`) reads the live world
via `RulesBridge::cant_move` (`crates/game-rules/src/wasm_rules.rs:2151-2163`:
`1` not your turn / `2` already moved / `3` stay latch). A card body is
interrupted by host requests and **re-runs from the top** with the host effect
already applied (`docs/ENGINE.md` "Drive replay semantics"), so on the landing
pass `main_moved` is already true: a naive `move_blocked` read answers `2` and
skips the teleport it just performed. The existing test
`rules/tests/tests/general.rs:229` (`tsugu_effects_land_when_the_move_is_spent`)
passes because this worktree's fix already drops the whole-play gate and the
in-body guard skips the teleport on the [停留] latch — it exercises neither a
spent main move nor a re-run in the non-stay case, which is where the slot
marker is load-bearing.

`detour.rs:71` and `shop_friends.rs:32-36` are already migrated to
`ctx::cant_move_code(...) != 3`; at counteraction time there is no activation
body and no re-run, so they can stay. `tsukushi_garden.rs:94-100` repeats
tsugu's slot workaround (`MOVED = to + 1`) for its move arm.

### 1.2 `unstoppable-blocked-read` (rows 403, 404, 1035, 1454)

Book (`Mor:（筑紫）迷茫的庭园`):

> （筑紫）迷茫的庭园：获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动

Book (glossary 14, `docs/rulebook/rulebook-doc.md:99`) — what that state means:

> \[不可阻挡\]：处于该状态时无视已存在的\[停留\]，\[晕眩\]效果也不会获得新的层数；无法获得新的\[除外\]层数。\[停留\]，\[晕眩\]，\[除外\]在适当时机依旧掉层。处于该状态时可选择受到的\[传送\]，\[强制移动\]，\[强制停下\]效果是否生效。

`Cx::abnormal_gate` (`crates/game-rules/src/wasm_rules.rs:5537-5635`) computes
the movement flag and **never reads it**:

```rust
let movement = matches!(
    kind,
    crate::AbKind(4) | crate::AbKind(5) | crate::AbKind(6) | crate::AbKind(7)
);
// The ruleset's grant verbs refuse while its 「不可阻挡」 clause holds
// (STA-01) -- checked before this window opens. The engine does not
// name that key.
```

The comment at 5550-5563 describes three clauses as if implemented; only the
grant-verb refusals for stun/exile exist (STA-01, `rules/bandori-std/src/status.rs:184-217`).
A received teleport, forced move, stop or reverse therefore lands on an
[不可阻挡] holder, and `tsukushi_garden.rs:139-145` (TODO) grants a protection
the engine ignores. `rules/status/src/unstoppable.rs:18-19` carries the
per-effect opt-out as STA-02.

Second half of the item: `own_stage` branches on the **kind**, not blocked-ness:

```rust
// rules/cards/card-roselia/src/own_stage.rs:74-85
match abnormal_kind() {
    Some(bandori_std::kind::abnormal::Stay) | Some(bandori_std::kind::abnormal::Stun) => {
        ctx::set_slot(player_id, "stageEnd", if yes { 1 } else { 2 });
    }
    _ => { ctx::set_slot(player_id, "resolveOverride", if yes { 1 } else { 0 }); }
}
```

A [停留] that lands while the player is [不可阻挡] does not hold the move
(`STAY_HOLDS_MOVE = "state(actor,'stay') > 0 && state(actor,'unstoppable') == 0"`,
`rules/status/src/stay.rs:72`), yet the code takes the turn-end branch. Nothing
on the trigger/effect payload says whether the landing actually blocked.
`guards.rs:151-176` pins the current kind branch.

### 1.3 `major-abnormal-scope` (rows 1098, 1171)

Book (`CRYCHIC:想要成为人类` (2)):

> \[自动\]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。

「该次移动过程中」 is **that move's span**. Today the card snapshots the
turn-wide counter at the boost consumption (`want_human.rs:115-121`,
`ctx::abnormal_count`) and diffs it at `TurnEnd` (`:164-177`):

```rust
ctx::set_slot(player_id, SLOT_AB_BEFORE, ctx::abnormal_count(player_id) + 1);
…
if ctx::abnormal_count(player_id) > ab_before - 1 { ctx::add_crystals(2, 0)?; }
```

A snapshot taken at `diceRollAfter` misses an abnormal that landed earlier in
the same move (a `diceRollPlan` / `diceRollValue` effect) and counts every one
that lands after the walk up to `TurnEnd`. `cross_move::m23` only exercises the
happy path (stop mid-walk). The `TurnCtx.abnormal` counter itself
(`world.rs:169-172`, bumped at `wasm_rules.rs:5624-5628`) is turn-wide and has
no span.

### 1.4 `immunity-causer` (rows 608, 1412)

Book (`PP:再次闪耀` [特]):

> 此卡不受除拥有此卡的玩家以外的玩家的效果影响。

C# `Card.Immune` + the clause's owner carve-out. The engine stores one bool,
`FieldCard.immune` (`state.rs:1108-1110`); the readers are
`is_immune_at(uid)` (`ctx.rs:3845`) / `card_immune(player, card)` (`:3529`) and
carry **no causer**. `shine_again.rs:86-94` has to set the plain
「不受任何效果影响」 flag and TODO the difference:

```rust
ctx::set_self_immune(true);
```

so the owner's own effects (e.g. `title_idol.rs:59-65` adding crystals to its
own cards) skip the card too. The same gap affects any future 「除拥有者以外」
wording; `no_expectation.rs:111` / `auction_pulled.rs:69` use the any-flavour
legitimately and must keep working.

### 1.5 `dice-immunity` (rows 710, 1462)

Book (`RAS:（MASKING）CRUSH ON THE DRUM!!!` [特]):

> \[特\]：
> 打出此卡的回合任何\[使用者\]使用的卡包含添加骰效果的卡不受任何其他效果影响。

C# sets `H._turnCtx.DiceCardsImmune = true` and the play flow skips
`Counteract` for any `def.AddsDice` card. Nothing of this exists:
`crush_drum.rs:55-64` only adds Xd20 to the plan and TODOs the clause; there is
no turn flag, no `AddsDice` on the play payload, and the `card` counteraction
window (`play.rs:4814-4821`) opens for every hand play. `OPEN-questions`
`OPEN-crush-immune` records the scope question.

---

## 2. Shared root causes

1. **Kind used as a proxy for effect.** `own_stage` keys on Stay/Stun; the
   gate's `movement` set (4|5|6|7) is the only kind test. Nothing produces the
   orthogonal question "did this landing hold the move".
2. **The engine cannot name a status key.** `abnormal_gate` says so at 5568-5570
   although `[不可阻挡]`'s clause is exactly an engine gate predicate. Status
   knowledge must reach the engine through a rule-evaluated gate, not a key.
3. **Body reads are replay-unstable.** A card-body read of `cant_move` is true
   on pass 1 and false on the landing pass; every card re-invents a marker
   slot. The activation needs an engine-stamped, pass-stable view.
4. **Immunity has one causer-blind flavour.** `immune` is a single bool and
   every reader ignores `by`, although all engine gate sites already carry a
   causer.
5. **Turn-scoped "this turn" facts are ad hoc.** `TurnCtx` holds per-turn
   latches, but there is no generic dice-immunity flag and no move-scoped
   abnormal span; cards snapshot turn-wide counters instead.

## 3. Capability as an invariant

The engine must satisfy all of the following, with the ruleset deciding every
policy question:

1. **`[不可阻挡]` is a gate the abnormal path asks.** For a movement abnormal
   (kinds 4|5|6|7) caused by *another* player, `abnormal_gate` asks a CEL-only
   ruleset gate and refuses the landing when the ruleset closes it. A refused
   landing never opens the `effect` / `abnormal` windows, never bumps
   `turn.abnormal`, never sets `ABNORMAL_HIT`. A self-caused move is chosen and
   bypasses the question.
2. **Blocked-ness is a payload fact, independent of the kind.** Every abnormal
   declaration carries `blocked` — true iff this landing would hold the
   mover's current move. It is derived from the move gate, the walk exemption
   and the remaining-move latch, never from the kind; teleport is never
   blocked.
3. **An activation has a stable main-move view.** `cant_move_code_for_activation`
   answers as `cant_move_code` would have answered when the activation started,
   on every replay pass, regardless of the move the activation itself made.
4. **Immunity is causer-aware.** A read distinguishes "any effect" from
   "except the owner's"; the owner's own effects pass an except-owner card.
5. **A dice-immunity turn exists.** A card can raise a turn flag; while it is
   up, a hand play of a card whose declared props contain `ADDS_DICE` skips the
   `card` counteraction window (the card's own body still runs).
6. **A move-scoped abnormal span exists.** A move records the abnormal counter
   at its start; every trigger carrying that move exposes whether the counter
   rose since — independent of the turn counter's later growth.

## 4. Surface

### 4.1 Abnormal gate kind — `crates/game-core/src/engine/rules.rs`, `crates/game-rules/src/gate.rs`

`Gate` gains a payload variant (raw `i32`; game-core does not depend on
card-sdk, same rule as `state::prop`):

```rust
/// May an abnormal effect of this kind (an `AbKind` wire code) from another
/// player land on the seat? CEL-only. Payload: `value` = the kind.
Abnormal(i32),
```

`gate.rs::kind_of` maps it to `TriggerKind::AbnormalGate`; the window builder
sets `win.value = kind`. `GATE_KINDS` (`host.rs:874`) gains the entry so
`validate_cel_only_gates` enforces CEL-only, and its fixed `[TriggerKind; 7]`
type grows to `[...; 8]`. `StubRules` (no opinion) means
open, like every gate.

New engine kind, wire id **107** (free: 104 is retired and 107 was never used;
the other groups' 111+ ids are bands in landing order — see 8):

```rust
/// `abnormalGate` -- may an abnormal of this kind land on the seat?
/// Filter, CEL-only, hook-only. Payload: the seat, `value` = the `AbKind`.
pub const AbnormalGate: Self = Self::engine(107);   // FilterKind
```

Distinct from the existing `abnormalGuard` hook: the guard is a placed card's
pre-declaration block; the gate is a turn-flow capability evaluated natively.

Not new: `ctx::gate(player, kind)` already routes here
(`hostfns::gate` → `abnormal_gate`), so every caller honors the refusal for
free.

### 4.2 Payload fields — `game_core::engine::rules`, guest `world::Trigger`/`TriggerMove`

`Trigger` (engine `rules.rs:231`, guest `game-rules/src/world.rs:35`) gains:

```rust
/// The play activation this raise belongs to; 0 = outside a play activation.
/// Stamped at the `card` raise from `turn.played.len() + 1`.
pub activation: i32,
/// Did the abnormal this trigger announces hold the move? Set on the
/// `abnormalGuard`, `effect` and `abnormal` raises; meaning only on an
/// abnormal. The `effect` window's `Effect` entry carries the same bit.
pub blocked: bool,
```

`TriggerMove` (engine `rules.rs:52`, guest `world.rs:95`) gains:

```rust
/// Did this move receive an abnormal effect since its start? `turn.abnormal`
/// compared against the move's own `abnormal_start`.
pub abnormal: bool,
```

`Effect` (engine `rules.rs:206`) gains `pub blocked: bool` — stamped only on
the `abnormal` entry so `effect::blocked(i)` mirrors `effect::value(i)`; every
other effect literal writes `false`. Only seven literals exist.

### 4.3 Persisted state

`MoveCtx` (`move_ctx.rs:79`, serialized inside `TurnCtx.plan`; the in-flight
walk's copy is re-derived from the routine snapshot):

```rust
/// `turn.abnormal` for the mover when this move started.
#[serde(default)]
pub abnormal_start: i32,
/// The play activation that asked for this move; 0 = engine-requested.
#[serde(default)]
pub activation: i32,
```

`TurnCtx` (`world.rs:143`, right after `abnormal`):

```rust
/// The activation that consumed this turn's main move; 0 = none (a roll).
#[serde(default)]
pub move_activation: i32,
/// 「打出此卡的回合」: hand plays of dice-adding cards skip the `card` window.
#[serde(default)]
pub dice_immune: bool,
```

### 4.4 Props — `crates/card-sdk/src/abi.rs`, `crates/game-core/src/state.rs`

```rust
/// 「此卡不受除拥有此卡的玩家以外的玩家的效果影响」: `1` = the placed
/// instance is immune to effects whose causer is not its owner
/// (`FieldCard.immune_except_owner`). Stamped at placement from the card's
/// static props. Mirrors `game_core::state::prop::IMMUNE_EXCEPT_OWNER`.
pub const IMMUNE_EXCEPT_OWNER: &str = "immuneExceptOwner";

/// 「卡包含添加骰效果」: `1` = the card's effect adds dice to a move
/// (`plan::add_extra_dice`). Read by the play path through
/// `CardRules::card_prop`, never by prose.
pub const ADDS_DICE: &str = "addsDice";
```

Both mirrored in `game_core::state::prop` (`state.rs:412`). `addsDice` is a
declared prop, not a new `ManifestEntry` field — props are already a generic,
opaque map, so the manifest *shape* does not change.

`FieldCard` (`state.rs:1096`) gains next to `immune`:

```rust
/// 「不受除拥有者以外的效果影响」; read through `is_immune_from_at`.
#[serde(default)]
pub immune_except_owner: bool,
```

Placement stamps it from the prop exactly like `immune` (`ops.rs:1227-1229`).

### 4.5 Imports — `crates/card-sdk/src/ctx.rs`, re-exported via `bandori_std::ctx`

```
trigger::activation() -> i32
trigger::move_abnormal() -> bool
trigger::abnormal_blocked() -> bool
effect::blocked(i: i32) -> bool                 // `pub mod effect`
cant_move_code_for_activation(player_id: i32, activation: i32) -> i32
is_immune_from_at(uid: i32, by: i32) -> bool
set_self_immune_except_owner(on: bool) -> bool  // whether the instance exists
dice_immune() -> bool
set_dice_immune(on: bool) -> bool               // returns the previous value
```

Semantics:

* `cant_move_code_for_activation` — same code space as `cant_move_code`
  (`1` not your turn, `2` already moved, `3` move gate holds, `0` free), except
  that "already moved" is suppressed when `turn.move_activation == activation`:
  ```rust
  if self.world.st.turn != player_id { return 1; }
  if self.world.st.skip_move { return 3; }
  if self.world.turn.main_moved && self.world.turn.move_activation != activation { return 2; }
  0
  ```
  Stay is checked before own-move suppression: a [停留] turn is blocked even if
  the activation managed a teleport.
* `is_immune_from_at(uid, by)` —
  `f.immune || (f.immune_except_owner && by != f.owner)`. `is_immune_at`
  remains the any-flavour read (bookkeeping sites keep it).
* `trigger::move_abnormal` — `mv.abnormal`; false without a move.
* `effect::blocked(i)` / `trigger::abnormal_blocked` — the payload bit; false
  outside an abnormal entry / announcement.
* `set_dice_immune` / `dice_immune` — the turn flag; the setter is idempotent.

### 4.6 Classification

| element | class |
|---|---|
| `Gate::Abnormal` + `AbnormalGate` kind + `GATE_KINDS` | new mechanism (wire id) |
| `Trigger.activation`, `Trigger.blocked`, `TriggerMove.abnormal`, `Effect.blocked` | wire fields (break) |
| `MoveCtx.abnormal_start`, `MoveCtx.activation`, `TurnCtx.move_activation`, `TurnCtx.dice_immune` | persisted, serde default |
| `FieldCard.immune_except_owner`, two props | persisted, serde default / opaque key |
| nine imports above | new imports (break) |
| detour / shop_friends `cant_move_code` != 3 | stays |

## 5. Engine wiring

### 5.1 The refusal (`wasm_rules.rs:5548-5595`)

After the out check and before `untargetable`, consume `movement`:

```rust
// 「可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」 (glossary 14):
// a movement abnormal from another player is refused while the status host's
// gate holds. A self-caused move is chosen, not 「受到的」.
if movement && by != player_id
    && !cx.world().gate_open(self, super::Gate::Abnormal(kind.as_i32()), s)
{
    // `log.abnormal_refused` is a new key; `value` carries the kind.
    cx.log(
        player_id,
        Msg::new("log.abnormal_refused")
            .player_id("who", player_id)
            .i("kind", i64::from(kind.as_i32())),
    );
    return Ok(false);
}
```

The grant verbs keep their own refusals (stun/exile, STA-01) because they
run before the window; this gate only adds the movement half. STA-02 (the
player's opt-out prompt) later replaces the blanket refusal with an ask; the
gate is its decision point.

### 5.2 Blocked computation (`same function`, before the payload closures at 5571)

```rust
let exempt = cx.world().st.players[s].state_get(key::MOVE_EXEMPT) > 0;
// The skip latch and `main_moved` are the *turn player's*; the move gate is
// per-seat, so it also answers for a player hit off their turn.
let on_turn = cx.world().st.turn == player_id;
// `main_moved` is set at the *start* of the main walk, so it only means
// "already moved" for a play other than the one that walk belongs to --
// otherwise every abnormal landing during the walk would read as blocked.
let moved_earlier = cx.world().turn.main_moved
    && cx.world().turn.move_activation != cx.trigger().activation;
let blocked = kind != crate::AbKind(4)
    && ((on_turn && (cx.world().st.skip_move || moved_earlier))
        || (!exempt && !cx.world().gate_open(self, super::Gate::Move, s)));
```

Stamp it in the shared `on` closure (`t.blocked = blocked`, 5571-5575), so the
`abnormalGuard`, `effect` and `abnormal` raises all carry it, and on the
`Effect { kind: "abnormal", … }` literal (5602-5608). Teleport is never
blocked; forced/stop/reverse are blocked exactly when the move clamp applies.

### 5.3 Activation stamp

* `play_from_hand` (4814): `let mut t = Trigger::new("card", i); t.activation = self.w.turn.played.len() as i32 + 1; …` — the ordinal is the play count, pushed once per play (`play.rs:4817-4819`, turn player only) and never re-pushed on a pass, so it is stable across the window's passes and differs per play. Off-turn plays share the current length with the last on-turn play; Q4 asks whether that needs its own counter.
* `HostRequest::Move` handler (3785): `let mut plan = plan; plan.activation = trigger.activation;` before `cx.card_move`.
* `play.rs::card_move` (789-817): when `as_main` sets `main_moved`, also `self.w.turn.move_activation = plan.activation;`.
* `main_move` (1370-1381) and `card_move_seeded`: `m.abnormal_start = self.w.turn.abnormal.get(i)…`.
* `Trigger::with_move` (`rules.rs:357`) gains a `now: i32` parameter and sets
  `mv.abnormal = now > m.abnormal_start`. The `raise!` macro (`rules.rs:398-414`)
  passes `$cx.w.turn.abnormal.get($m.player_id)`.
* `bridge_trigger` (7283-7330): carry `activation`, `blocked`, `mv.abnormal`
  into the guest view.

### 5.4 Immunity

`World::is_immune_from_at(uid, by)` next to `is_immune_at` (`ops.rs:1487`).
`set_self_immune_except_owner` writes `current_uid`'s field; placement reads
the prop (`ops.rs:1229`). Migrate the effect-touch consumers to the causer
read: `title_idol.rs:59`, `crystal_swap.rs:112` (its `slots()` takes the
acting player). `mumei_streamer.rs:162` keeps `is_immune_at` — there it is a
bookkeeping marker for cards the skill flipped, not an effect touch.

No change to the seat-wide `immune()` gate (`wasm_rules.rs:5635`): a
card-scoped immunity does not make the player immune to payments or
abnormals.

### 5.5 Dice immunity (`play.rs:4827-4834`)

```rust
let dice_immune = self.w.turn.dice_immune
    && self.rules.card_prop(id, card_sdk::abi::prop::ADDS_DICE) != 0;
let mut t = if dice_immune {
    let mut t = Trigger::new("card", i);
    t.card = Some(id.to_string());
    t.cards = vec![id.to_string()];
    t.by_card = Some(i as i32);
    t
} else {
    raise!(self, "card", i, card = Some(id.to_string()), cards = vec![id.to_string()], by_card = Some(i as i32))?
};
t.activation = self.w.turn.played.len() as i32 + 1;    // same stamp either way
```

The hand-built trigger must carry `activation`: the refused play goes on to
raise abnormal entries, and `cant_move_code_for_activation` reads that stamp to
recognise "already moved on this play". Without it a dice-immune activation
answers 0 and its own main walk reads as an earlier move.

The `twoCards` raise still runs (it is about the play action, not the card).
`crush_drum.rs` calls `ctx::set_dice_immune(true)` in its play body. The flag
dies with `TurnCtx` (rebuilt each turn); nothing else clears it.

### 5.6 Kani model

`rules/kani/src/host.rs` world model gains `field[i].immune_except_owner`,
`turn.dice_immune`, `turn.move_activation`, `trig.activation`, `trig.blocked`,
`trig.mv_abnormal`; the new `Op`s get loud-fail arms first, then models. The
engine's `abnormal_gate` is host logic and is covered by Rust tests; the proofs
cover the card bodies against the model.

## 6. Card usage, before / after

**1.1 tsugu (and tsukushi)** — before: slot marker `MOVED` + landed-position
test + a live `move_blocked`. After:

```rust
let activation = trigger::activation();
let station = ctx::tile_named(STATION);
…
if station >= 0 && ctx::cant_move_code_for_activation(player_id, activation) == 0 {
    plan::set_kind(MoveKind::Teleport); …; ctx::card_move(player_id);
}
```

(4)'s arm and `tsukushi_garden`'s move arm use the same read; the `MOVED`
constants and `rules_status::move_blocked` disappear from both bodies. Detour
and shop_friends are unchanged.

**1.2 tsukushi grant** — unchanged (`state::add(UNSTOPPABLE)`); the gate now
makes the grant mean what it says. `own_stage`:

```rust
if trigger::abnormal_blocked() {                    // the `effect` raise carries it
    ctx::set_slot(player_id, "stageEnd", if yes { 1 } else { 2 });
} else {
    ctx::set_slot(player_id, "resolveOverride", if yes { 1 } else { 0 });
}
```

`abnormal_kind()` remains only for the [除外] exclusion in `can_counteract`.

**1.3 want_human** — the boost consumption arms a span slot instead of the
counter:

```rust
ctx::set_slot(player_id, SLOT_IN_MOVE, 1);
…
// On::Action(Action::MoveAfter("slot('want_human_in_move') > 0", None, after_move))
fn after_move(player_id: i32) -> Asked {
    ctx::set_slot(player_id, SLOT_AB_HIT, trigger::move_abnormal() as i32);
    ctx::set_slot(player_id, SLOT_IN_MOVE, 0);
    Ok(())
}
```

`TurnEnd` tests `SLOT_AB_HIT`; `SLOT_AB_BEFORE` and the counter diff go.

**1.4 shine_again** — `ctx::set_self_immune(true)` →
`ctx::set_self_immune_except_owner(true)`. `no_expectation` / `auction_pulled`
keep the any-flavour setter.

**1.5 crush_drum** — add `.props(&[(prop::ADDS_DICE, 1)])` to the `CardDef` and
`ctx::set_dice_immune(true)` at the end of the play body. The TODO(ABI) block
goes.

## 7. Save, record, replay

Every new persisted field is `#[serde(default)]`, all defaults inert
(`false` / `0`): `MoveCtx.abnormal_start`, `MoveCtx.activation`,
`TurnCtx.move_activation`, `TurnCtx.dice_immune`,
`FieldCard.immune_except_owner`. Existing saves load unchanged; a save
mid-move with an absent `abnormal_start` reads `0`, which can only *over*-count
an early abnormal in a move that started before the load — acceptable and
self-corrects at the next move.

**SAVE_VERSION stays 18** (all bumps are behind this group). **RECORD_VERSION
stays 3.** Records are unaffected:
no input shape changes (`dice_immune` is set by the same card text and the
same play commands; a replay of an archived engine bundle keeps its own
bundle). The v102 precedent (`#[serde(default)]` field, no SAVE_VERSION bump)
is the rule.

Replay: the activation ordinal derives from `turn.played` (re-pushed each pass
before the raise), `abnormal_start` lives on the move snapshot, and
`move_activation` is written with `main_moved` — all re-derived identically.
`blocked` is computed at raise time from the re-derived world. No new
non-determinism.

## 8. Versioning

**Wire break: yes.** New trailing fields on `Trigger`, `TriggerMove` and
`Effect` (postcard is not self-describing), nine new imports, and the
`abnormalGate` kind. `ABI_VERSION` 115 → **116** (thirteenth group in landing order), entry inserted immediately
below the `ABI_VERSION` constant and above the newest note (the tree keeps its
notes newest-first under the constant):

```
/// v116: **status, blocked-ness and immunity**. `Gate::Abnormal(i32)` /
///    `FilterKind::AbnormalGate` (wire `abnormalGate`, engine id 107) is a
///    CEL-only gate that refuses a received movement abnormal on an
///    [不可阻挡] holder; `abnormal_gate` asks it and refuses the landing.
///    `Trigger` gains trailing `activation` / `blocked`, `TriggerMove`
///    trailing `abnormal`, `Effect` trailing `blocked` -- postcard is not
///    self-describing, so **that** is the break. New imports
///    `trigger::{activation,move_abnormal,abnormal_blocked}`,
///    `effect::blocked`, `cant_move_code_for_activation`,
///    `is_immune_from_at`, `set_self_immune_except_owner`,
///    `dice_immune` / `set_dice_immune`; new props `IMMUNE_EXCEPT_OWNER` /
///    `ADDS_DICE`; `FieldCard.immune_except_owner`, `TurnCtx.dice_immune` /
///    `move_activation`, `MoveCtx.abnormal_start` / `activation`.
///    `ctx::gate(player, kind)` now honors the gate. SAVE_VERSION /
///    RECORD_VERSION unchanged (all save fields `#[serde(default)]`).
```

G06 also proposes 105. One bump must land first; if G06 lands first this text
becomes **106** and both entries stand. Cross-doc id collision to reconcile at
implementation: G01 proposes 111-113 and G09 111-115; G12 uses **107**.

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/`)

| item | test | assertion |
|---|---|---|
| 1 | `g12_tsugu_plays_past_a_spent_main_move` | after the main move: roll/draw/discount/gain land, the teleport does not |
| 1 | `g12_tsugu_teleport_survives_a_replay` | the (2) buy prompt forces a re-run; the teleport applies exactly once |
| 1 | `g12_tsugu_stay_still_blocks_the_teleport` | [停留] turn: no teleport, effects still land |
| 1 | `g12_tsukushi_garden_move_arm_is_activation_stable` | replay via the settle ask does not double-move |
| 2 | `g12_unstoppable_refuses_a_received_teleport` | `ctx::gate` false; position unchanged; no `abnormal` entry, no `ABNORMAL_HIT` |
| 2 | `g12_unstoppable_does_not_refuse_the_owners_own_move` | 里美-style self `card_move` walks (m13 stays green) |
| 2 | `g12_unstoppable_still_takes_a_stay_layer` | the stay counter rises; the move gate stays open (m30 stays green) |
| 2 | `g12_own_stage_branches_on_blocked_not_kind` | stay under [不可阻挡] → `resolveOverride`; stay without → `stageEnd` |
| 2 | `g12_own_stage_turn_end_half_on_a_skipped_turn` | skip latch, non-stay abnormal → `stageEnd` |
| 3 | `g12_want_human_counts_an_abnormal_inside_the_move` | an abnormal before the boost consume still adds 2 crystals |
| 3 | `g12_want_human_ignores_a_post_move_abnormal` | one after `moveAfter`, before `TurnEnd`, adds none |
| 4 | `g12_shine_again_takes_the_owners_effect` | owner's mass crystal add reaches the card |
| 4 | `g12_shine_again_refuses_a_foreign_effect` | an opponent's mass add skips it |
| 5 | `g12_dice_turn_skips_the_window_for_a_dice_card` | a second dice-adding card cannot be countered that turn |
| 5 | `g12_dice_turn_keeps_the_window_for_a_plain_card` | 安可 still answers a non-dice card |
| 5 | `g12_dice_turn_ends_with_the_turn` | next turn the window is back |

Existing to keep green: `general::tsugu_effects_land_when_the_move_is_spent`,
`cross_move::{m13,m23,m30}`, `gap_window::{g23,g24}`,
`guards::own_stage_matches_non_exile_abnormal`,
`spec_card_pp::shine_again_places_immune`, `gap_move` crush tests.

### 9.2 Kani (all three targets: wasm32 / native / kani)

* one `Op` per new import (`kani_host.rs`), `ctx/sys.rs` wasm extern,
  `sys_kani.rs` wrapper (hand-written), `rules/kani/src/host.rs` loud-fail arm
  and model, `rules/kani/build.rs` import list, `native_shims.rs` extern +
  forward, and the `host.rs` `func_wrap` registration.
* proof `shine_again_00_immune_iff` extended: the landed instance has
  `immune_except_owner`; mutant `is_immune_from_at(uid, owner) == false` and
  `is_immune_from_at(uid, other) == true`.
* `own_stage_02_branch_iff`: symbolic `blocked` selects `stageEnd` iff true,
  `resolveOverride` iff false, for every non-exile kind.
* `want_human_02_span_iff`: `SLOT_AB_HIT` iff `trigger::move_abnormal()`.
* `tsugu_02_only_the_move_arm_gated`: `cant_move_code_for_activation == 0`
  calls `card_move`; `2` / `3` skip it; the draw and gain run in all three.
* `crush_02_sets_immunity`: after the play `dice_immune()` holds.
* invariant: with `StubRules` no gate opinion → `Gate::Abnormal` open; the
  `blocked` computation is false for kind 4 always.
* mutant per proof (kill the guard / the flag) as in every G-series batch.

## 10. Alternatives

1. **Engine reads the status key directly** (`state_get("unstoppable")`)
   instead of a gate. Rejected: the engine already refuses to name statuses
   (5568-5570) and a ruleset vocabulary change would need an engine edit.
2. **Implement `[不可阻挡]` as an `effect`-window hook** (the status host
   cancels `Effect{kind:"abnormal"}` itself) instead of the gate. Viable and
   needs no `Gate` kind, but the refusal then opens a full counteraction chain
   for an effect that never lands, and every future status must hook the same
   window for a *pre-window* refusal; the gate matches the established
   act/pay/move shape. Kept as fallback if STA-02 wants the player's prompt
   inside the window.
3. **`ctx::moved_this_activation() -> bool` with hidden engine state** instead
   of the explicit `(player, activation)` pair. Rejected: hidden state has to
   be rebuilt on replay anyway, and the id is stamped on the activation
   trigger, which is what makes the read pass-stable.
4. **Card-side `MOVED` slots everywhere** (status quo). Works but every card
   re-invents it, `turn_key` collides across two plays in one turn, and the
   landed-position half is card-specific.
5. **Seat-wide immunity** in `immune()` for shine_again instead of a
   card-scoped causer read. Rejected: 「此卡不受…」 protects the card, not the
   player's money; a second immune card on the field would wrongly shield the
   seat.
6. **A `CardDef` / `ManifestEntry` bool for `AddsDice`** instead of a prop.
   Rejected: the manifest shape is a wire break of its own, while props are an
   existing opaque map and `card_prop` already reads them.
7. **Match the dice-immunity by the played card's text** (`card_text_mentions`
   with 「添加骰」). Cheaper but prose-derived; the prop is explicit and lets a
   ruleset differ.

## 11. Open questions

1. **STA-02 / the player's choice.** Glossary 14 says 「可选择…是否生效」. This
   design refuses by default; the prompt that lets the holder accept is STA-02.
   Should the gate answer be overridden by the prompt, or should the gate
   become a hook that asks?
2. **Does a new [停留] layer land under `[不可阻挡]`?** Glossary 14 refuses new
   stun/exile layers and only 「无视」s existing stay; this design lets a stay
   layer land (it does not hold the move while the status is up). Confirm —
   the group text's "no layer added" may intend stay too.
3. **Dice-immunity scope.** C# skips `Counteract` for the whole play; the sheet
   says 「不受任何其他效果影响」. Does it also cover the owner's own
   counteractions on that play, and `twoCards`? This design skips the `card`
   window for every responder and keeps `twoCards`.
4. **`activation` ordinal lifetime.** It resets per turn (`turn.played`). A
   counteraction card that outlives the turn and calls `card_move` gets `0`
   (outside an activation) — acceptable today; confirm no card needs a
   cross-turn activation view.
5. **Blocked-ness on non-move turns.** The design counts the turn player's
   `main_moved` / skip latch as blocked so `own_stage`'s turn-end half fires,
   and only for the turn player (an off-turn hit uses the per-seat gate).
   Confirm a teleport landing after the move is still the "this move" half (it
   is designed as such).
6. **`ADDS_DICE` declarations.** Only `crush_drum` is known today. Which other
   sheets count as 「添加骰效果」 (a plan `add_extra_dice`, or also a fresh
   `ctx::roll`)? `OPEN-crush-immune` tracks the wording.
