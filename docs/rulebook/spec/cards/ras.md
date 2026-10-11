# C-RAS -- RAISE A SUILEN (15 cards + 5 character skills + band skill)

Unit C-RAS owns this file and `rules/tests/tests/spec_card_ras.rs`.

Sources (precedence): live sheet (`docs/rulebook/cards.json`,
`data/characters.json` `text` / `data/bands.json` `text`) >
`docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not authoritative.
TEST-FINDINGS §6 rulings apply unless newer sheet text supersedes them
(ignore anything marked 旧 / 原).

Scope: every `RAS:` entry in `cards.json` (15), every `band: "RAISE A SUILEN"`
character skill in `data/characters.json` (5), and the band skill
`RAISE A SUILEN` / `UNSTOPPABLE` in `data/bands.json`.

Notation: `[获得]` / `[消耗]` / `[支付]` / `[结算]` / `[经过]` / `[传送]` /
`[主要移动]` / `[反击]` / `[手]` / `[特]` / `[持续]` / `[指定]` / `[火罐]` /
`[奇迹水晶]` / `[除外]` per `rulebook-doc.md`. Clause IDs are
`C-RAS-<slug>-<n>`; the same strings appear in `card_ras.rs`.

Board tiles used below (`data/board.json`, engine index = 「#N格」 − 1):
CiRCLE 0, 购物中心 1, 江户川公园 7, RiNG 1 8, 羽丘女子学院 10, 白雪学园 18,
艺术学院高中 19, 瑟罗希亚国际学校 20, DUB MUSIC EXPERIMENT 21, 东京外 23,
Bandori车站 35, 武道馆 40, Space 42, 银河拉面馆 49, Live House Galaxy 50,
旭汤澡堂 52, CHUCHU的公寓 55, Live House 31.

「livehouse 格子」 / 「"Live House" 的颜色」: the book never defines the set.
The board's `group == 6` / colour `#CD1146` tiles are the natural reading
(RiNG 1/2/3/4, DUB MUSIC EXPERIMENT, Live House, 武道馆, Space,
Live House Galaxy). Recorded as **OPEN-livehouse**; every clause that names
them specifies only the printed behaviour over that set.

---

## Cards

### RAS:Change the world

* **Source:** `cards.json#RAS:Change the world`
* **Quote:** 「将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆」

#### C-RAS-change-the-world-01

* **Quote:** 「将此卡放置于你的一个有房屋的livehouse格子上」
* **Trigger:** play from hand.
* **Condition:** the user owns a livehouse tile with `houses ≥ 1`.
* **Effect:** the card is placed on that tile's board-field (`bfield`);
  the play is refused when the condition fails.
* **Frame:** only the user's hand / discard / field-of-board and the
  chosen tile's card slot move.
* **Executable:** `c_ras_change_the_world_place`.

#### C-RAS-change-the-world-02

* **Quote:** 「你的本次移动掷骰变为3d20」
* **Trigger:** the user's main-move dice after the placement.
* **Condition:** the card is on a tile.
* **Effect:** the roll consumes exactly three d20 faces (loaded dice of
  length 3); the walk is the sum of those faces.
* **Frame:** dice stream / `pos` / `roll` only.
* **Executable:** `c_ras_change_the_world_3d20`.

#### C-RAS-change-the-world-03

* **Quote:** 「期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶」
* **Trigger:** each `[经过]` of a livehouse the user does not own during
  that 3d20 walk.
* **Condition:** the passed tile is a livehouse and `owner ≠ user`.
* **Effect:** +1 [奇迹水晶] on this card (no printed cap).
* **Frame:** only this card's crystal count.
* **Executable:** asserted in `change_the_world_contract`.

#### C-RAS-change-the-world-04

* **Quote:** 「此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆」
* **Trigger:** the next `[收费]` on the card's tile.
* **Condition:** the card is still on the tile; `y` = crystals at trigger,
  `n` = house layers on the tile.
* **Effect:** the charge is raised by `50*(y+1)*n`; the tile's owner
  receives that raised amount (the quote's 「触发时获得等量资金」 is read
  as the owner collecting the raised charge -- the payer's payment rises
  by the same figure); the card then goes to its owner's discard.
* **Rulings & OPEN:** OPEN-livehouse. 「触发时获得等量资金」 -- OPEN-
  change-world-equal: 「等量」 may bind to the increment `50*(y+1)*n`
  (redundant with the raised charge) or to the whole raised amount; the
  payee is likewise unprinted. The executable pins the whole raised
  amount to the tile owner (the reading the charge ledger closes over).
* **Executable:** asserted in `change_the_world_contract`.

---

### RAS:EXIST

* **Source:** `cards.json#RAS:EXIST`
* **Quote:** 「将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡」

#### C-RAS-exist-01

* **Quote:** 「将此卡放置于自己场上」
* **Trigger:** play from hand.
* **Condition:** --.
* **Effect:** the card enters the user's field (`field:P*`).
* **Frame:** user's hand / discard / field.
* **Executable:** `c_ras_exist_place`.

#### C-RAS-exist-02

* **Quote:** 「直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你」
* **Trigger:** any single-target hand card (including one aimed at the
  EXIST holder) while EXIST is on the field.
* **Condition:** the card's effect is 「对单一玩家生效」 (one target).
* **Effect:** the target is rewritten to the EXIST holder. Multi-target
  cards are not redirected.
* **Rulings & OPEN:** 「对单一玩家生效」 is determined by the target
  count of the played card's [手] body. OPEN-exist-multi: the sheet does
  not say what happens to a card that *also* has a self branch.
* **Executable:** asserted in `exist_contract`.

#### C-RAS-exist-03

* **Quote:** 「你的下回合开始时将其翻入弃牌堆」
* **Trigger:** the holder's next turn start.
* **Condition:** EXIST is on the field.
* **Effect:** it leaves the field into the **owner's** discard
  (「翻入弃牌堆」 -- face-down is not observable; the zone is).
* **Frame:** owner's discard / holder's field.
* **Executable:** asserted in `exist_contract`.

#### C-RAS-exist-04

* **Quote:** 「若在此期间此卡没有造成影响，抽1张卡」
* **Trigger:** the same turn-start flip.
* **Condition:** no redirection (or other effect) fired while it sat.
* **Effect:** the owner draws exactly 1 card.
* **Rulings & OPEN:** OPEN-exist-influence: 「造成影响」 is undefined;
  the executable treats *any* redirect it performed as influence.
* **Executable:** asserted in `exist_contract`.

---

### RAS:PLEASE CHOOSE

* **Source:** `cards.json#RAS:PLEASE CHOOSE`
* **Quote:** 「[反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行：（1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一），（2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」

#### C-RAS-please-choose-01

* **Quote:** 「[反击] 当有玩家在livehouse格子上结算时」
* **Trigger:** a `[结算]` on a livehouse tile (the quote says 「有玩家」
  -- **any** player, including the holder; the executable only exercises
  another player's settle).
* **Condition:** the counteractor holds this [反击] card.
* **Effect:** the [反击] window opens (resolves before the settle, per
  `rulebook-doc.md` [反击]); the settler then picks (1) or (2).
* **Rulings & OPEN:** OPEN-livehouse.
* **Executable:** asserted in `please_choose_contract`.

#### C-RAS-please-choose-02

* **Quote:** 「（1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
* **Trigger:** the settler picks option (1).
* **Condition:** the settler holds a card whose [手] can `[指定]` the
  counteractor.
* **Effect:** that card is played immediately and the counteractor is
  among its targets.
* **Rulings & OPEN:** OPEN-please-choose-qual: which cards 「可将你指定
  为目标」 is not enumerated; the executable only covers the case where
  the settler holds `通用:登上武道馆` (targets every other player).
* **Executable:** documented; the (1) branch is exercised only through
  the (2) branch's counter-contrast in `card_ras.rs`.

#### C-RAS-please-choose-03

* **Quote:** 「（2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」
* **Trigger:** the settler picks option (2).
* **Condition:** --.
* **Effect:** the counteractor `[传送]`s to the settler's tile; the
  landing does **not** `[结算]`; band skills may still fire.
* **Frame:** counteractor's `pos` (and any band-skill follow-on).
* **Executable:** asserted in `please_choose_contract`.

---

### RAS:R. I. O. T.

* **Source:** `cards.json#RAS:R. I. O. T.`
* **Quote:** 「[反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。」

#### C-RAS-riot-01

* **Quote:** 「[反击] 当你被其他人的卡效果影响时」
* **Trigger:** another player's card effect would affect the holder.
* **Condition:** --.
* **Effect:** the [反击] window opens. Per `rulebook-doc.md`
  「[反击]X：Y … 结算优先于X」, Y resolves **before** X and X still
  happens (this card does not negate).
* **Executable:** `c_ras_riot_window`.

#### C-RAS-riot-02

* **Quote:** 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡」
* **Trigger:** the [反击] body.
* **Condition:** --.
* **Effect:** for every player: hand size `h` is discarded, then `h`
  cards are drawn (the played/countered card that already left the hand
  is not counted). The counter card itself is spent into the holder's
  discard.
* **Frame:** every player's hand / draw / discard; money unchanged by
  this clause (the countered effect may still move money).
* **Executable:** `c_ras_riot_cycle`.

#### C-RAS-riot-03

* **Quote:** 「你额外抽1张卡」
* **Trigger:** the same body.
* **Effect:** the holder draws one card on top of the cycle.
* **Executable:** `c_ras_riot_extra`.

---

### RAS:Repaint

* **Source:** `cards.json#RAS:Repaint`
* **Quote:** 「[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。」

#### C-RAS-repaint-01

* **Quote:** 「[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后」
* **Trigger:** after another player's main-move dice, while they are in
  the move phase.
* **Condition:** the holder holds this [反击].
* **Effect:** the window opens; the target is the mover.
* **Executable:** `c_ras_repaint_window`.

#### C-RAS-repaint-02

* **Quote:** 「使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数」
* **Trigger:** the [反击] body.
* **Condition:** the mover's original planned path (before the cut).
* **Effect:** the walk length is `roll - X`; `X` counts tiles on that
  original path owned by the Repaint holder (not the shortened path).
* **Executable:** asserted in `repaint_contract`.

#### C-RAS-repaint-03

* **Quote:** 「对方此次结算的支付减半」
* **Trigger:** the mover's settle after the cut walk.
* **Condition:** --.
* **Effect:** whatever the settle payment is (base rent, or a shaped
  payment -- see TEST-FINDINGS ruling 2026-10-06 / 祥，移动), it is
  halved. Rounding is not printed here; the standing 爱心义演 style
  (round the payment **up** to a multiple of 10, halved first) may or
  may not apply -- **OPEN-repaint-round**. The executable only covers
  an even rent (140 → 70).
* **Executable:** asserted in `repaint_halves_the_payment`.

---

### RAS:UNSTOPPABLE (card)

* **Source:** `cards.json#RAS:UNSTOPPABLE`
* **Quote:** 「投掷1d6mod6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。本次传送不触发结算，视为你的主要移动。且若骰点为1-3获得2000资金，若为4-6则获得1000资金。」

#### C-RAS-unstoppable-card-01

* **Quote:** 「投掷1d6mod6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓」
* **Trigger:** play from hand.
* **Condition:** --.
* **Effect:** one d6 is rolled; face `k ∈ 1..=6` teleports to the k-th
  named tile in the printed order (白雪学园 18, 艺术学院高中 19,
  瑟罗希亚国际学校 20, 银河拉面馆 49, 旭汤澡堂 52, CHUCHU的公寓 55).
* **Rulings & OPEN:** OPEN-unstoppable-mod: 「1d6mod6」 on face 6 is
  `6 mod 6 = 0`, but the body then says 「结果1-6」 -- the face 6 branch
  (CHUCHU的公寓) is what the text determines.
* **Executable:** `c_ras_unstoppable_card_teleport`.

#### C-RAS-unstoppable-card-02

* **Quote:** 「本次传送不触发结算，视为你的主要移动」
* **Trigger:** the landing.
* **Effect:** no `[结算]` on the destination; the move counts as the
  turn's `[主要移动]` (so no second main move).
* **Rulings & OPEN:** TEST-FINDINGS open ruling 1 (teleport-as-main-move
  settle shape) is about the *buy offer*; this clause only pins 「不触发
  结算」 + 「视为你的主要移动」.
* **Executable:** asserted in `unstoppable_card_contract`.

#### C-RAS-unstoppable-card-03

* **Quote:** 「若骰点为1-3获得2000资金，若为4-6则获得1000资金」
* **Trigger:** the same play.
* **Effect:** face 1-3 → `[获得] 2000`; face 4-6 → `[获得] 1000`.
* **Executable:** asserted in `unstoppable_card_contract`.

---

### RAS:成为最强

* **Source:** `cards.json#RAS:成为最强`
* **Quote:** 「roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到"Live House"），若你没有Livehouse格子，传送到"Live House"。视为你的主要移动。」

#### C-RAS-become-strongest-01

* **Quote:** 「roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到"Live House"）」
* **Trigger:** play from hand.
* **Condition:** the user owns at least one livehouse (else clause 02).
* **Effect:** one d10; face `k ∈ 1..=9` teleports to the k-th livehouse
  tile by board index; face `≥ 10` teleports to 「"Live House"」 (tile 31).
* **Rulings & OPEN:** OPEN-livehouse, OPEN-become-strongest-table: the
  text does not say whether the sorted table is every livehouse on the
  board or only those the user owns. These **are** distinguishable: with
  8 of 9 group-6 tiles owned (Live House is the agent tile), face 4 is
  Live House under the board-wide reading and RiNG 3 under owned-only.
  The executable pins the **board-wide** reading (「livehouse对应的格子
  （按格子编号排序）」 names the set, not 「你拥有的」).
* **Executable:** `c_ras_become_strongest_map`.

#### C-RAS-become-strongest-02

* **Quote:** 「若你没有Livehouse格子，传送到"Live House"」
* **Condition:** the user owns no livehouse.
* **Effect:** teleport to tile 31 regardless of the d10 face.
* **Executable:** asserted in `become_strongest_contract`.

#### C-RAS-become-strongest-03

* **Quote:** 「视为你的主要移动」
* **Effect:** the teleport is the turn's main move (no second one).
* **Executable:** documented (「视为你的主要移动」); the single-roll
  contract does not re-drive a second main move.

---

### RAS:游击演出

* **Source:** `cards.json#RAS:游击演出`
* **Quote:** 「\[特]：此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子，则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子。[手]：[反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后：传送至任意无主可购买的格子并[结算]，且必须购买。」

#### C-RAS-guerrilla-01 ([特] gate)

* **Quote:** 「此卡进入弃卡区的回合结束时，如果本回合的[结算]向其他玩家支付了至少1000资金且你不拥有任何可盖房的live house格子且场上已不存在可购买的此类格子」
* **Trigger:** the card enters the discard; the same turn is ending.
* **Condition:** that turn's `[结算]` paid another player `≥ 1000`; the
  user owns no buildable livehouse; no buyable livehouse remains unowned.
* **Effect:** an optional placement offer opens.
* **Rulings & OPEN:** OPEN-livehouse (「可盖房的」 / 「可购买的」);
  「本回合的[结算]向其他玩家支付」 is read as rent paid during this
  turn's settle windows.
* **Executable:** **untested** -- the [特] gate needs a full-turn setup
  (pay ≥1000 in settle, no buildable livehouse, none buyable). Documented
  only; see `guerrilla_counter_contract` for the [手][反击] half.

#### C-RAS-guerrilla-02 ([特] body)

* **Quote:** 「则可选择将此卡置于场上，指定你拥有的一个最贵的地契，使其对你视为live house格子」
* **Effect:** the card goes to the user's field; the user's most
  expensive deed is designated as a livehouse **for that user**.
* **Executable:** **untested** (same [特] gate setup).

#### C-RAS-guerrilla-03 ([手][反击] window)

* **Quote:** 「[反击]当你经过属于其他玩家的livehouse格子后，且当次[结算]时被其他玩家的格子收取资金后」
* **Trigger:** after `[经过]` an opponent's livehouse **and** the same
  move's settle charged the mover rent to another player.
* **Effect:** the [反击] window opens (both conditions required).
* **Executable:** `c_ras_guerrilla_counter_window`.

#### C-RAS-guerrilla-04 ([手][反击] body)

* **Quote:** 「传送至任意无主可购买的格子并[结算]，且必须购买」
* **Effect:** the holder `[传送]`s to any unowned buyable tile, runs
  `[结算]` there, and **must** buy it.
* **Frame:** holder's `pos` / `money` (price) / that tile's `owner`;
  the earlier rent is already settled.
* **Executable:** `c_ras_guerrilla_counter_body`.

---

### RAS:狂乱Hey Kids!!

* **Source:** `cards.json#RAS:狂乱Hey Kids!!`
* **Quote:** 「[反击] 在属于你的格子上结算时，打出此卡，将本次结算改为：将本格上的房屋转移到属于你的可建造格子上。转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋。转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；随后，你失去"转移后各格房屋造价总和－获得房屋数量×500"的资金。」

#### C-RAS-hey-kids-01 (window)

* **Quote:** 「[反击] 在属于你的格子上结算时」
* **Trigger:** a settle on a tile the holder owns.
* **Rulings (TEST-FINDINGS 2026-10-06):** opens when **another** player
  settles rent on the holder's tiles; the holder's own settle does not
  open it; a non-rent `[支付]` does not.
* **Executable:** `c_ras_hey_kids_window`.

#### C-RAS-hey-kids-02 (replace)

* **Quote:** 「将本次结算改为：将本格上的房屋转移到属于你的可建造格子上」
* **Effect:** the settle is replaced by a house transfer off this tile
  onto the holder's buildable tiles (no rent is charged; no buy offer).
* **Executable:** asserted in `hey_kids_replace_contract`.

#### C-RAS-hey-kids-03 (transfer shape)

* **Quote:** 「转移的房屋与目标格子中，至少一方数量为1，且每个目标格子至多获得1层房屋」
* **Effect:** each transfer leg moves houses so that source or target
  had exactly 1 (the printed 「至少一方数量为1」 is read as the per-leg
  constraint that the *smaller* side is 1, i.e. you cannot split a stack
  of 3 onto a stack of 3); every target gains at most 1 layer.
* **Rulings & OPEN:** OPEN-hey-kids-split: the exact split grammar is
  under-determined; the executable covers the 1-house → empty-target
  case (each target gains at most 1 layer). OPEN-hey-kids-sum: 「转移后
  各格房屋造价总和」 may mean every owned tile or only the tiles the
  transfer touched; the 1-house case cannot tell them apart.
* **Executable:** asserted in `hey_kids_replace_contract` (1-house case).

#### C-RAS-hey-kids-04 (money)

* **Quote:** 「转移时，消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金获得；随后，你失去"转移后各格房屋造价总和－获得房屋数量×500"的资金」
* **Effect:** let `C_src` = house cost consumed, `C_gain` = house cost
  gained. If `C_src > C_gain` the holder gains `C_src - C_gain` in cash.
  Then the holder loses `(sum of house costs on the tiles after the
  transfer) − (houses gained × 500)`.
* **Executable:** asserted in `hey_kids_replace_contract` (1-house case).

---

### RAS:练习室里的风暴

* **Source:** `cards.json#RAS:练习室里的风暴`
* **Quote:** 「（1）当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上，每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）（2）[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」

#### C-RAS-storm-01

* **Quote:** 「当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」
* **Trigger:** play from hand.
* **Condition:** the user is standing on a livehouse they own.
* **Effect:** the card is placed on that tile (board field), crystals 0.
* **Frame:** hand / discard / board field.
* **Executable:** `c_ras_storm_place`.

#### C-RAS-storm-02

* **Quote:** 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
* **Trigger:** the user `[经过]` CiRCLE.
* **Effect:** +1 crystal on the card, capped at 3.
* **Executable:** asserted in `storm_contract`.

#### C-RAS-storm-03

* **Quote:** 「[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」
* **Trigger:** a non-user `[结算]` at distance `X` from the card's tile.
* **Condition:** `1 ≤ X ≤ crystals`.
* **Effect:** the card goes to the user's discard; the settler is charged
  `(4-X)/4 × (the card tile's normal settle price)` and the user gains it.
* **Rulings & OPEN:** OPEN-storm-distance (circular vs absolute);
  OPEN-storm-price (「普通[结算]的价格」 = base rent, or rent at the
  tile's house level). The executable uses the house-level rent and the
  clockwise distance.
* **Executable:** asserted in `storm_contract` (X = 2 only).

---

### RAS:（LOCK）追逐梦想的步伐

* **Source:** `cards.json#RAS:（LOCK）追逐梦想的步伐`
* **Quote:** 「（1）抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上，游戏开始时[传送]到"东京外"获得3层[除外]（2）如果此卡拥有者的主要移动[经过]了"Bandori车站"则在触发结算前将行动终点改为"旭汤澡堂"，然后此卡[移除]」

#### C-RAS-lock-01

* **Quote:** 「抽取游戏开始的2手牌前将此卡从卡组展示给所有玩家并放置在自身场上」
* **Trigger:** opening deal (before the 2-card hand).
* **Condition:** the card is in the opening deck.
* **Effect:** it is revealed and placed on the owner's field instead of
  being dealt.
* **Executable:** **untested** as an opening-deal assertion (`place_raw`
  covers the on-field half); `lock_redirect_contract` covers clause 03.

#### C-RAS-lock-02

* **Quote:** 「游戏开始时[传送]到"东京外"获得3层[除外]」
* **Trigger:** game start.
* **Effect:** the owner `[传送]`s to 东京外 (tile 23) and gains 3 layers
  of `[除外]`.
* **Executable:** **untested** (game-start teleport + 3 [除外]); needs a
  `with_opening_draw` setup. `lock_redirect_contract` covers clause 03.

#### C-RAS-lock-03

* **Quote:** 「如果此卡拥有者的主要移动[经过]了"Bandori车站"则在触发结算前将行动终点改为"旭汤澡堂"，然后此卡[移除]」
* **Trigger:** the owner's main move `[经过]` Bandori车站 (tile 35).
* **Condition:** --.
* **Effect:** before `[结算]`, the destination becomes 旭汤澡堂 (tile 52);
  the card is then `[移除]`d (removed from the game).
* **Executable:** asserted in `lock_redirect_contract`.

---

### RAS:（MASKING）CRUSH ON THE DRUM!!!

* **Source:** `cards.json#RAS:（MASKING）CRUSH ON THE DRUM!!!`
* **Quote:** 「[特]：打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响。[手]：移动阶段前打出此卡，本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数」

#### C-RAS-crush-01 ([特])

* **Quote:** 「打出此卡的回合任何[使用者]使用的卡包含添加骰效果的卡不受任何其他效果影响」
* **Trigger:** the turn this card is played.
* **Effect:** the user's dice-adding cards are immune to other effects
  that turn.
* **Rulings & OPEN:** OPEN-crush-immune: 「不受任何其他效果影响」
  scope (counteractions? own follow-ons?) is not specified. The
  executable only covers the positive case: the +Xd20 of clause 02 is
  not stripped by a plain play.
* **Executable:** **untested** (「不受任何其他效果影响」 scope is
  OPEN-crush-immune); `crush_hand_contract` covers clause 02.

#### C-RAS-crush-02 ([手])

* **Quote:** 「移动阶段前打出此卡，本回合主要移动掷骰额外添加Xd20，X为你弃牌堆的卡数」
* **Trigger:** [手] play before the move phase.
* **Condition:** --.
* **Effect:** this turn's main-move roll is `1d20 + X·d20` where `X` is
  the discard-pile size at the moment of the play (the played card is
  spent after, so it is not in the pile when `X` is read -- see
  Findings if the engine counts it).
* **Executable:** asserted in `crush_hand_contract`.

---

### RAS:（PAREO）渐渐远去的你

* **Source:** `cards.json#RAS:（PAREO）渐渐远去的你`
* **Quote:** 「获得2个PAREO标记，然后视为你的房屋总数增加且可选择移除任意你拥有的格子上的一层房屋」

#### C-RAS-pareo-01

* **Quote:** 「获得2个PAREO标记」
* **Trigger:** play from hand.
* **Effect:** +2 PAREO marks on the user (token name `PAREO标记` /
  `PAREO`).
* **Executable:** asserted in `pareo_card_contract`.

#### C-RAS-pareo-02

* **Quote:** 「视为你的房屋总数增加」
* **Effect:** the play counts as a house-total increase for listeners
  (notably 鳰原令王那 skill （2）).
* **Executable:** **untested** as a listener check (「视为你的房屋总数
  增加」 for 鳰原令王那 (2)); `pareo_card_contract` covers clauses 01/03.

#### C-RAS-pareo-03

* **Quote:** 「可选择移除任意你拥有的格子上的一层房屋」
* **Effect:** optional: remove 1 house layer from any owned tile.
* **Executable:** asserted in `pareo_card_contract` (declined branch).

---

### RAS:（chuchu）演奏我的音乐吧

* **Source:** `cards.json#RAS:（chuchu）演奏我的音乐吧`
* **Quote:** 「将此卡放置于你以外的一名玩家场上并为其添加3个奇迹水晶，那名玩家的每个回合结束时失去一个；场上存在此卡的玩家下次购买地契时，[使用者]获得100资金，将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个；此卡进入弃牌堆前每触发一次该效果，此卡获得资金时额外获得100（上限500）。此卡奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡。」

#### C-RAS-chuchu-01

* **Quote:** 「将此卡放置于你以外的一名玩家场上并为其添加3个奇迹水晶」
* **Trigger:** play from hand.
* **Effect:** the card is placed on **another** player's field with 3
  crystals.
* **Executable:** asserted in `chuchu_card_contract`.

#### C-RAS-chuchu-02

* **Quote:** 「那名玩家的每个回合结束时失去一个」
* **Trigger:** the holder's turn end.
* **Effect:** −1 crystal on the card.
* **Executable:** asserted in `chuchu_card_contract`.

#### C-RAS-chuchu-03

* **Quote:** 「场上存在此卡的玩家下次购买地契时，[使用者]获得100资金，将此卡移至除[使用者]外行动序列下一名玩家的场上并将奇迹水晶补充至3个」
* **Trigger:** the holder buys a deed.
* **Effect:** the user gains 100; the card moves to the next player in
  seat order who is not the user; crystals refill to 3.
* **Executable:** **untested** (the buy-and-pass leg); `chuchu_card_contract`
  covers clauses 01/02.

#### C-RAS-chuchu-04

* **Quote:** 「此卡进入弃牌堆前每触发一次该效果，此卡获得资金时额外获得100（上限500）」
* **Trigger:** each clause-03 trigger, while the card is still in play.
* **Effect:** future 「此卡获得资金」 amounts gain +100 per trigger,
  capped at +500.
* **Rulings & OPEN:** OPEN-chuchu-which-gain: which 「获得资金」 the
  bonus rides is not named; the executable reads it as the clause-03
  100 itself growing by +100 per past trigger (100, 200, 300, ...).
* **Executable:** **untested** (OPEN-chuchu-which-gain).

#### C-RAS-chuchu-05

* **Quote:** 「此卡奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡」
* **Trigger:** the crystal count hits 0 (any cause).
* **Effect:** the card goes to the **user's** discard; the user draws 1.
* **Executable:** **untested** (crystal-hits-zero leg).

---

### RAS:（和奏瑞依）寄于指尖的执念

* **Source:** `cards.json#RAS:（和奏瑞依）寄于指尖的执念`
* **Quote:** 「[反击] 当你使用火罐进行掷骰时，可打出此卡并保留（写下）未被选择的另一个骰点，在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点。可保留多个骰点。」

#### C-RAS-layer-01

* **Quote:** 「[反击] 当你使用火罐进行掷骰时」
* **Trigger:** the holder uses a [火罐] to make a roll (e.g. 和奏瑞依
  skill （2）).
* **Effect:** the [反击] window opens; playing the card keeps a face.
* **Executable:** **untested** as a [反击] ring-up (`layer_press_contract`
  covers the press).

#### C-RAS-layer-02

* **Quote:** 「保留（写下）未被选择的另一个骰点」
* **Effect:** the face that was **not** chosen of the fire-pot roll is
  recorded.
* **Executable:** **untested** as a keep-the-other-face write.

#### C-RAS-layer-03

* **Quote:** 「在后续任意回合中消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点」
* **Trigger:** any later turn, via the placed card's activation.
* **Condition:** the holder has ≥1 kept die and ≥1 [火罐].
* **Effect:** spend 1 [火罐]; the turn's main-move roll is replaced by
  the kept face; that face is deleted.
* **Executable:** asserted in `layer_press_contract`.

#### C-RAS-layer-04

* **Quote:** 「可保留多个骰点」
* **Effect:** more than one face may be banked; the press picks one.
* **Executable:** the pick prompt in `layer_press_contract` (single face).

---

## Character skills (`data/characters.json`, band = RAISE A SUILEN)

### 和奏瑞依 -- 一次又一次竭尽全力

* **Source:** `characters.json#和奏瑞依`
* **Quote:** 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）（2）进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行」

#### C-RAS-rio-01

* **Trigger:** `[经过]` CiRCLE.
* **Effect:** +1 [火罐], initial 1, cap 1 (so a pass at the cap is a
  no-op).
* **Executable:** asserted in `rio_fire_contract`.

#### C-RAS-rio-02

* **Trigger:** after **any** roll.
* **Condition:** ≥1 [火罐].
* **Effect:** optional: spend 1 [火罐], roll again, execute one of the
  two faces.
* **Executable:** asserted in `rio_reroll_contract`.

### 朝日六花 -- 瞄准目标

* **Source:** `characters.json#朝日六花`
* **Quote:** 「（1）你购买的第一个非"旭汤澡堂"或任意"Live House"格子获得"Live House"的颜色（2）在你有初始"Live House"格子时你拥有的"旭汤澡堂"格子获得"Live House"的颜色」

#### C-RAS-rokka-01

* **Trigger:** the user's first deed purchase of a tile that is neither
  旭汤澡堂 nor already a Live House.
* **Effect:** that deed gains 「"Live House" 的颜色」 (counts as a
  livehouse for this user -- OPEN-livehouse).
* **Executable:** asserted in `rokka_first_deed_contract`.

#### C-RAS-rokka-02

* **Condition:** the user owns an initial Live House (tile 31, or a
  real group-6 deed -- OPEN).
* **Effect:** their 旭汤澡堂 also gains the Live House colour.
* **Executable:** **untested** (the 旭汤澡堂 colour leg).

### 佐藤益木 -- 与燃烧的红色一起驰骋

* **Source:** `characters.json#佐藤益木`
* **Quote:** 「（1）每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]（初始0，上限2）（2）移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外添加Xd10」

#### C-RAS-masking-01

* **Trigger:** `[经过]` 白雪学园 or 银河拉面馆.
* **Effect:** +1 [火罐], initial 0, cap 2.
* **Executable:** asserted in `masking_fire_contract`.

#### C-RAS-masking-02

* **Trigger:** before the move phase.
* **Effect:** spend `X` [火罐]; this turn's main-move roll gains `Xd10`.
* **Executable:** asserted in `masking_dice_contract`.

### 鳰原令王那 -- 梦幻可爱♪女仆

* **Source:** `characters.json#鳰原令王那`
* **Quote:** 「（1）游戏开始时如果场上有"Pastel✽Palettes"角色则额外获得1个正面[P✽P粉丝]（2）你拥有格子上的房屋总数增加时可选择失去1PAREO标记（初始1，上限3），所有非自己的玩家分摊支付你"格子的房屋造价×格子上的房屋数"四分之一的资金（所有你拥有的格子中取最高值）」

#### C-RAS-pareo-skill-01

* **Trigger:** game start.
* **Condition:** a Pastel✽Palettes character is in the match.
* **Effect:** +1 face-up [P✽P粉丝].
* **Executable:** **untested** (game-start [P✽P粉丝] leg).

#### C-RAS-pareo-skill-02

* **Trigger:** the user's house total increases.
* **Condition:** ≥1 PAREO mark (initial 1, cap 3).
* **Effect:** optional: lose 1 PAREO; every other player **分摊**-pays
  `¼ × max over owned tiles of (house cost × house count)`.
* **Rulings & OPEN:** OPEN-分摊 (how the ¼ is split among payers).
  The executable checks the total received equals `¼ × max` when the
  engine presents a single figure, and otherwise records the OPEN.
* **Executable:** **untested** (OPEN-分摊).

### 珠手知由 -- 天才制作人

* **Source:** `characters.json#珠手知由`
* **Quote:** 「加盖房屋时半价，如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」

#### C-RAS-chuchu-skill-01

* **Trigger:** any house build.
* **Effect:** the build cost is half the printed house cost.
* **Executable:** asserted in `chuchu_skill_half_contract` (and the turn-2
  control `chuchu_skill_half_on_a_later_turn`).

#### C-RAS-chuchu-skill-02

* **Condition:** the user built a house last turn.
* **Effect:** this turn's builds are free.
* **Executable:** `chuchu_skill_free_contract` (**stays failing** -- see
  Findings).

---

## Band skill (`data/bands.json#RAISE A SUILEN`)

### UNSTOPPABLE

* **Source:** `bands.json#RAISE A SUILEN`
* **Quote:** 「（1）每回合可选择只获得第一次受到的[异常移动效果]。（2）因该技能以外的效果在livehouse格子[触发结算]时，若你的下回合开始时仍在那个格子上，你的下一次主要移动可变为传送至你拥有的一个livehouse格子。」

#### C-RAS-band-01

* **Trigger:** each turn, when the player would take an
  `[异常移动效果]` (`rulebook-doc.md` 15: 传送 / 停留 / 晕眩 / 除外 /
  强制移动 / 强制停下 / 反方向移动).
* **Condition:** the player opts in for the turn.
* **Effect:** only the **first** such effect this turn applies; later
  ones are ignored.
* **Rulings & OPEN:** OPEN-band-opt-timing (when the opt-in is taken).
* **Executable:** **untested** (OPEN-band-opt-timing).

#### C-RAS-band-02

* **Trigger:** a `[触发结算]` on a livehouse caused by something other
  than this band skill; the player is still on that tile at their next
  turn start.
* **Effect:** the next `[主要移动]` **may** become a `[传送]` to a
  livehouse the player owns.
* **Rulings & OPEN:** OPEN-触发结算 (undefined), OPEN-livehouse.
* **Executable:** asserted in `band_teleport_offer_contract`.

---

## Open questions

| id | text | used by |
|---|---|---|
| OPEN-livehouse | 「livehouse 格子」 / 「"Live House" 的颜色」 undefined; read as board `group == 6` | almost every clause |
| OPEN-触发结算 | `[触发结算]` undefined (`rulebook-doc.md` 行动阶段 row 15) | C-RAS-band-02 |
| OPEN-分摊 | 分摊 undefined | C-RAS-pareo-skill-02 |
| OPEN-exist-influence | 「造成影响」 undefined | C-RAS-exist-04 |
| OPEN-exist-multi | multi-target / self-target cards under EXIST | C-RAS-exist-02 |
| OPEN-please-choose-qual | 「可将你指定为目标的牌」 not enumerated | C-RAS-please-choose-02 |
| OPEN-repaint-round | half of an odd payment | C-RAS-repaint-03 |
| OPEN-unstoppable-mod | 「1d6mod6」 vs 「结果1-6」 on face 6 | C-RAS-unstoppable-card-01 |
| OPEN-become-strongest-table | board-wide vs owned-only livehouse table | C-RAS-become-strongest-01 |
| OPEN-hey-kids-split | house-split grammar of 「至少一方数量为1」 | C-RAS-hey-kids-03 |
| OPEN-storm-distance | distance metric for 「距…X个格子」 | C-RAS-storm-03 |
| OPEN-storm-price | 「普通[结算]的价格」 base vs house-level | C-RAS-storm-03 |
| OPEN-crush-immune | 「不受任何其他效果影响」 scope | C-RAS-crush-01 |
| OPEN-chuchu-which-gain | which 「获得资金」 the +100 rides | C-RAS-chuchu-04 |
| OPEN-band-opt-timing | when 「每回合可选择」 is taken | C-RAS-band-01 |
| OPEN-change-world-equal | 「获得等量资金」 binds to the increment or the whole raised charge | C-RAS-change-the-world-04 |
| OPEN-hey-kids-sum | 「转移后各格房屋造价总和」 scope | C-RAS-hey-kids-04 |

## Findings

Genuine engine discrepancies stay as plain failing tests.

### C-RAS-chuchu-skill-02 -- free build after last turn's build

* **Book** (`characters.json#珠手知由`): 「如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」
* **Engine:** the second build is charged the **full** house cost (1500 on
  天文馆), not free (0) and not even the standing half (750).
* **Control:** `chuchu_skill_half_on_a_later_turn` -- without a prior-turn
  build the half (750) still applies on turn 2, so this is not a half-price
  regression; building last turn both fails to free the build **and** drops
  the half.
* **Executable:** `chuchu_skill_free_contract` (stays red).

