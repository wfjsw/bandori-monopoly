# C-AG -- Afterglow cards, character skills, band skill

Unit 12 owns this file and `rules/tests/tests/spec_card_ag.rs`. Sources, in
precedence order: live sheet (`docs/rulebook/cards.json`, `data/characters.json`
`text`, `data/bands.json` `text`) > `docs/rulebook/rulebook-doc.md`.
`rulebook.txt` is not authoritative. TEST-FINDINGS §6 rulings apply unless a
newer sheet text supersedes them (ignore anything marked 旧 / 原).

Scope: all 15 `AG:` cards in `cards.json`, the five Afterglow character skills
(`band == "Afterglow"` in `data/characters.json`), and the Afterglow band skill
in `data/bands.json`.

Clause IDs are `C-AG-<slug>-<n>`. Doc and `rules/tests/tests/spec_card_ag.rs` share
the same IDs. A discrepancy is a plain failing test; a clause that cannot be
tested says why. Ambiguous text gets an `OPEN-*` entry -- meaning is never
invented.

Board indices used below (`data/board.json`, 0-based engine tile index):
CiRCLE 0, 富士见坂 11, 星之鼓动山丘 44, 商店街 46, 羽泽咖啡厅 47, 山吹面包房 48,
银河拉面馆 49, 北泽精肉店 51, 旭汤澡堂 52. 「商店街」 colour / group 10 is tiles
46-49, 51, 52 (商店街 itself is the agent). 「商店街自己拥有的对应的格子（从商店街格子开始数）」
counts owned group-10 deeds walking forward from tile 46.

---

## AG:(兰) 像往常一样  (`as-usual`)

Exclusive to 美竹兰 (`characters.json#美竹兰.exclusiveCards`).

### C-AG-as-usual-01

* **ID:** `C-AG-as-usual-01`
* **Source:** `cards.json#AG:(兰) 像往常一样`
* **Quote:** 「（1）此卡可以当反击使用」
* **Trigger:** a [反击] window in which the card is held.
* **Condition:** the window's subject matches the card's (2) trigger, or the
  window is a generic counteract offer.
* **Effect:** the card may be declared as a counteraction (it is a valid
  `[反击]` responder).
* **Frame:** declaring it moves only the card's zones and whatever (2) does.
* **Rulings & OPEN:** none.
* **Executable:** `card_ag.rs` `as_usual_is_counteractable` (offer-when-held).

### C-AG-as-usual-02

* **ID:** `C-AG-as-usual-02`
* **Source:** `cards.json#AG:(兰) 像往常一样`
* **Quote:** 「（2）受到异常移动效果（包括你的技能）的回合结束前，回到起始地点并取消所有受到的效果（不进行任何结算）。」
* **Trigger:** the turn in which the owner received an [异常移动效果]
  (rulebook-doc.md:100: [传送] / [停留] / [晕眩] / [除外] / [强制移动] /
  [强制停下] / 反方向移动 -- including the owner's own skill).
* **Condition:** that turn has not yet ended.
* **Effect:** before turn end the owner returns to 起始地点 and every effect
  they received is cancelled; the return performs no [结算].
* **Frame:** see OPEN-起始地点; the cancelled effects' footprints are undone.
* **Rulings & OPEN:** **OPEN-起始地点** -- 「起始地点」 is not defined. Is it
  CiRCLE (the book's start tile), the seat's 开局 position, or the position
  before the abnormal move? **OPEN-像往常一样-undo** (TEST-FINDINGS §6) --
  does 「取消所有受到的效果」 undo money already paid? 「不进行任何结算」
  scopes only the return.
* **Executable:** documented, not contracted -- the return target is OPEN-起始地点.

---

## AG:ONE OF US  (`one-of-us`)

### C-AG-one-of-us-01

* **ID:** `C-AG-one-of-us-01`
* **Source:** `cards.json#AG:ONE OF US`
* **Quote:** 「将此卡放置在场上」
* **Trigger:** play from hand.
* **Condition:** --.
* **Effect:** the card enters the user's [场地] (field) and stays there.
* **Frame:** `field:P<user>` (and hand) only.
* **Rulings & OPEN:** none.
* **Executable:** `one_of_us_places_on_field`.

### C-AG-one-of-us-02

* **ID:** `C-AG-one-of-us-02`
* **Source:** `cards.json#AG:ONE OF US`
* **Quote:** 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」
* **Trigger:** the same play, after placement.
* **Condition:** another player on the board is an Afterglow character or owns
  a 商店街-colour deed.
* **Effect:** that player is chosen as the partner of the split.
* **Frame:** -- (a designation).
* **Rulings & OPEN:** **OPEN-场上** -- 「场上的」 is undefined (README index).
  「拥有商店街格子」 is read as owning a group-10 deed (商店街 colour).
* **Executable:** documented, not contracted -- 「场上」 is OPEN.

### C-AG-one-of-us-03

* **ID:** `C-AG-one-of-us-03`
* **Source:** `cards.json#AG:ONE OF US`
* **Quote:** 「指定双方各一块地契」
* **Trigger:** after the partner is chosen.
* **Condition:** each side owns at least one deed.
* **Effect:** one deed per side is designated for the split.
* **Frame:** -- (a designation).
* **Rulings & OPEN:** none beyond the above.
* **Executable:** documented, not contracted.

### C-AG-one-of-us-04

* **ID:** `C-AG-one-of-us-04`
* **Source:** `cards.json#AG:ONE OF US`
* **Quote:** 「触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）」
* **Trigger:** a [触发结算] of either designated deed.
* **Condition:** both deeds are still designated.
* **Effect:** the settle's income is split evenly between the two owners; no
  third party takes a share; 商店街-colour deeds must be designated first; the
  original owner's settle completes first, then the partner's money rises by
  their share **directly** (immune to every other effect).
* **Frame:** money of the two owners only on the share leg.
* **Rulings & OPEN:** **OPEN-触发结算** (undefined), **OPEN-分摊** (undefined).
  Only the "share goes to the partner as a direct money increase" half is
  determined; the split base is not.
* **Executable:** documented, not contracted -- depends on OPEN-触发结算.

### C-AG-one-of-us-05

* **ID:** `C-AG-one-of-us-05`
* **Source:** `cards.json#AG:ONE OF US`
* **Quote:** 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契」
* **Trigger:** bankruptcy of either designated owner.
* **Condition:** both designated deeds still exist.
* **Effect:** the two deeds are placed on this card and move to the surviving
  side's play area; that side is treated as owning them.
* **Frame:** `owner:*` of the two deeds, `bfield:*`, `field:P*`.
* **Rulings & OPEN:** depends on C-AG-one-of-us-03's designation surviving.
* **Executable:** documented, not contracted -- needs a full bankruptcy + designation setup.

---

## AG:Y.O.L.O  (`yolo`)

### C-AG-yolo-01

* **ID:** `C-AG-yolo-01`
* **Source:** `cards.json#AG:Y.O.L.O`
* **Quote:** 「你的任意掷骰结算前打出此卡，使结果增加1d4结果的数字」
* **Trigger:** before any of the user's dice rolls is finalised.
* **Condition:** the card is playable at that timing ([反击]-like window).
* **Effect:** the roll's result increases by the face of a 1d4.
* **Frame:** the roll (and whatever the roll then drives) only; the card leaves
  the hand to the discard (non-[持续]).
* **Rulings & OPEN:** sheet HINT (`新卡组卡` C3) is 「你的任意掷骰」 -- any of
  the user's rolls, not only the move roll. Order against other additive dice
  modifiers is OPEN (TEST-FINDINGS §6 R-4); this contract only pins the +1d4
  total.
* **Executable:** `yolo_adds_1d4`.

---

## AG:刻入天穹傲岸的烈光  (`glory`)

### C-AG-glory-01

* **ID:** `C-AG-glory-01`
* **Source:** `cards.json#AG:刻入天穹傲岸的烈光`
* **Quote:** 「[反击] 当你经过一名角色时，你可以打出此卡」
* **Trigger:** the user's move [经过]s a tile another character occupies.
* **Condition:** the card is in the user's hand (or offered in the counteract
  window).
* **Effect:** the card may be declared as a [反击] at that timing.
* **Frame:** declaration alone changes only the card's zones.
* **Rulings & OPEN:** 「经过」 is rulebook-doc.md:91 (X on the move [路径]).
  A teleport's path is only the destination (rulebook-doc.md:94), so a
  mid-route character is not 「经过」.
* **Executable:** `glory_swaps_half_of_dearest_deeds` (includes the offer gate).

### C-AG-glory-02

* **ID:** `C-AG-glory-02`
* **Source:** `cards.json#AG:刻入天穹傲岸的烈光`
* **Quote:** 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金」
* **Trigger:** the same [反击] resolving.
* **Condition:** the passed character owns at least one deed.
* **Effect:** the user gains floor/ceil? of `dearest_deed_base_price / 2` from
  the other player. The text says 「一半」 with no rounding rule; this contract
  uses integer division matching the printed half when the price is even, and
  records OPEN otherwise.
* **Frame:** `money:P<user>`, `money:P<other>`.
* **Rulings & OPEN:** **OPEN-半价取整** -- half of an odd 「基础购买价格」 is
  not specified. All printed prices are even today.
* **Executable:** `glory_swaps_half_of_dearest_deeds`.

### C-AG-glory-03

* **ID:** `C-AG-glory-03`
* **Source:** `cards.json#AG:刻入天穹傲岸的烈光`
* **Quote:** 「之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。」
* **Trigger:** immediately after C-AG-glory-02.
* **Condition:** -- (0 if the user owns nothing).
* **Effect:** the other player gains `user's dearest / 2` from the user.
* **Frame:** same two money slots.
* **Rulings & OPEN:** same rounding OPEN.
* **Executable:** `glory_swaps_half_of_dearest_deeds` (both legs).

---

## AG:即使夕阳落山  (`sunset`)

### C-AG-sunset-01

* **ID:** `C-AG-sunset-01`
* **Source:** `cards.json#AG:即使夕阳落山`
* **Quote:** 「[限]：第100轮次开始前此卡的X为原本X的一半。」
* **Trigger:** the [限] gate / the X assignment.
* **Condition:** the current round is before round 100.
* **Effect:** X is half of its base value (the halving of the computed X).
* **Frame:** X only.
* **Rulings & OPEN:** 「[限]」 (rulebook-doc.md:82) says a `[限]X` card may only
  be played from hand while X holds. The printed body makes the *X value* the
  limited quantity, not a play ban; `sunset_is_playable` pins that reading.
  「向上取整10」 is read as **round the payment up to a multiple of 10**
  (TEST-FINDINGS §6, 爱心义演 ruling 2026-10-06: 「rounds the *payment* up to
  a multiple of 10」; the same phrase appears in 基础[结算] 7).
* **Executable:** `sunset_demolishes_and_charges` (pins the printed X).

### C-AG-sunset-02

* **ID:** `C-AG-sunset-02`
* **Source:** `cards.json#AG:即使夕阳落山`
* **Quote:** 「[指定][使用者]拥有的四个有房子的格子和[使用者]以外的所有玩家。」
* **Trigger:** the [手] body.
* **Condition:** the user owns four housed deeds.
* **Effect:** those four tiles and every other [存活] player are designated.
* **Frame:** -- (designation).
* **Rulings & OPEN:** 「四个有房子的格子」 -- the text says four; if fewer than
  four are housed the card's designated set is undefined (OPEN-少于四格).
* **Executable:** covered by `sunset_demolishes_and_charges` (4 housed tiles).

### C-AG-sunset-03

* **ID:** `C-AG-sunset-03`
* **Source:** `cards.json#AG:即使夕阳落山`
* **Quote:** 「将X设为“被[指定]格子的地契价格和建造已有房屋的造价之和”÷“[使用者]以外的[存活]玩家数量”向上取整10」
* **Trigger:** the same [手] body, before the demolish / charge.
* **Condition:** designated tiles and the survivor count are known.
* **Effect:** `X = ceil_to_10( (Σ (price + houses×house_cost)) / n_survivors )`,
  then C-AG-sunset-01 may halve it. 「向上取整10」 = round up to a multiple of
  10 (the same reading as 基础[结算] 7's 「半价收费的[结算]（向上取整10）」).
* **Frame:** X only.
* **Rulings & OPEN:** **OPEN-取整顺序** -- is the divide then ceil-to-10, or
  ceil each term first? The clause implements divide then ceil-to-10, then
  the round-100 halving of C-AG-sunset-01 (so the *result* is halved).
* **Executable:** `sunset_demolishes_and_charges` (pins the printed number).

### C-AG-sunset-04

* **ID:** `C-AG-sunset-04`
* **Source:** `cards.json#AG:即使夕阳落山`
* **Quote:** 「1. 删除所有被[指定]格子上的1栋房；」
* **Trigger:** step 1 of the [手] body.
* **Condition:** each designated tile has ≥1 house.
* **Effect:** each designated tile loses exactly 1 house.
* **Frame:** `houses:T*` of the designated tiles.
* **Rulings & OPEN:** none.
* **Executable:** `sunset_demolishes_and_charges`.

### C-AG-sunset-05

* **ID:** `C-AG-sunset-05`
* **Source:** `cards.json#AG:即使夕阳落山`
* **Quote:** 「2. 所有被[指定]的玩家[支付][使用者]X资金。」
* **Trigger:** step 2, after the demolish.
* **Condition:** X is already computed.
* **Effect:** every designated player [支付]s the user X (so each of the other
  [存活] players pays X).
* **Frame:** `money:P*` of the payers and the user.
* **Rulings & OPEN:** [支付] is rulebook-doc.md:60 (A pays B).
* **Executable:** `sunset_demolishes_and_charges`.

---

## AG:商店街的青梅竹马  (`childhood`)

### C-AG-childhood-01

* **ID:** `C-AG-childhood-01`
* **Source:** `cards.json#AG:商店街的青梅竹马`
* **Quote:** 「投掷1d6并[传送]到商店街自己拥有的对应的格子（从商店街格子开始数）」
* **Trigger:** play from hand.
* **Condition:** the user owns ≥1 商店街-colour (group-10) deed.
* **Effect:** roll 1d6 = N; [传送] to the Nth owned group-10 deed counting
  forward from 商店街 (tile 46).
* **Frame:** `pos:P<user>` (and the roll bookkeeping).
* **Rulings & OPEN:** 「从商店街格子开始数」 -- the count includes deeds on and
  after tile 46 walking the board; a deed before 46 wraps. This contract
  arranges deeds after 46 so the count is literal.
* **Executable:** `childhood_teleports_to_nth_shop_tile`.

### C-AG-childhood-02

* **ID:** `C-AG-childhood-02`
* **Source:** `cards.json#AG:商店街的青梅竹马`
* **Quote:** 「如果投掷结果大于自己拥有的商店街格子数量则[传送]到商店街」
* **Trigger:** the same play, when the 1d6 exceeds the owned count.
* **Condition:** `roll > n_owned_group10`.
* **Effect:** [传送] to 商店街 (tile 46) itself.
* **Frame:** `pos:P<user>`.
* **Rulings & OPEN:** none.
* **Executable:** `childhood_over_roll_goes_to_shotengai`.

### C-AG-childhood-03

* **ID:** `C-AG-childhood-03`
* **Source:** `cards.json#AG:商店街的青梅竹马`
* **Quote:** 「视为你的主要移动」
* **Trigger:** the same play, as the move is declared.
* **Condition:** --.
* **Effect:** this [传送] counts as the turn's [主要移动] (rulebook-doc.md:86),
  so it consumes the once-per-turn main move and (being a card-driven
  [主要移动]) its settle is whatever the card says -- the card does not say
  「结算」, so it is a no-settle teleport that *is* the main move.
* **Frame:** `turn.main_moved` (bookkeeping) plus the teleport.
* **Rulings & OPEN:** **OPEN-传送主要移动结算** (TEST-FINDINGS §6 item 1) --
  should a teleport that *is* the main move settle like a main-move landing?
  The card text does not say 「结算」; this clause only records the
  「视为你的主要移动」 consumption.
* **Executable:** `childhood_is_main_move` (main_move flag / no second move).

---

## AG:回家的路上绕个道  (`detour`)

### C-AG-detour-01

* **ID:** `C-AG-detour-01`
* **Source:** `cards.json#AG:回家的路上绕个道`
* **Quote:** 「(1)此卡可作为反击使用」
* **Trigger:** a [反击] window.
* **Condition:** the card is held.
* **Effect:** the card may be declared as a counteraction.
* **Frame:** declaration only.
* **Rulings & OPEN:** none.
* **Executable:** documented, not contracted -- (1) prints no trigger X
  (rulebook-doc.md:79 wants 「[反击]X：Y」). `detour_can_be_a_counter` only
  records whether a given window offers the card; it does not assert.

### C-AG-detour-02

* **ID:** `C-AG-detour-02`
* **Source:** `cards.json#AG:回家的路上绕个道`
* **Quote:** 「(2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的（2）效果）」
* **Trigger:** the card's body.
* **Condition:** --.
* **Effect:** perform one "afterglow-style" move, treated as one use of the
  initial Afterglow character's (2) skill.
* **Frame:** that move's footprints.
* **Rulings & OPEN:** **OPEN-afterglow式移动** -- 「初始Afterglow角色」 is not
  defined. Is it 美竹兰 (the band's lead / first listed), or 「your」 character's
  (2), or a generic Afterglow (2)? The five (2) skills are different moves
  (backward, teleport-to-actor, parity, ramen-shift, reverse). Meaning is not
  invented; the clause is documented only.
* **Executable:** documented, not contracted (OPEN).

### C-AG-detour-03

* **ID:** `C-AG-detour-03`
* **Source:** `cards.json#AG:回家的路上绕个道`
* **Quote:** 「并将下一次的移动掷骰变更为1d6」
* **Trigger:** after the (2) move.
* **Condition:** the user's next move roll has not happened.
* **Effect:** that next move roll is 1d6 instead of 1d20.
* **Frame:** the next roll's die size / result.
* **Rulings & OPEN:** 「下一次的移动掷骰」 -- the user's next *move* roll (not
  any roll). Does it survive until the next turn? The text says 「下一次」 with
  no expiry; this contract checks the same-turn next move roll.
* **Executable:** documented, not contracted (depends on OPEN-afterglow式移动 for the (2) move that precedes it).

---

## AG:宣战布告  (`declaration`)

### C-AG-declaration-01

* **ID:** `C-AG-declaration-01`
* **Source:** `cards.json#AG:宣战布告`
* **Quote:** 「[手]：[反击]当你或你拥有的格子被其他玩家的卡效果影响时」
* **Trigger:** another player's card effect targets the user or a deed they own.
* **Condition:** the card is in the user's hand.
* **Effect:** a [反击] window offers the card at that timing. (The `[手]` tag
  means the card is a hand card; the body is `[反击]`-timed, so it may be
  played outside 运营.)
* **Frame:** offer only.
* **Rulings & OPEN:** 「被…卡效果影响」 is read as a targeting card effect
  (e.g. 通用:登上武道馆, PPP:抓到了), per the printed example class.
* **Executable:** `declaration_offered_against_targeting_card`.

### C-AG-declaration-02

* **ID:** `C-AG-declaration-02`
* **Source:** `cards.json#AG:宣战布告`
* **Quote:** 「[指定]那名玩家。」
* **Trigger:** the counter resolving.
* **Condition:** --.
* **Effect:** the player whose card caused the trigger is designated.
* **Frame:** designation.
* **Rulings & OPEN:** none.
* **Executable:** covered by `declaration_pays_500_and_draws` (the payer is that player).

### C-AG-declaration-03

* **ID:** `C-AG-declaration-03`
* **Source:** `cards.json#AG:宣战布告`
* **Quote:** 「被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。」
* **Trigger:** the same counter, after designation.
* **Condition:** --.
* **Effect:** the designated player [支付]s the user exactly 500, and the user
  draws exactly 1 card.
* **Frame:** `money:P<designated>`, `money:P<user>`, `hand/draw/discard:P<user>`.
* **Rulings & OPEN:** none.
* **Executable:** `declaration_pays_500_and_draws`.

---

## AG:无论是何种颜色的夕阳  (`colors`)

### C-AG-colors-01

* **ID:** `C-AG-colors-01`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「投掷1d6并根据结果获得对应效果: …（若结果严格大于6，则从1开始重新计数）」
* **Trigger:** play from hand.
* **Condition:** --.
* **Effect:** roll 1d6 (wrapping values >6 back through 1..6) and apply the
  matching numbered effect (C-AG-colors-02..07).
* **Frame:** the chosen effect's footprints only.
* **Rulings & OPEN:** the wrap rule only matters if the die can exceed 6; a
  plain 1d6 cannot. The clause still records it.
* **Executable:** documented, not contracted (the wrap only matters for dice > 6).

### C-AG-colors-02

* **ID:** `C-AG-colors-02`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「若为1则立刻获得1500资金」
* **Trigger:** roll = 1.
* **Condition:** --.
* **Effect:** the user [获得]s exactly 1500 (rulebook-doc.md:58, print money).
* **Frame:** `money:P<user>` only.
* **Rulings & OPEN:** none.
* **Executable:** `colors_1_gains_1500`.

### C-AG-colors-03

* **ID:** `C-AG-colors-03`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「若为2则从弃牌堆中选择一张牌放置到抽牌堆顶」
* **Trigger:** roll = 2.
* **Condition:** the user's discard is non-empty.
* **Effect:** one chosen discard card moves to the top of the draw pile.
* **Frame:** `discard:P<user>`, `draw:P<user>`.
* **Rulings & OPEN:** the choice target is player-chosen; the contract pins
  "one card left the discard and is now the draw top".
* **Executable:** documented, not contracted (choice target is player-picked; needs a discard pick prompt).

### C-AG-colors-04

* **ID:** `C-AG-colors-04`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「若为3则抽一张牌」
* **Trigger:** roll = 3.
* **Condition:** --.
* **Effect:** the user draws exactly 1 card.
* **Frame:** `hand/draw/discard:P<user>`.
* **Rulings & OPEN:** none.
* **Executable:** `colors_3_draws_one`.

### C-AG-colors-05

* **ID:** `C-AG-colors-05`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「若为4则可选择补满火罐或者获得2000资金」
* **Trigger:** roll = 4.
* **Condition:** --.
* **Effect:** the user chooses either 「补满火罐」 (fire pots to the cap) or
  [获得] 2000.
* **Frame:** `state:P<user>:fire` or `money:P<user>`.
* **Rulings & OPEN:** 「补满火罐」 = refills to the pot cap (rulebook-doc.md
  专有名词 2: 「持有的火罐数量不能超过持有上限」). Which pot (character skill
  vs other) is not spelled out; the contract takes the player's fire counter.
* **Executable:** `colors_4_takes_2000` (the 2000 branch).

### C-AG-colors-06

* **ID:** `C-AG-colors-06`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「若为5则从弃牌堆中选择一张牌加入手牌」
* **Trigger:** roll = 5.
* **Condition:** discard non-empty.
* **Effect:** one discard card moves to the hand.
* **Frame:** `hand/discard:P<user>`.
* **Rulings & OPEN:** none.
* **Executable:** documented, not contracted (choice target is player-picked).

### C-AG-colors-07

* **ID:** `C-AG-colors-07`
* **Source:** `cards.json#AG:无论是何种颜色的夕阳`
* **Quote:** 「若为6则选择1-5中的3个效果触发」
* **Trigger:** roll = 6.
* **Condition:** --.
* **Effect:** three of the five effects (results 1..5) are chosen and trigger.
* **Frame:** the union of the three effects' footprints.
* **Rulings & OPEN:** 「选择1-5中的3个效果」 is 3 **distinct** effects drawn
  from {1,2,3,4,5} -- 「3个效果」 names three effects, not one effect three
  times. The multiselect enforces this (one checkbox per effect). The three
  trigger in **ascending face order** (1 before 2 before …): the text
  specifies no order and a single multiselect has none, so ascending is the
  deterministic reading. Still not contracted (multi-effect sequencing).
* **Executable:** documented, not contracted.

---

## AG:朝同一片天空迈进  (`sky`)

### C-AG-sky-01

* **ID:** `C-AG-sky-01`
* **Source:** `cards.json#AG:朝同一片天空迈进`
* **Quote:** 「[反击]抽出此卡时立刻打出」
* **Trigger:** the card is drawn.
* **Condition:** --.
* **Effect:** the card is played immediately as a [反击] at that timing (it
  does not sit in the hand).
* **Frame:** the card's zones + C-AG-sky-02/03.
* **Rulings & OPEN:** 「抽出」 = the draw that put it into the hand (not a
  later optional play).
* **Executable:** `sky_pays_hand_times_600` / `sky_draws_when_hand_below_3`.

### C-AG-sky-02

* **ID:** `C-AG-sky-02`
* **Source:** `cards.json#AG:朝同一片天空迈进`
* **Quote:** 「如果你手牌数至少为3，获得手牌数*600的资金」
* **Trigger:** the auto-play resolving.
* **Condition:** hand size (counting the drawn card) ≥ 3.
* **Effect:** the user [获得]s `hand_len * 600`.
* **Frame:** `money:P<user>`.
* **Rulings & OPEN:** 「手牌数」 at the moment of the effect. The card is
  「立刻打出」 (C-AG-sky-01), so it is no longer in the hand when the body
  runs; 手牌数 is the remaining hand (rb setup: 3 other cards → 3×600).
* **Executable:** `sky_pays_hand_times_600`.

### C-AG-sky-03

* **ID:** `C-AG-sky-03`
* **Source:** `cards.json#AG:朝同一片天空迈进`
* **Quote:** 「如果你的手牌数小于3，抽一张卡」
* **Trigger:** the same auto-play.
* **Condition:** hand size < 3.
* **Effect:** draw exactly 1 card.
* **Frame:** `hand/draw/discard:P<user>`.
* **Rulings & OPEN:** none.
* **Executable:** `sky_draws_when_hand_below_3`.

### C-AG-sky-04

* **ID:** `C-AG-sky-04`
* **Source:** `cards.json#AG:朝同一片天空迈进`
* **Quote:** 「（开局时抽到此卡洗回）」
* **Trigger:** the opening deal.
* **Condition:** the card is among the opening hand.
* **Effect:** it is shuffled back into the deck instead of staying in the
  opening hand (and therefore does not auto-fire at 开局).
* **Frame:** the opening piles.
* **Rulings & OPEN:** 「洗回」 destination (draw pile vs whole deck) is not
  spelled out.
* **Executable:** `sky_opening_draw_is_shuffled_back` -- the harness's
  `with_opening_draw` runs `clean()` after the deal, which empties every
  hand and pile, so the test can only assert the card is not kept in an
  opening hand. A pre-clean zone check needs a Table seam the framework
  does not expose (`new_with_draw` is private).

---

## AG:绯红之魂  (`soul`)

### C-AG-soul-01

* **ID:** `C-AG-soul-01`
* **Source:** `cards.json#AG:绯红之魂`
* **Quote:** 「[手]：将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在这张卡上放置对应数量的[奇迹水晶]。」
* **Trigger:** the [手] play from 运营.
* **Condition:** the user can pay `k * 500` for some `k ∈ 1..=5`.
* **Effect:** the card is placed on the user's field; the user [消耗]s `k*500`
  and the card receives `k` [奇迹水晶].
* **Frame:** `field:P<user>`, `crystals:P<user>:…`, `money:P<user>`, card zones.
* **Rulings & OPEN:** 「选择[消耗]1到5次500资金」 -- k is chosen; the money is
  k separate 500 [消耗]s (delete money, rulebook-doc.md:59), summed k*500.
* **Executable:** `soul_places_and_loads_crystals`.

### C-AG-soul-02

* **ID:** `C-AG-soul-02`
* **Source:** `cards.json#AG:绯红之魂`
* **Quote:** 「（1）[反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）。」
* **Trigger:** the owner's [消耗] or [支付] (a counteract window).
* **Condition:** the card holds ≥1 crystal; the owner chooses to spend one.
* **Effect:** one crystal is removed; the amount falls by 1000 (floor 0); if
  it was a [支付], the payee [获得]s 500.
* **Frame:** `crystals`, the money legs, `money:P<payee>` if any.
* **Rulings & OPEN:** the quote in `cards.json` reads 「因导致的」 -- truncated
  (「因…导致的」). The clause covers the determined part: a [消耗] or [支付] of
  the owner. 「最少为0」 floors the *amount*, not the transfer.
* **Executable:** `soul_last_crystal_reduces_pay_and_leaves` (the last-crystal [支付] case).

### C-AG-soul-03

* **ID:** `C-AG-soul-03`
* **Source:** `cards.json#AG:绯红之魂`
* **Quote:** 「（2）[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]。」
* **Trigger:** the owner activates their character skill's (2).
* **Condition:** the card holds ≥1 crystal.
* **Effect:** exactly one crystal is removed.
* **Frame:** `crystals` on the card.
* **Rulings & OPEN:** 「自己原有的技能（2）」 = the owner's character skill
  clause (2), not a card. (Character (2) skills are the Afterglow pot skills.)
* **Executable:** documented, not contracted -- needs a character-skill activation on a soul
  owner; see `soul_skill2_spends_a_crystal`.

### C-AG-soul-04

* **ID:** `C-AG-soul-04`
* **Source:** `cards.json#AG:绯红之魂`
* **Quote:** 「（3）此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区。」
* **Trigger:** the crystal count reaches 0 (however).
* **Condition:** the card was on the field.
* **Effect:** the card moves to the user's discard (弃卡区).
* **Frame:** `field:P<user>`, `discard:P<user>`.
* **Rulings & OPEN:** 「[使用者]弃卡区」 is the user's discard pile
  (rulebook-doc.md:126). The clause fires as soon as the count hits 0 (the
  same reading as 该清CP了's on-card counter, TEST-FINDINGS §6).
* **Executable:** `soul_last_crystal_reduces_pay_and_leaves`.

---

## AG:（巴）商店街的救世主  (`savior`)

Exclusive to 宇田川巴.

### C-AG-savior-01

* **ID:** `C-AG-savior-01`
* **Source:** `cards.json#AG:（巴）商店街的救世主`
* **Quote:** 「【反击】当其他玩家抵押商店街地契时，你可以打出此卡，立刻支付常规收购价一半的价格从该玩家处收购该地契。」
* **Trigger:** another player mortgages a 商店街-colour deed.
* **Condition:** the card is held by 宇田川巴 (or offered as a counter).
* **Effect:** the card may be played; the user immediately pays half the
  「常规收购价」 and acquires that deed from the mortgager.
* **Frame:** `money:P*`, `owner:T<tile>`, `mortgaged:T<tile>`, card zones.
* **Rulings & OPEN:** 「常规收购价」 is the tile's purchase price (TEST-FINDINGS
  §6 item 13: 「the same number the rulebook calls 「地契购买价格」」).
  「商店街地契」 = a group-10 deed. Whether the acquired deed stays mortgaged is
  OPEN in the book; §6 item 13 records the engine un-mortgages (the book has
  no 「仍为抵押状态」, unlike 强制购买). The contract pins the money and the
  ownership transfer only.
* **Executable:** documented, not contracted (mortgage-counter + acquisition; §6 item 13 ruling).

### C-AG-savior-02

* **ID:** `C-AG-savior-02`
* **Source:** `cards.json#AG:（巴）商店街的救世主`
* **Quote:** 「当你抵押商店街地契时，你可以打出此卡，额外获得一份抵押收益并将地契翻回」
* **Trigger:** the user mortgages a 商店街-colour deed.
* **Condition:** the card is held.
* **Effect:** the user may play it; they receive one extra mortgage payout
  (the normal 50% of the purchase price, rulebook-doc.md:129) and the deed is
  turned face-up (un-mortgaged / 「翻回」).
* **Frame:** `money:P<user>`, `mortgaged:T<tile>`, card zones.
* **Rulings & OPEN:** 「额外获得一份抵押收益」 = +50% of the purchase price on
  top of the normal mortgage payout (so 100% of price total). 「翻回」 is the
  book's face-down-mortgage convention (rulebook-doc.md:131).
* **Executable:** documented, not contracted (「额外一份抵押收益」 + 「翻回」).

---

## AG:（摩卡）0.5倍速  (`half-speed`)

Exclusive to 青叶摩卡.

### C-AG-half-speed-01

* **ID:** `C-AG-half-speed-01`
* **Source:** `cards.json#AG:（摩卡）0.5倍速`
* **Quote:** 「(1) 将此卡放置在场上并获得3个奇迹水晶」
* **Trigger:** play from hand.
* **Condition:** --.
* **Effect:** the card is placed on the field with exactly 3 [奇迹水晶].
* **Frame:** `field:P<user>`, `crystals:P<user>:…`, card zones.
* **Rulings & OPEN:** none.
* **Executable:** `half_speed_places_with_three_crystals`.

### C-AG-half-speed-02

* **ID:** `C-AG-half-speed-02`
* **Source:** `cards.json#AG:（摩卡）0.5倍速`
* **Quote:** 「你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。」
* **Trigger:** the owner's turn end.
* **Condition:** the card is on the field.
* **Effect:** exactly one crystal is removed; at 0 the card enters the discard.
* **Frame:** `crystals`, `field:P<user>`, `discard:P<user>`.
* **Rulings & OPEN:** none.
* **Executable:** `half_speed_drains_one_crystal_per_turn_end`.

### C-AG-half-speed-03

* **ID:** `C-AG-half-speed-03`
* **Source:** `cards.json#AG:（摩卡）0.5倍速`
* **Quote:** 「(2) 此卡在场时，你的移动掷骰的最终结算/2（向上取整）」
* **Trigger:** the owner's move-roll settle, while the card is on the field.
* **Condition:** --.
* **Effect:** the final move-roll result is `ceil(roll / 2)`.
* **Frame:** the move distance / `pos`.
* **Rulings & OPEN:** 「最终结算」 is after additive modifiers; ceil is the
  standard 「向上取整」. Applies only to 移动掷骰, not to card dice.
* **Executable:** `half_speed_halves_the_move_roll`.

### C-AG-half-speed-04

* **ID:** `C-AG-half-speed-04`
* **Source:** `cards.json#AG:（摩卡）0.5倍速`
* **Quote:** 「且你的所有资金支付与消耗减半」
* **Trigger:** any [支付] / [消耗] of the owner while the card is on the field.
* **Condition:** --.
* **Effect:** the amount is halved (the text says 减半 with no rounding rule).
* **Frame:** the money leg.
* **Rulings & OPEN:** **OPEN-减半取整** -- odd amounts: round down, up, or
  nearest? Same OPEN as other halvings. Order against other multipliers is
  OPEN (TEST-FINDINGS §6 R-1).
* **Executable:** `half_speed_halves_a_spend` (even amounts).

---

## AG:（绯玛丽）如果并非没问题  (`not-ok`)

Exclusive to 上原绯玛丽.

### C-AG-not-ok-01

* **ID:** `C-AG-not-ok-01`
* **Source:** `cards.json#AG:（绯玛丽）如果并非没问题`
* **Quote:** 「【反击】当你的一次掷骰小于6时，你可以打出此卡使结果+1」
* **Trigger:** any of the user's die rolls lands on a face < 6.
* **Condition:** the card is held (counteract window).
* **Effect:** the result becomes `face + 1`.
* **Frame:** the roll result (and whatever it drives).
* **Rulings & OPEN:** 「一次掷骰」 is any single die of the user, not only the
  move roll (the text says 一次掷骰, not 移动掷骰).
* **Executable:** `not_ok_bumps_a_low_roll`.

---

## AG:（鸫）微小的『能做到』的事  (`tiny-can`)

Exclusive to 羽泽鸫.

### C-AG-tiny-can-01

* **ID:** `C-AG-tiny-can-01`
* **Source:** `cards.json#AG:（鸫）微小的『能做到』的事`
* **Quote:** 「打出此卡时，选择其中一个效果发动：」
* **Trigger:** play from hand.
* **Condition:** --.
* **Effect:** exactly one of (1) / (2) is chosen and activated.
* **Frame:** the chosen branch only.
* **Rulings & OPEN:** none on the choice itself.
* **Executable:** `tiny_can_offers_a_choice` (two options).

### C-AG-tiny-can-02

* **ID:** `C-AG-tiny-can-02`
* **Source:** `cards.json#AG:（鸫）微小的『能做到』的事`
* **Quote:** 「（1）【反击】时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳生效）」
* **Trigger:** as a [反击], when another effect would roll a numeric interval.
* **Condition:** the effect's result is a pure number-interval (e.g. Y.O.L.O's
  1d4), not a die-face table (e.g. 无论是何种颜色的夕阳's 1d6).
* **Effect:** that interval resolves at its theoretical max or min (the
  player's choice of which).
* **Frame:** the interval result only.
* **Rulings & OPEN:** **OPEN-@Tsugu** -- 「除@Tsugu ycm以外」 is a garbled
  sheet mention; the example (「可以对YOLO生效但无法对无论是何种颜色的夕阳生效」)
  is the determined part. "Directly changing a die face" is forbidden except
  for that (undefined) exception.
* **Executable:** documented, not contracted (needs a second card's interval in the same window).

### C-AG-tiny-can-03

* **ID:** `C-AG-tiny-can-03`
* **Source:** `cards.json#AG:（鸫）微小的『能做到』的事`
* **Quote:** 「（2）视为打出一张@Tsugu ycm」
* **Trigger:** the same play, branch (2).
* **Condition:** --.
* **Effect:** treated as playing one 「@Tsugu ycm」.
* **Frame:** --.
* **Rulings & OPEN:** **OPEN-@Tsugu-ycm** -- 「@Tsugu ycm」 is not a card name in
  `cards.json` (it looks like a Discord mention left in the sheet). Meaning is
  not invented; the clause is documented only.
* **Executable:** documented, not contracted (OPEN).

---

## Character skills (band = Afterglow)

Every Afterglow character shares clause (1). Per-character (2) (and (3) for
宇田川巴) follow.

### C-AG-firepot-01  (shared (1) of all five)

* **ID:** `C-AG-firepot-01`
* **Source:** `characters.json#美竹兰` (identical text on 青叶摩卡, 上原绯玛丽,
  宇田川巴, 羽泽鸫)
* **Quote:** 「（1）每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）」
* **Trigger:** turn boundaries of the skill owner.
* **Condition:** the owner has not used any Afterglow character's (2) skill
  for three of their turns.
* **Effect:** the owner gains one [火罐]; the cap is 1 and the start is 1
  (rulebook-doc.md 专有名词 2: the held count never exceeds the cap).
* **Frame:** `state:P<owner>:fire`.
* **Rulings & OPEN:** 「每三回合没有使用」 -- is the counter per the owner's
  turns (this contract) or any three game turns? 「Afterglow角色的（2）技能」
  includes *any* Afterglow character's (2), not just one's own.
* **Executable:** `firepot_starts_at_one_with_cap_one` + (documented) the
  three-turn refill.

### C-AG-ran-02  (美竹兰 叛逆的红挑染 (2))

* **ID:** `C-AG-ran-02`
* **Source:** `characters.json#美竹兰`
* **Quote:** 「（2）投掷移动步数前可使用一个[火罐]选择本次移动向后，并且向后移动[经过]CiRCLE时不获得CiRCLE奖励。」
* **Trigger:** before the owner's move roll.
* **Condition:** the owner holds 1 [火罐].
* **Effect:** spending one [火罐] makes the move go backward; while moving
  backward, [经过] CiRCLE grants no [CiRCLE奖励].
* **Frame:** `state:P<owner>:fire`, the move direction, `money`/hand of the
  CiRCLE reward.
* **Rulings & OPEN:** 「向后」 = counter-clockwise / opposite the normal
  clockwise main move (rulebook-doc.md:132).
* **Executable:** `ran_skill_moves_backward`.

### C-AG-moca-02  (青叶摩卡 我行我素 (2))

* **ID:** `C-AG-moca-02`
* **Source:** `characters.json#青叶摩卡`
* **Quote:** 「（2）当他人使用角色技能或打出手牌时，你可以消耗一个[火罐]传送到其所在格子（不触发结算且该传送本身不引发其他被动效果（如玩家重叠），该效果对每个玩家的被动技能和卡存在2次使用cd）」
* **Trigger:** another player uses a character skill or plays a hand card.
* **Condition:** the owner holds 1 [火罐].
* **Effect:** spending one [火罐] [传送]s the owner to that player's tile,
  with no [结算] and no further passive effects (e.g. no [重叠]); against each
  player's passives and cards there is a 2-use cooldown.
* **Frame:** `pos:P<owner>`, `state:P<owner>:fire`.
* **Rulings & OPEN:** **OPEN-2次cd** -- 「该效果对每个玩家的被动技能和卡存在
  2次使用cd」 is ambiguous. The sheet's `simple` column reads 「对同一个人用过
  一次后，Ta 接下来的 2 次不再问你」 (after ONE use, that player's next two
  triggers do not offer it). The contract pins only the teleport and the fire
  spend; the cooldown arithmetic is not invented.
* **Executable:** `moca_skill_teleports_to_the_actor`.

### C-AG-himari-02  (上原绯玛丽 大家一起迈出新的一步 (2))

* **ID:** `C-AG-himari-02`
* **Source:** `characters.json#上原绯玛丽`
* **Quote:** 「（2）移动阶段前，你可以消耗一个火罐指定你的此次移动仅在单/双数格上进行。」
* **Trigger:** before the owner's move phase.
* **Condition:** the owner holds 1 [火罐].
* **Effect:** spending one designates the move as only odd-numbered (or only
  even-numbered) tiles.
* **Frame:** the move path / landing.
* **Rulings & OPEN:** 「单/双数格」 -- tile *index* parity (data/board.json
  `index`, 1-based) is the natural reading. Only odd or only even landings.
* **Executable:** documented, not contracted (parity landings need a full path rewrite).

### C-AG-tomoe-02  (宇田川巴 豚骨酱油拉面大姐 (2))

* **ID:** `C-AG-tomoe-02`
* **Source:** `characters.json#宇田川巴`
* **Quote:** 「（2）移动阶段前，你可以使用一个[火罐]，使此次移动的起点向绝对距离“银河拉面馆”更近的方向移动10格。」
* **Trigger:** before the move phase.
* **Condition:** the owner holds 1 [火罐] and is **not** on 银河拉面馆
  (C-AG-tomoe-04).
* **Effect:** spending one moves the *start point* 10 tiles in the direction
  that is absolutely closer to 银河拉面馆 (tile 49).
* **Frame:** the move start (and then the path from there).
* **Rulings & OPEN:** 「向绝对距离…更近的方向」 -- the direction whose 10-tile
  step lands closer in absolute board distance to tile 49.
* **Executable:** documented, not contracted (start-shift needs the move-plan seam).

### C-AG-tomoe-03

* **ID:** `C-AG-tomoe-03`
* **Source:** `characters.json#宇田川巴`
* **Quote:** 「若你因此效果导致起点向前移动并超过了“银河拉面馆”，你补充一个火罐。」
* **Trigger:** after C-AG-tomoe-02's shift.
* **Condition:** the shift moved the start **forward** and past 银河拉面馆.
* **Effect:** the owner gains one [火罐] back (refund).
* **Frame:** `state:P<owner>:fire`.
* **Rulings & OPEN:** 「向前」 = the normal clockwise direction. 「超过了」 is
  strict overshoot, not landing on it.
* **Executable:** documented, not contracted (overshoot depends on the start-shift).

### C-AG-tomoe-04

* **ID:** `C-AG-tomoe-04`
* **Source:** `characters.json#宇田川巴`
* **Quote:** 「（3）若你位于银河拉面馆上，你无法使用（2）效果，你可以选择停留一回合，此效果每圈限一次」
* **Trigger:** the owner is on 银河拉面馆 at the move-phase gate.
* **Condition:** --.
* **Effect:** (2) is forbidden; the owner may instead [停留] for one turn;
  this stay option is limited to once per lap (每圈限一次).
* **Frame:** `state:P<owner>:stay` if chosen.
* **Rulings & OPEN:** 「每圈限一次」 -- one use per board lap (per time the
  owner passes CiRCLE / a lap counter). The harness has no lap counter; the
  contract pins the stay-once offer only.
* **Executable:** documented, not contracted (每圈限一次 needs a lap counter).

### C-AG-tsuzushi-02  (羽泽鸫 伟大的平凡 (2))

* **ID:** `C-AG-tsuzushi-02`
* **Source:** `characters.json#羽泽鸫`
* **Quote:** 「（2）当你进行主要移动时，你可以失去一个[火罐]使本次移动反向，如果你这样做了，你的下回合开始时，进行一次双倍掷骰的移动，向后移动经过CiRCLE时不获得CiRCLE奖励。」
* **Trigger:** the owner's [主要移动].
* **Condition:** the owner holds 1 [火罐].
* **Effect:** losing one [火罐] reverses the move; then at the owner's next
  turn start they perform a double-dice move (1d20, move 2× the face), and a
  backward pass of CiRCLE grants no [CiRCLE奖励].
* **Frame:** `state:P<owner>:fire`, the move direction, next-turn `pos`/money.
* **Rulings & OPEN:** 「双倍掷骰的移动」 = roll 1d20 and move twice the face
  (the simple column says 「掷 1d20，走 2 倍的格数并结算」). The extra move
  *does* settle per the simple column.
* **Executable:** documented, not contracted (next-turn double move spans a turn boundary).

---

## Band skill: Afterglow — 商店街的宠儿  (`band`)

### C-AG-band-01

* **ID:** `C-AG-band-01`
* **Source:** `data/bands.json#Afterglow`
* **Quote:** 「你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子」
* **Trigger:** the owner buys a deed (「购买」, rulebook-doc.md:155 / the buy
  action).
* **Condition:** the tile is a 商店街-colour (group-10) deed, **or** its
  purchase price ≤ 1200.
* **Effect:** one house is added on that tile, free (no house cost).
* **Frame:** `houses:T<tile>`, `owner:T<tile>`, `money:P<buyer>` (price only).
* **Rulings & OPEN:** 「商店街的」 is read as the 商店街 colour group (group 10
  in `data/board.json`: 羽泽咖啡厅, 山吹面包房, 银河拉面馆, 北泽精肉店,
  旭汤澡堂 -- not the 商店街 agent itself, which is unbuyable). 「价值小于等于
  1200」 is the tile's purchase `price`. 「自动免费」 means the house costs 0 and
  is not optional. Per TEST-FINDINGS HOST-06, a refused build must not pretend
  a house landed.
* **Executable:** `band_free_house_on_cheap_buy`, `band_free_house_on_shop_street_buy`.

### C-AG-band-02

* **ID:** `C-AG-band-02`
* **Source:** `data/bands.json#Afterglow`
* **Quote:** 「自动免费在上面加盖一栋房子」
* **Trigger:** the same buy, when the build is legal.
* **Condition:** the tile may carry another house (build caps).
* **Effect:** exactly one house appears, and the buyer's money falls by the
  purchase price only (not the house cost).
* **Frame:** `houses:T<tile>`, `money:P<buyer>`.
* **Rulings & OPEN:** build caps are the tile's printed level limit
  (rulebook-doc.md:157). A veto (e.g. 「不可盖房」) blocks the free house too
  (HOST-06); the buy itself still lands.
* **Executable:** `band_free_house_on_cheap_buy` (price 600 + free house).

---

## Open questions

| OPEN | text | notes |
|---|---|---|
| OPEN-起始地点 | 「起始地点」 of AG:(兰) 像往常一样 (2) | CiRCLE / 开局点 / pre-move pos? |
| OPEN-像往常一样-undo | 「取消所有受到的效果」 scope | does it undo money already paid? |
| OPEN-场上 | 「场上的」 (ONE OF US partner) | undefined (README index) |
| OPEN-触发结算 | 「触发结算」 | undefined (README index) |
| OPEN-分摊 | 「收益平分」 / 分摊 | undefined (README index) |
| OPEN-半价取整 | half of an odd 「基础购买价格」 | unspecified |
| OPEN-少于四格 | sunset with fewer than four housed deeds | designation set undefined |
| OPEN-取整顺序 | sunset X: divide then ceil-to-10? | vs ceil each term first |
| OPEN-传送主要移动结算 | teleport-as-main-move settle | TEST-FINDINGS §6 item 1 |
| OPEN-afterglow式移动 | 「初始Afterglow角色的（2）」 | which character? |
| OPEN-减半取整 | half-speed / other halvings, odd amounts | unspecified |
| OPEN-@Tsugu | 「除@Tsugu ycm以外」 (tiny-can (1)) | garbled sheet mention |
| OPEN-@Tsugu-ycm | 「视为打出一张@Tsugu ycm」 (tiny-can (2)) | not a card id |
| OPEN-每三回合 | fire-pot 「每三回合没有使用」 | whose turns? |
| OPEN-2次cd | moca skill 「2次使用cd」 | per player / per skill / per card? |
| OPEN-每圈 | tomoe (3) 「每圈限一次」 | lap counter undefined |

---

## Findings

### C-AG-yolo-01 (genuine, stays failing)

* **Book:** 「你的任意掷骰结算前打出此卡，使结果增加1d4结果的数字」 +
  rulebook-doc.md:126 「使用后的非[持续]手卡进入弃卡区」.
* **Engine:** the +1d4 **does** land (event `cards:card-ag.yolo_boost`,
  `pos = start + face + 1d4`), but the card is left in the hand
  (`hand = ["AG:Y.O.L.O"]`, `discard = []`). A used non-[持续] hand card must
  enter the 弃卡区.
* **Test:** `card_ag.rs::yolo_adds_1d4` (plain failing; no ignore).
* **Note:** the numeric half of the clause is honoured. Only the card's zone
  after use is wrong. Intermittent in the forall (other seeds take a
  [反击] path that does consume the card).
