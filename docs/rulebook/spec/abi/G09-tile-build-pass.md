# ABI design G09 — Tile, build, pass and house-removal hooks

**Status: draft for user review. Design only — no source edits, no commit.**
Worktree `/home/monopoly/web/.claude/worktrees/card-rulebook-review`, branch
`worktree-card-rulebook-review`.
**Group:** G09, family "Tile, build, pass and house-removal hooks".
**Ground truth:** `docs/rulebook/cards.json`; every fragment below was re-read
from that file on 2026-10-28 against the card id named beside it.

Terminology: **counteraction** = the [反击] window / chain link; **hook** = a
passive listener (`On::Action` / `On::Filter` / a `[持续]` field body). Never
"reaction". A **tile** number is a square identity; a **position** is a slot on
the board loop. They coincide until a tile-rule swap (item 2) exists.

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
writes raise no `effect` window at all, so a card cannot declare at its own
call site and the fix is host-side.

---

## 1. Problem, per item

### 1.1 `marina-suppression-window` (rows 880, 885, 1493)

Book (`通用:[月岛麻里奈]今天也要加油工作喔`):

> [反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。

**Current.** `marina_work.rs:20-32` counteracts
`bandori_std::kind::PassRewardAffected` with `pre::MINE` and no residual guard;
its body (`:48-64`) only logs. Both missing conjuncts are unreadable and the
window is at the wrong point:

* the only raise is inside `settle_reward` (`circle.rs:227`), *after* the
  reward pick. A pass onto #1 from #1, a pass vetoed by `pass_pays`
  (`circle.rs:75-101`), a `NO_REWARD` arm, or any pass whose reward is skipped
  never raises, so its settle cannot be answered either (rows 880, 885);
* nothing reads a suppression flag at any tile extra-effect point, so the
  body cannot make the pass/settle "正常进行而不受到其上的额外效果" (row 1493);
* there is no guest reader for "does this tile carry extra effects", so the
  guard would be forced to hard-code a card list.

**Expected.** A window opens on the pass/settle of the square itself,
independent of `pass_pays` / `NO_REWARD` / the reward pick. The counter sets a
one-shot move tag; every tile extra-effect point for that square skips the
extra instances while it is set. The tile's own base rule still runs.

### 1.2 `parade-tile-swap` (row 1089)

Book (`HHW:笑容大游行` (3)):

> [持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。

**Current.** `smile_parade.rs:196-253` hand-rolls a **settle** swap: the
`SettleBody` entry claims the body with `set_cancelled`, then calls
`card_settle_at` on the partner tile and suppresses re-entry with a move tag.
The [经过] half does not exist: passing the card's square still runs that
square's own pass rule, and passing 弦卷集团 still runs none of the card's
square's. The `[持续]` clause is one continuous statement; only half of it is
implemented (row 1089; `BACKLOG.md` ABI-13, which already scopes the capability
and its "must keeps").

**Expected.** While the card sits there, the square is 弦卷集团 for [经过] as
well as [结算]. Square identity stays static — `ctx::is_start`, `tile_is`,
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

* `smile_patrol.rs:77-79` free build tests only `is_buyable` + `!mortgaged`
  and then calls raw `ctx::add_house`; `prop::NO_BUILD` /
  `prop::NO_BUILD_ABOVE` / ring / tier caps do not veto (rows 118, 123). Its
  play gate (`:36`) and `to_you_far_away.rs:103` price affordability with raw
  `ctx::build_cost`, while an actual build charges `build_cost_pct` /
  `build_discount` — the two disagree (rows 521, 748).
* `miracle.rs:84-88` wants "a crystal *instead of* money" but has no effective
  price reader (`build_cost` is base; `build_cost_pct` / `build_discount` are
  write-only), so it always zeroes and always spends (row 124).
* `to_the_peak.rs:140-157` computes 1.5× itself, pays with `ctx::pay`, then
  calls raw `ctx::add_house` — no `buildBefore`, `buildAfter` or `houseAdded`
  from that path; `ctx::card_build` cannot express the 1.5× because it runs the
  rule body's own price (row 1206).
* `card_build` (`play.rs:1275-1285`) gates but then calls `deed_build`, which
  dispatches the tile rule body; the body (`property.rs:177-221`) hand-assembles
  `buildBefore` → `pay_build` → `add_house` → `houseAdded` → `buildAfter`. So
  the "pipeline" is duplicated between `play.rs:3033-3134` (`build`) and the
  rule body, and neither can be reused by a card.

**Expected.** One build pipeline. A card build is buildability-gated by the
same gate as the build step, priced by the effective figure, may carry an
explicit charge, and runs `buildBefore` / `houseAdded` / `buildAfter`.
Everything the three cards need is a read or a parameter, never a second body.

### 1.4 `tile-write-effect-declare` (rows 36, 38, 989)

Book (`AG:宣战布告`):

> [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。

**Current.** `declare_war.rs:52-74` already reads the whole effect list —
`ctx::effect::hits(player_id)` for the seat half and `ctx::effect::tile(i)` +
`ctx::tile_owner(t)` for the 「或你拥有的格子」 half. But tile writes
(`hostfns.rs:798-820` `set_houses` / `add_house` / `set_owner`) mutate the world
directly: no `Effect` is appended and no `effect` window opens. An
`add_house` from another player's card is therefore invisible, and the row
comment at `declare_war.rs:45-49` admits the same gap as C# (rows 36, 38, 989).

**Expected.** Every tile write declares
`Effect { kind, tile, target: owner }` on the running link **and opens the
`effect` window** before applying, so `effect::count/tile/has` guards see it.
A cancellation in that window voids the write.

### 1.5 `guerrilla-pass-path` (row 866)

Book (`RAS:游击演出` [手]):

> [反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后：传送至任意无主可购买的格子并[结算]，且必须购买。

**Current.** `guerrilla.rs:86-113` checks `Paid` / payer / payee /
`pay_is_rent` / no `by_card` and the free pool. Any rent to another player
opens it; the passed Live House is never tested (the TODO at `:104-107`).
`TriggerMove` carries no path, and the rent `paid` trigger raised during a
settle carries no move at all (`play.rs:3873-3884` has no `@m`), so the card
cannot tell a settle's own rent from an unrelated payment (row 866;
`BACKLOG.md` LOY-GUERRILLA-01).

**Expected.** The settle-causing move's full path is on the `paid` trigger;
the guard requires a path square that is a Live House owned by another player.

### 1.6 `house-removed-kind` (rows 819, 826)

Book (`R:（燐子）Ringing Bloom` (2)):

> 你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半

**Current.** The 「视为」 is recomputed on `HouseAdded` and `Bought`
(`ringing_bloom.rs:32-42,126-142`). `houseAdded` is raised only for `n > 0`
(`wasm_rules.rs:1941-1954`), and `set_houses` raises nothing at all
(`:1916-1926`). A razed max tile leaves `RENT_HOUSES` and the half-charge set
at the old maximum until the next build; a sale does the same (rows 819, 826;
`BACKLOG.md` CRYSTAL-08).

**Expected.** A fall re-runs the view exactly like a rise. The kind is the
symmetric half of `houseAdded` and fires on every downward house write.

---

## 2. Shared root causes

* **R1 — a tile has an identity and a set of effects, with no layer between
  them.** Dispatch knows only "all board instances on `trigger.tile`" plus
  "every player field instance, guard-filtered". Nothing can say "the square's
  own rule" versus "an extra effect on the square", and nothing can make one
  square run another's rule. Items 1 and 2.
* **R2 — pass and settle spins have no shared, early tile window.** The only
  "affected" window is buried in a rule body after a choice
  (`circle.rs:227`); a card that must see the *tile event before any extras*
  has nowhere to stand. Item 1.
* **R3 — tile writes are raw.** `set_houses` / `add_house` / `set_owner` are
  direct array writes: no declared effect, no window, and the rise log is
  one-directional. Items 4 and 6.
* **R4 — card-initiated builds are a second, thinner pipeline.**
  `add_house` commits a house with no gate, no price stage and no hooks;
  `card_build` gates but can only charge the rule body's own price. Items 3
  (and 6, because `add_house` is the shared write).
* **R5 — move facts are step-local snapshots.** `Move.tags` is copied to the
  trigger at raise time, but `plan::set_tag` only writes the plan and the walk
  syncs only `stop_at` / `resolve` (`play.rs:1812-1817`); `fold_plan_delta`
  copies tags only when they changed since move start (`:173-175`); `Move.path`
  never reaches a trigger, and money raised during a settle carries no move.
  Items 1 and 5.
* **R6 — the event-kind set is rise-only.** `HouseAdded` (75) exists; its fall
  does not. Item 6.

Every root cause is generic mechanism: no display text, no player decision
inside an import.

---

## 3. Capability as an invariant

> **A tile event resolves one governing square; the square's own rule runs,
> every extra effect on it is skippable as one set, every write to it is a
> declared effect with a rise/fall pair, and every build — engine or card —
> runs one ordered pipeline that the card can price but not bypass.**

Derived obligations:

* **Governing resolution is total and stable.** A pass or settle at position
  `p` resolves `square_at(p)` (the swap partner when one is bound, otherwise
  `p`), and *every* instance dispatch and tile-fact read of that event uses the
  square. Position is used for walking, `landed`, co-location and the one
  card-level "this physical square was settled" test; nothing else. Display
  *names* carry the square (a log names the tile the event is about, and the
  view's own landing highlight stays positional).
* **The square is the identity, the position is only where it sits.** The
  square's identity never changes: `tile.id`, `ctx::is_start`, `tile_is` and
  every identity fact (`tile_named`, ownership, house arrays, props, the
  square's rule and its extras) stay bound to the square, and a swap only
  changes which position answers at it. An identity reader fed a position
  converts through `square_at`; a physical reader fed a square through
  `tile_pos`.
* **The base rule is never an "extra".** An instance is an *extra* iff it is
  placed on the governing square (`FieldCard.tile == trigger.tile`) and its
  card id does not start with `tile:`. The square carries its effect set: a
  swap moves the set with the square, so dispatch and suppression are keyed by
  the square, never by the event position. The suppression tag hides exactly
  the extras of exactly that square.
* **The tile window precedes every extra.** `tileEffectBefore` is raised before
  the step's first tile raise (`passBefore`), and before the settle's `settle`
  dispatch, so a counter can arm suppression and every later stage of that
  square sees it.
* **One write path per tile fact.** `houseAdd` / `houseRemove` / `ownerSet`
  declare an `Effect`, open `effect`, then apply through one helper; a rise
  raises `houseAdded` and a fall raises `houseRemoved`, once each, after commit.
* **One build pipeline.** Gate → `buildBefore` → price → charge → commit →
  `houseAdded` → `buildAfter`, with the gate and the price shared by the
  engine step and every card verb. A card may choose *what* is charged (none,
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

Wire ids 126–130 (this group's README band; 104 is retired, 107 was never
used). All get an `EngineKind` row in `crates/game-kinds/src/lib.rs` beside 75,
with `is_filter: false` / `hook_only: false` like their rise sibling
`houseAdded`. 126–127 are dual-role: a `TriggerKind` const
(raise site, `ChainKind`) **and** an `ActionKind` const (the same id, the
`Self(TriggerKind::X)` alias form `HouseAdded` uses). 128–130 are
`Effect.kind` spellings only -- never raised, so they get a `TriggerKind` const
for `effect::kind`/`effect::has` and the wire name, but no raise site.

```rust
// TriggerKind — engine namespace
/// `tileEffectBefore` -- before a tile's extra effects run, on a pass step and
/// on a settle. Payload: `tile` (the governing square), `square` (the board
/// position), `mv` (the move/step). Dual-role: a counteraction window and a
/// hook.
pub const TileEffectBefore: Self = Self::engine(126);

/// `houseRemoved` -- a tile's house count has just fallen.
/// Payload: `tile` (the square), `value` (the count after the fall).
/// `player_id` is the tile's owner at the time of the write.
pub const HouseRemoved: Self = Self::engine(127);

// effect-only spellings (`Effect.kind`), one per write:
pub const HouseAdd: Self = Self::engine(128);    // value = delta, target = owner
pub const HouseRemove: Self = Self::engine(129); // value = -delta, target = owner
pub const OwnerSet: Self = Self::engine(130);    // value = new owner

// ActionKind — same ids, the `Self(TriggerKind::X)` alias form:
pub const TileEffectBefore: Self = Self(TriggerKind::TileEffectBefore);
pub const HouseRemoved: Self = Self(TriggerKind::HouseRemoved);
```

`engine()` is the fixed-generic raise-point namespace (`abi.rs:2134`); these
five are the fixed generic points this design adds and no card owns them.

### 4.2 `Trigger` and `TriggerMove` — `crates/game-core/src/engine/rules.rs`

```rust
pub struct Trigger {
    // …existing…
    /// The board position of this event (the slot the mover stepped onto /
    /// the settle ran at), `-1` on kinds with no position. `tile` is the
    /// governing square resolved from it; when no swap is bound the two are
    /// equal.
    pub pos: i32,
}

pub struct TriggerMove {
    // …existing…
    /// Every governing square this move has stepped on so far, in order,
    /// pushed at the same point as `MoveCtx.path` (so a `passBefore` of the
    /// step not yet taken does not see it; its `passTile` and settle do).
    /// Squares, not positions (item 5).
    pub path: Vec<i32>,
}
```

`with_move` (`rules.rs:355-370`) copies `m.path_square` into `path`. Both are
host-only structs (never serialized — see §7). `Trigger::new` initializes
`square` to `-1` like `tile`, and the one hand-written literal
(`wasm_rules.rs:9443`, the card-play bridge) sets it too.

### 4.3 Constants

```rust
// card_sdk::abi::prop — an *instance* prop (dies with the instance)
pub const TILE_SWAP: &str = "tileSwap"; // value = partner square

// card_sdk::abi::tag — a new module beside `prop`, for shared move-tag names
/// Suppress the extras of square `value` for the rest of this move.
pub const TILE_EFFECTS_OFF: &str = "tileEffectsOff";
```

`prop::TILE_SWAP` on a placed instance binds that instance's square to the
value square, one level, both directions. `tag::TILE_EFFECTS_OFF` with value
`q` suppresses extras on square `q` for the rest of the move. The module holds
only names the engine and cards must agree on; a card's private move tag may
keep a local constant.

### 4.4 Imports (`crates/card-sdk/src/ctx.rs`, re-exported via `bandori_std::ctx`)

```rust
// ---- tile identity / suppression reads
/// The board position of this event (the slot the mover stepped onto / the
/// settle ran at), `-1` where there is none. `tile.id` is the governing
/// square; the two are equal with no swap. Identity readers (`is_start`,
/// `tile_is`, `tile_named`) take the governing square (`tile.id`); the
/// position is for co-location, display and "was this physical square the
/// one settled" tests.
pub fn trigger::square() -> i32;

/// The board position square `tile` currently stands at (`tile` itself when
/// nothing is bound to it) -- the value of the engine's `pos_of_square`.
pub fn tile_pos(tile: i32) -> i32;

/// The square standing at board position `pos` (`pos` itself when nothing is
/// bound there) -- the value of the engine's `square_at`.
pub fn square_at(pos: i32) -> i32;

/// How many *extra* instances (non-`tile:` cards) lie on square `tile`. The
/// square carries its extras, so this is identity-keyed and does not move
/// when the square's position does.
pub fn tile_extra_count(tile: i32) -> i32;

// ---- build reads
/// The price a house on `tile` would actually cost this turn:
/// `max(0, build_cost * build_cost_pct / 100 - build_discount)`. No payer
/// side effects, no layer pop.
pub fn build_price(tile: i32) -> i32;
/// The current turn's build percentage (100 = full) and layered cut.
pub fn build_cost_pct() -> i32;
pub fn build_discount() -> i32;
/// `why_not_build_on` minus the ownership test (still includes the buyable,
/// ring, `NO_BUILD`, `NO_BUILD_ABOVE`, lingering, mortgage, cap, full and out
/// refusals; the `Gate::Pay` layers remain the caller's question). For a free
/// build on someone else's tile.
pub fn tile_buildable(player_id: i32, tile: i32) -> bool;

// ---- the one card-build verb
/// Run the full build pipeline (§5.3) on `tile`: gate, hooks, price, commit.
/// `charge` is a [`BuildCharge`] (`FIXED` uses `num`, `SCALED` uses
/// `num / den` of the effective price); `require_owner` adds the owner test to
/// the gate. `true` iff a house committed. Does not re-dispatch the tile's
/// `On::Build` body (the body may call this verb itself).
pub fn card_build_as(
    player_id: i32,
    tile: i32,
    charge: BuildCharge,
    num: i32,
    den: i32,
    require_owner: bool,
) -> bool;

/// The existing two-argument import (`ruleset_card_build`, `sys.rs:1485`),
/// kept unchanged as the `Effective` / owner-required alias. Archived bundles
/// import the symbol directly; new bodies may keep using it (§8).
pub fn card_build(player_id: i32, tile: i32) -> bool;

// ---- move facts
pub fn trigger::move_path_len() -> i32;
pub fn trigger::move_path(i: i32) -> i32; // the governing square at index i
```

SDK-side sugar (not an import, so no `sys.rs` row): `set_tile_swap(partner)`
is `set_prop(prop::TILE_SWAP, partner)` on the caller's own instance; `-1`
clears.

New enum:

```rust
#[repr(i32)] // wire convention, like `BuyKind`
pub enum BuildCharge {
    Effective = 0, // pay the pipeline price (pct, discount)
    Free = 1,      // pay nothing; hooks still run
    Fixed = 2,     // pay exactly `num`; pct/discount do not apply again
    Scaled = 3,    // pay effective × num / den, one ceil-to-10 after the mul
}
```

The import passes the enum as `i32`; an unknown value falls back to
`Effective`. The engine round-trips `paid.loss` and clamps all charges at 0.

### 4.5 Classification

* `tileEffectBefore` and `houseRemoved` are **action** kinds with a hand
  window (like `houseAdded` and `settleBody`); the effect-only spellings never
  raise.
* `card_build_as` is an action verb (hook-running, may prompt through the money
  pipeline); `build_price` / `build_cost_pct` / `build_discount` /
  `tile_buildable` / `tile_extra_count` / `square` / `tile_pos` / `move_path*`
  are pure reads.
* `ctx::set_tile_swap` is sugar over `set_prop`; no import.
* Condition vocabulary (`crates/game-rules/src/cond_pre.rs:1298`, the
  `TileRef` scope): `tile.square` joins `tile.id`; the `TileSnap` initializer
  gains `square: t.square as i64` beside `id`. `tile.id` stays the square,
  `tile.square` is the event position. No other CEL field changes; a placed
  card's `card.tile` is already its square.

### 4.6 State

* `World::swap_at(pos) -> Option<(i32, i32)>` (the identity standing there and
  the binding instance uid), `World::square_at(pos) -> i32` and
  `World::pos_of_square(square) -> i32`. Derived from instances carrying
  `prop::TILE_SWAP`; no new world field (§7).
* `Cx::settle_move: Vec<TriggerMove>` (the nesting stack described in 5.6).
* `SettleFrame.square` (the governed square, decided when the frame opens:
  `SettleFrame::new` takes it as an argument; 5.1).

### 4.7 Wire-breaking vs additive

| change | class |
|---|---|
| five new engine kind ids 126–130 | wire (kind table + `from_wire`) |
| new imports above | wire (import table, `sys.rs`, `sys_kani.rs`, `native_shims.rs`) |
| `HostRequest::BuildAs { player_id, tile, charge, num, den, require_owner }` | host-internal (the new verb's request) |
| `HostRequest::TileWrite { op, tile, value }` (see §5.4) | host-internal (`set_houses` / `add_house` / `set_owner` become round-trips) |
| `HostRequest::Raise.square` (copied from the enclosing trigger) | host-internal |
| `Trigger.square` / `TriggerMove.path` | host-only, exposed only through new imports |
| `MoveCtx.path_square` | serialized save field, `#[serde(default)]` (no `SAVE_VERSION` move, §7) |
| `house_log` tuple gains `delta` | host-internal scratch (never saved) |
| CEL `tile.square` | host-only, additive to the condition scope (`card.tile` already reads the square) |
| `prop::TILE_SWAP` / `tag::TILE_EFFECTS_OFF` | data names, additive |
| tile writes open `effect` / raise `houseRemoved` | behavioural, old bundles keep their engine (§8) |

---

## 5. Engine wiring

### 5.1 Governing resolution (`crates/game-core/src/engine/ops.rs`, `world.rs`)

```rust
fn swap_records(&self) -> impl Iterator<Item = &FieldCard> {
    self.st
        .board_field
        .iter()
        .chain(self.st.players.iter().flat_map(|p| p.field.iter()))
}

/// The identity standing at board position `pos`, and the instance binding it.
fn swap_at(&self, pos: i32) -> Option<(i32, i32)> {
    let partner = |f: &FieldCard| f.props.get(prop::TILE_SWAP).copied().filter(|&p| p >= 0);
    // (a) the binding instance sits at `pos`: the identity standing there is
    // its partner.
    if let Some(f) = self.swap_records().find(|f| f.tile == pos && partner(f).is_some()) {
        return Some((partner(f)?, f.uid));
    }
    // (b) the reverse: some instance names `pos` as its partner, so `pos`'s
    // own identity stands at that instance's position.
    self.swap_records()
        .find(|f| f.tile >= 0 && partner(f) == Some(pos))
        .map(|f| (f.tile, f.uid))
}

pub fn square_at(&self, pos: i32) -> i32 {
    self.swap_at(pos).map_or(pos, |(sq, _)| sq)
}

/// The position the identity `square` currently stands at -- `square_at`'s
/// inverse, with the same two cases mirrored.
pub fn pos_of_square(&self, square: i32) -> i32 {
    // (a') some instance names `square` as its partner: the identity stands
    // where that instance is.
    if let Some(f) = self
        .swap_records()
        .find(|f| f.tile >= 0 && f.props.get(prop::TILE_SWAP).copied() == Some(square))
    {
        return f.tile;
    }
    // (b') `square` is itself the host position of a record: the identity
    // stands at that record's partner. (The tile base instance at the same
    // position carries no prop, so scan past it.)
    self.swap_records()
        .filter(|f| f.tile == square)
        .find_map(|f| f.props.get(prop::TILE_SWAP).copied().filter(|&p| p >= 0))
        .unwrap_or(square)
}
```

Both handle the pair as a two-way binding: a card at `x` carrying partner
`g` makes `square_at(x) = g`, `square_at(g) = x`, `pos_of_square(g) = x` and
`pos_of_square(x) = g`.
One level only: a partner's own record is not followed further (open
question 2). The scan is `O(board + player fields)`; a swap exists only while
such an instance is placed, so the hot walk path pays it only during those
cards.

**Raise sites route through one of two new arms**, both inserted before the
generic `$field = $value` arm (other fields such as `value` / `target` /
`effects` keep using it): a `tile = $sq` arm takes a **square** and derives
`pos = pos_of_square(sq)`, and a `square = $pos` arm takes a **position**
and derives `tile = square_at(pos)`. Every position-fed raise site therefore
moves to `pos =`; each must change (the number is the line today):

* turn start: `turnStartBefore` / `turnStart` (`play.rs:609, 643`, the player's
  pos);
* move frame: `moveBefore` (`:1662, 1906`, `m.from`), `teleported` / the two
  `moveResolved` (`:1974, 1982, 2038`, `m.to` / the player's pos),
  `moveAfter` (`:2025`, `m.to`);
* walk: `tileEffectBefore` / `passBefore` / `passTile` / `pass` (`:1769, 1794,
  1803`, `next`), `passPlayer` in `overlap_at` (`:2004`, `at`);
* teleport: `passTile` (`:1938`, `to`), and the new `tileEffectBefore` (5.2)
  which does not exist today;
* settle: `tileEffectBefore`, `settleBefore`, `settle`, `settleBody`,
  `settleAfter`, `tileResolved` (`:2063, 2166-2226`). A walk-driven frame
  takes the square the landing step already resolved (the move's last
  `path_square` entry, falling back to `square_at(at)` only for a move-less
  settle), so the landing pass and its settle cannot disagree on the square;
  `card_settle_at` pins it instead (5.1). The frame passes
  `tile = frame.square, pos = frame.at` in that order -- arms assign in
  written order, so the explicit `pos` overwrites the `tile =` derivation
  -- and the frame, `settle_tile` and `tileResolved` agree even if a binding
  is written mid-settle.

Square-fed sites (buy, build, mortgage, redeem, charge, the tile writes) keep
`tile =` and get `square` derived. A raise that already carries a *position*
into `tile =` without conversion is the main mechanical risk of this section;
the grep lint (9.3) checks the remaining call sites. Direct constructions that
bypass the macro (`raise_core` in the commit loop at `wasm_rules.rs:4848`, and
the play bridge's literal) set `square` the same way by hand.

**Nested guest raises inherit both.** `hostfns::raise` (`hostfns.rs:1445`)
already copies the enclosing trigger's `tile` onto the raise; it copies
`square` in the same block. The `raise!` macro's `@m` copies the move and its
tags as today.

**Dispatch is unchanged: `tile_rule_instances(t.tile)`.** A tile rule and the
extras lying on it belong to the square; the swap moves that set with the
square (smile_parade's own rule note: 「what the tile *is*, not what is on
it」 -- the cards travel with it, they are not re-anchored). Both hook walks
(`wasm_rules.rs:5009-5160`), the job collector (`:5294-5420`) and
`settle_tile` therefore keep filtering by `f.tile == t.tile`, and the
player-field loop is unchanged. Because a placed instance's `f.tile` *is* its
square, `ctx::self_tile()` / CEL `card.tile` already read the square and every
existing guard (`trigger.tile == self_tile`, `card.tile == tile.id`) keeps
working -- no per-guard edit. Four follow-ups:

* **`players_on*`** compares the stander's position: it counts a stander whose
  `square_at(player_pos)` equals the argument, so existing
  `players_on(tile.id)` call sites need no edit.
* **Physical prop reads** on the mover become governing-square reads
  (`circle.rs` `settle_reward`'s `player_pos` → `square_at(player_pos)` for
  both `tile_prop` calls, the `START` flush at `play.rs:1784`).
* **Physical position reads** from a trigger use `trigger::square()` where the
  question is "where did this happen" (5.1's walk/teleport/settle sites, and
  the 6.2 discard).
* **View.** A board slot draws the square that answers there (`square_at(p)`);
  a field card draws where it was placed (`f.tile`, which is its placement
  slot and its square unless the target slot was already swapped), so a card
  sits on the face its tile swapped with. Pawns, the landed highlight and the
  discard pile stay positional. Without the first, a swap would run the right
  rule under the wrong picture.

That a card lying on a square travels with it is the choice; the opposite
("the card stays with the physical slot") would desynchronise the settle
redirect the card itself performs (pre-swap C# redirects the settle to the
partner, so the partner's extras fire) and is listed as A8.

**Settle body.** `settle_tile` (`wasm_rules.rs:8926`) takes the frame's move
and both frame fields -- `(player_id, at, square, m, main)` from the
`SettleTile` stage (`play.rs:2196`, today
`settle_tile(self, frame.player, frame.at, frame.m.main)` -- the new argument
rides between `at` and `m.main`) -- drives
`tile_rule_instances(square)` (the square and its extras, which travel
together), skipping each extra 5.2's suppression test rejects with
`(m, square, uid)` while the `tile:` base always runs, and reads
`owner = owners[square]`. The body trigger
carries `tile = square`, `pos = at`, and keeps `mv: None` (its own body is
driven directly, not walked; nested raises still get the move from 5.6's
stamping stack). The built-in fallback becomes
`land_at_built_in(player, at, square, main)` (`play.rs:2300`): `prep_land` /
`landed` take the position, `self.tile(square)`, `owners[square]`, the
`log.land` `tile` field and the `offer_buy` / `offer_build` / `pay_rent` calls
take the square. Those and
`rent_figure` / `build_cost` / `why_not_build_on` /
`tile_buildable` / `build_price` are **square-keyed** throughout
(`owners[t]`, `houses[t]`, `mortgaged[t]`, `tile_prop(t)`, the static rent
table and the message/`Pay.tile` field), which is the same value tile rule
bodies already pass (`self_tile`); `buy_quote_for` / `deed_buy` /
`purchase::table_rent` index identities the same way. Position survives only
where it is physical: `prep_land` / `landed`, the frame's `at`, the walk,
`players_on`'s internal `square_at(player_pos)` (5.1) and the view-only
`with_tile` / `auction_tile` highlight (`pos_of_square(t)`), while message
`tile` fields keep the square.
`card_settle_at(player, square, main)` maps the argument through
`pos_of_square` before pushing the frame, and the frame **pins**
`frame.square = square` (the named identity) instead of calling `square_at` --
a card-initiated settle names a tile identity, and a swap already bound by
another instance must not turn that identity into a different square.

**Must keeps**, asserted by tests: `ctx::is_start(t)`, `tile_is(t, kind)` and
`tile_named` keep reading the static board on the **governing square**
(`tile.id`); external cards keyed *on* CiRCLE therefore still fire on the
CiRCLE square wherever it sits. The `START` early flush reads the prop of the
governing square, and every `*_at(square)` reader is square-indexed. A swap
only changes which rule instance answers and which square facts a tile event
reads; `prop::START` is never copied.

### 5.2 The tile window and suppression (item 1)

**Raise points.** The window must precede **every** extra-effect point of the
step, `passBefore` included, so it opens before that raise:

```rust
// walk, after the segment counters are set, before passBefore
let sq = self.w.square_at(next);
raise!(self, "tileEffectBefore", i, @m m, tile = sq, square = next)?;
self.pull_plan_tags(&mut m); // 5.6
raise!(self, "passBefore", i, @m m, tile = sq, square = next)?;
// …existing position write / flush…
raise!(self, "passTile", i, @m m, tile = sq, square = next)?;
```

`sq` is resolved once per step and reused for the whole step (the path push
included), so a binding written inside the window cannot make the pass's
square and its settle disagree. The teleport does the same once at its
destination. In `step_settle_run` a new
`SettleStage::RaiseTileEffect` is inserted before `RaiseSettle` (the frame now
starts there), so the window precedes the `settle` raise (owner resolution
stays in `RaiseSettle`):

```rust
SettleStage::RaiseTileEffect => {
    let (i, at, sq) = (frame.player, frame.at, frame.square);
    let m = frame.m.clone();
    raise!(self, "tileEffectBefore", i, @m m, tile = sq, square = at)?;
    self.pull_plan_tags(&mut frame.m);
    frame.stage = SettleStage::RaiseSettle;
}
```

A cancelled `tileEffectBefore` cancels nothing else (it is a window, not a
gate); `set_cancelled` there only drops the rest of that window's own hook
list -- the `passTile` / `settle` raise is a separate trigger and still runs.
Marina does not cancel it; it arms the tag (6.1).

**Suppression predicate** (`wasm_rules.rs`, one helper used by the two hook
walks, the job collector and `settle_tile`):

```rust
fn extra_suppressed(world: &World, mv: Option<&MoveCtx>, tile: i32, uid: i32) -> bool {
    let Some(mv) = mv else { return false };
    if !mv.tags.iter().any(|(k, v)| k == tag::TILE_EFFECTS_OFF && *v == tile) {
        return false;
    }
    world.field_by_uid(uid).is_some_and(|f| {
        f.tile == tile && !f.card.starts_with("tile:")
    })
}
```

The player-field loops call it with the visited `(uid, _)`; the board loops
with each `(uid, _)`; all pass `t.mv.as_ref()` and `t.tile`, in the collector
as well as in the direct walks.
Board-wide lists (`event:`, `mark:`, `status:`) have
`tile < 0` or an unrelated anchor and stay unsuppressed. `settle_tile` calls
the same helper with the frame's move and `square`; its one `tile < 0` /
no-instance path (`land_at_built_in`) is the square's own base body and always
runs.
`tileEffectBefore` itself is dispatched *before* the tag is armed on the first
pass: the counteraction round of that window runs unfiltered (marina must be
offered), while the window's *hook* walk afterwards reads the tag the counter
just set, so the extras' own `tileEffectBefore` bodies are skipped. A second
pass over the same square in one move is already suppressed (the tag persists
— intended: "本次"). The tag dies with the move (5.6), so the next move is
normal.

`tile_extra_count(tile)` implements the same classification for guards (counts
board + player instances on `tile` whose id is not `tile:`). It is the
residual guard's 「受到其他效果影响」 test; the CEL pre keeps only
`tile_is(tile.id, "circle")` (6.1), and `pre::MINE`'s text carries the
「[使用者]」 half.

### 5.3 The one build pipeline (item 3)

Move the existing body of `build()` (`play.rs:3033-3134`) into one engine
routine. `build()` itself raises `buildBefore` before any guard (out/playing
only), while the real gate is `why_not_build_on` (`play.rs:2887`), checked in
`card_build` (`:1280`) -- outside the moved body. Keep that split in the
routine: the gate stays at the callers, the pipeline starts at `buildBefore`,
so a card path that already gated does not re-prompt.

```rust
pub fn build_task(
    &mut self, i: usize, t: usize, mut charge: BuildPlan,
) -> Flow<bool> // BuildPlan = { kind: BuildCharge, num, den, require_owner }
```

Order (today's `card_build` gate first, then the routine):

1. out / playing / gate: `require_owner ? why_not_build_on : why_not_buildable`.
   A refusal is logged (as today, `card_build` logs `why`) and returns false.
2. `buildBefore` raise (`@tile`), veto on `is_cancelled`.
3. price: `base = build_cost(t)`; for `Effective` apply `pct.clamp(0,100)`
   then the layered cut (exact current block, including the overflow refund);
   `Free` → 0; `Fixed` → `num`; `Scaled` → the post-pct/cut figure × num / den
   with one `ceil_to(…,10)` on the result. `FIXED`'s amount is already rounded
   by its caller.
4. pop one discount layer (unchanged).
5. if cost > 0: the `PayKind::Build` payment, tile = the square.
6. the refund re-check: refuse-with-refund when the tile is full, and when
   `require_owner` and the tile is not the payer's (a `Free` card build may put
   a house on an unowned tile, which the free build does today). Then the house
   write through `cx.write_houses` (5.4) so `houseAdd` declares. `houseAdded`
   is raised **exactly once**, right after the write and before `buildAfter`,
   with the tile's owner as `player_id` when it has one (`-1` → the payer); the
   routine does not also log to `house_log`, so the current double raise (the
   body's explicit raise plus the `add_house` commit log) disappears.
7. `buildAfter`.

Two paths reach it:

* the new guest verb `ctx::card_build_as` -- `HostRequest::BuildAs { player_id,
  tile, charge: Effective | Free | Fixed | Scaled, num, den, require_owner }`;
* the retained `ctx::card_build` / `HostRequest::Build`, which keeps sending
  the existing `{ player_id, tile }` request. Its handler calls
  `build_task(…, Effective, 0, 0, true)`; no guest-visible shape changes.

`card_build_as` **does not dispatch the tile's `On::Build` body**. The step
path still does (`drive_tile_op`); `property.rs:177-221` collapses to a
wrapper that is now only the body's entry:

```rust
pub(crate) fn build(player_id: i32) -> card_sdk::Asked {
    let at = ctx::self_tile().unwrap_or(-1);
    if at >= 0 {
        ctx::card_build_as(player_id, at, BuildCharge::Effective, 0, 0, true);
    }
    Ok(())
}
```

Because the verb is not re-dispatched, a ruleset tile with a *custom* build
body still runs it on the step path but not on a card build (open question
10). `standalone_build` (`purchase.rs:191`, the no-body fallback) becomes
`card_build_as(…, Effective, 0, 0, true)` too.

`why_not_build_on` splits into `why_not_build_on` (owned) and
`why_not_buildable` (the same checks minus the owner comparison; the buyable,
ring, `NO_BUILD`, `NO_BUILD_ABOVE`, lingering, mortgage, tier cap, full and
out arms all stay), both still `Option<Msg>` so the refusal keeps its reason;
the guest import `tile_buildable` is the bool wrapper (`is_none`) that a card
reads. `card_offer_build` (`play.rs:1292-1330`) prices its
options with `build_price` instead of raw `build_cost`. `pay_build` stays as
an import for archived bundles, though the current property body no longer
calls it; its duplicated pct/discount block is replaced by `build_price` so
the two cannot drift.

### 5.4 Tile writes declare an effect (item 4)

Two engine methods own every house/owner write a card body can make; the buy
path keeps its own primitive (`purchase::assign_deed`, below). They compute
the *effective* change first (clamping to the tile's cap), and do nothing at
all -- no window, no write -- when the value is unchanged (`ownerSet` with the
same owner included):

```rust
// crates/game-core/src/engine/cx.rs
pub enum HouseWrite { Set(i32), Add(i32) } // signed delta allowed on Add

impl Match {
    /// Declare `houseAdd` / `houseRemove` for `tile` on a fresh `effect`
    /// window, then apply. `Flow<i32>` = the post count; a cancellation
    /// answers the pre count.
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
// `value` is the size of the change, unsigned (129 is the removal spelling).
let kind = if delta > 0 { "houseAdd" } else { "houseRemove" };
let target = self.w.st.owners[t]; // the owner *before* the write, -1 if none
let t0 = raise!(self, "effect", by, tile = t as i32, target = target, effects = vec![
    Effect { kind: kind.into(), target, from: -1, tile: t as i32, value: delta.abs() },
])?;
if t0.is_cancelled() { return Ok(before); }
self.w.st.houses[t] = after;
Ok(after)
```

`by` is the running call's player (`call.player_id()` in the bridge), the same
`by_card` every hosted effect uses; `ownerSet` uses the analogous shape with
`value = owner`.

**Guest path.** `hostfns::set_houses` / `add_house` / `set_owner` cannot reach
`Cx`, so each becomes a `HostRequest::TileWrite { op, tile, value }` request
(`op`: `0` = set houses, `1` = add houses, `2` = set owner) answered inline by
a handler beside `Build` (`wasm_rules.rs:3959`) that calls the method above.
The handler also does the run bookkeeping a guest add always did:
`flush_header()` and, for a non-zero delta on an owned tile,
`house_log.push((owner, tile, now, delta))` (5.5). The request answers the
post value, or the pre value when cancelled. The return shapes are unchanged:
`add_house` still answers the count, `set_houses` / `set_owner` answer nothing.
The pause is required for replay correctness: `add_house` is not idempotent,
and a request answer is skipped on the body's replay pass.

**Not affected:** `tile_rule_instances`; `set_tile_prop` stays outside (a view
write; open question 4). `AssignDeed` keeps its direct
`purchase::assign_deed` write and only contributes the house fall (5.5); it is
an engine commit after the buy pipeline, not a guest effect.

### 5.5 `houseRemoved` (item 6)

`house_log` becomes `(owner, tile, now, delta)`; every guest house write
reaches it through the `TileWrite` handler, which logs a delta of either sign
(still only when `owner >= 0` at the time of the write). The `AssignDeed`
handler (`wasm_rules.rs:4229-4245`) pushes a fall for the **previous** owner
when `houses` lowers. `commit_after` (`:4848`) raises `houseAdded` for
`delta > 0` and `houseRemoved` for `delta < 0`, with `value = now` and the
logged `owner` as `player_id` -- the owner at write time, which for a raze on
assignment is the seller. No entry, hence no raise, when the clamp leaves the
count equal (for example `add_house(-1)` at 0).

`ringing_bloom` adds `On::Action(Action::HouseRemoved(...))` beside
`HouseAdded`/`Bought`, and a `Discarded` entry that clears the tile props the
card armed.

### 5.6 Move tags and path (items 1, 5)

* `MoveCtx` gains `#[serde(default)] path_square: Vec<i32>` (a plan is saved,
  so the field is defaulted for old saves); the walk pushes the step's resolved
  square (5.2's `sq`) beside `m.path.push(next)`; teleport sets
  `[square_at(to)]`.
* `fold_plan_delta` (`play.rs:173-175`) copies tags **unconditionally**
  (`m.tags = plan.tags.clone()`, dropping the `if plan.tags != before.tags`
  guard), fixing the pre-armed-tag gap.
* `pull_plan_tags(&mut m)` (same copy) runs after every tile-carrying raise and
  after every `tileEffectBefore`/settle stage.
* At `moveResolved` and at teleport completion (and on either cancellation
  path) `m.tags.clear()` and `turn.plan.tags.clear()` — the tag is one move.
* `step_settle_run` pushes the frame's `TriggerMove` onto `Cx::settle_move`
  before its first stage and pops at `Done`. The bridge's `counteract` entry
  (`wasm_rules.rs:9465`, the single point both engine `raise!` and
  `raise_core` reach) stamps `t.mv = cx.settle_move.last().cloned()` when the
  trigger has no move and the kind is one of `effect` / `pay` / `payAfter` /
  `paid`. This gives the rent `paid` trigger (`play.rs:3873`) the settle's
  move, its tags and its full path — no per-payment `@m` at every rule body.

### 5.7 Kani model

`crates/card-sdk/src/kani_host.rs` gains `Op` rows 411–430 (the enum ends at
`remove_mark` 340; G02/G04/G06/G05/G07 take 341-410 ahead of this group), one
per new import, and `sys_kani.rs` (`set_tag` at 1066,
`card_build` at 1166, `trig_move_tag` at 786) / `native_shims.rs` / the host
trait register each beside its neighbours.

Stubs in `rules/kani/src/host.rs`: `build_cost_pct` = 100, `build_discount` =
0, `build_price` = the tile's base cost (the stub's own `build_cost` table),
`tile_buildable` = 1, `tile_extra_count` = 0, `card_build_as` = 1,
`trigger_square` = the trigger tile, `tile_pos` = the argument,
`square_at` = the argument, `move_path_len` = 0, `move_path` = 0.
The swap needs no op: `set_prop(prop::TILE_SWAP, …)` is the existing op.

The pure model lives in `rules/kani/src/spec/abi_g09.rs`: `price_of(base, pct,
cut)`, `suppressed(tag_value, trigger_tile, trigger_pos, inst_tile, is_base)`,
`fall_raised(before, after)`, `pipeline_commits(...)`. Proofs are §9.4.

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
let sq = trigger::tile();
ctx::plan::set_tag(tag::TILE_EFFECTS_OFF, sq);
ctx::log(player_id, &Msg::new(key!("marina_work_normal"))
    .player_id("who", player_id).tile("tile", sq));
```

The log's tile is now real rather than a static "#1". The condition spells
`pre::MINE`'s text (`actor == owner`; `pre` is a `&'static str`, so the half
cannot be a constant reference) and adds `tile_is(tile.id, "circle")` -- the
book's "#1格子" (`ctx::is_start` is that same test). `tile.id` is the
governing square, so a swapped start square still passes, and
`circle_rebuild`'s re-home cannot lose it (`prop::START` never leaves the
square; the re-home only re-points the effects).

### 6.2 `smile_parade` (item 2)

**Before** (`:52, 178-253`): one `SettleBody` entry hand-redirects (1) and (3)
through `card_settle_at`, keyed `TAG_REDIRECT` / `TAG_AS_AGENT`; no pass half.

**After.** Two mechanisms share the card, each where it belongs:

* (3), the continuous swap, becomes **one prop write**. `counteract` calls
  `ctx::set_tile_swap(ctx::tile_named('弦卷集团'))` right after its
  `place_card_on` (the body runs with the new instance as `self`, so the prop
  lands on it). The binding is instance-owned and keyed to the instance's
  square, so (2)'s `set_self_tile` (which keeps props) carries it to the new
  square without a re-arm, and the discard drops it with the instance.
  `SettleBody` loses the (3) branch entirely; both the pass and the settle
  dispatch resolve to the partner already, so the entry shrinks to the (1)
  path: condition `tile.id >= 0`, `settle_body_guard` =
  `!trigger::cancelled() && move_tag(TAG_REDIRECT) == 0 &&
  move_tag(TAG_AS_AGENT) != 0`.
* (1), 「当次移动的移动终点视为"弦卷集团"地产商」, is not a swap of two
  squares: the card sits on group while the *endpoint* must settle as the
  agent. Keep the `TAG_AS_AGENT` branch and its `card_settle_at(player, group,
  main = true)`; the as-agent half rides the tag, which the settle reads.
  `card_settle_at` pins the named square, so a swap bound by an older copy
  cannot turn the nested settle back into that copy's square.
  `TAG_REDIRECT` survives only as the card-local re-entry/latch marker
  (`resolve_guard`, `on_resolved`); `TAG_AS_AGENT` stays.
* (3)'s discard tests the settle's **position against the slot the card was
  placed on**, which after the placement is its `self_tile()` numerically
  (`trigger::square() == card_t`; `at` becomes `trigger::square()`, the bare
  `at == card_t` comparison stays but now compares a position with the
  placement slot). So the next settle on the endpoint the card was placed on
  discards (after its agent body), while a settle at 弦卷集团's own home slot,
  which now runs the card's tile, does not -- matching the pre-swap behaviour
  (open question 11). The placing settle itself stays exempt through the
  existing `PROP_ARRIVED` latch, and the test agrees with the draw position
  because the card is drawn at that same placement slot (§5.1).
* (2)'s 「[移动终点]」 placement takes the endpoint's **square**:
  `set_self_tile(trigger::tile())` (the square standing at the endpoint), and
  the message names it with `trigger::tile()`. The instance's binding (the
  `(g,g)` self-record armed at (1)) is keyed by its own square, so re-pointing
  the square to the endpoint makes it `(endpoint, g)`: the endpoint position
  now resolves to the group (which is (1)'s point and lets the (1) settle run
  as agent), and the card, placed on the endpoint square, stays drawn there
  (the view draws the group's face at that slot, so the card sits on the
  group). Its
  「卡仍位于弦卷集团」 condition is a square test
  (`card.tile == tile_named('弦卷集团')`, unchanged -- the anchor is the
  square); it is false after (2), as the book intends. A card-initiated settle
  is still excluded by `settle_after_guard`.

The endpoint square *is* `trigger::tile()`: the mover's endpoint position
resolves to the square standing there. `settle_body`'s endpoint checks are
gone with the (3) branch; the surviving (1) branch keeps `at = trigger::tile()`.

### 6.3 `smile_patrol` (item 3)

**Before** (`:24-92`): raw `add_house`, `is_buyable/!mortgaged` only, raw price
in `cant_play`/`card_offer_build`.

**After**:

```rust
// cant_play: affordability at the effective price
if ctx::can_build_on(player_id, t) && money >= ctx::build_price(t) { … }
// the roll face is a position; every build read is square-keyed
let pos = (r - 1).rem_euclid(ctx::tile_count());
let tile = ctx::square_at(pos);
// the verb carries the buildability gate (`require_owner` false, so someone
// else's / an unowned tile is allowed)
let before = ctx::houses_of(tile);
if ctx::card_build_as(player_id, tile, BuildCharge::Free, 0, 0, false) {
    ctx::log(…, ctx::houses_of(tile));
} else {
    ctx::log(…, before);
}
```

The paid half's `owned_tiles` already yields squares (`ops.rs:264` iterates
the owner array), so `card_offer_build` needs no change beyond its price.

`card_offer_build` does the first half: its option filter and the option's
`cost` label both switch from `build_cost` to `build_price` (still through
`deed_build`, so a tile with a custom body keeps it).

### 6.4 `miracle` (item 3)

**Before** (`:84-98`): always `set_build_discount(build_cost, 1)`, always
spend a crystal.

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
// The cut is absolute and subtracted *after* the pct, so it must cover the
// whole post-pct figure, not just what is still due: `price + the cut already
// armed`. Setting only `price` would leave an earlier card's cut standing.
ctx::set_build_discount(price + ctx::build_discount(), 1);
```

`after_build` spends one crystal only when the slot is 1, then clears the slot
(so a second stacked copy's after-hook at the same build does not spend a
second one). The entry guard still requires a crystal and `card.placed`;
`trigger::tile()` is the governing square the build is on.

### 6.5 `to_the_peak` (item 3)

**Before** (`:139-164`): `pay` + raw `add_house`.

**After**:

```rust
let amount = ceil_to(ctx::build_price(tile) as i64 * 3 / 2, 10);
if ctx::money_of(player_id) < amount { return Ok(()); }
if ctx::card_build_as(player_id, tile, BuildCharge::Fixed, amount, 0, true) {
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
let n = trigger::move_path_len();
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
settles has length 0 and stays closed. The TODO at `:104-107` goes away; the
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

* **SAVE_VERSION stays 16.** No new `Saved` field: the swap rides
  `FieldCard.props["tileSwap"]` (already serialized), the tag rides
  `TurnCtx.plan.tags` (already serialized, v7), and `Trigger` /
  `TriggerMove` are never serialized (`Saved.pending` holds `Routine` state and
  `Live`, not triggers). The one new serialized field is
  `MoveCtx.path_square`, marked `#[serde(default)]`; a save taken mid-move
  restores an empty path, so only a `paid` window during that restored move
  sees `move_path_len == 0`. `house_log` is per-run scratch. A save taken with a
  swap armed restores the prop and therefore the swap; a save mid-move
  restores the tag. A body paused at a `TileWrite` is covered by the existing
  `Saved.pending` routine: the request is re-derived when the body re-runs, so
  no new saved field exists. `HostRequest` is host-internal (not serialized;
  `#[derive(Debug, Clone, PartialEq)]` only), so variants and fields added to
  it move no version.
* **RECORD_VERSION stays 3.** The input stream is unchanged; the record's
  `EngineStamp.save_version` does not move.
* **Replay.** Old `.bdrec` files replay on their archived engine bundle, so
  the behavioural changes to `add_house` / `set_houses` / `set_owner` /
  `card_build` do not alter old replays. The retained two-argument build import
  (§8) keeps an old bundle's card table loadable, though it is the bundle's
  engine that answers it.

---

## 8. Versioning

`ABI_VERSION` 112 → **113** (tenth group in landing order), entry inserted immediately below the constant
(newest first):

```
/// v113: **tile events, one build pipeline, tile-write effects, house falls,
///    move paths.** New engine kinds `tileEffectBefore` (126),
///    `houseRemoved` (127) and the effect-only spellings `houseAdd` (128) /
///    `houseRemove` (129) / `ownerSet` (130). `Trigger` gains `square` (the
///    board position; `tile` is the square, and a square's rule instances,
///    extras, owner, houses, mortgage and props all travel with it) and
///    `TriggerMove` gains `path`; `prop::TILE_SWAP` (SDK sugar
///    `ctx::set_tile_swap`) re-points a position at a square, one level, and
///    `ctx::square_at` / `ctx::tile_pos` are the only conversions — identity
///    keys stay on the square, so `ctx::self_tile` and `card.tile` already
///    read it. New imports `ctx::trigger::{square, move_path_len,move_path}`,
///    `ctx::tile_extra_count`, `ctx::tile_pos`, `ctx::square_at`,
///    `ctx::tile_buildable`, `ctx::build_price`, `ctx::build_cost_pct`,
///    `ctx::build_discount`, `ctx::card_build_as`; new move tag
///    `tag::TILE_EFFECTS_OFF`; new CEL field `tile.square`. Host requests:
///    `BuildAs` (the build verb), `TileWrite` (the guest tile writes) and a
///    `square` on `Raise`;
///    `ruleset_card_build` (two arguments) stays as the
///    `BuildCharge::Effective` alias. `set_houses` / `add_house` /
///    `set_owner` now open an `effect` window with a declared tile effect
///    before applying and may be cancelled there; `add_house` / `set_houses`
///    raise `houseRemoved` on a fall, and `assign_deed` contributes a fall for
///    the previous owner. SAVE_VERSION 16 and RECORD_VERSION 3 unchanged.
```

The old two-argument import is a real alias, not sugar in the guest: an
archived bundle calls the symbol directly. `card_build`'s handler keeps
answering the same `HostRequest::Build` shape, so old bundles are unchanged.

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
| 2 | `g09_parade_pass_runs_the_swapped_rule` | card on CiRCLE → passing 弦卷集团 pays the CiRCLE reward, passing the card's square does not |
| 2 | `g09_parade_settle_runs_the_swapped_rule` | card on a rent tile → settling that square runs the agent body; settling 弦卷集团 charges the rent tile's rent to its owner |
| 2 | `g09_parade_discard_only_on_its_own_position` | a settle on the endpoint slot the card was placed on discards (after the agent body); a settle at 弦卷集团's own home slot, which runs the card's tile, does not |
| 2 | `g09_parade_identity_stays` | an effect keyed `tile.id == tile_named('CiRCLE')` fires where the CiRCLE identity now stands, and the start reward follows the same resolution (the `START` prop is never copied) |
| 2 | `g09_parade_swap_ends_on_discard` | after (3)'s discard both squares run their own rules again |
| 2 | `g09_parade_view_draws_the_square_at_each_slot` | while bound, each slot's tile is `square_at(p)` and the card draws at its placement slot (`card.tile`) |
| 3 | `g09_card_build_gate_matches_the_build_step` | matrix of ring / full / mortgaged / `NO_BUILD` / `NO_BUILD_ABOVE` — refused identically by `card_build_as` |
| 3 | `g09_patrol_free_build_respects_tile_vetoes` | 3d20 onto a `NO_BUILD` tile raises nothing |
| 3 | `g09_patrol_affordability_uses_the_effective_price` | `pct = 50` → playable with half the base cost and refused with just under |
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
| 5 | `g09_guerrilla_path_is_the_settling_move_only` | a rent between two settles has `move_path_len == 0` |
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
swap rows the event position's own square `x` is bound to a partner `g`: the
base rule and its extras all carry `f.tile == g`, while the only instance
carrying `x` is the binding card itself. The correct dispatch (`t.tile`)
therefore returns exactly the partner's set, and a mutant dispatching by
`t.square` returns just the binding card. A tag row values
`TILE_EFFECTS_OFF` with the position instead of the square: nothing is
suppressed, so the square-valued tag is pinned too.

### 9.3 Static sweep

* A grep lint lists every site where a **position** reaches an identity
  reader, or a **square** reaches a physical one. Identity readers take a
  square: `tile_is` / `tile_named`, `is_start`, `tile_owner`, `houses_of` /
  `add_house` / `set_houses`, `build_cost` / `tile_prop` / `set_tile_prop`,
  `set_self_tile`, `place_card_on`, the `tile` of a `HostRequest`. Physical
  readers take a position: `ctx::dist`, `teleport_to`
  / `plan::set_teleport_to`, `tile_steps_ahead`, `move_from`, the discard
  slot, `trigger::square()`. Position sources are `ctx::player_pos(p)`,
  `trigger::move_from()`, a roll face and the teleport cursor; square sources
  are `tile_named`, `owned_tiles`, `self_tile`, `trigger::tile`,
  `ctx::ask_tile` over a square list. The repair is `square_at(pos)` into an
  identity reader, `tile_pos(square)` (or `trigger::square()`) on the
  physical side, and `ctx::ask_tile` keeps square lists as squares. Known
  hits:
  `rules/tiles/src/circle.rs` `pass_pays` (`is_start(move_from)`) and
  `settle_reward` (`tile_prop(player_pos)`),
  `skills/skill-groups/src/ras.rs:57,85` (`ON_HOUSE` is a square tested
  against `player_pos`) and `:105` (`set_teleport_to(owned_livehouses…)`),
  `skill-personas/src/tomoe_ramen.rs:68,81-85` (`shop()` is a square tested
  against `player_pos` and fed to `dist`),
  `skill-personas/src/tomori_poem.rs:117,122,151` and
  `saki_crychic.rs:58` (`dist(trigger.tile(), position)`), saki's `nudge`
  (`:119-134`, `landing ± 1` positions but `landing` is `trigger.tile`),
  `card-ras/src/studio_storm.rs` `on_own_lh` (`tile_owner(pos)`,
  `is_live_house_for(pos)`) and `settle_body` (`dist(trigger.tile(),
  self_tile)`),
  `card-mygo/src/haneoka.rs:39-48,80-84` (`pos = player_pos` handed to
  `tile_owner` / `houses_of` / `can_build_on` / `add_house`),
  `card-ag/src/ran_as_usual.rs:101` (`turn_snap().pos` in a message `tile`),
  `card-mygo/src/endless_journey.rs:44` and `card-ras/src/studio_storm.rs:72`
  (`place_card_on(player_pos)`), and `smile_parade`'s (3) discard
  (`trigger::square() == card_t`, 6.2). Already
  square-to-square and
  unchanged: `card.tile`, `self_tile`, name lookups, `ask_tile` over
  `owned_tiles`, and the CEL `card.tile == tile.id` pre
  (`card-mygo/src/rana_funny.rs:24`, `card-ppp/src/kasumi_love_all.rs:28`).
  Pure position pairs (`clear_cp`'s free-square scan, `surprise_duel`'s
  `tile_forward`, `tomori_inner_shout`'s `dist`) stay positions.
* Every card body that calls `add_house(±1)` for a build path is checked
  against `card_build_as` (the raw write stays legal for a pure move, e.g.
  `parking_space` moving a house between tiles). `smile_patrol` (6.3) and
  `haneoka` (its d20's ≥10 branch) are the two free builds that must move;
  `hey_kids` / `sunset` / `pareo_far` stay raw (moves, not builds).
* Every new import has a `sys.rs` entry with its `link_name`, a
  `sys_kani.rs` sibling, a `native_shims.rs` symbol and a `kani_host::Op` row;
  `crates/rules-native/tests/abi_link_names.rs` fails if any is missing, and
  the `Op` row count must stay in step with `sys_kani.rs`.

### 9.4 Kani obligations

`rules/kani/src/spec/abi_g09.rs` models the pure functions; the harnesses sit
in `rules/kani/src/proofs/abi_g09.rs`:

* `price_iff` — `price_of` equals `max(0, base * pct / 100 - cut)` for arbitrary
  inputs; mutants clamping before the cut and dropping the 0 floor fail.
* `pipeline_commits_iff` — a commit happens iff gate passes, `buildBefore`
  stands, the charge is payable and the tile takes a house; a mutant skipping
  the gate or the veto fails.
* `suppressed_iff` — `suppressed(tag, trigger_tile, trigger_pos, inst_tile,
  is_base)` is true exactly when the tag and the instance's tile both equal
  `trigger_tile` and the instance is not a `tile:` base; a mutant comparing
  either against `trigger_pos`, or one that also hides the base, fails.
* `fall_iff` — a `houseRemoved` is raised exactly when `after < before`, once;
  a mutant on `<=` fails.
* `swap_involution` — with one record `(a,b)`, `square_at(a) == b`,
  `square_at(b) == a`, `square_at(x) == x` otherwise; with no record identity.
  The same model proves `pos_of_square` is the two-way inverse
  (`pos_of_square(b) == a`, `pos_of_square(a) == b`, and
  `pos_of_square(x) == x` for an unrelated `x`), so `square_at(pos_of_square(s)) == s`
  and `pos_of_square(square_at(p)) == p` for every `s`/`p` in the pair.
* `path_is_every_step` — `path.len == steps` and each push equals the traversed
  square; a mutant keeping only the endpoint fails.
* `start_never_travels` — for any swap record, `is_start(square_at(p))` is
  unchanged for every `p` where the start prop is read.

Card bodies stay PROTOCOL-LEVEL; the proofs sit on the pure SDK-side model.

---

## 10. Alternatives

**A1 — keep `PassRewardAffected`, add a tag read at the extras only.**
Smallest change for item 1: raise nothing new; marina stays on the reward
window, body sets a tag, the extra points read it. Rejected because the
window is behind the reward choice: a pass with no reward cannot be answered
(the item's own defect) and a settle has no window at all.

**A2 — run another tile's PassTile body from a card, no swap layer.**
`tile_rule_instances` stays single-tile; `smile_parade` counteracts `PassTile`
on its square and calls a new `ctx::tile_pass_at(owner, group)` that re-raises
the other rule's body, with the same `TAG_REDIRECT` one-shot. Much smaller
than §5.1, but the settle half must keep its hand-rolled redirect (two
mechanisms), extras on either square still do not travel, and
`circle_rebuild` gets nothing. Rejected in favour of the swap, which is
ABI-13's promised capability and subsumes both halves.

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
rise point (id 75, `abi.rs:2624`) used by existing cards; making its sign meaningful
would change every existing listener and its docs.

**A7 — full physical position/owner swap for item 2.** Swap ownership, houses
and mortgage arrays as well as the rule. Truest to 「交换位置」 but touches the
save layout and every array reader. Deferred to open question 1.

**A8 — pin extras to the physical slot.** Dispatch the base by `t.tile` but the
extras by `t.square`, so a card lying at a position is not carried by the
swap. Rejected: the pre-swap card redirects the settle to the partner, so the
partner's extras fire; pinned extras would run the slot's set instead, and a
placed `tile:` card would need a third rule.

---

## 11. Open questions

1. **Does the swap move ownership/houses/mortgage, or only the rule and its
   facts?** Design: the settled *square* keeps its owner, houses and mortgage
   (`owners[trigger.tile]`, the square-keyed arrays); they are never copied to
   the position. The live consumers (agent, event cafe) need no more. If a
   general pair swap with physical arrays is required, the safe path is A7 and
   a separate audit.
2. **Two overlapping swaps.** One level only; the first record in board order
   wins. Is a stack (`a→b→c`) wanted, and does the second card displace the
   first? The live case is two `smile_parade` copies: the second copy's (1)
   placement on 弦卷集团 is shadowed by the first copy's reverse binding, so a
   settle at the 弦卷集团 position resolves to the first copy's square. Is
   "first placed wins" the wanted ruling, or should a copy already on the
   group square outrank it?
3. **`tileEffectBefore` on every tile step.** It opens a hand round on every
   walk step even with no extras. Is a payload flag (`extra_count`) or a
   raise-only-when-nonzero rule wanted? Design raises unconditionally so a
   card's own "when affected" guard is its only filter.
4. **`set_tile_prop` inside `effect`.** View writes (`ringing_bloom`'s
   `RENT_FACTOR` reconciliation) would flood the window if every prop write
   declared. Design keeps `set_tile_prop` outside; should a *card-authored*
   tile-prop write declare?
5. **What is "extra"?** Design: placed on the square and not a `tile:` rule.
   Does a `status:`/`mark:` instance anchored to the square count when it does
   carry a tile (they are board-wide today, `tile = -1`)?
6. **Cancel semantics of a house write.** A cancelled `add_house` answers the
   old count, which a body can read as "already at cap". Is a distinct
   `-1`-style "cancelled" answer needed?
7. **`Fixed` charge vs build modifiers.** A build before-hook may still set
   `build_cost_pct` / discount, which a `Fixed` charge ignores by design.
   Should a hook's discount on a `Fixed` build reduce it (and how is the
   already-paid `to_the_peak` refund handled)?
8. **Miracle's "可"** — the design still always spends when it can. C# reads
   the effective price; whether the player chooses per build or the card
   auto-takes is a judgement call (`OPEN-视为房屋数` neighbourhood).
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
    slot, which now runs the card's square's rule, does not. The alternative
    reading -- discard whenever the card's *square* was the one settled,
    wherever it stands -- is equally text-supported; C# and the pre-swap
    behaviour are the placement-slot reading, which the design keeps.
