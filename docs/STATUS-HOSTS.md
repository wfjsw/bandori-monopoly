# Status markers as rule hosts

Each rulebook status marker ([停留] / [晕眩] / [除外] / 火罐, and the cheap
companions [不可阻挡] / 「无手牌」) becomes its own **standing pseudo card** --
`status:stay`, `status:stun`, `status:exile`, `status:fire`, `status:unstoppable`,
`status:no_hand` -- following the `mark:cp` precedent
(`rules/tile_marks/src/{lib,cp}.rs`, `docs/TILES.md`). The engine keeps only
**generic** turn-flow verbs and player gates; it stops naming stay / stun /
exile / fire. Cards call one canonical API in `rules/status`, not the
`card-sdk` `ctx::{give_stay, gain_fire, ...}` surface.

Motivation (user request 2026-10-11): *"Canonicalize and standardize these
APIs. Consider making each marker that has special meaning its own rule host
to process, instead of relying on the engine."*

---

## 1. Rulebook ground fact

Sources: `data/rules.txt` (「标志物解释」 / status glossary) is the status
definition; `docs/rulebook/cards.json` and the skill sheets supply the card
clauses that grant or read the statuses. Quote each passage beside the host
body that implements it. Gaps are `TODO(规则书)`.

### [无法移动] (`data/rules.txt` 47)

> [无法移动]：处于该状态时[主要移动]的[移动终点]视为当前所在格子且不[结算]。
> 无法使用非[传送]的移动效果（如果因效果随机到非[传送]的移动效果则抵消该效果）。

A **generic move gate**, not a marker of its own. Stay / stun / exile all
「处于该状态时[无法移动]」, so they set the same gate while their layers > 0.

### [停留] (`data/rules.txt` 48)

> [停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。

* layered counter; wear-off **one layer at the owner's turn end**;
* while layers > 0 and the player is not [不可阻挡], the move gate is set;
* dropping the last layer (with no [除外] holding the move) un-latches the
  turn's `skip_move` -- done by the host's count-changed handler, **not** by
  the grant site (memory: *state clauses belong on events*).

### [晕眩] (`data/rules.txt` 49)

> [晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果，无法收付款。
> 重复受到该效果时可叠加，玩家的每回合结束时移除一层。

* layered counter; wear-off at the owner's turn end;
* gates: move / act (skills + [手]) / receive-payment;
* `stunStart` is the layer that only **starts counting next turn** (item
  expiry `Tick::TurnStart`) -- today stamped by `default_expiry` in
  `game-core/src/state.rs`; the stun host declares it.

### [除外] (`data/rules.txt` 50)

> [除外]：处于该状态时[无法移动]，不能被[指定]，无法使用[主动]技能和手卡的[手]效果，
> 无法收付款；玩家棋子不失为存在于任何格子上。该效果生效时无法重复获得该效果，
> 玩家的每回合开始前移除一层。处于该状态时玩家技能和[场地]上的[持续]效果停止生效。
> 获得该状态的效果会写明状态结束后在玩家回合开始时要[传送]到的格子。

* layered counter; wear-off **one layer at the owner's turn start** (before
  `turnStart`); the return [传送] is written as `exileTo`, and
  「视为当回合的主要移动」 is `exileMain` (MyGO:无路矢);
* gates: move / act / receive-payment / **on-board** (「棋子不失为存在于任何
  格子上」 -- `present_from` / `players_on` skip exiled seats) / targeted
  (「不能被[指定]」);
* 「该效果生效时无法重复获得该效果」 -- granting while already exiled does
  not stack (`TODO(规则书)`: today `give_exile` stacks layers; see §6);
* while exiled, 「玩家技能和[场地]上的[持续]效果停止生效」 -- existing
  engine read of `exile() > 0` in skill / continuous-effect gates moves to the
  on-board / act gates.

### 火罐 (`data/rules.txt` 126)

> 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过持有上限

* counter + cap (`StateVar.max` today, written by the character skill);
* gain is partial («at most up to the cap») and logged
  (`log.gain_fire`, 效果适用-style: which effect, why, outcome);
* spend opens the `markerSpend` [反击] window **before** the pots move
  (user ruling 2026-10-07); a cancelled window spends nothing;
* a cap change pulls the value down under the new ceiling
  (`add_fire_max`).

### [不可阻挡] (`data/rules.txt` 51)

> [不可阻挡]：处于该状态时无视已存在的[停留]，[晕眩]效果也不会获得新的层数；
> 无法获得新的[除外]层数。[停留]，[晕眩]，[除外]在适当时机依旧掉层。
> 处于该状态时可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效。

Cheap host: a boolean-ish counter that (a) suppresses the move gate for stay,
(b) refuses new stun / exile layers, (c) still lets existing layers tick off.
TODO(规则书): the "choose whether [传送]/[强制移动]/[强制停下] apply" clause
is a separate prompt surface -- not in this batch.

### 「无手牌」 (`noHand`)

Not a named glossary marker; it is the continuous 「手牌数没有上限」 /
「may hold no hand cards」 companion already keyed as `noHand`. Kept as a host
for symmetry (its TurnStart/TurnEnd 2→1→0 dance lives in `play.rs` today).

---

## 2. Precedent: `mark:cp`

`rules/tile_marks/src/cp.rs` is the model:

| piece | `mark:cp` | `status:*` |
|---|---|---|
| id | `mark:cp` | `status:stay` … |
| bind once to | `BOARD_OWNER` (`-1`), `tile = -1` | same |
| bind path | `game_core::data::mark_rule_ids` -> `bind_tiles` | sibling `status_rule_ids`, same path |
| owns | tile [CP点] units of its named counter | the status layers on players |
| talks via | `On::Message`, `On::Action` | `On::Action` (+ `On::Message` for grants) |
| cards never special-case it | yes | yes |

Memory rulings enforced here:

* **Rulebook is ground fact** -- every host body quotes its passage
  (`TODO(规则书)` on gaps).
* **Marker = counter of the creating (pseudo) card** -- the status layers are
  counters of `status:*`, bound to players. The hosts live for the whole
  match (standing pseudo cards), so the layers live as long as the match.
* **「…时」 state clauses are event handlers** on that state's change
  (`CounterChanged` / the status triggers), not inline checks at each spend
  site. Un-latching `skip_move` on the last [停留] layer is the canonical
  example.
* **counteraction** for [反击] windows, **hook** for passive listeners.
* **card-sdk stays generic** -- `rules/status` is a **game** crate. No
  火罐/停留/晕眩/除外 names in `card-sdk`.
* **效果适用 logs which effect, why, outcome** -- `gain_fire`'s
  `log.gain_fire` shape stays; every host write that is an effect logs the
  same way.
* **Wire contract** -- `MatchPlayer.state` keys `stay`, `stun`, `stunStart`,
  `exile`, `exileTo`, `fire`, `handLimit` are frozen
  (`webui/src/core/names.ts` `stateOf`, `Popups.tsx`, `Players.tsx`,
  `autopilot.ts`). The view keeps presenting them.

---

## 3. Concrete design

### 3.1 Storage: the keyed state map is the player-bound counter

**Adjustment from the sketch, recorded.** The brief sketched «each owns its
counter bound to players (a unit bound to a player)», which in the
mark/counter model would be `MatchPlayer.tokens` / `add_held`. The code says
otherwise:

* `MatchPlayer.state` is already documented as *"the counters this player
  carries -- status pots plus the arbitrary named slots content invents"*
  (`game-core/src/state.rs`).
* The web client reads those keys by name from `MatchPlayer.state`
  (`stateOf(x, "stay")` etc.). Moving the values into `tokens` would need a
  projection back into `state` for the view, plus a `SAVE_VERSION` migration,
  for no semantic gain.
* Wear-off is already item-carried (`StateVar.expires`, `default_expiry`
  stamps `Tick::TurnEnd` / `Tick::TurnStart`, `tick_state` does the decrement
  generically and names no keys).

So: **the named state keys stay the storage and the wire**. Each `status:*`
host is the **creating rule** of its key(s) -- it owns the semantics
(wear-off declaration, clamping, gates, clauses) and is the only writer of
the grant/clear verbs. Cards go through `rules/status`, never raw
`state_set`. If a later batch wants literal `tokens` rows, the host API is
the seam; the wire projection is a view concern.

| host | state keys owned | extra slots |
|---|---|---|
| `status:stay` | `stay` | -- |
| `status:stun` | `stun`, `stunStart` | -- |
| `status:exile` | `exile`, `exileTo` | `exileMain` |
| `status:fire` | `fire` (value + `max` = cap) | -- |
| `status:unstoppable` | `unstoppable` | -- |
| `status:no_hand` | `noHand` | -- |

### 3.2 New crate `rules/status`

Workspace member (add to `rules/Cargo.toml` `members`), wired into
`rules/cards/card-all` like `rules_tile_marks`, concatenated by
`tools/rules-aggregate.mjs`. Layout:

```
rules/status/
  Cargo.toml
  src/lib.rs      # CARDS table, status_rule_ids echo, module docs
  src/stay.rs     # STATUS_STAY: CardDef
  src/stun.rs
  src/exile.rs
  src/fire.rs
  src/unstoppable.rs
  src/no_hand.rs
```

Each host is a `CardDef` (`card_sdk::CardDef`) with `On::Action` /
`On::Message` entries quoting its passage. Bound once via a new
`game_core::data::status_rule_ids()` (sibling of `mark_rule_ids`), appended
to the same `bind_tiles` loop that places `mark:*` (idempotent, `tile = -1`,
`BOARD_OWNER`).

### 3.3 Canonical card-facing API (`rules/status`)

One shape for every status. Thin wrappers over generic `ctx` calls
(counter / state / send / gates). **Not** in `card-sdk`.

```rust
// rules/status/src/lib.rs (re-exports)
pub mod stay { pub fn add(p: i32, n: i32); pub fn get(p: i32) -> i32; pub fn clear(p: i32); }
pub mod stun { pub fn add(p: i32, n: i32); pub fn add_start(p: i32, n: i32); /* stunStart */
               pub fn get(p: i32) -> i32; pub fn clear(p: i32); }
pub mod exile { pub fn send(p: i32, n: i32, to: i32);  // grant n layers, return tile
                pub fn get(p: i32) -> i32; pub fn clear(p: i32); }
pub mod fire { pub fn get(p: i32) -> i32; pub fn max(p: i32) -> i32;
               pub fn gain(p: i32, n: i32, why: &Msg) -> Result<i32, Prompt>;
               pub fn spend(p: i32, n: i32, why: &Msg) -> Result<bool, Prompt>;
               pub fn add_max(p: i32, n: i32) -> i32; }
pub mod unstoppable { pub fn add(p: i32, n: i32); pub fn get(p: i32) -> i32; }
pub mod no_hand { pub fn add(p: i32, n: i32); pub fn get(p: i32) -> i32; }

// generic gate readers (namespaced, replace can_pay / cant_move)
pub fn can_act(p: i32) -> bool;          // !stunned && !exiled && !out
pub fn move_blocked(p: i32) -> Option<Msg>;  // Some(why) when the main move is closed
pub fn can_pay(p: i32) -> bool;          // payment gate (stun/exile)
```

`give_extra_turn` is **turn order, not a marker**. It stays an engine verb,
renamed / namespaced to `ctx::turn::extra_turn(p)` (same wire, new guest
import name only if the old one is dropped in the ABI bump).

### 3.4 Engine: generic primitives + gates

The engine stops reading `stunned()` / `exile()` / `stay()` at policy sites.
Instead it reads **gates** the hosts maintain:

| gate | set by | read in place of |
|---|---|---|
| `act` (skills + [手]) | stun, exile | `stunned() \|\| exile()>0` in skill / hand-effect gates |
| `move` (main move) | stay (unless unstoppable), stun, exile | `skip_move` derivation, `cant_move` |
| `pay` (receive payment) | stun, exile | `can_pay`, `blocked`, payee filter (`play.rs` ~3260) |
| `on_board` (positioned / present) | exile | `exile()==0` in `present_from`, `players_on`, targeting |
| `targeted` (may be [指定]d) | exile | 「不能被[指定]」 |

Implementation sketch (Phase B):

* A `Gates` bitset / bool fields on `MatchPlayer` (or a small
  `state::gates` map), **not** a game-vocabulary name in the engine -- the
  engine only knows «this player cannot act / move / pay / be present».
* Hosts maintain gates from their counter's change hook
  (`CounterChanged` on the status key, or the existing `Stay` / `Stun` /
  `Exile` / `FireSpent` triggers). The un-latch of `skip_move` when
  [停留] hits 0 is `status:stay`'s handler, not `give_stay`.
* `cx.rs` `can_pay` / `blocked`, `mod.rs` the alive-seat filters, `play.rs`
  the turn-start / payee / teleport gates, `ops.rs` ~1013 all read gates.

**Adjustment (Phase B, recorded).** The sketch said «hosts set the gates».
The code proves the gates cannot live only on wasm hosts: `tick_state` and
the turn flow run **outside a card run**, so a host hook cannot update them
in time for `turn_start`'s stun / stay checks. So:

* Gates are **derived**, not stored: `MatchPlayer::status_gates() ->
  StatusGates` (`game-core/src/state.rs`) computes `act` / `move` / `pay` /
  `on_board` / `targetable` from the counters the hosts own.
* That function is the hosts' clause, kept engine-side for timing. The
  names stay / stun / exile appear **only** there and in `rules/status`.
* Policy sites read `status_gates()` and no longer branch on
  `stunned()` / `exile()` / `stay()`.
* `World::stay_unlatch` is the `status:stay` count-changed clause
  (un-skip on last layer); `give_stay` and `end_turn`'s `tick_state` call
  it. A true `CounterChanged` hook on the state key is the Phase C shape
  once state writes raise one.

**Generic turn-flow verbs** the hosts drive (and cards may call via
`ctx::turn`):

* `skip_main_move(p)` -- set the move gate for this turn (today `skip_move`);
* `end_turn_now(p)` -- the stunned / exiled auto-skip path;
* `extra_turn(p)` -- push `extra_turns` (engine order, not a marker).

**Ordering is frozen.** Today (`play.rs` `turn_start` / `end_turn`):

1. `turnStartBefore` -- before any status tick;
2. exile decrement; if layers remain -> log `log.exiled_skip`, `end_turn`
   (auto-skip); if 0 -> consume `exileTo` / `exileMain`, log
   `log.exile_over`, teleport (optionally as the main move);
3. `tick_state(Tick::TurnStart)` -- `stunStart` -> `stun` here;
4. `noHand` 2 -> 1;
5. `turn_snap` / `turn_start_pos` snapshot;
6. `turnStart`;
7. stun -> log `log.stunned_skip`, `end_turn`;
8. stay && !unstoppable -> `skip_move = true`, log `log.stay_skip`;
9. stage holds 开始 for a beat, then 运营.

End of turn (`end_turn`):

1. `turnEndBefore`;
2. `tick_state(Tick::TurnEnd)` -- stay / stun layers drop here;
3. `noHand` 1 -> 0;
4. `turnEnd` / `turnEndAfter` / `endTurnAfter`.

Phase B **must preserve this order** (hook order / new `TriggerKind`s if
needed) so the `rb_*` suite keeps passing. Exile resolves before `turnStart`;
stun / stay checks run after it.

### 3.5 What leaves the engine

| today | tomorrow |
|---|---|
| `ops.rs` `give_stay` (incl. the un-skip latch) | `status:stay` host + generic `skip_main_move` |
| `ops.rs` `give_stun` | `status:stun` host |
| `ops.rs` `give_exile` | `status:exile` host |
| `ops.rs` `gain_fire` / `add_fire_max` | `status:fire` host |
| `wasm_rules` `spend_fire` + `markerSpend` | `status:fire` host (window stays generic `HostRequest::Marker`) |
| `wasm_rules` `can_pay` / `cant_move` | generic gates + `status::can_act` / `move_blocked` |
| `play.rs` exile tick / return teleport | generic turn-start tick + `status:exile` hook / engine verb `teleport_as` |
| `play.rs` stun skip / stay skip | gates (hosts maintain); engine reads gates |
| `state.rs` `is_status_key` / `default_expiry` | stay: expiry stays item-carried (`Tick::TurnEnd` etc.); clamping floor-at-0 moves to the host verbs (or stays as the generic `state_set` floor -- see §6) |
| `key::{STAY,STUN,...}` constants | **kept** (wire + host vocabulary); the engine just stops *branching* on them |

`give_extra_turn` / `extra_turns` stays engine-side (turn order).

### 3.6 cond vocabulary (`rules/cond`, `game-rules/cond_pre.rs`)

Today `cond_pre.rs` exposes `stay` / `stun` / `exile` as bespoke CEL fields
(`owner_stay`, `stay(seat)`, ...). Target: read them through the generic
counter / state vocab. **Feasible without regressing only if** the CEL
surface keeps the same names as sugar over `state_get`. Plan: leave the
names, re-implement the accessors as sugar over the generic state reader
already used for `exile`; no bespoke engine field. If a CEL regression shows
up in Phase D, keep the accessors and note `TODO(规则书)` / `TODO(ABI)` here.

### 3.7 ABI / save / replay

* Phase B: old `ctx::{give_stay, ...}` still work (host-backed shims). Suite
  green. No ABI bump.
* Phase C batch 1 (done): every card / skill / tile / event / fixture call
  site now goes through `rules_status::{stay,stun,exile,fire}::*` or the
  generic gate readers. No card names `ctx::give_stay` etc.
* Phase C batch 2 (leftover): the old guest imports stay for now, marked
  `#[doc(hidden)]`, because `give_stay` / `give_stun` / `give_exile` open the
  abnormal [反击] window (`abnormal_gate`) and `gain_fire` / `spend_fire` /
  `add_fire_max` own the cap / log / `markerSpend` service. They are the
  **status hosts' engine services**, not the card-facing API.
  TODO(ABI): genericize to `apply_abnormal(p, AbKind, n, to)` +
  `marker::{gain_capped,spend}` + `clear_move_skip`, then delete the
  game-named imports (HOST-09/10 dead-surface removal) and bump
  `ABI_VERSION` 53 -> 54.
* `give_extra_turn` stays an engine verb (turn order, not a marker).
* `SAVE_VERSION` bumps only if the serialized `MatchState` shape changes.
  Keeping `MatchPlayer.state` as storage means **no save bump** in the
  default plan. Gates are derived (not stored) -- no shape change.
* Replays stay backward compatible: archived engine bundles keep replaying
  old records; do not break the record / save wire.

### 3.8 webui

Reads (frozen):

* `webui/src/core/names.ts` -- `stateOf(x, key)`, `stateMax(x, key)`;
* `webui/src/scenes/board/Players.tsx` -- stay / stun+stunStart / exile /
  fire:fireMax badges;
* `webui/src/scenes/board/Popups.tsx` -- same keys in the player popup;
* `webui/src/game/autopilot.ts` -- `canPay` from `stun` / `exile` values.

No webui change in this batch: the keys and their `{value, min, max,
expires}` shape stay. If a gate field is added to the public view later, it
is additive.

---

## 4. Ordering table (observable, must not drift)

| step | who today | who tomorrow | trigger raised |
|---|---|---|---|
| turn begins | `next_turn` | same | `TurnBegan`, `log.turn` / `log.turn_extra` |
| before status tick | `turnStartBefore` | same | `turnStartBefore` |
| exile layer tick | `play.rs` names `exile` | `status:exile` wear-off via generic tick + hook | `log.exiled_skip` / `log.exile_over` |
| exile return teleport | `play.rs` + `exileMain` | `status:exile` hook + engine `teleport_as` | walk / teleport triggers |
| `stunStart` -> `stun` | `tick_state(TurnStart)` | same (item expiry) | -- |
| `noHand` 2->1 | `play.rs` | `status:no_hand` hook | -- |
| snapshot | `turn_snap` | same (reads the keys) | -- |
| `turnStart` | same | same | `turnStart` |
| stun auto-skip | `play.rs` names `stunned()` | engine reads `act`/`move` gate | `log.stunned_skip` |
| stay skip | `play.rs` names `stay()` | engine reads `move` gate (host sets it) | `log.stay_skip` |
| ops / move / settle | engine | same, gates consulted | -- |
| `turnEndBefore` | same | same | `turnEndBefore` |
| stay/stun layer tick | `tick_state(TurnEnd)` | same (item expiry) | -- |
| `noHand` 1->0 | `play.rs` | `status:no_hand` hook | -- |
| `turnEnd` / `turnEndAfter` / `endTurnAfter` | same | same | those |

---

## 5. Migration order

Call-site census (`ctx::` wrappers in `rules/`, excluding `card-sdk`
definitions): `give_stay` 15, `give_stun` 10, `give_exile` 11, `gain_fire`
45, `spend_fire` 42, `stay_of` 6, `stun_of` 7, `exile_of` 2, `cant_move` 35,
`can_pay` 6, `give_extra_turn` 3, `add_fire_max` 2, `fire_max` 4, `fire` 8.
Plus tiles / events / skills / fixtures (~235 grep hits including
definitions and comments).

### Phase A (this doc) -- investigate, no implementation edits

* map every engine read/write of the status keys (§3.4 / §3.5);
* map every ordering dependency (§4);
* this document. **Commit.**

### Phase B -- engine primitives + host crate, old API still works

1. `rules/status` crate + hosts (stay / stun / exile / fire / unstoppable /
   no_hand), bound via `status_rule_ids` -> `bind_tiles`.
2. Generic gates on `MatchPlayer` (serde-default) + generic turn verbs
   (`skip_main_move`, `end_turn_now`, `extra_turn` renamed).
3. Host-backed shims: `ctx::{give_stay, ...}` keep their signatures and wire
   names; they route through the hosts (or stay engine-backed with the hosts
   listening). Full `cargo test` green.
4. Un-latch of `skip_move` moves into `status:stay`'s count-changed handler.
5. `can_pay` / `cant_move` become gate readers.

### Phase C -- migrate cards, drop old ABI, bump

Migrate per band crate (smallest first so the shim surface shrinks
visibly), then tiles / events / skills / fixtures:

1. `card-pp`, `card-morfonica`, `card-ag`, `card-hhw`, `card-crychic`;
2. `card-ppp`, `card-ras`, `card-sumimi`, `card-general`;
3. `card-roselia`, `card-mygo`, `card-mujica`;
4. `tiles`, `events`, `skills/`, `fixtures/`;
5. delete the old `card-sdk` wrappers + guest imports (HOST-09/10 style);
   bump `ABI_VERSION` 53 -> 54; docs note.

One commit per batch.

### Phase D -- tests

* `cargo test` for `game-core`, `game-rules` (including `tests/rb_*.rs`;
  an `#[ignore]`d discrepancy stays ignored -- record new findings in
  `docs/rulebook/TEST-FINDINGS.md`), `rules` workspace;
* wasm card build via `node tools/build-ruleset.mjs` (**never** export
  `CARGO_TARGET_DIR`);
* new host tests: tick (stay / stun TurnEnd, stunStart TurnStart, exile
  TurnStart), clamp (floor 0), gate (move / act / pay / on-board), un-latch
  (last stay layer restores `skip_move`), exile return **as** the main move
  (`exileMain`), fire cap / partial gain / `markerSpend` cancel, stun
  payee refused (`play.rs` ~3260 shape).

---

## 6. Open questions / TODO(规则书)

| # | clause | today | note |
|---|---|---|---|
| 1 | [除外] 「该效果生效时无法重复获得该效果」 (`rules.txt` 50) | `give_exile` stacks layers | Conservative reading: already-exiled player gains **no** new layer. Change in `status:exile`. Tag every site that still stacks. |
| 2 | [不可阻挡] 「可选择受到的[传送]，[强制移动]，[强制停下]效果是否生效」 | not implemented | Separate prompt surface; out of this batch. |
| 3 | [无法移动] 「移动终点视为当前所在格子且不[结算]」 vs "cannot roll" | `skip_move` closes the main move entirely | Matches the observed engine; the book's wording is about the endpoint being "here". Keep current behaviour; quote both. |
| 4 | 「无手牌」 glossary text | `noHand` 2->1->0 dance | Not in `rules.txt` 「标志物解释」; source is card text. Host keeps the dance. |
| 5 | cond CEL `exile` field vs generic counter vocab | bespoke accessors | Sugar over `state_get` is fine; only a real vocab merge needs a TODO. |
| 6 | `is_status_key` floor-at-0 | engine-wide for stay/stun/stunStart/exile/fire | Keep as the generic `state_set` floor (it is already "the status rule, not the engine enforcing bounds" per the comment). Hosts additionally clamp in their verbs. |
| 7 | fire cap source | `StateVar.max`, written by the character skill | Book says 「持有上限」 is mandated by the skill text. Unchanged. |

---

## 7. API diff (target)

### Hidden in `card-sdk` (host services; cards must not call)

`ctx::fire`, `ctx::fire_max`, `ctx::gain_fire`, `ctx::spend_fire`,
`ctx::give_stay`, `ctx::give_stun`, `ctx::give_exile`,
`ctx::stay_of`, `ctx::stun_of`, `ctx::exile_of`, `ctx::can_pay`,
`ctx::cant_move`, `ctx::add_fire_max` -- `#[doc(hidden)]`, called only by
`rules/status`. `ctx::give_extra_turn` stays a documented engine verb.

### In `rules/status` (new)

`status::stay::{add,get,clear}`, `status::stun::{add,add_start,get,clear}`,
`status::exile::{send,get,clear}`, `status::fire::{get,max,gain,spend,add_max}`,
`status::unstoppable::{add,get}`, `status::no_hand::{add,get}`,
`status::can_act`, `status::move_blocked`, `status::can_pay`.

### Engine verbs (generic)

`ctx::turn::extra_turn`, `ctx::turn::skip_main_move`, `ctx::turn::end_turn_now`
(wire names TBD in Phase B; keep old `give_extra_turn` as a shim until
Phase C).

### Unchanged wire

`MatchPlayer.state` keys `stay`, `stun`, `stunStart`, `exile`, `exileTo`,
`fire`, `handLimit` (and their `StateVar` shape). Trigger names
`stay` / `stun` / `exile` / `fireSpent` / `markerSpend` stay.
---

## 8. Status (2026-10-13)

Landed on `status-hosts-v2` (rebased over the closeout batches, ABI 57):

* `rules/status` hosts `status:{stay,stun,exile,fire,unstoppable,no_hand}`
  linked into `card-all`, `rules-native` and the aggregate build (native and
  sandbox manifests agree: 291 cards).
* Every card / skill / tile / event call site uses `rules_status::*`; the
  `ctx::{give_stay,…,cant_move,can_pay}` imports are `#[doc(hidden)]` host
  services. The never-implemented `clear_move_skip` import was dropped.
* `[除外]` is **not** part of the `skip_move` latch (closeout-2 ruling):
  `World::stay_unlatch` follows 停留/不可阻挡 only; `status_gates().move` still
  includes exile.
* Remaining `TODO(ABI)`: genericize `give_*` / `gain_fire` / `spend_fire` into
  `apply_abnormal` + `marker::*` and bump the ABI (§3.7).

---

## 9. Gates are CEL-only `On::Gate` entries (ABI 58+)

The act / pay / move gates are no longer `MatchPlayer::status_gates()` read at
policy sites. Each `status:*` host declares the gates it holds as **CEL-only**
`On::Gate` entries (`TriggerKind::{ActGate, PayGate, MoveGate}`):

| host | gates | condition |
|---|---|---|
| `status:stay` | `MoveGate` | `state(actor,'stay') > 0 && state(actor,'unstoppable') == 0` |
| `status:stun` | `ActGate`, `PayGate`, `MoveGate` | `state(actor,'stun') + state(actor,'stunStart') > 0` (stun + the pending `stunStart` layer) |
| `status:exile` | `ActGate`, `PayGate`, `MoveGate` | `state(actor,'exile') > 0` |

* **CEL only.** The condition is the whole answer: the ruleset load refuses a
  gate entry with a residual guard or without a condition
  (`host::validate_cel_only_gates`). That is what lets the engine ask the gate
  in a pure `&self` read (the view, rollouts, `Cx::can_pay`) and a card guard
  ask it through `rules_status::can_pay` -- a wasm guard could not be run
  there, and a gate cannot be *raised* from inside a pure guard.
* **Vocabulary.** A condition reads other statuses (`stay` needs
  `unstoppable`), so the CEL vocabulary exposes the generic
  `state(p, 'key')` -- a seat's player-state value for an arbitrary string
  key, missing = 0 (`docs/GUARDS.md`). The IR carries it in a separate
  `(seat, key, value)` table (`STATE_CAP = 64`).
* **Engine side.** `CardRules::gate_open(world, Gate, player)` -> `World::gate_open`.
  The wasm ruleset evaluates the conditions over a players-only window
  (`game_rules::gate`); `StubRules` returns `None` and the engine falls back to
  `MatchPlayer::status_gates()`, the same clauses in Rust.
  `tests/status_gate_drift.rs` pins the two equal on every layer combination.
* **Still engine-side:** `on_board` / `targetable` ([除外] 「不失为存在于任何
  格子上」, 「不能被[指定]」) and the stay latch (`give_stay` / `stay_unlatch`).
  `TODO(规则书)`: give them gate kinds too.
* **No guest `ActGate` reader.** `rules_status::can_act` was removed (it was an
  alias of `can_pay` with no caller). `TODO(ABI)`: a generic
  `gate_open(kind, player)` import would serve both.
* **ABI 60 `ExclusiveUse`.** The same CEL-only shape, one more gate kind, asked
  when an exclusive card is about to be played. It is a **band-skill** rule
  host (CRYCHIC's variant/base denial lives on `skill:CRYCHIC:…`), not a status
  host -- see `docs/GUARDS.md` §Exclusive-use gate for the window shape, the
  engine's default own/base verdict, and why deck building keeps the
  `sharesExclusives` data trait instead of a second gate.
