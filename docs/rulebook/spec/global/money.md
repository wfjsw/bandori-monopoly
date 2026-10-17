# G-MONEY -- 资金变动与账本

Unit 1 owns this file and `rules/spec/src/global/money.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Bankruptcy money is unit 7 (`G-BANKRUPT`) -- this unit skips `bankrupt` /
`auction` windows rather than restating that cash-out.

Covers: [获得] / [消耗] / [支付] semantics and the ledger (only named players
change); the 支付阶段 stage order; mortgage 50% / redeem 60% / force-buy 2×
and their immunity (其他规则 6); rent by level; [地产商] half rate ceil 10;
build cost; the buy figure (price + houses) that the force-buy doubles.

---

## G-MONEY-01

* **ID:** `G-MONEY-01`
* **Source:** `rulebook-doc.md:60`
* **Quote:** 「[支付]：“A[支付]BX资金”的效果为玩家A向玩家B支付X资金。」
* **Trigger:** any observed input whose event window contains money moves.
* **Condition:** the window's only money-moving events are `pay` / `rent` legs
  with both sides present and no `gain` modifier (a one-sided print / delete
  is G-MONEY-02/03; a modified transfer is the ledger).
* **Effect:** for each such leg of X from A to B: `money(A)` fell by X,
  `money(B)` rose by X, and every other player's money is unchanged.
* **Frame:** only the named pair's `money:*` may move on a pure-transfer step.
* **Rulings & OPEN:** a `gain` arg on the log line (`log.with_received`) means
  a card raised the amount the payee received; that is not a bare
  「A[支付]BX」. Bankruptcy and auction windows move money without ledger
  legs (G-BANKRUPT) and are skipped. Build-refund windows credit money without
  a money event and are skipped (observability gap).
* **Executable:** `rules_spec::global::money::g_money_01`.

---

## G-MONEY-02

* **ID:** `G-MONEY-02`
* **Source:** `rulebook-doc.md:58`
* **Quote:** 「[获得]：“[获得]X资金”的效果为直接获得X资金，等同于印钱。」
* **Trigger:** a window whose only money-moving event is a one-sided
  `gain` / `pass` of X.
* **Condition:** `gain`/`pass` names player P with value X > 0; no other
  money-moving event in the window.
* **Effect:** `money(P)` rose by exactly X; every other player is unchanged.
* **Frame:** only `money:P` may move.
* **Rulings & OPEN:** a `gain` of 0 is a no-op and is skipped. A window that
  also pays / loses / mortgages is the general ledger (G-MONEY-04).
* **Executable:** `g_money_02`. Scenario: `gain_prints_money`.

---

## G-MONEY-03

* **ID:** `G-MONEY-03`
* **Source:** `rulebook-doc.md:59`
* **Quote:** 「[消耗]：“[消耗]X资金”的效果为直接失去X资金，等同于删钱。」
* **Trigger:** a window whose only money-moving event is a one-sided `lose`
  of X.
* **Condition:** `lose` names player P with value X > 0; no other
  money-moving event in the window.
* **Effect:** `money(P)` fell by exactly X; every other player is unchanged.
* **Frame:** only `money:P` may move.
* **Rulings & OPEN:** see G-MONEY-02.
* **Executable:** `g_money_03`. Scenario: `lose_deletes_money`.

---

## G-MONEY-04

* **ID:** `G-MONEY-04`
* **Source:** `rulebook-doc.md:58-60` (the three money verbs).
* **Quote:** 「[获得]…直接获得X资金」/「[消耗]…直接失去X资金」/「A[支付]BX资金…
  玩家A向玩家B支付X资金」
* **Trigger:** every observed input whose window contains at least one
  money-moving event (and, when it contains none, that nobody moved).
* **Condition:** the window is not a ledger gap (bankrupt / auction /
  build-refund).
* **Effect:** for each seat `i`, `post.money(i) - pre.money(i)` equals the sum
  of that seat's legs (`gain`/`pass` +X, `lose` -X, `pay`/`rent` -loss /
  +received, `mortgage` +X, `redeem` -X, `forcebuy` -X / +X).
* **Frame:** a seat with no leg has delta 0 -- **only named players change**.
* **Rulings & OPEN:** `buy` / `build` markers carry no money of their own (the
  `lose` leg does). A `gain` arg on a transfer is the payee's actual receipt.
  Bankruptcy cash-out and auction bids have no ledger legs and are skipped
  (G-BANKRUPT). A window that filled the event tail (EVENT_KEEP = 400) is
  skipped: `events_since` can drop the head of a long settle. OPEN-支付阶段7
  does not change the books.
* **Executable:** `g_money_04`.

---

## G-MONEY-05

* **ID:** `G-MONEY-05`
* **Source:** `rulebook-doc.md:129` (red note `:32`)
* **Quote:** 「抵押拥有的地契，抵押后[获得]地契购买价格50%的资金」
* **Trigger:** a window containing a `mortgage` event for tile T by player P.
* **Condition:** T's `price` is the 「地契购买价格」.
* **Effect:** `money(P)` rose by exactly `price / 2`; the deed is mortgaged in
  `post`. On a bare mortgage no other seat's money moves.
* **Frame:** only `money:P` and `mortgaged:T` move.
* **Rulings & OPEN:** the rate is a fixed 50% of the **price**, not of
  price+houses (contrast G-MONEY-07). Every printed price on the current
  board is even, so the half is exact. Immune to money modifiers (其他规则 6,
  G-MONEY-08).
* **Executable:** `g_money_05`. Scenario: `mortgage_pays_exactly_half_the_price`.

---

## G-MONEY-06

* **ID:** `G-MONEY-06`
* **Source:** `rulebook-doc.md:130` (red `60%` at `:23`)
* **Quote:** 「赎回拥有的地契，赎回后[消耗]地契购买价格60%的资金」
* **Trigger:** a window containing a `redeem` event for tile T by P.
* **Condition:** T's `price` is the 「地契购买价格」.
* **Effect:** `money(P)` fell by exactly 60% of `price`; the deed is no longer
  mortgaged in `post`.
* **Frame:** only `money:P` and `mortgaged:T` move on a bare redeem.
* **Rulings & OPEN:** the book states the rate and not the rounding. Every
  printed price on the current board times 0.6 is an integer, so
  `price * 60 / 100` is exact (OPEN-60%舍入). Rate is of **price**, not
  price+houses. Immune to money modifiers (其他规则 6).
* **Executable:** `g_money_06`. Scenario: `redeem_costs_exactly_sixty_percent`.

---

## G-MONEY-07

* **ID:** `G-MONEY-07`
* **Source:** `rulebook-doc.md:161` (red note `:25`)
* **Quote:** 「[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，
  从该玩家处强行购买该格地契，获得的地契仍为抵押状态」
* **Trigger:** a window containing a `forcebuy` event for tile T.
* **Condition:** T is mortgaged in `pre` (the mortgaged-deed branch of
  基础[结算] 6.2).
* **Effect:** the buyer pays and the owner receives exactly
  `2 * (price + houses * house)`; `owners[T]` is the buyer in `post`;
  `mortgaged[T]` is still true.
* **Frame:** only the pair's `money:*` and `owner:T` / `mortgaged:T` move on a
  bare force-buy.
* **Rulings & OPEN:** 「购买格子地契和建造已有房子的资金总价」 is the same
  figure as the optional-buy price (G-MONEY-11). The engine keeps the mortgage
  unconditionally; the book says 「仍为抵押状态」 with no exception here.
  Immune to money modifiers (其他规则 6).
* **Executable:** `g_money_07`. Scenario:
  `force_buy_is_double_price_plus_houses_and_stays_mortgaged`.

---

## G-MONEY-08

* **ID:** `G-MONEY-08`
* **Source:** `rulebook-doc.md:146`
* **Quote:** 「抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动抵押，赎回，或强制购买。」
* **Trigger:** a window containing a `mortgage` / `redeem` / `forcebuy`
  event.
* **Condition:** no effect in the window 「写明改动抵押，赎回，或强制购买」.
* **Effect:** the event's `value` equals the fixed formula (50% / 60% /
  2× price+houses) and is **not** a card-scaled figure.
* **Frame:** n/a (a read of `value` against the tile data).
* **Rulings & OPEN:** the sheet's `为新版做的改动` `A20` names the 「写明」
  exception for 抵押/赎回 only and does not settle whether a pay-hook may
  retune a force-buy (TEST-FINDINGS §6.12). The engine moves all three outside
  the money pipeline, so the amount is always the native formula; a non-native
  amount is a violation.
* **Executable:** `g_money_08`. Scenarios:
  `force_buy_ignores_a_pay_multiplier`, `mortgage_ignores_a_pay_multiplier`.

---

## G-MONEY-09

* **ID:** `G-MONEY-09`
* **Source:** `rulebook-doc.md:160`
* **Quote:** 「[支付]拥有格子的玩家格子地契所标记的现等级地租。」
* **Trigger:** a `rent` event on a property tile with a non-empty rent table,
  no half-charge marker, and no `gain` modifier.
* **Condition:** `houses[T]` is the current level; `tile.rent[level]` is the
  marked 现等级地租.
* **Effect:** the payer pays the owner exactly that table entry (or a figure a
  standing scale produced from it -- see Rulings).
* **Frame:** only the pair's `money:*` moves on a bare rent.
* **Rulings & OPEN:** RiNG rent is not on the table (OPEN-RiNG地租 -- the book
  gives no RiNG rent formula). A card or standing 「支付减半」/「翻倍」
  (支付阶段 4) may scale the settled figure; those windows are skipped. The
  exact marked rate is pinned by the scenario on a vanilla board. House count
  is the counted value (「房屋数视为…」 overrides ride `World::rent_houses`;
  OPEN-视为房屋数).
* **Executable:** `g_money_09`. Scenario:
  `rent_is_the_marked_rate_at_the_current_level`.

---

## G-MONEY-10

* **ID:** `G-MONEY-10`
* **Source:** `rulebook-doc.md:162`
* **Quote:** 「需向该格子同色的所有[可购买格子]从前到后依次进行一次半价收费的[结算]（向上取整10）」
* **Trigger:** a `rent` event whose message carries the half-charge marker
  (`full` / `cut`).
* **Condition:** `full` is the pre-half rate (the 现等级地租).
* **Effect:** `cut == ceil(full / 2 / 10) * 10` and the money that moved is
  `cut`.
* **Frame:** n/a on the rate; the pair's money is the ledger's job.
* **Rulings & OPEN:** 「同色」 is the board's colour group (OPEN-Agent同色 --
  whether 「该格获得所有颜色」 widens the set is undecided). The rounding is
  「向上取整10」, not banker's rounding: 301 → 160.
* **Executable:** `g_money_10`. Scenarios: `agent_half_charge_rounds_up_to_ten`,
  `agent_half_charge_is_front_to_back`.

---

## G-MONEY-11

* **ID:** `G-MONEY-11`
* **Source:** `rulebook-doc.md:155`
* **Quote:** 「可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。」
* **Trigger:** a `buy` event for tile T by player P.
* **Condition:** T was unowned in `pre`; `houses[T]` is the standing house
  count.
* **Effect:** P paid exactly `price + houses * house` (when no card retuned
  the figure); `owners[T]` is P in `post`.
* **Frame:** only `money:P` and `owner:T` / `mortgaged:T` / `houses:T` move on
  a bare buy.
* **Rulings & OPEN:** a card or standing BuyAdd / BuyMul / BuySet may rewrite
  the buy price without a marker in the window; the per-act clause then only
  checks the ownership transfer and lets the ledger close the money. The exact
  `price + houses * house` figure is pinned by the scenario on a vanilla
  board. The force-buy figure is twice this same base (G-MONEY-07).
* **Executable:** `g_money_11`. Scenario: `buy_costs_price_plus_houses`.

---

## G-MONEY-12

* **ID:** `G-MONEY-12`
* **Source:** `rulebook-doc.md:157`
* **Quote:** 「可选择[消耗]格子地契所标注的房屋建筑费进行升级建造」
* **Trigger:** a `build` event for tile T by player P that raised a house.
* **Condition:** T is owned by P and not mortgaged; `tile.house` is the
  「房屋建筑费」.
* **Effect:** P paid exactly `tile.house` (when no card retuned it) and
  `houses[T]` rose by 1.
* **Frame:** only `money:P` and `houses:T` move on a bare build.
* **Rulings & OPEN:** 「升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得」
  is a funding restriction (the engine refuses the build outright on a
  mortgaged deed). A card or standing build discount may retune the cost
  without a marker in the window; the per-act clause then only checks the
  house rose. The exact 「房屋建筑费」 figure is pinned by the scenario on a
  vanilla board.
* **Executable:** `g_money_12`. Scenario: `build_costs_the_marked_house_fee`.

---

## G-MONEY-13

* **ID:** `G-MONEY-13`
* **Source:** `rulebook-doc.md:229-232` (支付阶段 rows 1-4)
* **Quote:** 「| 1 | 支付计算前 | | | | |
  | 2 | 支付计算时 | 支付增加/减少(固定值) | | | 例：白鹭千圣… |
  | 3 | 支付计算后 | | | | N/A |
  | 4 | 支付前 | 支付减半/翻倍 | | | 例：八潮瑠唯…」
* **Trigger:** a payment whose settled figure carries **both** a fixed
  adjustment and a scale.
* **Condition:** the declared amount is B; the fixed adjustment is ±A; the
  scale is ×s (with 「向上取整10」 where the text says so).
* **Effect:** the settled amount is `scale(B ± A)`, **not** `scale(B) ± A`.
* **Frame:** the ledger (G-MONEY-04) owns who moved.
* **Rulings & OPEN:** the table's stage names are the book's; the engine
  raises them as `paySplitAdd` (row 2) then `paySplitMul` (row 4). 丸山彩（2） /
  白鹭千圣（2） say 「分摊前」, which is the pre-split figure the row-2
  window rewrites. Live sheet text for the table's example cards supersedes
  the older gloss in the table's 例 column (秘密与青春的虹彩 is now
  （1）减半 / （2）1.5倍, not cancel / double). OPEN-支付阶段7 (where row 7
  sits) does not change this order.
* **Executable:** `g_money_13` (the chain identity on half-charge markers).
  Scenarios: `pay_stages_add_before_halve`,
  `pay_stages_fixed_add_on_receive`. The add-before-scale order itself cannot
  be read per-act (standing effects hide the declared figure), so it lives in
  the scenario.

---

## G-MONEY-14

* **ID:** `G-MONEY-14`
* **Source:** `rulebook-doc.md:233-235` (支付阶段 rows 5-7)
* **Quote:** 「| 5 | 支付时 | 支付时/无需失去资金/取消支付/更改目标 | 进行支付 |
  | 6 | 支付后 | 支付后 | | |
  | 7 | 资金变动(合并到[支付后]?) | | 非无效/改变支付，无效支付，和改变目标的效果结算 |」
* **Trigger:** a window in which a payment was cancelled / set to
  「无需失去资金」, or a 「资金变动」 trigger fired on the settled loss.
* **Condition:** the cancellation is visible as the absence of a money-moving
  event for that payment.
* **Effect:** no seat's money moves on account of the cancelled payment;
  「资金变动」 triggers see the **settled** figure.
* **Frame:** the ledger (G-MONEY-04) owns the rest of the window.
* **Rulings & OPEN:** 「更改目标」 (redirect) shows up as `other` naming a
  different seat; G-MONEY-01/04 then close the books against that pair.
  OPEN-支付阶段7: the book is unsure whether row 7 merges into 支付后; the
  trigger amount is still the settled figure either way (the sheet's
  振翅高飞的练习曲 (1) 「一次性失去2000以上资金时，抽一张卡」 is the row-7
  example, and notes 「该技能对有角色破产后的地契拍卖支出不生效」).
* **Executable:** `g_money_14`. Scenarios: `pay_stages_cancel_moves_nothing`,
  `pay_stages_money_change_sees_the_settled_loss`.

---

## Open questions

* **OPEN-分摊** (分摊 undefined) -- a 分摊 clause cannot be written yet.
* **OPEN-支付阶段7** -- where 支付阶段 row 7 (「资金变动」) sits relative to
  支付后 is undecided (`rulebook-doc.md:235` says 「合并到[支付后]?」). The
  settled-amount reading is specified either way.
* **OPEN-60%舍入** -- the redeem rate is 60% of the price and the book does
  not name a rounding mode. Every printed price on the current board times
  0.6 is an integer, so the question is moot on this board; a future odd
  price would need a ruling.
* **OPEN-RiNG地租** -- RiNG deeds have no rent table (`data/board.json`
  `rent: []`). The book only says 「所有RiNG不可升级」 and gives no RiNG rent
  formula (TEST-FINDINGS §6.9). This unit does not invent one.
* **OPEN-Agent同色** -- 「每种颜色拥有一个地产商格子」 never says whether
  `extraColor` / 「该格获得所有颜色」 widens the colour group
  (TEST-FINDINGS §6.10). The basic same-group sweep is covered.
* **OPEN-视为房屋数** -- 「房屋数视为…」 overrides ride the counted house
  count; whether Ringing Bloom (3) reads the counted or the real count is
  TEST-FINDINGS §6.3.
* **OPEN-强制购买支付钩** -- whether a 「写明改动强制购买」 effect may retune
  the force-buy sum is undecided (TEST-FINDINGS §6.12). G-MONEY-08 treats a
  non-native amount as a violation, matching the engine's current
  outside-the-pipeline transfer.

## Findings

(Empty -- every scripted scenario and the fuzz soak honour the quotes above.
If a soak violation appears, record it here with the clause ID and the
one-line reason.)