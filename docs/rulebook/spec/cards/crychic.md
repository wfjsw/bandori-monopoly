# CRYCHIC -- cards, character skills, band skill

Unit `c-crychic`. Owns `rules/tests/tests/spec_card_crychic.rs` and this file.

Sources, in precedence order (see `docs/rulebook/spec/README.md`):

1. Live sheet: `docs/rulebook/cards.json` (15 `CRYCHIC:` entries),
   `data/characters.json` (5 characters with `band == "CRYCHIC"`),
   `data/bands.json` (`name == "CRYCHIC"`).
2. `docs/rulebook/rulebook-doc.md` for global vocabulary ([手] / [反击] /
   [持续] / [特] / [停留] / [晕眩] / [传送] / [触发结算] / [奇迹水晶]).
3. TEST-FINDINGS §6 rulings unless newer sheet text supersedes them.

`rulebook.txt` is **not** authoritative. A discrepancy is a plain failing
test. Clauses that cannot be tested yet get a doc entry saying why.

Clause IDs: `C-CRYCHIC-<slug>-<n>`.

---

## CRYCHIC:一起演奏音乐的命运共同体 (`destiny`)

**Source:** `cards.json#CRYCHIC:一起演奏音乐的命运共同体`

**Quote:**

> 一起演奏音乐的命运共同体
> ：
> （1）[手] 直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。
> （2）若此卡打出后（1）效果未产生作用，将此卡返回手牌（不触发乐队技能的（3）效果）。

### C-CRYCHIC-destiny-01

* **Quote:** 「（1）[手] 直到你的下回合开始，每当场上任意格子发生一次收款时，你移动到你前方的下一个属于行动序列后一名玩家的格子（不触发结算）。」
* **Trigger:** a 「收款」 (money collection on any tile) during the window that
  opens when this [手] is played and closes at the user's next turn start.
* **Condition:** the card's (1) is live; the collection is a 「收款」.
* **Effect:** the user moves (no settlement) to the next tile ahead that
  belongs to the player after them in the 行动序列.
* **Frame:** only the user's `pos` moves; no settle-side money / house / draw.
* **Rulings & OPEN:**
  * **OPEN-收款** -- 「收款」 is not defined in the rulebook body. The text
    determines only that *some* collection on *any* tile arms the move; which
    ledger legs count (rent? gain? 分摊?) is undecided. This clause specifies
    only the move itself once a collection is taken as given.
  * **OPEN-行动序列后一名** -- the next player in act order is the seat after
    the user in 回合顺序; the text does not say what happens when that player
    owns no tile ahead (loop? skip? stop?). Undecided.
* **Executable:** not yet written. The window is cross-turn and 「收款」 is
  OPEN; a contract would have to invent the trigger set. Documented instead.
* **Testable parts observed elsewhere:** the play itself is accepted
  (`rb_crychic::destiny_together_plays`).

### C-CRYCHIC-destiny-02

* **Quote:** 「（2）若此卡打出后（1）效果未产生作用，将此卡返回手牌（不触发乐队技能的（3）效果）。」
* **Trigger:** the window of (1) closes (user's next turn start) with (1)
  having produced no move.
* **Condition:** (1) never moved the user.
* **Effect:** the card returns to the user's hand. The return does **not** fire
  band skill (3) (the 4→5 crystal).
* **Frame:** only the card's zone (field/hand) moves; band crystals unchanged.
* **Rulings & OPEN:** band (3) is `C-CRYCHIC-band-03`; 「不触发…（3）效果」 is
  the exemption.
* **Executable:** not yet written (depends on the same OPEN window).

---

## CRYCHIC:主唱太拼命了 (`vocalist`)

**Source:** `cards.json#CRYCHIC:主唱太拼命了`
**Quote:** 「主唱太拼命了：[反击] 一次性向其他玩家支付5000以上资金时，免除此次支付。」

### C-CRYCHIC-vocalist-01

* **ID:** `C-CRYCHIC-vocalist-01`
* **Quote:** 「一次性向其他玩家支付5000以上资金时，免除此次支付。」
* **Trigger:** a [反击] window on a one-shot payment of >= 5000 from the user
  to another player.
* **Condition:** the payment amount is >= 5000 and is one transfer (not a
  split of a larger total).
* **Effect:** the payment is waived: neither side's money moves for this leg.
* **Frame:** `money:*` unchanged for this leg; no other path moves.
* **Rulings & OPEN:** 「一次性」 rules out a 分摊 leg; 分摊 itself is
  OPEN-分摊.
* **Executable:** `crychic_vocalist_waives_big_payment` (asserts the payer's
  money is unchanged after the counter).

---

## CRYCHIC:优雅的呐喊 (`elegant`)

**Source:** `cards.json#CRYCHIC:优雅的呐喊`
**Quote:** 「优雅的呐喊：[反击] 当你在回合外受到抽卡效果时打出，你的下回合结束时抽一张卡。」

### C-CRYCHIC-elegant-01

* **ID:** `C-CRYCHIC-elegant-01`
* **Quote:** 「[反击] 当你在回合外受到抽卡效果时打出，你的下回合结束时抽一张卡。」
* **Trigger:** a [反击] window the user answers while they are **not** the
  turn player and a draw effect is targeting them.
* **Condition:** the user is out of turn; the effect is a draw.
* **Effect:** the card is discarded as a [反击]; at the **end of the user's
  next turn** they draw exactly 1 card.
* **Frame:** the counter step: only the counter card's zone; the later draw
  step: only the user's hand/draw/discard.
* **Rulings & OPEN:** 「回合外」 is the turn player being someone else.
* **Executable:** `crychic_elegant_is_a_counter_in_hand` pins the card as a
  held [反击]. The full window + deferred draw is **not contracted**: the
  live sheet has no 「让一名其他玩家抽卡」 card, and the only redirect seam
  (二叶筑紫 (1)) does not open its prompt on the draw of
  通用:10次招募（1回限定） in this harness. See § Findings for the setup gap.

---

## CRYCHIC:初演大成功 (`debut`)

**Source:** `cards.json#CRYCHIC:初演大成功`
**Quote:** 「初演大成功：打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外），回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。」

### C-CRYCHIC-debut-01

* **ID:** `C-CRYCHIC-debut-01`
* **Quote:** 「打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）」
* **Trigger:** any money-losing step of the user for the rest of the turn the
  card was played.
* **Condition:** the card was played this turn; the loss is not an auction and
  not a payment the book marks as immune to money-change effects.
* **Effect:** the user's money does not fall.
* **Frame:** `money:P_user` never decreases; other players' money may move
  (they may still receive).
* **Rulings & OPEN:** the auction exemption and 「写明不受资金变动效果影响」
  (mortgage / redeem / force-buy, per 基础[结算]) are out of scope.
* **Executable:** `c_crychic_debut_no_drop` (armed on the play step) and the
  driving assertion in `crychic_debut_blocks_rent_decrease` (rent the same
  turn).

### C-CRYCHIC-debut-02

* **ID:** `C-CRYCHIC-debut-02`
* **Quote:** 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。」
* **Trigger:** the end of the turn the card was played.
* **Condition:** the card was played this turn.
* **Effect:** the user gains 1 layer of [晕眩] (`stun` +1) and the band skill
  card gains 1 [奇迹水晶].
* **Frame:** only `state:P_user:stun` and `crystals:P_user:<band skill>`.
* **Executable:** `c_crychic_debut_turn_end` and
  `crychic_debut_turn_end_stun_and_crystal`.

---

## CRYCHIC:去唱卡拉ok吧 (`karaoke`)

**Source:** `cards.json#CRYCHIC:去唱卡拉ok吧`
**Quote:** 「去唱卡拉ok吧：进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。」

### C-CRYCHIC-karaoke-01

* **ID:** `C-CRYCHIC-karaoke-01`
* **Quote:** 「进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。」
* **Trigger:** this card is played from hand.
* **Condition:** the user is in a state where a main move is legal.
* **Effect:** up to 5 dice are rolled; one chosen face becomes this turn's
  movement roll; the walk is a normal dice move (path, settle, CiRCLE).
* **Frame:** the chosen face's walk only; the discarded faces cause no move.
* **Rulings & OPEN:** 「至多5次」 lets the user stop early; the choice is an
  engine prompt (option list). Which faces are kept when the user re-rolls is
  the card's own dice loop, not the global re-roll.
* **Executable:** `crychic_karaoke_picks_a_roll` (asserts the landing is one
  of the rolled faces).

---

## CRYCHIC:如果能一直持续下去... (`ifonly`)

**Source:** `cards.json#CRYCHIC:如果能一直持续下去...`
**Quote:**

> 如果能一直持续下去...：
> （1）[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。
> （2）[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。

### C-CRYCHIC-ifonly-01

* **ID:** `C-CRYCHIC-ifonly-01`
* **Quote:** 「[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。」
* **Trigger:** the [手] body, played from hand.
* **Condition:** the user has a band skill card with X crystals (X may be 0).
* **Effect:** all X crystals are removed from the band skill card; the user
  gains `2000 + 500*X` money.
* **Frame:** only `money:P_user`, the played card's zone, and
  `crystals:P_user:<band skill>`.
* **Executable:** `c_crychic_ifonly_cashout` +
  `crychic_ifonly_cashs_out_band_crystals` (exact `2000+500*X`, crystals 0).

### C-CRYCHIC-ifonly-02

* **ID:** `C-CRYCHIC-ifonly-02`
* **Quote:** 「[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。」
* **Trigger:** a hand-size check while the [持续] is live and hand >= 7.
* **Condition:** the card is the live [持续]; hand >= 7.
* **Effect:** the card goes to the discard immediately (leaves the field).
* **Frame:** only the card's zone.
* **Rulings & OPEN:** the check fires on any hand-size change; the text does
  not name a narrower window. Base hand limit is 5 (rulebook-doc.md:126) so
  >= 7 requires a raised limit (the CRYCHIC band skill removes the limit).
* **Executable:** `crychic_ifonly_discards_at_hand_seven`.

---

## CRYCHIC:想要成为人类 (`human`)

**Source:** `cards.json#CRYCHIC:想要成为人类`
**Quote:**

> 想要成为人类
> ：（1）将此卡置于场上并从1-20间选择并声明X，将其写下。每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶。
> （2）[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。

### C-CRYCHIC-human-01

* **ID:** `C-CRYCHIC-human-01`
* **Quote:** 「（1）将此卡置于场上并从1-20间选择并声明X，将其写下。每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶。」
* **Trigger:** the play of this card (placement + declaration), then every
  move roll of the user whose face is <= X.
* **Condition:** X is declared in 1..=20 and recorded.
* **Effect:** the card sits on the field with X written down; each move roll
  <= X adds 1 [奇迹水晶] to the card.
* **Frame:** placement: the card's zone + `fprop` (the X write-down). Each
  crystal: only `crystals:P_user:human`.
* **Rulings & OPEN:** sheet 2026-10-06 新卡组卡 M2 reads 「小于等于X」
  (equal counts; supersedes an older 「小于X」).
* **Executable:** `crychic_human_crystal_on_low_roll` (declares X, rolls
  <= X, asserts +1 crystal).

### C-CRYCHIC-human-02

* **ID:** `C-CRYCHIC-human-02`
* **Quote:** 「（2）[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。」
* **Trigger:** the user's turn start, when the card holds >= 2 crystals.
* **Condition:** crystals >= 2 at turn start.
* **Effect:** all crystals are removed; the next move roll gains `+(20-X)`;
  if that move is hit by an [异常移动效果], the card gains 2 crystals.
* **Frame:** turn start: only the card's crystals + a move-modifier latch;
  the roll step: `roll` / `pos`; the abnormal-move branch: only the card's
  crystals.
* **Rulings & OPEN:** [异常移动效果] is the G-STATUS set (rulebook-doc);
  which effects are in it is that unit's clause. The `+ (20-X)` applies to
  the **roll result**, not to the walked distance.
* **Executable:** `crychic_human_turn_start_consumes_crystals` (asserts the
  two crystals are consumed at turn start). The `+(20-X)` and the abnormal-
  move branch are not yet contracted; they need a move-modifier observation
  the frame currently hides as bookkeeping (`roll:P*` is filtered).

---

## CRYCHIC:想要抓住... (`grab`)

**Source:** `cards.json#CRYCHIC:想要抓住...`
**Quote:**

> 想要抓住...：
> （1）获得一层[停留]。
> （2）你的下回合开始前，当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]。
> （3）你的下回合开始前，若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。

### C-CRYCHIC-grab-01

* **ID:** `C-CRYCHIC-grab-01`
* **Quote:** 「（1）获得一层[停留]。」
* **Trigger:** this card is played.
* **Condition:** none beyond the play.
* **Effect:** the user gains 1 layer of [停留] (`stay` +1).
* **Frame:** only `state:P_user:stay`.
* **Executable:** `c_crychic_grab_stay` + `crychic_grab_grants_stay`.

### C-CRYCHIC-grab-02

* **Quote:** 「（2）你的下回合开始前，当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]。」
* **Trigger:** the first other player who [经过] the user before the user's
  next turn start.
* **Condition:** the passer is not the user; the window is still open.
* **Effect:** before that player's [触发结算], the user moves 1 tile forward
  and [触发结算] themselves.
* **Frame:** only the user's `pos` + the two settle side-effects.
* **Rulings & OPEN:** **OPEN-触发结算** -- [触发结算] is undefined
  (`rulebook-doc.md` / README open index). The clause specifies only the
  move and that a settlement follows.
* **Executable:** not yet written ([触发结算] OPEN; a pass-sequence setup is
  legal but the settle contents are undecided).

### C-CRYCHIC-grab-03

* **ID:** `C-CRYCHIC-grab-03`
* **Quote:** 「（3）你的下回合开始前，若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。」
* **Trigger:** the user's turn start, with no player having passed them since
  the play.
* **Condition:** the (2) branch never fired.
* **Effect:** the user chooses and [传送]s to the nearest tile in one of the
  four cardinal directions (may wrap the map; may not be the current tile),
  then [触发结算]; this counts as the main move.
* **Frame:** only `pos` + the settle side-effects; no dice.
* **Rulings & OPEN:** OPEN-触发结算 again; 「直线距离最近」 with four
  candidates and ties is the test seam (pick any nearest).
* **Executable:** not yet written as a contract (the teleport-as-main-move
  settle shape is TEST-FINDINGS open item 1). The play's (1) is contracted.

---

## CRYCHIC:春日影 (`haruhikage`)

**Source:** `cards.json#CRYCHIC:春日影`
**Quote:**

> 春日影
> ：（1）[特] 若抽到此卡时你的总资产大于等于20000，可选择使其直接从抽牌堆打出，依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。
> （2）（此卡可作为[反击]打出）使用一次Crychic角色的技能

### C-CRYCHIC-haruhikage-01

* **Quote:** 「（1）[特] 若抽到此卡时你的总资产大于等于20000，可选择使其直接从抽牌堆打出，依次抽牌直至你的手牌数为6，若受到弃牌效果则中断此效果。」
* **Trigger:** this card is drawn, with the drawer's 总资产 >= 20000.
* **Condition:** assets >= 20000 at the draw.
* **Effect:** optional: the card plays directly from the draw pile, then the
  user draws until hand = 6; a discard effect interrupts the draw-to-6 loop.
* **Frame:** the play + the draws; the interrupt stops further draws.
* **Rulings & OPEN:**
  * 总资产 is the `assets` mirror (money + deed value), not just money.
  * **OPEN-春日影抽6** (TEST-FINDINGS G29): is each iteration of the
    draw-to-6 a replaceable draw (subject to other draw-replacing effects)?
    Undecided.
* **Executable:** not yet contracted as a full loop. A draw-triggered play is
  arranged in the harness via `set_draw` + a draw effect
  (`rb_ag::sky_fires_on_draw` idiom); the draw-to-6 loop needs the G29 ruling.

### C-CRYCHIC-haruhikage-02

* **ID:** `C-CRYCHIC-haruhikage-02`
* **Quote:** 「（此卡可作为[反击]打出）使用一次Crychic角色的技能」
* **Trigger:** a [反击] window in which one of the CRYCHIC character skills
  applies (the card is offered as a counter).
* **Condition:** the user holds this card; a Crychic character skill's
  counter-time is live.
* **Effect:** the card is discarded as the counter; the borrowed character
  skill fires once (e.g. 长崎素世（CRYCHIC）'s cancel-rent, 椎名立希（CRYCHIC）'s
  re-roll, 丰川祥子（CRYCHIC）'s step-toward).
* **Frame:** the counter card's zone + the borrowed skill's own effect.
* **Executable:** `crychic_haruhikage_counter_uses_a_crychic_skill` (the
  rent-cancel branch).

---

## CRYCHIC:是我自己的问题 (`myown`)

**Source:** `cards.json#CRYCHIC:是我自己的问题`
**Quote:**

> 是我自己的问题
> ：[反击]
> （1）主要移动结束时，[触发结算]前打出此卡，使自己额外远离绝对距离最近的玩家一格（若距离最近的玩家在身前则向后移动，若与其他玩家重合则可选择任意方向）。
> （2）若受到[异常移动效果]影响，此卡不生效

### C-CRYCHIC-myown-01

* **Quote:** 「（1）主要移动结束时，[触发结算]前打出此卡，使自己额外远离绝对距离最近的玩家一格（若距离最近的玩家在身前则向后移动，若与其他玩家重合则可选择任意方向）。」
* **Trigger:** a [反击] window after the main move ends and before
  [触发结算].
* **Condition:** the main move just landed; the user is not being hit by an
  [异常移动效果] (see (2)).
* **Effect:** the user moves 1 tile **away** from the absolutely nearest
  player (backwards if the nearest is ahead; free direction if stacked).
* **Frame:** only `pos` of the user.
* **Rulings & OPEN:** OPEN-触发结算 for the window edge; 「绝对距离」 is the
  short way around the ring.
* **Executable:** not yet written (the settle-before window is real -- see
  `rb_crychic::haruhikage_counteracts_settle_before` -- but the nearest-player
  distance rule needs a dedicated arrange).

### C-CRYCHIC-myown-02

* **Quote:** 「（2）若受到[异常移动效果]影响，此卡不生效」
* **Trigger:** the same window, when the move was already altered by an
  [异常移动效果].
* **Condition:** an abnormal move effect applied to this move.
* **Effect:** the card does not take effect (no extra step).
* **Frame:** nothing moves.
* **Executable:** not yet written (needs the G-STATUS [异常移动效果] set).

---

## CRYCHIC:（soyo）回到曾经 (`soyoreturn`)

**Source:** `cards.json#CRYCHIC:（soyo）回到曾经`
**Quote:**

> （soyo）回到曾经：
> （1）[特] 抽到此卡时立刻从抽牌堆打出并执行以下操作之一 ：
> 1. 将你的所有手牌放入弃牌堆，然后获得弃牌数*500的资金，抽1张卡，为你的一个格子付费加盖一间房屋，然后[移除]此卡；
> 2. 获得1000资金并将此卡加入手牌
> （2）[手] 获得500资金

### C-CRYCHIC-soyoreturn-01

* **Quote:** 「（1）[特] 抽到此卡时立刻从抽牌堆打出并执行以下操作之一」
* **Trigger:** this card is drawn (from the draw pile).
* **Condition:** none beyond the draw.
* **Effect:** the card plays directly from the draw pile; the user picks
  branch 1 or 2.
* **Frame:** depends on the branch.
* **Branch 1:** hand -> discard; gain `discarded * 500`; draw 1; pay to build
  1 house on one owned tile; the card is [移除] (removed from the game).
* **Branch 2:** gain 1000; the card joins the hand.
* **Rulings & OPEN:** 「付费加盖」 is the normal build price (G-SETTLE).
  Branch 1's house is mandatory (「为你的一个格子付费加盖一间房屋」).
* **Executable:** not yet contracted as a draw-triggered [特]; the [手] body
  is contracted (below). The draw idiom is `set_draw` + a draw effect.

### C-CRYCHIC-soyoreturn-02

* **ID:** `C-CRYCHIC-soyoreturn-02`
* **Quote:** 「（2）[手] 获得500资金」
* **Trigger:** the [手] body, played from hand.
* **Condition:** none.
* **Effect:** the user gains exactly 500 money.
* **Frame:** only `money:P_user` + the played card's zone.
* **Executable:** `c_crychic_soyoreturn_hand` +
  `crychic_soyoreturn_hand_gains_500`.

---

## CRYCHIC:（灯）内心的呐喊 (`tomoricry`)

**Source:** `cards.json#CRYCHIC:（灯）内心的呐喊`
**Quote:**

> （灯）内心的呐喊：
> （1）打出此卡时，你可重置一次"想要成为人类"所声明的X（不影响水晶）并获得两个X差值*120的资金。
> （2）此卡不会被你乐队技能的（2）效果移除，乐队技能的（2）效果发动时展示此卡并将其洗入抽牌堆。
> （3）若你的乐队技能为"MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上，你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能。

### C-CRYCHIC-tomoricry-01

* **Quote:** 「（1）打出此卡时，你可重置一次"想要成为人类"所声明的X（不影响水晶）并获得两个X差值*120的资金。」
* **Trigger:** this card is played (optional).
* **Condition:** a live 「想要成为人类」 has a written X.
* **Effect:** the X is re-declared (crystals stay); the user gains
  `(|old_X - new_X| * 2) * 120` -- 「两个X差值*120」 is read as
  `2 * |ΔX| * 120`.
* **Frame:** the human card's X write-down + `money:P_user`.
* **Rulings & OPEN:** 「两个X差值」 is ambiguous (2 * ΔX * 120, or
  (two X-values' difference) * 120). This clause records the formula as
  **OPEN-两个X差值**; the executable below asserts only the chosen reading and
  must be flipped if the book meant the other.
* **Executable:** not yet contracted (depends on OPEN-两个X差值).

### C-CRYCHIC-tomoricry-02

* **Quote:** 「（2）此卡不会被你乐队技能的（2）效果移除，乐队技能的（2）效果发动时展示此卡并将其洗入抽牌堆。」
* **Trigger:** band skill (2) fires (the CRYCHIC transform).
* **Condition:** this card is in one of the user's zones.
* **Effect:** the card is **not** removed with the other CRYCHIC cards; it is
  revealed and shuffled into the draw pile.
* **Frame:** the card's zone moves to the draw pile; it is not in `event_removed`.
* **Executable:** not yet contracted (needs the full band-(2) transform).

### C-CRYCHIC-tomoricry-03

* **Quote:** 「（3）若你的乐队技能为"MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上，你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能。」
* **Trigger:** this card is played while the user's band skill is MyGO!!!!!;
  later, the user is within 5 tiles of the placed card.
* **Condition:** band skill is MyGO!!!!!; the user owns at least one tile.
* **Effect:** the card is placed on the user's earliest-owned tile; within
  5 front/back the user may spend 1 [火罐] to use 高松灯（MyGO!!!!!）'s (2)
  skill once.
* **Frame:** placement: the card's tile; the skill use: that skill's own
  effect + 1 fire.
* **Executable:** not yet contracted (cross-band; needs a MyGO band skill
  swap).

---

## CRYCHIC:（睦）从没有觉得... (`mutsumifeel`)

**Source:** `cards.json#CRYCHIC:（睦）从没有觉得...`
**Quote:**

> （睦）从没有觉得... ：
> （1）打出此卡时，使用者可以选择（2）或（3）效果之一发动。
> （2）消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的（2）效果，然后弃一张卡。
> （3）此卡[移除]并向抽牌堆中加入一张"表演的本能"，将手牌与弃牌堆全部放入抽牌堆并洗切，然后抽2张卡。你本回合的移动以"CiRCLE"为起点（不触发起点地块效果）

### C-CRYCHIC-mutsumifeel-01

* **Quote:** 「（1）打出此卡时，使用者可以选择（2）或（3）效果之一发动。」
* **Trigger:** this card is played.
* **Condition:** none.
* **Effect:** a choice prompt of (2) vs (3); exactly one branch runs.
* **Frame:** the chosen branch's own effects.
* **Executable:** the choice is observed in
  `rb_crychic::mutsumi_crychic_card_choice`; not contracted here (the branch
  bodies are the real clauses).

### C-CRYCHIC-mutsumifeel-02

* **Quote:** 「（2）消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的（2）效果，然后弃一张卡。」
* **Trigger:** branch (2) of the play.
* **Condition:** the band skill card has >= 0 crystals.
* **Effect:** min(3, crystals) crystals are removed; band skill (2) runs
  immediately; the user discards 1 card.
* **Frame:** the band card's crystals + band-(2)'s own effects + one discard.
* **Executable:** not yet contracted (needs band (2) to be runnable in the
  middle of a card play).

### C-CRYCHIC-mutsumifeel-03

* **ID:** (doc only; executable pending)
* **Quote:** 「（3）此卡[移除]并向抽牌堆中加入一张"表演的本能"，将手牌与弃牌堆全部放入抽牌堆并洗切，然后抽2张卡。你本回合的移动以"CiRCLE"为起点（不触发起点地块效果）」
* **Trigger:** branch (3) of the play.
* **Condition:** none.
* **Effect:** the card is [移除] (removed from the game); 「表演的本能」
  (`Mujica:（睦/mortis）表演的本能`) is added to the draw pile; hand and
  discard are shuffled into the draw; the user draws 2; this turn's move
  starts from CiRCLE (no start-tile effect).
* **Frame:** card zones + the 2 draws; `pos` starts the walk at CiRCLE.
* **Rulings & OPEN:** the exact card id of 「表演的本能」 is the live-sheet
  `Mujica:（睦/mortis）表演的本能` (the text names it without a band prefix).
  「不触发起点地块效果」 suppresses the CiRCLE start hook.
* **Executable:** not yet contracted (the shuffle + move-from-CiRCLE pair is
  two transitions).

---

## CRYCHIC:（祥子）带领着大家 (`sakilead`)

**Source:** `cards.json#CRYCHIC:（祥子）带领着大家`
**Quote:** 「（祥子）带领着大家：回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家，使你的下次主要移动结果对那些玩家一起执行，你先触发结算，此后其他玩家按行动顺序依次触发结算；触发结算时进行的支付价格减半。」

### C-CRYCHIC-sakilead-01

* **Quote:** 「回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家」
* **Trigger:** the user's turn start, sharing a tile with >= 1 other player.
* **Condition:** overlap at turn start.
* **Effect:** the card may be played; the overlapping players are recorded.
* **Frame:** the card's zone + a record of seats.
* **Executable:** not yet contracted (the record is an internal latch).

### C-CRYCHIC-sakilead-02

* **Quote:** 「使你的下次主要移动结果对那些玩家一起执行，你先触发结算，此后其他玩家按行动顺序依次触发结算」
* **Trigger:** the next main move after the play.
* **Condition:** players were recorded.
* **Effect:** the same move result is applied to each recorded player; the
  user settles first, then the others in 行动顺序.
* **Frame:** each participant's `pos` + their own settle side-effects.
* **Rulings & OPEN:** OPEN-触发结算; 行动顺序 is the turn order.
* **Executable:** not yet contracted (multi-player walk).

### C-CRYCHIC-sakilead-03

* **Quote:** 「触发结算时进行的支付价格减半。」
* **Trigger:** each settle of the shared move.
* **Condition:** the settle involves a 支付.
* **Effect:** the payment price is halved.
* **Frame:** the payment leg's money.
* **Rulings & OPEN:** TEST-FINDINGS open item 4 (two halvings stack to ¼)
  applies if another half-rate effect is live; 「支付价格」 covers shaped
  payments (user ruling 2026-10-06 on 祥，移动).
* **Executable:** not yet contracted.

---

## CRYCHIC:（立希）即便比不上... (`rikireroll`)

**Source:** `cards.json#CRYCHIC:（立希）即便比不上...`
**Quote:** 「（立希）即便比不上...：[反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。」

### C-CRYCHIC-rikireroll-01

* **ID:** `C-CRYCHIC-rikireroll-01`
* **Quote:** 「[反击] 进入移动阶段后，触发结算前可打出，进行一次重骰，你可重骰至掷骰结果与本回合内你骰出过的所有骰点都不同为止。」
* **Trigger:** a [反击] window after the move roll and before [触发结算].
* **Condition:** the user has rolled at least one face this turn.
* **Effect:** the user may re-roll, repeating until the new face differs from
  every face already rolled this turn; that face becomes the move.
* **Frame:** `roll` / `pos` only (the discarded faces cause no walk).
* **Rulings & OPEN:** OPEN-触发结算 for the window edge; 「本回合内你骰出过的
  所有骰点」 includes faces from cards like 去唱卡拉ok吧.
* **Executable:** `crychic_rikireroll_counter_rerolls` (asserts the counter
  is offered after the move roll and resolves a re-roll).

---

## Character skill: 高松灯（CRYCHIC）跌跌撞撞... (`tomoriskill`)

**Source:** `data/characters.json#高松灯（CRYCHIC）`
**Quote:**

> （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
> （2）你回合内的任意时刻，可消耗1火罐清除自身一层[停留]
> （3）当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算。

### C-CRYCHIC-tomoriskill-01

* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
* **Trigger:** [经过] CiRCLE (tile #1 / index 0).
* **Condition:** fire count < cap.
* **Effect:** +1 [火罐]; the cap is 1 (initial 1).
* **Frame:** `state:P_user:fire` only.
* **Executable:** not yet contracted as a walk-past (the walk is a multi-
  event step). The cap is pinned by `rb_crychic::tomori_skill_fire_on_circle`
  (`fire_max == 1`).

### C-CRYCHIC-tomoriskill-02

* **Quote:** 「（2）你回合内的任意时刻，可消耗1火罐清除自身一层[停留]」
* **Trigger:** a skill press during the user's turn.
* **Condition:** fire >= 1 and stay >= 1.
* **Effect:** fire -1, stay -1.
* **Frame:** `state:P_user:fire` and `state:P_user:stay` only.
* **Executable:** not yet contracted (the skill press is `Table::skill`).

### C-CRYCHIC-tomoriskill-03

* **ID:** `C-CRYCHIC-tomoriskill-03`
* **Quote:** 「（3）当你的移动掷骰小于等于6时，立刻获得一层[停留]并在回合结束时触发结算。」
* **Trigger:** a move roll with face <= 6.
* **Condition:** none.
* **Effect:** +1 [停留] immediately; a settle happens at end of turn (the
  walk is cancelled by the stay).
* **Frame:** `state:P_user:stay` + the deferred settle.
* **Executable:** `crychic_tomoriskill_low_roll_grants_stay`.

---

## Character skill: 长崎素世（CRYCHIC）雨中祈晴 (`soyoskill`)

**Source:** `data/characters.json#长崎素世（CRYCHIC）`
**Quote:** 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付，使你下回合的主要移动变为传送至触发此技能的那格。」

### C-CRYCHIC-soyoskill-01

* **ID:** `C-CRYCHIC-soyoskill-01`
* **Quote:** 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付，使你下回合的主要移动变为传送至触发此技能的那格。」
* **Trigger:** another player settles (and would pay) on a tile the user owns.
* **Condition:** the tile is owned by the user; the other player is not the
  user.
* **Effect:** optional: the payment is cancelled; the user's next main move
  becomes a [传送] to that tile (one-shot).
* **Frame:** the cancel: the payment leg's money; the teleport: only the
  user's `pos` + that tile's settle.
* **Executable:** `crychic_soyoskill_cancel_arms_teleport` (cancel + the
  one-shot teleport on the next main move).

---

## Character skill: 椎名立希（CRYCHIC）克服劣等感 (`rikiskill`)

**Source:** `data/characters.json#椎名立希（CRYCHIC）`
**Quote:** 「每当你的移动掷骰结果严格小于你以外玩家中上一次进行的移动掷骰结果且那次移动掷骰结果不大于20，若你本回合未使用技能，你下回合可在移动前重骰一次并选择第二次的结果使用。」

### C-CRYCHIC-rikiskill-01

* **Quote:** the quote above.
* **Trigger:** the user's move roll, when it is strictly less than the last
  other player's move roll (that roll <= 20) and the user has not used a
  skill this turn.
* **Condition:** a prior other-player move roll exists and is <= 20.
* **Effect:** next turn, before the move, the user may re-roll once and pick
  the second result.
* **Frame:** the next turn's roll / pos.
* **Rulings & OPEN:** 「不大于20」 gates the *other* roll, not the user's.
* **Executable:** not yet contracted (cross-turn re-roll latch).

---

## Character skill: 丰川祥子（CRYCHIC）你愿意和我组建乐队吗？ (`sakiskill`)

**Source:** `data/characters.json#丰川祥子（CRYCHIC）`
**Quote:**

> （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）
> （2）其他人的主要移动结束时，[触发结算]前若在你的前后5格内，可选择向你支付300资金并使自己的移动终点沿最短路径向你靠近1格。
> （3）你的主要移动结束时，[触发结算]前若前后五格内有别的玩家，你可消耗1火罐，选择其中一名玩家并使自己的移动终点沿最短路径向其靠近1格。
> （4）若你的乐队技能为“Ave Mujica”，你距离其他玩家5格以内时处于状态2，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。

### C-CRYCHIC-sakiskill-01

* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）」
* **Trigger:** [经过] CiRCLE.
* **Condition:** fire < 2.
* **Effect:** +1 [火罐]; initial 1, cap 2.
* **Frame:** `state:P_user:fire` only.
* **Executable:** cap pinned by `rb_crychic::saki_crychic_skill_fire_cap`
  (`fire_max == 2`). Walk-past not contracted.

### C-CRYCHIC-sakiskill-02

* **Quote:** 「（2）其他人的主要移动结束时，[触发结算]前若在你的前后5格内，可选择向你支付300资金并使自己的移动终点沿最短路径向你靠近1格。」
* **Trigger:** another player's main move ends within 5 tiles of the user
  (front or back), before [触发结算].
* **Condition:** the distance along the ring is <= 5 either way.
* **Effect:** optional: the mover pays the user 300 and the endpoint steps
  1 tile toward the user (shortest path).
* **Frame:** the 300 transfer + the mover's `pos`.
* **Executable:** not yet contracted (the settle-before window is real; see
  `rb_crychic::haruhikage_counteracts_settle_before` for the window shape).

### C-CRYCHIC-sakiskill-03

* **Quote:** 「（3）你的主要移动结束时，[触发结算]前若前后五格内有别的玩家，你可消耗1火罐，选择其中一名玩家并使自己的移动终点沿最短路径向其靠近1格。」
* **Trigger:** the user's main move ends with another player within 5.
* **Condition:** fire >= 1.
* **Effect:** fire -1; the endpoint steps 1 tile toward the chosen player.
* **Frame:** `state:P_user:fire` + `pos`.
* **Executable:** not yet contracted.

### C-CRYCHIC-sakiskill-04

* **Quote:** 「（4）若你的乐队技能为“Ave Mujica”，你距离其他玩家5格以内时处于状态2，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。」
* **Trigger:** distance to any other player <= 5 while the band skill is Ave
  Mujica.
* **Condition:** band skill is Ave Mujica.
* **Effect:** the user is in 状态2 (`skillState`); crystals on the band card
  may only be added on a reshuffle.
* **Frame:** `state:P_user:skillState`; crystal writes are gated.
* **Executable:** not yet contracted (needs a band-skill swap to Mujica).

---

## Character skill: 若叶睦（CRYCHIC）精致的人偶 (`mutsumiskill`)

**Source:** `data/characters.json#若叶睦（CRYCHIC）`
**Quote:**

> （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
> （2）开局时指定一名其他玩家，你的移动阶段开始时，可消耗1火罐，使本次的移动掷骰结果为那名玩家上一次的移动掷骰结果；那名玩家破产时，可重新指定一名玩家。
> （3）乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格小于3时进入状态1，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。

### C-CRYCHIC-mutsumiskill-01

* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
* **Trigger:** [经过] CiRCLE.
* **Effect:** +1 [火罐]; initial 1, cap 1.
* **Frame:** `state:P_user:fire` only.
* **Executable:** cap pinned by `rb_crychic::mutsumi_crychic_skill_fire_cap`.

### C-CRYCHIC-mutsumiskill-02

* **Quote:** 「（2）开局时指定一名其他玩家，你的移动阶段开始时，可消耗1火罐，使本次的移动掷骰结果为那名玩家上一次的移动掷骰结果；那名玩家破产时，可重新指定一名玩家。」
* **Trigger:** game start (designation) and each move-phase start (the spend).
* **Condition:** fire >= 1; the designated player has a prior move roll.
* **Effect:** fire -1; this move's roll equals the designated player's last
  move roll. On that player's bankruptcy the designation may be replaced.
* **Frame:** `state:P_user:fire` + `roll`.
* **Executable:** not yet contracted.

### C-CRYCHIC-mutsumiskill-03

* **Quote:** 「（3）乐队技能为Ave Mujica时，任意时刻手牌大于等于3时进入状态2，严格小于3时进入状态1，但只有洗牌时可以为你的乐队技能卡上添加奇迹水晶。」
* **Trigger:** hand-size changes while the band skill is Ave Mujica.
* **Condition:** band skill is Ave Mujica.
* **Effect:** hand >= 3 -> 状态2; hand < 3 -> 状态1; band-card crystals only
  on reshuffle.
* **Frame:** `state:P_user:skillState`.
* **Executable:** not yet contracted (needs a band-skill swap).

---

## Band skill: CRYCHIC 美好的往日幻影 (`band`)

**Source:** `data/bands.json#CRYCHIC`
**Quote:**

> CRYCHIC团技能：（1）你的手牌数没有上限，任何时刻拥有手牌数大于等于6时，你无法获得或失去资金，无法从手中打出任何牌且领取CiRCLE奖励时必须选择抽一张卡。
> （2）你的回合结束时，若你的抽牌堆与弃牌堆中都没有卡，移除此卡与你所有区域的所有“CRYCHIC”卡，将你剩余的所有手牌放入抽牌堆，并向抽牌堆中加入角色对应的自选“MyGO”或“Ave Mujica”卡至抽牌堆中总共有10张卡并洗切；获得角色对应的“MyGO”或“Ave Mujica”乐队技能卡，然后抽2张卡。
> （3）每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶 （上限10）；当此卡移除时，你获得X*500资金，X为此卡上的奇迹水晶数。

### C-CRYCHIC-band-01

* **ID:** `C-CRYCHIC-band-01` / `C-CRYCHIC-band-01b`
* **Quote:** 「（1）你的手牌数没有上限，任何时刻拥有手牌数大于等于6时，你无法获得或失去资金，无法从手中打出任何牌且领取CiRCLE奖励时必须选择抽一张卡。」
* **Trigger:** hand size is observed; hand >= 6 is a standing gate.
* **Condition:** hand count.
* **Effect:**
  * no hand limit (`hand_limit == None`);
  * hand >= 6: no money gain or loss, no plays from hand, CiRCLE reward must
    be the draw (not the 2000).
* **Frame:** the gate: money moves and play events are suppressed.
* **Executable:** `crychic_band_no_hand_limit` (`C-CRYCHIC-band-01`) and
  `crychic_band_blocks_play_at_six` (`C-CRYCHIC-band-01b`).

### C-CRYCHIC-band-02

* **Quote:** 「（2）你的回合结束时，若你的抽牌堆与弃牌堆中都没有卡，移除此卡与你所有区域的所有“CRYCHIC”卡，将你剩余的所有手牌放入抽牌堆，并向抽牌堆中加入角色对应的自选“MyGO”或“Ave Mujica”卡至抽牌堆中总共有10张卡并洗切；获得角色对应的“MyGO”或“Ave Mujica”乐队技能卡，然后抽2张卡。」
* **Trigger:** the user's turn end, with draw pile and discard both empty.
* **Condition:** `draw == 0 && discard == 0`.
* **Effect:** the band card and every CRYCHIC card in every zone is removed;
  the remaining hand goes to the draw pile; character-corresponding MyGO or
  Ave Mujica cards are chosen and added until the draw pile holds 10, then
  shuffled; the matching band skill card is gained; the user draws 2.
* **Frame:** nearly every zone of the user + the band-skill field card.
* **Rulings & OPEN:**
  * 「所有区域的所有CRYCHIC卡」 includes the hand, discard, field and draw.
  * **C-CRYCHIC-tomoricry-02** exempts （灯）内心的呐喊 (revealed and shuffled
    in instead of removed).
  * 「角色对应的自选」 is an open choice: the MyGO / Ave Mujica counterpart of
    the user's CRYCHIC character (e.g. 高松灯（CRYCHIC） -> 高松灯's MyGO
    exclusive). The text says 「自选」 so the user picks which counterpart.
  * The removal cash-out is band (3)'s last sentence.
* **Executable:** not yet contracted (a full band transform across 10 cards
  and a band-skill swap).

### C-CRYCHIC-band-03

* **ID:** `C-CRYCHIC-band-03`
* **Quote:** 「（3）每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶 （上限10）；当此卡移除时，你获得X*500资金，X为此卡上的奇迹水晶数。」
* **Trigger:** a hand-size change that crosses 4 -> 5; and the band card's
  removal.
* **Condition:** the crossing is exactly 4 -> 5 (not 3 -> 5 in one step? the
  text says 「从4增加至5」, so the crossing edge).
* **Effect:** +1 crystal (cap 10); on removal, the user gains `X*500`.
* **Frame:** `crystals:P_user:<band>`; on removal also `money:P_user`.
* **Rulings & OPEN:** whether a jump 3 -> 5 in one draw counts is
  **OPEN-四到五** (the text names the 4 -> 5 edge).
* **Executable:** `crychic_band_crystal_on_four_to_five` (observes a 4 -> 5
  climb via a draw effect).

---

## Open questions (this unit)

| OPEN | text | clauses |
|---|---|---|
| OPEN-收款 | 「收款」 undefined | destiny-01 |
| OPEN-行动序列后一名 | next-player tile search undefined | destiny-01 |
| OPEN-触发结算 | [触发结算] undefined | grab-02/03, myown-01, sakilead-02, rikireroll-01 |
| OPEN-春日影抽6 | is each draw-to-6 iteration replaceable? (TEST-FINDINGS G29) | haruhikage-01 |
| OPEN-两个X差值 | 「两个X差值*120」 formula | tomoricry-01 |
| OPEN-四到五 | does a 3 -> 5 jump count as 「从4增加至5」? | band-03 |
| OPEN-分摊 | 分摊 undefined | vocalist-01 (「一次性」 excludes it) |

## Findings

* **C-CRYCHIC-band-01b** (genuine, stays failing): 「任何时刻拥有手牌数大于等于6时
  …无法从手中打出任何牌」 -- the engine **accepts** the play of 通用:GREAT at
  hand 6 and runs its body (money +2000, card removed). Both gates of band (1)
  are printed (no play, no money move); the play gate is not enforced. The
  driving contract `crychic_band_blocks_play_at_six` names the clause and the
  quote and stays red.
* **C-CRYCHIC-band-03** (genuine, stays failing): 「每当你的手牌数从4增加至5，
  为此卡放置一个奇迹水晶」 -- a clean 4 -> 5 draw (landing on CiRCLE, 基础[结算]
  1) leaves the band skill card's crystals at 0. Contract
  `crychic_band_crystal_on_four_to_five` stays red.
* **C-CRYCHIC-elegant-01** (setup gap, not an engine finding): the printed
  window is 「回合外受到抽卡效果」. No live-sheet card simply makes another
  player draw; 二叶筑紫 (1) is the only redirect, and its hook does not open
  on 通用:10次招募（1回限定）'s draw in this harness. The deferred draw is
  therefore not contracted. Not an engine discrepancy -- the arrange is
  missing.

## Coverage note

Every numbered clause of every CRYCHIC card, character skill and the band
skill has a section above. Clauses marked **Executable: not yet written**
are the ones whose trigger or effect rests on an OPEN question, or which
need a multi-transition arrange the current harness cannot observe in one
`Step`. They are specified only to the extent the text determines.