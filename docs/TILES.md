# Tile rules

Standardize tile behaviour the same way card and skill rules are implemented.
Where [CARDS.md](CARDS.md) covers what a *card* does and [ENGINE.md](ENGINE.md)
covers the match shell, this doc covers what a *board tile* does when someone
lands on it -- and how cards bend that.

Status: **Phase 2 complete; Phase 3 mostly complete (engine-fix batch,
2026-10-06)**. The migration runs in three phases (below); each step records
its test gate in [Per-step results](#per-step-results). Phase 2's six tile
kinds settle through rule instances on the board owner. Phase 3 removed the
CiRCLE-reward flag, `settleAtEnd` and (for placed cards) `noBuild`, moved the
「支付减半」 scale into the money pipeline's `payMul` stage, and made the settle
body its own [反击]-able chain link. What is still short is listed under
[Left short](#left-short).

## Why

Today tile behaviour is a hard-coded `match tile.kind` in
`crates/game-core/src/engine/play.rs` (`land_at`), plus two special cases
outside it (`pay_rent`'s RiNG formula, `circle_reward`). Cards bend tiles
through **engine flags** -- side channels the rulebook never names:

| flag | where | set by | after Phase 3 |
|---|---|---|---|
| ~~`no_circle_reward`~~ | ~~`MoveCtx.plan` + `state::key::NO_CIRCLE_REWARD`~~ | detour, PPP band (2), PP band (3), 凑友希那 (1), 赤音, tsugumi | **done** -- `prop::NO_REWARD` on the rule instance |
| `rent_factor` / `pay_factor` | `MoveCtx.plan` | studio_storm, kasumi_love_all, council_check / sakiko_lead, saki_move, rana_funny, taki_serious, repaint, kaede_support | **moved** -- the read is the money pipeline's `payMul` stage (`scale_settle_payment`), so it covers the payment as card effects shape it; the arming is still `plan::set_*_factor` for a hand card (see [Left short](#left-short)) |
| ~~`buy_discount` / `free_buy` / `raze_on_buy`~~ | ~~`TurnCtx`~~ | tsugu_ycm, roselia band (1) / maze_warehouse | **done** -- `ctx::linger` + `BuyAdd` −1500 / `BuyMul` ½ / `BuySet` 0 + `BuyAssign` houses 0 (`docs/PURCHASE.md`) |
| ~~`extraColor:<tile>` / `st.tile_colors`~~ | ~~per-player state / `MatchState`~~ | soyo_colors, asahi_aim, roselia band (1), guerrilla, soyo_clear | **done** -- tile props `colorFor:<p>` (per player) and `prop::ANY_COLOR` (「该格获得所有颜色」); `is_color` / `is_live_house_for` read them |
| ~~`noBuild` (play-from-hand)~~ | ~~per-player state~~ | council_check | **done** -- a `ctx::linger` instance carrying `prop::NO_BUILD`, which `why_not_build_on` reads |
| ~~`settleAtEnd`~~ | ~~per-player state~~ | rinne_rain, tomori_crychic | **done** -- `ctx::before_turn_end` + `On::AtEnd` whose body calls `ctx::settle` |
| ~~`settleInstead`~~ | ~~`HookKind`~~ | ~~parking_space, smile_parade~~ | **done** -- `HookKind::SettleBody`; hey_kids moved to `ChainKind::SettleBody` |

Cards and skills are **rule instances** (`CardDef` → manifest → `FieldCard`
placed by `bind_skills`); tiles are not. That asymmetry is what this doc
removes. It also removes the field stand-ins and `slot` scratch that cards use
today to remember per-tile facts (Anon Tokyo's `anon_tokyo_link_<tile>` slots,
Change the world's `change_world_turn`, studio_storm's placement note) --
cross-test cases `t01`–`t05` in `crates/game-rules/tests/rb_cross_tiles.rs` are
`#[ignore]`d on exactly that gap.

The rulebook decides *what*; the C# port is not a spec.

## The rule book

`data/rules.txt`, 「基础[结算]规则」 (lines 94–107) is the whole of the
settlement spec:

> · CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。
> 　· [经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]。
> · CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。
> 　· 抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。
> 　· 事件结算后进入事件弃卡区。如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。
> · 无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。
> · 玩家拥有的[可购买格子]的[结算]是：
> 　· 如果格子地契未抵押则可选择[消耗]格子地契所标注的房屋建筑费进行升级建造，每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得。
> 　· 如果格子地契已抵押则无效果。
> · 其他玩家拥有的[可购买格子]的[结算]是：
> 　· 如果格子地契未抵押则[支付]拥有格子的玩家格子地契所标记的现等级地租。
> 　· 如果格子地契已抵押则可选择[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态。此次购买的价格不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款。
> · [地产商]的[结算]为：若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10），否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。

Supporting glossary (same file):

> · [可购买格子]：非CiRCLE，CiRCLE 咖啡厅，江户川乐器店，流星堂，或[地产商]的格子。
> · [CiRCLE奖励]：[获得]2000资金或抽1张卡。
> · [结算]：执行格子上的所有效果，包括基础效果以及技能或卡所导致的效果。

The last one is the licence for this whole design: **[结算] runs every effect
on the tile, base and card-driven alike** -- so a tile's effects belong in the
same rule-instance machinery as cards.

What the book does **not** say, marked `TODO(规则书)` in the bodies:

* **RiNG rent.** The formula 「地主拥有的 RiNG 数量 × ringMultiplier × 1d20」
  is a `data/match_rules.json` note, not rulebook text. 「所有RiNG不可升级」
  (line 102) is the only RiNG clause in the book. TODO(规则书): the dice-rent
  table and the 「至少1个」 floor on the ring count.
* **The agent's 「同色」** is not defined beyond 「每种颜色拥有一个地产商格子」.
  The board's `group` is what 「颜色」 means here. TODO(规则书): whether
  `extraColor` / 「该格获得所有颜色」 widens 「同色」 for the agent.
* **CiRCLE reward when stunned** forces the card half. TODO(规则书): the book
  says 「晕眩…无法收付款」 but never says the reward becomes card-only.
* **Mortgaged-owner force-buy** 「不受任何资金变动效果影响，收款方无论处于
  何种状态都可正常收款」 is stated for the forced purchase only. TODO(规则书):
  whether the ordinary rent path's pay-hooks may target it (today they cannot
  -- `offer_force_buy` moves money directly).

## The crate: `rules/tiles`

One crate, one `.rs` per tile kind, one `CardDef` each -- the same authoring
shape as `rules/cards/card-*` and `rules/skills/*`:

| id | file | board kinds | rulebook |
|---|---|---|---|
| `tile:property` | `property.rs` | `property` | lines 100–106 |
| `tile:ring` | `ring.rs` | `ring` | line 102 (「所有RiNG不可升级」) + TODO |
| `tile:agent` | `agent.rs` | `agent` | line 107 |
| `tile:circle` | `circle.rs` | `circle` | lines 95–96 |
| `tile:edogawa` | `edogawa.rs` | `edogawa` | line 95 |
| `tile:event` | `event.rs` | `cafe`, `ryuseido` | lines 97–99 |
| `mark:cp` | `cp.rs` | (none -- board-wide) | line 125 (「CP点：放置于路面上的指示物」) + 通用:该清CP了 |

The crate is a **rule crate**, not a card crate: its ids are `tile:*`, not
`data/cards.json` ids. `tools/rules-aggregate.mjs` gains `rules/tiles` as a
third scan root (besides `rules/cards` and `rules/skills`) so `card-all` links
it; `tools/build-ruleset.mjs` needs no change.

Each body quotes the passage above and cites it per line, exactly like a card
(`docs/CARDS.md` → 「Every line cites the rule book」). `python
tools/rulebook/check.py` is extended to require the quote on `rules/tiles/*`
the way it does on `rules/cards/*` -- the passage lives in the file header
since `docs/rulebook/cards.json` has no `tile:*` ids.

### Board marks: `mark:cp`

`mark:*` is a second shape of board-owned rule: a **mark owner** rather than a
tile-kind rule. There is one instance per mark category on the neutral board
owner, governing **no single tile** (`tile = -1`), placed at match start next to
the `tile:*` ones (`bind_tiles`, `game_core::data::mark_rule_ids`). Like an
event instance, it hears every trigger its rule declares wherever the trigger
points -- the hook dispatch adds `mark:*` instances to the board list for a
tile-carrying trigger too.

`mark:cp` owns the **tile-mark** half of the [CP点] lifecycle (「CP点：放置于
路面上的指示物」, `data/rules.txt` 125). There are **two kinds of [CP点]**
(user ruling 2026-10-07: 「自己[场上]1个[CP点] referred to the cp point
attached to the card. There are points on the tile (which mandated by
tilemark) and points on the card (mandated by the card rule)」):

* **Tile [CP点]** -- the [`TileMark`]s of category `mark_category::CP`, owned
  by this rule instance. That is the `data/rules.txt` 125 object.
* **On-card [CP点]** -- `FieldCard::cp` on the 该清CP了 card instance, the card
  rule's own stock (crystals-like; 通用:该清CP了 [手] 「在自己[场上]添加6个
  [CP点]」 seeds it at 6). The view shows it as the field card's counter badge.

| step | clause | API |
|---|---|---|
| placement | 通用:该清CP了 [手] 「在任意一个没有角色和[CP点]的格子上添加1个[CP点]」 | `ctx::place_cp(tile)` -- the *where* is the placer's gate |
| stacking | 「添加1个[CP点]」 onto a 「没有[CP点]的格子」 | one mark object per tile, `count` is the [CP点] there |
| landing | 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金」 | `On::Hook(SettleAfter)` on this instance -- spends **both** kinds |
| removal | 「移除格子上的个[CP点]」 | `ctx::clear_cp(tile)` (a tile-mark write) |
| on-card seed | 「并在自己[场上]添加6个[CP点]」 | the card rule's `ctx::add_cp(6, 0)` -- not this owner |
| on-card spend | 「自己[场上]1个[CP点]」 | `ctx::add_cp_at(src, -1, 0)`, `src` = the mark's `TileMark.src` |

Cards reach the tile marks **only** through that small API (`ctx::place_cp` /
`count_cp` / `count_cp_from` / `clear_cp` / `cp_src_at`), never through the
generic mark ops, and the on-card count through `ctx::cp_attached` /
`add_cp` / `cp_at` / `add_cp_at`. A [CP点] mark is **not owned by any player**:
`TileMark.owner` is always `BOARD_OWNER` (`-1`). Provenance is
`TileMark.src` (the placing card instance, what 「此卡在格子上添加的[CP点]及其
产物」 keys on) plus `TileMark.card` (its id, the view's 「来自」).

`TileMark.category` is what says which category a mark is (`""` = a
player/generic mark coloured by `owner`'s seat; `mark_category::CP` for [CP点]).
Both `category` and `src` are serde-defaulted, so a pre-category save still
loads -- `Match::restore` re-reads the old CP `kind`
(`cards:card-general.clear_cp_mark`) into the category and drops its player
owner. The old per-player 「自己[场上]」 counter `mark::CP_FIELD_TOK` is gone:
the card's own `FieldCard::cp` replaces it (ABI v38).

**The settle clause's reading** (recorded; see `rules/tiles/src/cp.rs` for the
long form). 「在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个
[CP点]，[获得]800资金」: any player's [结算] on a tile carrying a [CP点] removes
the tile's mark **and** one on-card [CP点] of the 该清CP了 card the mark is
attached to (`TileMark.src`, which must hold ≥1), and the **settler** gains
800 -- the subject of 「[结算]」 carries over to 「[获得]」 (the rulebook tip
「吃多个CP点达到2000以上收益」 has the eater profit). C# instead kept a
per-player `Tok(Seat, "CP点")` and gated the clause on `m.Seat == Seat`; the
2026-10-07 ruling replaces the counter with the card's own count and this
reading drops the gate. The 该清CP了 graveyard rule is the on-card count
hitting 0 (`HookKind::CpChanged`), not the tile marks running out.

### The bodies stay thin

The engine keeps the money/deck work as **`ctx` primitives**, so a tile body is
a list of citations, not a reimplementation:

| primitive | wraps today | used by |
|---|---|---|
| `ctx::draw` / `ctx::draw_event` | `draw_r` / `draw_event` | circle, edogawa, event |
| `ctx::offer_buy` / `ctx::offer_build` / `ctx::offer_force_buy` | `offer_*` | property, ring |
| `ctx::pay_rent(t, half)` | `pay_rent` (incl. the RiNG formula and the agent's 「半价收费」 half-charge) | property, ring, agent |
| `ctx::buy` / `ctx::buy_quotes` / `ctx::card_build` | the `BuyKind::Agent` purchase / build | agent |
| `ctx::ask_tiles` | a tile ask with per-option labels, prices, and an AI choice hint | agent |
| `ctx::ai_agent_choice` | `ai_agent_choice` (the ask's AI hint) | agent |
| `ctx::is_color` / `ctx::tile_group` / `ctx::is_buyable` | the 同色 set (colour overrides included) | agent |
| `ctx::raise` | a guest-raised trigger point (`circleAffected`) | circle |
| `ctx::gain_typed` | a bank print with the Pay's `typ` + log line | circle |
| `ctx::prop_at` / `ctx::set_prop_at` | instance props by uid (`NO_REWARD` on a field instance) | circle |
| `ctx::exile_of` / `ctx::stun_of` | the player's `[除外]` / `[晕眩]` layers | circle |

These are **not** new side channels: each is a named verb the engine already
runs, published so a rule body can call it. They raise the same `buy*` /
`build*` / `pay` / `circleAffected` triggers they raise today.

`tile:property`'s body, in full, is the rulebook's four-way branch (unowned /
own / other / mortgaged) over those primitives. `tile:ring` is `tile:property`
plus the dice-rent term. `tile:edogawa` is one draw. `tile:agent` is the
「若…则…否则…」 branch (the 同色 set, the all-owned half-rent sweep, and one
buy/build offer). `tile:circle` is the landing draw plus the [经过] reward
(`circle::settle_reward`, shared with `event:协助CiRCLE重建`'s cafe re-home).
That is the point: once the body is a rule instance, a card can swap it, stack
another one next to it, or rewrite its props -- without the engine knowing the
card's name.

## Binding

At match start, **every board tile gets rule instances on a neutral board
owner**, mirroring `bind_skills`:

* **Board owner = `-1`** (`state::BOARD_OWNER`). Not a seat: turn order,
  scoring, `player_count` and the alive/out loops never see it.
* **Storage** is a new `World::board_field: Vec<FieldCard>` -- the same
  `FieldCard` a player's field holds. Each instance carries
  `owner = user = -1`, `tile = <board tile index>`, and its `props`.
* **`bind_tiles(data, rules)`** runs once from `setup` / `Match::new` (next to
  `bind_skills`) and is idempotent. It places **one instance per tile** of the
  kind's def: `tile:property` on a `property` tile, `tile:ring` on a `ring`
  tile, and so on. `cafe` and `ryuseido` both get `tile:event`.
* **`FieldCard.uid`** is what identifies the instance (same as skills): a card
  that attaches a second `tile:circle` to a tile adds an instance; it does not
  rename the first.

### Tile data rides the property map

`TileData` (price, rent table, group, …) is stamped into the instance's
`props` at bind time -- `CardDef::props` for what the *rule* declares,
`FieldCard.props` for the per-tile values. The keys are `card_sdk::abi::prop`
constants, mirrored in `game_core::state::prop`:

| key | source | notes |
|---|---|---|
| `price` | `TileData.price` | land price (houses are extra) |
| `house` | `TileData.house` | build cost per level |
| `group` | `TileData.group` | colour group; `prop::ALL_COLORS = -2` is 「该格获得所有颜色」 |
| `buildMax` | `rent.len() - 1` | 「每块地有标注的等级上限」; 0 = 「所有RiNG不可升级」 |
| `rentLen` | `rent.len()` | |
| `rent:0` … `rent:N` | `TileData.rent` | the rent table, one key per level |
| `ringMult` | `match_rules.ring_multiplier` | ring only; TODO(规则书) |

Defined default `0` for every key, same as `CardDef::props`. A key a tile does
not have reads as `0`.

The rent table as `rent:0`…`rent:N` is deliberate: props are `key -> i32`, and
a table is just N of them. `buildMax` is derived, not stored twice -- the body
reads `rentLen - 1` -- but is declared so a card can raise the cap (「升级
上限+1」) without touching the table.

## Settlement

`settle_at` becomes a **two-phase link**, and the tile's rule instances *are*
the second phase. This replaces `land_at`'s `match` **and** `HookKind::SettleInstead`.

```
settleBefore                          (unchanged)
  ↓
settle            -- declaration + [反击] window (unchanged).
  |                 A counteraction that negates this link means the settle
  |                 never happened: no body, no settleAfter.
  ↓
settleBody        -- the settle body, a chain link.
  |  (a) field hooks on the tile, player-owned: a card placed on this tile
  |      may `set_cancelled()` to *replace* the body -- today's
  |      `settleInstead` gesture. `settleAfter` still runs.
  |  (b) if not cancelled: the tile's board-owned rule instances settle, in
  |      instance order. Each is an effect link, so [反击]s to it and field
  |      hooks apply as they would to any effect.
  ↓
settleAfter                           (unchanged)
```

Why two phases inside one point: the rulebook's 「执行格子上的**所有**效果」
says the body is a *list*, and 「技能或卡的效果优先」 says a card can override
one entry. Cancelling the `settle` link (「the settle never happened」) and
replacing the body (「it settled, differently」) are two different things and
stay two different gestures -- the same distinction `Negation::{Activation,
Effect}` draws for [反击].

**`HookKind::SettleInstead` is removed.** Its three users (Parking Space,
笑容大游行, Hey Kids) move to the `settleBody` replace gesture. Wire name
`settleBody`; ABI bump.

### What stays an engine primitive

Buying and building stay **player actions in the end step**, not part of the
settle body. The tile rule declares *whether they are allowed and the price*
through its props (`price`, `house`, `buildMax`, `noBuild`, `buyDiscount`,
`freeBuy`, `razeOnBuy`); `buyable_here` / `can_build_here` / `why_not_build_on`
read those props instead of `TileData` + `TurnCtx` flags. The settle body calls
`settle_buy` / `settle_build` to *offer* them at landing (the rulebook's
「可选择」); the end step offers them again after a main move.

Mortgage / redeem / forced purchase money moves stay outside all effects
unless a rule says otherwise (rulebook: 「抵押，赎回，和强制购买的资金变动不
受任何效果影响」), which is why `offer_force_buy` remains a primitive that
moves money directly.

## Cards become rule operations

A card that bends a tile now **attaches, swaps or retunes rule instances**
instead of poking an engine flag. The board owner is a normal `FieldCard`
list, so `place_card` / `unplace_card` / `set_card_tile` / props work on it
with `player_id = -1`.

| card | today | new |
|---|---|---|
| 黑衣人的补给 | ~~a 「黑衣人的补给」 tile mark + `CircleLike` special case~~ **done** | **attaches a `tile:circle` instance to 弦卷集团** while it carries the crystal (`place_card_on(BOARD_OWNER, tile, "tile:circle")`); passing/landing runs it alongside `tile:agent`. 「获得CiRCLE格子的全部效果」 is additive. No card hook draws. |
| 笑容大游行 | `settleInstead` + tile-position rewrite | **swaps the two tiles' instances** (「那格视为与"弦卷集团"格子交换位置」); the move's endpoint borrows 弦卷集团's `tile:agent` instance for that one settle |
| （kkr）前往笑容集结的地方！ | ~~a `settleAfter` hook on a field stand-in~~ moved to `settleBody` | 「视为格子的收款」 is an entry in the tile's settle effect list (`SETTLE-STAGES.md` §4 M3), skipped when a field card replaces the body. Long-term home: a collect rule instance on CiRCLE. |
| Parking Space | `HookKind::SettleInstead` | **replaces the body in the chain** (a `settleBody` instance on the Space tile) |
| Hey Kids | `HookKind::SettleInstead` | same |
| （soyo）色彩 | `ctx::set_tile_color` / `ALL_COLORS` | **sets the tile instance's `group` prop** (`ALL_COLORS`) |
| Roselia band (1) | `set_extra_color` | sets `group` on the tile instance |
| 朝日六花 | `set_extra_color` | sets `group` on the tile instance |
| Anon Tokyo | tile marks + `anon_tokyo_link_<tile>` slots + a field stand-in for `PayAdd` | **attaches a link rule instance to each tile**, partner in its props (「被[奇迹水晶]连接的格子收费时…」 is that instance's pay entry) |
| 练习室里的风暴 | `place_card_on` + slot + `rent_factor` | **attaches a remote-settle rule instance to its tile**; the (4-X)/4 scale is that instance's prop |
| Change the world | `place_card_on` + `change_world_turn` slot + `pay_factor` | **attaches a surcharge instance to its tile**; the 50×(y+1)×n term is its props |

The `rb_cross_tiles.rs` cases this un-ignores (migration legitimately fixes
them -- the assertions stay as written): `t01_anon_tokyo_link`,
`t02_anon_link_plus_fire_bird`, `t03_anon_link_plus_repaint`,
`t05_one_of_us_shares`. `t04` and `t11` stay `#[ignore]` (they are `RULING:`,
not implementation gaps).

### Engine flags → rule operations

Every flag in the table at the top leaves the engine. The home for each:

| flag | home after |
|---|---|
| `no_circle_reward` (plan) | prop `noReward` on the tile's `tile:circle` instance; `tile:circle`'s Pass entry reads it (and consumes the tile-side arm). A rule that suppresses the reward (PPP band (2), PP band (3), 凑友希那 (1), detour, 赤音, tsugumi) **sets the prop** on the instance, and clears it when its own clause ends (「…时」 goes in an event handler on that state's change). |
| `NO_CIRCLE_REWARD` (state) | same. The "held as data on the source" idiom moves to "held as a prop on the tile" -- the source still owns the arming/disarming, but the reader is the tile instance, not a per-player latch. |
| `rent_factor` | prop `rentFactor` (milli) on the tile instance; `settle_rent` applies it |
| `pay_factor` | prop `payFactor` (milli) on the tile instance; `settle_rent` applies it. (Today both are `MoveCtx.plan` scalars read only in `pay_rent`.) |
| ~~`buy_discount`~~ | **done** -- a `ctx::linger` instance whose def hooks `BuyAdd` (tsugu_ycm −1500, roselia band (1) is `BuyMul` ½ instead). The hook rewrites the running quote price, so it covers any tile the player buys. |
| ~~`free_buy`~~ | **done** -- the same instance hooks `BuySet` 0 (maze_warehouse) |
| ~~`raze_on_buy`~~ | **done** -- the same instance hooks `BuyAssign` and rewrites `deal_houses` to 0 |
| ~~`extraColor:` / `st.tile_colors`~~ | **done** -- `prop::ANY_COLOR` (tile-wide 「该格获得所有颜色」) and `colorFor:<p>` (per player) on the tile's rule instance |
| `noBuild` | prop `noBuild` on the tile instance; `why_not_build_on` reads it |
| `settleAtEnd` | a scheduled turn-end rule op (`On::AtEnd`) whose body calls `ctx::settle` -- 「并在回合结束时触发结算」 is a scheduling clause, not a tile fact (rinne_rain, tomori_crychic) |
| `settleInstead` | **done** -- renamed `HookKind::SettleBody`, the body-replace gesture on the tile's rule instances |

`buy_price` / `build_cost` / `mortgage_value` / `force_buy_price` / the ring
rent table all become **reads of the instance's props**, so a card that retunes
a price retunes the instance -- the same surface the end-step actions use.

## Buying and building

Routed through the purchase surface (`docs/PURCHASE.md`, `engine/play/purchase.rs`):

* **allowed?** `BuyGate` (every `BuyKind`, Force included) + the tile prop
  `BUYABLE` (「可购买格子」, stamped at bind time from `TileData::is_buyable`).
* **price?** `CardRules::buy_quote` → `base_quote` (land + houses; 2× for
  `Force`), then the stages `BuyAdd` → `BuyMul` → `BuySet` (each floored at 0).
  `WasmRules` runs those hooks in pure guard mode against a world copy, only
  when a hooking instance exists, and the commit re-quotes so quote == charge.
* **deal?** `assign_deed` (owner / houses / mortgage in one write);
  `BuyAssign` hooks rewrite before commit.
* **preview?** `st.buy_price` / `st.build_cost` at END, `MatchPrompt.price` /
  `prices[]` on the prompts.

The rulebook's 「可选择」 offers stay prompts in `settle_buy` / `settle_build`;
the end step re-offers after a main move. 「非写明可选择的效果在可发动时必须
发动」 does not apply here -- these clauses say 可选择.

## Perf

**Budget: ≤ 25% ms/game** on the bot-game benchmark, and ≤ 25% on the per-run
fire-up cost. If wasm per-landing cost exceeds it, evaluate native execution of
the built-in tile rules (the default `CardRules::settle_tile` impl already runs
the bodies as plain Rust for `StubRules`; promoting that to the general native
path is the fallback, `rules/card-sdk/src/native.rs`'s arena is the ABI surface
it would use).

### Before (2026-10-06, this machine)

| measure | value |
|---|---|
| `cargo test -p game-rules --test ruleset -- --nocapture fire_up`, one real run | **703.6 µs** ×100 |
| same, `fire_up_breakdown` full run | **619.6 µs**/call |
| `cargo run -p game-core --release --example sim -- 50 4 200` | **278.5 ms/game** (13.92 s / 50 games, avg 196.4 rounds) |

The sim runs `StubRules`, so it prices the **engine shell** (walk, rent, buy,
build, auction, bot prompts) and not the wasm ruleset. It is the right
regression gauge for the engine-side cost of the new dispatch (instance lookup,
prop reads, the extra `settle_body` raise). The wasm-side cost of actually
running a tile body is the fire-up number × landings; the two together are the
budget.

Allowed regression: **≤ 348 ms/game** (278.5 × 1.25) and **≤ 880 µs** per
fire-up run.

### After (Phase 2 complete, 2026-10-06)

| measure | before | after | delta |
|---|---|---|---|
| `fire_up`, one real run | 703.6 µs | **675.0 µs** | −4% |
| `fire_up_breakdown` full run | 619.6 µs | **444.6 µs** | −28% |
| module size | 328 KiB | 332 KiB | +4 KiB (six tile bodies) |
| `sim -- 50 4 200` | 278.5 ms/game | **258.7 ms/game** | **−7%** |

Both inside the budget (≤ 348 ms/game, ≤ 880 µs). The sim runs `StubRules`, so
it prices the engine-side dispatch (`settle_tile`'s instance lookup and the
fallback to `land_at_built_in`) and not the wasm bodies -- but its event-kind
counts are **identical** to baseline (rent 19,612 / build 4,641 / buy 2,908 /
forcebuy 415 / agent half rent 4,221 / circle money 5,915), so the migration is
behaviour-preserving on the shell. The wasm side is the fire-up number: one
instantiation per tile body, the same cost a field hook already pays.

## ABI and SAVE_VERSION

Checked: `card_sdk::abi::ABI_VERSION = 40`,
`game_core::engine::mod::SAVE_VERSION = 4` (the match save),
`game_core::profile::SAVE_VERSION = 3` (the player profile -- untouched here).

| bump | when | why |
|---|---|---|
| **ABI → 31** | Phase 2 first step | `OnKind::Settle` (the settle body), `prop` keys for tile data, `ctx::settle_*` primitives, `place_card` on `BOARD_OWNER` |
| **SAVE_VERSION → 3** | Phase 2 first step | `World::board_field` (tile rule instances) is part of the world; a v2 save has none and would restore a match with unbound tiles |
| **ABI → 32** | Phase 3 (engine-fix batch) | `tile_prop` / `set_tile_prop` (write a rule instance's props by tile), `settle_circle_reward`, `ChainKind::SettleBody` (shares `TriggerKind::SettleBody`'s wire value), board-owned instances hearing field hooks. The `MoveCtx` plan is not part of the save, so SAVE_VERSION stays at 3. |
| **ABI → 50** | agent/circle body migration (2026-10-10) | the tile bodies run the branch themselves. Added: `ask_tiles` (labels + prices + AI hint), `ai_agent_choice`, `raise` (guest-raised `circleAffected`), `gain_typed` (bank print with `typ` + `Pay::text`), `prop_at` / `set_prop_at`, `exile_of`, `trigger::move_from`. **Removed**: `agent_landing` / `settle_circle_reward` and their `HostRequest`s -- the engine keeps `Play::agent_landing` / `Play::circle_reward` as the `StubRules` built-in only. SAVE_VERSION unchanged. |

## Migration

One kind at a time, in the plan's order. After **each** step:

```sh
node tools/build-ruleset.mjs
cargo test -p game-core
cargo test -p game-rules --no-fail-fast
cargo test -p game-rules --test fuzz_interactions
python tools/i18n/check.py        # 4 pre-existing problems
python tools/rulebook/check.py
```

Baseline (as of the Phase 2 runs, 2026-10-06) is **644+ passed / 0 failed**;
the live counts are in [TEST-FINDINGS.md](rulebook/TEST-FINDINGS.md) §Status
and move with the concurrent batches. Pass and ignore counts must be
unchanged across a migration step, except tests a migration legitimately
fixes -- those come un-ignored and nothing else. **Never weaken a test
assertion.** If a test encoded engine-flag internals rather than behaviour,
report it.

| step | kind | what moves | notes |
|---|---|---|---|
| 1 | `tile:edogawa` | one `settle_draw` | **done** -- smallest body; proves bind + dispatch |
| 2 | `tile:event` | `settle_draw` + `settle_event` | **done** -- added the `ctx::draw_event` primitive |
| 3 | `tile:circle` | the landing draw + the [经过] reward | **done** -- the landing draw is `On::Settle`; the reward is the instance's **Pass entry** (`On::Hook(&[HookKind::PassTile])` -> `circle::settle_reward`, the body itself). The walk's `circle_reward` is the **built-in fallback** when no instance is bound |
| 4 | `tile:agent` | the 「若…则…否则…」 branch | **done** -- the body is the branch (同色 set / all-owned half-rent / one buy-build offer). `Play::agent_landing` is the **built-in fallback** |
| 5 | `tile:ring` | `tile:property` + dice rent | **done** -- the `rent_factor` / `pay_factor` *read* moved to the pipeline's `payMul` stage (step 9) |
| 6 | `tile:property` | the four-way branch | **done** -- `land_at`'s `match` is now the **built-in fallback** (`land_at_built_in`) for `StubRules` and unbound kinds, not the only path |
| 7 | flags | card migrations | **done** -- `settleInstead` → `HookKind::SettleBody`, `settleAtEnd` → `On::AtEnd`, the CiRCLE veto → `prop::NO_REWARD`, `noBuild` → `prop::NO_BUILD` (placed) or a linger instance (hand card), `buy_*` → `ctx::linger` + `BuyAdd`/`BuyMul`/`BuySet`/`BuyAssign`, `extraColor` → `colorFor:<p>` / `prop::ANY_COLOR` |
| 8 | docs + perf | ENGINE.md, CARDS.md, this doc | **done** |
| 9 | Phase 3 (engine-fix batch) | the flag removal + card migrations | **mostly done** -- see [Left short](#left-short) |

Fallback at any step: the default `CardRules::settle_tile` keeps today's
`land_at` body for `StubRules` and for a kind with no instance, so a step that
breaks can be reverted by un-binding one kind without touching the others.

## Left short

Phase 2 is complete: all six tile kinds settle through rule instances on the
board owner, and the engine's `land_at` is now the **built-in fallback** rather
than the only path. Phase 3 landed the following (2026-10-06):

* **The [经过] CiRCLE reward** is `tile:circle`'s **Pass entry** --
  `On::Hook(&[HookKind::PassTile])` running `circle::settle_reward` (the body
  itself: suppression, the choice, the `circleAffected` window, the payout).
  `event:协助CiRCLE重建` re-homes the same function onto 「CiRCLE咖啡厅」.
  The hook dispatch visits `board_field` (tile-filtered, and only for kinds a
  `tile:*` rule declares), so board-owned instances hear `passTile` and the
  other hooks they declare. Suppression is `prop::NO_REWARD`; `plan::no_circle_reward`
  and `state::key::NO_CIRCLE_REWARD` are gone. The walk's `circle_reward` is
  the **built-in fallback** when no rule instance is bound (`StubRules`), the
  same shape as `settle_tile` -> `land_at_built_in`.
* **The settle body is its own [反击]-able chain link.** `ChainKind::SettleBody`
  shares `TriggerKind::SettleBody`'s wire value, and `is_hook_only` no longer
  lists it, so a hand card may [反击] the body. Cancelling `Settle` means 「the
  settle never happened」 (no body, no `settleAfter`); cancelling `SettleBody`
  means 「it settled, differently」 (the body is skipped, `settleAfter` runs).
  Hey Kids moved to `ChainKind::SettleBody` and to the user ruling (2026-10-06):
  it fires when **another** player settles rent on my tiles. Parking Space and
  笑容大游行 keep `HookKind::SettleBody` (the body-replace gesture).
* **`settleAtEnd` is gone** -- 「并在回合结束时额外进行一次[触发结算]」 is a
  scheduling clause: `ctx::before_turn_end` + `On::AtEnd` whose body calls
  `ctx::settle`.
* **「支付减半」** rides the money pipeline's `payMul` stage
  (`scale_settle_payment`), not `pay_rent`, so the scale covers the payment as
  card effects shape it (a rent-region expansion, a forced stop-and-pay).
* **`tools/rulebook/check.py`** now checks `rules/tiles/*` against
  `data/rules.txt` the way it checks `rules/cards/*` against `cards.json`.

Concretely still short:

* ~~**`buy_discount` / `free_buy` / `raze_on_buy` are still `TurnCtx` fields.**~~
  **Done.** The three flags and their setters are gone (ABI 41). A turn-scoped
  「本回合购买格子时[消耗]资金时降低1500」 is a `ctx::linger` instance whose def
  carries a `BuyAdd` hook (`docs/PURCHASE.md`) -- it applies to *any* tile the
  player buys, which is exactly why it cannot live on one tile instance. Same
  for 「不[消耗]资金」 (`BuySet` 0) and 「拆除那个格子上的所有房屋」
  (`BuyAssign` houses 0).
* ~~**`extraColor:` is still per-player state.**~~ **Done.** The clause split
  is now the two props: （soyo） 「该格获得所有颜色」 is `prop::ANY_COLOR` on the
  tile (tile-wide), while 朝日六花 / Roselia band (1) / 游击演出 / soyo_clear are
  per-player `colorFor:<p>`. `is_color` / `is_live_house_for` read both, the
  same way `agent_colour_set` does. TODO(规则书) ruling 5: whether 「所有颜色」
  and the per-player 「视为live house」 widen the agent's 同色 set is unchanged
  and still open.
* ~~**`noBuild` is half-migrated.**~~ **Done.** `why_not_build_on` reads
  `prop::NO_BUILD` on the player's field instances, on the tile's rule
  instance, and on a **lingering** instance (`TurnCtx.lingering`) -- the
  hand-card home 学生会的检查 uses. The per-player `noBuild` scratch key is gone.
* **`rent_factor` / `pay_factor` still arm through `plan::set_*_factor`.** The
  *read* moved into the `payMul` stage, but a hand card (祥，移动, Repaint,
  （立希）…) leaves the field and cannot leave a hook behind, so the move plan
  remains the arming surface for a settle-scoped scale. The tile-prop home
  (`prop::RENT_FACTOR` / `PAY_FACTOR`) is read in the same stage for a card
  that retunes a tile. TODO(规则书): whether a settle-scoped scale should be a
  placed marker carrying a `payMul` hook instead.
* **`rb_cross_tiles.rs`'s `t01`–`t05` and `t11` stay `#[ignore]`d** -- they are
  the card-side gaps (Anon Tokyo's link, ONE OF US, the Fire bird ordering
  ruling) that the card migrations of step 7 still owe.

### 「支付减半」 wording comparison (ruling 2026-10-06)

「触发结算时进行的支付价格减半」 covers the settlement payment **as card
effects shape it**. The cards whose sheet wording matches that reading and now
ride the `payMul` stage: **祥，移动**, **（祥子）带领着大家** (sakiko_lead),
**（乐奈）有趣的女人** (rana_funny), **（立希）想认真去做** (taki_serious, ¼
rather than ½), **Repaint**, **八幡海铃:熟练的支援贝斯手** (kaede_support).

Wording that differs, left as it is with `TODO(规则书)`:

* **（香澄）大家我都喜欢哦** -- 「那名玩家此次[结算]如果[支付]地租则地租只算作
  原本的一半」. Only *rent*, not every payment: `rent_factor`-shaped.
* **练习室里的风暴** -- 「此次[结算]的地租为普通[结算]的(4-X)/4倍」. Only
  *rent*, and a per-settlement scale: `rent_factor`-shaped.
* **学生会的检查** -- 「因此卡强制停下的玩家的结算地租价格为原价格一半」.
  Only *rent*: `rent_factor`-shaped.

## Per-step results

Counts are `cargo test` `passed / failed / ignored`. The baseline for these
runs was the combined `cargo test -p game-core -p game-rules` run:
**696 passed / 0 failed / 85 ignored** (as of the Phase 2 steps, 2026-10-06
-- the tree has grown since; see
[TEST-FINDINGS.md](rulebook/TEST-FINDINGS.md) §Status). Per-step runs
exclude the concurrent agent's in-flight `rb_gap_*.rs` targets (one of which
does not compile against `CardRules`); the rest of the suite is the gate and
is unchanged at every step.

| step | build-ruleset | game-core | game-rules | fuzz | i18n | rulebook | notes |
|---|---|---|---|---|---|---|---|
| baseline | ok | 44 / 0 / 1 | 648 / 0 / 84 (excl. rb_gap) | 8 / 0 / 1 | 4 pre-existing | 0 | fire_up 703.6 µs; sim 278.5 ms/game |
| 1 edogawa | ok (251 rules) | 44 / 0 / 1 | 648 / 0 / 84 | 8 / 0 / 1 | 4 pre-existing | 0 | `On::Settle` + `bind_tiles` + `settle_tile`; ABI v31, SAVE_VERSION 3 |
| 2 event | ok (252) | 44 / 0 / 1 | 648 / 0 / 84 | 8 / 0 / 1 | 4 pre-existing | 0 | `ctx::draw_event` primitive added |
| 3 circle | ok (254) | 44 / 0 / 1 | 648 / 0 / 84 | 8 / 0 / 1 | 4 pre-existing | 0 | landing draw only; the [经过] reward stays on the walk |
| 4 agent | (with 3) | (with 3) | (with 3) | (with 3) | (with 3) | (with 3) | the 「若…则…否则…」 body; gated in the same run as 3 |
| 5 ring | ok (256) | 44 / 0 / 1 | 648 / 0 / 84 | 8 / 0 / 1 | 4 pre-existing | 0 | `ctx::pay_rent` / `offer_buy` / `offer_force_buy` / `offer_build` added |
| 6 property | (with 5) | (with 5) | (with 5) | (with 5) | (with 5) | (with 5) | the four-way branch; gated in the same run as 5 |
| 7 flags | | | | | | | **partial** -- see [Left short](#left-short) |
| 8 docs+perf | ok | — | — | — | 4 pre-existing | 0 | fire_up 675.0 µs; sim 258.7 ms/game |
| 9 Phase 3 (engine-fix batch) | ok (256) | 44 / 0 / 1 | green, intentional ignores only | 9 / 0 / 0 | 0 | 0 | ABI v32; sim **262.0 ms/game** (baseline 262.0), event counts identical (rent 19,612 / build 4,641 / buy 2,908 / forcebuy 415 / circle money 5,915 / agent half rent 4,221) |

Step 9 removed `#[ignore]` from nine ruling tests the migration fixed:
`rb_ras`'s eight `hey_kids_*` (the settle-body chain link + the user ruling
that it fires on **another** player's rent settle) and
`rb_mujica`'s `saki_move_halves_settlement_payments` (the `payMul`-stage
halving). No assertion was weakened. `rb_mujica`'s other two 「支付减半」
ruling tests stay `#[ignore]`d: `saki_move_halves_a_door_surcharge_rent` has a
setup that cannot reach the hill it names (a 3-tile move from tile 12), and
`saki_move_halves_a_forced_stop_and_pay` expects 140 where both 祥，移动 and
（香澄）大家我都喜欢哦 halve (280 -> 70) -- both are the concurrent test
agent's work in progress, not this migration's.

## Conventions

* The rulebook decides *what*; the C# port is not a spec.
* No raw money pokes, no side-channel flags, no prose-keyed behaviour.
* 「…时」 clauses go in event handlers on that state's change, not inline
  checks at each spend site.
* Anything left short of the text gets a `TODO(规则书)（N）` naming the gap.
* A fix that makes an ignored test pass removes its `#[ignore]`. Never weaken
  assertions. A test that asserted an engine flag rather than behaviour gets
  reported, not silently rewritten.
* Commit nothing.