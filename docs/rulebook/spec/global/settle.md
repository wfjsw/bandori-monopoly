# G-SETTLE -- 基础[结算]

Unit 5 owns this file and `rules/spec/src/global/settle.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Scope: 「基础[结算]规则」 (`rulebook-doc.md:148-162`) for every tile kind in
`data/board.json` (60 tiles: property 41, agent/[地产商] 11, ring 4, circle 1,
cafe 1, edogawa 1, ryuseido 1). The book's item numbering skips **4**. Stage
names from `docs/rulebook/SETTLE-STAGES.md` (settleBefore / settle / settleBody
/ settleAfter) are **observability only** -- the specs come from the book.

---

## G-SETTLE-01

* **ID:** `G-SETTLE-01`
* **Source:** `rulebook-doc.md:158` (owner) / `:160`-`:161` (other)
* **Quote:** 「如果格子地契已抵押则无效果。」 (owner-landed). The other-owned
  branch at `:161` is the complement: a mortgaged deed charges **no rent**
  (「如果格子地契未抵押则**[支付]…现等级地租**」 is the only rent case).
* **Trigger:** an observed input whose event window contains a `rent` leg
  attributed to a landed settle on tile `T`.
* **Condition:** `T` is a player-owned buyable tile that is mortgaged in `pre`.
* **Effect:** no `rent` event for `T` in this window.
* **Frame:** n/a (read of the event window).
* **Rulings & OPEN:** the force-buy branch of a mortgaged tile is `G-SETTLE-11`,
  not rent. An event's tile is the `to` field on the money line when present.
* **Executable:** `rules_spec::global::settle::g_settle_01`.

---

## G-SETTLE-02

* **ID:** `G-SETTLE-02`
* **Source:** `rulebook-doc.md:150`
* **Quote:** 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。」
* **Trigger:** an observed input whose window logs a landing on a `circle` or
  `edogawa` tile (`log.land_start` / `log.land_draw`).
* **Condition:** the window has no card activation (a card can replace the
  body -- L6 「技能和卡的效果优先」).
* **Effect:** exactly one hand card is drawn (`draw` event `value == 1`); no
  event card is drawn.
* **Frame:** the event half of cafe / 流星堂 does **not** run.
* **Rulings & OPEN:** the [经过] CiRCLE reward (`rulebook-doc.md:151`) is a
  pass effect, not this settle -- owned by G-MOVE. OPEN-结算定义.
* **Executable:** `g_settle_02`. Scenario: `g_settle_02_circle_and_edogawa_draw_one`.

---

## G-SETTLE-03

* **ID:** `G-SETTLE-03`
* **Source:** `rulebook-doc.md:152`
* **Quote:** 「CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。」
* **Trigger:** an observed input whose window logs a landing on a `cafe` or
  `ryuseido` tile (`log.land_event`).
* **Condition:** the window has no card activation.
* **Effect:** exactly one hand card is drawn and exactly one event is drawn,
  in that order (hand draw precedes the event draw).
* **Frame:** see G-SETTLE-04 / G-SETTLE-05.
* **Rulings & OPEN:** OPEN-结算定义.
* **Executable:** `g_settle_03`. Scenario: `g_settle_03_cafe_draws_one_card_then_one_event`.

---

## G-SETTLE-04

* **ID:** `G-SETTLE-04`
* **Source:** `rulebook-doc.md:153`-`:154`
* **Quote:** 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」 /
  「事件结算后进入事件弃卡区。」
* **Trigger:** an observed input whose window draws an `event` card.
* **Condition:** the drawn event is not a derived 「永久移除」 and not left on
  the active list (a permanent event's own text is unit 21).
* **Effect:** the event id is never in the drawer's hand; after the window it
  sits in `event_discard` (or `event_removed` / `event_active`).
* **Frame:** n/a (zone read).
* **Rulings & OPEN:** a permanent event (「将此卡放置于场地中央」) stays in play
  (L6). Publicity is checked as "not in any hand"; the engine has no separate
  face-down event zone.
* **Executable:** `g_settle_04`. Scenario: `g_settle_03_cafe_draws_one_card_then_one_event`.

---

## G-SETTLE-05

* **ID:** `G-SETTLE-05`
* **Source:** `rulebook-doc.md:154`
* **Quote:** 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
* **Trigger:** an observed input that draws an event while `pre`'s event deck
  is empty and the discard is non-empty.
* **Condition:** `pre.event_deck` empty, `pre.event_discard` non-empty.
* **Effect:** the draw succeeds (an `event` is logged).
* **Frame:** n/a.
* **Rulings & OPEN:** 「洗切」 is a shuffle -- order is not specified.
* **Executable:** `g_settle_05`. Scenario: `g_settle_05_empty_event_deck_reshuffles_the_discard`.

---

## G-SETTLE-06

* **ID:** `G-SETTLE-06`
* **Source:** `rulebook-doc.md:155`
* **Quote:** 「无主的[可购买格子]的[结算]是：**可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。**」
* **Trigger:** a `buy` event on a buyable tile that was unowned in `pre`.
* **Condition:** no card activation (BuyAdd / BuyMul / BuySet are L6).
* **Effect:** the buyer's money falls by exactly `price + houses * house_cost`;
  ownership goes to the buyer.
* **Frame:** only that player's money and that tile's owner / mortgage move.
* **Rulings & OPEN:** the **timing** of the offer is part of this clause -- the
  optional buy *is* the settle. See Findings (engine defers main-land buy to
  the end step).
* **Executable:** `g_settle_06`.
  Scenarios: `g_settle_06_unowned_buy_price_includes_the_houses`,
  `g_settle_06_unowned_buy_is_the_settle`.

---

## G-SETTLE-07

* **ID:** `G-SETTLE-07`
* **Source:** `rulebook-doc.md:157`
* **Quote:** 「如果格子地契未抵押则**可选择[消耗]格子地契所标注的房屋建筑费进行升级建造**，每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。」
* **Trigger:** a `build` event on a buyable tile owned by the builder in `pre`.
* **Condition:** the tile was not mortgaged in `pre`; no card activation.
* **Effect:** money falls by the tile's `house` cost; houses rise by 1.
* **Frame:** only that player's money and that tile's houses move.
* **Rulings & OPEN:** level cap is G-SETTLE-08; funding restriction is
  G-SETTLE-09. Timing: same as G-SETTLE-06 -- the optional build *is* the
  settle. See Findings.
* **Executable:** `g_settle_07`.
  Scenarios: `g_settle_07_build_costs_the_marked_house`,
  `g_settle_07_own_build_is_the_settle`.

---

## G-SETTLE-08

* **ID:** `G-SETTLE-08`
* **Source:** `rulebook-doc.md:157`
* **Quote:** 「每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。」
* **Trigger:** any observed input (a `build` event, or a post-state house count).
* **Condition:** `pre` was at or below the cap (arranged states are
  non-canonical).
* **Effect:** after a build, `houses[t] <= level_cap(t)`; a `build` never lands
  on a `ring` tile; 购物中心 caps at 3.
* **Frame:** n/a.
* **Rulings & OPEN:** the cap is read as `rent.len() - 1` (the marked levels on
  the deed's rent table). board.json carries no separate cap field; the book's
  example matches 购物中心's four rent rows.
* **Executable:** `g_settle_08`. Scenario: `g_settle_08_level_caps`.

---

## G-SETTLE-09

* **ID:** `G-SETTLE-09`
* **Source:** `rulebook-doc.md:157`
* **Quote:** 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得。」
* **Trigger:** a build whose funding might include a mortgage of the deed
  being upgraded.
* **Condition:** the builder cannot cover the house cost from cash + other
  mortgageable deeds.
* **Effect:** the build does not happen; mortgaging the deed being upgraded
  does not unlock it (and 5.2 then makes the mortgaged deed's settle inert).
* **Frame:** n/a.
* **Rulings & OPEN:** **scenario-only** -- the per-act window cannot see which
  mortgage funded a build. Not in `CLAUSES`.
* **Executable:** (none per-act). Scenario:
  `g_settle_09_upgrade_cannot_be_funded_by_mortgaging_that_deed`.

---

## G-SETTLE-10

* **ID:** `G-SETTLE-10`
* **Source:** `rulebook-doc.md:160`
* **Quote:** 「如果格子地契未抵押则**[支付]拥有格子的玩家格子地契所标记的现等级地租。**」
* **Trigger:** a `rent` event on a buyable tile owned by someone else.
* **Condition:** the tile was not mortgaged in `pre`; no card activation; not
  a RiNG (OPEN-RING-RENT); not an agent half-charge window -- neither the
  all-owned sweep (`log.agent_all_owned`) nor the choose-one else-branch
  kind=2 commit (`log.agent_offer_commit`).
* **Effect:** the payer pays the owner exactly `rent[h]` where `h` is the
  standing house count (「现等级」).
* **Frame:** only the two money balances move (ledger -- G-MONEY-01).
* **Rulings & OPEN:** OPEN-RING-RENT. Agent half-rate is G-SETTLE-13 /
  the agent else-branch (kind=2). A further L6 「支付减半」 may reshape the
  charge.
* **Executable:** `g_settle_10`.
  Scenarios: `g_settle_10_rent_is_the_marked_current_level`,
  `g_settle_10_frame_only_money_moves`.

---

## G-SETTLE-11

* **ID:** `G-SETTLE-11`
* **Source:** `rulebook-doc.md:161`
* **Quote:** 「如果格子地契已抵押则可选择**[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态**。」
* **Trigger:** a `forcebuy` event on tile `T`.
* **Condition:** `T` was mortgaged and owned by someone else in `pre`.
* **Effect:** the buyer pays exactly `2 * (price + houses * house_cost)`;
  ownership moves to the buyer; `mortgaged[T]` stays true; houses unchanged.
* **Frame:** only the two money balances and the tile's owner move; the
  mortgage flag is unchanged.
* **Rulings & OPEN:** the immunity half is G-SETTLE-12. See also
  其他规则注意事项 6.
* **Executable:** `g_settle_11`. Scenario:
  `g_settle_11_force_buy_is_double_and_stays_mortgaged`.

---

## G-SETTLE-12

* **ID:** `G-SETTLE-12`
* **Source:** `rulebook-doc.md:161` / `:146`
* **Quote:** 「此次购买的价格不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款。」 /
  「抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制购买。」
* **Trigger:** a `forcebuy` event (any window, including ones with card
  activations).
* **Condition:** none beyond the trigger.
* **Effect:** the charge is exactly `2 * (price + houses * house_cost)` even
  under pay multipliers; the payee receives that amount even if stunned.
* **Frame:** n/a.
* **Rulings & OPEN:** a card that *explicitly* rewrites 强制购买 is outside.
* **Executable:** `g_settle_12`. Scenario:
  `g_settle_12_force_buy_ignores_modifiers_and_pays_a_stunned_owner`.

---

## G-SETTLE-13

* **ID:** `G-SETTLE-13`
* **Source:** `rulebook-doc.md:162`
* **Quote:** 「若与该格子同色的所有[可购买格子]均已属于其他玩家则需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10）。」
* **Trigger:** a window that logs `log.agent_all_owned` and then `rent` legs
  on the agent's colour set.
* **Condition:** every same-colour buyable was owned by someone else in `pre`;
  no card activation.
* **Effect:** each tile charges `ceil(full / 2 / 10) * 10` where `full` is its
  marked current-level rent; charges appear in board (「从前到后」) order.
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-同色 (colour = board `group`); OPEN-RING-RENT; the
  half on a **mortgaged** member is unspecified (see Open questions).
* **Executable:** `g_settle_13`. Scenario:
  `g_settle_13_agent_half_charge_ceil10_front_to_back`.

---

## G-SETTLE-14

* **ID:** `G-SETTLE-14`
* **Source:** `rulebook-doc.md:162`
* **Quote:** 「否则可选择该格子同色的无主或玩家拥有的[可购买格子]之一进行一次[结算]。」
* **Trigger:** a window that settles an agent without `log.agent_all_owned`.
* **Condition:** some same-colour buyable is unowned or self-owned in `pre`.
* **Effect:** at most one nested settle of a same-colour unowned-or-own
  buyable. Other-owned tiles are never the chosen target of this branch.
* **Frame:** n/a.
* **Rulings & OPEN:** 「进行一次[结算]」 is one settle of the chosen tile. The
  choice is a `tile` prompt.
* **Executable:** `g_settle_14`. Scenario:
  `g_settle_14_agent_choose_one_settles_once`.

---

## Open questions

* **OPEN-结算定义** -- 「[结算]」 is defined twice (`rulebook-doc.md:61` and
  `:87`). Use the 专有名词 definition for "execute every effect on the tile";
  the 移动 one is the landing subset.
* **OPEN-收费标价** -- 「收费标价」 undefined (Sweet Escape gate; TEST-FINDINGS §6).
* **OPEN-RING-RENT** -- RiNG has no marked rent table (`data/board.json`
  `rent: []`). 「支付…现等级地租」 has nothing to read. The engine's dice
  formula (`rings x ringMultiplier x 1d20`) comes from `match_rules.json`, not
  the rulebook. Clauses skip RiNG rent amounts.
* **OPEN-同色** -- 「同色」 for the agent is only defined as 「每种颜色拥有一个地产商格子」
  (`rulebook-doc.md:64`). This spec reads colour as the board `group`. Whether
  colour-prop overrides (「该格获得所有颜色」) widen it is undecided.
* **OPEN-AGENT-MORTGAGED-HALF** -- the all-owned agent sweep charges
  「半价收费的[结算]」. A mortgaged member's settle has no rent (5.2 / 6.2);
  whether the half applies to the optional force-buy (making it 1x) or the
  member is skipped is unspecified. Clauses skip mortgaged members.

## Findings

* **LESS (G-SETTLE-06 / G-SETTLE-07 timing)** -- genuine, stays failing: the book
  makes the optional buy / build *the settle* of an unowned / own buyable
  (「无主的[可购买格子]的[结算]是：可选择[消耗]…」 /
  「玩家拥有的[可购买格子]的[结算]是：…可选择[消耗]…升级建造」). The engine's
  `tile:property` body only *announces* on a main landing (`log.land_unowned` /
  `log.land_own`) and defers the offers to the end step
  (`docs/TILES.md` "the end step offers them again after a main move").
  Non-main (card-driven) settles do offer during the settle. Tests:
  `g_settle_06_unowned_buy_is_the_settle`,
  `g_settle_07_own_build_is_the_settle`.
