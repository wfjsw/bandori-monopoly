# ABI design G09 — Tile, build, pass and house-removal hooks

**Status: draft for user review. Design only — no source edits.**
Worktree `/home/monopoly/web/.claude/worktrees/worktree-g09-design-review`, branch
`worktree-worktree-g09-design-review`.
**Group:** G09, family "Tile, build, pass and house-removal hooks".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below was re-read
from that file against the card id named beside it.
**Base:** `ABI_VERSION` 113, `SAVE_VERSION` 16, `RECORD_VERSION` 4, engine kinds
through 131, Kani `Op` through 450 (G01–G08 and G05/G13/G06/G07 landed).

Terminology: **counteraction** = the [反击] window / chain link; **hook** = a
passive listener (`On::Action` / `On::Filter` / a `[持续]` field body). Never
"reaction".

**Naming.** No identifier in this design abbreviates a word: *position*, not
`pos`; `position_of_tile`, not `tile_pos`; `numerator` / `denominator`, not
`num` / `den`; `build_cost_percent`, not `build_cost_pct` (the pre-existing
setter `set_build_cost_pct` keeps its spelling — it is an import already).
Existing abbreviated names (`Trigger.tile`, `mv`, `MoveCtx.path`) are left as
they are; only what this group adds is spelt out.

**Identity vocabulary.** A **tile** is a board identity — the static data row,
its owner, houses, mortgage, props and its rule instance all belong to the tile
and never move. A **position** is a slot on the board loop where a tile
currently stands. The two coincide until item 2's swap exists; after it they
must never be confused. Where a reader takes a tile the doc says *tile*, where
it takes a slot it says *position*.

**Closed slugs and rows:** all six items are covered.

| # | slug | severity | rows |
|---|---|---|---|
| 1 | `marina-suppression-window` | medium | 880, 885, 1493 |
| 2 | `parade-tile-swap` | medium | 1089 |
| 3 | `build-pipeline-queries` | medium | 118, 123, 124, 521, 748, 1206 |
| 4 | `tile-write-effect-declare` | medium | 36, 38, 989 |
| 5 | `guerrilla-pass-path` | medium | 866 |
| 6 | `house-removed-kind` | medium | 819, 826 |

**Demotions: none.** Each item needs at least one engine-side mechanism that
does not exist ([`§1`](#1-problem-per-item) states why per item). Item 4 is the
closest to A — the guest already reads declared effects — but the engine's tile
writes raise no `effect` window at all, so a card cannot declare at its own call
site and the fix is host-side.

---

## 1. Problem, per item

### 1.1 `marina-suppression-window` (rows 880, 885, 1493)

Book (`通用:[月岛麻里奈]今天也要加油工作喔`):

> [反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。

**Current.** `marina_work.rs` counteracts `bandori_std::kind::PassRewardAffected`
with `condition::MINE` and no residual guard; its body only logs. Both missing
conjuncts are unreadable and the window is at the wrong point:

* the only raise is inside the CiRCLE rule's reward path, *after* the reward
  pick (`circle.rs`, `settle_reward`). A pass onto #1 from #1, a pass vetoed by
  the rule's own pay gate, a `NO_REWARD` arm, or any pass whose reward is
  skipped never raises, so its settle cannot be answered either (rows 880, 885);
* nothing reads a suppression flag at any tile extra-effect point, so the body
  cannot make the pass/settle "正常进行而不受到其上的额外效果" (row 1493);
* there is no guest reader for "does this tile carry extra effects", so the
  guard would be forced to hard-code a card list.

**Expected.** A window opens on the pass/settle of the tile itself, independent
of the rule body's own pay gate / `NO_REWARD` / the reward pick. The counter
sets a one-shot move tag; every tile extra-effect point for that tile skips the
extra instances while it is set. The tile's own base rule still runs.

### 1.2 `parade-tile-swap` (row 1089)

Book (`HHW:笑容大游行` (3)):

> [持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。

**Current.** `smile_parade.rs:188-245` hand-rolls a **settle** swap: the
`SettleBody` entry claims the body with `set_cancelled`, then calls
`settle_tile_at` on the partner tile and suppresses re-entry with a move tag.
The [经过] half does not exist: passing the card's slot still runs that slot's
own pass rule, and passing 弦卷集团 still runs none of the card's slot's. The
`[持续]` clause is one continuous statement; only half of it is implemented
(row 1089; `BACKLOG.md` ABI-13, which already scopes the capability and its
"must keeps").

**Expected.** While the card sits there, the slot is 弦卷集团 for [经过] as well
as [结算]. Tile identity stays static — `ctx::is_start`, `tile_is`,
`prop::START` and cards keyed *on* CiRCLE do not ride the swap.

### 1.3 `build-pipeline-queries` (rows 118, 123, 124, 521, 748, 1206)

Book fragments:

`HHW:微笑巡逻队`:

> 付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）

`MyGO:难以复刻的奇迹` (1):

> （1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上，当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。

`R:向着顶点`:

> 若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。

**Current.** Three card builds take three different thin paths, and the
engine's own build pipeline is a fourth copy of the same stages:

* `smile_patrol.rs:61-62` tests only `is_buyable` + `!mortgaged` and then calls raw
  `ctx::add_house`; `prop::NO_BUILD` / `prop::NO_BUILD_ABOVE` / ring / tier caps
  do not veto (rows 118, 123). Its play gate and `to_you_far_away.rs` price
  affordability with raw `ctx::build_cost`, while an actual build charges
  `build_cost_pct` / `build_discount` — the two disagree (rows 521, 748).
* `miracle.rs:79-92` wants "a crystal *instead of* money" but has no effective
  price reader (`build_cost` is base; `build_cost_pct` / `build_discount` are
  write-only), so it always zeroes and always spends (row 124).
* `to_the_peak.rs:97-143` computes 1.5× itself, pays with `ctx::pay`, then calls
  raw `ctx::add_house` — no `buildBefore`, `buildAfter` or `houseAdded` from that
  path; `ctx::card_build` cannot express the 1.5× because it runs the rule
  body's own price (row 1206).
* `card_build` (`play.rs:1793`) gates but then calls `deed_build`, which
  dispatches the tile rule body; the body (`property.rs:169-207`) hand-assembles
  `buildBefore` → `pay_build` → `add_house` → `houseAdded` → `buildAfter`. The
  same stages exist a second time in `build()` (`play.rs:3640-3753`). Either
  copy is reachable by cards only through the two-argument `card_build`.

**Expected.** One build pipeline. A card build is buildability-gated by the
same gate as the build step, priced by the effective figure, may carry an
explicit charge, and runs `buildBefore` / `houseAdded` / `buildAfter`.
Everything the three cards need is a read or a parameter, never a second body.

### 1.4 `tile-write-effect-declare` (rows 36, 38, 989)

Book (`AG:宣战布告`):

> [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。

**Current.** `declare_war.rs:59-66` already reads the whole effect list —
`ctx::effect::hits(player_id)` for the seat half and `ctx::effect::tile(i)` +
`ctx::tile_owner(t)` for the 「或你拥有的格子」 half. But tile writes
(`wasm_rules.rs:2049-2105` `set_houses` / `add_house` / `set_owner`) mutate the
world copy directly: no `Effect` is appended and no `effect` window opens.

**Expected.** Every tile write declares
`Effect { kind, tile, target: owner }` on the running link **and opens the
`effect` window** before applying, so `effect::count/tile/has` guards see it.
A cancellation in that window voids the write.

### 1.5 `guerrilla-pass-path` (row 866)

Book (`RAS:游击演出` [手]):

> [反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后：传送至任意无主可购买的格子并[结算]，且必须购买。

**Current.** `guerrilla.rs:80-100` checks `Paid` / payer / payee /
`pay_is_rent` / no `by_card` and the free pool. Any rent to another player
opens it; the passed Live House is never tested (the TODO at `:96-98`).
`TriggerMove` carries no path, and the rent `paid` trigger raised during a
settle carries no move at all (`play.rs:4707-4717` sets no `@m`), so the card
cannot tell a settle's own rent from an unrelated payment (row 866;
`BACKLOG.md` LOY-GUERRILLA-01).

**Expected.** The settle-causing move's full path is on the `paid` trigger;
the guard requires a path tile that is a Live House owned by another player.

### 1.6 `house-removed-kind` (rows 819, 826)

Book (`R:（燐子）Ringing Bloom` (2)):

> 你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半

**Current.** The 「视为」 is recomputed on `HouseAdded` and `Bought`
(`ringing_bloom.rs:30-40,112-144`). A razed max tile leaves `RENT_HOUSES` and
the half-charge set at the old maximum until the next build; a sale does the
same (rows 819, 826; `BACKLOG.md` CRYSTAL-08). `add_house` logs a rise only
(`wasm_rules.rs:2060-2087`) and `set_houses` logs nothing at all; there is no
fall kind to listen to.

**Expected.** A fall re-runs the view exactly like a rise. The kind is the
symmetric half of `houseAdded` and fires on every downward house write.

---

## 2. Shared root causes

* **R1 — a tile has an identity and a set of effects, with no layer between
  them.** Dispatch knows only "all board instances on `trigger.tile`" plus
  "every player field instance, guard-filtered" (`wasm_rules.rs:5983-6230`).
  Nothing can say "the tile's own rule" versus "an extra effect on the tile",
  and nothing can make one slot run another tile's rule. Items 1 and 2.
* **R2 — pass and settle steps have no shared, early tile window.** The only
  "affected" window is buried in a rule body after a choice; a card that must
  see the *tile event before any extras* has nowhere to stand. Item 1.
* **R3 — tile writes are raw.** `set_houses` / `add_house` / `set_owner` are
  direct array writes on the world copy: no declared effect, no window, and the
  rise log is one-directional. Items 4 and 6.
* **R4 — card-initiated builds are a second, thinner pipeline.**
  `add_house` commits a house with no gate, no price stage and no hooks;
  `card_build` gates but can only charge the rule body's own price. Items 3
  (and 6, because `add_house` is the shared write).
* **R5 — move facts are step-local snapshots.** `TriggerMove.tags` is copied at
  raise time (`rules.rs:637-663`), but `plan::set_tag` only writes the plan and
  the walk syncs only `stop_at` / `settles` (`play.rs:2165-2172`);
  `MoveCtx.path` never reaches a trigger, and money raised during a settle
  carries no move. Items 1 and 5.
* **R6 — the event-kind set is rise-only.** `HouseAdded` (75) exists; its fall
  does not. Item 6.

Every root cause is generic mechanism: no display text, no player decision
inside an import.

---

## 3. Capability as an invariant

> **A tile event resolves one governing tile; the tile's own rule runs, every
> extra effect on it is skippable as one set, every write to it is a declared
> effect with a rise/fall pair, and every build — engine or card — runs one
> ordered pipeline that the card can price but not bypass.**

Derived obligations:

* **Governing resolution is total and stable.** A pass or settle at position
  `p` resolves `tile_at(p)` (the swap partner when one is bound, otherwise
  `p`), and *every* instance dispatch and tile-fact read of that event uses the
  tile. Position is used for walking, `landed`, co-location and the one
  card-level "this physical slot was settled" test; nothing else. Display
  *names* carry the tile (a log names the tile the event is about, and the
  view's own landing highlight stays positional).
* **The tile is the identity, the position is only where it sits.** The tile's
  identity never changes: `tile.id`, `ctx::is_start`, `tile_is` and every
  identity fact (`tile_named`, ownership, house arrays, props, the tile's rule
  and its extras) stay bound to the tile, and a swap only changes which position
  answers at it. An identity reader fed a position converts through `tile_at`;
  a physical reader fed a tile through `position_of_tile`.
* **The base rule is never an "extra".** An instance is an *extra* iff it is
  placed on the governing tile (`FieldCard.tile == trigger.tile`) and its card
  id does not start with `tile:`. The tile carries its effect set: a swap moves
  the set with the tile, so dispatch and suppression are keyed by the tile,
  never by the event position. The suppression tag hides exactly the extras of
  exactly that tile.
* **The tile window precedes every extra.** `tileEffectBefore` is raised before
  the step's first tile raise (`passBefore`), and before the settle's `settle`
  dispatch, so a counter can arm suppression and every later stage of that tile
  sees it.
* **One write path per tile fact.** `houseAdd` / `houseRemove` / `ownerSet`
  declare an `Effect`, open `effect`, then apply through one helper; a rise
  raises `houseAdded` and a fall raises `houseRemoved`, once each, after commit.
* **One build pipeline.** Gate → `buildBefore` → price → charge → commit →
  `houseAdded` → `buildAfter`, with the gate and the price shared by the engine
  step and every card verb. A card may choose *what* is charged (none,
  effective, fixed, effective×ratio); it may not skip a stage.
* **All of a move's facts ride every trigger it raises.** Tags are synced from
  the plan at every tile raise, every trigger carries the path taken so far,
  and a trigger raised inside a settle inherits the settling move and its full
  path.
* **Rise and fall are symmetric.** A kind that announces a write has its
  inverse; a write with no delta raises neither.

---

## 4. Surface

Everything below is generic mechanism. No display text. No player decision is
made inside an import: `card_build_as` charges, hooks and the card decide.

### 4.1 New engine kinds — `crates/card-sdk/src/abi.rs`, `crates/game-kinds`

Wire ids **132–136** (fresh band above the highest id in use, 131; see §8 for
the README allocation shift). All get an `EngineKind` row in
`crates/game-kinds/src/lib.rs` beside 75, with `is_filter: false` /
`hook_only: false` like their rise sibling `houseAdded`. 132–133 are dual-role:
a `TriggerKind` const (raise site, `ChainKind`) **and** an `ActionKind` const
(the same id, the `Self(TriggerKind::X)` alias form `HouseAdded` uses at
`abi.rs:2471`). 134–136 are `Effect.kind` spellings only — never raised, so they
get a `TriggerKind` const for `effect::kind` / `effect::has` and the wire name,
but no raise site.

```rust
// TriggerKind — engine namespace
/// `tileEffectBefore` -- before a tile's extra effects run, on a pass step and
/// on a settle. Payload: `tile` (the governing tile), `position` (the board
/// slot), `mv` (the move/step). Dual-role: a counteraction window and a hook.
pub const TileEffectBefore: Self = Self::engine(132);

/// `houseRemoved` -- a tile's house count has just fallen.
/// Payload: `tile`, `value` (the count after the fall).
/// `player_id` is the tile's owner at the time of the write.
pub const HouseRemoved: Self = Self::engine(133);

// effect-only spellings (`Effect.kind`), one per write:
pub const HouseAdd: Self = Self::engine(134);    // value = delta, target = owner
pub const HouseRemove: Self = Self::engine(135); // value = delta, target = owner
pub const OwnerSet: Self = Self::engine(136);    // value = new owner

// ActionKind — same ids, the `Self(TriggerKind::X)` alias form:
pub const TileEffectBefore: Self = Self(TriggerKind::TileEffectBefore);
pub const HouseRemoved: Self = Self(TriggerKind::HouseRemoved);
```

`engine()` is the fixed-generic raise-point namespace (`abi.rs:3274`); these
five are the fixed generic points this design adds and no card owns them.

### 4.2 `Trigger` and `TriggerMove` — `crates/game-core/src/engine/rules.rs`

```rust
pub struct Trigger {
    // …existing…
    /// The board position of this event (the slot the mover stepped onto /
    /// the settle ran at), `-1` on kinds with no position. `tile` is the
    /// governing tile resolved from it; when no swap is bound the two are
    /// equal.
    pub position: i32,
}

pub struct TriggerMove {
    // …existing…
    /// Every governing tile this move has stepped on so far, in order,
    /// pushed at the same point as `MoveCtx.path` (so a `passBefore` of the
    /// step not yet taken does not see it; its `passTile` and settle do).
    /// Tiles, not positions (item 5).
    pub path: Vec<i32>,
}
```

`with_move` (`rules.rs:637`) copies `m.path` into `path`. Both are host-only
structs (never serialized — see §7). `Trigger::default` initializes `position`
to `-1` like `tile` (`rules.rs:536`), and every hand-written literal
(`wasm_rules.rs:9207` the bridge, `:11257` `drive_tile_op`, `:11446`
`settle_tile`, `:11931` / `:11994` the roll drivers) sets it too.

### 4.3 Raise-site arms — `rules.rs:680`

Two arms inserted before the generic `$field = $value` arm (other fields such as
`value` / `target` / `effects` keep using it). The arms need the context, so the
entry arm forwards it (`raise!(@set $cx, __trigger …)`) and the arms bind
`$cx`:

```rust
(@set $cx:expr, $t:ident, position = $p:expr) => {{
    $t.position = $p;
    $t.tile = $cx.tile_at($p);
}};
(@set $cx:expr, $t:ident, tile = $s:expr) => {{
    $t.tile = $s;
    $t.position = $cx.position_of_tile($s);
}};
(@set $cx:expr, $t:ident, slot = $p:expr) => {{
    $t.position = $p;
}};
```

`PlayCtx::tile_at` / `PlayCtx::position_of_tile` delegate to the world (§5.1).
Arms assign in written order, so a site that already knows both writes
`position = p, tile = s`; the explicit `tile` wins and derives the position.
`slot = p` exists for the case the derivation gets wrong: one that knows both
**independently**, because the position is not `tile`'s standing slot. The only
such site is `tileResolved` (`play.rs`'s `settle_at`), which carries the
governing tile yet whose "this physical slot" test must see the slot the settle
ran at -- `tile = tl, slot = at` (the `tile` arm's derivation would rewrite it
to `position_of_tile(tl)`).

### 4.4 Constants

```rust
// card_sdk::abi::prop — an *instance* prop (dies with the instance)
pub const SWAPPED_TILE: &str = "swappedTile"; // value = partner tile

// card_sdk::abi::tag — a new module beside `prop`, for shared move-tag names
/// Suppress the extras of tile `value` for the rest of this move.
pub const SUPPRESS_TILE_EFFECTS: &str = "suppressTileEffects";
```

`prop::SWAPPED_TILE` on a placed instance binds that instance's tile to the
value tile, one level, both directions. `tag::SUPPRESS_TILE_EFFECTS` with value
`t` suppresses extras on tile `t` for the rest of the move. The module holds
only names the engine and cards must agree on; a card's private move tag may
keep a local constant.

### 4.5 Imports (`crates/card-sdk/src/ctx.rs`, re-exported via `bandori_std::ctx`)

Twelve new imports, Kani `Op` 452–463 (§5.7).

```rust
// ---- identity / position conversion
/// The board position of this event (the slot the mover stepped onto / the
/// settle ran at), `-1` where there is none. `tile.id` is the governing
/// tile; the two are equal with no swap. Identity readers (`is_start`,
/// `tile_is`, `tile_named`) take the governing tile (`tile.id`); the
/// position is for co-location, display and "was this physical slot the one
/// settled" tests.
pub fn trigger::position() -> i32;

/// The board position tile `t` currently stands at (`t` itself when nothing
/// is bound to it) -- the value of the engine's `position_of_tile`.
pub fn position_of_tile(t: i32) -> i32;

/// The tile standing at board position `p` (`p` itself when nothing is bound
/// there) -- the value of the engine's `tile_at`.
pub fn tile_at(p: i32) -> i32;

/// How many *extra* effects lie on tile `t`: instances whose card is not the
/// tile's own `tile:` rule (a player card through the tile standing at its
/// slot, a board instance by its own tile) plus tile marks. The tile carries
/// its extras, so the count is keyed by the **governing** tile and a swap
/// moves it with the tile, not the slot.
pub fn tile_extra_count(t: i32) -> i32;

/// The board position **this instance** sits at (its placement slot). The
/// slot, where `self_tile()` is the tile standing there.
pub fn self_position() -> i32;

// ---- build reads
/// The price a house on tile `t` would actually cost this turn:
/// `max(0, build_cost * build_cost_percent / 100 - build_discount)`. No payer
/// side effects, no layer pop.
pub fn build_price(t: i32) -> i32;
/// The current turn's build percentage (100 = full) and layered cut.
pub fn build_cost_percent() -> i32;
pub fn build_discount() -> i32;
/// `cant_build_on` minus the ownership test (still includes the buyable,
/// ring, `NO_BUILD`, `NO_BUILD_ABOVE`, lingering, mortgage, cap, full and out
/// refusals; the `Gate::Pay` layers remain the caller's question). For a free
/// build on someone else's tile.
pub fn tile_buildable(player_id: i32, t: i32) -> bool;

// ---- the one card-build verb
/// Run the full build pipeline (§5.3) on tile `t`: gate, hooks, price,
/// commit. `charge` is a [`BuildCharge`]; `Fixed` uses `amount`, `Scaled`
/// uses `numerator / denominator` of the effective price; `owner_required`
/// adds the owner test to the gate. `true` iff a house committed.
/// Does not re-dispatch the tile's `On::Build` body.
pub fn card_build_as(
    player_id: i32,
    t: i32,
    charge: BuildCharge,
    amount: i32,
    numerator: i32,
    denominator: i32,
    owner_required: bool,
) -> bool;

/// The existing two-argument import (`ruleset_card_build`, `sys.rs:1764`),
/// kept unchanged as the `Effective` / owner-required alias. Archived bundles
/// import the symbol directly; new bodies may keep using it (§8).
pub fn card_build(player_id: i32, t: i32) -> bool;

// ---- move facts
pub fn trigger::move_path_length() -> i32;
pub fn trigger::move_path(i: i32) -> i32; // the governing tile at index i
```

`Fixed` needs `amount` *and* `Scaled` needs a ratio, so the verb takes both
pairs; the unused one is `0`. Unknown `charge` values fall back to `Effective`.

SDK-side sugar (not an import, so no `sys.rs` row): `set_swapped_tile(partner)`
is `set_prop(prop::SWAPPED_TILE, partner)` on the caller's own instance; `-1`
clears.

New enum:

```rust
#[repr(i32)] // wire convention, like `BuyKind`
pub enum BuildCharge {
    Effective = 0, // pay the pipeline price (percent, discount)
    Free = 1,      // pay nothing; hooks still run
    Fixed = 2,     // pay exactly `amount`; percent/discount do not apply again
    Scaled = 3,    // pay effective × numerator / denominator, one ceil-to-10
}
```

The engine round-trips `paid.loss` and clamps all charges at 0.

### 4.6 Classification

* `tileEffectBefore` and `houseRemoved` are **action** kinds with a hand
  window (like `houseAdded` and `settleBody`); the effect-only spellings never
  raise.
* `card_build_as` is an action verb (hook-running, may prompt through the money
  pipeline); `build_price` / `build_cost_percent` / `build_discount` /
  `tile_buildable` / `tile_extra_count` / `position` / `position_of_tile` /
  `tile_at` / `move_path*` are pure reads.
* `ctx::set_swapped_tile` is sugar over `set_prop`; no import.
* Condition vocabulary (`crates/game-rules/src/condition.rs:1454`, the
  `TileSnap` scope in `crates/cond/src/ctx.rs:68`): `tile.position` joins
  `tile.id`; the initializer gains `position: t.position as i64` beside `id`.
  `tile.id` stays the tile, `tile.position` is the event position. No other CEL
  field changes; a placed card's `card.tile` is already its tile.

### 4.7 State

* `World::swap_binding_at(position) -> Option<(i32, i32)>` (the identity
  standing there and the binding instance uid), `World::tile_at(position) ->
  i32` and `World::position_of_tile(tile) -> i32`. Derived from instances
  carrying `prop::SWAPPED_TILE`; no new world field (§7).
* No new settle stack: the hint rides the existing `SettleEntry` —
  `SettleEntry.move_context: Option<Move>` (5.6), which costs the struct its
  `Copy` (it becomes `Clone`; the readers already copy it out or borrow).
* `SettleFrame.tile` (the governed tile, decided when the frame opens: `new`
  takes it as an argument; 5.1). `SettleEntry.tile` changes meaning from the
  frame's position to its tile.
* `FieldCard.position: i32` (the placement slot; `-1` = same as `tile`, §5.1).
* `MatchState.tileAt: Vec<i32>` — per position, the tile standing there (§5.1
  view).

### 4.8 Wire-breaking vs additive

| change | class |
|---|---|
| five new engine kind ids 132–136 | wire (kind table + `from_wire`) |
| new imports above | wire (import table, `sys.rs`, `sys_kani.rs`, `native_shims.rs`) |
| `HostRequest::BuildAs { player_id, tile, charge, amount, numerator, denominator, owner_required }` | host-internal (the new verb's request) |
| `HostRequest::TileWrite { operation, tile, value }` (see §5.4) | host-internal (`set_houses` / `add_house` / `set_owner` become round-trips) |
| `HostRequest::Raise.position` (copied from the enclosing trigger) | host-internal |
| `Trigger.position` / `TriggerMove.path` | host-only, exposed only through new imports |
| `MoveCtx.path` re-keyed from positions to tiles | serialized save field, same type (no `SAVE_VERSION` move, §7) |
| `FieldCard.position` (placement slot) | serialized save field, `serde(default)` `-1` (no `SAVE_VERSION` move, §7) |
| `house_log` tuple gains a signed delta | host-internal scratch (never saved) |
| CEL `tile.position` | host-only, additive to the condition scope |
| `prop::SWAPPED_TILE` / `tag::SUPPRESS_TILE_EFFECTS` | data names, additive |
| `MatchState.tileAt` | view field, `serde(default)`, additive |
| tile writes open `effect` / raise `houseRemoved` | behavioural, old bundles keep their engine (§8) |

---

## 5. Engine wiring

### 5.1 Governing resolution (`crates/game-core/src/engine/ops.rs`, `world.rs`)

**A placed instance needs its slot back.** `FieldCard.tile` is the tile an
instance is *on*; dispatch, `self_tile` and every guard read it. A swap record
is anchored at the slot the instance physically occupies, which diverges from
that tile the moment a card is placed on a swapped slot. `FieldCard` therefore
gains

```rust
/// The board position this instance sits at. `-1` = the same as `tile` (an
/// unswapped placement). Written for a seat's placement only, never for a
/// board-owned rule instance.
#[serde(default, skip_serializing_if = "is_negative")]
pub position: i32,
```

with `fn position(&self) -> i32 { if self.position >= 0 { self.position } else { self.tile } }`.
Old saves have no swapped tile, so `-1` is exactly right for them (§7).

```rust
fn swap_records(&self) -> impl Iterator<Item = &FieldCard> {
    self.st
        .board_field
        .iter()
        .chain(self.st.players.iter().flat_map(|p| p.field.iter()))
}

/// The tile standing at board position `p`, and the instance binding it.
fn swap_binding_at(&self, p: i32) -> Option<(i32, i32)> {
    let partner = |f: &FieldCard| f.props.get(prop::SWAPPED_TILE).copied().filter(|&q| q >= 0);
    // (a) the binding instance sits at `p`: the tile standing there is the
    // tile the instance names as its partner.
    if let Some(f) = self.swap_records().find(|f| f.position() == p && partner(f).is_some()) {
        return Some((partner(f)?, f.uid));
    }
    // (b) the reverse: some instance of tile `p` names a partner, so tile `p`
    // stands where that instance sits.
    self.swap_records()
        .find(|f| f.tile == p && partner(f).is_some())
        .map(|f| (f.position(), f.uid))
}

pub fn tile_at(&self, p: i32) -> i32 {
    self.swap_binding_at(p).map_or(p, |(s, _)| s)
}

/// The position tile `t` currently stands at -- `tile_at`'s inverse, with the
/// same two cases mirrored.
pub fn position_of_tile(&self, t: i32) -> i32 {
    // (a') some instance names `t` as its partner: tile `t` stands at that
    // instance's slot.
    if let Some(f) = self
        .swap_records()
        .find(|f| f.props.get(prop::SWAPPED_TILE).copied() == Some(t))
    {
        return f.position();
    }
    // (b') some instance is on tile `t`: tile `t` stands at that instance's
    // partner. (The tile base instance at the same position carries no prop,
    // so scan past it.)
    self.swap_records()
        .filter(|f| f.tile == t)
        .find_map(|f| f.props.get(prop::SWAPPED_TILE).copied().filter(|&q| q >= 0))
        .unwrap_or(t)
}
```

Both handle the pair as a two-way binding: an instance whose own tile is `c`,
sitting at slot `s` and carrying partner `g`, makes `tile_at(s) = g`,
`tile_at(g) = s` (its slot -- which is where tile `c` stands),
`position_of_tile(g) = s` and `position_of_tile(c) = g`. One level only: a
partner's own record is not followed further (open question 2). The scan is
`O(board + player fields)`; a swap exists only while such an instance is
placed, so the hot walk path pays it only during those cards.

`place_card_on` (for a seat, not the board owner) and `set_self_tile` write the
slot, not just the tile: both resolve `position = position_of_tile(t)`
**before** changing `tile = t` (so the lookup still sees the old record), and
clear `position` back to `-1` when it equals `t`. A board-owned placement keeps
`-1`. `ctx::self_tile()` answers the governing tile
`tile_at(f.position())`, so every guard comparing it with `trigger.tile` keeps
working, and a new `ctx::self_position()` answers `f.position()`.

**Raise sites move to the new arms.** Every site that today feeds a *position*
into `tile =` switches to `position =`; tile-fed sites keep `tile =` and get
`position` derived. The inventory (line numbers are today's):

* turn start: `turnStartBefore` / `turnStart` (`play.rs:758, 792`, the player's
  position);
* move frame: `moveBefore` (`:2002` `m.from`; `:2347` teleport `from`),
  `teleported` / the two `moveResolved` (`:2419, 2427, 2485`, `m.to` / the
  player's position), `moveAfter` (`:2471`, `m.to`);
* walk: `tileEffectBefore` (new) / `passBefore` / `passTile` / `pass`
  (`:2123, 2149, 2159`, `next`), `passPlayer` in `overlap_at` (`:2449`, `at`);
* teleport: `passTile` (`:2382`, `to`) and the new `tileEffectBefore` (5.2);
* settle (`settleBefore` in `after_walk` `:2513`; stages `settle` `:2666`,
  `settleAdd` `:2683`, `settleRentAdd` `:2702`, `settleMul` `:2711`,
  `settleRentMul` `:2716`, `settleBody` `:2739`, `settleAfter` `:2773`,
  `tileResolved` `:2788`): `tile = frame.tile, position = frame.at`. A
  walk-driven frame takes the tile the landing step already resolved (the
  move's last `path` entry, falling back to `tile_at(at)` only for a move-less
  settle), so the landing pass and its settle cannot disagree; `settle_tile_at`
  pins it instead. The frame passes `tile` first and `position` second -- arms
  assign in written order, so the explicit `position` is the one kept -- and
  the frame, `settle_tile` and `tileResolved` agree even if a binding is
  written mid-settle.

Square-fed sites (buy, build, mortgage, redeem, charge, the tile writes) keep
`tile =` and get `position` derived. A raise that already carries a *position*
into `tile =` without conversion is the main mechanical risk of this section;
the grep lint (9.3) checks the remaining call sites. Direct constructions that
bypass the macro set both by hand: `raise_core` (`wasm_rules.rs:7018`) derives
`position` from `tile` after its `set` closure unless the closure wrote one, and
the literals of 4.2 set what they mean.

**Nested guest raises inherit both.** `hostfns::raise_as` (`hostfns.rs:1796`)
already copies the enclosing trigger's `tile` onto the raise (`:1809`); it
copies `position` in the same block. The `raise!` macro's `@m` copies the move
and its tags as today.

**Dispatch: `tile_rule_instances(t.tile)`.** A tile rule and the extras lying
on it belong to the tile; the swap moves that set with the tile
(smile_parade's own note: 「what the tile *is*, not what is on it」 -- the cards
travel with it, they are not re-anchored). The board list (`:6083`), the job
collector (`:6346`) and `settle_tile` keep filtering by `f.tile == t.tile` (board
instances are never placed on a slot other than their tile). The **player-field
loop** (`:5983`) and `tile_extra_count` filter by `tile_at(f.position()) ==
t.tile`: a card standing on a swapped slot hears the tile it displays. One
refinement of that narrowing:

* **Only tile-local kinds narrow** (`tile_local_kind`,
  `wasm_rules.rs:76`): the tile's effect window and the house / owner / build
  writes -- `tileEffectBefore`, `houseAdded`, `houseRemoved`, `houseAdd`,
  `houseRemove`, `ownerSet`, `buildBefore`, `buildAfter`. Every other kind is
  raised with whichever tile it concerns and may be answered from anywhere, so
  narrowing it would drop listeners whose guard reads `trigger.tile` precisely
  *because* their own tile is not it: `passTile` (练习室里的风暴 (1) collects
  from a distance), `paySplitAdd` (`PPP:Tomorrow's Door` (3) taxes a settle
  from the owner's area), `effect` (a counteraction is answered from wherever
  its card lies), and every settle raiser (`settleBody`, `rentAdd`, …).
* **`tileResolved` compares the slot, not the narrow.** Its one question is
  "was this physical slot the one settled", and the raiser stamps
  `tile = tl, slot = at`, so the terminal matches `f.position() ==
  trigger.position()` (falling back to `f.tile`, which a board instance
  answers with its identity tile). This is also why the raiser cannot use the
  `tile =` arm alone (§4.3). Because
`self_tile()` returns that same governing tile and CEL `card.tile` is compiled
to it, every existing guard (`trigger.tile == self_tile`, `card.tile ==
tile.id`) keeps working -- no per-guard edit, and `World::card_tile` (the source of CEL `card.tile`) is
filled from `tile_at(f.position())` instead of `f.tile` (`game-rules/src/condition.rs:1029`). Five
follow-ups:

* **`players_on*`** compares the stander's position: it counts a stander whose
  `tile_at(player_pos)` equals the argument, so existing `players_on(tile.id)`
  call sites need no edit.
* **Physical prop reads** on the mover become governing-tile reads (`circle.rs`
  `settle_reward`'s `player_pos` → `tile_at(player_pos)` for both `tile_prop`
  calls, the `START` flush at `play.rs:2139`).
* **Physical position reads** from a trigger use `trigger::position()` where the
  question is "where did this happen" (5.1's walk/teleport/settle sites, the
  `START` test, and the 6.2 discard).
* **Settle body.** `settle_tile` (`wasm_rules.rs:11422`) takes the frame's move
  and both frame fields -- `(player_id, position, tile, m, is_main_move)` from the
  `SettleTile` stage (`play.rs:2763`, today
  `rules.settle_tile(self, frame.player, frame.at, frame.m.is_main_move)`;
  `frame.tile` rides between `at` and `m`) -- drives `tile_rule_instances(tile)`
  (the tile and its extras, which travel together), skipping each extra 5.2's
  suppression test rejects with `(m, tile, uid)` while the `tile:` base always
  runs, and reads `owner = owners[tile]`. The body trigger carries
  `tile = tile, position = position`, and keeps `mv: None` (its own body is
  driven directly, not walked; nested raises still get the move from 5.6's
  stamping). The built-in fallback becomes
  `land_at_built_in(player, position, tile, main)` (`play.rs:2891`):
  `prep_land` / `landed` take the position, `self.tile(tile)`, `owners[tile]`,
  the `log.land` `tile` field and the `offer_buy` / `offer_build` / `pay_rent`
  calls take the tile. Those and `rent_figure` / `build_cost` /
  `cant_build_on` / `tile_buildable` / `build_price` are **tile-keyed**
  throughout (`owners[t]`, `houses[t]`, `mortgaged[t]`, `tile_prop(t)`, the
  static rent table and the message/`Pay.tile` field), which is the same value
  tile rule bodies already pass (`self_tile`); `buy_quote_for` / `deed_buy` /
  `purchase::table_rent` index identities the same way. Position survives only
  where it is physical: `prep_land` / `landed`, the frame's `at`, the walk,
  `players_on`'s internal `tile_at(player_pos)` (5.1) and the view-only
  `with_tile` / `auction_tile` highlight (`position_of_tile(t)`), while message
  `tile` fields keep the tile.
  `settle_tile_at(player, tile, main)` maps the argument through
  `position_of_tile` before pushing the frame, and the frame **pins**
  `frame.tile = tile` (the named identity) instead of calling `tile_at` -- a
  card-initiated settle names a tile identity, and a swap already bound by
  another instance must not turn that identity into a different tile.
  `SettleEntry.tile` is pushed from `frame.tile`, and `PlayCtx::settle_tile()`
  (cx.rs:1159) therefore answers a tile.
* **View.** `MatchState.tileAt[p]` is filled with `tile_at(p)` in `state_inner`
  / `public_state`; a board slot draws the tile that answers there, and a field
  card renders at `position_of_tile(f.tile)` (so a card sits on the face its
  tile swapped with). `MovePlan.reach` is tile-keyed; the client maps through
  `tileAt`. Pawns, the landed highlight and the discard pile stay positional.
  Without this, a swap would run the right rule under the wrong picture.

**Must keeps**, asserted by tests: `ctx::is_start(t)`, `tile_is(t, kind)` and
`tile_named` keep reading the static board on the **governing tile**
(`tile.id`); external cards keyed *on* CiRCLE therefore still fire on the
CiRCLE tile wherever it sits. The `START` early flush reads the prop of the
governing tile, and every `*_at(tile)` reader is tile-indexed. A swap only
changes which rule instance answers and which tile facts a tile event reads;
`prop::START` is never copied.

### 5.2 The tile window and suppression (item 1)

**Raise points.** The window must precede **every** extra-effect point of the
step, `passBefore` included, so it opens before that raise:

```rust
// walk, after `next` is computed, before passBefore
let tile = self.w.tile_at(next);
raise!(self, "tileEffectBefore", i, @m m, position = next, tile = tile)?;
raise!(self, "passBefore", i, @m m, position = next, tile = tile)?;
// …existing position write / flush…
raise!(self, "passTile", i, @m m, position = next, tile = tile)?;
```

`tile` is resolved once per step and reused for the whole step (the `path` push
included, `m.path.push(tile)` at `:2132`), so a binding written inside the
window cannot make the pass's tile and its settle disagree. The teleport does
the same once at its destination (`m.path = vec![self.w.tile_at(to)]`).

In `step_settle_run` a new `SettleStage::RaiseTileEffect` is inserted before
`RaiseSettle` (the frame now starts there), so the window precedes the `settle`
raise (owner resolution stays in `RaiseSettle`):

```rust
SettleStage::RaiseTileEffect => {
    let (i, at, t) = (frame.player, frame.at, frame.tile);
    let m = frame.m.clone();
    raise!(self, "tileEffectBefore", i, @m m, position = at, tile = t)?;
    self.pull_plan_tags(&mut frame.m);
    frame.stage = SettleStage::RaiseSettle;
}
```

A cancelled `tileEffectBefore` cancels nothing else (it is a window, not a
gate); `set_cancelled` there only drops the rest of that window's own hook
list -- the `passTile` / `settle` raise is a separate trigger and still runs.
Marina does not cancel it; it arms the tag (6.1).

**Suppression predicate** (`wasm_rules.rs`, one helper used by the two hook
walks and the job collector):

```rust
fn extra_suppressed(world: &World, mv: Option<&TriggerMove>, t: &Trigger, uid: i32) -> bool {
    let tagged = |tags: &[(String, i32)]| {
        tags.iter().any(|(k, v)| k == tag::SUPPRESS_TILE_EFFECTS && *v == t.tile)
    };
    let set = mv.is_some_and(|m| tagged(&m.tags)) || tagged(&world.turn.plan.tags);
    if !set {
        return false;
    }
    world
        .field_by_uid(uid)
        .is_some_and(|f| f.tile == t.tile && !f.card.starts_with("tile:"))
}
```

Both tag sources are read. The trigger's `mv.tags` is the raise-time snapshot;
`world.turn.plan.tags` is the live source a counteraction writes, which is what
makes the tag armed *inside* the `tileEffectBefore` hand round visible to the
hook round of that same window (the plan is written by the counter, the trigger
snapshot lagged behind it). The player-field loops call it with the visited
`(uid, _)`; the board list with each `(uid, _)`; all pass `t.tile`.

Board-wide lists (`event:`, `mark:`, `status:`) have `tile < 0` or an unrelated
anchor and stay unsuppressed. `settle_tile` calls the same helper with the
frame's move and tile; its one `tile < 0` / no-instance path
(`land_at_built_in`) is the tile's own base body and always runs.
`tileEffectBefore`'s counteraction round itself runs unfiltered (marina must be
offered), while the window's *hook* walk afterwards reads the tag the counter
just set, so the extras' own `tileEffectBefore` bodies are skipped. A second
pass over the same tile in one move is already suppressed (the tag persists —
intended: "本次"). The tag dies with the move (5.6), so the next move is
normal.

`tile_extra_count(t)` implements the same classification for guards (counts
board instances on `t`, player instances whose slot displays `t`, and tile
marks on `t`; none of them a `tile:` base). It is the residual guard's
「受到其他效果影响」 test -- a [CP点] is the case that is not an instance at
all; the CEL condition keeps only `tile_is(tile.id, "circle")` (6.1), and
`condition::MINE`'s text carries the 「[使用者]」 half.

### 5.3 The one build pipeline (item 3)

Move the existing body of `build()` (`play.rs:3640-3753`) into one engine
routine. The step path raises `buildBefore` before any guard (`:3645`) while
the real gate is `cant_build_on`, checked in `card_build` (`:1798`) -- outside
the moved body. Keep that split in the routine: the gate stays at the callers,
the pipeline starts at `buildBefore`, so a card path that already gated does not
re-prompt.

```rust
pub struct BuildPlan { charge: BuildCharge, amount: i32, numerator: i32, denominator: i32 }

/// `owner_required` adds the owner test to step 1's gate (the step path and the
/// cards that build only on their own tiles pass `true`; a free foreign build
/// passes `false`).
pub fn build_task(&mut self, i: usize, t: usize, plan: BuildPlan, owner_required: bool)
    -> Flow<bool>
```

Order (today's `card_build` gate first, then the routine):

1. out / playing / gate: `owner_required ? cant_build_on : guard_buildable`.
   A refusal is logged (as today, `card_build` logs `reason`) and returns false.
2. `buildBefore` raise (`tile = t`), veto on `is_cancelled`.
3. price: `base = build_cost(t)`; for `Effective` apply `percent.clamp(0,100)`
   then the layered cut (exact current block, including the overflow refund);
   `Free` → 0; `Fixed` → `amount`; `Scaled` → the post-percent/cut figure ×
   numerator / denominator with one `ceil_to(…,10)` on the result. `Fixed`'s
   amount is already rounded by its caller.
4. pop one discount layer (unchanged).
5. if cost > 0: the `PayKind::Build` payment, tile = `t`.
6. the refund re-check: refuse-with-refund when the tile is full, and when
   `owner_required` and the tile is not the payer's (a `Free` card build may put
   a house on an unowned tile, which the free build does today). Then the house
   write through `cx.write_houses` (5.4) so `houseAdd` declares. `houseAdded`
   is raised **exactly once**, right after the write and before `buildAfter`,
   with the tile's owner as `player_id` when it has one (`-1` → the payer); the
   routine does not also log to `house_log`, so the current double raise (the
   body's explicit raise plus the `add_house` commit log) disappears.
7. `buildAfter`.

Two paths reach it:

* the new guest verb `ctx::card_build_as` -- `HostRequest::BuildAs { player_id,
  tile, charge, amount, numerator, denominator, owner_required }`;
* the retained `ctx::card_build` / `HostRequest::Build`, which keeps sending
  the existing `{ player_id, tile }` request. Its handler calls
  `build_task(…, Effective, 0, 0, 0, true)`; no guest-visible shape changes.

`card_build_as` **does not dispatch the tile's `On::Build` body**.
`property.rs:169-207` collapses to a wrapper that is now only the body's entry:

```rust
pub(crate) fn build(player_id: i32) -> card_sdk::Asked {
    let t = ctx::self_tile().unwrap_or(-1);
    if t >= 0 {
        ctx::card_build_as(player_id, t, BuildCharge::Effective, 0, 0, 0, true);
    }
    Ok(())
}
```

Because the verb is not re-dispatched, a ruleset tile with a *custom* build
body still runs it on the step path but not on a card build (open question 10).
`standalone_build` (`purchase.rs:191`, the no-body fallback) becomes the same
`Effective` call; `deed_build` stays the entry
that dispatches a body on the step path.

`cant_build_on` splits into `cant_build_on` (owned) and `guard_buildable` (the
same checks minus the owner comparison; the buyable, ring, `NO_BUILD`,
`NO_BUILD_ABOVE`, lingering, mortgage, tier cap, full and out arms all stay),
both still `Option<Msg>` so the refusal keeps its reason; the guest import
`tile_buildable` is the bool wrapper (`is_none`) that a card reads.
`card_offer_build` (`play.rs:1808`) and `offer_build` (`:3389`) price their
options and affordability with `build_price` instead of raw `build_cost`, and
`offer_build`'s `with_tile(t)` becomes `.with_tile(self.w.position_of_tile(t))`
(a prompt names a slot). `pay_build` stays as an import for archived bundles,
though the current property body no longer calls it; its duplicated
percent/discount block is replaced by `build_price` so the two cannot drift.

### 5.4 Tile writes declare an effect (item 4)

Two engine methods own every house/owner write a card body can make; the buy
path keeps its own primitive (`purchase::assign_deed`, below). They compute the
*effective* change first (clamping to the tile's cap), and do nothing at all --
no window, no write -- when the value is unchanged (`ownerSet` with the same
owner included):

```rust
// crates/game-core/src/engine/cx.rs
pub enum HouseWrite { Set(i32), Add(i32) } // signed delta allowed on Add

impl PlayCtx<'_> {
    /// Declare `houseAdd` / `houseRemove` for `tile` on a fresh `effect`
    /// window, then apply. `Flow<i32>` = the post count; a cancellation
    /// answers the condition count.
    pub fn write_houses(&mut self, by: usize, t: usize, w: HouseWrite) -> Flow<i32>;

    /// Declare `ownerSet` for `tile`, then apply. `Flow<i32>` = the new owner;
    /// a cancellation answers the old one.
    pub fn write_owner(&mut self, by: usize, t: usize, owner: i32) -> Flow<i32>;
}
```

Each builds its trigger through the ordinary `raise!` path:

```rust
let cap = self.w.data.tiles[t].max_houses; // the clamp `add_house` inlines today
let before = self.w.st.houses[t];
let after = match w {
    HouseWrite::Set(n) => clamp(n, 0, cap),
    HouseWrite::Add(d) => clamp(before.saturating_add(d), 0, cap),
};
if after == before { return Ok(before); }
let delta = after - before;
// `value` is the size of the change, unsigned (135 is the removal spelling).
let kind = if delta > 0 { "houseAdd" } else { "houseRemove" };
let target = self.w.st.owners[t]; // the owner *before* the write, -1 if none
let trigger = raise!(self, "effect", by, tile = t as i32, target = target, effects = vec![
    Effect { kind: kind.into(), target, from: -1, tile: t as i32, value: delta.abs() },
])?;
if trigger.is_cancelled() { return Ok(before); }
self.w.st.houses[t] = after;
Ok(after)
```

`by` is the running call's player (`call.player_id()` in the bridge), the same
`by_card` every hosted effect uses; `ownerSet` uses the analogous shape with
`value = owner`.

**Guest path.** `hostfns::set_houses` / `add_house` / `set_owner`
(`hostfns.rs:986-1007`) cannot reach `PlayCtx`, so each becomes a
`HostRequest::TileWrite { operation, tile, value }` request, `operation` being a
`#[repr(i32)]` enum `TileWriteOperation { SetHouses = 0, AddHouses = 1,
SetOwner = 2 }` on the game-rules side answered
inline by a handler beside `Build` (`wasm_rules.rs:4770`) that calls the method
above. `visible_host_request` (`:105-136`) classifies `TileWrite` as
`Some("house")`. The handler also does the run bookkeeping a guest add always
did: `flush_header()` and, for a non-zero delta on an owned tile,
`house_log.push((owner, tile, now, delta))` (5.5). The request answers the post
value, or the condition value when cancelled. The return shapes are unchanged:
`add_house` still answers the count, `set_houses` / `set_owner` answer nothing.
The pause is required for replay correctness: `add_house` is not idempotent,
and a request answer is skipped on the body's replay pass; the loop re-seeds the
run world from the live one (`wasm_rules.rs:3719`), so a handler's live write is
what the re-run sees.

**Not affected:** `tile_rule_instances`; `set_tile_prop` stays outside (a view
write; open question 4). `AssignDeed` (handler `wasm_rules.rs:5075`) keeps its
direct `purchase::assign_deed` write and only contributes the house fall (5.5);
it is an engine commit after the buy pipeline, not a guest effect.

### 5.5 `houseRemoved` (item 6)

`house_log` becomes `(owner, tile, now, delta)`; every guest house write reaches
it through the `TileWrite` handler, which logs a delta of either sign (still
only when `owner >= 0` at the time of the write). The `AssignDeed` handler
pushes a fall for the **previous** owner when `houses` lowers.
`commit_after` (`:5717`) raises `houseAdded` for `delta > 0` and `houseRemoved`
for `delta < 0`, with `value = now` and the logged `owner` as `player_id` --
the owner at write time, which for a raze on assignment is the seller. No
entry, hence no raise, when the clamp leaves the count equal (for example
`add_house(-1)` at 0).

`ringing_bloom` adds `On::Action(Action::HouseRemoved(...))` beside
`HouseAdded`/`Bought`, and a `Discarded` entry that clears the tile props the
card armed.

### 5.6 Move tags and path (items 1, 5)

* `MoveCtx.path` (`move_ctx.rs:168`, already saved) is re-keyed from positions
  to tiles: the walk pushes the step's resolved tile (5.2), the teleport sets
  `[tile_at(to)]`, and `card_settle_move` sets `vec![tile]` instead of inheriting
  the outer plan's path. `to_plan`'s `reach` therefore broadcasts tiles
  (`state.rs:23`'s doc already says "tile"); `MovePlan.from` stays a position.
* `fold_plan_delta` (`play.rs:173-175`) copies tags **unconditionally**
  (`m.tags = plan.tags.clone()`, dropping the `if plan.tags != before.tags`
  guard), fixing the pre-armed-tag gap.
* `pull_plan_tags(&mut m)` (same copy) runs after every tile-carrying raise and
  after every `tileEffectBefore` / settle stage.
* At `moveResolved` and at teleport completion (and on either cancellation
  path) `m.tags.clear()` and `turn.plan.tags.clear()` -- the tag is one move.
* The frame's `Move` is stored on its `SettleEntry` at entry creation
  (`play.rs:2606`, `move_context: Some(frame.m.clone())`), so every stage of the
  settle -- `tileEffectBefore` included -- can hand it down; the entry is
  popped with the frame at `:2637`. Two choke points stamp
  `t.mv = cx.settle_stack.last().and_then(|e| e.move_context.clone())` when the
  trigger has no move and the kind is one of `effect` / `pay` / `payAfter` /
  `paid`: `PlayCtx::raise` (`:5971`) for engine raises and `WasmRules::counteract`
  (`wasm_rules.rs:12029`) for the bridge's `raise_core`. `effect` is included so
  a tile write or a `target` declaration from inside a settle also carries the
  move and its path. This gives the rent `paid` trigger (`play.rs:4707`) the
  settle's move, its tags and its full path -- no per-payment `@m` at every
  rule body.
* The `TileWrite` effects from 5.4 get their `position` from the `tile =` arm;
  nothing else to carry.

### 5.7 Kani model

`crates/card-sdk/src/kani_host.rs` gains `Op` rows 452–463 (the enum ends at
`nearest_with_prop` 442), one per new import, and `sys_kani.rs` /
`native_shims.rs` / the host trait register each beside its neighbours
(`set_tag` at 1066, `card_build` at 1166, `trig_move_tag` at 786).

Stubs in `rules/kani/src/host.rs`: `build_cost_percent` = 100,
`build_discount` = 0, `build_price` = the tile's base cost (the stub's own
`build_cost` table), `tile_buildable` = 1, `tile_extra_count` = 0,
`card_build_as` = 1, `trig_position` = the trigger tile, `position_of_tile` =
the argument, `tile_at` = the argument, `self_position` = `self_tile`,
`trig_move_path_length` = 0, `trig_move_path` = 0, `tile_of` = the world's own
`tile_at`. The swap needs no op: `set_prop(prop::SWAPPED_TILE, …)` is the
existing op.

The pure model lives in `rules/kani/src/spec/abi_g09.rs`:
`price_of(base, percent, cut)`, `suppressed(tag_value, trigger_tile,
trigger_position, instance_tile, is_base)`, `fall_raised(before, after)`,
`pipeline_commits(gate, before_stands, payable, takes_house)`, the one-record
`Swap` / `tile_at` / `position_of_tile` / `start_never_travels` pair, and
`travel(start, steps)` / `start + i`. Each has a mutant beside it that
disagrees where the hazard is. Proofs are §9.4.

---

## 6. Card usage

### 6.1 `marina_work` (item 1)

**Before** (`marina_work.rs:20-64`): counteract `PassRewardAffected`, no guard,
body logs.

**After**:

```rust
On::Counteract(
    &[ChainKind::TileEffectBefore],
    "actor == owner && tile_is(tile.id, \"circle\")",
    Some(can_counteract),
    counteract,
)

fn can_counteract(_owner: i32) -> bool {
    ctx::tile_extra_count(trigger::tile()) > 0
}
```

Body:

```rust
let t = trigger::tile();
ctx::plan::set_tag(tag::SUPPRESS_TILE_EFFECTS, t);
ctx::log(player_id, &Msg::new(key!("marina_work_normal"))
    .player_id("who", player_id).tile("tile", t));
```

The log's tile is now real rather than a static "#1". The condition spells
`condition::MINE`'s text (`actor == owner`; `condition` is a `&'static str`, so
the half cannot be a constant reference) and adds `tile_is(tile.id, "circle")` --
the book's "#1格子" (`ctx::is_start` is that same test). `tile.id` is the
governing tile, so a swapped start tile still passes, and `circle_rebuild`'s
re-home cannot lose it (`prop::START` never leaves the tile; the re-home only
re-points the effects).

### 6.2 `smile_parade` (item 2)

**Before** (`:51, 188-245`): one `SettleBody` entry hand-redirects (1) and (3)
through `settle_tile_at`, keyed `TAG_REDIRECT` / `TAG_AS_AGENT`; no pass half.

**After.** Two mechanisms share the card, each where it belongs:

* (3), the continuous swap, becomes **one prop write**. `counteract` calls
  `ctx::set_swapped_tile(group_tile())` right after its `place_card_on` (the
  body runs with the new instance as `self`, so the prop lands on it). The
  binding is instance-owned and keyed to the instance's slot, so (2)'s
  `set_self_tile` (which keeps props and re-slots) carries it to the endpoint
  without a re-arm, and the discard drops it with the instance. `SettleBody` loses the
  (3) branch entirely; both the pass and the settle dispatch resolve to the
  partner already, so the entry shrinks to the (1) path: condition
  `tile.id >= 0`, `settle_body_guard` =
  `!trigger::cancelled() && move_tag(TAG_REDIRECT) == 0 &&
  move_tag(TAG_AS_AGENT) != 0`.
* (1), 「当次移动的移动终点视为"弦卷集团"地产商」, is not a swap of two tiles:
  the card sits on the group while the *endpoint* must settle as the agent.
  Keep the `TAG_AS_AGENT` branch and its `settle_tile_at(player, group,
  is_main_move = true)`; the as-agent half rides the tag, which the settle
  reads. `settle_tile_at` pins the named tile, so a swap bound by an older copy
  cannot turn the nested settle back into that copy's tile. `TAG_REDIRECT`
  survives only as the card-local re-entry/latch marker (`resolve_guard`,
  `on_resolved`); `TAG_AS_AGENT` stays.
* (3)'s discard tests the settle's **position against the slot the card was
  placed on**. The placement slot is `self_position()` (the physical slot the
  card is drawn at), so `at` becomes `trigger::position()` and the comparison is
  `trigger::position() == ctx::self_position()`. A settle
  on the endpoint slot the card was placed on discards (after its agent body),
  while a settle at 弦卷集团's own home slot, which now runs the card's tile,
  does not -- matching the pre-swap behaviour (open question 11). The placing
  settle itself stays exempt through the existing `PROP_ARRIVED` latch, and the
  test agrees with the draw position because the card is drawn at that same
  slot (§5.1).
* (2)'s 「[移动终点]」 placement takes the endpoint's **tile**:
  `set_self_tile(trigger::tile())` (the tile standing at the endpoint), and the
  message names it with `trigger::tile()`. The instance's binding is anchored
  at its slot, so re-slotting to the endpoint makes it `(endpoint, group)`: the endpoint position now resolves to the group (which is
  (1)'s point and lets the (1) settle run as agent), and the card, placed on the
  endpoint tile, stays drawn at that slot (the view draws the group's face
  there, so the card sits on the group). Its 「卡仍位于弦卷集团」 condition is the **physical square**,
  `self_position() == tile_named('弦卷集团')`, plus
  `trigger::position() != ctx::self_position()`: after (2) the endpoint slot
  displays the group, so a display-tile read (`card.tile`, equal to
  `self_tile()`) would still say "on the group" and offer (2) a second time on
  the very next settle. The qualifier is what keeps (2)'s
  「移除其上全部晶体」 from discarding a card that has left the group's square. A card-initiated
  settle is still excluded by `settle_after_guard`.

The endpoint tile *is* `trigger::tile()`: the mover's endpoint position resolves
to the tile standing there. `settle_body`'s endpoint checks are gone with the
(3) branch; the surviving (1) branch keeps `at = trigger::tile()`.

### 6.3 `smile_patrol` (item 3)

**Before** (`:23-84`): raw `add_house`, `is_buyable/!mortgaged` only, raw price
in `cant_play` / `card_offer_build`.

**After**:

```rust
// cant_play: affordability at the effective price
if ctx::can_build_on(player_id, t) && money >= ctx::build_price(t) { … }
// the roll face is a position; every build read is tile-keyed
let position = (r - 1).rem_euclid(ctx::tile_count());
let t = ctx::tile_at(position);
// the verb carries the buildability gate (`owner_required` false, so someone
// else's / an unowned tile is allowed)
let before = ctx::houses_of(t);
if ctx::card_build_as(player_id, t, BuildCharge::Free, 0, 0, 0, false) {
    ctx::log(…, ctx::houses_of(t));
} else {
    ctx::log(…, before);
}
```

The paid half's `owned_tiles` already yields tiles (`ops.rs:434` iterates the
owner array), so `card_offer_build` needs no change beyond its price.

`card_offer_build` does the first half: its option filter and the option's
`cost` label both switch from `build_cost` to `build_price` (still through
`deed_build`, so a tile with a custom body keeps it on this path).

### 6.4 `miracle` (item 3)

**Before** (`:79-92`): always `set_build_discount(build_cost, 1)`, always spend
a crystal.

**After** `before_build`:

```rust
let t = trigger::tile();
if t < 0 { ctx::set_slot(player_id, "miracle_used", 0); return Ok(()); }
let price = ctx::build_price(t);
if price <= 0 || ctx::crystals() < 1 {
    ctx::set_slot(player_id, "miracle_used", 0);
    return Ok(());
}
ctx::set_slot(player_id, "miracle_used", 1);
// The cut is absolute and subtracted *after* the percent, so it must cover
// the whole post-percent figure, not just what is still due: `price + the cut
// already armed`. Setting only `price` would leave an earlier card's cut
// standing.
ctx::set_build_discount(price + ctx::build_discount(), 1);
```

`after_build` spends one crystal only when the slot is 1, then clears the slot
(so a second stacked copy's after-hook at the same build does not spend a
second one). The entry guard still requires a crystal and `card.placed`;
`trigger::tile()` is the governing tile the build is on.

### 6.5 `to_the_peak` (item 3)

**Before** (`:97-143`): `pay` + raw `add_house`.

**After**:

```rust
let amount = ceil_to(ctx::build_price(t) as i64 * 3 / 2, 10);
if ctx::money_of(player_id) < amount { return Ok(()); }
if ctx::card_build_as(player_id, t, BuildCharge::Fixed, amount, 0, 0, true) {
    ctx::log(…);
}
```

The verb pays through `PayKind::Build`, runs `buildBefore` / `houseAdded` /
`buildAfter`, and the card no longer calls `pay` + `add_house`.

### 6.6 `declare_war` (item 4)

Guard unchanged — the effect list now contains `houseAdd` / `houseRemove` /
`ownerSet` with the tile, so `(0..effect::count()).any(|i| effect::tile(i) …)`
reaches it. New tests only; a tile write that is cancelled in its `effect`
window never lands.

### 6.7 `guerrilla` (item 5)

`can_counteract` gains, after the existing kind checks:

```rust
let n = trigger::move_path_length();
let mut passed = false;
for i in 0..n {
    let t = trigger::move_path(i);
    let owner = ctx::tile_owner(t);
    if owner >= 0 && owner != player_id && ctx::is_live_house_for(owner, t) {
        passed = true;
        break;
    }
}
if !passed { return false; }
```

The `Paid` trigger now carries the settle's move, so an unrelated rent between
settles has length 0 and stays closed. The TODO at `:96-98` goes away; the
free-pool test stays.

### 6.8 `ringing_bloom` (item 6)

Add:

```rust
On::Action(Action::HouseRemoved("card.placed && actor == owner", None, recompute_view)),
// the raise names the discarded card; `card.placed` is already false here
On::Action(Action::Discarded("trigger_card == card.id", None, on_discarded)).silent(),
```

`on_house_added` is renamed `recompute_view` and its existing `HouseAdded` /
`Bought` entries point at the new name; the two new entries make the view
symmetric. `on_discarded` clears `RENT_HOUSES` and every `RENT_FACTOR` the card
armed and recomputes nothing (the card is gone) -- it also closes the existing
TODO that a removal path other than `pay_after` leaves the tile props armed.
`apply_view` itself is unchanged — a fall now re-runs it.

---

## 7. Save / record / replay

* **SAVE_VERSION stays 16.** The swap rides `FieldCard.props["swappedTile"]`
  (already serialized) anchored on the new `FieldCard.position` (serialized
  with a `-1` default and skipped when unswapped, so an old save's rows are
  byte-identical and mean "slot = tile", which is exactly right for them); the
  tag rides
  `TurnCtx.plan.tags` (already serialized, v7), and `Trigger` / `TriggerMove`
  are never serialized (`Saved.pending` holds `Routine` state and `Live`, not
  triggers). `MoveCtx.path` keeps its type (`Vec<i32>`), only its meaning
  changes; a save taken mid-move before v114 and restored after it carries
  positions in that one path, which the next move overwrites -- only a `paid`
  window during the restored move can see it. `house_log` is per-run scratch.
  `SettleEntry` and `SettleFrame` are transient, as is `MovePlan.reach`. A save
  taken with a swap armed restores the prop and therefore the swap; a save
  mid-move restores the tag. A body paused at a `TileWrite` is covered by the
  existing `Saved.pending` routine: the request is re-derived when the body
  re-runs, so the pause adds no saved field. `HostRequest` is host-internal (not
  serialized; `#[derive(Debug, Clone, PartialEq)]` only), so variants and
  fields added to it move no version.
* **RECORD_VERSION stays 4.** The input stream is unchanged; the record's
  `EngineStamp.save_version` does not move.
* **Replay.** Old `.bdrec` files replay on their archived engine bundle, so
  the behavioural changes to `add_house` / `set_houses` / `set_owner` /
  `card_build` do not alter old replays. The retained two-argument build import
  (§8) keeps an old bundle's card table loadable, though it is the bundle's
  engine that answers it. `MatchState.tileAt` is a view field;
  `public_state` fills it, and an archived engine simply does not.

---

## 8. Versioning

`ABI_VERSION` 112 → **113** (tenth group in landing order), entry inserted
immediately below the constant (newest first):

```
/// v113: **tile events, one build pipeline, tile-write effects, house falls,
///    move paths.** New engine kinds `tileEffectBefore` (132),
///    `houseRemoved` (133) and the effect-only spellings `houseAdd` (134) /
///    `houseRemove` (135) / `ownerSet` (136). `Trigger` gains `position` (the
///    board position; `tile` is the tile, and a tile's rule instances, extras,
///    owner, houses, mortgage and props all travel with it) and `TriggerMove`
///    gains `path`; `prop::SWAPPED_TILE` (SDK sugar `ctx::set_swapped_tile`)
///    re-points a position at a tile, one level, and `ctx::tile_at` /
///    `ctx::position_of_tile` are the only conversions — identity keys stay on
///    the tile, so `ctx::self_tile` and `card.tile` already read it. New
///    imports `ctx::trigger::{position, move_path_length, move_path}`,
///    `ctx::tile_extra_count`, `ctx::position_of_tile`, `ctx::tile_at`,
///    `ctx::self_position`, `ctx::tile_buildable`, `ctx::build_price`,
///    `ctx::build_cost_percent`,
///    `ctx::build_discount`, `ctx::card_build_as`; new move tag
///    `tag::SUPPRESS_TILE_EFFECTS`; new CEL field `tile.position`. Host
///    requests: `BuildAs` (the build verb), `TileWrite` (the guest tile
///    writes) and a `position` on `Raise`; `ruleset_card_build` (two
///    arguments) stays as the `BuildCharge::Effective` alias. `set_houses` /
///    `add_house` / `set_owner` now open an `effect` window with a declared
///    tile effect before applying and may be cancelled there; `add_house` /
///    `set_houses` raise `houseRemoved` on a fall, and `assign_deed`
///    contributes a fall for the previous owner. SAVE_VERSION 16 and
///    RECORD_VERSION 4 unchanged.
```

**README band shift.** G02's landed arithmetic kinds consumed 126–131, which
were the tail of this group's allocated row (126–130) and G10's (131). Kind ids
are wire-stable, so the allocation moves rather than the landed ids: G09 takes
**132–136**, G10 **137**, G11 **138–139**; G12 keeps the still-free 107 and G14
the empty row. The Op row moves to **452–463** (twelve imports). The README
table and the group rows are edited with this document.

The old two-argument import is a real alias, not sugar in the guest: an archived
bundle calls the symbol directly. `card_build`'s handler keeps answering the
same `HostRequest::Build` shape, so old bundles are unchanged.

---

## 9. Tests and Kani obligations

### 9.1 Per item (`rules/tests/tests/`)

| item | test | assertion |
|---|---|---|
| 1 | `g09_marina_offered_on_a_circle_with_an_extra` | a card placed on CiRCLE + a pass over it offers marina |
| 1 | `g09_marina_not_offered_on_a_plain_circle` | `tile_extra_count == 0` → no offer |
| 1 | `g09_marina_window_opens_when_the_reward_does_not` | pass from CiRCLE to CiRCLE / `NO_REWARD` set → still offered; its settle's extras suppressed |
| 1 | `g09_marina_suppresses_pass_and_settle_extras` | kkr on CiRCLE: with marina, no 6000 on the pass's settle; the base draw/reward still runs; a second move charges again |
| 1 | `g09_marina_does_not_suppress_the_base_rule` | CiRCLE's own draw and reward still happen |
| 2 | `g09_parade_pass_runs_the_swapped_rule` | card on CiRCLE → passing 弦卷集团 pays the CiRCLE reward, passing the card's slot does not |
| 2 | `g09_parade_settle_runs_the_swapped_rule` | card on a rent tile → settling that slot runs the agent body; settling 弦卷集团 charges the rent tile's rent to its owner |
| 2 | `g09_parade_discard_only_on_its_own_position` | a settle on the endpoint slot the card was placed on discards (after the agent body); a settle at 弦卷集团's own home slot, which runs the card's tile, does not |
| 2 | `g09_parade_identity_stays` | an effect keyed `tile.id == tile_named('CiRCLE')` fires where the CiRCLE identity now stands, and the start reward follows the same resolution (the `START` prop is never copied) |
| 2 | `g09_parade_swap_ends_on_discard` | after (3)'s discard both slots run their own rules again |
| 2 | `g09_parade_view_draws_the_tile_at_each_slot` | while bound, each slot's tile is `tileAt[p]` and the card draws at its `position` |
| 3 | `g09_card_build_gate_matches_the_build_step` | matrix of ring / full / mortgaged / `NO_BUILD` / `NO_BUILD_ABOVE` — refused identically by `card_build_as` |
| 3 | `g09_patrol_free_build_respects_tile_vetoes` | 3d20 onto a `NO_BUILD` tile raises nothing |
| 3 | `g09_patrol_affordability_uses_the_effective_price` | `percent = 50` → playable with half the base cost and refused with just under |
| 3 | `g09_miracle_free_build_spends_one_crystal_not_money` | price > 0 + one crystal → 0 paid, crystal 1 → 0 |
| 3 | `g09_miracle_already_free_build_keeps_the_crystal` | another discount makes the price 0 → crystal kept |
| 3 | `g09_to_the_peak_charges_one_and_a_half_and_runs_hooks` | pays ceil(1.5×effective, 10), `buildBefore` can veto, `houseAdded` and `buildAfter` fire |
| 3 | `g09_card_build_house_added_precedes_buildAfter` | hook order pinned |
| 4 | `g09_declare_war_answers_an_owned_tile_house_add` | other player's `add_house` on your tile → offered |
| 4 | `g09_declare_war_answers_an_owned_tile_house_removal` | `add_house(-1)` → offered |
| 4 | `g09_declare_war_not_offered_on_an_unowned_tile` | owner -1 → not offered |
| 4 | `g09_tile_write_cancel_skips_the_write` | a counteraction cancelling the `effect` leaves the count/owner unchanged |
| 5 | `g09_guerrilla_not_offered_on_a_rent_without_a_passed_lh` | plain rent → closed |
| 5 | `g09_guerrilla_offered_after_passing_another_players_lh` | path contains the rival's Live House → offered |
| 5 | `g09_guerrilla_not_offered_after_passing_own_lh` | own Live House only → closed |
| 5 | `g09_guerrilla_path_is_the_settling_move_only` | a rent between two settles has `move_path_length == 0` |
| 6 | `g09_house_removed_on_add_house_fall` | `add_house(t, -1)` raises once, `value` = post count |
| 6 | `g09_house_removed_on_set_houses_fall` | `set_houses(t, n)` with `n < current` raises |
| 6 | `g09_house_removed_not_raised_on_equal_clamp` | `add_house(-1)` at 0 raises nothing |
| 6 | `g09_ringing_bloom_recomputes_on_raze` | razing the max tile lowers `RENT_HOUSES` and clears stale `RENT_FACTOR`s |
| 6 | `g09_ringing_bloom_half_ends_when_the_card_leaves` | discard clears the props |

Existing suites that must stay green: the `smile_parade` settle suite,
`circle_rebuild_*`, `property` / `deed` build tests, `declare_war_*`,
`guerrilla_*`, `ringing_bloom_*`, and every `*_kani` `ppcost` export.

### 9.2 Suppression matrix

`rules/tests/tests/chain_eligibility.rs` gains `g09_suppression_matrix` over
(base / extra-board / extra-player / event / mark / status) × (tag set / unset)
× (base rule swap bound / not), asserting the dispatch set exactly. In the
swap rows the event position's own tile `x` is bound to a partner `g`: the base
rule and its extras all carry `f.tile == g`, while the only instance carrying
`x` is the binding card itself. The correct dispatch (`t.tile`) therefore
returns exactly the partner's set, and a mutant dispatching by `t.position`
returns just the binding card. A tag row values `SUPPRESS_TILE_EFFECTS` with
the position instead of the tile: nothing is suppressed, so the tile-valued tag
is pinned too.

### 9.3 Static sweep

* A grep lint lists every site where a **position** reaches an identity reader,
  or a **tile** reaches a physical one. Identity readers take a tile: `tile_is`
  / `tile_named`, `is_start`, `tile_owner`, `houses_of` / `add_house` /
  `set_houses`, `build_cost` / `tile_prop` / `set_tile_prop`, `set_self_tile`,
  `place_card_on`, the `tile` of a `HostRequest`. Physical readers take a
  position: `ctx::dist`, `teleport_to` / `plan::set_teleport_to`,
  `tile_steps_ahead`, `move_from`, the discard slot, `trigger::position()`.
  Position sources are `ctx::player_pos(p)`, `trigger::move_from()`, a roll face
  and the teleport cursor; tile sources are `tile_named`, `owned_tiles`,
  `self_tile`, `trigger::tile`, `ctx::ask_tile` over a tile list. The repair is
  `tile_at(position)` into an identity reader, `position_of_tile(tile)` (or
  `trigger::position()`) on the physical side, and `ctx::ask_tile` keeps tile
  lists as tiles. Known hits: `rules/tiles/src/circle.rs:77-92` `pass_pays`
  (`is_start`) and `settle_reward` (`tile_prop(player_pos)`),
  `skills/skill-groups/src/ras.rs:57,85` (`ON_HOUSE` is a tile tested against
  `player_pos`) and `:105` (`set_teleport_to(owned_livehouses…)`),
  `skill-personas/src/tomoe.rs:68,81-85` (`shop()` is a tile tested against
  `player_pos` and fed to `dist`), `skill-personas/src/tomori.rs:117,122,151`
  and `sakiko_crychic.rs:58` (`dist(trigger.tile(), position)`), saki's `nudge`
  (`:119-134`, `landing ± 1` positions but `landing` is `trigger.tile`),
  `card-ras/src/studio_storm.rs` `on_own_lh` (`tile_owner(pos)`,
  `is_live_house_for(pos)`) and `settle_body` (`dist(trigger.tile(),
  self_tile)`), `card-mygo/src/haneoka.rs:35-48,64-70` (`pos = player_pos`
  handed to `tile_owner` / `houses_of` / `can_build_on` / `add_house`),
  `card-ag/src/ran_as_usual.rs:101` (`turn_snapshot().pos` in a message `tile`),
  `card-mygo/src/endless_journey.rs:44` and `card-ras/src/studio_storm.rs:72`
  (`place_card_on(player_pos)`), and `smile_parade`'s (3) discard
  (`trigger::position() == ctx::self_position()`, 6.2). Already
  tile-to-tile and unchanged: `card.tile`, `self_tile`, name lookups, `ask_tile`
  over `owned_tiles`, and the CEL `card.tile == tile.id` condition
  (`card-mygo/src/rana_funny.rs:24`, `card-ppp/src/kasumi_love_all.rs:28`).
  Pure position pairs (`clear_cp`'s free-tile scan, `surprise_duel`'s
  `tile_forward`, `tomori_inner_shout`'s `dist`) stay positions.
* Every card body that calls `add_house(±1)` for a build path is checked
  against `card_build_as` (the raw write stays legal for a pure move, e.g.
  `parking_space` moving a house between tiles). `smile_patrol` (6.3) and
  `haneoka` (its d20's ≥10 branch) are the two free builds that must move;
  `hey_kids` / `sunset` / `pareo_far` stay raw (moves, not builds).
* Every new import has a `sys.rs` entry with its `link_name`, a `sys_kani.rs`
  sibling, a `native_shims.rs` symbol and a `kani_host::Op` row;
  `crates/rules-native/tests/abi_link_names.rs` fails if any is missing, and
  the `Op` row count must stay in step with `sys_kani.rs`.

### 9.4 Kani obligations

`rules/kani/src/spec/abi_g09.rs` models the pure functions; the harnesses sit
in `rules/kani/src/proofs/abi_g09.rs`:

* `price_iff` — `price_of` equals `max(0, base * percent / 100 - cut)` for
  arbitrary inputs; mutants clamping before the cut and dropping the 0 floor
  fail.
* `pipeline_commits_iff` — a commit happens iff gate passes, `buildBefore`
  stands, the charge is payable and the tile takes a house; a mutant skipping
  the gate or the veto fails.
* `suppressed_iff` — `suppressed(tag, trigger_tile, trigger_position,
  instance_tile, is_base)` is true exactly when the tag and the instance's tile
  both equal `trigger_tile` and the instance is not a `tile:` base; a mutant
  comparing either against `trigger_position`, or one that also hides the base,
  fails.
* `fall_iff` — a `houseRemoved` is raised exactly when `after < before`, once;
  a mutant on `<=` fails.
* `swap_involution` — with one record `(a,b)`, `tile_at(a) == b`,
  `tile_at(b) == a`, `tile_at(x) == x` otherwise; with no record identity.
  The same model proves `position_of_tile` is the two-way inverse
  (`position_of_tile(b) == a`, `position_of_tile(a) == b`, and
  `position_of_tile(x) == x` for an unrelated `x`), so
  `tile_at(position_of_tile(t)) == t` and
  `position_of_tile(tile_at(p)) == p` for every `t`/`p` in the pair.
* `path_is_every_step` — `path.len == steps` and each push equals the traversed
  tile; a mutant keeping only the endpoint fails.
* `start_never_travels` — for any swap record, `is_start(tile_at(p))` is
  unchanged for every `p` where the start prop is read.

Card bodies stay PROTOCOL-LEVEL; the proofs sit on the pure SDK-side model.
All seven run green (`tools/kani/run.sh --unit abi_g09`). Pillar 6's
`houseRemoved` proofs sit beside the existing `ringing_bloom` suite; their
hooks added `recompute_view_kani` / `on_discarded_kani` wrappers (9.1).

---

## 10. Alternatives

**A1 — keep `PassRewardAffected`, add a tag read at the extras only.**
Smallest change for item 1: raise nothing new; marina stays on the reward
window, body sets a tag, the extra points read it. Rejected because the window
is behind the reward choice: a pass with no reward cannot be answered (the
item's own defect) and a settle has no window at all.

**A2 — run another tile's PassTile body from a card, no swap layer.**
`tile_rule_instances` stays single-tile; `smile_parade` counteracts `PassTile`
on its slot and calls a new `ctx::tile_pass_at(owner, group)` that re-raises the
other rule's body, with the same `TAG_REDIRECT` one-shot. Much smaller than
§5.1, but the settle half must keep its hand-rolled redirect (two mechanisms),
extras on either tile still do not travel, and `circle_rebuild` gets nothing.
Rejected in favour of the swap, which is ABI-13's promised capability and
subsumes both halves.

**A3 — keep `card_build`, add hook raises to `add_house`.**
`add_house` would raise `buildBefore`/`buildAfter` around itself. Rejected:
`add_house` is also used for pure house moves (`parking_space`, `hey_kids`,
`sunset`) where a build hook is wrong, and price/gate would still be absent.

**A4 — declare tile effects on the enclosing link, no new `effect` window.**
`set_houses` appends to `trigger.effects` and declare_war reads it. Rejected:
the enclosing link's `effect` dispatch has already returned by the time a card
body writes, so the guard runs before the effect exists; only a fresh window
is timely.

**A5 — path via a per-move player slot written by a placed card.**
A helper field card on every tile records passes. Rejected: guerrilla is a hand
card and would depend on another card being in play; the engine already knows
the path.

**A6 — one signed `houseChanged` kind instead of `houseAdded` /
`houseRemoved`.** Fewer ids. Rejected: `houseAdded` is the established, named
rise point (id 75, `abi.rs:3134`) used by existing cards; making its sign
meaningful would change every existing listener and its docs.

**A7 — full physical position/owner swap for item 2.** Swap ownership, houses
and mortgage arrays as well as the rule. Truest to 「交换位置」 but touches the
save layout and every array reader. Deferred to open question 1.

**A8 — pin extras to the physical slot.** Dispatch the base by `t.tile` but the
extras by `t.position`, so a card lying at a position is not carried by the
swap. Rejected: the pre-swap card redirects the settle to the partner, so the
partner's extras fire; pinned extras would run the slot's set instead, and a
placed `tile:` card would need a third rule.

**A9 — rename the existing `tile` field to `position` and add `tile`.** The
propagated kernel would match the new vocabulary exactly, but every one of the
~200 existing `tile =` raise sites and the guest `trigger.tile` spelling would
change at once, including archived bundles. Rejected for one version; `tile`
already means identity and only the position-fed sites move.

---

## 11. Open questions

1. **Does the swap move ownership/houses/mortgage, or only the rule and its
   facts?** Design: the settled *tile* keeps its owner, houses and mortgage
   (`owners[trigger.tile]`, the tile-keyed arrays); they are never copied to
   the position. The live consumers (agent, event cafe) need no more. If a
   general pair swap with physical arrays is required, the safe path is A7 and
   a separate audit.
2. **Two overlapping swaps.** One level only; the first record in board order
   wins. Is a stack (`a→b→c`) wanted, and does the second card displace the
   first? The live case is two `smile_parade` copies: the second copy's (1)
   placement on 弦卷集团 is shadowed by the first copy's reverse binding, so a
   settle at the 弦卷集团 position resolves to the first copy's tile. Is
   "first placed wins" the wanted ruling, or should a copy already on the
   group tile outrank it?
3. **`tileEffectBefore` on every tile step.** It opens a hand round on every
   walk step even with no extras. Is a payload flag (`extra_count`) or a
   raise-only-when-nonzero rule wanted? Design raises unconditionally so a
   card's own "when affected" guard is its only filter.
4. **`set_tile_prop` inside `effect`.** View writes (`ringing_bloom`'s
   `RENT_FACTOR` reconciliation) would flood the window if every prop write
   declared. Design keeps `set_tile_prop` outside; should a *card-authored*
   tile-prop write declare?
5. **What is "extra"?** Design: placed on the tile and not a `tile:` rule.
   Does a `status:`/`mark:` instance anchored to the tile count when it does
   carry a tile (they are board-wide today, `tile = -1`)?
6. **Cancel semantics of a house write.** A cancelled `add_house` answers the
   old count, which a body can read as "already at cap". Is a distinct
   `-1`-style "cancelled" answer needed?
7. **`Fixed` charge vs build modifiers.** A build before-hook may still set
   `build_cost_percent` / discount, which a `Fixed` charge ignores by design.
   Should a hook's discount on a `Fixed` build reduce it (and how is the
   already-paid `to_the_peak` refund handled)?
8. **Miracle's "可"** — the design still always spends when it can.
9. **`tile_extra_count` and immunity.** An extra whose own card is immune /
   face-down still counts as "受到其他效果影响"; should the count follow the
   dispatch's face-down filter?
10. **Custom tile build bodies.** `card_build_as` does not re-dispatch the
    tile's `On::Build` body, so a custom body runs on the step path only. Is
    that acceptable, or should the verb dispatch the body and the body call a
    second, non-dispatching entry?
11. **Discard on the swapped fraction.** (3)'s discard is keyed to the
    placement slot the card sits at, so the settle on the endpoint it was
    placed on discards (after the agent body). A settle on 弦卷集团's own home
    slot, which now runs the card's tile's rule, does not.
