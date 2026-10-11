# C-MOR -- Morfonica (Mor)

Unit 14 owns this file and `rules/tests/tests/spec_card_mor.rs`. Scope: the 15
`Mor:` cards in `docs/rulebook/cards.json`, the skills of every Morfonica
character in `data/characters.json` (`band == "Morfonica"`), and the band
skill in `data/bands.json`.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`
`text`, `data/bands.json` `text`) > `docs/rulebook/rulebook-doc.md`.
`rulebook.txt` is not authoritative. TEST-FINDINGS §6 rulings unless newer
sheet text supersedes them.

Clause IDs are `C-MOR-<slug>-<n>`. Card keys keep the `Mor:` prefix; the
slug is the short handle used in both the doc and the code.

---

## Card contracts

### C-MOR-light -- Mor:你的光芒将照亮前路

Card text (cards.json#Mor:你的光芒将照亮前路):

> 当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡，此次移动以"月之森女子学院"为起点（不触发起点地块效果）。

#### C-MOR-light-01

* **ID:** `C-MOR-light-01`
* **Source:** `cards.json#Mor:你的光芒将照亮前路`
* **Quote:** 「当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡」
* **Trigger:** a `play` of this card from hand.
* **Condition:** the user's tile is within 20 steps of 月之森女子学院 (either
  direction, wrapping; distance 20 is in range).
* **Effect:** the play is accepted. Outside the range the play must be
  refused (the card stays in hand).
* **Frame:** the play gate is a precondition; no world change on refusal.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_light_gate`].

#### C-MOR-light-02

* **ID:** `C-MOR-light-02`
* **Source:** `cards.json#Mor:你的光芒将照亮前路`
* **Quote:** 「此次移动以"月之森女子学院"为起点（不触发起点地块效果）」
* **Trigger:** the play that passes the gate above.
* **Condition:** the card's [手] body runs.
* **Effect:** the move that follows starts at 月之森女子学院, not at the
  user's current tile; the start-tile effect of 月之森 does not fire.
* **Frame:** see [`c_mor_light_frame`].
* **Rulings & OPEN:** which move this is (「此次移动」 -- the turn's main
  move, or a free move) is not spelled out; the engine treats it as the
  main move. OPEN-移动起点: 「此次移动」 as a stand-in for the main move.
* **Executable:** [`c_mor_light_start`].

---

### C-MOR-again -- Mor:再次牵起手来

Card text (cards.json#Mor:再次牵起手来):

> [手]：
> [反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金。
> [持续]：
> [消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。

#### C-MOR-again-01

* **ID:** `C-MOR-again-01`
* **Source:** `cards.json#Mor:再次牵起手来`
* **Quote:** 「[反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金。」
* **Trigger:** a [反击] window opened after the previous player (in action
  order) consumed or paid more than 0 money.
* **Condition:** the counteruser holds the card in hand.
* **Effect:** the card is placed on the counteruser's field and the
  counteruser consumes (loses) the same amount the previous player paid.
* **Frame:** the counteruser's money and field, plus the card leaving the
  hand / entering the field.
* **Rulings & OPEN:** 「行动序列前一名」 is the player immediately before
  the user in action order (wrapping).
* **Executable:** [`c_mor_again_counter`].

#### C-MOR-again-02

* **ID:** `C-MOR-again-02`
* **Source:** `cards.json#Mor:再次牵起手来`
* **Quote:** 「[持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。」
* **Trigger:** a [消耗] or [支付] that would move the field-card owner's
  money.
* **Condition:** the card sits on the owner's field ([持续]).
* **Effect:** that money change is cancelled (the owner neither loses nor
  pays) and the card is placed into the owner's discard.
* **Frame:** the cancelled leg moves nobody's money; the card leaves the
  field and enters the discard.
* **Rulings & OPEN:** OPEN-取消范围 -- whether the counterpart's half of a
  transfer is also cancelled (「取消此次资金变动」 is one-sided on the
  owner) is left to the engine; the clause pins the owner's side.
* **Executable:** [`c_mor_again_persist`].

---

### C-MOR-courage -- Mor:勇气展翅高飞之时

Card text (cards.json#Mor:勇气展翅高飞之时):

> 掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半，若为地产商地块，获得1000资金

#### C-MOR-courage-01

* **ID:** `C-MOR-courage-01`
* **Source:** `cards.json#Mor:勇气展翅高飞之时`
* **Quote:** 「掷骰3d20，结果对应序号格子的所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半」
* **Trigger:** the [手] body of this card.
* **Condition:** the 3d20 sum (tile number N, index N-1) names a tile
  whose owner is another player.
* **Effect:** that owner pays the user
  `tile.price + (tile.house × houses_on_tile) / 2`.
* **Frame:** only the two players' money moves.
* **Rulings & OPEN:** ruling 2026-10-06 (TEST-FINDINGS §6, Decided): the
  land price counts in full and only the house cost is halved. Integer
  division of the house total (even halves of odd house costs) is not
  specified; the clause uses the engine's integer half.
* **Executable:** [`c_mor_courage_pay`].

#### C-MOR-courage-02

* **ID:** `C-MOR-courage-02`
* **Source:** `cards.json#Mor:勇气展翅高飞之时`
* **Quote:** 「若为地产商地块，获得1000资金」
* **Trigger:** the same [手] body, when the rolled tile is a 地产商 tile.
* **Condition:** the tile's kind is agent (地产商).
* **Effect:** the user gains 1000. (The 「所有者向你支付」 branch does not
  run -- an agent tile has no owner payment.)
* **Frame:** only the user's money moves.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_courage_agent`].

#### C-MOR-courage-03

* **ID:** `C-MOR-courage-03`
* **Source:** `cards.json#Mor:勇气展翅高飞之时`
* **Quote:** 「结果对应序号格子的所有者向你支付…」
* **Trigger:** the same [手] body, when the rolled tile is unowned.
* **Condition:** the tile has no owner (and is not an agent -- that is
  courage-02).
* **Effect:** nobody pays; the user's money is unchanged.
* **Frame:** no money moves.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_courage_unowned`].

---

### C-MOR-summer -- Mor:夏日合宿

Card text (cards.json#Mor:夏日合宿):

> 打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌

#### C-MOR-summer-01

* **ID:** `C-MOR-summer-01`
* **Source:** `cards.json#Mor:夏日合宿`
* **Quote:** 「打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。」
* **Trigger:** the play of this card.
* **Condition:** the play is not negated.
* **Effect:** until the user's next turn start, foreign effects cannot
  designate the user. The card sits on the field for that window.
* **Frame:** the card leaves the hand and enters the field.
* **Rulings & OPEN:** OPEN-指定 -- 「指定」 (designation) is the engine's
  targeting notion; the clause pins "foreign effects do not affect the
  user", which is what the quote determines.
* **Executable:** [`c_mor_summer_shield`].

#### C-MOR-summer-02

* **ID:** `C-MOR-summer-02`
* **Source:** `cards.json#Mor:夏日合宿`
* **Quote:** 「当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」
* **Trigger:** the end of the shield window (the user's next turn start).
* **Condition:** no effect was negated by the shield during the window.
* **Effect:** the user draws one card; the card leaves the field.
* **Frame:** the user's hand / draw / discard and the field.
* **Rulings & OPEN:** the negation counter is engine-internal; the clause
  observes "the window ended and the card left the field with a draw".
* **Executable:** [`c_mor_summer_draw`].

---

### C-MOR-centrifuge -- Mor:离心力，不为所动

Card text (cards.json#Mor:离心力，不为所动):

> [反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果

#### C-MOR-centrifuge-01

* **ID:** `C-MOR-centrifuge-01`
* **Source:** `cards.json#Mor:离心力，不为所动`
* **Quote:** 「[反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡」
* **Trigger:** a [反击] window on a targeting of the user.
* **Condition:** this is the second (or later) targeting of the user by
  another player's skill or card since the end of the user's previous
  turn and before the start of their next.
* **Effect:** the counter may be played (it is optional, 「可以」).
* **Frame:** a decline leaves the world unchanged.
* **Rulings & OPEN:** OPEN-目标计数 -- what counts as 「成为…目标」 is the
  engine's designation notion.
* **Executable:** [`c_mor_centrifuge_gate`].

#### C-MOR-centrifuge-02

* **ID:** `C-MOR-centrifuge-02`
* **Source:** `cards.json#Mor:离心力，不为所动`
* **Quote:** 「直到下个你的回合开始时，无效化你受到的所有效果」
* **Trigger:** the counter play that passed the gate.
* **Condition:** --.
* **Effect:** until the user's next turn start, effects that would hit the
  user are negated; the card sits on the field for that window.
* **Frame:** the card leaves the hand and enters the field.
* **Rulings & OPEN:** OPEN-无效化 -- 「无效化你受到的所有效果」 scope
  (does it cancel already-resolved legs?) is not spelled out.
* **Executable:** [`c_mor_centrifuge_shield`].

---

### C-MOR-rainbow -- Mor:秘密与青春的虹彩

Card text (cards.json#Mor:秘密与青春的虹彩):

> [反击]
> （1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。
> （2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。

#### C-MOR-rainbow-01

* **ID:** `C-MOR-rainbow-01`
* **Source:** `cards.json#Mor:秘密与青春的虹彩`
* **Quote:** 「（1）当你向学妹或同级生支付时，打出此卡，此次支付金额减半。」
* **Trigger:** a [反击] window on a payment the user is about to make.
* **Condition:** the payee is the user's junior (学妹) or same grade
  (同级生) -- see the grade table in TEST-FINDINGS §6 / the card's own
  note. Grade relation is a static character fact.
* **Effect:** the payment amount is halved.
* **Frame:** only the two players' money moves, at the halved amount.
* **Rulings & OPEN:** ruling 2026-10-06: grades come from a static
  normalised table (Morfonica hi-2 = 11, PPP hi-3 = 12, MyGO hi-1 = 10,
  univ = 13). Halving is the payment amount, rounded as the engine does
  (integer half).
* **Executable:** [`c_mor_rainbow_half`].

#### C-MOR-rainbow-02

* **ID:** `C-MOR-rainbow-02`
* **Source:** `cards.json#Mor:秘密与青春的虹彩`
* **Quote:** 「（2）当学姐或同级生向你支付的时候，打出此卡，使此次支付资金变成1.5倍。」
* **Trigger:** a [反击] window on a payment the user is about to receive.
* **Condition:** the payer is the user's senior (学姐) or same grade.
* **Effect:** the payment amount becomes 1.5×.
* **Frame:** only the two players' money moves, at the 1.5× amount.
* **Rulings & OPEN:** the grade table above. 1.5× rounding is not spelled
  out; the clause uses the engine's integer 1.5× (140 → 210).
* **Executable:** [`c_mor_rainbow_times`].

---

### C-MOR-wing -- Mor:纯真振翅

Card text (cards.json#Mor:纯真振翅):

> 传送到移动方向20格后（不触发结算），立刻进行移动掷骰

#### C-MOR-wing-01

* **ID:** `C-MOR-wing-01`
* **Source:** `cards.json#Mor:纯真振翅`
* **Quote:** 「传送到移动方向20格后（不触发结算）」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the user is [传送]'d 20 tiles forward in the move direction;
  the destination tile does not settle (no rent / no tile effect).
* **Frame:** the user's position; no money / ownership moves from the
  destination.
* **Rulings & OPEN:** OPEN-移动方向 -- 「移动方向」 is the user's current
  walk direction (forward for a normal player). Reverse movers (仓田真白)
  go the other way; that interaction is not pinned here.
* **Executable:** [`c_mor_wing_teleport`].

#### C-MOR-wing-02

* **ID:** `C-MOR-wing-02`
* **Source:** `cards.json#Mor:纯真振翅`
* **Quote:** 「立刻进行移动掷骰」
* **Trigger:** right after the teleport above.
* **Condition:** --.
* **Effect:** a movement dice roll is made and the user walks that many
  tiles from the teleport destination, settling the landing.
* **Frame:** the user's position, and whatever the landing settles.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_wing_roll`].

---

### C-MOR-starry -- Mor:蝴蝶飞舞的星月夜

Card text (cards.json#Mor:蝴蝶飞舞的星月夜):

> （1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]，此卡没有[奇迹水晶]时加入弃牌堆; 此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次
> （2）当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]，此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]。
> （3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。

#### C-MOR-starry-01

* **ID:** `C-MOR-starry-01`
* **Source:** `cards.json#Mor:蝴蝶飞舞的星月夜`
* **Quote:** 「（1）支付X次1000的的资金，将此卡放置在场地中央并在此卡上放置X个[奇迹水晶]」
* **Trigger:** the play of this card; the player chooses X.
* **Condition:** the user can pay X × 1000.
* **Effect:** the user pays X × 1000, the card is placed (field) with X
  [奇迹水晶].
* **Frame:** the user's money and the field / crystal path.
* **Rulings & OPEN:** OPEN-X -- the book does not give X a range; the
  engine offers 1..=5. 「场地中央」 is not defined as a board slot; the
  card is a field card of the user.
* **Executable:** [`c_mor_starry_place`].

#### C-MOR-starry-02

* **ID:** `C-MOR-starry-02`
* **Source:** `cards.json#Mor:蝴蝶飞舞的星月夜`
* **Quote:** 「此卡没有[奇迹水晶]时加入弃牌堆」
* **Trigger:** any drop of the crystal count to 0.
* **Condition:** the card is on the field and holds 0 crystals.
* **Effect:** the card leaves the field and enters the discard.
* **Frame:** the field / discard.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_starry_empty`].

#### C-MOR-starry-03

* **ID:** `C-MOR-starry-03`
* **Source:** `cards.json#Mor:蝴蝶飞舞的星月夜`
* **Quote:** 「此卡在场时，你每次移动掷骰时可以放弃第一次的结果重骰一次」
* **Trigger:** the owner's movement dice roll while the card is on the
  field.
* **Condition:** --.
* **Effect:** the owner may abandon the first result and re-roll once.
* **Frame:** the roll / position of the owner.
* **Rulings & OPEN:** OPEN-重骰次数 -- 「重骰一次」 (one re-roll) is clear;
  whether the second result is final is implied.
* **Executable:** [`c_mor_starry_reroll`].

#### C-MOR-starry-04

* **ID:** `C-MOR-starry-04`
* **Source:** `cards.json#Mor:蝴蝶飞舞的星月夜`
* **Quote:** 「（2）当其他角色的移动掷骰结果是偶数时可消耗1000资金并移除此卡的一个[奇迹水晶]」
* **Trigger:** another player's movement dice result is even.
* **Condition:** the card is on the field with ≥1 crystal; the owner
  chooses to spend 1000.
* **Effect:** the owner pays 1000 and one crystal is removed.
* **Frame:** the owner's money and the crystal count.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_starry_even`].

#### C-MOR-starry-05

* **ID:** `C-MOR-starry-05`
* **Source:** `cards.json#Mor:蝴蝶飞舞的星月夜`
* **Quote:** 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」
* **Trigger:** the owner [经过]s CiRCLE.
* **Condition:** the card is on the field with ≥1 crystal.
* **Effect:** one crystal is removed.
* **Frame:** the crystal count.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_starry_circle`].

#### C-MOR-starry-06

* **ID:** `C-MOR-starry-06`
* **Source:** `cards.json#Mor:蝴蝶飞舞的星月夜`
* **Quote:** 「（3）此卡上的每个[奇迹水晶]移除时此卡拥有者获得1000资金。」
* **Trigger:** every crystal removal from the card (any of 04 / 05 / 02).
* **Condition:** --.
* **Effect:** the owner gains 1000 per crystal removed.
* **Frame:** the owner's money and the crystal count.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_starry_payout`].

---

### C-MOR-tritone -- Mor:迷茫之蝶们的三全音

Card text (cards.json#Mor:迷茫之蝶们的三全音):

> [反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金

#### C-MOR-tritone-01

* **ID:** `C-MOR-tritone-01`
* **Source:** `cards.json#Mor:迷茫之蝶们的三全音`
* **Quote:** 「[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上」
* **Trigger:** a [反击] window opened because the user is about to lose
  or pay money.
* **Condition:** the user holds the card.
* **Effect:** the user gains the amount they just lost (net of the loss),
  and the card is placed on the field with 3 [奇迹水晶].
* **Frame:** the user's money, the field, the crystal count.
* **Rulings & OPEN:** 「任意时刻」 makes this a whenever-window; the
  amount is 「此次失去的资金金额」 (the loss that just resolved).
* **Executable:** [`c_mor_tritone_gain`].

#### C-MOR-tritone-02

* **ID:** `C-MOR-tritone-02`
* **Source:** `cards.json#Mor:迷茫之蝶们的三全音`
* **Quote:** 「三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金」
* **Trigger:** the third owner turn-end after placement (the crystal
  count hits 0).
* **Condition:** the card is on the field.
* **Effect:** one crystal is removed at each owner turn end; when they run
  out the card is discarded and the user pays back the money it gained.
* **Frame:** the crystal count, the field / discard, and the payback leg.
* **Rulings & OPEN:** OPEN-三全音收款人 -- 「支付由此卡获得的资金」
  names no payee (bank vs the original payee). TEST-FINDINGS §6 records
  the gap. The clause pins the amount and the discard, not the payee.
* **Executable:** [`c_mor_tritone_payback`].

---

### C-MOR-noble -- Mor:高贵的微蓝

Card text (cards.json#Mor:高贵的微蓝):

> 当你位于一块地契价值大于等于2200的地块时，可以打出此卡，如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。

#### C-MOR-noble-01

* **ID:** `C-MOR-noble-01`
* **Source:** `cards.json#Mor:高贵的微蓝`
* **Quote:** 「当你位于一块地契价值大于等于2200的地块时，可以打出此卡」
* **Trigger:** a play of this card.
* **Condition:** the user's tile has deed price ≥ 2200.
* **Effect:** the play is accepted (「可以」 -- optional).
* **Frame:** a refusal leaves the world unchanged.
* **Rulings & OPEN:** 「地契价值」 is the tile's purchase price.
* **Executable:** [`c_mor_noble_gate`].

#### C-MOR-noble-02

* **ID:** `C-MOR-noble-02`
* **Source:** `cards.json#Mor:高贵的微蓝`
* **Quote:** 「如果此地块属于你：1.立刻进行一次触发结算。2.在该地块上放置一个标记，有标记时此地块不能被指定。」
* **Trigger:** the play, when the tile belongs to the user.
* **Condition:** the tile is the user's.
* **Effect:** (1) a 触发结算 of the tile runs immediately; (2) a mark is
  placed on the tile and, while the mark is there, the tile cannot be
  designated.
* **Frame:** the tile's marks, and whatever the forced settle moves.
* **Rulings & OPEN:** OPEN-触发结算 -- 「触发结算」 as a forced re-settle
  of the tile is not defined in the body of the rulebook (the OPEN index
  names [触发结算]). Only the mark placement is pinned as an exact
  post-state.
* **Executable:** [`c_mor_noble_mark`].

---

### C-MOR-nnm -- Mor:（NNM）稍微努力了一下

Card text (cards.json#Mor:（NNM）稍微努力了一下):

> 弃置手中x枚角色标记，发动以下效果中的一个：
> （1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张
> （2）获得x次经过CiRCLE时的资金奖励
> （3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的（2）技能，此次技能不受数量或轮数限制
> （4）当奇迹水晶耗尽时，将此卡置入弃牌堆

#### C-MOR-nnm-01

* **ID:** `C-MOR-nnm-01`
* **Source:** `cards.json#Mor:（NNM）稍微努力了一下`
* **Quote:** 「弃置手中x枚角色标记，发动以下效果中的一个」
* **Trigger:** the play of this card.
* **Condition:** the user holds x ≥ 1 character marks (角色标记) in the
  counter stock.
* **Effect:** x marks are discarded and exactly one of （1）（2）（3） runs.
* **Frame:** the chosen branch's paths only.
* **Rulings & OPEN:** OPEN-角色标记 -- the mark namespace is engine-side
  (`角色标记:<skill id>`); the book does not define the token shape.
  OPEN-多分支选择 -- TEST-FINDINGS §6 item 8: the engine's multi-activation
  dispatch runs the first `On::Play` entry only, so a hand play can be
  refused. Genuine discrepancy.
* **Executable:** [`c_mor_nnm_choice`].

#### C-MOR-nnm-02

* **ID:** `C-MOR-nnm-02`
* **Source:** `cards.json#Mor:（NNM）稍微努力了一下`
* **Quote:** 「（1）抽x张卡（可超过上限），回合结束后将手牌弃置到五张」
* **Trigger:** branch （1） of the play.
* **Condition:** x marks were discarded.
* **Effect:** the user draws x cards (hand may exceed the limit); at the
  end of the user's turn the hand is discarded down to 5.
* **Frame:** the user's hand / draw / discard.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_nnm_draw`].

#### C-MOR-nnm-03

* **ID:** `C-MOR-nnm-03`
* **Source:** `cards.json#Mor:（NNM）稍微努力了一下`
* **Quote:** 「（2）获得x次经过CiRCLE时的资金奖励」
* **Trigger:** branch （2）.
* **Condition:** x marks were discarded.
* **Effect:** the user gains x times the CiRCLE pass money reward (the
  band-modified figure for a Morfonica owner: the 1000/1500/2000 cycle).
* **Frame:** the user's money.
* **Rulings & OPEN:** sheet 2026-10-06 新卡组卡 G7 rewords this from
  「x*2000资金」; the clause pins x times the *current* money reward.
* **Executable:** [`c_mor_nnm_circle_gain`].

#### C-MOR-nnm-04

* **ID:** `C-MOR-nnm-04`
* **Source:** `cards.json#Mor:（NNM）稍微努力了一下`
* **Quote:** 「（3）将此卡放置在场上并放置x个奇迹水晶，你可以移除一个奇迹水晶视为发动你的（2）技能，此次技能不受数量或轮数限制」
* **Trigger:** branch （3）; later, a skill press while the card is placed.
* **Condition:** the card is on the field with ≥1 crystal.
* **Effect:** the card is placed with x crystals; a press removes one
  crystal and uses the owner's (2) skill (广町七深's mark-claim), ignoring
  the skill's usual once-per-turn / count limits.
* **Frame:** the field / crystal count and the borrowed skill's paths.
* **Rulings & OPEN:** OPEN-借用技能 -- 「你的（2）技能」 is the owner's
  character skill clause (2); the book does not say which owner if the
  card is not owned by 广町七深.
* **Executable:** [`c_mor_nnm_place`].

#### C-MOR-nnm-05

* **ID:** `C-MOR-nnm-05`
* **Source:** `cards.json#Mor:（NNM）稍微努力了一下`
* **Quote:** 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」
* **Trigger:** the crystal count on the placed card reaches 0.
* **Condition:** --.
* **Effect:** the card is put into the discard.
* **Frame:** the field / discard.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_nnm_empty`].

---

### C-MOR-rui-demon -- Mor:（Rui）正论恶魔

Card text (cards.json#Mor:（Rui）正论恶魔):

> 向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，每当你的技能被触发后，立即将x设置为5

#### C-MOR-rui-demon-01

* **ID:** `C-MOR-rui-demon-01`
* **Source:** `cards.json#Mor:（Rui）正论恶魔`
* **Quote:** 「向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the user pays every other player 100 (immune to modifiers),
  then every other player pays the user 300 (modifiable).
* **Frame:** every player's money.
* **Rulings & OPEN:** 「全场玩家」 includes every other living seat. The
  immunity of the 100 is a pipeline property; the clause pins the 100
  leg at face value and the 300 legs at face value when unmodified.
* **Executable:** [`c_mor_rui_demon_money`].

#### C-MOR-rui-demon-02

* **ID:** `C-MOR-rui-demon-02`
* **Source:** `cards.json#Mor:（Rui）正论恶魔`
* **Quote:** 「在此卡结算过程中，每当你的技能被触发后，立即将x设置为5」
* **Trigger:** a trigger of the user's own skill while the card body is
  still resolving.
* **Condition:** the user is 八潮瑠唯 (the only Morfonica skill with X).
* **Effect:** X (`skill.yuriCrit.x`) is set to 5.
* **Frame:** the X counter.
* **Rulings & OPEN:** sheet 2026-10-06 新卡组卡 G15: 「每当…触发后，立即」
  (every fire, immediately). The card body's own money legs are what
  fire the skill.
* **Executable:** [`c_mor_rui_demon_x`].

---

### C-MOR-toko-swap -- Mor:（toko）

Card text (cards.json#Mor:（toko）):

> 选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回

#### C-MOR-toko-swap-01

* **ID:** `C-MOR-toko-swap-01`
* **Source:** `cards.json#Mor:（toko）`
* **Quote:** 「选择一个自己被抵押的地契和任意玩家颜色相同的被抵押地契交换，地契+房屋价格更低的玩家向更高的玩家支付差价，互换后将你得到的地契免费赎回」
* **Trigger:** the play of this card; a swap pair is chosen.
* **Condition:** both deeds are mortgaged and share the board colour
  group; the user owns one of them.
* **Effect:** the two deeds exchange owners; the player whose
  (deed price + house cost) is lower pays the higher the difference; the
  deed the user receives is redeemed (un-mortgaged) for free; the deed
  the user gave away stays mortgaged.
* **Frame:** the two tiles' owners / mortgaged flags, and the two
  players' money.
* **Rulings & OPEN:** 「颜色」 is the board `group`. 「地契+房屋价格」
  sums the deed price and the built-house cost on that tile. The free
  redeem applies only to the deed the user receives.
* **Executable:** [`c_mor_toko_swap`].

---

### C-MOR-xiaobai -- Mor:（小白）

Card text (cards.json#Mor:（小白）):

> [反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金

#### C-MOR-xiaobai-01

* **ID:** `C-MOR-xiaobai-01`
* **Source:** `cards.json#Mor:（小白）`
* **Quote:** 「[反击]当你将要向其他玩家支付时打出此卡，此次支付改为失去同等的资金并令此次支付的对象失去此次金额一半的资金」
* **Trigger:** a [反击] window on a payment the user is about to make to
  another player.
* **Condition:** the user holds the card.
* **Effect:** the payment becomes a pure loss of the same amount by the
  user (the payee receives nothing from this leg), and the intended
  payee loses half that amount.
* **Frame:** the two players' money only.
* **Rulings & OPEN:** half of an odd amount is not spelled out; the
  clause uses the engine's integer half (140/2 = 70).
* **Executable:** [`c_mor_xiaobai`].

---

### C-MOR-garden -- Mor:（筑紫）迷茫的庭园

Card text (cards.json#Mor:（筑紫）迷茫的庭园):

> 获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动

#### C-MOR-garden-01

* **ID:** `C-MOR-garden-01`
* **Source:** `cards.json#Mor:（筑紫）迷茫的庭园`
* **Quote:** 「获得100资金。」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the user gains 100.
* **Frame:** the user's money.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_garden_gain`].

#### C-MOR-garden-02

* **ID:** `C-MOR-garden-02`
* **Source:** `cards.json#Mor:（筑紫）迷茫的庭园`
* **Quote:** 「投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家）」
* **Trigger:** right after the gain.
* **Condition:** --.
* **Effect:** a d6 is rolled; the user [传送]s to the tile one step before
  the chosen player (self → one step forward), treating the teleport as
  the main move. With fewer than 6 players the die wraps (5-player roll
  of 6 → the first player).
* **Frame:** the user's position; the main-move flag for the turn.
* **Rulings & OPEN:** OPEN-行动条 -- 「行动条上」 is the action-order
  track; the chosen player is the Nth in action order.
* **Executable:** [`c_mor_garden_teleport`].

#### C-MOR-garden-03

* **ID:** `C-MOR-garden-03`
* **Source:** `cards.json#Mor:（筑紫）迷茫的庭园`
* **Quote:** 「可以选择是否触发结算。」
* **Trigger:** after the teleport lands.
* **Condition:** --.
* **Effect:** the player may run the landing settle, or skip it. A skip
  means no rent / tile effect from the landing.
* **Frame:** a skip leaves money / ownership unchanged by the landing.
* **Rulings & OPEN:** OPEN-触发结算 (same as noble).
* **Executable:** [`c_mor_garden_settle_choice`].

#### C-MOR-garden-04

* **ID:** `C-MOR-garden-04`
* **Source:** `cards.json#Mor:（筑紫）迷茫的庭园`
* **Quote:** 「直到下个你的回合开始时，你无法被异常移动」
* **Trigger:** the play.
* **Condition:** --.
* **Effect:** until the user's next turn start, [异常移动效果] cannot
  move the user.
* **Frame:** the user's position under foreign move effects.
* **Rulings & OPEN:** OPEN-异常移动 -- the [异常移动效果] set is a
  G-STATUS concern; the clause pins "the user's position does not change
  under a foreign abnormal move during the window".
* **Executable:** [`c_mor_garden_immunity`].

---

## Character skills

### C-MOR-mashiro -- 仓田真白「向后全速前进」

Source: `data/characters.json#仓田真白`.

> （1）[主动移动]时移动掷骰变为2d20（2）反方向移动

#### C-MOR-mashiro-01

* **ID:** `C-MOR-mashiro-01`
* **Source:** `characters.json#仓田真白`
* **Quote:** 「（1）[主动移动]时移动掷骰变为2d20」
* **Trigger:** the main movement dice roll.
* **Condition:** the seat's character is 仓田真白.
* **Effect:** the roll is 2d20 (the sum walks that many tiles).
* **Frame:** the roll / position.
* **Rulings & OPEN:** OPEN-主动移动 -- the book's [主动移动] vs
  [主要移动] spelling; the clause is about the turn's main roll.
* **Executable:** [`c_mor_mashiro_2d20`].

#### C-MOR-mashiro-02

* **ID:** `C-MOR-mashiro-02`
* **Source:** `characters.json#仓田真白`
* **Quote:** 「（2）反方向移动」
* **Trigger:** any walk of this player.
* **Condition:** --.
* **Effect:** the walk goes backwards (decreasing tile index).
* **Frame:** the position.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_mashiro_reverse`].

---

### C-MOR-touko -- 桐谷透子「天上天下，唯我独尊」

Source: `data/characters.json#桐谷透子`.

> （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，指定前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的地租的一半/指定玩家数（向上取整百）。

#### C-MOR-touko-01

* **ID:** `C-MOR-touko-01`
* **Source:** `characters.json#桐谷透子`
* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
* **Trigger:** [经过] CiRCLE.
* **Condition:** the seat's character is 桐谷透子; the fire pot is below
  the cap of 1.
* **Effect:** one fire pot is gained (capped at 1). The pot starts at 1.
* **Frame:** the fire counter.
* **Rulings & OPEN:** 「初始1」 is a deck-at-game-start figure; the clause
  pins the cap and the pass gain.
* **Executable:** [`c_mor_touko_fire`].

#### C-MOR-touko-02

* **ID:** `C-MOR-touko-02`
* **Source:** `characters.json#桐谷透子`
* **Quote:** 「（2）一回合一次，在你的经营阶段，你可以消耗一个火罐并指定你的一个地块，指定前后各一格范围内（不包括该格子本身）的所有其他玩家支付你X，X为此地的地租的一半/指定玩家数（向上取整百）。」
* **Trigger:** the skill press in 运营.
* **Condition:** once this turn; the user has ≥1 fire pot and owns a tile.
* **Effect:** the pot is spent; a tile is designated; every other player
  within ±1 of that tile (excluding the tile itself) pays
  X = ceil100( (that tile's base rent / 2) / n ), where n is the number
  of designated players.
* **Frame:** the fire counter and the money of the designated players.
* **Rulings & OPEN:** 「地租」 is the tile's base rent (the no-house
  figure). 「向上取整百」 rounds X up to a multiple of 100. OPEN-地租档
  -- which rent step (houses) counts as 「此地的地租」 is not spelled
  out; the clause pins the no-house rent (`rent[0]`), matching the
  existing green test.
* **Executable:** [`c_mor_touko_skill`].

---

### C-MOR-nanami -- 广町七深「这是很普通的事吧？」

Source: `data/characters.json#广町七深`.

> （1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）（2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）（3）在时机合适时候，你可以使用手中的角色标记，视为使用该角色的技能（尽力补齐资源消耗，比如双[火罐]消耗技能和翻转[P✽P粉丝]）。

#### C-MOR-nanami-01

* **ID:** `C-MOR-nanami-01`
* **Source:** `characters.json#广町七深`
* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）」
* **Trigger:** [经过] CiRCLE.
* **Condition:** the seat's character is 广町七深; fire < 2.
* **Effect:** one fire pot is gained (capped at 2).
* **Frame:** the fire counter.
* **Rulings & OPEN:** none.
* **Executable:** [`c_mor_nanami_fire`].

#### C-MOR-nanami-02

* **ID:** `C-MOR-nanami-02`
* **Source:** `characters.json#广町七深`
* **Quote:** 「（2）当有玩家在使用角色技能时，你可以支付一个火罐得到一个该角色的标记（当场上存活玩家数量大于等于3时：每种角色标记上限1，该技能对被动技能存在3回合CD）」
* **Trigger:** another player's character-skill use.
* **Condition:** the user has ≥1 fire pot; with ≥3 living players the
  user holds <1 mark of that character.
* **Effect:** one fire pot is spent and one mark of that character is
  gained. Passive skills carry a 3-turn CD on the claim.
* **Frame:** the fire counter and the mark stock.
* **Rulings & OPEN:** OPEN-角色标记 (same as NNM); OPEN-被动CD -- the
  CD shape of a passive skill claim is not defined beyond 「3回合CD」.
* **Executable:** documented, not directly driveable without a foreign
  skill press in the same window; see the doc note below.

#### C-MOR-nanami-03

* **ID:** `C-MOR-nanami-03`
* **Source:** `characters.json#广町七深`
* **Quote:** 「（3）在时机合适时候，你可以使用手中的角色标记，视为使用该角色的技能（尽力补齐资源消耗，比如双[火罐]消耗技能和翻转[P✽P粉丝]）。」
* **Trigger:** an appropriate timing for the borrowed skill.
* **Condition:** the user holds a mark of that character.
* **Effect:** the mark is spent and the character's skill is used as if
  it were the user's, with resource costs filled from the mark.
* **Frame:** the mark stock and the borrowed skill's paths.
* **Rulings & OPEN:** OPEN-借用技能; OPEN-时机合适 -- the gate is not
  defined. Untestable as a black-box contract without a host skill-press
  harness for every borrowed skill; documented only.
* **Executable:** documented only (see Findings / untestable).

---

### C-MOR-tsukushi -- 二叶筑紫「交给班长吧」

Source: `data/characters.json#二叶筑紫`.

> （1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量，若为抽卡效果则记录2200。单轮结算（比如登上武道馆的结算）中此效果最多触发一次（2）当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作。以此效果获得的金钱数量不能超过记录数字+300，超出部分由原主获得。此效果对“抽牌”动作发动时必须翻面记录数字为2200及以上的标记。（3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得移除标记数量*800的资金（每次发动（3）技能时单个标记的收益减200，最低单标记收益200）。

#### C-MOR-tsukushi-01

* **ID:** `C-MOR-tsukushi-01`
* **Source:** `characters.json#二叶筑紫`
* **Quote:** 「（1）当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色进行一次该动作，发送给对方一个X（占位）标记并记录获得因此效果获得资金的数量，若为抽卡效果则记录2200。单轮结算（比如登上武道馆的结算）中此效果最多触发一次」
* **Trigger:** a gain of money or a draw by the user.
* **Condition:** the skill has not already fired in this settle chain.
* **Effect:** the action is redirected to another player; that player
  gets an X mark; the amount is recorded (draw records 2200).
* **Frame:** the receiving player's money / hand and the X mark stock.
* **Rulings & OPEN:** OPEN-X标记 -- the mark shape / name is engine-side.
  「单轮结算」 is one settle chain.
* **Executable:** [`c_mor_tsukushi_redirect`].

#### C-MOR-tsukushi-02

* **ID:** `C-MOR-tsukushi-02`
* **Source:** `characters.json#二叶筑紫`
* **Quote:** 「（2）当有角色获得金钱或抽卡时，你可以将对方的X标记翻面并代替其进行一次该动作。以此效果获得的金钱数量不能超过记录数字+300，超出部分由原主获得。此效果对“抽牌”动作发动时必须翻面记录数字为2200及以上的标记。」
* **Trigger:** another player's gain of money or draw.
* **Condition:** that player holds a face-up X mark (for a draw, the
  recorded figure is ≥ 2200).
* **Effect:** the mark is flipped and the user replaces the action; the
  money the user takes is capped at recorded + 300, the excess goes to
  the original player.
* **Frame:** the two players' money / hand and the mark.
* **Rulings & OPEN:** OPEN-X标记.
* **Executable:** [`c_mor_tsukushi_replace`].

#### C-MOR-tsukushi-03

* **ID:** `C-MOR-tsukushi-03`
* **Source:** `characters.json#二叶筑紫`
* **Quote:** 「（3）当场上除你之外的玩家都拥有一枚翻面后的X标记时，移除所有X标记，你获得移除标记数量*800的资金（每次发动（3）技能时单个标记的收益减200，最低单标记收益200）。」
* **Trigger:** every other living player holds a flipped X mark.
* **Condition:** --.
* **Effect:** all X marks are removed and the user gains
  `n × per_mark`, where `per_mark` starts at 800 and drops by 200 on
  every later (3) fire, floor 200.
* **Frame:** the X mark stock and the user's money.
* **Rulings & OPEN:** OPEN-X标记; the decay counter of 「每次发动」 is
  engine-side.
* **Executable:** [`c_mor_tsukushi_sweep`].

---

### C-MOR-rui-crit -- 八潮瑠唯「正论暴击」

Source: `data/characters.json#八潮瑠唯`.

> （1）如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能（2）（抵押或赎回以外）当你即将获得或失去资金（在所有其他资金量改动效果生效后并结果不等于0）时，roll1d20，若结果小于等于X，将X重置为0，此次获得资金翻倍或无需失去资金（收款或付款方同样受到影响），若结果大于X，X+1。X初始为0

#### C-MOR-rui-crit-01

* **ID:** `C-MOR-rui-crit-01`
* **Source:** `characters.json#八潮瑠唯`
* **Quote:** 「（1）如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能（2）」
* **Trigger:** a copier of the (2) skill resets their X to 0.
* **Condition:** the skill (2) or character card is being copied.
* **Effect:** the copier loses the (2) skill.
* **Frame:** the copier's skill set.
* **Rulings & OPEN:** OPEN-复制 -- the copy mechanism is not defined in
  the rulebook body. Untestable as a black-box contract; documented only.
* **Executable:** documented only.

#### C-MOR-rui-crit-02

* **ID:** `C-MOR-rui-crit-02`
* **Source:** `characters.json#八潮瑠唯`
* **Quote:** 「（抵押或赎回以外）当你即将获得或失去资金（在所有其他资金量改动效果生效后并结果不等于0）时，roll1d20，若结果小于等于X，将X重置为0，此次获得资金翻倍或无需失去资金（收款或付款方同样受到影响），若结果大于X，X+1。X初始为0」
* **Trigger:** about to gain or lose a non-zero amount of money (not
  mortgage / redeem), after all other money modifiers.
* **Condition:** the seat's character is 八潮瑠唯.
* **Effect:** 1d20 is rolled. On ≤ X: X resets to 0 and the gain doubles
  (or the loss is cancelled); both sides of a transfer are affected. On
  > X: X increments by 1. X starts at 0.
* **Frame:** the money of the affected seats and the X counter.
* **Rulings & OPEN:** OPEN-双方影响 -- 「收款或付款方同样受到」 is read
  as the transfer amount changing for both sides (the engine's current
  shape). The 1d20 is the engine RNG (not a loaded die).
* **Executable:** [`c_mor_rui_crit`].

---

## Band skill

### C-MOR-band -- 振翅高飞的练习曲

Source: `data/bands.json#Morfonica`.

> （1）当你一次性失去2000以上资金时，抽一张卡。（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环

#### C-MOR-band-01

* **ID:** `C-MOR-band-01`
* **Source:** `bands.json#Morfonica`
* **Quote:** 「（1）当你一次性失去2000以上资金时，抽一张卡。」
* **Trigger:** a single loss of ≥ 2000 money.
* **Condition:** the seat carries the Morfonica band skill.
* **Effect:** the player draws one card.
* **Frame:** the player's hand / draw / discard and money.
* **Rulings & OPEN:** 「一次性」 is one loss event, not a sum of several.
  「2000以上」 is inclusive of 2000 (the sheet's 「以上」 convention).
* **Executable:** [`c_mor_band_draw`].

#### C-MOR-band-02

* **ID:** `C-MOR-band-02`
* **Source:** `bands.json#Morfonica`
* **Quote:** 「（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环」
* **Trigger:** a [CiRCLE奖励] choice of money.
* **Condition:** the seat carries the Morfonica band skill.
* **Effect:** the amount is 1000 / 1500 / 2000 cycling by the count of
  money rewards taken so far (1st = 1000, 2nd = 1500, 3rd = 2000, 4th =
  1000, ...).
* **Frame:** the player's money.
* **Rulings & OPEN:** the cycle is per player (「选择[获得]资金次数」).
* **Executable:** [`c_mor_band_cycle`].

---

## Open questions

* **OPEN-移动起点** -- 「此次移动」 in `Mor:你的光芒将照亮前路` (does it
  replace the main move, or grant a free move?).
* **OPEN-取消范围** -- `Mor:再次牵起手来` [持续] 「取消此次资金变动」:
  one-sided (owner only) or both legs of a transfer?
* **OPEN-指定** -- 「指定」 (designation) as used by 夏日合宿 / 高贵的微蓝.
* **OPEN-无效化** -- `Mor:离心力，不为所动` 「无效化你受到的所有效果」
  scope.
* **OPEN-X** -- `Mor:蝴蝶飞舞的星月夜` X range (engine offers 1..=5).
* **OPEN-场地中央** -- 「场地中央」 as a board slot (the card is a field
  card of the user).
* **OPEN-重骰次数** -- starry's re-roll is once; the book does not say
  the second result is final (it is implied).
* **OPEN-三全音收款人** -- payee of the 三全音 payback (bank vs original
  payee). TEST-FINDINGS §6 records the gap.
* **OPEN-触发结算** -- 「触发结算」 as a forced re-settle (高贵的微蓝,
  迷茫的庭园). The OPEN index names [触发结算].
* **OPEN-角色标记** -- the 角色标记 namespace / shape (NNM, 广町七深).
* **OPEN-多分支选择** -- NNM's 「发动以下效果中的一个」 vs the engine's
  multi-activation dispatch (TEST-FINDINGS §6 item 8). Genuine
  discrepancy.
* **OPEN-借用技能** -- 「你的（2）技能」 / 「视为使用该角色的技能」.
* **OPEN-地租档** -- which rent step counts as 「此地的地租」 in 桐谷透子
  (2).
* **OPEN-被动CD** -- the 3-turn CD of a passive-skill mark claim.
* **OPEN-行动条** -- 「行动条上」 (迷茫的庭园).
* **OPEN-异常移动** -- the [异常移动效果] set (迷茫的庭园 immunity).
* **OPEN-X标记** -- the X (占位) mark namespace / shape (二叶筑紫).
* **OPEN-复制** -- the copy mechanism (八潮瑠唯 (1)).
* **OPEN-双方影响** -- 「收款或付款方同样受到」 (八潮瑠唯 (2)).
* **OPEN-主动移动** -- [主动移动] vs [主要移动] spelling (仓田真白).

## Untestable clauses

* `C-MOR-nanami-02` / `C-MOR-nanami-03` -- need a foreign character-skill
  press and a borrowed-skill harness in the same window; the black-box
  Table cannot arrange 「有玩家在使用角色技能时」 as an observed input.
* `C-MOR-rui-crit-01` -- needs a skill-copy mechanism (OPEN-复制).
* `C-MOR-garden-04`'s full [异常移动效果] set is a G-STATUS concern; the
  Mor contract pins only "a foreign abnormal move leaves the position
  unchanged" with one concrete card.

## Findings

* **LESS (C-MOR-nnm-05)** -- genuine, stays failing: 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」
  -- spending the last crystal via the (3) press leaves the card on the
  field with 0 crystals. The press itself (C-MOR-nnm-04) does remove the
  crystal; the empty-card discard does not run.
* **C-MOR-tritone-02** (OPEN, not a finding): the payback recipient is
  undefined; the contract pins the amount and the discard only.

Previously recorded TEST-FINDINGS §4 / §6 Morfonica gaps that this unit's
contracts now see **green** (the engine honours the book; the older notes
are stale):

* 纯真振翅 「立刻进行移动掷骰」 (`C-MOR-wing-02`) -- the follow-on roll
  lands jumped+walk.
* 星月夜 「当其他角色的移动掷骰结果是偶数时…」 (`C-MOR-starry-04`) -- the
  offer prompts.
* （NNM）稍微努力了一下 hand play (`C-MOR-nnm-01` / `-02`) -- the
  mark-spending choice runs and branch （1） draws x.

