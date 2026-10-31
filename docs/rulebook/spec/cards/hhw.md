# C-HHW -- Hello, Happy World! (15 cards + 5 character skills + 1 band skill)

Unit 13 owns this file and `rules/tests/tests/spec_card_hhw.rs`. Clause IDs are
`C-HHW-<slug>-<n>`.

Sources (precedence): live sheet (`docs/rulebook/cards.json`,
`data/characters.json` `text`, `data/bands.json` `text`) >
`docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not authoritative.
TEST-FINDINGS §6 rulings unless superseded by newer sheet text (ignore tabs
marked 旧 / 原).

Board facts used below (`data/board.json`, 「#N格」 = index `N-1`):
CiRCLE `#1` (idx 0), 水族馆 `#4` (idx 3), 弦卷集团 `#29` (idx 28),
弦卷豪宅 `#30` (idx 29), 微笑号 `#27` (idx 26), 北泽精肉店 (name lookup).

Rulings that this unit leans on (TEST-FINDINGS §6 / sheet):
* **爱心义演 rounding** (user ruling 2026-10-06): 「向上取整10」 rounds the
  *payment* up to a multiple of 10, **halved first**:
  `final = ceil((payment / 2) / 10) * 10`. An exact multiple is unchanged.
* **运动的天赋 re-roll floor** (sheet 2026-10-06 新卡组卡 F9): 「至少为10」 is
  inclusive -- a first roll of 10 is kept.
* **热气球演出 mod60** (sheet F11): a 3d20 sum of 60 wraps via `mod 60` (0),
  not tile `#60` as a raw sum. Destination is 「结果对应序号的格子」 whose
  序号 run `1..=60`.
* **Wacha Mocha** (sheet F13): 「[场]」 is the placement branch of a merged
  play; the jump is the press of the already-placed card in 主要阶段.

---

## Cards

### HHW:Happy, Lucky, Smile, Yeah！

> Happy, Lucky, Smile, Yeah！：
> 投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。

* `C-HHW-hlsy-01` -- roll & direction. Source: `cards.json#HHW:Happy, Lucky, Smile, Yeah！`.
  Quote: 「投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向…」
  **Trigger:** [手] play of this card. **Condition:** none beyond a legal play.
  **Effect:** exactly one 1d4 is rolled; face `1..=4` selects 上 / 下 / 左 / 右
  (mapping of face → direction is not printed; see OPEN-HHW-DIR). Face 0 from
  `mod 4` is undefined (OPEN-HHW-DIR).
* `C-HHW-hlsy-02` -- teleport as main move. Quote:
  「…传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。」
  **Trigger:** same play. **Effect:** the player is [传送]ped (path = endpoint
  only) to the farthest tile in that direction; the move counts as the turn's
  [主要移动]. **Frame:** play leaves the hand to discard (non-[持续]); `pos`
  and turn main-move bookkeeping may move; nothing else may.
* **Executable:** `hlsy_teleports_and_is_the_main_move` / `hlsy_frame`.
* **OPEN-HHW-DIR:** face → direction mapping, `1d4mod4` of face 0, and the
  board geometry of 上/下/左/右 / 「直线距离最远」 are not defined in the
  rulebook body. Only the teleport-as-main-move shape is specified.

### HHW:出发！后台之旅！

> 出发！后台之旅！：
> 看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆，每翻入一张获得1000资金

* `C-HHW-backstage-01` -- look / return / discard. Quote:
  「看牌堆顶2张牌，选择0~2张以任意顺序放回，剩余的翻入弃牌堆」
  **Trigger:** [手] play. **Condition:** draw pile holds at least the top 2
  (a short pile is OPEN-HHW-SHORT-PILE; the text does not say).
  **Effect:** the top 2 are examined; the player returns `k ∈ [0,2]` of them
  (any order) to the draw pile and the other `2-k` enter the discard.
* `C-HHW-backstage-02` -- payout. Quote: 「每翻入一张获得1000资金」
  **Effect:** user's money rises by exactly `1000 × (cards flipped to discard)`.
* **Executable:** `backstage_pays_1000_per_discard`.
* **OPEN-HHW-SHORT-PILE:** fewer than 2 cards in the draw pile.

### HHW:因为我一直相信着你

> 因为我一直相信着你：
> 至少有另一张手牌时可发动，消耗800资金，公开你的卡组与手牌，选择一名玩家，使其从你的手牌中选择一张放入弃牌堆，然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。

* `C-HHW-believe-01` -- activation gate. Quote:
  「至少有另一张手牌时可发动」
  **Condition:** besides this card, the user holds ≥1 other hand card.
  **Effect:** a play with only this card in hand is refused.
* `C-HHW-believe-02` -- cost. Quote: 「消耗800资金」
  **Effect:** user's money falls by exactly 800 ([消耗] = delete money).
* `C-HHW-believe-03` -- discard one hand card. Quote:
  「…选择一名玩家，使其从你的手牌中选择一张放入弃牌堆」
  **Effect:** exactly one remaining hand card (not this card, already played)
  enters the discard. The chooser is another player (OPEN-HHW-BELIEVE-CHOOSER
  only for "which" player; the count is determined).
* `C-HHW-believe-04` -- take two and reshuffle. Quote:
  「然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。」
  **Effect:** two cards from the draw pile enter the hand; the draw pile is
  reshuffled (order of the remainder is unspecified -- OPEN-HHW-RESHUFFLE).
* **Executable:** `believe_refuses_with_only_this_card`, `believe_contract` / `believe_cost`, `believe_frame`.
* **OPEN-HHW-BELIEVE-CHOOSER:** which player chooses the discarded card
  (「选择一名玩家」 -- the user picks the chooser, or any other seat?).
* **OPEN-HHW-RESHUFFLE:** reshuffle of an already-shuffled pile; only the
  multiset conservation is determined.

### HHW:微笑巡逻队

> 微笑巡逻队：
> 付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）

* `C-HHW-patrol-01` -- paid build. Quote:
  「付款并在任意自己的格子加盖一层房屋」
  **Trigger:** [手] play. **Effect:** the user pays the build cost (price
  「付款」 is the normal house cost; OPEN-HHW-PATROL-PRICE if the card means
  a different figure) and one own tile gains +1 house.
* `C-HHW-patrol-02` -- free extra house. Quote:
  「投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）」
  **Effect:** one 3d20 is rolled; the tile whose 序号 equals the sum (or the
  three faces? OPEN-HHW-PATROL-3D20) gains +1 house **for free** when it is
  可建造 (per 「[可购买格子]」 plus build caps -- the exact 「可建造」 set is
  OPEN-HHW-BUILDABLE).
* **Executable:** `patrol_paid_build` / `patrol_frame`. The free 3d20 house is **not scripted**.
* **OPEN-HHW-PATROL-PRICE:** what 「付款」 amounts to.
* **OPEN-HHW-PATROL-3D20:** 3d20 as sum vs three separate tiles.
* **OPEN-HHW-BUILDABLE:** 「可建造格子」 is not defined; the book's nearest
  defined set is 「[可购买格子]」 (rulebook-doc.md:65).

### HHW:梦幻的回礼

> 梦幻的回礼：[反击] 在场上其他玩家即将被不属于你的格子收费时打出，向场上你以外的任意一名玩家的一个格子进行一次支付。若成功进行支付，则自动使用一次你的乐队技能进行双倍支付并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶，并为将要进行支付的那名玩家减免相当于你支付金额的数额。

* `C-HHW-return-01` -- counter window. Quote:
  「[反击] 在场上其他玩家即将被不属于你的格子收费时打出」
  **Trigger:** a [反击] window when another player is about to be charged
  (收费) by a tile that is **not** the counter-user's. **Effect:** the card
  may be played in that window; 「结算优先于X」 (rulebook-doc.md:73).
* `C-HHW-return-02` -- pay a stranger's tile. Quote:
  「向场上你以外的任意一名玩家的一个格子进行一次支付。」
  **Effect:** the user [支付]s an amount to one tile owned by some other
  player (the amount / price basis is OPEN-HHW-RETURN-AMOUNT).
* `C-HHW-return-03` -- band-skill double pay. Quote:
  「若成功进行支付，则自动使用一次你的乐队技能进行双倍支付」
  **Condition:** the payment of (2) succeeded.
  **Effect:** the band skill 传播笑容 (1) is used once in its 「支付双倍」
  form (not optional here -- 「自动」).
* `C-HHW-return-04` -- two crystals. Quote:
  「并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶」
  **Effect:** the band-skill field instance gains exactly +2 奇迹水晶, one
  recording each of the two named players (the payee of (2) and the soon-to-
  pay player of the window). Crystal → player binding is not spelled out
  (OPEN-HHW-RETURN-BOOK).
* `C-HHW-return-05` -- relief. Quote:
  「并为将要进行支付的那名玩家减免相当于你支付金额的数额。」
  **Effect:** the charged player's payment falls by the amount the user paid
  in (2).
* **Executable:** `dream_return_is_offered_on_another_players_charge` (window
  shape only). (2)–(5) **not scripted** -- they need a live rent window plus
  a successful pay and the band double.
* **OPEN-HHW-RETURN-AMOUNT:** the amount of 「进行一次支付」.
* **OPEN-HHW-RETURN-BOOK:** how a crystal 「records」 a player.

### HHW:热气球演出

> 热气球演出：
> 投掷4次3d20mod60并记录其结果，选择其中之一，传送至结果对应序号的格子，视为你的主要移动

* `C-HHW-balloon-01` -- four rolls. Quote:
  「投掷4次3d20mod60并记录其结果」
  **Trigger:** [手] play. **Effect:** four independent `3d20 mod 60` results
  are recorded. A sum of 60 wraps to 0 (ruling; 序号 `1..=60`, so `0 ≡ 60`).
* `C-HHW-balloon-02` -- pick and teleport. Quote:
  「选择其中之一，传送至结果对应序号的格子，视为你的主要移动」
  **Effect:** the player picks one of the four; [传送] to that 序号 tile;
  the move is the turn's [主要移动].
* **Executable:** `balloon_teleports_to_a_chosen_result` / `balloon_frame`.

### HHW:爱心义演

> 爱心义演：
> 打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2，本回合中向其他玩家支付时你的付款减半（向上取整10）（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）

* `C-HHW-charity-01` -- pass bonus. Quote:
  「打出此卡的回合内，你若进行掷骰移动，每初次经过一个属于你的格子，使你的总移动数+2」
  **Trigger:** this turn, on a dice move. **Effect:** the first time the path
  [经过]s each tile the user owns, total move +2. 「初次」 = once per tile per
  move (not once per recompute? OPEN-HHW-CHARITY-RECOMPUTE).
* `C-HHW-charity-02` -- pay half. Quote:
  「本回合中向其他玩家支付时你的付款减半（向上取整10）」
  **Effect:** any payment the user makes **to another player** this turn is
  halved, then rounded **up** to a multiple of 10 (ruling 2026-10-06).
  Exact: `final = ceil((payment / 2) / 10) * 10`.
* `C-HHW-charity-03` -- Kokoro CiRCLE. Quote:
  「（若弦卷心已将专属卡置于CiRCLE上，则CiRCLE也算作属于弦卷心的格子）」
  **Condition:** 弦卷心's exclusive card 「前往笑容集结的地方！」 is on
  CiRCLE. **Effect:** for that 弦卷心, CiRCLE counts as 「属于」 her for (1).
* **Executable:** `charity_halves_payment` (140→70). The ceil-to-10 leg is
  pinned only as the printed formula in
  `charity_rounds_the_half_up_to_a_multiple_of_ten` -- **not** an engine
  assertion (no 1.5× rent source lives in this unit).
* **OPEN-HHW-CHARITY-RECOMPUTE:** whether a path recompute re-fires 「初次经过」.

### HHW:笑容大游行

> 笑容大游行：
> （1）[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，当次移动的移动终点视为“弦卷集团”地产商；为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则将其放入弃牌堆。
> （2）[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上并移除其上全部奇迹水晶
> （3）[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。

* `C-HHW-parade-01` -- counter on passing 弦卷集团. Quote (1) first sentence.
  **Trigger:** [反击] when the user [经过]s 弦卷集团 (`#29`).
  **Effect:** the card is placed on that tile; the current move's
  [移动终点] becomes the 弦卷集团 地产商 (the 地产商 of that colour group).
* `C-HHW-parade-02` -- three crystals and decay. Quote (1) later sentences.
  **Effect:** the card gains exactly 3 奇迹水晶; the owner removes one at
  each of their turn ends; when the count hits 0 and the card is still on
  弦卷集团 it goes to the discard.
* `C-HHW-parade-03` -- relocate after settle. Quote (2).
  **Trigger:** [持续], after [触发结算]. **Effect:** may move the card from
  弦卷集团 to the [移动终点] tile and remove all its crystals.
* `C-HHW-parade-04` -- tile swap. Quote (3) first half.
  **Trigger:** [持续] while the card sits on a tile. **Effect:** that tile is
  treated as swapped with 弦卷集团 (for effects that name either tile).
* `C-HHW-parade-05` -- graveyard after settle. Quote (3) second half.
  **Effect:** after any player [触发结算]s on the tile the card sits on, the
  card goes to the discard.
* **Executable:** **not scripted** ([触发结算] OPEN; known stack overflow).
* **OPEN-HHW-触发结算:** [触发结算] is undefined in the rulebook body (README
  OPEN-触发结算). Only the placement / crystal / discard arithmetic is
  specified.

### HHW:运动的天赋

> 运动的天赋：
> 将此卡放置于自己场上并放置3个奇迹水晶，每回合结束时失去一个，为0时置入弃牌堆。此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。每次移动掷骰时，重骰移动掷骰直至结果至少为10为止。

* `C-HHW-athletic-01` -- place with 3 crystals. Quote:
  「将此卡放置于自己场上并放置3个奇迹水晶」
  **Trigger:** [手] play. **Effect:** the card enters the user's field with
  exactly 3 crystals.
* `C-HHW-athletic-02` -- turn-end decay. Quote:
  「每回合结束时失去一个，为0时置入弃牌堆。」
  **Effect:** at the owner's each turn end, −1 crystal; at 0 the card goes
  to the discard.
* `C-HHW-athletic-03` -- per-roll payout. Quote:
  「此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。」
  **Condition:** the card is on the field. **Effect:** each dice roll grants
  money once: first 700, then 600, … floor 100. The counter is per card
  instance across the game (「每次减少100」).
* `C-HHW-athletic-04` -- re-roll floor. Quote:
  「每次移动掷骰时，重骰移动掷骰直至结果至少为10为止。」
  **Effect:** a movement roll below 10 is re-rolled until ≥ 10. 10 is
  inclusive (ruling).
* **Executable:** `athletic_places_three_crystals`,
  `athletic_decays_one_per_turn_end`,
  `athletic_re_rolls_below_ten_and_keeps_ten`,
  `athletic_pays_700_then_600_on_rolls`.

### HHW:黑衣人的补给

> 黑衣人的补给：
> [反击] 经过“CiRCLE”格子（#1）时可打出此卡，在“弦卷集团”（#29格）格子上放置一个奇迹水晶，该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。

* `C-HHW-supply-01` -- counter window. Quote first sentence.
  **Trigger:** [反击] when the user [经过]s CiRCLE (`#1`).
  **Effect:** the card may be played; a 奇迹水晶 is placed on 弦卷集团 (`#29`).
* `C-HHW-supply-02` -- CiRCLE clone. Quote:
  「该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。」
  **Condition:** 弦卷集团 carries ≥1 crystal. **Effect:** that tile gains all
  CiRCLE effects (the CiRCLE reward / settle body). 「全部效果」 is the
  tile's printed effect set -- see OPEN-HHW-CIRCLE-FX for any CiRCLE card
  riders.
* `C-HHW-supply-03` -- remove on pass. Quote:
  「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。」
  **Effect:** after the user passes 弦卷集团, that tile loses 1 crystal.
* **Executable:** `supply_counter_seeds_a_crystal_on_tsuzumi`. (02)/(03) **not scripted**.

### HHW:（kkr）前往笑容集结的地方！

> （kkr）前往笑容集结的地方！：
> 支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款

* `C-HHW-kkr-01` -- pay 10000. Quote:
  「支付10000资金（视为买地花费）」
  **Trigger:** [手] play. **Effect:** the user pays exactly 10000; the payment
  is treated as a land-purchase cost (so modifiers that name 「买地」 apply;
  the base amount is 10000).
* `C-HHW-kkr-02` -- place on CiRCLE. Quote: 「并将此卡置于CiRCLE上」
  **Effect:** the card is placed on the CiRCLE tile (board-owned placement,
  not a player field).
* `C-HHW-kkr-03` -- settle rent. Quote:
  「若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款」
  **Trigger:** another player [触发结算]s on CiRCLE. **Effect:** that player
  pays 6000 to the owner (「所有者」 -- the card's owner / the player who
  placed it; OPEN-HHW-KKR-OWNER if the board owns CiRCLE), treated as a tile
  collection.
* **Executable:** `kkr_pays_10000` (cost + CiRCLE placement). (3) **not scripted**.
* **OPEN-HHW-KKR-OWNER:** who 「所有者」 is when CiRCLE has no deed owner.
* **OPEN-HHW-触发结算:** as above.

### HHW:（美咲）

> （美咲）[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动），随后那格及相邻2格上的所有其他玩家[支付]你500资金。

* `C-HHW-misaki-01` -- counter window. Quote first clause.
  **Trigger:** [反击] after a fire-pot movement roll, before [触发结算].
* `C-HHW-misaki-02` -- consume and teleport. Quote:
  「消耗所有火罐使你传送至你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算（视为你的主要移动）」
  **Effect:** all fire pots are spent; the user [传送]s to the tile of a
  chosen player standing between 移动起点 and 预定移动终点; that tile is
  [触发结算]ed; the move is the turn's [主要移动].
* `C-HHW-misaki-03` -- neighbourhood charge. Quote:
  「随后那格及相邻2格上的所有其他玩家[支付]你500资金。」
  **Effect:** every other player on that tile or the two neighbouring tiles
  (each side? OPEN-HHW-NEIGHBOUR) pays the user 500.
* **Executable:** `misaki_card_is_a_counter_not_a_hand_play`. (2)–(3) **not scripted**.
* **OPEN-HHW-NEIGHBOUR:** 「相邻2格」 -- two tiles each side, or a total of two.
* **OPEN-HHW-触发结算:** as above.

### HHW:（育美）

> （育美）
> （1）[手] 投掷2次4d20并将一个育美标记放置到投掷结果之一的格子上，你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记；若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。
> （2）[反击] 此卡可在你回合外收到资金的回合结束时打出，当前回合内你每获得过一次资金，此卡的投掷次数+1

* `C-HHW-hagumi-01` -- two 4d20. Quote (1) first half.
  **Trigger:** [手] play. **Effect:** two independent `4d20` rolls.
* `C-HHW-hagumi-02` -- mark or the >60 branch. Quote (1).
  **Condition:** the chosen result ≤ 60 (the 「大于60」 branch is the else).
  **Effect (≤60):** one 育美标记 is placed on the tile of that 序号.
  **Effect (>60):** gain 1000 and draw 1; no mark is placed.
* `C-HHW-hagumi-03` -- pass stop. Quote (1):
  「你经过育美标记时可在那格强制停下并获得2000资金，然后移除该标记」
  **Trigger:** the user [经过]s a tile carrying their 育美标记.
  **Effect:** may [强制停下] there, gain 2000, remove the mark.
* `C-HHW-hagumi-04` -- counter outside own turn. Quote (2).
  **Trigger:** [反击] at the end of a turn in which the user received money
  outside their own turn. **Condition:** the card is in hand.
  **Effect:** the card may be played; its 「投掷次数」 for the current turn
  goes up by 1 per money gain that turn.
* **Executable:** `hagumi_two_rolls_place_a_mark` (≤60 branch). >60 / (3) / (4) **not scripted**.
* **OPEN-HHW-HAGUMI-TIME:** 「投掷次数+1」 -- extra 4d20s on a later activation
  of (1), or extra rolls of some other effect of the same card.

### HHW:（花音）Wacha Mocha 啪嗒进行曲

> （花音）Wacha Mocha 啪嗒进行曲：[场]花音每次倒走时此卡获得一个水母标记；主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。

* `C-HHW-wacha-01` -- place and count. Quote first half.
  **Trigger:** [场] play (placement). **Effect:** the card enters the user's
  field; each time 花音 walks backwards (倒走), the card gains 1 水母标记.
  「倒走」 is not defined (OPEN-HHW-BACKWARDS) -- the counter's *existence*
  is determined, its trigger shape is not.
* `C-HHW-wacha-02` -- the jump. Quote second half.
  **Trigger:** 主要阶段, card on field, ≥9 水母标记.
  **Effect:** clear all markers; [传送] to 水族馆 (`#4`) or 弦卷豪宅 (`#30`);
  the teleport is the turn's [主要移动]; the card then goes to the discard.
* **Executable:** `wacha_places_from_hand`, `wacha_jump_at_nine_markers`.
* **OPEN-HHW-BACKWARDS:** 「倒走」 (reverse movement) is not defined in the
  rulebook body (反方向移动 is an 「异常移动效果」, rulebook-doc.md:100).

### HHW:（薰）怪盗hello happy

> （薰）怪盗hello happy：
> 将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。

* `C-HHW-phantom-01` -- place with charge. Quote first clause.
  **Trigger:** [手] play. **Effect:** the card is placed on the user's field
  with 充能 3 / 衰减 1 (three charge counters, −1 per turn -- the counter
  name is not printed; OPEN-HHW-CHARGE).
* `C-HHW-phantom-02` -- plant a mark. Quote:
  「并向一名玩家场上放置一个怪盗标记」
  **Effect:** one 怪盗标记 is placed on one chosen player's field (the book
  does not say 「你以外」; the user's own field is OPEN).
* `C-HHW-phantom-03` -- forced stop this turn. Quote:
  「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。」
  **Condition:** this turn's movement phase; the path [经过]s that player.
  **Effect:** may [强制停下] there and [触发结算].
* `C-HHW-phantom-04` -- Kaoru's tile rewrite. Quote second sentence.
  **Condition:** the card is on the field; a player who can move sits on
  薰's tile. **Effect:** that player's [主要移动] becomes `1d2` (forward /
  backward) and `1d10` (distance), settled as that move.
* **Executable:** `phantom_places_on_field`. (3)/(4) **not scripted**.
* **OPEN-HHW-CHARGE:** 「充能3，衰减1」's counter name and whether it is
  奇迹水晶.
* **OPEN-HHW-触发结算:** as above.

---

## Character skills (`data/characters.json`, band = Hello, Happy World!)

### 弦卷心 -- 弦卷集团

> （1）开局时获得1000资金（2）[经过]CiRCLE时额外获得1500资金

* `C-HHW-kokoro-01` -- opening 1000. Quote (1).
  **Trigger:** game start. **Effect:** money +1000 (on top of the base 10000).
  **Executable:** `kokoro_opening_bonus`.
* `C-HHW-kokoro-02` -- CiRCLE pass. Quote (2).
  **Trigger:** the owner [经过]s CiRCLE. **Effect:** money +1500 **extra**
  (stacks with the base CiRCLE reward of 2000).
  **Executable:** `kokoro_circle_pass_extra_1500`.

### 濑田薰 -- 梦幻的王子殿下

> （1）每次[经过]或被[经过]时，若场上不存在[怪盗标记]，获得1火罐（上限7）；若场上存在[怪盗标记]，则移除场上的一个[怪盗标记]（2）主要阶段可消耗7火罐在绝对距离最近的一名玩家场上放置3个[怪盗标记]，并使你的本次移动以微笑号为起点。（3）你非传送的主要移动不会经过场上拥有[怪盗标记]的玩家所拥有的格子。

* `C-HHW-kaoru-01` -- pass pots / mark removal. Quote (1).
  **Trigger:** each [经过] or 被[经过] of the owner.
  **Condition:** no / some 怪盗标记 on the field.
  **Effect:** +1 火罐 (cap 7) when none exist; remove one mark when some do.
  **Executable:** `kaoru_pass_gives_one_pot_per_step`.
* `C-HHW-kaoru-02` -- seven-pot plant. Quote (2).
  **Trigger:** 主要阶段, active. **Condition:** 7 fire pots available.
  **Effect:** spend 7; place 3 怪盗标记 on the nearest player's field
  (absolute board distance); this move starts from 微笑号 (`#27`).
  **Executable:** `kaoru_skill_plants_three_marks_for_seven_fire` (red, Finding).
* `C-HHW-kaoru-03` -- path filter. Quote (3).
  **Condition:** main move is not [传送]; a player holding a 怪盗标记 owns
  tiles. **Effect:** the path does not [经过] those tiles.
  **Executable:** **not scripted** (whole-walk shape).

### 北泽育美 -- 全垒打！

> （1）每个回合在北泽精肉店生成一个可乐饼。你经过格子上的可乐饼时可以将其转移到自己场上（持有上限10），其他角色经过格子上可乐饼时可以将其转移到自己场上并向你支付50*X资金，X为该角色持有的可乐饼数量。你将可乐饼转移至自己场上时可用其替换掉一个其他不位于[持续]卡上的标记。角色每拥有一个可乐饼，移动时多投掷1个1d2。

* `C-HHW-hagumi-skill-01` -- spawn. Quote (1) first sentence.
  **Trigger:** the owner's turn start, once per her turn (ruling 2026-10-29:
  「每个回合」 reads as 「你每个回合开始」, the `simple` tab -- not every
  player's turn end). **Effect:** one 可乐饼 appears on 北泽精肉店.
* `C-HHW-hagumi-skill-02` -- own pass transfer. Quote (1).
  **Trigger:** the owner [经过]s a tile holding a 可乐饼.
  **Effect:** may transfer it to their own field (cap 10).
* `C-HHW-hagumi-skill-03` -- other's pass. Quote (1).
  **Trigger:** another player [经过]s a tile holding a 可乐饼.
  **Effect:** they may transfer it and pay the owner `50 * X` where `X` is
  **their** post-transfer croquette count.
* `C-HHW-hagumi-skill-04` -- replace a mark. Quote (1):
  「你将可乐饼转移至自己场上时可用其替换掉一个其他不位于[持续]卡上的标记。」
* `C-HHW-hagumi-skill-05` -- extra dice. Quote (1) last sentence.
  **Condition:** a player holds `n` 可乐饼. **Effect:** movement rolls gain
  `n` extra `1d2`.
* **Executable:** `hagumi_extra_d2_per_croquette` (the (5) extra 1d2 only).

### 松原花音 -- 真正的迷子

> (1) 游戏开始起点为#30弦卷豪宅 (2) 移动阶段进行两次移动掷骰并将它们的结果相减作为你的移动格数 (3) 如果三个回合内没有拿过Circle奖励，则下回合移动掷骰改为1d20+1d4

* `C-HHW-kanon-01` -- start tile. Quote (1).
  **Effect:** the player starts on 弦卷豪宅 (`#30`), not CiRCLE.
  **Executable:** `kanon_starts_on_mansion`.
* `C-HHW-kanon-02` -- difference move. Quote (2).
  **Effect:** the main move rolls two movement dice and moves
  `|a - b|`? or `a - b`? The text is 「将它们的结果相减」 (subtract) -- a
  negative result is OPEN-HHW-KANON-NEG. Determined: two rolls, not one 1d20.
  **Executable:** `kanon_moves_by_the_difference_of_two_rolls` (positive difference only).
* `C-HHW-kanon-03` -- no-reward clock. Quote (3).
  **Condition:** no CiRCLE reward taken within three turns.
  **Effect:** the next turn's movement roll becomes `1d20 + 1d4`.
  **Executable:** **not scripted** (three-turn clock).

### 奥泽美咲 -- 另一个我

> （1）每次经过“弦卷集团”（#29）格子后获得2火罐（初始0，上限2），若你在传送时CiRCLE位于你正向移动的移动起点和移动终点之间，则你下次经过“弦卷集团”前可将“弦卷集团”格子额外视为CiRCLE。（2）你的回合开始时，你可以消耗一个火罐，使你的移动掷骰额外获得一个1d10，且你的移动视为[传送]。（3）使用火罐进行移动掷骰后，触发结算前，你可以消耗1火罐，获得一层在你的下回合开始前移除的[除外]。

* `C-HHW-misaki-skill-01` -- pass pots. Quote (1) first half.
  **Trigger:** after [经过]ing 弦卷集团 (`#29`). **Effect:** +2 火罐
  (initial 0, cap 2).
  **Executable:** `misaki_pass_gives_two_fire`.
* `C-HHW-misaki-skill-02` -- CiRCLE clone on next pass. Quote (1) second half.
  **Condition:** a [传送] whose positive direction has CiRCLE strictly
  between start and end. **Effect:** until the next 弦卷集团 pass, that tile
  may additionally be treated as CiRCLE.
* `C-HHW-misaki-skill-03` -- turn-start dice rider. Quote (2).
  **Trigger:** the owner's turn start. **Condition:** may spend 1 fire pot.
  **Effect:** the movement roll gains one extra `1d10` and the move is
  treated as [传送].
  **Executable:** **not scripted**.
* `C-HHW-misaki-skill-04` -- pre-settle exile. Quote (3).
  **Trigger:** after a fire-pot movement roll, before [触发结算].
  **Condition:** may spend 1 fire pot. **Effect:** gain one layer of [除外]
  that is removed at the start of the owner's **next** turn.
  **Executable:** **not scripted**.

---

## Band skill (`data/bands.json`)

### Hello, Happy World! -- 传播笑容

> （1）向他人的格子付款时可选择支付双倍价格，若如此做，记录该玩家并使此卡获得一个奇迹水晶。（2）被记录的玩家的格子产生收款时，移除此卡的一个奇迹水晶，记录该次收款的价格（可累加）。（3）你进行盖房时，可从盖房花费中减去所记录的数字，然后将记录数字归零。

* `C-HHW-band-01` -- optional double. Quote (1).
  **Trigger:** paying onto another player's tile.
  **Effect:** may pay **double** the price; then record that player and the
  band-skill card gains 1 奇迹水晶.
* `C-HHW-band-02` -- record collections. Quote (2).
  **Trigger:** a recorded player's tile collects.
  **Effect:** remove 1 crystal; the collection price is recorded and
  accumulates.
* `C-HHW-band-03` -- build discount. Quote (3).
  **Trigger:** the owner builds. **Effect:** the recorded number may be
  subtracted from the build cost; the record then resets to 0.
* **Executable:** `band_offers_a_double_payment`, `band_double_pays_twice_and_gains_a_crystal`. (3) **not scripted**.
* **OPEN-HHW-BAND-RECORD:** the storage of 「记录」 (player list + running
  price). Only the crystal count and the money / house arithmetic are
  directly observable.

---

## Untested clauses (and why)

| clause | why no dedicated contract |
|---|---|
| C-HHW-hlsy-01 | face → direction map is OPEN-HHW-DIR; only the teleport shape is pinned |
| C-HHW-backstage-01 (order of return) | put-back order is player choice; count is pinned |
| C-HHW-believe-03 (which player chooses) | OPEN-HHW-BELIEVE-CHOOSER; the discard count is pinned |
| C-HHW-patrol-02 | OPEN-HHW-PATROL-3D20 / BUILDABLE; paid build is pinned |
| C-HHW-return-02..05 | need a live rent window + band double + relief; only the window offer is pinned |
| C-HHW-charity-01 | path recompute 「初次经过」 is OPEN-HHW-CHARITY-RECOMPUTE |
| C-HHW-parade-01..05 | [触发结算] is OPEN; (1) also stack-overflows the engine (cross-agent t13) |
| C-HHW-supply-02/03 | CiRCLE-clone and pass-removal need a multi-act settle script |
| C-HHW-kkr-03 | needs another player's [触发结算] on CiRCLE (cross-agent t15 shape) |
| C-HHW-misaki-02/03 | fire-pot roll + neighbour charge; window is multi-act |
| C-HHW-hagumi-03/04 | force-stop window and turn-end [反击] |
| C-HHW-wacha-01 (倒走) | OPEN-HHW-BACKWARDS; place + jump are pinned |
| C-HHW-phantom-02..04 | mark plant is arranged via the card's own prompt; rewrite of the main move is a multi-act path |
| C-HHW-kaoru-03 | path filter is a whole-walk shape |
| C-HHW-hagumi-skill-01..04 | croquette spawn / transfer / pay need a multi-turn multi-actor script |
| C-HHW-kanon-03 | three-turn CiRCLE-reward clock |
| C-HHW-misaki-skill-02/03/04 | teleport-clone, turn-start dice rider, pre-settle [除外] |
| C-HHW-band-03 | build discount needs a legal `build` window after the record |

## Open questions

| id | text | notes |
|---|---|---|
| OPEN-HHW-DIR | 1d4mod4 → 上/下/左/右; face 0; 「直线距离最远」 | board geometry not printed |
| OPEN-HHW-SHORT-PILE | draw pile < 2 on 出发！后台之旅！ | |
| OPEN-HHW-BELIEVE-CHOOSER | who chooses the discarded card | |
| OPEN-HHW-RESHUFFLE | reshuffle of an empty-remainder pile | |
| OPEN-HHW-PATROL-PRICE | 「付款」 amount on 微笑巡逻队 | |
| OPEN-HHW-PATROL-3D20 | 3d20 as sum vs three tiles | |
| OPEN-HHW-BUILDABLE | 「可建造格子」 | nearest defined set is [可购买格子] |
| OPEN-HHW-RETURN-AMOUNT | amount of 「进行一次支付」 | |
| OPEN-HHW-RETURN-BOOK | crystal 「records」 a player | |
| OPEN-HHW-CHARITY-RECOMPUTE | 「初次经过」 after path recompute | |
| OPEN-HHW-触发结算 | [触发结算] | README OPEN-触发结算 |
| OPEN-HHW-KKR-OWNER | 「所有者」 of the CiRCLE-placed card | |
| OPEN-HHW-NEIGHBOUR | 「相邻2格」 | |
| OPEN-HHW-HAGUMI-TIME | 「投掷次数+1」 meaning | |
| OPEN-HHW-BACKWARDS | 「倒走」 | |
| OPEN-HHW-CHARGE | 「充能3，衰减1」 counter name | |
| OPEN-HHW-KANON-NEG | negative difference on 花音 (2) | `simple` says signed (first − second; negative walks backward); `text` does not |
| OPEN-HHW-BAND-RECORD | band skill 「记录」 storage | |
| OPEN-HHW-TEXT-VS-SIMPLE | `characters.json` `text` vs `simple` | 北泽育美 `simple` also has a gift-on-pass, a 4-croquette trade, and an enemy-at-4 punish; 松原花音 `simple` decides signed subtraction (「用第一次减第二次：正数往前走，负数往后走」); 濑田薰 `text` says 「主要阶段」 while `simple` says 「运营阶段」; 北泽育美 spawn is 「每个回合」 (`text`) vs 「你每个回合开始」 (`simple`); ruling 2026-10-29 resolves it to the owner's own turn start (C-HHW-hagumi-skill-01). |
| OPEN-HHW-RETURN-RELIEF | relief = pre-double or post-double (2) amount | (3) forces 「支付双倍」; (5) says 「相当于你支付金额」 |

## Findings

Genuine engine gaps (plain failing tests; no `#[ignore]`):

* **C-HHW-kaoru-01** -- book: 「若场上不存在[怪盗标记]，获得1火罐…；若场上存在
  [怪盗标记]，则移除场上的一个[怪盗标记]」. With a live [怪盗标记] (the counter
  `HHW:（薰）怪盗hello happy` itself plants, stable key
  `cards:card-hhw.kaoru_thief_tok`) on another player, a 3-step own walk
  granted +3 火罐 and left the mark untouched. The skill's existence check
  does not see the counter the printed card plants. Test:
  `kaoru_removes_a_phantom_mark_instead_of_a_pot`.
* **C-HHW-kaoru-02** -- book: 「消耗7火罐…放置3个[怪盗标记]」. The press spends
  the 7 火罐 (`kaoru_spend` logged) and answers the nearest-player
  pick, but places **0** [怪盗标记] (no `cards:card-hhw.kaoru_thief_tok` on any
  seat, no tile mark). Test: `kaoru_skill_plants_three_marks_for_seven_fire`.

Interpretations the contracts pin (not gaps):

* **C-HHW-band-01/02** -- a double-pay onto a recorded player's tile is itself
  that tile collecting, so (1) '+1 奇迹水晶' and (2) '−1 奇迹水晶' can cancel
  in one settle. The observable at rest is the running 「记录」 (`hhw_saved`),
  not a net crystal gain. Tests: `band_offers_a_double_payment`,
  `band_double_pays_twice_and_gains_a_crystal`.
* **C-HHW-believe-03/04** -- hand multiset is start 3, −1 played, −1 discarded,
  +2 drawn = 3 at rest. The chooser pick is a `choice` prompt of card options
  answered by option index.
