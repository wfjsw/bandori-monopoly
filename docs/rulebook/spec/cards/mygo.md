# C-MYGO -- MyGO!!!!! cards, character skills, band skill

Unit 16 owns this file and `rules/spec/tests/card_mygo.rs`. All 15 `MyGO:*`
entries in `docs/rulebook/cards.json`, the five `MyGO!!!!!` character skills
in `data/characters.json`, and the band skill in `data/bands.json`.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Clause IDs: `C-MYGO-<slug>-<n>`. Every clause of every printed
（1）（2）…, [手], [持续], [反击], [限] body is listed. A clause that cannot be
tested from the harness says so under **Executable**.

---

## Card: `MyGO:[千早爱音]Anon Tokyo`

Card text (`cards.json#MyGO:[千早爱音]Anon Tokyo`):

```text
[千早爱音]Anon Tokyo：
[限]：
自己所在的格子是[可购买格子]。
[手]：
[指定][使用者]所在当格的任一相邻的[可购买格子]，根据使用者所在格子和被[指定]格子的状态依次进行以下操作：
1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1），被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半；
2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。
```

### C-MYGO-anon-tokyo-01

* **ID:** `C-MYGO-anon-tokyo-01`
* **Source:** `cards.json#MyGO:[千早爱音]Anon Tokyo`
* **Quote:** 「[限]：自己所在的格子是[可购买格子]。」
* **Trigger:** a `play` of this card from hand.
* **Condition:** pre-state of the user's `pos`.
* **Effect:** the play is refused unless `pos` names a
  [可购买格子] (rulebook-doc.md:65: non-CiRCLE, non-CiRCLE 咖啡厅, non-江户川乐器店,
  non-流星堂, non-[地产商]).
* **Frame:** n/a (a refused play is not a transition).
* **Rulings & OPEN:** none.
* **Executable:** `test anon_tokyo_limit_and_link_pay`.

### C-MYGO-anon-tokyo-02

* **ID:** `C-MYGO-anon-tokyo-02`
* **Source:** `cards.json#MyGO:[千早爱音]Anon Tokyo`
* **Quote:** 「[指定][使用者]所在当格的任一相邻的[可购买格子]」
* **Trigger:** the [手] body, after the play is accepted.
* **Condition:** user is on a [可购买格子].
* **Effect:** the designation prompt offers exactly the adjacent
  [可购买格子] set (adjacent = board neighbours of `pos`).
* **Frame:** n/a (prompt state is not in `World`).
* **Rulings & OPEN:** none.
* **Executable:** `test anon_tokyo_branch_two_moves_one_and_settles`.

### C-MYGO-anon-tokyo-03

* **ID:** `C-MYGO-anon-tokyo-03`
* **Source:** `cards.json#MyGO:[千早爱音]Anon Tokyo`
* **Quote:** 「1. [使用者]同时拥有上述的两个格子则[消耗]其中地契购买价格中更高者的资金的一半并在上述的两个格子间放置1个[奇迹水晶]（上限1）」
* **Trigger:** the [手] body, branch 1 (user owns both tiles).
* **Condition:** `owners[pos] == user` and `owners[designated] == user`.
* **Effect:** user's money falls by exactly
  `max(price[pos], price[designated]) / 2` ([消耗] = delete money); a
  crystal-link mark is placed on each of the two tiles, at most 1 link.
* **Frame:** only `money:P{user}` and the board marks move.
* **Rulings & OPEN:** 「上限1」 is the link cap (one crystal between the pair).
* **Executable:** `c_mygo_anon_tokyo_link_pay`, `test anon_tokyo_link_pay_and_marks`.

### C-MYGO-anon-tokyo-04

* **ID:** `C-MYGO-anon-tokyo-04`
* **Source:** `cards.json#MyGO:[千早爱音]Anon Tokyo`
* **Quote:** 「被[奇迹水晶]连接的格子收费时，会额外收取被连接的其他格子收费的一半」
* **Trigger:** a rent/charge settle on a crystal-linked tile.
* **Condition:** the settled tile carries a link mark; the linked partner
  tile has a known 收费.
* **Effect:** the payer is charged the settled tile's own rent **plus**
  half of the linked partner tile's rent.
* **Frame:** only `money:*` of payer and owner move on that settle.
* **Rulings & OPEN:** OPEN-t04 -- does the link add half of a **mortgaged**
  partner tile's 收费? TEST-FINDINGS §6 records the question; the clause
  specifies only the determined part (unmortgaged partner rent).
* **Executable:** `test anon_tokyo_link_adds_half_partner_rent`.

### C-MYGO-anon-tokyo-05

* **ID:** `C-MYGO-anon-tokyo-05`
* **Source:** `cards.json#MyGO:[千早爱音]Anon Tokyo`
* **Quote:** 「2. [使用者]不同时拥有上述的两个格子则进入移动阶段并将本回合的[主要移动]改为向前或后移动1格到被[指定]格子且[结算]。」
* **Trigger:** the [手] body, branch 2 (user does not own both).
* **Condition:** the two tiles are not both owned by the user.
* **Effect:** the turn enters the move phase; the [主要移动] becomes a
  1-step move (forward or backward) to the designated tile, and that tile
  is [结算]d.
* **Frame:** only the user's `pos` (and the settle of the landing tile) move.
* **Rulings & OPEN:** the forward/backward choice is the player's; the
  clause checks the end position and that a settle ran.
* **Executable:** `test anon_tokyo_branch_two_moves_one_and_settles`.

---

## Card: `MyGO:即使迷茫着`

```text
即使迷茫着：
（1）[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。
（2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。
```

### C-MYGO-even-lost-01

* **ID:** `C-MYGO-even-lost-01`
* **Source:** `cards.json#MyGO:即使迷茫着`
* **Quote:** 「（1）[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。」
* **Trigger:** a [反击] window opened by another player's card effect on you.
* **Condition:** you hold this card; the affecting card is not your own.
* **Effect:** playing this as [反击] places it on your 场上 (field) and
  removes it from hand. An own-card effect must **not** open this window.
* **Frame:** only `hand:P{you}` / `field:P{you}` / `discard` move.
* **Rulings & OPEN:** none.
* **Executable:** `test even_lost_counter_and_continuing`,
  `test even_lost_counter_not_for_own_card`.

### C-MYGO-even-lost-02

* **ID:** `C-MYGO-even-lost-02`
* **Source:** `cards.json#MyGO:即使迷茫着`
* **Quote:** 「（2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。」
* **Trigger:** the [持续] press of this field card during 主要阶段 (运营).
* **Condition:** the card is on your field; you have a hand count `H`.
* **Effect:** the card leaves the field into 弃牌堆; you enter the move
  phase; the [主要移动] steps equal `H`.
* **Frame:** the user's `pos` / zones / `discard` move; the card is not in
  hand or on field afterwards.
* **Rulings & OPEN:** none.
* **Executable:** `test even_lost_counter_and_continuing`.

---

## Card: `MyGO:哪怕这旅程没有终点`

```text
哪怕这旅程没有终点：
 [手] 将此卡放置于当前格子上并为其放置4个奇迹水晶，每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。当此卡奇迹水晶数量为0时，将此卡置入弃牌堆。
（2）[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数
```

### C-MYGO-journey-01

* **ID:** `C-MYGO-journey-01`
* **Source:** `cards.json#MyGO:哪怕这旅程没有终点`
* **Quote:** 「[手] 将此卡放置于当前格子上并为其放置4个奇迹水晶」
* **Trigger:** the [手] play of this card.
* **Condition:** none beyond a legal play from hand.
* **Effect:** the card is placed on the current tile with exactly 4
  [奇迹水晶].
* **Frame:** only the user's hand/field zones move.
* **Rulings & OPEN:** none.
* **Executable:** `c_mygo_journey_place`, `test journey_places_on_tile_with_four_crystals`.

### C-MYGO-journey-02

* **ID:** `C-MYGO-journey-02`
* **Source:** `cards.json#MyGO:哪怕这旅程没有终点`
* **Quote:** 「每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶。」
* **Trigger:** a turn end.
* **Condition:** the just-finished turn's [主要移动] steps `S` and this card
  is on the field with `crystals > 0`.
* **Effect:** if `S > 6` (strictly), exactly one crystal is lost; if
  `S <= 6`, the count is unchanged.
* **Frame:** only `crystals:P{owner}:<card>` moves.
* **Rulings & OPEN:** 「主要移动数」 is the step count of that turn's main
  move (determined by the quote's comparison).
* **Executable:** `test journey_drain_and_discard`,
  `test journey_no_drain_when_move_is_six_or_less`.

### C-MYGO-journey-03

* **ID:** `C-MYGO-journey-03`
* **Source:** `cards.json#MyGO:哪怕这旅程没有终点`
* **Quote:** 「当此卡奇迹水晶数量为0时，将此卡置入弃牌堆。」
* **Trigger:** the crystal count of this card reaches 0.
* **Condition:** the card is on the field.
* **Effect:** the card leaves the field into 弃牌堆.
* **Frame:** only the owner's field/discard zones move.
* **Rulings & OPEN:** none.
* **Executable:** `test journey_drain_and_discard`.

### C-MYGO-journey-04

* **ID:** `C-MYGO-journey-04`
* **Source:** `cards.json#MyGO:哪怕这旅程没有终点`
* **Quote:** 「（2）[持续] 触发结算时，获得X*60资金，X为你此次主要移动[经过]的格数」
* **Trigger:** a [触发结算] of the owner after a [主要移动].
* **Condition:** the card is on the field; the move's [经过] count is `X`.
* **Effect:** the owner gains exactly `X * 60` money ([获得] = print).
* **Frame:** only `money:P{owner}` moves among money paths.
* **Rulings & OPEN:** OPEN-触发结算 -- the book never defines
  [触发结算] as a term (it appears as the 行动阶段 row-15 label). The clause
  is written against the settle that follows a [主要移动], which is what the
  quote names.
* **Executable:** `test journey_place_and_settle_gain`.

---

## Card: `MyGO:壱雫空`

```text
壱雫空：
 清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。（此卡可在眩晕时打出）
```

### C-MYGO-hitoshizuku-01

* **ID:** `C-MYGO-hitoshizuku-01`
* **Source:** `cards.json#MyGO:壱雫空`
* **Quote:** 「清除场上所有[停留]与[眩晕]效果」
* **Trigger:** the play of this card.
* **Condition:** any players hold [停留] / [眩晕] layers.
* **Effect:** every [停留] and [眩晕] layer on every player is removed
  (the statuses go to 0).
* **Frame:** only `state:P*:stay` / `state:P*:stun` (and the money of -02)
  move.
* **Rulings & OPEN:** the glossary writes [晕眩]; this card writes [眩晕].
  Both name the same status (`stay` / `stun` keys).
* **Executable:** `c_mygo_hitoshizuku_clear`, `test hitoshizuku_clears_all_stay_and_stun`.

### C-MYGO-hitoshizuku-02

* **ID:** `C-MYGO-hitoshizuku-02`
* **Source:** `cards.json#MyGO:壱雫空`
* **Quote:** 「所有玩家因本效果每清除一种效果则支付此卡使用者1000资金」
* **Trigger:** same play.
* **Condition:** a player held at least one of the two effect types.
* **Effect:** that player [支付]s the user 1000 **per effect type cleared**
  for that player (ruling: per effect type per affected player -- two types
  on one player = 2000).
* **Frame:** only `money:*` of the affected players and the user move.
* **Rulings & OPEN:** Decided (TEST-FINDINGS §6 2026-10-06): 「每清除一种效果」
  is per effect type per affected player; the user gains 1000 extra per own
  type (clause -03).
* **Executable:** `test hitoshizuku_clear_charge_and_stun_play`.

### C-MYGO-hitoshizuku-03

* **ID:** `C-MYGO-hitoshizuku-03`
* **Source:** `cards.json#MyGO:壱雫空`
* **Quote:** 「若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金」
* **Trigger:** same play.
* **Condition:** the user held one or both statuses.
* **Effect:** the user gains an extra 1000 per own effect type cleared
  (in addition to the -02 payments landing on the user as payee).
* **Frame:** only `money:P{user}` moves for this leg.
* **Rulings & OPEN:** Decided, as above.
* **Executable:** `test hitoshizuku_clear_charge_and_stun_play`.

### C-MYGO-hitoshizuku-04

* **ID:** `C-MYGO-hitoshizuku-04`
* **Source:** `cards.json#MyGO:壱雫空`
* **Quote:** 「（此卡可在眩晕时打出）」
* **Trigger:** a play attempt while the user is [眩晕].
* **Condition:** the user has stun layers.
* **Effect:** the play is accepted (stun normally forbids [手] effects).
* **Frame:** n/a.
* **Rulings & OPEN:** none.
* **Executable:** `test hitoshizuku_clear_charge_and_stun_play`.

---

## Card: `MyGO:无路矢`

```text
无路矢：
 指定场上自己以外的一位玩家所在格子，获得2层[除外]并在[除外]层数归0后[传送]至该格子，视为当回合的主要移动。[除外]期间本应获得的格子收入由此前指定的那名玩家获得。
```

### C-MYGO-murosshi-01

* **ID:** `C-MYGO-murosshi-01`
* **Source:** `cards.json#MyGO:无路矢`
* **Quote:** 「指定场上自己以外的一位玩家所在格子」
* **Trigger:** the play of this card.
* **Condition:** at least one other living player is on the board.
* **Effect:** the user designates one other player's current tile; the
  destination is that tile.
* **Frame:** the designated tile is recorded (via `exileTo` / the like).
* **Rulings & OPEN:** none.
* **Executable:** `test murosshi_exile_and_teleport`.

### C-MYGO-murosshi-02

* **ID:** `C-MYGO-murosshi-02`
* **Source:** `cards.json#MyGO:无路矢`
* **Quote:** 「获得2层[除外]」
* **Trigger:** the same play, after designation.
* **Condition:** none.
* **Effect:** the user gains exactly 2 layers of [除外].
* **Frame:** only `state:P{user}:exile` moves.
* **Rulings & OPEN:** [除外] is defined at rulebook-doc.md:98.
* **Executable:** `c_mygo_murosshi_exile`, `test murosshi_gains_two_exile`.

### C-MYGO-murosshi-03

* **ID:** `C-MYGO-murosshi-03`
* **Source:** `cards.json#MyGO:无路矢`
* **Quote:** 「在[除外]层数归0后[传送]至该格子，视为当回合的主要移动。」
* **Trigger:** the user's [除外] count reaches 0 (turn start removes a layer).
* **Condition:** the destination tile was designated by this card.
* **Effect:** the user is [传送]d to the designated tile; that teleport is
  the turn's [主要移动].
* **Frame:** only `pos:P{user}` (and the landing settle) moves.
* **Rulings & OPEN:** 「视为当回合的主要移动」 -- the one-main-move-per-turn
  rule then applies; the book does not say whether an already-taken main
  move is undone (unspecified; clause checks the teleport and the end pos).
* **Executable:** `test murosshi_exile_and_teleport`.

### C-MYGO-murosshi-04

* **ID:** `C-MYGO-murosshi-04`
* **Source:** `cards.json#MyGO:无路矢`
* **Quote:** 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得。」
* **Trigger:** a tile income the user would have gained while [除外] is up.
* **Condition:** the user holds [除外] layers from this card.
* **Effect:** the income goes to the previously designated player, not the
  user.
* **Frame:** only `money:*` of the income parties move.
* **Rulings & OPEN:** 「格子收入」 is not a glossary term; the clause is
  written for rent/reward the user would have received.
* **Executable:** `test murosshi_income_goes_to_designated_player`.

---

## Card: `MyGO:普通与理所当然`

```text
普通与理所当然：
 [反击] 受到异常移动效果影响后可打出，使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。
```

### C-MYGO-normal-01

* **ID:** `C-MYGO-normal-01`
* **Source:** `cards.json#MyGO:普通与理所当然`
* **Quote:** 「[反击] 受到异常移动效果影响后可打出」
* **Trigger:** a [反击] window after the user was hit by an
  [异常移动效果] (rulebook-doc.md:100: [传送], [停留], [晕眩], [除外],
  [强制移动], [强制停下], 反方向移动).
* **Condition:** the user holds this card; the abnormal move effect just
  resolved on them.
* **Effect:** the card may be played in that window (optional).
* **Frame:** the card leaves hand into discard on play.
* **Rulings & OPEN:** none.
* **Executable:** `test normal_copies_last_main_move_steps`.

### C-MYGO-normal-02

* **ID:** `C-MYGO-normal-02`
* **Source:** `cards.json#MyGO:普通与理所当然`
* **Quote:** 「使你下一次主要移动的格数变为移动你最近一次非传送的主要移动的移动格数。」
* **Trigger:** the next [主要移动] of the user after this [反击].
* **Condition:** the user has a recorded most-recent non-teleport main
  move of `S` steps.
* **Effect:** that main move takes exactly `S` steps.
* **Frame:** only `pos:P{user}` (and the landing settle) moves.
* **Rulings & OPEN:** OPEN-G13 (TEST-FINDINGS §6) -- does the copy carry a
  **signed** 移动格数 (backward)? The quote says 格数 (a count); the clause
  checks the absolute step count of the copied move.
* **Executable:** `test normal_copies_last_main_move_steps`.

---

## Card: `MyGO:羽丘的不可思议女孩`

```text
羽丘的不可思议女孩：
 位于属于自己的格子上时，投掷1d20，若出目至少为10则在当前格子免费加盖一层房屋，若出目至少为15，则额外抽一张卡，至少为20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃牌堆并抵消一次任意付款。若小于10，此卡放入弃牌堆且视为此卡未生效
```

### C-MYGO-haneoka-01

* **ID:** `C-MYGO-haneoka-01`
* **Source:** `cards.json#MyGO:羽丘的不可思议女孩`
* **Quote:** 「位于属于自己的格子上时，投掷1d20」
* **Trigger:** the play of this card.
* **Condition:** the user is on a tile they own.
* **Effect:** a 1d20 is rolled; the following branches key off the face.
* **Frame:** n/a beyond the branches below.
* **Rulings & OPEN:** the play is gated on being on one's own tile.
* **Executable:** `test haneoka_thresholds_and_ineffective`.

### C-MYGO-haneoka-02

* **ID:** `C-MYGO-haneoka-02`
* **Source:** `cards.json#MyGO:羽丘的不可思议女孩`
* **Quote:** 「若出目至少为10则在当前格子免费加盖一层房屋」
* **Trigger:** the same roll.
* **Condition:** face `>= 10`.
* **Effect:** exactly one house is added on the current tile; no money is
  paid (「免费」).
* **Frame:** only `houses:T{tile}` moves; `money:P{user}` is unchanged.
* **Rulings & OPEN:** Sheet-superseded (TEST-FINDINGS §6): thresholds are
  「至少为10/15/20」 -- 10, 15, 20 themselves qualify.
* **Executable:** `test haneoka_thresholds_and_ineffective`.

### C-MYGO-haneoka-03

* **ID:** `C-MYGO-haneoka-03`
* **Source:** `cards.json#MyGO:羽丘的不可思议女孩`
* **Quote:** 「若出目至少为15，则额外抽一张卡」
* **Trigger:** the same roll.
* **Condition:** face `>= 15`.
* **Effect:** the user draws exactly one card (hand net +1).
* **Frame:** only the user's hand/draw zones move.
* **Rulings & OPEN:** as above (15 inclusive).
* **Executable:** `test haneoka_thresholds_and_ineffective`.

### C-MYGO-haneoka-04

* **ID:** `C-MYGO-haneoka-04`
* **Source:** `cards.json#MyGO:羽丘的不可思议女孩`
* **Quote:** 「至少为20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃牌堆并抵消一次任意付款。」
* **Trigger:** the same roll, face `>= 20`; later, a payment cancel.
* **Condition:** face `>= 20`.
* **Effect:** the card is placed on the user's 场上 (not discarded). Later
  the user may discard it to cancel any one payment.
* **Frame:** the card stays on field until the cancel.
* **Rulings & OPEN:** the payment-cancel press is a later window; the
  contract covers the placement.
* **Executable:** `test haneoka_thresholds_and_ineffective`.

### C-MYGO-haneoka-05

* **ID:** `C-MYGO-haneoka-05`
* **Source:** `cards.json#MyGO:羽丘的不可思议女孩`
* **Quote:** 「若小于10，此卡放入弃牌堆且视为此卡未生效」
* **Trigger:** the same roll, face `< 10`.
* **Condition:** face `< 10`.
* **Effect:** the card goes to 弃牌堆 and is treated as 未生效 (no house,
  no draw, no field placement).
* **Frame:** only the user's zones move; houses/money/hand of the branches
  are unchanged.
* **Rulings & OPEN:** 「未生效」 also feeds the band skill (3) over-cap
  crystal.
* **Executable:** `test haneoka_thresholds_and_ineffective`.

---

## Card: `MyGO:若能再次交汇`

```text
若能再次交汇：
[反击]移动掷骰后，且场上有可被经过的玩家时可打出，持续进行移动掷骰直至[经过]下一名玩家（最多掷骰至移动超过原本移动终点的20格以后）。
```

### C-MYGO-meet-again-01

* **ID:** `C-MYGO-meet-again-01`
* **Source:** `cards.json#MyGO:若能再次交汇`
* **Quote:** 「[反击]移动掷骰后，且场上有可被经过的玩家时可打出」
* **Trigger:** a [反击] window after the move dice, with a passable player
  on the board.
* **Condition:** the user holds this card; some other player sits on a
  tile the remaining move could [经过].
* **Effect:** the card may be played in that window.
* **Frame:** the card leaves hand into discard.
* **Rulings & OPEN:** 「可被经过」 is not further defined; the contract
  arranges a player strictly ahead on the path.
* **Executable:** `test meet_again_rerolls_until_passing`.

### C-MYGO-meet-again-02

* **ID:** `C-MYGO-meet-again-02`
* **Source:** `cards.json#MyGO:若能再次交汇`
* **Quote:** 「持续进行移动掷骰直至[经过]下一名玩家（最多掷骰至移动超过原本移动终点的20格以后）」
* **Trigger:** the resolution of this [反击].
* **Condition:** the original move end is `E`; a player sits at `P`.
* **Effect:** the move dice keep rolling until the path [经过]s the next
  player, capped at moving past `E` by more than 20 tiles.
* **Frame:** only the mover's `pos` / dice stream move.
* **Rulings & OPEN:** the 20-tile cap is measured from the **original**
  end, per the quote.
* **Executable:** `test meet_again_rerolls_until_passing`.

---

## Card: `MyGO:轮符雨`

```text
轮符雨：
 使自己获得一层[停留]并在回合结束时额外进行一次[触发结算]
```

### C-MYGO-rinpuu-01

* **ID:** `C-MYGO-rinpuu-01`
* **Source:** `cards.json#MyGO:轮符雨`
* **Quote:** 「使自己获得一层[停留]」
* **Trigger:** the play of this card.
* **Condition:** none.
* **Effect:** the user gains exactly one layer of [停留].
* **Frame:** only `state:P{user}:stay` moves.
* **Rulings & OPEN:** [停留] = 无法移动 (rulebook-doc.md:96).
* **Executable:** `c_mygo_rinpuu_stay`, `test rinpuu_gives_one_stay`.

### C-MYGO-rinpuu-02

* **ID:** `C-MYGO-rinpuu-02`
* **Source:** `cards.json#MyGO:轮符雨`
* **Quote:** 「并在回合结束时额外进行一次[触发结算]」
* **Trigger:** the user's turn end.
* **Condition:** the card resolved this turn.
* **Effect:** one additional [触发结算] runs at turn end (beyond the normal
  end-step bookkeeping).
* **Frame:** the extra settle touches only the settled tile's effects.
* **Rulings & OPEN:** OPEN-触发结算 (see journey-04). The contract checks
  that a second settle window's effects fire (money/house on the end tile).
* **Executable:** `test rinpuu_extra_settle_at_turn_end`.

---

## Card: `MyGO:那天的雨`

```text
那天的雨：
 将此卡放置于自己场上并为其放置5个奇迹水晶，打出此卡时及你的每回合开始时投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]，每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃牌堆（骰点大于10则固定为高级住宅区对应颜色的格子）
```

### C-MYGO-rainday-01

* **ID:** `C-MYGO-rainday-01`
* **Source:** `cards.json#MyGO:那天的雨`
* **Quote:** 「将此卡放置于自己场上并为其放置5个奇迹水晶」
* **Trigger:** the play of this card.
* **Condition:** none.
* **Effect:** the card is on the user's 场上 with exactly 5 [奇迹水晶].
* **Frame:** only the user's field zones move.
* **Rulings & OPEN:** none.
* **Executable:** `c_mygo_rainday_place`, `test rainday_places_five_crystals`.

### C-MYGO-rainday-02

* **ID:** `C-MYGO-rainday-02`
* **Source:** `cards.json#MyGO:那天的雨`
* **Quote:** 「打出此卡时及你的每回合开始时投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]」
* **Trigger:** the play, and each of the owner's turn starts.
* **Condition:** the card is on the field.
* **Effect:** roll 1d10 = `n`; the `n`-th [地产商] in board order,
  **excluding 东京外**, names a colour; every player on a tile of that
  colour **or adjacent** to such a tile gains one layer of [停留].
* **Frame:** only `state:P*:stay` moves (plus the dice).
* **Rulings & OPEN:** agent order excluding 东京外 is the board order of
  `kind == "agent"` tiles other than 东京外 (主要街道, 中心学区, 周边学区,
  弦卷集团, 大学路, 梦开始的地方, 商店街, 高级住宅区, ... -- the [地产商]
  colour tiles). OPEN: 「骰点大于10则固定为高级住宅区」 cannot fire on a
  plain 1d10 (max 10); recorded as dead text, not invented.
* **Executable:** `test rainday_place_and_zone_stay`.

### C-MYGO-rainday-03

* **ID:** `C-MYGO-rainday-03`
* **Source:** `cards.json#MyGO:那天的雨`
* **Quote:** 「每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃牌堆」
* **Trigger:** a turn end.
* **Condition:** the card is on the field with `C > 0` crystals.
* **Effect:** exactly one crystal is removed; when the count hits 0 the
  card goes to 弃牌堆.
* **Frame:** only the card's crystal count / the owner's zones move.
* **Rulings & OPEN:** 「每回合结束时」 -- every player's turn end, not only
  the owner's (the quote does not scope it to 「你的」).
* **Executable:** `test rainday_drains_one_crystal_per_turn_end`.

### C-MYGO-rainday-04

* **ID:** `C-MYGO-rainday-04`
* **Source:** `cards.json#MyGO:那天的雨`
* **Quote:** 「（骰点大于10则固定为高级住宅区对应颜色的格子）」
* **Trigger:** the 1d10 roll of -02.
* **Condition:** the face is `> 10`.
* **Effect:** the zone is fixed to the 高级住宅区 colour.
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-rainday-d10 -- a 1d10 face is 1..=10, so the
  condition is unreachable without a dice modifier. Not invented.
* **Executable:** untestable on a plain 1d10; documented only.

---

## Card: `MyGO:难以复刻的奇迹`

```text
难以复刻的奇迹：

（1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上，当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。
（2）[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡进入弃牌堆。
（3）若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。
```

### C-MYGO-miracle-01

* **ID:** `C-MYGO-miracle-01`
* **Source:** `cards.json#MyGO:难以复刻的奇迹`
* **Quote:** 「（1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上」
* **Trigger:** the [手] play of this card.
* **Condition:** the user's band-skill field card holds `B` crystals.
* **Effect:** the card is on 场上; the band card's crystal count goes to 0;
  this card receives all `B` of them.
* **Frame:** only the two field cards' crystal counts / the hand move.
* **Rulings & OPEN:** 「乐队技能」 is the band-skill field card
  (`skill:*` with `band_skill`). OPEN-乐队卡 (the card class is undefined);
  the clause is written against the bound band skill instance.
* **Executable:** `c_mygo_miracle_place`, `test miracle_transfers_band_crystals_on_play`.

### C-MYGO-miracle-02

* **ID:** `C-MYGO-miracle-02`
* **Source:** `cards.json#MyGO:难以复刻的奇迹`
* **Quote:** 「当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费。」
* **Trigger:** a 加盖 (build) action while this card holds `>= 1` crystal.
* **Condition:** the card is on field with `C >= 1`.
* **Effect:** the build may remove exactly one crystal instead of paying
  the house cost; the user's money is unchanged by the build cost.
* **Frame:** only `crystals:P{user}:<card>` and `houses:T{tile}` move.
* **Rulings & OPEN:** 「加盖动作」 is the build action (基础[结算]).
* **Executable:** `test miracle_build_spends_one_crystal_not_money`.

### C-MYGO-miracle-03

* **ID:** `C-MYGO-miracle-03`
* **Source:** `cards.json#MyGO:难以复刻的奇迹`
* **Quote:** 「（2）[持续] 当你[经过]场上的所有玩家各一次，获得相当于场上奇迹水晶数与你最相近的玩家资金后三位的资金，然后此卡进入弃牌堆。」
* **Trigger:** a [经过] stream in which the user has passed every other
  living player at least once.
* **Condition:** this card is on field; let `Q` be the player whose total
  field [奇迹水晶] count is closest to the user's.
* **Effect:** the user gains money equal to the last three digits of `Q`'s
  money, then this card goes to 弃牌堆.
* **Frame:** only `money:P{user}` and the owner's zones move.
* **Rulings & OPEN:** OPEN-miracle-pct -- 「资金后三位」 is ambiguous
  (last 3 decimal digits of the money amount vs. some other reading). The
  clause specifies only the determined structure: pick nearest-crystal
  player `Q`, then take `Q.money % 1000`. 「最相近」 ties are unspecified.
* **Executable:** `test miracle_pass_everyone_pays_last_three_digits`.

### C-MYGO-miracle-04

* **ID:** `C-MYGO-miracle-04`
* **Source:** `cards.json#MyGO:难以复刻的奇迹`
* **Quote:** 「（3）若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效。」
* **Trigger:** this card entering 弃牌堆.
* **Condition:** its crystal count is `> 0`.
* **Effect:** the card is treated as 未生效 (its (2) gain did not land).
* **Frame:** n/a (the (2) gain is suppressed).
* **Rulings & OPEN:** 「未生效」 feeds the band skill (3) over-cap crystal.
* **Executable:** `test miracle_transfers_band_crystals_and_ineffective_discard`.

---

## Card: `MyGO:（soyo）混合的颜色`

```text
（soyo）混合的颜色：
 将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。
```

### C-MYGO-soyo-mixed-01

* **ID:** `C-MYGO-soyo-mixed-01`
* **Source:** `cards.json#MyGO:（soyo）混合的颜色`
* **Quote:** 「将此卡放置于你拥有地契的一个格子」
* **Trigger:** the play of this card.
* **Condition:** the user owns a deed on some tile `T`.
* **Effect:** the card is placed associated with `T`.
* **Frame:** only the user's field / hand zones move.
* **Rulings & OPEN:** exclusive to 长崎素世 (`characters.json#长崎素世`).
* **Executable:** `test soyo_mixed_attaches_and_halves_agent_charge`.

### C-MYGO-soyo-mixed-02

* **ID:** `C-MYGO-soyo-mixed-02`
* **Source:** `cards.json#MyGO:（soyo）混合的颜色`
* **Quote:** 「该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房）」
* **Trigger:** a colour-group query or a developer build on `T`.
* **Condition:** the card is on `T`.
* **Effect:** `T` is in every colour group for settle purposes; `T` itself
  cannot be built on via a developer of a colour the user does not own.
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-agent-colour (TEST-FINDINGS §6 #10) -- does
  「该格获得所有颜色」 widen the colour group for the 「同色」 developer
  gate? Not decided; the clause only records the printed restriction.
* **Executable:** untestable without the colour-group ruling; documented.

### C-MYGO-soyo-mixed-03

* **ID:** `C-MYGO-soyo-mixed-03`
* **Source:** `cards.json#MyGO:（soyo）混合的颜色`
* **Quote:** 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。」
* **Trigger:** a player settles a [地产商] of a colour that is not the
  card's own colour, and the mixed-colour card causes that charge.
* **Condition:** the settle is on a developer tile of another colour.
* **Effect:** the charge is halved again on top of the developer's own
  half-price charge (¼ of the underlying rent, the agent rounding noted in
  the global set).
* **Frame:** only `money:*` of the settler and payee move.
* **Rulings & OPEN:** the developer half-rate (向上取整10) is G-SETTLE's;
  this clause stacks one more halving.
* **Executable:** `test soyo_mixed_attaches_and_halves_agent_charge`.

---

## Card: `MyGO:（乐奈）有趣的女人`

```text
（乐奈）有趣的女人：
将此卡置于当前格子上，每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶，当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个“抹茶芭菲”或强制停下并[触发结算]，如果强制停下则此卡洗入弃牌堆。 由此卡效果导致[触发结算]时需支付资金减半
```

### C-MYGO-rana-fun-01

* **ID:** `C-MYGO-rana-fun-01`
* **Source:** `cards.json#MyGO:（乐奈）有趣的女人`
* **Quote:** 「将此卡置于当前格子上」
* **Trigger:** the play of this card.
* **Condition:** none.
* **Effect:** the card is placed on the current tile.
* **Frame:** only the user's hand/field zones move.
* **Rulings & OPEN:** exclusive to 要乐奈.
* **Executable:** `test rana_fun_place_and_crystal_on_pass`.

### C-MYGO-rana-fun-02

* **ID:** `C-MYGO-rana-fun-02`
* **Source:** `cards.json#MyGO:（乐奈）有趣的女人`
* **Quote:** 「每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶」
* **Trigger:** a player [经过]s the card's tile without [触发结算] on it.
* **Condition:** the card is on the field.
* **Effect:** the card gains exactly one [奇迹水晶].
* **Frame:** only the card's crystal count moves.
* **Rulings & OPEN:** a landing settle on the tile does **not** add a
  crystal (the quote excludes it).
* **Executable:** `test rana_fun_place_and_crystal_on_pass`.

### C-MYGO-rana-fun-03

* **ID:** `C-MYGO-rana-fun-03`
* **Source:** `cards.json#MyGO:（乐奈）有趣的女人`
* **Quote:** 「当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家选择失去一个“抹茶芭菲”或强制停下并[触发结算]」
* **Trigger:** the next [经过] by a player other than the owner, after
  `crystals >= 5`.
* **Condition:** `crystals >= 5`; the passer is not the owner.
* **Effect:** that player chooses: lose one 抹茶芭菲, or [强制停下] and
  [触发结算] the card's tile.
* **Frame:** the choice path touches the passer's tokens/pos/money.
* **Rulings & OPEN:** OPEN-强制停下 (definition truncated at
  rulebook-doc.md:93). The clause checks the two-choice prompt and the
  chosen branch's end state.
* **Executable:** `test rana_fun_choice_and_force_stop`.

### C-MYGO-rana-fun-04

* **ID:** `C-MYGO-rana-fun-04`
* **Source:** `cards.json#MyGO:（乐奈）有趣的女人`
* **Quote:** 「如果强制停下则此卡洗入弃牌堆。」
* **Trigger:** the 强制停下 branch of -03.
* **Condition:** the passer chose 强制停下.
* **Effect:** this card is shuffled into 弃牌堆 (leaves the tile).
* **Frame:** only the owner's zones move.
* **Rulings & OPEN:** none.
* **Executable:** `test rana_fun_choice_and_force_stop`.

### C-MYGO-rana-fun-05

* **ID:** `C-MYGO-rana-fun-05`
* **Source:** `cards.json#MyGO:（乐奈）有趣的女人`
* **Quote:** 「由此卡效果导致[触发结算]时需支付资金减半」
* **Trigger:** a [触发结算] this card caused.
* **Condition:** the settle was forced by this card.
* **Effect:** every [支付] of that settle is halved.
* **Frame:** only `money:*` of the settle parties move.
* **Rulings & OPEN:** 「需支付资金」 is the payment the settler owes.
* **Executable:** `test rana_fun_forced_settle_pays_half`.

---

## Card: `MyGO:（灯）不再迷茫`

```text
（灯）不再迷茫：
（1）获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数
（2）你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗，当此卡上的水晶由于此效果以外的原因减少时，此卡立刻获得等同于减少量的[奇迹水晶]
（3）此卡上的奇迹水晶耗尽后，[移除]此卡
```

### C-MYGO-tomori-card-01

* **ID:** `C-MYGO-tomori-card-01`
* **Source:** `cards.json#MyGO:（灯）不再迷茫`
* **Quote:** 「（1）获得两层仅能被自然流失效果移除的[停留]」
* **Trigger:** the play of this card.
* **Condition:** none.
* **Effect:** the user gains exactly 2 layers of [停留] that only natural
  decay (turn-end ticks) may remove.
* **Frame:** only `state:P{user}:stay` moves.
* **Rulings & OPEN:** exclusive to 高松灯. 「自然流失效果」 is not a
  glossary term; the clause checks the layer count and that ordinary
  removers do not strip them (see the black-box suite).
* **Executable:** `test tomori_card_place_x_plus_1_and_remove`.

### C-MYGO-tomori-card-02

* **ID:** `C-MYGO-tomori-card-02`
* **Source:** `cards.json#MyGO:（灯）不再迷茫`
* **Quote:** 「将此卡放置于场上并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数」
* **Trigger:** the same play.
* **Condition:** the user discards `X` hand cards.
* **Effect:** the card is on 场上; the hand is empty; the card holds
  exactly `X + 1` [奇迹水晶].
* **Frame:** only the user's zones / the card's crystals move.
* **Rulings & OPEN:** none.
* **Executable:** `c_mygo_tomori_card_place`, `test tomori_card_x_plus_1_crystals`.

### C-MYGO-tomori-card-03

* **ID:** `C-MYGO-tomori-card-03`
* **Source:** `cards.json#MyGO:（灯）不再迷茫`
* **Quote:** 「（2）你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗」
* **Trigger:** a character-skill press that costs 火罐.
* **Condition:** this card holds `>= 1` crystal.
* **Effect:** one crystal may be removed instead of the fire-pot cost;
  the fire count is unchanged by the skill's cost.
* **Frame:** only `crystals:P{user}:<card>` moves for the cost.
* **Rulings & OPEN:** none.
* **Executable:** `test tomori_card_crystal_pays_fire_and_removes`.

### C-MYGO-tomori-card-04

* **ID:** `C-MYGO-tomori-card-04`
* **Source:** `cards.json#MyGO:（灯）不再迷茫`
* **Quote:** 「当此卡上的水晶由于此效果以外的原因减少时，此卡立刻获得等同于减少量的[奇迹水晶]」
* **Trigger:** a crystal decrease on this card that is **not** the -03
  skill-cost substitution.
* **Condition:** the card is on field.
* **Effect:** the card immediately regains the same number of crystals
  (net zero for that decrease).
* **Frame:** only the card's crystal count moves.
* **Rulings & OPEN:** none.
* **Executable:** `test tomori_card_crystal_pays_fire_and_removes`.

### C-MYGO-tomori-card-05

* **ID:** `C-MYGO-tomori-card-05`
* **Source:** `cards.json#MyGO:（灯）不再迷茫`
* **Quote:** 「（3）此卡上的奇迹水晶耗尽后，[移除]此卡」
* **Trigger:** this card's crystal count reaching 0.
* **Condition:** the card is on field.
* **Effect:** the card is [移除]d from the game (not discarded into a pile).
* **Frame:** only the owner's field zones move.
* **Rulings & OPEN:** [移除] = remove from the game (rulebook-doc.md:74).
* **Executable:** `test tomori_card_crystal_pays_fire_and_removes`.

---

## Card: `MyGO:（立希）想认真去做`

```text
（立希）想认真去做：
 使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为原价的四分之一
```

### C-MYGO-rikki-act-01

* **ID:** `C-MYGO-rikki-act-01`
* **Source:** `cards.json#MyGO:（立希）想认真去做`
* **Quote:** 「使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]」
* **Trigger:** the play of this card.
* **Condition:** a set `S` of players holds [停留]; each sits at `p`.
* **Effect:** every player in `S` immediately [触发结算]s one tile of the
  user's choice within `[p-2, p+2]`. Players without [停留] are untouched.
* **Frame:** only the `S` players' `pos` (to the chosen tile) and that
  tile's settle effects move.
* **Rulings & OPEN:** exclusive to 椎名立希. OPEN-触发结算 as usual.
* **Executable:** `test rikki_act_stay_holders_quarter_pay`.

### C-MYGO-rikki-act-02

* **ID:** `C-MYGO-rikki-act-02`
* **Source:** `cards.json#MyGO:（立希）想认真去做`
* **Quote:** 「本次结算导致的所有[支付]变为原价的四分之一」
* **Trigger:** the same forced settles.
* **Condition:** a [支付] of amount `P` would arise.
* **Effect:** that payment becomes `P / 4` (the quote's 「四分之一」).
* **Frame:** only `money:*` of the settle parties move.
* **Rulings & OPEN:** rounding of `P/4` is not specified; the contract
  uses a rent divisible by 4 (60 -> 15).
* **Executable:** `test rikki_act_stay_holders_quarter_pay`.

---

## Character skill: 高松灯 「诗超绊」

Source: `data/characters.json#高松灯`.

```text
（1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）
（2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家[触发结算]前强制将该玩家[传送]至自己所在的格子，然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半。
（3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在[触发结算]前将自己[传送]至那名玩家所在的格子。
```

### C-MYGO-tomori-skill-01

* **ID:** `C-MYGO-tomori-skill-01`
* **Source:** `characters.json#高松灯`
* **Quote:** 「（1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）」
* **Trigger:** a [经过] of any RiNG tile by 高松灯.
* **Condition:** fire count `F < 4` (cap).
* **Effect:** `F` rises by exactly 1; the pot starts at 4 and max is 4.
* **Frame:** only `state:P{user}:fire` moves.
* **Rulings & OPEN:** [经过] is the whole move path (rulebook-doc.md:91),
  so a RiNG merely passed through counts.
* **Executable:** `test tomori_skill_fire_on_passing_ring`.

### C-MYGO-tomori-skill-02

* **ID:** `C-MYGO-tomori-skill-02`
* **Source:** `characters.json#高松灯`
* **Quote:** 「（2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家[触发结算]前强制将该玩家[传送]至自己所在的格子」
* **Trigger:** another player's [移动终点] lands within ±5 of 高松灯.
* **Condition:** 高松灯 holds `>= 4` fire pots; the window is before that
  player's [触发结算].
* **Effect:** spending 4 pots force-[传送]s that player to 高松灯's tile,
  before their settle.
* **Frame:** only the target's `pos` and the fire count move.
* **Rulings & OPEN:** the ±5 window is inclusive distance on the ring
  board (not path length).
* **Executable:** `test tomori_skill_teleport_windows`.

### C-MYGO-tomori-skill-03

* **ID:** `C-MYGO-tomori-skill-03`
* **Source:** `characters.json#高松灯`
* **Quote:** 「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半。」
* **Trigger:** the forced teleport of -02 resolves.
* **Condition:** the target rolls 1d20 = `D`.
* **Effect:** if `D <= 6` the target gains one [停留] and settles at the
  end of their next turn; payments under this skill are half price.
* **Frame:** only the target's state / money (halved pay) move.
* **Rulings & OPEN:** 「自己的下回合结束时」 -- the target's next turn end
  (R-7 extra-turn interaction is OPEN in TEST-FINDINGS).
* **Executable:** `test tomori_skill_teleport_windows`.

### C-MYGO-tomori-skill-04

* **ID:** `C-MYGO-tomori-skill-04`
* **Source:** `characters.json#高松灯`
* **Quote:** 「（3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在[触发结算]前将自己[传送]至那名玩家所在的格子。」
* **Trigger:** 高松灯's own [移动终点] within ±5 of another player.
* **Condition:** 高松灯 holds `>= 1` fire pot; window is before settle.
* **Effect:** spending 1 pot [传送]s 高松灯 to that player's tile before
  settle.
* **Frame:** only 高松灯's `pos` and fire count move.
* **Rulings & OPEN:** none.
* **Executable:** `test tomori_skill_teleport_windows`.

---

## Character skill: 千早爱音 「重新开始」

Source: `data/characters.json#千早爱音`.

```text
（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限3）
（2）在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡
```

### C-MYGO-anon-skill-01

* **ID:** `C-MYGO-anon-skill-01`
* **Source:** `characters.json#千早爱音`
* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限3）」
* **Trigger:** a [经过] of CiRCLE by 千早爱音.
* **Condition:** fire `F < 3`.
* **Effect:** `F` rises by exactly 1; initial 2, max 3.
* **Frame:** only `state:P{user}:fire` moves.
* **Rulings & OPEN:** none.
* **Executable:** `test anon_skill_fire_on_passing_circle`.

### C-MYGO-anon-skill-02

* **ID:** `C-MYGO-anon-skill-02`
* **Source:** `characters.json#千早爱音`
* **Quote:** 「（2）在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡」
* **Trigger:** after a [停留] or [传送] effect has affected the user and
  that effect has settled.
* **Condition:** the user holds `>= 2` fire pots.
* **Effect:** spending 2 pots draws exactly one card.
* **Frame:** only fire count and the user's hand/draw zones move.
* **Rulings & OPEN:** the sheet's 「谁的效果都算，回合内外都算」 (simple
  column) is descriptive; the quote itself is unscoped.
* **Executable:** `test anon_skill_draw_after_abnormal_resolves`.

---

## Character skill: 要乐奈 「投币式停车场的猫」

Source: `data/characters.json#要乐奈`.

```text
（1）每次[经过]CiRCLE时获得一个[火罐]（初始3，上限3）
（2）可花费3个火罐传送至space代替本回合的移动，该次传送不可进行地契购买。在space上结算时，若space已属于其他玩家，免除付款并抽1张卡；若自己为space的拥有者，则可选择将该次结算改为在space格子上添加一个“抹茶芭菲”，每个将使其收费增加500资金，其他玩家在space触发结算时可将自己拥有的一个“抹茶芭菲”转移到该格上以免除当次付款
（3）你与其他玩家重合时，获得其800资金并使那个玩家获得一个“抹茶芭菲”；拥有“抹茶芭菲”的玩家经过任意“RiNG”时可使用一个“抹茶芭菲”获得400资金
```

### C-MYGO-rana-skill-01

* **ID:** `C-MYGO-rana-skill-01`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始3，上限3）」
* **Trigger:** a [经过] of CiRCLE by 要乐奈.
* **Condition:** fire `F < 3`.
* **Effect:** `F` rises by exactly 1; initial 3, max 3.
* **Frame:** only fire moves.
* **Rulings & OPEN:** none.
* **Executable:** `test rana_skill_fire_on_passing_circle`.

### C-MYGO-rana-skill-02

* **ID:** `C-MYGO-rana-skill-02`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「（2）可花费3个火罐传送至space代替本回合的移动，该次传送不可进行地契购买。」
* **Trigger:** the skill press in place of the main move.
* **Condition:** fire `>= 3`.
* **Effect:** spending 3 pots [传送]s to Space (tile name `Space`); that
  teleport does not allow a deed purchase.
* **Frame:** only `pos` / fire move; no `owners` change from a buy.
* **Rulings & OPEN:** none.
* **Executable:** `test rana_skill_space_and_overlap`.

### C-MYGO-rana-skill-03

* **ID:** `C-MYGO-rana-skill-03`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「在space上结算时，若space已属于其他玩家，免除付款并抽1张卡」
* **Trigger:** a settle on Space owned by another player.
* **Condition:** Space's owner is not the user.
* **Effect:** the user pays nothing and draws exactly one card.
* **Frame:** only the user's hand/draw move; money is unchanged.
* **Rulings & OPEN:** none.
* **Executable:** `test rana_skill_space_and_overlap`.

### C-MYGO-rana-skill-04

* **ID:** `C-MYGO-rana-skill-04`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「若自己为space的拥有者，则可选择将该次结算改为在space格子上添加一个“抹茶芭菲”，每个将使其收费增加500资金」
* **Trigger:** a settle on Space owned by the user.
* **Condition:** the user owns Space.
* **Effect:** the user may add a 抹茶芭菲 token to Space instead; each
  token raises Space's charge by exactly 500.
* **Frame:** only Space's charge-relevant tokens move.
* **Rulings & OPEN:** 「抹茶芭菲」 is a token (characters.json), not a
  glossary term.
* **Executable:** `test rana_skill_parfait_on_space`.

### C-MYGO-rana-skill-05

* **ID:** `C-MYGO-rana-skill-05`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「其他玩家在space触发结算时可将自己拥有的一个“抹茶芭菲”转移到该格上以免除当次付款」
* **Trigger:** another player's settle on Space.
* **Condition:** that player owns a 抹茶芭菲.
* **Effect:** moving one of their tokens to Space exempts that payment.
* **Frame:** only tokens / the settler's money (unchanged) move.
* **Rulings & OPEN:** none.
* **Executable:** `test rana_skill_parfait_on_space`.

### C-MYGO-rana-skill-06

* **ID:** `C-MYGO-rana-skill-06`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「（3）你与其他玩家重合时，获得其800资金并使那个玩家获得一个“抹茶芭菲”」
* **Trigger:** a [重叠] (the user ends a move on another player's tile).
* **Condition:** another player occupies the user's landing tile.
* **Effect:** the user gains exactly 800 from that player (pay 800); that
  player gains one 抹茶芭菲.
* **Frame:** only `money` of the two and the other's tokens move.
* **Rulings & OPEN:** the live sheet says 800 (the older 行动阶段 example
  said 1000; sheet wins).
* **Executable:** `test rana_skill_space_and_overlap`.

### C-MYGO-rana-skill-07

* **ID:** `C-MYGO-rana-skill-07`
* **Source:** `characters.json#要乐奈`
* **Quote:** 「拥有“抹茶芭菲”的玩家经过任意“RiNG”时可使用一个“抹茶芭菲”获得400资金」
* **Trigger:** a [经过] of any RiNG by a player holding a 抹茶芭菲.
* **Condition:** that player owns `>= 1` 抹茶芭菲.
* **Effect:** spending one token gains exactly 400 money.
* **Frame:** only the passer's money / tokens move.
* **Rulings & OPEN:** none.
* **Executable:** `test rana_skill_parfait_on_ring_gains_400`.

---

## Character skill: 长崎素世 「通透的颜色」

Source: `data/characters.json#长崎素世`.

```text
（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
（2）移动掷骰后可消耗一个火罐，使自己本回合的[移动终点]对你视为对应颜色的地产商格子，持续至你的下回合开始；若以此法单次免除了至少1500资金的[支付]，则你下次经过CiRCLE时不获得火罐。
```

### C-MYGO-soyo-skill-01

* **ID:** `C-MYGO-soyo-skill-01`
* **Source:** `characters.json#长崎素世`
* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
* **Trigger:** a [经过] of CiRCLE by 长崎素世.
* **Condition:** fire `F < 1` (i.e. 0).
* **Effect:** `F` rises by exactly 1; initial 1, max 1.
* **Frame:** only fire moves.
* **Rulings & OPEN:** none.
* **Executable:** `test soyo_skill_fire_on_passing_circle`.

### C-MYGO-soyo-skill-02

* **ID:** `C-MYGO-soyo-skill-02`
* **Source:** `characters.json#长崎素世`
* **Quote:** 「（2）移动掷骰后可消耗一个火罐，使自己本回合的[移动终点]对你视为对应颜色的地产商格子，持续至你的下回合开始」
* **Trigger:** after the move dice.
* **Condition:** fire `>= 1`.
* **Effect:** spending 1 pot makes this turn's [移动终点] count as the
  corresponding-colour [地产商] tile **for you**, until your next turn
  starts.
* **Frame:** only fire / the settle-target interpretation move.
* **Rulings & OPEN:** OPEN-agent-colour (TEST-FINDINGS §6 #10) for the
  colour-group widening; the clause checks the fire spend and the
  until-next-turn window on the determined part.
* **Executable:** `test soyo_skill_endpoint_as_coloured_agent`.

### C-MYGO-soyo-skill-03

* **ID:** `C-MYGO-soyo-skill-03`
* **Source:** `characters.json#长崎素世`
* **Quote:** 「若以此法单次免除了至少1500资金的[支付]，则你下次经过CiRCLE时不获得火罐。」
* **Trigger:** a single use of -02 that exempted `>= 1500` of [支付].
* **Condition:** the exempted payment total `>= 1500`.
* **Effect:** the next CiRCLE [经过] grants no fire pot.
* **Frame:** only fire moves (or does not).
* **Rulings & OPEN:** 「单次免除」 is the one skill use, not a cumulative
  total across turns.
* **Executable:** `test soyo_skill_endpoint_as_coloured_agent`.

---

## Character skill: 椎名立希 「决定练习日的会议」

Source: `data/characters.json#椎名立希`.

```text
（1）场上每有玩家获得一层[停留]时，你获得一个[火罐]（初始0，上限5）
（2）运营阶段可消耗1个[火罐]为你场上的任何一张卡添加一个[奇迹水晶]，每回合限一次。
```

### C-MYGO-rikki-skill-01

* **ID:** `C-MYGO-rikki-skill-01`
* **Source:** `characters.json#椎名立希`
* **Quote:** 「（1）场上每有玩家获得一层[停留]时，你获得一个[火罐]（初始0，上限5）」
* **Trigger:** any player gaining one layer of [停留].
* **Condition:** 椎名立希's fire `F < 5`.
* **Effect:** `F` rises by exactly 1 per such layer; initial 0, max 5.
* **Frame:** only `state:P{rikki}:fire` moves.
* **Rulings & OPEN:** 「场上每有玩家」 includes the skill holder themself.
* **Executable:** `test rikki_skill_fire_on_any_stay_gain`.

### C-MYGO-rikki-skill-02

* **ID:** `C-MYGO-rikki-skill-02`
* **Source:** `characters.json#椎名立希`
* **Quote:** 「（2）运营阶段可消耗1个[火罐]为你场上的任何一张卡添加一个[奇迹水晶]，每回合限一次。」
* **Trigger:** the skill press in 运营.
* **Condition:** fire `>= 1`; not yet used this turn.
* **Effect:** spending 1 pot adds exactly one [奇迹水晶] to one field card
  of the user's choice. A second use in the same turn is refused.
* **Frame:** only fire / that card's crystals move.
* **Rulings & OPEN:** none.
* **Executable:** `test rikki_skill_adds_crystal_once_per_turn`.

---

## Band skill: MyGO!!!!! 「迷途之星」

Source: `data/bands.json#MyGO!!!!!`.

```text
（1）开局时投掷3d20，并取出目作为你本局游戏的起始点
（2）若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。你的回合中，可于移动掷骰前选择移动1格以替代移动掷骰并移除一个[奇迹水晶]
（3）每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]（最多超出2个）；你的回合中，可移除此卡的两个[奇迹水晶]以抽一张卡。
```

### C-MYGO-band-01

* **ID:** `C-MYGO-band-01`
* **Source:** `bands.json#MyGO!!!!!`
* **Quote:** 「（1）开局时投掷3d20，并取出目作为你本局游戏的起始点」
* **Trigger:** game start for a MyGO!!!!! player.
* **Condition:** none.
* **Effect:** the player's starting tile is the 3d20 sum (3..=60 on a
  60-tile board, i.e. the face sum itself).
* **Frame:** only `pos` at setup.
* **Rulings & OPEN:** the board is 60 tiles; 「取出目」 is the sum of the
  three dice (the only reading that names a single start tile).
* **Executable:** `test band_start_tile_is_the_opening_3d20`.

### C-MYGO-band-02

* **ID:** `C-MYGO-band-02`
* **Source:** `bands.json#MyGO!!!!!`
* **Quote:** 「（2）若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
* **Trigger:** a move dice result `>= 16`.
* **Condition:** the band card's crystal count `C < 1`.
* **Effect:** the band card gains exactly one crystal; the roll gain is
  capped at 1.
* **Frame:** only `crystals:P{user}:<band>` moves.
* **Rulings & OPEN:** the cap (上限1) applies to this roll gain; the (3)
  over-cap crystals are a different clause.
* **Executable:** `test band_crystals_move_and_draw`.

### C-MYGO-band-03

* **ID:** `C-MYGO-band-03`
* **Source:** `bands.json#MyGO!!!!!`
* **Quote:** 「你的回合中，可于移动掷骰前选择移动1格以替代移动掷骰并移除一个[奇迹水晶]」
* **Trigger:** the skill press before the move dice on the owner's turn.
* **Condition:** the band card holds `>= 1` crystal.
* **Effect:** the owner moves exactly 1 tile (no dice), and exactly one
  crystal is removed.
* **Frame:** only `pos` / band crystals move.
* **Rulings & OPEN:** none.
* **Executable:** `test band_crystals_move_and_draw`.

### C-MYGO-band-04

* **ID:** `C-MYGO-band-04`
* **Source:** `bands.json#MyGO!!!!!`
* **Quote:** 「（3）每次你的卡在未生效的情况下进入弃牌堆时，为此卡添加一个可超出上限的[奇迹水晶]（最多超出2个）」
* **Trigger:** one of the owner's cards entering 弃牌堆 as 未生效.
* **Condition:** the band card's over-cap count `O < 2`.
* **Effect:** the band card gains exactly one crystal that may exceed the
  roll cap, at most 2 over.
* **Frame:** only band crystals move.
* **Rulings & OPEN:** 「未生效」 appears on 羽丘 (<10) and 难以复刻的奇迹
  (3).
* **Executable:** `test band_ineffective_card_adds_over_cap_crystal`.

### C-MYGO-band-05

* **ID:** `C-MYGO-band-05`
* **Source:** `bands.json#MyGO!!!!!`
* **Quote:** 「你的回合中，可移除此卡的两个[奇迹水晶]以抽一张卡。」
* **Trigger:** the skill press on the owner's turn (draw half).
* **Condition:** the band card holds `>= 2` crystals.
* **Effect:** exactly two crystals are removed; the owner draws exactly
  one card.
* **Frame:** only band crystals / hand-draw zones move.
* **Rulings & OPEN:** OPEN-multi-activation (TEST-FINDINGS §6 #8) -- the
  engine may merge (2)'s move half and (3)'s draw half into one press
  entry. The clause checks the draw half's numbers whenever it runs.
* **Executable:** `test band_crystals_move_and_draw`.

---

## Open questions

* **OPEN-t04** -- Anon Tokyo link and a mortgaged partner tile's 收费
  (TEST-FINDINGS §6 t04). The link clause only pins the unmortgaged case.
* **OPEN-G13** -- 普通与理所当然: does the copied 移动格数 keep a sign
  (backward move)? The clause checks the absolute step count.
* **OPEN-触发结算** -- [触发结算] is not defined in the glossary; clauses
  that name it specify only the settle-after-move reading.
* **OPEN-强制停下** -- the glossary definition is truncated
  (rulebook-doc.md:93). rana-fun-03/04 only check the two-choice prompt and
  the discard on the stop branch.
* **OPEN-乐队卡** -- 「乐队技能」 as a card class is undefined; miracle-01
  is written against the bound band-skill field instance.
* **OPEN-miracle-pct** -- 「资金后三位」 is ambiguous; miracle-03 pins
  `money % 1000` of the nearest-crystal player as the determined reading.
* **OPEN-agent-colour** -- 「该格获得所有颜色」 / soyo-skill-02 colour-group
  widening (TEST-FINDINGS §6 #10) is undecided.
* **OPEN-rainday-d10** -- 「骰点大于10」 on a 1d10 is unreachable without a
  modifier; rainday-04 is documented, not invented.
* **OPEN-stay-midturn** -- glossary says [停留]=无法移动; whether a
  mid-turn [停留] holder may finish an already-started main move is a
  global G-STATUS question (black-box suite records a gap).

## Findings

Genuine book-vs-engine discrepancies (plain failing tests, kept red):

* **C-MYGO-tomori-card-03** -- 「你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替
  此次技能的火罐消耗」. Book: one crystal of （灯）不再迷茫 pays a character skill's
  火罐 cost. Engine: spends the fire pot (or refuses) and leaves the crystal
  count untouched. Test: `tomori_card_crystal_pays_fire_and_removes`.
  (Matches the black-box suite's `light_crystal_pays_skill_fire_cost`.)
* **C-MYGO-anon-skill-02** -- 「在受到[停留][传送]效果影响并结算该效果后，可以消耗
  2个火罐，抽一张卡」. Book: after another's [停留]/[传送] settles on you, spend 2
  fire pots to draw 1. Engine: the skill press is accepted but neither spends
  the pots nor draws. Test: `anon_skill_draw_after_abnormal_resolves`.
* **C-MYGO-rana-skill-03** -- 「在space上结算时，若space已属于其他玩家，免除付款
  并抽1张卡」. Book: foreign-owned Space settles free and draws 1. Engine: draws
  (`rana_parking_free`) but still charges rent (observed 400 with 1 house).
  Test: `rana_skill_space_and_overlap`.
* **C-MYGO-rana-fun-03** -- 「当奇迹水晶总数为5或以上时使下一个经过的你以外的玩家
  选择失去一个“抹茶芭菲”或强制停下并[触发结算]」. Book: the passer **chooses**.
  Engine: auto-resolves the 强制停下 branch (`rana_funny_stop`) with no
  「选择」 prompt. Test: `rana_fun_choice_and_force_stop`.
* **C-MYGO-hitoshizuku-02** -- 「所有玩家因本效果每清除一种效果则支付此卡使用者
  1000资金」. Ruling (TEST-FINDINGS §6): 1000 per effect type per affected
  player (stay=1 + stun=1 -> 2000). Engine: charged 3000 for that shape
  (looks like 1000 per cleared **layer** globally). Test:
  `hitoshizuku_clear_charge_and_stun_play`.
* **C-MYGO-band-04** -- 「为此卡添加一个可超出上限的[奇迹水晶]（最多超出2个）」.
  Book: an 未生效 card adds a crystal that **may exceed** the roll cap of 1,
  up to 1+2. Engine: from a full cap of 1 the gain does not land (stays 1).
  Test: `band_ineffective_card_adds_over_cap_crystal`.
* **C-MYGO-murosshi-04** -- 「[除外]期间本应获得的格子收入由此前指定的那名玩家
  获得」. Book: rent the [除外]d user would have received is redirected to the
  designated player. Engine: the foreign settler pays 0 (the rent vanishes
  instead of redirecting). Test: `murosshi_income_goes_to_designated_player`.
* **C-MYGO-normal-02** -- 「使你下一次主要移动的格数变为移动你最近一次非传送的
  主要移动的移动格数」. Book: the next main move takes exactly the last
  non-teleport main move's step count (4 in the contract). Engine: the move
  took 9 steps (the copy did not pin the count). Test:
  `normal_copies_last_main_move_steps`.
* **C-MYGO-soyo-skill-02** -- 「移动掷骰后可消耗一个火罐…」. Book: the post-roll
  press spends 1 fire pot. Engine: the press is accepted with no pot spent
  (same shape as anon-skill-02). Test: `soyo_skill_endpoint_as_coloured_agent`.
* **C-MYGO-rana-skill-04** -- 「若自己为space的拥有者，则可选择将该次结算改为在
  space格子上添加一个“抹茶芭菲”」. Book: the own-Space settle may add a 抹茶芭菲.
  Engine: no token is placed on the skill-teleport settle. Test:
  `rana_skill_parfait_on_space`.
