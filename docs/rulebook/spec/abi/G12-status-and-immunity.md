# G12 — Status, blocked-ness and immunity gates

Status: **implemented** (2026-11-04 revision of the 2026-10-27 draft; the
design landed as ABI v114).
Worktree `/home/monopoly/web/.claude/worktrees/g12-design`, branch
`worktree-g12-design`. Landed directly on ABI 113 (G01, G08, G02, G04, G03,
G06, G05+G13 and G07 have landed before it), ahead of G09–G11, so its numbers
are §8's, not the README's earlier planned row.

Group: **G12**. Family: the status layer's effects on the turn flow — the
replay-stable move gate, the [不可阻挡] bypass, move-scoped abnormal reads,
causer-aware card immunity and the dice-adding immunity.

Names are spelled out (G03 R25, G06/G07 headers). **Activation** is one engine
routine run: one play, one press or one turn-step run, identified by the
turn's monotonically increasing **activation ordinal**. **Blocked-ness** (one
word; the wire field keeps the short `abnormal_blocked`) is whether a landed
abnormal actually stopped this move from going where it would have gone — a
*derived* predicate, not the abnormal's kind. **Kind** is the `AbKind` wire code (1 stay / 2 stun /
3 exile / 4 teleport / 5 forced / 6 stop / 7 reverse). The group exists
because the engine confuses kind with blocked-ness, and because the
ruleset's `<status>` key is invisible to the engine on three paths.

Status hosts live in `rules/status`; the engine keeps only generic verbs
(`docs/STATUS-HOSTS.md`). The engine neither names `stay` nor switches on a
kind.

Ground truth is `docs/rulebook/cards.json`; every quoted fragment below was
re-read there and is identified by its card id beside it. The row ids are the
triage sheet rows, the same ids `BACKLOG.md` section B uses.

Reading order: `docs/GUARDS.md` §4.2 and §6 (the window vocabulary and the
derived chain lookups), `docs/ENGINE.md` "Drive replay semantics", G04
§4.4/§5.1 (the move plan and the spent main slot), G06 §6.7 (the effect
window's stage order this group slots into), G03 §5.8 (settle answers), G07
§5.1 (the targeting order around the immunity read).

**Closed slugs and rows:** all five items are covered.

| # | slug | severity | rows |
|---|---|---|---|
| 1 | `move-gate-replay-read` | high | 225, 988, 1498 |
| 2 | `unstoppable-blocked-read` | medium | 403, 404, 1035, 1454 |
| 3 | `major-abnormal-scope` | medium | 1098, 1171 |
| 4 | `immunity-causer` | medium | 608, 1412 |
| 5 | `dice-immunity` | medium | 710, 1462 |

**No item demotes to A.** Every item needs at least one engine-side fact the
current ABI cannot express: a replay-stable activation identity, a ruleset
gate kind that carries its abnormal kind, a move span, a causer-carrying
immunity flavour, or a turn-scoped dice-immunity flag.

## 0. Review record

This revision came out of a full review of the 2026-10-27 draft against
master, followed by a naming pass and an empirical replay experiment (§1.1).
The draft's architecture was mostly right; its base numbers, its line
citations and several surface decisions were not.

| # | Draft defect | Correction |
|---|---|---|
| R1 | The header claimed base ABI 115 / SAVE 18 and cited the draft's own worktree `card-rulebook-review`. Master is ABI 113 / SAVE 16 / RECORD 4 and the worktree is gone. | Header carries the real base and the real worktree; §8 walks the landing arithmetic from master, not from the draft. |
| R2 | Every engine citation was stale by hundreds of lines (`wasm_rules.rs:5537-5635` for `abnormal_gate`, `:2151-2163` for `cant_move`, `state.rs:1096` for `FieldCard`, `ctx.rs:1905` for `cant_move_code`). | Every site is re-cited: `abnormal_gate` 6668-6779, `refuse_abnormal` 6643-6667, `cant_move` 2345-2358, `FieldCard` 1135-1181, `cant_move_code` 2332-2337. |
| R3 | §1.1 said tsugu still gates the **whole play** through `cant_play`. That was already removed by `0bb7e634`; `zz_unstop_probe.rs` and the whole-card gate no longer exist. | §1.1 describes the body guard that is actually there, and pins the replay hazard with a failing test run instead of with the removed gate. |
| R4 | Item 1 cited `detour.rs` and `shop_friends.rs` as if they still needed migrating; they already read `cant_move_code(...) != 3`, and live in `card-ag`, not `card-general`. | §6.6 keeps both unchanged and states why a window body needs no ordinal. |
| R5 | The design put the "already moved" suppression on a **cartesian** `cant_move_code_for_activation(player, activation)` whose `activation` value the card had to obtain; nothing said what a mid-chain activation should pass. | §5.2 stamps one ordinal on every trigger in the single `counteract` funnel, so a card hands back the value it is already running under instead of reconstructing one. |
| R6 | `Trigger.activation` counted `turn.played.len() + 1`, i.e. hand plays only. A pressed skill or a turn-step run has no hand play and would collide on 0. | One `turn.activation_ordinal`, bumped once per routine in `Play::start` before the routine snapshot, so a replay sees the same value (§5.2). |
| R7 | The draft carried **both** `Trigger.blocked` and `Effect.blocked`, i.e. the same bit twice, once per effect entry. | Only `Trigger.abnormal_blocked`: the `effect` window's trigger *is* the entry reader (`own_stage` already reads `effect::kind(i)` off it), and the settlement hook needs it too. |
| R8 | `MoveCtx.abnormal_start` compared a turn-wide counter against a snapshot. | `MoveCtx.abnormal` is a bool set on the in-flight frame, plus `TurnCtx.move_abnormal` latched at move end for after-the-move readers (§5.5). |
| R9 | §4.5's `cant_move_code_for_activation` put the skip check *before* the spent check, silently answering "gate" where the plain read answers "already moved"; the note then claimed the opposite order. | §5.4 imposes one order, and the plain `cant_move` composes from the same helper so the two can never disagree. |
| R10 | The draft's `Trigger.activation` was also to serve the move span, so one field had two unrelated meanings. | Activation identity and the move span are separate fields with separate doc comments (`activation_ordinal` vs `abnormal`). |
| R11 | §4.1 assigned the new engine kind **107**. 107 is the *ruleset* namespace's `fansFlip`; free **engine** ids resumed at 121 (G06) and the highest engine id is now 131. | Engine kind **134** — behind G11's already-planned 132–133 (§9). |
| R12 | §4.1 named `Effect.blocked`'s import `effect::blocked(i)` inside `pub mod effect` but never said what index means in the `abnormal` settlement hook, which has no effect array. | §5.6 removes the question: the bit is `trigger::abnormal_blocked()`, readable on every trigger of the announcement. |
| R13 | The draft put `IMMUNE_EXCEPT_OWNER` in `card_sdk::abi::prop` *and* `game_core::state::prop` while the engine only needed the placement stamp; `ADDS_DICE` meanwhile was left without a consumer. | All three keys live in `game_core::state::prop` (mirrored in `card_sdk::abi::prop` for cards), where their reader is: the engine stamps on placement and reads at the play (§4.4, §5.7). |
| R14 | §1.4's `title_idol` example was given as the owner-effect that skips the card, but nothing in the draft said how the *stamp* survives a replay of the placement. | Placement stamping is already per-instance and survives by construction (§5.7); the test pins the stamp, not the replay. |
| R15 | §5.5's dice immunity was to be set by the card body, so the card was unprotected on its own `card` window. | The turn flag is raised at play from a declared prop, *before* the window (§5.8). |
| R16 | §7 said SAVE stays 18 but ALSO added fields to `MoveCtx` without saying whether the in-flight frame is saved. | §7.2 states that the frame is saved with `TurnCtx.plan_stack` since G04 and that every new field is `#[serde(default)]`. |
| R17 | §10's tests were named but not placed; G06 established one named file per group. | One new `rules/tests/tests/g12_status.rs` (§9.1). |
| R18 | Nothing in the draft said what `abnormal_gate` should do with the gate's answer beyond "refuse", and the position was left to the reader. | §5.1 fixes the order: out → untargetable → abnormalGuard → immuneAll → **gate** → effect window → settle, and the gate reuses `refuse_abnormal`. |
| R19 | The draft had no necessity section and no clause match (G06 added both after review). | §12 and §13 exist. |
| R20 | §11 alternative A3 said the ordinal "could read `turn.played`", without noting that `played` is cleared per turn and holds ids, not occurrences. | §10's A3 states the id-collision case (the same card twice in a turn). |

## 1. Items

### 1.1 `move-gate-replay-read` (rows 225, 988, 1498)

Book (`通用:@Tsugu ycm`), the only fragment that needs the main move at all:

> @Tsugu ycm：
> [手]：
> 投掷3d10并根据结果依次进行以下操作：
> 1. 结果至少为22则抽1张卡；
> 2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；
> 3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；
> 4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。

Only clauses (2) and (4) rewrite the main move. The play itself has no
`cant_play` any more (dropped in `0bb7e634`), and the body guards only those
two arms (`tsugu_ycm.rs:108-122`, `:182-190`). The guard is never a bare
`move_blocked` read: it carries a **hand-rolled replay marker**.

```rust
// rules/cards/card-general/src/tsugu_ycm.rs:108-122
let moved_here = ctx::player_pos(player_id) == station
    || ctx::slot(player_id, MOVED) == ctx::turn_key();   // MOVED = "tsugu_ycm_moved"
if moved_here || rules_status::move_blocked(player_id).is_none() {
    ctx::set_slot(player_id, MOVED, ctx::turn_key());
    …
    ctx::run_planned_move(player_id);
}
```

`ctx::cant_move_code` (`crates/card-sdk/src/ctx.rs:2332-2337`) reads the live
world through `RulesBridge::cant_move`
(`crates/game-rules/src/wasm_rules.rs:2345-2358`): `1` not your turn / `2`
already moved / `3` the move gate holds. A body is interrupted by host
requests and re-runs from the routine snapshot, so on a re-run after its own
teleport `main_moved` is true and the plain read answers `2` — the body drops
the move it already performed.

**The hazard is not theoretical.** Removing only `moved_here` from the
condition above (marker write kept), rebuilding the ruleset and running
`cargo test -p ruleset-tests --test spec_card_general tsugu` fails
`tsugu_ge26_teleports`:

```
expected a prompt; events: [… "log.outcome.plan_teleport", "log.teleport", "log.reshuffle"]
```

The teleport itself replays (the snapshot restores `main_moved`, so the move
runs again), but the `ask_tile` for clause (2)'s purchase prompt is reached
with `main_moved` already re-set by the replay's own first hop and the
*purchase* half is what the missing prompt shows. With the marker restored
the same suite passes 4/4. `tsukushi_garden.rs:87-89` repeats the workaround
(`MOVED = to + 1`) for its move arm.

`detour.rs:40` and `shop_friends.rs:24-34` are already on
`ctx::cant_move_code(...) != 3`; both read from a counteraction window, where
there is no activation body to re-run, so they stay.

### 1.2 `unstoppable-blocked-read` (rows 403, 404, 1035, 1454)

Book (`Mor:（筑紫）迷茫的庭园`):

> （筑紫）迷茫的庭园：获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动

Book (glossary 14) — what the granted state means:

> \[不可阻挡\]：处于该状态时无视已存在的\[停留\]，\[晕眩\]效果也不会获得新的层数；无法获得新的\[除外\]层数。\[停留\]，\[晕眩\]，\[除外\]在适当时机依旧掉层。处于该状态时可选择受到的\[传送\]，\[强制移动\]，\[强制停下\]效果是否生效。

`abnormal_gate` computes the movement set and **never reads it**
(`wasm_rules.rs:6699-6708`):

```rust
let movement = matches!(
    kind,
    crate::AbKind(4) | crate::AbKind(5) | crate::AbKind(6) | crate::AbKind(7)
);
```

The comment above it describes three clauses as if implemented; only the
ruleset's own grant-verb refusals for stun and exile exist (STA-01,
`rules/bandori-std/src/status.rs:185`, `:212`, `:284`). A received teleport,
forced move, stop or reverse therefore lands on an [不可阻挡] holder, and
`tsukushi_garden.rs:113-134` grants a protection the engine ignores (its TODO
says so).

Second half of the item: `own_stage` branches on the **kind**, not on
blocked-ness (`rules/cards/card-roselia/src/own_stage.rs:65-79`):

```rust
if matches!(
    abnormal_kind(),
    Some(bandori_std::kind::abnormal::Stay) | Some(bandori_std::kind::abnormal::Stun)
) {
    ctx::set_turn_end_settle_answer(player_id, …);
} else {
    ctx::set_move_settle_answer(player_id, …);
}
```

A [停留] that lands on an [不可阻挡] holder does not hold the move
(`STAY_HOLDS_MOVE = "state(actor,'stay') > 0 && state(actor,'unstoppable') == 0"`,
`rules/status/src/stay.rs:69`), yet the code takes the turn-end branch. Nothing
on the trigger or the effect entry says whether the landing actually blocked;
`guards.rs`'s own_stage row pins the current kind branch.

### 1.3 `major-abnormal-scope` (rows 1098, 1171)

Book (`CRYCHIC:想要成为人类` (2)):

> \[自动\]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。

「该次移动过程中」 is **that move's span**. The card snapshots the turn-wide
counter when the boost is consumed (`want_human.rs:131-150`,
`ctx::abnormal_count`) and diffs it at `TurnEnd` (`:181-196`):

```rust
ctx::set_prop(SLOT_AB_BEFORE, ctx::abnormal_count(player_id) + 1);
…
if ctx::abnormal_count(player_id) > ab_before - 1 { ctx::add_crystals(2, 0)?; }
```

A snapshot taken at `diceRollSettled` misses an abnormal that landed earlier
in the same move (a `diceRollPlan` / `diceRollValue` effect) and counts every
one that lands after the walk, up to `TurnEnd`. `cross_move::m23` exercises
only the happy path. `TurnCtx.abnormal` (`world.rs:242`) is turn-wide and has
no span.

### 1.4 `immunity-causer` (rows 608, 1412)

Book (`PP:再次闪耀` [特]):

> 此卡不受除拥有此卡的玩家以外的玩家的效果影响。

The engine stores one causer-blind bool, `FieldCard.immune`
(`state.rs:1149`); the readers are `is_immune_at(uid)` (`ctx.rs`,
`ops.rs:1690-1693`) and `card_immune(player, card)` (`ops.rs:2019-2023`) and
carry no causer. `shine_again.rs:81` settles for the plain 「不受任何效果
影响」 flag and TODOs the difference:

```rust
ctx::set_self_immune(true);
```

so the owner's own effects (e.g. `title_idol.rs` adding crystals to its own
cards) skip the card too. `no_expectation.rs` and `auction_pulled.rs` use the
any-flavour legitimately and must keep working.

### 1.5 `dice-immunity` (rows 710, 1462)

Book (`RAS:（MASKING）CRUSH ON THE DRUM!!!` [特]):

> \[特\]：
> 打出此卡的回合任何\[使用者\]使用的卡包含添加骰效果的卡不受任何其他效果影响。

Nothing of this exists: `crush_drum.rs:55-58` only adds Xd20 to the plan and
carries a `TODO(规则书)[judgement]`, there is no turn flag, no `addsDice`
property on the play, and the `card` counteraction window
(`play.rs:5753-5760`) opens for every hand play. `OPEN-questions`
`OPEN-crush-immune` records the scope question.

## 2. Shared root causes

1. **Kind used as a proxy for effect.** `own_stage` keys on stay/stun; the
   gate's movement set (4|5|6|7) is the only other kind test. Nothing produces
   the orthogonal question "did this landing hold the move".
2. **The engine cannot name a status key.** `abnormal_gate` says so in its
   own comment although [不可阻挡]'s clause is exactly an engine gate
   predicate. Status knowledge must reach the engine through a rule-evaluated
   gate, never a key.
3. **No replay-stable identity for "the activation that is running".** Two
   cards hand-roll a slot marker because a body cannot ask whether *its own*
   run already did something.
4. **No span on a move.** The one counter that exists is turn-wide.
5. **Immunity is one bool.** The book's two flavours (any / except the owner)
   collapse, so a card must choose the wrong one.

## 3. Blocked-ness as an invariant

Blocked-ness is derived once, in `abnormal_gate`, and carried on the trigger.
It has two halves and only the second is the ruleset's:

```
blocked = the landing took effect on this mover's move
        = the mover has no MOVE_EXEMPT for this walk
          and the ruleset does not exempt this kind for the mover
```

The engine half is `key::MOVE_EXEMPT` — the same expression the clamp uses
(`play.rs:1275`). The ruleset half is the *exemption* read of the new gate
(§5.1) and is exactly the kinds' own wording:

* Teleport is never blocked: the move still happens, it is *replaced*
  (`run_planned_teleport`).
* The three layers (stay, stun, exile) close the move gate — that is exactly
  what 「无法移动」 means — unless the mover is [不可阻挡].
* Forced, stop and reverse overwrite or trim the walk at the clamp whenever
  the mover is not exempt.

A refused landing (out, untargetable, guard, `immuneAll`, the new gate) has no
blocked-ness because nothing landed.

## 4. Surface

### 4.1 Abnormal gate kind — `crates/game-core/src/engine/rules.rs`, `crates/game-rules/src/gate.rs`

`Gate` gains a payload variant (raw `i32`; `game-core` does not depend on
`card-sdk`, the same rule as `state::prop`). It is asked twice — once for the
refusal and once for blocked-ness — told apart by a flag rather than by a
second kind:

```rust
/// May an abnormal of this kind (an `AbKind` wire code) from `by` land on the
/// seat? `exemption = false` asks the refusal; `true` asks "would this kind
/// hold this mover's move?" (blocked-ness, §3). CEL-only.
Abnormal {
    /// The `AbKind` wire code.
    kind: i32,
    /// Who caused it (`by == player` is the mover's own effect).
    by: i32,
    /// Refusal read (`false`) or blocked-ness read (`true`).
    exemption: bool,
},
```

`gate.rs::kind_of` maps both reads to `TriggerKind::AbnormalGate`; the window
builder sets `win.value = kind`, `win.by = by`, and, for the exemption read,
`win.step = 1` (the same flag overload the `ExclusiveUse` window already
uses). `GATE_KINDS`
(`host.rs:911-919`) gains the entry so `validate_cel_only_gates` enforces
CEL-only, and its fixed `[TriggerKind; 7]` type grows to `[…; 8]` (the error
string at `host.rs:957-963` lists the kinds by name and gains
`AbnormalGate`).

New engine kind, wire id **134** (behind G11's planned 132–133; §8):

```rust
/// `abnormalGate` — may an abnormal of this kind from `by` land on the
/// seat? Filter, CEL-only, hook-only. Payload: the seat, `value` = the kind,
/// `by_card` = the causer, `step = 1` on the blocked-ness read.
pub const AbnormalGate: Self = Self::engine(134);   // FilterKind
```

Distinct from the existing `abnormalGuard` hook: the guard is a placed card's
pre-declaration block; the gate is a turn-flow capability the engine asks
natively before a window opens.

Not new: the status host answers it through `On::Filter(Filter::AbnormalGate(
condition, None, refuse))` entries; no import is added for the ask (§6.2).

### 4.2 Payload fields — `game_core::engine::rules`, guest `world::Trigger`/`TriggerMove`

`Trigger` (`rules.rs:366-476`) gains, next to `is_main_move` (the existing
"this trigger belongs to a fact" precedent):

```rust
/// The activation this raise belongs to; stamped by `counteract` from
/// `turn.activation_ordinal`. 0 outside a routine.
pub activation_ordinal: i32,
/// Did the abnormal this trigger announces hold the move? Meaningful only on
/// the `effect` and `abnormal` raises of an abnormal; false elsewhere.
pub abnormal_blocked: bool,
```

`TriggerMove` (`rules.rs:51-83`) gains:

```rust
/// Did an abnormal land during this move? Set on the move's frame as it
/// lands, readable from every trigger raised later in the walk.
pub abnormal: bool,
```

`with_move` (`rules.rs:637-658`) copies it and the guest mirror
(`game-rules/src/world.rs:192`) and bridge (`wasm_rules.rs:9175-9192`) carry
it.

### 4.3 Persisted state

Every one of these is per-turn or per-move state, `#[serde(default)]`, and
`Default` keeps the neutral value.

`TurnCtx` (`world.rs`, beside `main_moved` / `played` / `abnormal`):

```rust
/// One higher per routine; stamped onto a trigger by `counteract`.
#[serde(default)]
pub activation_ordinal: i32,
/// The activation that consumed this turn's main move; 0 = none.
#[serde(default)]
pub move_activation_ordinal: i32,
/// A landed abnormal during this turn's main move (read at turn end).
#[serde(default)]
pub move_abnormal: bool,
/// 「打出此卡的回合」: hand plays of dice-adding cards skip the `card` window.
#[serde(default)]
pub dice_immune: bool,
```

`MoveCtx` (`move_ctx.rs`, serialized inside a `PlanFrame`):

```rust
/// An abnormal landed somewhere in this walk's span.
#[serde(default)]
pub abnormal: bool,
```

`FieldCard` (`state.rs:1149`, next to `immune`):

```rust
/// 「不受除拥有者以外的效果影响」; read through `is_immune_from_at`.
#[serde(default)]
pub immune_except_owner: bool,
```

### 4.4 Props — `crates/game-core/src/state.rs` (`prop`), mirrored in `card_sdk::abi::prop`

```rust
/// 「不受除拥有此卡的玩家以外的玩家的效果影响」: `1` = the placed instance
/// gets `FieldCard.immune_except_owner` stamped at placement.
pub const IMMUNE_EXCEPT_OWNER: &str = "immuneExceptOwner";

/// 「卡包含添加骰效果」: `1` = the card's effect adds dice to a move
/// (`plan::add_extra_dice`). Read at the play to test the dice immunity.
pub const ADDS_DICE: &str = "addsDice";

/// 「打出此卡的回合…」: `1` = playing the card raises `turn.dice_immune` for
/// the rest of its turn.
pub const GRANTS_DICE_IMMUNITY: &str = "grantsDiceImmunity";
```

These are declared props, not `ManifestEntry` fields: the map is already
generic and opaque, so the manifest shape does not change. The engine reads
the two play-time keys through `CardRules::card_prop` (`rules.rs:865-867`),
never from prose.

### 4.5 Imports — `crates/card-sdk/src/ctx.rs`, re-exported via `bandori_std::ctx`

```
trigger::activation_ordinal() -> i32
trigger::abnormal_blocked() -> bool
trigger::move_abnormal() -> bool
cant_move_code_for_activation(player_id: i32, activation_ordinal: i32) -> i32
is_immune_from_at(uid: i32, by: i32) -> bool
set_self_immune_except_owner(on: bool) -> bool
dice_immune() -> bool
set_dice_immune(on: bool) -> bool
```

Semantics:

* `trigger::activation_ordinal` — the running activation; a card passes it
  back rather than the host guessing.
* `cant_move_code_for_activation` — same code space as `cant_move_code`
  (`1` / `2` / `3` / `0`), except that "already moved" is suppressed when
  `turn.move_activation_ordinal == activation_ordinal` (§5.4).
* `is_immune_from_at(uid, by)` — `f.immune || (f.immune_except_owner && by !=
  f.owner)`; false when `uid` has no instance. `is_immune_at` stays the
  causer-free read for bookkeeping and the any-flavour cards.
* `set_self_immune_except_owner` — mirrors `set_self_immune`: writes the
  running instance's flag, returns whether the instance exists.
* `trigger::move_abnormal` — `turn.move_abnormal || the in-flight frame's
  abnormal`; false without a move.
* `dice_immune` / `set_dice_immune` — the turn flag; the setter is idempotent
  and exists for dynamic grants, while the printed clause rides the prop
  (§5.8).

### 4.6 Classification

| element | class |
|---|---|
| `Gate::Abnormal`, `AbnormalGate` kind, `GATE_KINDS` | new mechanism (wire id) |
| `Trigger.activation_ordinal`, `Trigger.abnormal_blocked`, `TriggerMove.abnormal` | wire fields (break) |
| five `TurnCtx`/`MoveCtx`/`FieldCard` fields | persisted, serde default |
| three props | opaque key |
| eight imports | new imports (break) |
| `detour` / `shop_friends` `cant_move_code != 3` | stays |

## 5. Engine wiring

### 5.1 The gate inside `abnormal_gate` (`wasm_rules.rs:6668-6779`)

The order becomes: out → untargetable → abnormalGuard → `immuneAll` →
**gate** → `effect` window → settle. The gate is a refusal like the two
before it, so it reuses `refuse_abnormal` and reports `abnormalRefused`
(kind 121) — the hook's existing contract already says "refused before it
landed, no window opened".

```rust
// after the `immuneAll` block; `s` is the victim's seat, `kind` the raw ask
let ask = game_core::engine::Gate::Abnormal {
    kind: kind.as_i32(),
    by: by_player,
    exemption: false,
};
if !cx.world().gate_open(self, ask, s) {
    // 「无视已存在的[停留]」/「无法获得新的层数」：the layer or the move
    // rewrite does not land at all. Existing layers still decay.
    self.refuse_abnormal(cx, player_id, by_player, kind, on)?;
    // the refusing rule's key from `gate_skip_msgs`, one new key
    cx.log(player_id, self.gate_skip_msg(ask, s).player_id("who", player_id));
    return Ok(false);
}
```

`RulesBridge::gate_open` (`wasm_rules.rs:11282-11295`) copies the payload onto
the window built by `window_of_world` (`win.value`, `win.by`, and `win.step`
for the exemption read); nothing else in the window changes. The ruleset's
[不可阻挡] host answers two families of entry, both keyed on `value` (the
kind):

* refusal, kinds 4/5/6/7:
  `On::Filter(Filter::AbnormalGate("step == 0 && value >= 4 && by != actor", None, refuse))`
* exemption: a gate entry's condition is its **closed** case, so this names
  the kinds that *hold* the move — `On::Filter(Filter::AbnormalGate(
  "step == 1 && value != 4 && (value >= 5 || state(actor,'unstoppable') == 0)",
  None, exempt))`: forced / stop / reverse always; the layers only while not
  unstoppable; teleport never. The engine reads
  `blocked = !MOVE_EXEMPT && !gate_open(…)`, i.e. a closed gate is a held
  move.

`by != actor` is 「可选择受到的…是否生效」: a self-caused move (a card's own
`run_planned_move`, 里美's 「期间[不可阻挡]」) is chosen, not 「受到的」, and
still walks. `rules/status/src/unstoppable.rs` is the host; STA-02's TODO
goes.

`log.unstoppable_skip` is the refusal's new message key (§5.8 names the
group's other one; display text lives in the i18n tables, never in the ABI).

### 5.2 Activation ordinal

The counter is bumped in `Play::start`, **before** the routine snapshot
(`mod.rs:1544-1547`), so a replay from that snapshot re-reads the same value
instead of allocating a new one:

```rust
fn start(&mut self, r: Routine) {
    self.world.turn.activation_ordinal += 1;
    let snapshot = self.world.clone();
    self.execute(r, snapshot, vec![]);
}
```

`counteract` stamps it next to `step`, in **both** funnels — the engine's
`Play::counteract` (`play.rs:5947-5966`) and the host's `RulesBridge`
(`wasm_rules.rs`, which already stamps the raise's `uid` at the same choke),
so a guest raise and an engine raise agree. The `uid` is not a substitute for
the ordinal: it is an allocation counter (the host's own comment there),
while the ordinal is read back out of the snapshot.

**A body's trigger is not a raise.** The host builds play, tile-settle and
event-rule body triggers with a struct literal, outside `counteract`, so
those constructors call one `stamp` helper that copies the same two fields:

```rust
fn stamp(&self, cx: &PlayCtx, t: &mut world::Trigger) {
    t.step = cx.state().step;
    t.activation_ordinal = cx.world().turn.activation_ordinal;
}
```

This is load-bearing and was found only by running the §9.1 test: without it
the body reads `0` while the move was stamped with the routine's ordinal, and
the replay still drops the teleport. Nested raises and bodies inherit the
running value, which is what makes a status-host body dispatched inside a card
body answer the *card's* activation.

### 5.3 The move stamp

`main_move` (`play.rs:1883`) and the `is_main_move` arm of `run_planned_move`
(`play.rs:1045-1047`) stamp the consuming activation:

```rust
if is_main_move {
    self.w.turn.main_moved = true;
    self.w.turn.move_activation_ordinal = self.w.turn.activation_ordinal;
}
```

The turn reset is the wholesale `self.w.turn = TurnCtx { ..Default::default() }`
at `play.rs:687-691`, so `move_activation_ordinal`, `move_abnormal`,
`dice_immune` and the ordinal itself fall back to their neutral values with
the clone's own `Default`, beside `main_moved`.

### 5.4 `cant_move` and its ordinal form

One helper, two entry points — the plain read composes from the ordinal form
with the *current* activation, so they cannot drift:

```rust
fn cant_move_from(&self, player_id: i32, activation_ordinal: i32) -> i32 {
    if self.world.st.turn != player_id { return 1; }
    if self.world.st.skip_move { return 3; }
    if self.world.turn.main_moved
        && self.world.turn.move_activation_ordinal != activation_ordinal
    {
        return 2;
    }
    0
}

fn cant_move(&self, player_id: i32) -> i32 {
    self.cant_move_from(player_id, self.trigger.activation_ordinal)
}
```

The order is turn → skip → spent, the same as today: a [停留] turn answers
`3` even after this activation's teleport, and a spent move from an earlier
activation answers `2` however the move gate stands.

### 5.5 Move span (`MoveCtx.abnormal`, `TurnCtx.move_abnormal`)

In `abnormal_gate`, right before the `effect` raise (i.e. only for landings):

```rust
// The span: the walk this landed in, and the latch read after the walk.
if let Some(f) = cx.world_mut().turn.plan_stack.last_mut() {
    f.plan.abnormal = true;
}
```

`end_plan` copies the finished main frame's bit into
`turn.move_abnormal` before the frame dies; a later move does not clear it
(only the turn start does), which is what 「该次移动」 at `TurnEnd` needs.

### 5.6 Blocked-ness computation

Just before the `effect` raise, after all refusals:

```rust
let exempt = cx.world().st.players[s].state_get(game_core::state::key::MOVE_EXEMPT) > 0;
let ask = game_core::engine::Gate::Abnormal {
    kind: kind.as_i32(),
    by: by_player,
    exemption: true,
};
let blocked = !exempt && !cx.world().gate_open(self, ask, s);
```

`exempt` is the same expression the clamp uses at `play.rs:1275`; the second
half asks §3's exemption read (the gate answers *open* = exempt). Both the
`effect` and the `abnormal` raise set `t.abnormal_blocked = blocked` through
their `on` closure; every other raise site leaves it false.

### 5.7 Immunity

`is_immune_at` is unchanged. `is_immune_from_at(uid, by)` is additive:

```rust
pub fn is_immune_from_at(&self, uid: i32, by: i32) -> bool {
    self.field_by_uid(uid)
        .is_some_and(|f| f.immune || (f.immune_except_owner && by != f.owner))
}
```

`place_card_on` stamps `immune_except_owner` from `prop::IMMUNE_EXCEPT_OWNER`
exactly like `immune` (`ops.rs:1407`), and `set_self_immune_except_owner`
writes the running instance (`ops.rs`, beside `set_immune_at`).

### 5.8 Dice immunity

In `play_from_hand`, before the `card` raise (`play.rs:5753`):

```rust
let rules = self.rules;
if rules.card_prop(id, crate::state::prop::GRANTS_DICE_IMMUNITY) != 0 {
    self.w.turn.dice_immune = true;
}
let immune_play = self.w.turn.dice_immune
    && rules.card_prop(id, crate::state::prop::ADDS_DICE) != 0;
```

The `card` raise below then branches: `immune_play` builds the trigger with
the same payload and stamps it (the `step` / ordinal copy) **without**
calling the counteraction dispatcher, so nothing is offered and the play
cannot be negated or rewritten at its own window. The card still flashes and
its body still runs. The skip writes one `log.play_immune` line.

Two new message keys in the whole group, both log keys, no ABI display text:
`log.unstoppable_skip` (§5.1) and `log.play_immune`.

The immunity is turn-scoped and seat-agnostic (「任何使用者」): whichever
player plays a dice-adding card that turn is protected. Everything after the
window (the body, the settle) is unaffected; effects that later target the
card by uid consult `is_immune_from_at` in their own bodies.

### 5.9 Kani model

`rules/kani/src/host.rs` gains the eight ops' arms. `arm_card_move`
(`:1643`) already models a move; the new arms are pure readers over it and
over the card props. The model bound stays 0..=2 players and one tile, as in
every group.

## 6. Card usage, before / after

### 6.1 `通用:@Tsugu ycm`

Before: §1.1. After — the marker, `MOVED` and `ctx::turn_key` go:

```rust
if station >= 0
    && ctx::cant_move_code_for_activation(
        player_id, ctx::trigger::activation_ordinal()) == 0
{
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(station);
    ctx::plan::set_settles(false);
    ctx::run_planned_move(player_id);
}
```

The `station >= 0` half and the (4) fallback stay. `slot` / `turn_key` remain
available for other cards.

### 6.2 `rules/status` hosts

`stun.rs` / `exile.rs` keep their grant-time refusals (STA-01) — those stop
the *layer*, not the move, and they stay because a body can add a layer
without going through the engine at all. `unstoppable.rs` gains the two
condition families of §5.1 and loses the TODO. `stay.rs` keeps
`STAY_HOLDS_MOVE` as the `MoveGate` condition and its own
`skip_main_move(after > 0 && !unstoppable)`; the [不可阻挡] bypass needs no
change there, since stays still land under it and the gate's exemption read
(§5.6) is what reports them unblocked.

### 6.3 `Mor:（筑紫）迷茫的庭园` (`tsukushi_garden.rs:80-134`)

The `MOVED` marker and `player_pos == to` test go exactly as in §6.1; the
guard becomes the ordinal read. The TODO at `:113-116` and the STA-02 TODO
both go: the grant is now enforced by the engine.

### 6.4 `R:选择自己的舞台` (`own_stage.rs:65-79`)

The kind match becomes one read:

```rust
if ctx::trigger::abnormal_blocked() {
    ctx::set_turn_end_settle_answer(player_id, if yes { Settle } else { Skip });
} else {
    ctx::set_move_settle_answer(player_id, if yes { Settle } else { Skip });
}
```

`can_counteract` keeps its `!= Exile` filter; `abnormal_kind` stays for it.

### 6.5 `CRYCHIC:想要成为人类` (`want_human.rs`)

`SLOT_AB_BEFORE` goes. `diceRollSettled` only consumes the boost; `TurnEnd`
reads `ctx::trigger::move_abnormal()`. The value is latched before the move,
so an arming whose move never ran answers false (correct: 「该次移动过程中」
never happened).

### 6.6 `AG:绕路` / `AG:店内友人` — unchanged

Both read `ctx::cant_move_code(...) != 3` from a counteraction window. A
counteraction is not an activation body that could have driven the move, so
the plain read is exact and the ordinal form would be the same value.

### 6.7 `PP:再次闪耀` (`shine_again.rs:76-84`)

The `[特]` prop `immuneExceptOwner = 1` replaces the body's
`ctx::set_self_immune(true)`; the body call stays only if a *dynamic* grant is
wanted (it is not). The TODO goes. A test pins that an owner's own effect
lands and another player's does not.

### 6.8 `RAS:（MASKING）CRUSH ON THE DRUM!!!` (`crush_drum.rs`)

Two props: `grantsDiceImmunity = 1`, `addsDice = 1`. The
`TODO(规则书)[judgement]` is narrowed to the unruled scope question (§11.2).

## 7. Save, record, replay

### 7.1 Replay

The ordinal is the whole point: it is bumped before the snapshot, so a
halt-and-replay re-runs a body with the same ordinal and a move its own
routine already drove reads back as "not spent". No answer stream changes
shape; `docs/ENGINE.md`'s drive-replay contract needs no amendment.

### 7.2 Save

Every new field is `#[serde(default)]`. `TurnCtx` and `FieldCard` are saved
as they are; `MoveCtx` rides `TurnCtx.plan_stack` (G04) and the in-flight
frame is saved with it, so a mid-move save keeps its span bit. An old save
loads with `activation_ordinal = 0` and `move_activation_ordinal = 0`; a
restored turn that had already moved therefore reads as "spent by an earlier
activation", which is the pre-G12 answer after a restart. Documented, not
fixed (open question §11.3).

### 7.3 Record

`RECORD_VERSION` stays 4. Records store commands and answers, both unchanged;
the derived bits are recomputed on replay of an archived bundle.

## 8. Landing arithmetic

The group landed directly on master (ABI 113, SAVE 16, RECORD 4, highest
engine kind 131, highest Kani `Op` 442) ahead of G09–G11, so the numbers are
the base's, not the README's earlier row for G12 (which assumed it landed
fourth):

* **ABI 113 → 114.** The eight imports and the three wire fields are a
  break; a module built before the bump cannot instantiate against the new
  host, which is the point of the version.
* **SAVE 16 unchanged.** G12 adds no step of its own: §7.2's defaults make
  it backward compatible. G09–G11's rows shift one step each.
* **RECORD 4** unchanged.
* **Engine kind 134.** 132–133 stay reserved for G11; 131 is G10's.
* **Kani `Op` 443–450**, the eight free slots at the tail of G07's band.

## 9. Tests and Kani obligations

### 9.1 Tests — new `rules/tests/tests/g12_status.rs`

| item | test |
|---|---|
| 1 | `g12_tsugu_replays_its_own_teleport` — the failing run of §1.1 as a pinned test: play at ≥ 26, answer the buy prompt, then assert the position and that the purchase offer opened |
| 1 | `g12_tsugu_stay_latch_still_skips` — a [停留] turn, the teleport is skipped |
| 1 | `g12_tsukushi_garden_replays_its_own_move` |
| 2 | `g12_unstoppable_refuses_a_received_teleport`, `…_forced_move`, `…_stop`, `…_reverse` — the abnormal does not land, `abnormalRefused` fires |
| 2 | `g12_unstoppable_still_lands_a_stay_layer` — the layer lands and still decays on its tick |
| 2 | `g12_unstoppable_keeps_an_existing_layer_decaying` |
| 2 | `g12_own_stage_takes_the_move_branch_under_unstoppable` — the landed stay is unblocked, so the move settle is chosen |
| 3 | `g12_want_human_counts_an_abnormal_before_the_roll` (diceRollPlan), `g12_want_human_ignores_one_after_the_walk` |
| 4 | `g12_shine_again_ignores_another_players_effect`, `g12_shine_again_accepts_its_owners` |
| 5 | `g12_crush_drum_immune_turn_skips_the_card_window`, `g12_crush_drum_immunity_does_not_outlive_the_turn`, `g12_a_plain_card_still_opens_its_window` |

`g12_own_stage_takes_the_move_branch_under_unstoppable` is why §5.1 refuses
only kinds 4–7: a stay still lands under [不可阻挡] (「也不会获得新的层数」
names only 晕眩 and 除外), and the blocked-ness read is what says it does not
hold. A `MOVE_EXEMPT` walk gives the same branch without a status.

### 9.2 Kani — all three cfg paths (wasm32 / native / kani)

* `abnormal_gate` gets a model proof over the seven kinds: refused iff the
  ruleset closes the gate, and blocked-ness is exactly §3.
* `cant_move_from` gets an exactness proof: `0` iff the turn matches, the move
  is not skipped and either the main move is free or it was driven by the
  asking activation — no more, no fewer codes.
* The immunity read gets a truth-table proof over `(immune,
  immune_except_owner, owner == by)`.

All three are `#[kani::proof]` on the body, exact (a `#[ignore]`d
discrepancy fails), per G15's contract.

### 9.3 Conformance

`rules/tests/tests/g06_windows.rs`'s `abnormalRefused` coverage gains the
gate refusal; `guards.rs`'s own_stage row flips its expectation.

## 10. Alternatives

* **A1 — leave the card markers.** Cheapest today; every future card that
  reads the move gate needs its own marker, in its own key namespace, with its
  own turn key. Five cards already need it.
* **A2 — a `turn.played.len()`-based ordinal** (the draft). Hand plays only;
  a pressed skill or a step run collides on 0.
* **A3 — compare the consuming card id** instead of an ordinal. The same card
  played twice in one turn collides; the id is not an occurrence. The
  trigger's `uid` fails the same test from the other side: it is an allocation
  counter, not a semantic identity, so it is only stable per raise *order*
  (§5.2).
* **A4 — a boolean "this body already moved" on the routine.** A nested
  raise from another card would clear it. The ordinal scopes by inheritance.
* **A5 — make the status hosts name keys in the engine** (an
  `unstoppable` field on `TurnCtx`). Every future status needs an engine
  change; the gate is already the generic seam.
* **A6 — derive blocked-ness in cards** from `abnormal_kind() != Teleport`
  plus an unstoppable read. Duplicated per card, and the exemption is
  engine-only state.
* **A7 — a second immunity bool** (`immune_from_others`) beside `immune`. Two
  readers, two paths; one field with a causer parameter is smaller.
* **A8 — no dice-immunity field; a card-side `slot`.** The immunity is a
  turn property and must survive the play's own replay; a slot write in the
  body comes too late for the body's own window (§5.8, R15).

## 11. Open questions

1. **Does a refused movement abnormal still count as 「受到影响」 for the
   cards that read the window?** A received teleport refused by [不可阻挡]
   never reaches the `effect` window under §5.1, so 「不受…效果影响」-class
   cards are not offered and `own_stage` takes no branch from it. Reading:
   correct — nothing 「影响」 the move. Needs a ruling; the tests in §9.1 are
   written to the immediate reading.
2. **`OPEN-crush-immune` scope** — does "any user" include the card's own
   turn owner's *later* plays (yes under §5.8), and does "not affected by any
   other effect" reach settle-time effects after the play (not under §5.8; a
   `TODO(规则书)` stays until ruled).
3. **A save restored across a G12 upgrade** cannot tell "moved earlier this
   turn" from "moved by this activation" (§7.2). The pre-G12 behaviour
   (treat as spent) is kept; a boot fix-up would need the routine.
4. **`immuneAll` vs the causer flavour** — G06's `immuneAll` is seat-wide and
   causer-free; the card flavour is per-instance. They stay separate
   mechanisms; nothing in the book asks for a seat-wide "except the owner".

## 12. Necessity

This section answers why each piece exists and why it could not be done
smaller. It is written item by item (a reviewer can strike one without
striking the group) and surface by surface (a reviewer can strike a field),
and it names the shortest card-side alternative with the exact point where it
breaks.

### 12.1 The group

One hole with two ends: the engine has no vocabulary for "did this landing
affect *this* move" and none for "immune except the owner". Items 1 and 2 are
the two sides of one predicate — 1 reads it across a replay, 2 produces it
instead of the kind; 3 reads the span; 4 is the missing immunity flavour; 5 a
per-turn protection for one class of play. Landing 1 alone leaves 2's kind
branch wrong (`own_stage` would still split on stay/stun); landing 2 alone
leaves 1's replay drop. Neither is a complete answer to "the landing did or
did not hold the move", and a reader needs both halves.

### 12.2 Per item

* **1 (the replay-stable read).** Today's card-side alternative is the slot
  marker tsugu and 迷茫的庭园 already carry. It is three lines per card, a
  private key per card and a turn-key comparison — and it is **wrong on a
  second play in the same turn**: the marker still holds this turn's key, so
  the second play skips the gate, calls `run_planned_move` into a spent slot,
  and then reaches its purchase prompt for a teleport that never happened.
  `played` cannot stand in (ids, not occurrences) and `trigger.uid` cannot
  either (allocation order, §5.2). Necessity: one counter bumped inside the
  already-single routine entry, one stamped field, one comparison inside the
  helper both readers share.
* **2 (the landing gate).** Nothing card-side can decide it: the landing is
  decided before any body of the affected card runs. A5 (an `unstoppable`
  field the engine reads) needs an engine edit per future status and puts a
  game key into `game-core`; [不可阻挡] happens to need **two** different
  answers (refuse, and does-it-block), which a single field cannot give
  without a second boolean in the same place. Necessity: one `Gate` variant,
  one hit-test in the function that already runs the refusal pipeline, and
  two CEL condition strings in the status host.
* **3 (the span).** Today's snapshot-and-diff is card-side and already
  implemented (`SLOT_AB_BEFORE`) — it is the failure, not the fix: the diff
  spans the whole turn from `diceRollSettled` to `TurnEnd`, so it counts a
  landing after the walk and misses one before the roll. Reading
  `turn.abnormal` at the walk's end would need a walk-end hook the card has
  no access to; a card cannot see the frame at all. Necessity: one frame bool
  written where the landing already is, one turn latch copied at `end_plan`.
* **4 (the causer).** `immune` is a bool read by two readers. The shortest
  alternative is a second bool (`immune_from_others`), which duplicates both
  readers and the placement stamp, and leaves `set_self_immune` ambiguous for
  the next card author. A causer parameter is the same two readers collapsed
  into one function with a three-term predicate.
* **5 (the turn flag).** The shortest alternative is a slot set by the body —
  one write, no ABI field. It fails on the card itself: the body runs *after*
  the `card` window, so the playing card cannot protect its own play; the
  window is also raised natively, so a body cannot influence it in time.
  Necessity: one turn flag, one prop read at the play, one condition at the
  raise.

### 12.3 Why the engine and not the ruleset

Each item's decision has to exist **at a point no card body can reach**:
before a routine snapshot (1), before a window opens (2, 5), inside the walk
(3), and at every reader of a field (4). That is the same test G07 used to
put the designation in the engine. The rulebook's own words stay in the
status host: the engine still names neither `stay` nor `unstoppable`.

### 12.4 Surface, field by field

| surface | forced by | if removed |
|---|---|---|
| `Gate::Abnormal` | 2 | the engine names `unstoppable` |
| `AbnormalGate` / kind 134 | 2 | a guest import and a body call per ask site |
| `Trigger.activation_ordinal` | 1 | cards cannot obtain the value they must pass |
| `Trigger.abnormal_blocked` | 2 | 3's kind proxy returns |
| `TriggerMove.abnormal` | 3 | 3's turn-wide diff returns |
| `turn.activation_ordinal` | 1 | nothing to stamp at the funnel |
| `turn.move_activation_ordinal` | 1 | the suppression cannot identify the driver |
| `turn.move_abnormal` | 3 | 3 breaks at `TurnEnd` |
| `turn.dice_immune` | 5 | 5 has no span and dies with the play |
| `MoveCtx.abnormal` | 3 | the pre-roll landing is missed |
| `FieldCard.immune_except_owner` | 4 | 4 falls back to the cause-blind `immune` |
| `trigger::activation_ordinal` | 1 | A2's hand-play-only count |
| `cant_move_code_for_activation` | 1 | every reader re-writes the comparison |
| `is_immune_from_at` | 4 | every reader re-writes the three terms |
| `set_self_immune_except_owner` | 4 | a dynamic grant needs two setter calls |
| `trigger::abnormal_blocked` | 2 | readers re-derive from the kind |
| `trigger::move_abnormal` | 3 | the latch cannot be read |
| `dice_immune` / `set_dice_immune` | 5 | dynamic grants cannot extend the flag |
| three props | 4, 5 | their literals repeat in card bodies |

### 12.5 The breaks are the price, and it is small

* **ABI 117.** Eight append-only imports and three wire fields; a module built
  before the bump cannot instantiate, which is what the version is for.
* **SAVE stays 18.** Every new field is `#[serde(default)]` in a struct that
  already saves (or in the saved plan frame), so no old save is misread.
* **No decision in an import.** The gate answers are CEL conditions; the
  imports are readers and two flags. `cant_move_code_for_activation` returns a
  code, never a choice.
* **No new display text in the ABI.** Two log keys, both resolved in the i18n
  tables.
* **Three game-named prop keys reach the engine**, which G07 avoided by
  passing its key as an argument. Here the engine must read them *before* a
  body exists, so an argument has no caller; they sit in `state::prop` beside
  `IMMUNE` / `DESIGNATES`, which are the same kind of key.
* **Landing independence.** 1 and 2 compile and land separately, 3 needs only
  `abnormal_gate`, and 4/5 are disjoint. The band in §8 is the only coupling.

### 12.6 What would argue against the group

* *"Just keep the markers."* True until the second play of the turn (12.2/1),
  the next reader card, and 迷茫的庭园's own `to + 1` variant — three costs,
  all already paid by two cards.
* *"Put `unstoppable` in the engine."* A rebuild per status, and a second
  boolean as soon as [不可阻挡] needs the blocked-ness answer too.
* *"Fix 3 by reading `abnormal` at the settle."* The settle is after the walk;
  the pre-roll landing is already lost, and the walk's own landing is counted
  by the same turn-wide figure it was meant to replace.
* *"Defer 4 and 5."* Legitimate: they are disjoint from 1–3. Striking them
  keeps 再次闪耀 on the wrong flavour and CRUSH's clause unimplemented, and
  costs the two fields the other cards do not read.
* *"Defer the group."* The replay defect is a live high-severity bug with a
  failing test in §1.1; every deferral keeps the two markers and the wrong
  `own_stage` branch.

## 13. Clause match

### 13.1 `通用:@Tsugu ycm` (item 1)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「结果至少为22则抽1张卡」 | unchanged | existing `tsugu_ge22_draws` | matched |
| 2 | 「结果至少为26则…改为[传送]…并不[结算]，且可选择购买…」 | ordinal read (§6.1), `set_settles(false)` | `g12_tsugu_replays_its_own_teleport` | was gap |
| 3 | 「[消耗]资金时降低1500」 | unchanged | existing | out of group |
| 4 | 「小于26则选择…或…」 | the same ordinal read on the fallback | `g12_tsugu_stay_latch_still_skips` | matched |

### 13.2 `Mor:（筑紫）迷茫的庭园` + glossary 14 (item 2)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「获得100资金」 | unchanged | existing | out of group |
| 2 | 「投掷1d6并…传送…」 | ordinal read; the grant comes after | `g12_tsukushi_garden_replays_its_own_move` | was gap |
| 3 | 「可以选择是否触发结算」 | unchanged | existing | matched |
| 4 | 「直到下个你的回合开始时，你无法被异常移动」 | the `AbnormalGate` entries (§5.1, §6.2) | the four refusal tests | was gap (engine ignored it) |
| 5 | 「无视已存在的[停留]」 | the layer already exists; `STAY_HOLDS_MOVE` keeps it decaying | existing | matched |

### 13.3 `R:选择自己的舞台` (item 2)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「受到[除外]以外的异常移动效果影响时可打出此卡」 | unchanged filter | existing | matched |
| 2 | 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」 | `abnormal_blocked()` branch (§6.4) | §9.1 ruling-dependent test | was gap |

### 13.4 `CRYCHIC:想要成为人类` (2) (item 3)

| # | clause | design element | test | verdict |
|---|---|---|---|---|
| 1 | 「若该次移动过程中受到异常移动效果影响」 | `move_abnormal` (§6.5) | the two span tests | was gap |
| 2 | 「为该卡添加两个奇迹水晶」 | unchanged `add_crystals` | existing | matched |

### 13.5 `PP:再次闪耀` (4) and `RAS:…CRUSH ON THE DRUM!!!` (5)

| card | clause | design element | test | verdict |
|---|---|---|---|---|
| 再次闪耀 | 「不受除拥有此卡的玩家以外的…影响」 | `immune_except_owner` + `is_immune_from_at` (§6.7) | the two immunity tests | was gap |
| CRUSH | 「任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响」 | the turn flag + the window skip (§5.8, §6.8) | the three dice tests | was gap |

### 13.6 Neighbours the design must not break

| card | clause | how it stays true |
|---|---|---|
| `通用:后勤人员的努力` | 「将打出的那张卡[手]效果…变为两倍」 | it counters the `card` window; a dice-adding play under an immune turn is simply not offered to it |
| `PP:No Expectation` / `PP:拍卖落槌` | 「不受任何效果影响」 | `is_immune_at` unchanged; `is_immune_from_at` is a superset only for the new flavour |
| `R:EXIST`, `Mujica:#J11` | the target window and the redirect | untouched; the gate sits inside `abnormal_gate`, before the window |
| `rules/status/stun`, `exile` | STA-01 grant refusals | unchanged; the gate adds a *landing* refusal beside them |

### 13.7 Result of the match

Every affected clause is either unchanged or mapped to a named element with a
test. The six gaps found by this pass — 13.1 clause 2, 13.2 clauses 2 and 4,
13.3 clause 2, 13.4 clause 1 and both 13.5 rows — are closed by §5 and pinned
in §9.1. The two ruling-dependent tests (13.2 clause 4's offer and 13.3
clause 2's branch) are named as such in §11.1.
