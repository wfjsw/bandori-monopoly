# G-BANKRUPT -- [破产] / [存活] / 离开 / 拍卖

Unit 7 owns this file and `rules/spec/src/global/bankruptcy.rs`. Append
clauses; do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Primary quotes:

* `rulebook-doc.md:62` 「[破产]：当玩家无法支付某笔支出时（包括抵押），将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象（例：[支付]其他玩家的格子[结算]）。」
* `rulebook-doc.md:63` 「[存活]：非[破产]状态且未离开游戏的玩家。」
* `rulebook-doc.md:66` 「[拍卖]：[拍卖]地契时所有[存活]且可[消耗]资金的玩家参与喊价，最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权。每名参与玩家不限喊价次数。最低喊价金额为100，最低单次太高喊价金额为100。」
* `rulebook-doc.md:114` 「可在所有玩家同意时结束游戏。一些参考时点包括：在有一个玩家进入[破产]时，仅剩一名[存活]玩家时，或因时间原因多名玩家需要离开游戏。」
* `rulebook-doc.md:121` 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
* `rulebook-doc.md:129` 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择**抵押拥有的地契，抵押后[获得]地契购买价格50%的资金**。」
* `rulebook-doc.md:133-135` 游戏流程 9 (bankruptcy / 弃权 cleanup + auction)

---

## G-BANKRUPT-01

* **ID:** `G-BANKRUPT-01`
* **Source:** `rulebook-doc.md:135`
* **Quote:** 「**其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
* **Trigger:** an observed input whose event window contains a `bankrupt` or
  `left` event for player `P`.
* **Condition:** `P` owned at least one buyable tile in `pre`.
* **Effect:** in `post`, no buyable tile is owned by `P`. (The auction of
  min(3, n) deeds is `G-BANKRUPT-04`; this one only checks the un-own step.)
* **Frame:** n/a (read of `owners`).
* **Rulings & OPEN:** 「[可购买格子]」 excludes CiRCLE / 咖啡厅 / 江户川 /
  流星堂 / [地产商] (`rulebook-doc.md:65`). Bankruptcy money has **no ledger
  legs** (the engine cashes mortgages silently and zeroes the payer) -- that
  is a documented observability gap and is why G-MONEY-01 skips `bankrupt`
  windows.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_01`.

---

## G-BANKRUPT-02

* **ID:** `G-BANKRUPT-02`
* **Source:** `rulebook-doc.md:62`
* **Quote:** 「[破产]：当玩家无法支付某笔支出时（包括抵押），将所有可折现的资产折现并将所有资金[消耗]或[支付]给导致破产的效果对象（例：[支付]其他玩家的格子[结算]）。」
* **Trigger:** an observed input whose event window contains a `bankrupt`
  event for player `P`.
* **Condition:** `P` could not cover the expense even after 抵押.
* **Effect:** in `post`, `P.money == 0` (「将所有资金[消耗]或[支付]…」). The
  amount handed to the causing effect's target is the `bankrupt` event's
  `value`; exact legs are checked in `tests/g_bankrupt.rs`
  (`bankrupt_pays_out_to_the_creditor`, `bankrupt_without_creditor_consumes_the_funds`).
* **Frame:** n/a (read of `money`).
* **Rulings & OPEN:** 「可折现的资产」 is not defined as a set -- OPEN-可折现资产.
  The determined part is the 50% mortgage liquidation of deeds
  (`rulebook-doc.md:129`) and that all funds leave `P`. A `left` (弃权) seat is
  not covered: the book gives no cash-out for leaving.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_02`.

---

## G-BANKRUPT-03

* **ID:** `G-BANKRUPT-03`
* **Source:** `rulebook-doc.md:134`
* **Quote:** 「将其控制的所有棋子，角色卡，乐队卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。」
* **Trigger:** an observed input whose event window contains a `bankrupt` or
  `left` event for player `P`.
* **Condition:** (none beyond the trigger.)
* **Effect:** in `post`, `P`'s hand is empty and `P`'s field is empty (the
  field holds the character card, the band card and every 「[持续]」 instance --
  they leave the game with the seat, so their effects stop). `P` is marked out
  (`bankrupt` or `left`).
* **Frame:** n/a (reads of `hand` / `field` / flags).
* **Rulings & OPEN:** 「棋子」 has no world slot besides `pos`, which the book
  does not say is cleared -- OPEN-棋子移出. 「乐队卡」 is undefined as a card
  class -- OPEN-乐队卡.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_03`.
* **Scenarios:** `bankrupt_clears_hand_and_field`, `leave_runs_the_same_cleanup`.

---

## G-BANKRUPT-04

* **ID:** `G-BANKRUPT-04`
* **Source:** `rulebook-doc.md:135`
* **Quote:** 「**其拥有的[可购买格子]变成无主状态**并抽取3张其中格子的地契进行[拍卖]（少于3则全部）。」
* **Trigger:** a window that takes `P` out with `n > 0` buyable deeds in
  `pre`, and the match is still playing afterwards.
* **Condition:** `P` owned `n` buyable tiles in `pre`.
* **Effect:** the window logs an `auction_leftovers` sample of exactly
  `min(3, n)` tiles, every one of which was `P`'s deed in `pre`.
* **Frame:** n/a (read of the event list).
* **Rulings & OPEN:** which 3 of the n are drawn is the engine's shuffle -- the
  book only fixes the count. Mortgage status of the drawn deeds is
  OPEN-拍卖抵押状态. When the game ends on the same step the book does not say
  the auction still runs -- the clause skips. A hook that steals a deed before
  the draw shrinks the pool.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_04`.
* **Scenarios:** `leave_auctions_up_to_three_deeds`,
  `leave_auctions_all_when_fewer_than_three`.

---

## G-BANKRUPT-05

* **ID:** `G-BANKRUPT-05`
* **Source:** `rulebook-doc.md:66`
* **Quote:** 「[拍卖]地契时所有[存活]且可[消耗]资金的玩家参与喊价…」
* **Trigger:** a step whose window completed an auction (`log.auction_won`).
* **Condition:** the winning bid names player `W`.
* **Effect:** `W` is [存活] in `post` (`!bankrupt && !left`). The full
  participant list lives on the host prompt, not in `World`, so the
  everyone-participates half is a scenario assertion.
* **Frame:** n/a (read of flags).
* **Rulings & OPEN:** 「可[消耗]资金」 is OPEN-可消耗资金. This clause checks
  only the [存活] half of the conjunction. The [存活] definition itself is
  `rulebook-doc.md:63` (「非[破产]状态且未离开游戏的玩家」), pinned by
  `alive_is_neither_bankrupt_nor_left`.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_05`.
* **Scenarios:** `auction_prompt_waits_on_alive_seats`,
  `alive_is_neither_bankrupt_nor_left`.

---

## G-BANKRUPT-06

* **ID:** `G-BANKRUPT-06`
* **Source:** `rulebook-doc.md:66`
* **Quote:** 「最先喊价最高金额的玩家[消耗]同等资金并获得地契和对应格子拥有权。」
* **Trigger:** a window containing a completed auction (`log.auction_won`).
* **Condition:** the winning bid is `X` on tile `T` by player `W`.
* **Effect:** `post`'s `owners[T] == W`. On a window with no other
  money-moving events, `W`'s net money change is `mortgage credits - X`.
* **Frame:** n/a (reads of `money` / `owners`).
* **Rulings & OPEN:** 「最先喊价最高金额」 -- with a 100 minimum raise the
  highest amount is unique; the first (and only) player to bid it is the
  winner. OPEN-拍卖最高额 covers other readings. Houses ride along --
  OPEN-拍卖房屋. The bid floor / raise floor / unlimited bids are scenario
  assertions (`auction_bid_floor_raise_and_winner`), because a *rejected* bid
  is not an observed transition.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_06`.
* **Scenarios:** `auction_bid_floor_raise_and_winner`,
  `auction_winner_pays_exactly_the_bid`.

---

## G-BANKRUPT-07

* **ID:** `G-BANKRUPT-07`
* **Source:** `rulebook-doc.md:121`
* **Quote:** 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
* **Trigger:** a step that leaves exactly one [存活] player and ends the match.
* **Condition:** `post` has one [存活] seat and `phase == "ended"`.
* **Effect:** `post.st.winner` is that seat.
* **Frame:** n/a (reads of flags / `phase` / `winner`).
* **Rulings & OPEN:** the quote is the 正规-mode rule; 娱乐 mode ends by
  consent (`rulebook-doc.md:114`). The engine also auto-ends on the last
  survivor in every mode -- compatible with 娱乐's 「参考时点」, not asserted
  as a 娱乐 requirement. MatchMode ↔ 正规/娱乐 is OPEN-模式映射.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_07`.
* **Scenarios:** `last_alive_player_wins`,
  `entertainment_ends_by_unanimous_consent`,
  `one_dissent_blocks_the_consent_end`.

---

## G-BANKRUPT-08

* **ID:** `G-BANKRUPT-08`
* **Source:** `rulebook-doc.md:129`
* **Quote:** 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择**抵押拥有的地契，抵押后[获得]地契购买价格50%的资金**。」
* **Trigger:** a window containing `mortgage` events (the raise-funds path
  that runs before bankruptcy, and the voluntary 运营-phase mortgage).
* **Condition:** the mortgage is of a tile `T` owned by the mortgagor.
* **Effect:** each mortgage leg credits exactly `T.price * 50%`
  (`event.value == price / 2`). On a pure-mortgage window the mortgagor's net
  money change equals the sum of those credits.
* **Frame:** n/a (read of `money` / `mortgaged`).
* **Rulings & OPEN:** integer halving of an odd price is OPEN-抵押半价取整 (the
  shipped board's prices are all even). 「可以选择」 means the offer may be
  declined -- a declined mortgage is not an error here.
* **Executable:** `rules_spec::global::bankruptcy::g_bankrupt_08`.
* **Scenarios:** `shortfall_may_be_funded_by_mortgage_at_half_price`,
  `mortgage_credits_half_the_deed_price`.

---

## Open questions

| OPEN | text | notes |
|---|---|---|
| OPEN-乐队卡 | 乐队卡 | undefined as a card class (bankruptcy removes them). |
| OPEN-可折现资产 | 可折现的资产 | undefined as a set (houses? only deeds?). The 50% deed mortgage is determined; house liquidation is not. |
| OPEN-棋子移出 | 棋子移出游戏 | `World.pos` is the only piece slot and the book does not say it is cleared. |
| OPEN-可消耗资金 | 可[消耗]资金 | undefined gate for auction eligibility (cash? cash + mortgageable? `can_pay`?). |
| OPEN-拍卖最高额 | 最先喊价最高金额 | with a 100 raise the highest bid is unique; other readings unspecified. |
| OPEN-拍卖抵押状态 | 拍卖地契的抵押状态 | the book does not say whether a drawn deed stays mortgaged. |
| OPEN-拍卖房屋 | 拍卖上的房屋 | the book does not say what happens to houses on an auctioned deed. |
| OPEN-拍卖无人出价 | nobody bids | the book does not say what happens to a deed nobody wants. |
| OPEN-抵押半价取整 | 50% rounding | odd deed prices are unaddressed (shipped prices are all even). |
| OPEN-模式映射 | 正规 / 娱乐 ↔ MatchMode | the rulebook modes are not named against the engine's Solo/Casual/Ranked. |
| OPEN-离开资金 | 弃权时的资金 | the book gives no cash-out for leaving (unlike 破产). |

## Findings

(Empty -- every scenario above is derived from the quote alone. If an engine
gap shows up as a red test, name the clause id and the one-line reason here.)
