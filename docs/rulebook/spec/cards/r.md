# C-R -- Roselia (Group R)

Unit 11 owns this file and `rules/tests/tests/spec_card_r.rs`. Scope: all 16 `R:`
entries in `docs/rulebook/cards.json`, the skills of every Roselia character in
`data/characters.json` (`band == "Roselia"`), and the Roselia band skill in
`data/bands.json`.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

**Livehouse = colour group 6 buyable deeds.** `data/board.json` groups the
Live House colour (`#CD1146`, `group == 6`) as: `RiNG 1..4`,
`DUB MUSIC EXPERIMENT`, `武道馆`, `Space`, `Live House Galaxy` (all buyable)
and the `Live House` agent (not buyable). Every 「Livehouse格子」 /
「Live House格子」 / 「Livehouse地块」 in this group's text is that buyable
set. A bare 「RiNG」 / 「“RiNG”」 names only the four `kind == "ring"` tiles.

Clause IDs: `C-R-<slug>-<n>`, `<slug>` the card / skill short name (band
prefix dropped).

---

## R:Fire bird

**Source:** `cards.json#R:Fire bird`

> 支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶，自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶，自己的所有格子收费变成1.5倍，奇迹水晶耗尽时将此卡放入弃牌堆

### C-R-Fire bird-01 -- pay 1600, place, add X crystals

* **ID:** `C-R-Fire bird-01`
* **Source:** `cards.json#R:Fire bird`
* **Quote:** 「支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶」
* **Trigger:** the [手] play of this card.
* **Condition:** the user has ≥1600 money and chooses some X.
* **Effect:** user's money falls by exactly 1600; the card is on the user's
  field; the placed instance carries exactly the chosen X [奇迹水晶].
* **Rulings & OPEN:** the book's 「X（自选）」 states no range. X = 0 would
  exhaust immediately (clause 04); the executable contract asks for a
  positive X and pins the count. OPEN-FIRE-X if a 0-or->10 choice matters.
* **Frame:** only the user's money / hand / field / discard / draw move.
* **Rulings & OPEN:** see the Condition note (OPEN-FIRE-X).
* **Executable:** `fire_bird_place` / `fire_bird_place_contract`.

### C-R-Fire bird-02 -- turn-end burn

* **ID:** `C-R-Fire bird-02`
* **Source:** `cards.json#R:Fire bird`
* **Quote:** 「自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶」
* **Trigger:** a turn-end window while the card sits on its owner's field.
* **Condition:** the placed instance holds ≥1 [奇迹水晶].
* **Effect:** the owner loses exactly 400 and the instance loses exactly 1
  crystal.
* **Frame:** only that player's money and that field instance's crystals.
* **Rulings & OPEN:** 「每回合结束时」 is every turn's end (not only the
  owner's). Spec checks a turn-end transition that reports the burn.
* **Executable:** `fire_bird_burn`.

### C-R-Fire bird-03 -- rent ×1.5

* **ID:** `C-R-Fire bird-03`
* **Source:** `cards.json#R:Fire bird`
* **Quote:** 「自己的所有格子收费变成1.5倍」
* **Trigger:** a settle that pays rent to the card's owner.
* **Condition:** the card is on the owner's field with ≥1 crystal.
* **Effect:** the rent collected is 1.5× the unmodified rent.
* **Frame:** only the rent pair's money.
* **Rulings & OPEN:** stacking with other multipliers is OPEN-R1 (TEST-FINDINGS
  §6 item R-1). Rounding of a non-integer 1.5× is OPEN-ROUND10 (the book
  does not say; the executable pins the integer case 140 → 210).
* **Executable:** `fire_bird_rent`.

### C-R-Fire bird-04 -- discard on empty crystals

* **ID:** `C-R-Fire bird-04`
* **Source:** `cards.json#R:Fire bird`
* **Quote:** 「奇迹水晶耗尽时将此卡放入弃牌堆」
* **Trigger:** the instance's crystal count reaches 0.
* **Condition:** the card was on the owner's field.
* **Effect:** the card is in the owner's discard (and not on the field).
* **Frame:** the owner's field / discard.
* **Rulings & OPEN:** none.
* **Executable:** `fire_bird_exhaust`.

---

## R:NFO

**Source:** `cards.json#R:NFO`

> 投掷1d6并在回合结束后获得一层[停留]。若投掷结果为1，获得1500资金；若结果为2，立刻移动到你前方最近一名玩家的前方一格并使其格子上的玩家分摊支付你1000资金，视为本回合的主要移动但不[触发结算]；若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆；若结果为4，选择你弃牌堆中的一张满足打出条件的卡打出；若结果为5，为自己的角色卡添加6个奇迹水晶，你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300；若结果为6，依次获得结果1-5的全部效果。（若结果严格大于6，则从1开始重新计数）

### C-R-NFO-01 -- roll 1d6

* **ID:** `C-R-NFO-01`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「投掷1d6」
* **Trigger:** the [手] play of this card.
* **Condition:** --.
* **Effect:** the window reports a 1d6 roll (face in 1..=6).
* **Frame:** --.
* **Rulings & OPEN:** `data/cards.json` reads 「投掷1d6mod6」; the live sheet
  (`docs/rulebook/cards.json`) is authoritative and says 1d6. Faces >6 wrap
  (clause 09).
* **Executable:** `nfo_roll`.

### C-R-NFO-02 -- [停留] after the turn ends

* **ID:** `C-R-NFO-02`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「并在回合结束后获得一层[停留]」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** after the current turn ends the user wears 1 layer of [停留].
* **Frame:** the user's status.
* **Rulings & OPEN:** none.
* **Executable:** `nfo_stay`.

### C-R-NFO-03 -- face 1 = +1500

* **ID:** `C-R-NFO-03`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「若投掷结果为1，获得1500资金」
* **Trigger:** the play, when the d6 face is 1 (or 7, 13, ... -- clause 09).
* **Condition:** --.
* **Effect:** the user gains exactly 1500.
* **Frame:** only the user's money among money fields.
* **Rulings & OPEN:** none.
* **Executable:** `nfo_face1`.

### C-R-NFO-04 -- face 2 = jump + 分摊 1000

* **ID:** `C-R-NFO-04`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「若结果为2，立刻移动到你前方最近一名玩家的前方一格并使其格子上的玩家分摊支付你1000资金，视为本回合的主要移动但不[触发结算]」
* **Trigger:** the play, face 2.
* **Condition:** there is another player ahead on the path.
* **Effect:** the user lands on the tile one ahead of that nearest player;
  the players on **that nearest player's tile** (「其格子」) pay a total of
  1000 to the user (分摊); the move is the turn's main move; no [触发结算]
  runs at the landing.
* **Frame:** the mover's `pos`, the payers' and mover's `money`, `turn.main_moved`.
* **Rulings & OPEN:** 「其格子」 is read as the nearest player's tile (the
  last named entity), not the landing tile one ahead. OPEN-分摊 (how 1000
  is split is undefined); OPEN-触发结算 (「不[触发结算]」). Spec checks the
  landing tile and that the mover's money rose by exactly 1000 and the
  payers on the nearest player's tile paid a total of 1000, without
  asserting the per-payer split.
* **Executable:** `nfo_face2`.

### C-R-NFO-05 -- face 3 = field + 1000 off the next pay

* **ID:** `C-R-NFO-05`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「若结果为3，将此卡置于场上，你下次付款时自动减免1000资金的消耗并将此卡置入弃牌堆」
* **Trigger:** the play, face 3.
* **Condition:** --.
* **Effect:** the card is on the user's field; the next payment the user makes
  is reduced by 1000; then the card goes to the discard.
* **Frame:** the user's field / discard / money (when the discount fires).
* **Rulings & OPEN:** 「下次付款」 scope (which payments count) is not
  enumerated; spec checks the discount on an ordinary rent payment.
* **Executable:** `nfo_face3`.

### C-R-NFO-06 -- face 4 = replay a discard card

* **ID:** `C-R-NFO-06`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「若结果为4，选择你弃牌堆中的一张满足打出条件的卡打出」
* **Trigger:** the play, face 4.
* **Condition:** the discard holds at least one card that meets its play
  condition.
* **Effect:** one such card is played (its own [手] body runs).
* **Frame:** that card's own contract.
* **Rulings & OPEN:** 「满足打出条件」 reuses each card's own gate; which
  discard card is chosen is free.
* **Executable:** `nfo_face4` (scenario; see doc note under Findings if the
  engine refuses).

### C-R-NFO-07 -- face 5 = 6 crystals on the character card + spend-to-boost

* **ID:** `C-R-NFO-07`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「若结果为5，为自己的角色卡添加6个奇迹水晶，你每次获得资金时，可消耗一个奇迹水晶使本次的额度提高300」
* **Trigger:** the play, face 5; later, each time the user gains money.
* **Condition:** --.
* **Effect:** the character card carries +6 [奇迹水晶]; each later gain may
  spend 1 crystal to raise that gain by 300.
* **Frame:** the character-card crystal count and the gain amount.
* **Rulings & OPEN:** OPEN-场上 -- 「为自己的角色卡添加6个奇迹水晶」 uses
  the character card as the crystal host, which the book's 「[场上]」 gloss
  does not define. Spec checks a +6 crystal stock on a host the engine names
  as the character card, and the optional +300 spend.
* **Executable:** `nfo_face5`.

### C-R-NFO-08 -- face 6 = 1-5 in order

* **ID:** `C-R-NFO-08`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「若结果为6，依次获得结果1-5的全部效果。」
* **Trigger:** the play, face 6.
* **Condition:** --.
* **Effect:** clauses 03-07 all run, in order 1→5.
* **Frame:** the union of 03-07's frames.
* **Rulings & OPEN:** order is stated (「依次」); interleaving with other
  effects is not.
* **Executable:** `nfo_face6` (checks at least the +1500 of face 1).

### C-R-NFO-09 -- faces >6 wrap

* **ID:** `C-R-NFO-09`
* **Source:** `cards.json#R:NFO`
* **Quote:** 「（若结果严格大于6，则从1开始重新计数）」
* **Trigger:** the play, when the rolled face is strictly greater than 6.
* **Condition:** --.
* **Effect:** the face is reduced modulo the 1-6 cycle (7→1, 8→2, ...).
* **Frame:** --.
* **Rulings & OPEN:** the wrap formula 「从1开始重新计数」 is read as
  face' = ((face - 1) mod 6) + 1.
* **Executable:** not tested -- `Table::dice` clamps loaded faces to the
  die's sides, so a face of 7 cannot be forced. The wrap formula is
  recorded; no red test.

---

## R:Sprechchor

**Source:** `cards.json#R:Sprechchor`

> 在Livehouse地块开始回合时，可打出此卡并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目

### C-R-Sprechchor-01 -- timing gate

* **ID:** `C-R-Sprechchor-01`
* **Source:** `cards.json#R:Sprechchor`
* **Quote:** 「在Livehouse地块开始回合时，可打出此卡」
* **Trigger:** the start of the user's turn while standing on a Livehouse (RiNG) tile.
* **Condition:** --.
* **Effect:** the card may be played from hand (optional).
* **Frame:** --.
* **Rulings & OPEN:** none.
* **Executable:** `sprechchor_gate` (refusal off-Livehouse).

### C-R-Sprechchor-02 -- 1000 + X*120

* **ID:** `C-R-Sprechchor-02`
* **Source:** `cards.json#R:Sprechchor`
* **Quote:** 「并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目」
* **Trigger:** the play (gate satisfied).
* **Condition:** --.
* **Effect:** a 1d20 is rolled and the user gains exactly `1000 + X*120`.
* **Frame:** only the user's money (and the card zones).
* **Rulings & OPEN:** none.
* **Executable:** `sprechchor_pay`.

---

## R:[衍生] 觉悟

**Source:** `cards.json#R:[衍生] 觉悟`

> [特]：此卡[移除]时获得1000资金。
> [手]：[移除]此卡。

### C-R-觉悟-01 -- [特] pays 1000 on [移除]

* **ID:** `C-R-觉悟-01`
* **Source:** `cards.json#R:[衍生] 觉悟`
* **Quote:** 「[特]：此卡[移除]时获得1000资金。」
* **Trigger:** this card is [移除]d (from any zone).
* **Condition:** --.
* **Effect:** the user gains exactly 1000.
* **Frame:** the user's money and the card's zones.
* **Rulings & OPEN:** 「获得」 subject is the card's user (house style).
* **Executable:** `resolve_removed_pays`.

### C-R-觉悟-02 -- [手] removes it

* **ID:** `C-R-觉悟-02`
* **Source:** `cards.json#R:[衍生] 觉悟`
* **Quote:** 「[手]：[移除]此卡。」
* **Trigger:** the [手] play of this card.
* **Condition:** the card is in hand.
* **Effect:** the card is [移除]d (not in hand / discard / draw / field); then
  clause 01 pays.
* **Frame:** the user's card zones and money.
* **Rulings & OPEN:** none.
* **Executable:** `resolve_hand_removes`.

---

## R:live前的准备

**Source:** `cards.json#R:live前的准备`

> [反击] 经过江户川乐器店时可打出此卡，使自己在江户川乐器店强制停下并触发结算

### C-R-live前的准备-01 -- [反击] while passing the store

* **ID:** `C-R-live前的准备-01`
* **Source:** `cards.json#R:live前的准备`
* **Quote:** 「[反击] 经过江户川乐器店时可打出此卡」
* **Trigger:** the user's path [经过]s tile 江户川乐器店 (index 30).
* **Condition:** the card is in a [反击]-playable zone.
* **Effect:** a [反击] window offers this card (optional).
* **Frame:** --.
* **Rulings & OPEN:** OPEN-强制停下 is truncated in the book; the stop
  landing itself is still checkable (clause 02).
* **Executable:** `live_prep_window`.

### C-R-live前的准备-02 -- force stop + settle

* **ID:** `C-R-live前的准备-02`
* **Source:** `cards.json#R:live前的准备`
* **Quote:** 「使自己在江户川乐器店强制停下并触发结算」
* **Trigger:** the card is played in the window of clause 01.
* **Condition:** --.
* **Effect:** the user's piece is on 江户川乐器店 after the move and a settle
  of that tile runs.
* **Frame:** the mover's `pos` and the settle's own money / zones.
* **Rulings & OPEN:** OPEN-触发结算 (the settle body is undefined).
* **Executable:** `live_prep_stop`.

---

## R:向着顶点

**Source:** `cards.json#R:向着顶点`

> 移动到下一个可被购买的livehouse格子，视为你的主要移动。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。

### C-R-向着顶点-01 -- move to the next buyable Livehouse

* **ID:** `C-R-向着顶点-01`
* **Source:** `cards.json#R:向着顶点`
* **Quote:** 「移动到下一个可被购买的livehouse格子，视为你的主要移动。」
* **Trigger:** the [手] play while some Livehouse tile is unowned.
* **Condition:** at least one Livehouse tile has no owner.
* **Effect:** the user lands on the next such tile clockwise from their
  position, and the turn's main move is spent.
* **Frame:** the mover's `pos`, `turn.main_moved`.
* **Rulings & OPEN:** 「可被购买」 is read as unowned (an unowned deed is the
  buyable state; mortgage / other gates are not named).
* **Executable:** `toward_top_move`.

### C-R-向着顶点-02 -- else build at 1.5×

* **ID:** `C-R-向着顶点-02`
* **Source:** `cards.json#R:向着顶点`
* **Quote:** 「若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。」
* **Trigger:** the [手] play while every Livehouse tile is owned.
* **Condition:** the user owns at least one Livehouse tile.
* **Effect:** the user may pay 1.5× that tile's one-house build price to add
  one house (optional).
* **Frame:** that tile's `houses`, the user's money.
* **Rulings & OPEN:** OPEN-RING-BUILD -- `rulebook-doc.md:157` says
  「所有RiNG不可升级」, which would make this branch dead on RiNG tiles.
  The card text is newer sheet text and is taken as the live rule for this
  card; the conflict is listed under Open questions.
* **Executable:** `toward_top_build` (scenario).

---

## R:学生会的检查

**Source:** `cards.json#R:学生会的检查`

> 移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出，向抽牌堆中加入一张“觉悟”，本回合无法加盖房屋，但可支付那格一层房屋的建造价格一半将此卡放于那个格子上，使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算。因此卡强制停下的玩家的结算地租价格为原价格一半。若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。

### C-R-学生会的检查-01 -- gate: owned deed within ±3

* **ID:** `C-R-学生会的检查-01`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「移动结束后前后三格内若存在你拥有地契的格子」
* **Trigger:** after the user's move ends.
* **Condition:** some tile within 3 steps forward or backward of the landing
  tile is owned by the user.
* **Effect:** the card may be played before [触发结算].
* **Frame:** --.
* **Rulings & OPEN:** 「前后三格」 is the path-distance ring of radius 3
  (inclusive), not a Euclidean neighbour. 「[触发结算]前」 is
  OPEN-触发结算.
* **Executable:** `student_council_gate`.

### C-R-学生会的检查-02 -- add 觉悟 to the draw pile

* **ID:** `C-R-学生会的检查-02`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「向抽牌堆中加入一张“觉悟”」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the user's draw pile gains one `R:[衍生] 觉悟`.
* **Frame:** the user's draw pile.
* **Rulings & OPEN:** the derived card's full id is `R:[衍生] 觉悟`.
* **Executable:** `student_council_add_resolve`.

### C-R-学生会的检查-03 -- no building this turn

* **ID:** `C-R-学生会的检查-03`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「本回合无法加盖房屋」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** for the rest of the turn the user cannot add houses.
* **Frame:** the user's / turn's build gate.
* **Rulings & OPEN:** none.
* **Executable:** `student_council_no_build`.

### C-R-学生会的检查-04 -- place at half house cost

* **ID:** `C-R-学生会的检查-04`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「但可支付那格一层房屋的建造价格一半将此卡放于那个格子上」
* **Trigger:** the play of this card (optional part).
* **Condition:** the gated tile is one the user owns and can pay for.
* **Effect:** the user pays half of that tile's one-house build price and the
  card is placed on that tile.
* **Frame:** the user's money, the tile's card placement.
* **Rulings & OPEN:** rounding of 「建造价格一半」 is not stated.
* **Executable:** `student_council_place`.

### C-R-学生会的检查-05 -- forced stop of a passer

* **ID:** `C-R-学生会的检查-05`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「使下一个经过且移动终点不在此各的你以外的玩家强制停下并触发结算」
* **Trigger:** another player's path [经过]s the card's tile and their move
  end is not that tile.
* **Condition:** that player is not the card's owner.
* **Effect:** that player force-stops on the tile and settles.
* **Frame:** the passer's `pos` and the settle.
* **Rulings & OPEN:** 「此各」 is a typo for 「此格」. OPEN-触发结算.
* **Executable:** `student_council_force_stop` (scenario).

### C-R-学生会的检查-06 -- half rent for the stopped player

* **ID:** `C-R-学生会的检查-06`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「因此卡强制停下的玩家的结算地租价格为原价格一半。」
* **Trigger:** the settle of clause 05.
* **Condition:** --.
* **Effect:** the rent that player pays is half the original price.
* **Frame:** the rent pair's money.
* **Rulings & OPEN:** 「原价格」 is the unmodified rent.
* **Executable:** `student_council_half_rent` (scenario).

### C-R-学生会的检查-07 -- refund on under-collection

* **ID:** `C-R-学生会的检查-07`
* **Source:** `cards.json#R:学生会的检查`
* **Quote:** 「若此卡进入弃牌堆时结算获得的资金小于本卡原应拿到的资金或未结算则获得发动此卡消耗的资金。」
* **Trigger:** this card enters the discard.
* **Condition:** the settle it set up paid less than 「本卡原应拿到的资金」,
  or never settled.
* **Effect:** the owner gains the money spent to activate the card.
* **Frame:** the owner's money.
* **Rulings & OPEN:** 「本卡原应拿到的资金」 is undefined (the amount the
  forced-stop settle should have produced). Spec only documents the clause;
  it is not asserted. OPEN-STUDENT-REFUND.
* **Executable:** documented, not tested (see above).

---

## R:必然的联系（莉莎）

**Source:** `cards.json#R:必然的联系（莉莎）`

> 【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。

### C-R-必然的联系-01 -- [反击] after a skill teleport

* **ID:** `C-R-必然的联系-01`
* **Source:** `cards.json#R:必然的联系（莉莎）`
* **Quote:** 「【反击】当你使用技能进行传送后，你可以打出此卡」
* **Trigger:** the user has just [传送]ed via a skill.
* **Condition:** the card is in a [反击]-playable zone.
* **Effect:** a [反击] window offers this card (optional).
* **Frame:** --.
* **Rulings & OPEN:** none.
* **Executable:** `lisa_link_window` (scenario).

### C-R-必然的联系-02 -- designate a co-located character

* **ID:** `C-R-必然的联系-02`
* **Source:** `cards.json#R:必然的联系（莉莎）`
* **Quote:** 「并指定一个和你在同一地块的角色」
* **Trigger:** the play of this card.
* **Condition:** some other character stands on the user's tile.
* **Effect:** that character is designated.
* **Frame:** --.
* **Rulings & OPEN:** none.
* **Executable:** covered by clause 03.

### C-R-必然的联系-03 -- place on their play area + drop to discard

* **ID:** `C-R-必然的联系-03`
* **Source:** `cards.json#R:必然的联系（莉莎）`
* **Quote:** 「将此卡放在对方的游戏区，…当你发动该效果后，此卡进入弃牌堆。」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the card is placed on the designated player's play area (field),
  then (after the effect) it enters the discard.
* **Frame:** the two players' field / discard.
* **Rulings & OPEN:** the two clauses are one activation: place, grant the
  ignore, then discard. Spec checks the post-state (discard) and the
  placement window if the engine exposes it.
* **Executable:** `lisa_link_place`.

### C-R-必然的联系-04 -- ignore 【距离最近】

* **ID:** `C-R-必然的联系-04`
* **Source:** `cards.json#R:必然的联系（莉莎）`
* **Quote:** 「你对对方使用技能将可以无视【距离最近】这一限制。」
* **Trigger:** after clause 03, while the effect is live.
* **Condition:** the user targets the designated player with a skill that
  has a 【距离最近】 limit.
* **Effect:** the limit is ignored.
* **Frame:** --.
* **Rulings & OPEN:** 「【距离最近】」 is only named, never defined as a
  keyword; the one live user is 今井莉莎's own skill 2. Spec documents the
  clause; asserting "limit ignored" needs a second skill text that names the
  limit and is not in this unit.
* **Executable:** documented, not tested.

---

## R:曲奇时间

**Source:** `cards.json#R:曲奇时间`

> 将自己弃牌堆的卡全部返回抽牌堆并洗切，获得500*X资金，X为返回卡的总数。

### C-R-曲奇时间-01 -- discard → draw, shuffled

* **ID:** `C-R-曲奇时间-01`
* **Source:** `cards.json#R:曲奇时间`
* **Quote:** 「将自己弃牌堆的卡全部返回抽牌堆并洗切」
* **Trigger:** the [手] play of this card.
* **Condition:** the discard is non-empty (a copy of this card leaving for
  the discard is not among 「弃牌堆的卡」 at play time).
* **Effect:** every card that was in the discard is now in the draw pile; the
  discard is empty of those copies; the draw pile is shuffled.
* **Frame:** the user's draw / discard / hand.
* **Rulings & OPEN:** shuffle order is not asserted (only the multiset).
* **Executable:** `cookie_return`.

### C-R-曲奇时间-02 -- +500·X

* **ID:** `C-R-曲奇时间-02`
* **Source:** `cards.json#R:曲奇时间`
* **Quote:** 「获得500*X资金，X为返回卡的总数。」
* **Trigger:** the same play.
* **Condition:** X cards were returned.
* **Effect:** the user gains exactly `500 * X`.
* **Frame:** the user's money.
* **Rulings & OPEN:** none.
* **Executable:** `cookie_pay`.

---

## R:蓝玫瑰的骄傲

**Source:** `cards.json#R:蓝玫瑰的骄傲`

> 打出此卡时，
> （1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金 。
> （2）若严格少于，则传送至下一个未被购买的Livehouse格子；若此时已没有未被购买的Livehouse格子，则传送至“RiNG 4”（不触发结算），均视为你的主要移动。

### C-R-蓝玫瑰的骄傲-01 -- (1) 20% of Livehouse deeds

* **ID:** `C-R-蓝玫瑰的骄傲-01`
* **Source:** `cards.json#R:蓝玫瑰的骄傲`
* **Quote:** 「（1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金 。」
* **Trigger:** the [手] play, when `#(owned Livehouse) ≥ #(owned non-Livehouse)`.
* **Condition:** the user owns at least one Livehouse tile.
* **Effect:** the user gains 20% of the sum of those Livehouse tiles' deed
  values (tile price).
* **Frame:** the user's money and card zones.
* **Rulings & OPEN:** 「地契价值」 is the printed tile price
  (`data/board.json` `price`). Rounding of 20% is OPEN-ROUND10; the
  executable pins the integer case 1000 → 200.
* **Executable:** `blue_rose_payout`.

### C-R-蓝玫瑰的骄傲-02 -- (2) teleport to the next unbought Livehouse

* **ID:** `C-R-蓝玫瑰的骄傲-02`
* **Source:** `cards.json#R:蓝玫瑰的骄傲`
* **Quote:** 「（2）若严格少于，则传送至下一个未被购买的Livehouse格子」
* **Trigger:** the play, when `#(owned Livehouse) < #(owned non-Livehouse)`
  and some Livehouse is unowned.
* **Condition:** --.
* **Effect:** the user [传送]s to the next unowned Livehouse tile clockwise;
  the move is the turn's main move.
* **Frame:** the user's `pos`, `turn.main_moved`.
* **Rulings & OPEN:** none.
* **Executable:** `blue_rose_teleport`.

### C-R-蓝玫瑰的骄傲-03 -- (2) else RiNG 4, no settle

* **ID:** `C-R-蓝玫瑰的骄傲-03`
* **Source:** `cards.json#R:蓝玫瑰的骄傲`
* **Quote:** 「若此时已没有未被购买的Livehouse格子，则传送至“RiNG 4”（不触发结算），均视为你的主要移动。」
* **Trigger:** the play under (2) with every Livehouse owned.
* **Condition:** --.
* **Effect:** the user [传送]s to `RiNG 4`; no [触发结算]; main move.
* **Frame:** the user's `pos`, `turn.main_moved`.
* **Rulings & OPEN:** OPEN-触发结算 (what 「不触发结算」 suppresses).
* **Executable:** `blue_rose_ring4` (scenario).

---

## R:轨迹

**Source:** `cards.json#R:轨迹`

> [反击] [特]
> （1）在场上有玩家破产时，展示此卡，你获得那名玩家的任意一张地契（自动免费赎回），并拆除那个对应格子的所有房屋。

### C-R-轨迹-01 -- on bankruptcy, reveal

* **ID:** `C-R-轨迹-01`
* **Source:** `cards.json#R:轨迹`
* **Quote:** 「（1）在场上有玩家破产时，展示此卡」
* **Trigger:** some player goes [破产] while this card is held.
* **Condition:** --.
* **Effect:** the card is revealed (it is a [特]; the reveal is the trigger
  acknowledgement).
* **Frame:** --.
* **Rulings & OPEN:** 「展示」 has no separate zone; the effect is clauses
  02-03.
* **Executable:** `track_reveal` (scenario).

### C-R-轨迹-02 -- take one deed, free redeem

* **ID:** `C-R-轨迹-02`
* **Source:** `cards.json#R:轨迹`
* **Quote:** 「你获得那名玩家的任意一张地契（自动免费赎回）」
* **Trigger:** the same bankruptcy window.
* **Condition:** the bankrupt player owned at least one deed.
* **Effect:** the user chooses one of those deeds and owns it, unmortgaged,
  at no cost.
* **Frame:** `owner:T*`, `mortgaged:T*`, the bankrupt's / user's assets.
* **Rulings & OPEN:** 「任意」 is the user's free choice among the bankrupt's
  deeds.
* **Executable:** `track_take_deed` (scenario).

### C-R-轨迹-03 -- demolish houses

* **ID:** `C-R-轨迹-03`
* **Source:** `cards.json#R:轨迹`
* **Quote:** 「并拆除那个对应格子的所有房屋。」
* **Trigger:** the same.
* **Condition:** --.
* **Effect:** that tile's house count is 0.
* **Frame:** `houses:T*`.
* **Rulings & OPEN:** none.
* **Executable:** `track_demolish` (scenario).

---

## R:选择自己的舞台

**Source:** `cards.json#R:选择自己的舞台`

> [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算

### C-R-选择自己的舞台-01 -- [反击] vs an abnormal move

* **ID:** `C-R-选择自己的舞台-01`
* **Source:** `cards.json#R:选择自己的舞台`
* **Quote:** 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡」
* **Trigger:** the user is hit by an [异常移动效果] other than [除外].
* **Condition:** the card is in a [反击]-playable zone.
* **Effect:** a [反击] window offers this card (optional).
* **Frame:** --.
* **Rulings & OPEN:** the [异常移动效果] set is G-STATUS's business.
* **Executable:** `own_stage_window`.

### C-R-选择自己的舞台-02 -- choose whether to settle

* **ID:** `C-R-选择自己的舞台-02`
* **Source:** `cards.json#R:选择自己的舞台`
* **Quote:** 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the user chooses yes/no for whether this move (or the turn end
  when the move was prevented) [触发结算]s.
* **Frame:** the settle window of that landing / turn end.
* **Rulings & OPEN:** OPEN-触发结算 (the body of the settle).
* **Executable:** `own_stage_choose` (scenario).

---

## R:（ykn）louder

**Source:** `cards.json#R:（ykn）louder`

> 每次打出此卡时使ring的价格基础乘数+5

### C-R-louder-01 -- ring price base multiplier +5

* **ID:** `C-R-louder-01`
* **Source:** `cards.json#R:（ykn）louder`
* **Quote:** 「每次打出此卡时使ring的价格基础乘数+5」
* **Trigger:** every [手] play of this card.
* **Condition:** --.
* **Effect:** the ring price base multiplier rises by exactly 5.
* **Frame:** the ring-multiplier slot (and the card's own zones).
* **Rulings & OPEN:** OPEN-RING-MULT -- 「价格基础乘数」 is not defined in
  the book; `data/match_rules.json` names `ringMultiplier` (rent =
  `#RiNG × ringMultiplier × 1d20`) but the book never ties the two. Spec
  asserts the observable multiplier slot rose by 5 and does not invent which
  price it scales.
* **Executable:** `louder_mult`.

---

## R:（亚子）黑暗大魔姬亚子

**Source:** `cards.json#R:（亚子）黑暗大魔姬亚子`

> （1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。

### C-R-黑暗大魔姬亚子-01 -- [反击] before a pay to another player

* **ID:** `C-R-黑暗大魔姬亚子-01`
* **Source:** `cards.json#R:（亚子）黑暗大魔姬亚子`
* **Quote:** 「（1）[反击] 当你即将向其他玩家支付资金时可打出此卡」
* **Trigger:** the user is about to [支付] another player.
* **Condition:** the card is in a [反击]-playable zone.
* **Effect:** a [反击] window offers this card (optional).
* **Frame:** --.
* **Rulings & OPEN:** none.
* **Executable:** `ako_dark_window`.

### C-R-黑暗大魔姬亚子-02 -- gain 1 [眩晕]

* **ID:** `C-R-黑暗大魔姬亚子-02`
* **Source:** `cards.json#R:（亚子）黑暗大魔姬亚子`
* **Quote:** 「使自己获得一层[眩晕]」
* **Trigger:** the play of this card.
* **Condition:** --.
* **Effect:** the user gains exactly 1 layer of [眩晕].
* **Frame:** the user's status.
* **Rulings & OPEN:** whether the stunned user still pays is G-STATUS /
  G-MONEY's window (rulebook 49 「无法收付款」 vs the card body). This
  clause only pins the stun.
* **Executable:** `ako_dark_stun`.

### C-R-黑暗大魔姬亚子-03 -- off-hand play draws 1

* **ID:** `C-R-黑暗大魔姬亚子-03`
* **Source:** `cards.json#R:（亚子）黑暗大魔姬亚子`
* **Quote:** 「若此卡从手牌以外的地方打出，你抽一张卡。」
* **Trigger:** the same play, when the card did not come from hand.
* **Condition:** the card was in a non-hand playable zone (field, etc.).
* **Effect:** the user draws exactly 1 card.
* **Frame:** the user's hand / draw / discard.
* **Rulings & OPEN:** none.
* **Executable:** `ako_dark_offhand_draw` (scenario).

---

## R:（燐子）Ringing Bloom

**Source:** `cards.json#R:（燐子）Ringing Bloom`

> （1）将此卡放置于自身场上
> （2）你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半
> （3）你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。

### C-R-Ringing Bloom-01 -- place on own field

* **ID:** `C-R-Ringing Bloom-01`
* **Source:** `cards.json#R:（燐子）Ringing Bloom`
* **Quote:** 「（1）将此卡放置于自身场上」
* **Trigger:** the [手] play of this card.
* **Condition:** --.
* **Effect:** the card is on the user's field.
* **Frame:** the user's field / hand / discard.
* **Rulings & OPEN:** none.
* **Executable:** `ringing_bloom_place`.

### C-R-Ringing Bloom-02 -- houses "as if" = max, raised tiles rent halved

* **ID:** `C-R-Ringing Bloom-02`
* **Source:** `cards.json#R:（燐子）Ringing Bloom`
* **Quote:** 「（2）你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半」
* **Trigger:** while the card is on the field.
* **Condition:** the user owns tiles with differing house counts.
* **Effect:** for rent purposes every owned tile's house count reads as the
  user's maximum; tiles whose count this raised charge half rent.
* **Frame:** rent amounts (not the printed `houses` values).
* **Rulings & OPEN:** OPEN-RINGING-X (TEST-FINDINGS §6 #3) -- whether clause
  (3)'s X is the raised or the real count. Clause 02's 「视为」 is a read;
  the printed `houses` are not rewritten.
* **Executable:** `ringing_bloom_rent` (scenario).

### C-R-Ringing Bloom-03 -- after a non-RiNG rent: discard + 500·X

* **ID:** `C-R-Ringing Bloom-03`
* **Source:** `cards.json#R:（燐子）Ringing Bloom`
* **Quote:** 「（3）你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。」
* **Trigger:** the user's non-RiNG tile has just charged rent.
* **Condition:** the card is on the field.
* **Effect:** the card goes to the discard, then the user gains `500 * X`
  where X is the house count of the rent tile.
* **Frame:** the user's field / discard / money.
* **Rulings & OPEN:** OPEN-RINGING-X -- X after clause 02's 「视为」 or the
  real count. Spec asserts the discard move and that the gain is `500 * X`
  for the X the engine reports as the rent tile's house count (the counted
  value, per the green pin in TEST-FINDINGS #3), and records the open.
* **Executable:** `ringing_bloom_pay`.

---

## R:（纱夜）弹奏弹奏弹奏，继续弹奏

**Source:** `cards.json#R:（纱夜）弹奏弹奏弹奏，继续弹奏`

> [反击] 时机合适时打出，打出时视为使用一次此卡使用者的技能。

### C-R-弹奏-01 -- [反击] 「时机合适时」

* **ID:** `C-R-弹奏-01`
* **Source:** `cards.json#R:（纱夜）弹奏弹奏弹奏，继续弹奏`
* **Quote:** 「[反击] 时机合适时打出」
* **Trigger:** a [反击] window the engine considers 「合适」.
* **Condition:** the card is in a [反击]-playable zone.
* **Effect:** the card may be played.
* **Frame:** --.
* **Rulings & OPEN:** OPEN-SAYO-TIMING -- 「时机合适时」 is not defined; the
  window set is whatever the engine opens. Spec does not invent a timing.
* **Executable:** documented, not tested as a window shape.

### C-R-弹奏-02 -- counts as one use of the user's skill

* **ID:** `C-R-弹奏-02`
* **Source:** `cards.json#R:（纱夜）弹奏弹奏弹奏，继续弹奏`
* **Quote:** 「打出时视为使用一次此卡使用者的技能。」
* **Trigger:** the play of this card.
* **Condition:** the user has a bound character skill.
* **Effect:** that skill is treated as used once (its press / activation runs
  as if the user had used the skill).
* **Frame:** the skill's own contract.
* **Rulings & OPEN:** which skill entry fires when a character has several is
  the multi-activation question (TEST-FINDINGS §6 #8). Spec documents the
  clause; a scenario needs a skill whose use is directly observable and is
  optional (see Findings if the engine does not run it).
* **Executable:** `sayo_play_skill` (scenario).

---

## Character skills (`data/characters.json`, `band: "Roselia"`)

### 凑友希那 / 来练习吧

**Source:** `characters.json#凑友希那`

> （1）初始获得“RiNG 4”格子，从“RiNG 4”格子开始游戏，首次经过CiRCLE不获得经过奖励
> （2）如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动
> （3）移动终点为任意“RiNG”时，获得一个火罐（上限1）。任何时刻当你不位于RiNG时，失去所有的火罐。当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家强制停下并触发结算。

#### C-R-来练习吧-01 -- start: own RiNG 4, start there

* **ID:** `C-R-来练习吧-01`
* **Source:** `characters.json#凑友希那`
* **Quote:** 「（1）初始获得“RiNG 4”格子，从“RiNG 4”格子开始游戏」
* **Trigger:** game start.
* **Condition:** the seat is 凑友希那.
* **Effect:** the seat owns `RiNG 4` and its piece starts there.
* **Frame:** that tile's `owner`, that player's `pos`.
* **Rulings & OPEN:** none.
* **Executable:** `yukina_start`.

#### C-R-来练习吧-02 -- first CiRCLE pass: no reward

* **ID:** `C-R-来练习吧-02`
* **Source:** `characters.json#凑友希那`
* **Quote:** 「首次经过CiRCLE不获得经过奖励」
* **Trigger:** the first [经过] of CiRCLE.
* **Condition:** --.
* **Effect:** no CiRCLE pass reward is granted.
* **Frame:** the player's money / card zones on that pass.
* **Rulings & OPEN:** 「经过奖励」 body is G-MOVE's; this clause only
  suppresses it.
* **Executable:** `yukina_first_circle`.

#### C-R-来练习吧-03 -- may stop on the first RiNG passed

* **ID:** `C-R-来练习吧-03`
* **Source:** `characters.json#凑友希那`
* **Quote:** 「（2）如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动」
* **Trigger:** a move whose path [经过]s at least one RiNG.
* **Condition:** --.
* **Effect:** the user may stop on the first such RiNG (optional).
* **Frame:** the mover's `pos`.
* **Rulings & OPEN:** none.
* **Executable:** `yukina_stop_first_ring` (scenario).

#### C-R-来练习吧-04 -- +1 火罐 on a RiNG landing, cap 1

* **ID:** `C-R-来练习吧-04`
* **Source:** `characters.json#凑友希那`
* **Quote:** 「（3）移动终点为任意“RiNG”时，获得一个火罐（上限1）。」
* **Trigger:** a move whose end tile is a RiNG.
* **Condition:** --.
* **Effect:** the user gains 1 [火罐], never exceeding 1.
* **Frame:** the user's `fire` state.
* **Rulings & OPEN:** none.
* **Executable:** `yukina_fire_on_ring`.

#### C-R-来练习吧-05 -- drop all 火罐 off a RiNG

* **ID:** `C-R-来练习吧-05`
* **Source:** `characters.json#凑友希那`
* **Quote:** 「任何时刻当你不位于RiNG时，失去所有的火罐。」
* **Trigger:** any state change that leaves the user off every RiNG.
* **Condition:** the user held ≥1 [火罐].
* **Effect:** the user's [火罐] count is 0.
* **Frame:** the user's `fire` state.
* **Rulings & OPEN:** none.
* **Executable:** `yukina_fire_drop`.

#### C-R-来练习吧-06 -- spend 1 火罐 to force-stop a passer

* **ID:** `C-R-来练习吧-06`
* **Source:** `characters.json#凑友希那`
* **Quote:** 「当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家强制停下并触发结算。」
* **Trigger:** another player's path [经过]s the user's tile.
* **Condition:** the user holds ≥1 [火罐].
* **Effect:** the user may spend 1 to force-stop that player here and settle
  (optional).
* **Frame:** the passer's `pos`, the user's `fire`, the settle.
* **Rulings & OPEN:** OPEN-触发结算.
* **Executable:** `yukina_force_stop`.

### 冰川纱夜 / 踏上荆棘之路的觉悟

**Source:** `characters.json#冰川纱夜`

> （1）每次[经过]CiRCLE时获得2个[火罐]，每次被别的玩家[经过]时获得1个[火罐]（初始10，上限10）
> （2）你可以消耗6个[火罐]将非[传送]的主要移动添加1或2格
> （3）第一个玩家被淘汰，经过Circle时获得的火罐加1

#### C-R-踏上荆棘之路的觉悟-01 -- CiRCLE pass +2, being passed +1, cap 10

* **ID:** `C-R-踏上荆棘之路的觉悟-01`
* **Source:** `characters.json#冰川纱夜`
* **Quote:** 「（1）每次[经过]CiRCLE时获得2个[火罐]，每次被别的玩家[经过]时获得1个[火罐]（初始10，上限10）」
* **Trigger:** [经过] CiRCLE, or being [经过] by another player.
* **Condition:** --.
* **Effect:** +2 / +1 [火罐] respectively, never above 10; the start cap is
  10.
* **Frame:** the user's `fire` state.
* **Rulings & OPEN:** 「初始10」 is the cap (the opening stock is the cap per
  house style / the simple text); spec asserts the cap and the deltas.
* **Executable:** `sayo_fire_gain`.

#### C-R-踏上荆棘之路的觉悟-02 -- spend 6 to add 1 or 2 steps

* **ID:** `C-R-踏上荆棘之路的觉悟-02`
* **Source:** `characters.json#冰川纱夜`
* **Quote:** 「（2）你可以消耗6个[火罐]将非[传送]的主要移动添加1或2格」
* **Trigger:** before / during a non-[传送] main move.
* **Condition:** the user holds ≥6 [火罐].
* **Effect:** the user may spend exactly 6 to extend the move by exactly 1 or
  2 tiles (optional).
* **Frame:** the user's `fire`, the move length.
* **Rulings & OPEN:** none.
* **Executable:** `sayo_extend` (scenario).

#### C-R-踏上荆棘之路的觉悟-03 -- first elimination: CiRCLE +1 more

* **ID:** `C-R-踏上荆棘之路的觉悟-03`
* **Source:** `characters.json#冰川纱夜`
* **Quote:** 「（3）第一个玩家被淘汰，经过Circle时获得的火罐加1」
* **Trigger:** a CiRCLE [经过] after some player has been eliminated.
* **Condition:** at least one player is out.
* **Effect:** the CiRCLE-pass gain is 3 (2 + 1), not 2.
* **Frame:** the user's `fire`.
* **Rulings & OPEN:** 「被淘汰」 is [破产] / leaving (G-BANKRUPT).
* **Executable:** `sayo_fire_after_elim` (scenario).

### 今井莉莎 / 慈爱女神

**Source:** `characters.json#今井莉莎`

> （1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）
> （2）移动阶段前，你可以消耗一个火罐，将此次移动变为传送到距离你正向距离最近的角色所在的格子，触发结算。本次传送触发结算前，你可以额外消耗一个火罐或使与你在同一个格子上的另一个角色获得一个可超过上限的临时火罐，若你这样做，此次传送不触发任何结算。
> （3）你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐需要支付你600资金（被冲榜类效果被动消耗无需支付）。

#### C-R-慈爱女神-01 -- CiRCLE pass +1, cap 2

* **ID:** `C-R-慈爱女神-01`
* **Source:** `characters.json#今井莉莎`
* **Quote:** 「（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）」
* **Trigger:** [经过] CiRCLE.
* **Condition:** --.
* **Effect:** +1 [火罐], never above 2; cap 2.
* **Frame:** the user's `fire`.
* **Rulings & OPEN:** none.
* **Executable:** `lisa_fire_gain`.

#### C-R-慈爱女神-02 -- spend 1 to teleport to the nearest character ahead

* **ID:** `C-R-慈爱女神-02`
* **Source:** `characters.json#今井莉莎`
* **Quote:** 「（2）移动阶段前，你可以消耗一个火罐，将此次移动变为传送到距离你正向距离最近的角色所在的格子，触发结算。」
* **Trigger:** before the main move.
* **Condition:** the user holds ≥1 [火罐] and some character is ahead.
* **Effect:** the user may spend 1 to [传送] to the nearest character ahead
  (正向距离最近) and settle (optional).
* **Frame:** the user's `fire`, `pos`.
* **Rulings & OPEN:** 「正向距离最近」 is clockwise path distance.
* **Executable:** `lisa_teleport`.

#### C-R-慈爱女神-03 -- cancel settle by a second 火罐 or a temp 火罐

* **ID:** `C-R-慈爱女神-03`
* **Source:** `characters.json#今井莉莎`
* **Quote:** 「本次传送触发结算前，你可以额外消耗一个火罐或使与你在同一个格子上的另一个角色获得一个可超过上限的临时火罐，若你这样做，此次传送不触发任何结算。」
* **Trigger:** after the clause-02 teleport, before its settle.
* **Condition:** the user holds ≥1 extra [火罐], or another character shares
  the landing tile.
* **Effect:** the user may spend 1 more [火罐] **or** give a co-located
  character 1 temporary [火罐] (which may exceed the cap); then the teleport
  does not settle.
* **Frame:** `fire` on both players, the settle window.
* **Rulings & OPEN:** OPEN-触发结算.
* **Executable:** `lisa_cancel_settle` (scenario).

#### C-R-慈爱女神-04 -- temp 火罐 lifetime and 600 fee

* **ID:** `C-R-慈爱女神-04`
* **Source:** `characters.json#今井莉莎`
* **Quote:** 「（3）你提供的临时火罐将会在两回合后失去，使用你提供的临时火罐需要支付你600资金（被冲榜类效果被动消耗无需支付）。」
* **Trigger:** two turns after the grant; whenever the holder spends one.
* **Condition:** the [火罐] is a temp one this skill granted.
* **Effect:** it expires after two turns; actively spending it pays 600 to
  the granter; passive 冲榜-style spends do not pay.
* **Frame:** the holder's `fire`, both players' money.
* **Rulings & OPEN:** 「冲榜类效果」 is not enumerated; only the active-spend
  fee is asserted.
* **Executable:** `lisa_temp_fee` (scenario).

### 宇田川亚子 / 对帅气的憧憬

**Source:** `characters.json#宇田川亚子`

> （1）每次经过CiRCLE时，若选择抽卡，则可以看抽牌堆顶至多两张卡，选择其中1张加入手牌，剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己场上。
> （2）经过CiRCLE领取奖励时，若自己场上存在背面朝上的卡，则本次领取奖励被替换为将自己场上背面朝上的卡拿取至手中。
> （3）若自己场上背面朝上放置的卡为[反击]卡，可在符合条件时将其直接打出。

#### C-R-对帅气的憧憬-01 -- peek-2 draw replacement

* **ID:** `C-R-对帅气的憧憬-01`
* **Source:** `characters.json#宇田川亚子`
* **Quote:** 「（1）每次经过CiRCLE时，若选择抽卡，则可以看抽牌堆顶至多两张卡，选择其中1张加入手牌，剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己场上。」
* **Trigger:** a CiRCLE pass where the user chooses the draw reward.
* **Condition:** the draw pile is non-empty.
* **Effect:** the user looks at the top ≤2, takes 1 into hand; the remainder
  goes to the discard, or (paying 500) face-down onto the user's field.
* **Frame:** the user's hand / draw / discard / field / money.
* **Rulings & OPEN:** none for the determined part (take 1 of the peeked).
* **Executable:** `ako_peek_draw` (scenario).

#### C-R-对帅气的憧憬-02 -- face-down cards replace the reward

* **ID:** `C-R-对帅气的憧憬-02`
* **Source:** `characters.json#宇田川亚子`
* **Quote:** 「（2）经过CiRCLE领取奖励时，若自己场上存在背面朝上的卡，则本次领取奖励被替换为将自己场上背面朝上的卡拿取至手中。」
* **Trigger:** a CiRCLE pass reward while a face-down card sits on the user's
  field.
* **Condition:** --.
* **Effect:** the reward is replaced: the face-down cards go to hand.
* **Frame:** the user's field / hand.
* **Rulings & OPEN:** none.
* **Executable:** `ako_face_down_to_hand` (scenario).

#### C-R-对帅气的憧憬-03 -- face-down [反击] may be played directly

* **ID:** `C-R-对帅气的憧憬-03`
* **Source:** `characters.json#宇田川亚子`
* **Quote:** 「（3）若自己场上背面朝上放置的卡为[反击]卡，可在符合条件时将其直接打出。」
* **Trigger:** a [反击] window whose condition a face-down field [反击] card
  meets.
* **Condition:** that card is face-down on the user's field.
* **Effect:** it may be played from the field (optional).
* **Frame:** that card's own contract.
* **Rulings & OPEN:** none.
* **Executable:** `ako_face_down_counter` (scenario).

### 白金燐子 / 即使1cm也要前进

**Source:** `characters.json#白金燐子`

> （1）每次[经过]CiRCLE时获得三个[火罐]（初始3，上限3）
> （2）此技能可以正常使用或在[晕眩]状态下使用。你可以在移动掷骰前消耗X个火罐，使这回合移动掷骰的结果固定为X*6。本次移动不受异常移动效果影响。

#### C-R-即使1cm也要前进-01 -- CiRCLE pass +3, cap 3

* **ID:** `C-R-即使1cm也要前进-01`
* **Source:** `characters.json#白金燐子`
* **Quote:** 「（1）每次[经过]CiRCLE时获得三个[火罐]（初始3，上限3）」
* **Trigger:** [经过] CiRCLE.
* **Condition:** --.
* **Effect:** +3 [火罐], never above 3; cap 3.
* **Frame:** the user's `fire`.
* **Rulings & OPEN:** none.
* **Executable:** `rinko_fire_gain`.

#### C-R-即使1cm也要前进-02 -- pre-roll spend X → fixed X*6, immune

* **ID:** `C-R-即使1cm也要前进-02`
* **Source:** `characters.json#白金燐子`
* **Quote:** 「（2）此技能可以正常使用或在[晕眩]状态下使用。你可以在移动掷骰前消耗X个火罐，使这回合移动掷骰的结果固定为X*6。本次移动不受异常移动效果影响。」
* **Trigger:** before the move roll (including while [晕眩]).
* **Condition:** the user holds X ≥ 1 [火罐].
* **Effect:** the user may spend exactly X to fix the roll at `X*6`; the
  resulting move ignores [异常移动效果] (optional).
* **Frame:** the user's `fire`, the move length, the roll.
* **Rulings & OPEN:** which effects count as 异常 is G-STATUS's set.
* **Executable:** `rinko_fix_roll`.

---

## Band skill -- Roselia / 对音乐的纯粹

**Source:** `bands.json#Roselia`

> （1）本局游戏购买的第一个非Live House格子购买价格减半，在你拥有任意初始Live House格子前，那个格子对你视为Live House。
> （2）购买任何Live House格子的地契时购买价格减半

### C-R-对音乐的纯粹-01 -- first non-Livehouse is half price

* **ID:** `C-R-对音乐的纯粹-01`
* **Source:** `bands.json#Roselia`
* **Quote:** 「（1）本局游戏购买的第一个非Live House格子购买价格减半」
* **Trigger:** the game's first purchase of a non-Livehouse deed.
* **Condition:** the buyer has the Roselia band skill.
* **Effect:** the buyer pays half the tile's purchase price.
* **Frame:** the buyer's money, `owner:T*`.
* **Rulings & OPEN:** 「本局游戏」 is per-player (each Roselia seat's own
  first); rounding of 「减半」 is not stated (even prices only in practice).
* **Executable:** `band_first_non_livehouse`.

### C-R-对音乐的纯粹-02 -- the first non-Livehouse counts as Livehouse

* **ID:** `C-R-对音乐的纯粹-02`
* **Source:** `bands.json#Roselia`
* **Quote:** 「在你拥有任意初始Live House格子前，那个格子对你视为Live House。」
* **Trigger:** while the player owns no Livehouse tile yet.
* **Condition:** the clause-01 tile is owned.
* **Effect:** that tile counts as a Livehouse for that player (until a real
  Livehouse is owned). Once the player owns a real (own-group) Livehouse the
  tile stops counting; if they later own none again it counts again.
* **Implementation:** the `skill.roselia.first` slot stores the half-priced
  tile (`tile + 1`; 0 = not taken). The `colorFor:<p>` prop on that tile is set
  to Livehouse when it is bought, cleared (`-1`) when a real Livehouse is
  bought (Bought hook), and re-synced at each turn start (catches non-buy
  acquisitions and losses).
* **Frame:** the Livehouse count the skill's own clauses and other
  Livehouse-gated effects read.
* **Rulings & OPEN:** 「初始Live House格子」 is a real (RiNG) tile; the
  「视为」 is a read, not a board-kind rewrite. Spec documents the clause;
  a positive assertion needs an effect that keys off 「Livehouse」 counts and
  is in this unit (`R:蓝玫瑰的骄傲` (1)) -- see the scenario.
* **Executable:** not tested -- a positive assertion needs an effect that
  keys off 「Livehouse」 counts for that player; `R:蓝玫瑰的骄傲` (1) is the
  planned probe and is not written. OPEN-BAND-PRETEND.
  The end condition (real Livehouse owned) is implemented but equally untested.

### C-R-对音乐的纯粹-03 -- any Livehouse purchase is half price

* **ID:** `C-R-对音乐的纯粹-03`
* **Source:** `bands.json#Roselia`
* **Quote:** 「（2）购买任何Live House格子的地契时购买价格减半」
* **Trigger:** any purchase of a Livehouse deed.
* **Condition:** the buyer has the Roselia band skill.
* **Effect:** the buyer pays half the tile's purchase price.
* **Frame:** the buyer's money, `owner:T*`.
* **Rulings & OPEN:** none.
* **Executable:** `band_livehouse_half`.

---

## Open questions

| OPEN | text | notes |
|---|---|---|
| OPEN-分摊 | NFO face 2 「分摊支付你1000资金」 | how the 1000 splits among the tile's players is undefined. |
| OPEN-触发结算 | [触发结算] | undefined (README index); affects NFO-04, live前的准备-02, 学生会的检查-01/05, 蓝玫瑰-03, 来练习吧-06, 慈爱女神-03, 选择自己的舞台-02. |
| OPEN-场上 | NFO face 5 「为自己的角色卡添加6个奇迹水晶」 | character card as crystal host is outside the 「[场上]」 gloss. |
| OPEN-强制停下 | [强制停下] | definition truncated (`rulebook-doc.md:93`). |
| OPEN-RING-BUILD | 向着顶点-02 「为属于你的一个livehouse格子加盖一层房屋」 vs `rulebook-doc.md:157` 「所有RiNG不可升级」 | newer card text taken as live; conflict recorded. |
| OPEN-STUDENT-REFUND | 学生会的检查-07 「本卡原应拿到的资金」 | undefined amount. |
| OPEN-RING-MULT | louder 「ring的价格基础乘数」 | not defined in the book. |
| OPEN-RINGING-X | Ringing Bloom (3) X after (2)'s 「视为」 | TEST-FINDINGS §6 #3 still open. |
| OPEN-SAYO-TIMING | 弹奏 「时机合适时」 | undefined window. |
| OPEN-R1 | Fire bird 1.5× stacking | TEST-FINDINGS §6 R-1. |
| OPEN-ROUND10 | rounding of 1.5× and 20% | book does not say; integer cases pinned. |
| OPEN-FIRE-X | Fire bird 「X（自选）」 range | no range in the text. |
| OPEN-BAND-PRETEND | band (2) 「视为 Live House」 | no probe written. |

---

## Findings

### C-R-学生会的检查-01 -- play window (test bug, fixed)

An earlier draft gave the card **after** the roll and then forced a [手]
play from 结束, which the engine correctly refused (`err.play_phase`). The
book's window is 「移动结束后…[触发结算]前」 and is a settle-window offer:
the card must already be held when the move ends. The fixed test gives it
before the roll and takes the offer. Not an engine gap.

### C-R-学生会的检查-02 -- 觉悟 never enters the draw (genuine, stays failing)

* **Book says:** 「向抽牌堆中加入一张“觉悟”」 -- the play adds one
  `R:[衍生] 觉悟` to the user's draw pile.
* **Engine does:** accepts the [反击] activation in the right window
  (`log.play_counteract`, `cards:card-roselia.council_check_added`) but no
  觉悟 appears in any player's hand / draw / discard, the field, the board
  field, or the marks. The draw pile is empty afterwards.
* **Test:** `student_council_contract` / clause `student_council_add_resolve`.
* **Rulings:** the quote is unconditional on the optional placement branch;
  declining the placement must not drop the 觉悟.

### C-R-向着顶点-02 / OPEN-RING-BUILD -- noted, not red

The card's 「加盖一层房屋」 on a Livehouse conflicts with
`rulebook-doc.md:157` 「所有RiNG不可升级」. Newer sheet text is taken as live;
no red test (the branch needs every Livehouse owned and an optional build).

