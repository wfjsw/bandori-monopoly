# C-PPP -- Poppin' Party cards, character skills, band skill

Unit 10 owns this file and `rules/spec/tests/card_ppp.rs`. Every clause of
every PPP entry is listed; a clause that cannot be driven from a legal setup
says why under **Executable**.

Sources (precedence): live sheet (`docs/rulebook/cards.json`,
`data/characters.json`, `data/bands.json`) > `docs/rulebook/rulebook-doc.md`.
`rulebook.txt` is not authoritative. TEST-FINDINGS §6 rulings apply unless a
newer sheet quote supersedes them.

Scope: 18 `PPP:` cards, 5 Poppin' Party character skills
(`data/characters.json` `band == "Poppin' Party"`), and the band skill
`星之鼓动` (`data/bands.json`).

**Second pass (iff):** each clause must pass iff the rule does exactly what
the book says -- not less and not more. Contracts below are exact-effect +
exact-frame; the ones listed under **Iff-covered** additionally have pure
verifiers and LESS/MORE/WRONG mutation tests.

Token / vocabulary notes used by the clauses:

* **团卡** = the band-skill field instance (`skill:<band>:<skill>`, `band_skill`).
  Crystal reads/writes land on that instance.
* **星星贴纸** is a player token named `星星贴纸`.
* **多惠兔子** is a tile mark; **saaya标记** is a player token.
* **[奇迹水晶]** is `FieldCard::crystals` on the named field card.
* **[移除]** = leave every private / public zone of the game (not the discard).

---

## PPP:Bang Dream!

**Source:** `cards.json#PPP:Bang Dream!`

> Bang Dream!：
> [手]：
> 为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房。

### C-PPP-bang-dream-01

* **ID:** `C-PPP-bang-dream-01`
* **Source:** `cards.json#PPP:Bang Dream!`
* **Quote:** 「为[使用者]的团卡添加一个[奇迹水晶]」
* **Trigger:** the [手] body of this card, played from hand.
* **Condition:** the user has a band-skill field instance (团卡).
* **Effect:** that instance's crystal count rises by exactly 1.
* **Frame:** only the user's band-card crystal count (and the played-card
  zones) may move on this leg; see `bang_dream_frame`.
* **Rulings & OPEN:** OPEN-乐队卡 does not apply -- 团卡 is the bound band
  skill instance, not a separate 「乐队卡」 class.
* **Executable:** `c_ppp_bang_dream_01` / test `bang_dream_contract`.

### C-PPP-bang-dream-02

* **ID:** `C-PPP-bang-dream-02`
* **Source:** `cards.json#PPP:Bang Dream!`
* **Quote:** 「然后[传送]至任意[使用者]拥有的格子且可选择盖房」
* **Trigger:** same [手] body, after (and only after) the crystal add.
* **Condition:** the user owns at least one tile.
* **Effect:** the user is [传送] to a tile they own. 「可选择盖房」 is optional
  and is not a requirement.
* **Frame:** the user's `pos` (and optionally a house on the destination)
  plus the played-card zones.
* **Rulings & OPEN:** none.
* **Executable:** test `bang_dream_contract` (tile pick answered to an owned
  tile; `pos` must equal that tile).

---

## PPP:Popipa

**Source:** `cards.json#PPP:Popipa`

> Popipa：
> [手]：
> 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]

### C-PPP-popipa-01

* **ID:** `C-PPP-popipa-01`
* **Source:** `cards.json#PPP:Popipa`
* **Quote:** 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
* **Trigger:** [手] play of `PPP:Popipa`.
* **Condition:** user has room for the draw (or the engine's over-limit
  discard is a different clause, G-CARDS-01).
* **Effect:** (a) money +1000; (b) one `PPP:[衍生]Pipopa` enters the draw pile
  (multiset); (c) exactly one card is drawn; (d) `PPP:Popipa` is [移除] -- it is
  in neither hand, draw, nor discard.
* **Frame:** user money / hand / draw / discard only.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_popipa_01` / test `popipa_contract`.

---

## PPP:Returns

**Source:** `cards.json#PPP:Returns`

> Returns：
>  [特]：
>
> （1）如果此卡被加入初始卡组则卡组卡数添加8。
>
> （2）游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。
> [持续]：
>
> （1）无效[拥有者]Poppin' Party团卡的
> （4）效果。
>
> （2）[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。
>
> （3）使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。
>
> （4）[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。

### C-PPP-returns-01

* **ID:** `C-PPP-returns-01`
* **Source:** `cards.json#PPP:Returns` [特]（1）
* **Quote:** 「如果此卡被加入初始卡组则卡组卡数添加8。」
* **Trigger:** deck-construction / opening-deal window (before play).
* **Condition:** this card is in the starting deck.
* **Effect:** the deck size is 8 larger than the printed base.
* **Frame:** n/a (setup, not an observed transition).
* **Rulings & OPEN:** OPEN-结算定义 not involved.
* **Executable:** not driven as a Step -- deck construction is outside the
  observed-act window (`Table` cannot add to the starting-deck size). The
  opening deal in `Table::with_opening_draw` is the closest seam; left as a
  doc-only clause.

### C-PPP-returns-02

* **ID:** `C-PPP-returns-02`
* **Source:** `cards.json#PPP:Returns` [特]（2）
* **Quote:** 「游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。」
* **Trigger:** game start, after the opening deal.
* **Condition:** this card was in the owner's starting deck.
* **Effect:** the card sits on the owner's field (not in hand/draw/discard),
  and the owner holds a **borrowed** copy of another surviving player's band
  skill (an extra / copied 团卡 instance).
* **Frame:** the owner's field / private zones.
* **Rulings & OPEN:** 「团卡」 is the band-skill instance. OPEN-乐队卡 is about
  the undefined 「乐队卡」 class and does not cover this. Which band is borrowed
  is a player choice (prompt) and is not asserted.
* **Executable:** not in this suite (see Coverage): `returns_placed_and_borrows_band` (vanilla table with
  Returns forced into the opening draw via `with_opening_draw` is not yet a
  public Table API on `rules_spec`; the field placement is checked when the
  card is already on field and a second surviving seat's band is bound).

### C-PPP-returns-c1

* **ID:** `C-PPP-returns-c1`
* **Source:** `cards.json#PPP:Returns` [持续]（1）
* **Quote:** 「无效[拥有者]Poppin' Party团卡的（4）效果。」
* **Trigger:** continuous, while this card is on the owner's field.
* **Condition:** the owner has a Poppin' Party band skill with clause (4)
  「不能通过[主要移动]的[结算]盖房」.
* **Effect:** that (4) does **not** apply -- the owner may build through a
  main-move settle as if the band (4) were absent.
* **Frame:** the build decision itself is a different act; this clause only
  forbids (4) from blocking it.
* **Rulings & OPEN:** none.
* **Executable:** test `returns_neutralises_band_build_lock` (main-move settle
  offers a build while Returns is on field).

### C-PPP-returns-c2

* **ID:** `C-PPP-returns-c2`
* **Source:** `cards.json#PPP:Returns` [持续]（2）
* **Quote:** 「[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。」
* **Trigger:** the owner's turn start, while this card is on field.
* **Condition:** there is another surviving player.
* **Effect:** a prompt offers that player's 团卡; choosing a different one
  replaces the borrowed instance and clears **all** crystals on it.
* **Frame:** the borrowed band instance and its crystals.
* **Rulings & OPEN:** 「所有[奇迹水晶]」 is the crystals on the borrowed
  instance only.
* **Executable:** not in this suite (see Coverage): `returns_turn_start_borrow_prompt` (the turn start
  opens a prompt; answering a different band replaces the instance).

### C-PPP-returns-c3

* **ID:** `C-PPP-returns-c3`
* **Source:** `cards.json#PPP:Returns` [持续]（3）
* **Quote:** 「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。」
* **Trigger:** activating the **borrowed** band skill's active ([主]) effect.
* **Condition:** the borrowed instance is in use; the owner has ≥1 星星贴纸
  (or the activation cannot pay).
* **Effect:** exactly 1 星星贴纸 is spent as a cost of that activation.
* **Frame:** the owner's `星星贴纸` token count.
* **Rulings & OPEN:** none.
* **Executable:** doc-only for the tax in isolation -- the borrowed-skill
  activation path is the same `use_skill` as any band press and the tax is
  observable as a sticker drop when the press runs (covered indirectly by
  `returns_grants_sticker_after_general_card`'s sibling press tests). Not
  given its own forall: the press's body is another card's clause.

### C-PPP-returns-c4

* **ID:** `C-PPP-returns-c4`
* **Source:** `cards.json#PPP:Returns` [持续]（4）
* **Quote:** 「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。」
* **Trigger:** after every [手] effect of a 通用 card has fully resolved, for
  the owner of this card.
* **Condition:** this card is on the owner's field.
* **Effect:** the owner's 星星贴纸 rises by exactly 1.
* **Frame:** owner's 星星贴纸 (and the played 通用 card's own zones).
* **Rulings & OPEN:** 「全部生效后」 = after the whole [手] body, not between
  clauses of the body.
* **Executable:** `c_ppp_returns_c4` / test `returns_sticker_after_general`.

---

## PPP:STAR BEAT!

**Source:** `cards.json#PPP:STAR BEAT!`

> STAR BEAT!：
> [手]：[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”，然后进入移动阶段并选择以下操作之一：
> 1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算；
> 2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算。

### C-PPP-star-beat-01

* **ID:** `C-PPP-star-beat-01`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”」
* **Trigger:** the [手] body, before the move-stage choice.
* **Condition:** none beyond a legal play.
* **Effect:** the user gains 1 layer of that status (a build-after-settle
  option within absolute distance 5 of a self-owned tile, consumed on use).
* **Frame:** the user's status counters.
* **Rulings & OPEN:** 「绝对距离」 is the circular distance already used by
  「献给远方的你」's worked example (#11 vs #1 = 10). The engine's key for
  this status is `star_beat.layers` (observed as a `state:P*` delta when the
  body runs); the book does not name a key, so the clause only requires a
  +1 layer of a status that gates a distance-5 build option.
* **Executable:** checked as a `state:P0:star_beat` frame allowance on the
  STAR BEAT contracts (the layer is granted by the same [手] body as the
  option choice). Not a standalone predicate -- the book does not name the
  status key.

### C-PPP-star-beat-02

* **ID:** `C-PPP-star-beat-02`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「然后进入移动阶段并选择以下操作之一」
* **Trigger:** after the status grant.
* **Condition:** none.
* **Effect:** the user is in the move stage and a prompt offers exactly the
  two printed options (sticker+teleport vs Nd10 walk).
* **Frame:** stage / prompt bookkeeping.
* **Rulings & OPEN:** none.
* **Executable:** not in this suite (see Coverage): `star_beat_offers_two_options` (prompt after play).

### C-PPP-star-beat-03

* **ID:** `C-PPP-star-beat-03`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算」
* **Trigger:** choosing option 1.
* **Condition:** let `F` = number of players whose **money decimal
  representation contains the digit 5**.
* **Effect:** 星星贴纸 +2; the main move becomes a [传送] to tile index
  `(45 * (F + 1)) mod 60` (0-based board index; the book's 「#N格」 is
  index `N-1`, so the printed 「X格」 is read as board index X) **and that
  move settles**.
* **Frame:** user money is unchanged by the option itself (the settle may
  move money); user's sticker count +2; user's `pos` ends on the computed
  tile after the settle.
* **Rulings & OPEN:** 「资金数包含5」 is read as "the decimal representation
  of the money amount contains the digit 5" (e.g. 1500, 5000, 1050 all
  count). No OPEN -- the phrase is determinate enough to evaluate. Tile
  numbering: the board is 60 tiles 0..59 and the formula is `mod 60`, so
  the destination is the board index equal to the formula's value.
* **Executable:** `c_ppp_star_beat_03` / test `star_beat_option_1` (random
  money layouts via `forall`; F is computed in the test from the arranged
  money).

### C-PPP-star-beat-04

* **ID:** `C-PPP-star-beat-04`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算」
* **Trigger:** choosing option 2.
* **Condition:** same `F` as (03).
* **Effect:** the main move becomes a walk of `(2 * (F + 1))` d10 dice,
  summed, and that move settles. No stickers are granted by this option.
* **Frame:** user money unchanged by the option itself; no sticker change;
  `pos` advances by the rolled total (mod 60 path).
* **Rulings & OPEN:** same `F` reading as (03).
* **Executable:** test `star_beat_option_2` (loaded dice; sum of the loaded
  faces is the walk length).

---

## PPP:Tomorrow's Door

**Source:** `cards.json#PPP:Tomorrow's Door`

> Tomorrow's Door：
>
> （1）此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”
>
> （2）将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡
> （1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域
>
> （3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。

### C-PPP-door-01

* **ID:** `C-PPP-door-01`
* **Source:** `cards.json#PPP:Tomorrow's Door` （1）
* **Quote:** 「此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”」
* **Trigger:** n/a -- this is the sequence definition the other clauses read.
* **Condition:** n/a.
* **Effect:** the ordered sequence is exactly those 10 named tiles.
* **Frame:** n/a.
* **Rulings & OPEN:** none.
* **Executable:** constant checked by `door_sequence()` in the test file.

### C-PPP-door-02

* **ID:** `C-PPP-door-02`
* **Source:** `cards.json#PPP:Tomorrow's Door` （2）
* **Quote:** 「将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」
* **Trigger:** the [手] body places the card; afterwards, the **user's** pass
  over the card's current tile.
* **Condition:** the card is in the sequence (or already at the last entry).
* **Effect:** placement starts on 流星堂. Each user pass advances the card to
  the next sequence entry. When the card is already on 大阪中之岛公园, the
  pass moves it to the user's play area (field).
* **Frame:** the card's tile (or field placement).
* **Rulings & OPEN:** 「游玩区域」 is the player's field. Only the **user's**
  passes advance it -- 「此卡使用者每次[经过]」.
* **Executable:** test `door_walks_the_sequence` (place, then user passes
  each intermediate tile; assert the card's tile follows the sequence).

### C-PPP-door-03

* **ID:** `C-PPP-door-03`
* **Source:** `cards.json#PPP:Tomorrow's Door` （3）
* **Quote:** 「此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。」
* **Trigger:** another player (not the owner) settling on an owner-owned tile
  or on 梦开始的地方, while the card is in the owner's play area.
* **Condition:** the card is on the owner's field; the settler ≠ owner; the
  settle tile is owner-owned or 梦开始的地方.
* **Effect:** the settler pays an **extra** `houses(星之鼓动山丘) * 100` to
  the owner (on top of the base settle).
* **Frame:** settler and owner money.
* **Rulings & OPEN:** TEST-FINDINGS §6 #6 -- when 星之鼓动山丘 is co-owned by
  the PPP band's 「所有角色视为同时拥有」, does the hill count as
  「[拥有者]拥有的格子」? Record-only RULING, no decision in the book. The
  surcharge **amount** (`houses × 100`) is determinate and is what the
  clause pins; the co-owned-hill trigger is not asserted.
* **Executable:** doc-only for the co-owned-hill case. The amount formula is
  checked in not in this suite (see Coverage): `door_surcharge_amount` when the settle tile is an
  ordinary owner-owned tile (not the hill).

---

## PPP:[衍生]Pipopa

**Source:** `cards.json#PPP:[衍生]Pipopa`

> [衍生]Pipopa：
> [手]：
> 获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]

### C-PPP-pipopa-01

* **ID:** `C-PPP-pipopa-01`
* **Source:** `cards.json#PPP:[衍生]Pipopa`
* **Quote:** 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」
* **Trigger:** [手] play of `PPP:[衍生]Pipopa`.
* **Condition:** none beyond a legal play.
* **Effect:** (a) money +1000; (b) one `PPP:[衍生]Popipapapipopa` is placed on
  the user's field; (c) `PPP:[衍生]Pipopa` is [移除] (not in hand/draw/discard).
* **Frame:** user money / field / played-card zones.
* **Rulings & OPEN:** the printed name 「Popipapapipopa」 is the card
  `PPP:[衍生]Popipapapipopa`.
* **Executable:** `c_ppp_pipopa_01` / test `pipopa_contract`.

---

## PPP:[衍生]Popipapapipopa

**Source:** `cards.json#PPP:[衍生]Popipapapipopa`

> [衍生]Popipapapipopa：
> [持续]
>
> （1）此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。
>
> （2）消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。

### C-PPP-popipapapipopa-c1

* **ID:** `C-PPP-popipapapipopa-c1`
* **Source:** `cards.json#PPP:[衍生]Popipapapipopa` （1）
* **Quote:** 「此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。」
* **Trigger:** the card's owner [经过] one of the five named tiles.
* **Condition:** the card is on the owner's field; crystal count < 10.
* **Effect:** this card's crystals rise by exactly 1, never past 10.
* **Frame:** this card's crystals only (among card state).
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_popipapapipopa_c1` / test
  `popipapapipopa_crystal_on_pass`.

### C-PPP-popipapapipopa-c2

* **ID:** `C-PPP-popipapapipopa-c2`
* **Source:** `cards.json#PPP:[衍生]Popipapapipopa` （2）
* **Quote:** 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」
* **Trigger:** a [消耗] or [支付] the owner is making (the money-moving
  window of that spend).
* **Condition:** the card has ≥1 crystal; the spend is being reduced.
* **Effect:** spending `k` crystals reduces the money movement by `150 * k`,
  floored at 0; those `k` crystals are removed.
* **Frame:** the spend amount and this card's crystals.
* **Rulings & OPEN:** TEST-FINDINGS G26 -- does the reduction also apply to a
  rent the **owner receives**? The quote says 「消耗或支付」 (spend/pay), not
  收取; the clause covers the owner's own spend/pay legs only. The receive
  side is not asserted.
* **Executable:** `c_ppp_popipapapipopa_c2` / test
  `popipapapipopa_crystal_reduces_spend` (k crystals, spend ≥ 150k).

---

## PPP:[衍生]拍卖撤下来了

**Source:** `cards.json#PPP:[衍生]拍卖撤下来了`

> [衍生]拍卖撤下来了：
> [特]：此卡加入手牌时将此卡放置在自身场上，每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）
> [持续]：此卡拥有者火罐上限加1且不受任何其他效果影响。

### C-PPP-auction-off-01

* **ID:** `C-PPP-auction-off-01`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [特]
* **Quote:** 「此卡加入手牌时将此卡放置在自身场上」
* **Trigger:** this card enters a hand (draw / deal / any add-to-hand).
* **Condition:** none.
* **Effect:** it is immediately placed on that player's field (it does not
  stay in the hand).
* **Frame:** that player's field / hand.
* **Rulings & OPEN:** none.
* **Executable:** test `auction_off_places_itself_on_hand_add` (via
  `with_opening_draw` or `give`).

### C-PPP-auction-off-02

* **ID:** `C-PPP-auction-off-02`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [特]
* **Quote:** 「每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）」
* **Trigger:** the owner's turn start, while this card is on field.
* **Condition:** a `PPP:仓库里的Random Star` is on the owner's field.
* **Effect:** that Random Star is [移除], then the owner loses exactly 540
  money.
* **Frame:** owner's field / money.
* **Rulings & OPEN:** the 「（如果有，然后失去540资金）」 parenthetical is
  read as: the 540 loss is part of this same removal (it fires when the
  Random Star was present and is removed).
* **Executable:** not in this suite (see Coverage): `auction_off_removes_random_star_and_loses_540`.

### C-PPP-auction-off-c1

* **ID:** `C-PPP-auction-off-c1`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [持续]
* **Quote:** 「此卡拥有者火罐上限加1」
* **Trigger:** continuous, while on field.
* **Condition:** the owner's fire-pot max is readable (`state` bounds).
* **Effect:** `max(火罐) = baseline + 1` while the card is present.
* **Frame:** the fire-pot max bound.
* **Rulings & OPEN:** the baseline is the character's printed cap (or the
  engine default 0 when no skill sets one).
* **Executable:** test `auction_off_raises_fire_cap` (max delta +1 vs a
  stripped baseline).

### C-PPP-auction-off-c2

* **ID:** `C-PPP-auction-off-c2`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [持续]
* **Quote:** 「不受任何其他效果影响」
* **Trigger:** continuous.
* **Condition:** any other effect would touch this instance.
* **Effect:** that effect does not modify this instance (props, crystals,
  removal-by-other, etc.).
* **Frame:** this instance stays as placed.
* **Rulings & OPEN:** the engine's `FieldCard::immune` flag is the natural
  seat of this; the clause only requires the observable "unchanged by
  others" outcome.
* **Executable:** doc-only -- driving a specific "other effect" would require
  naming a second card's body, which is another unit's clause. The flag is
  checked indirectly (`immune` is a snapshot path) in
  `auction_off_raises_fire_cap`'s frame.

---

## PPP:仓库里的Random Star

**Source:** `cards.json#PPP:仓库里的Random Star`

> 仓库里的Random Star：
>
> （1）将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端
>
> （2）[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]

### C-PPP-random-star-01

* **ID:** `C-PPP-random-star-01`
* **Source:** `cards.json#PPP:仓库里的Random Star` （1）
* **Quote:** 「将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端」
* **Trigger:** the card's activation (play / body).
* **Condition:** none beyond a legal activation.
* **Effect:** (a) this card is on the owner's field; (b) the discard pile and
  hand are shuffled into the draw pile (multiset union, no other loss); (c)
  exactly one `PPP:[衍生]拍卖撤下来了` sits at the **bottom** of the draw
  pile.
* **Frame:** owner's field / hand / draw / discard.
* **Rulings & OPEN:** 「底端」 is the end opposite the draw face. The table
  exposes the pile top-first, so the auction card is the **last** element of
  `draw_pile()`.
* **Executable:** `c_ppp_random_star_01` / test `random_star_sweeps`.

### C-PPP-random-star-02

* **ID:** `C-PPP-random-star-02`
* **Source:** `cards.json#PPP:仓库里的Random Star` （2）
* **Quote:** 「[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]」
* **Trigger:** the owner [经过] 流星堂, while this card is on field.
* **Condition:** the owner holds ≥2 星星贴纸 (the cost is 「可使用」 --
  optional).
* **Effect:** spending 2 星星贴纸 force-stops the owner at 流星堂 and that
  tile settles.
* **Frame:** owner's sticker count and `pos`.
* **Rulings & OPEN:** OPEN-强制停下 is truncated in the global book; the
  clause only pins "stops on 流星堂 and settles", not the truncated
  remainder.
* **Executable:** not in this suite (see Coverage): `random_star_force_stop` (2 stickers, pass the tile,
  accept the force-stop).

---

## PPP:向着未来的路标

**Source:** `cards.json#PPP:向着未来的路标`

> 向着未来的路标：
> 本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金

### C-PPP-signpost-01

* **ID:** `C-PPP-signpost-01`
* **Source:** `cards.json#PPP:向着未来的路标`
* **Quote:** 「本回合的主要移动设为移动60格子并不触发结算」
* **Trigger:** this card's activation (its body).
* **Condition:** none beyond a legal activation.
* **Effect:** the turn's main move is a 60-tile walk that does **not**
  trigger 结算 along the way or at the end.
* **Frame:** the walk's `pos` change only (no settle money / no tile
  effects).
* **Rulings & OPEN:** OPEN-结算定义 -- 「不触发结算」 is read as "no
  基础[结算] and no tile settle", which is the only determinate reading; the
  double definition of [结算] is recorded as OPEN but does not change "no
  settle fires".
* **Executable:** test `signpost_sixty_no_settle`.

### C-PPP-signpost-02

* **ID:** `C-PPP-signpost-02`
* **Source:** `cards.json#PPP:向着未来的路标`
* **Quote:** 「回合结束时[失去]1000资金」
* **Trigger:** the owner's turn end, after the main move.
* **Condition:** none.
* **Effect:** the owner's money falls by exactly 1000 ([失去] = delete money).
* **Frame:** owner money only at the turn-end leg.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_signpost_02` / test `signpost_lose_1000_at_end`.

---

## PPP:抓到了

**Source:** `cards.json#PPP:抓到了`

> 抓到了：
> 移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。

### C-PPP-caught-01

* **ID:** `C-PPP-caught-01`
* **Source:** `cards.json#PPP:抓到了`
* **Quote:** 「移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。」
* **Trigger:** this card's activation.
* **Condition:** there is a player on a tile strictly ahead of the user
  (clockwise; 「前方」).
* **Effect:** the user moves to that player's tile; this counts as the turn's
  main move; a build is optional (not required).
* **Frame:** user `pos` (and optionally a house).
* **Rulings & OPEN:** 「你前方一名玩家」 -- when several players are ahead,
  which one is a player choice; the clause pins "the destination is some
  player's tile ahead". 「前方」 is the clockwise forward direction from the
  mover.
* **Executable:** test `caught_moves_to_ahead_player` (one player strictly
  ahead; assert `pos` equals that seat).

---

## PPP:献给远方的你

**Source:** `cards.json#PPP:献给远方的你`

> 献给远方的你：
> [手]：[传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]，然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖（例：[使用者]在#11号格子，一名玩家在#1号格子则绝对距离为10）。

### C-PPP-far-away-01

* **ID:** `C-PPP-far-away-01`
* **Source:** `cards.json#PPP:献给远方的你`
* **Quote:** 「[传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]」
* **Trigger:** the [手] body.
* **Condition:** at least one other player is standing somewhere.
* **Effect:** the user [传送] to a tile occupied by a player at maximum
  absolute distance from the user, and that landing settles.
* **Frame:** user `pos` and settle money / tile effects of the landing tile.
* **Rulings & OPEN:** 「绝对距离」 is circular: the example pins
  `min(|a-b|, 60-|a-b|)` -- #11 vs #1 = 10. Ties (several players equally
  far) make 「任意」 a free choice among them.
* **Executable:** test `far_away_teleports_to_farthest`.

### C-PPP-far-away-02

* **ID:** `C-PPP-far-away-02`
* **Source:** `cards.json#PPP:献给远方的你`
* **Quote:** 「然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖」
* **Trigger:** after the settle of (01).
* **Condition:** the user owns at least one buildable tile at maximum
  absolute distance.
* **Effect:** optionally, the user adds a house on one such tile. (Optional
  -- 「可以」.)
* **Frame:** that tile's house count.
* **Rulings & OPEN:** 「可盖房」 = the tile allows a further house under the
  global build caps; not re-specified here.
* **Executable:** not in this suite (see Coverage): `far_away_optional_house` (decline the build prompt;
  house counts unchanged -- the optional branch that *builds* is a choice,
  so the contract pins the offer, not a forced build).

---

## PPP:迷宫般的仓库

**Source:** `cards.json#PPP:迷宫般的仓库`

> 迷宫般的仓库：
> 位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋。如果[经过]“流星堂”则[强制停下]且[消耗]6000资金

### C-PPP-maze-01

* **ID:** `C-PPP-maze-01`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「位于“流星堂”前后5格内时，可打出此卡」
* **Trigger:** attempting to play this card.
* **Condition:** the user's position has absolute distance ≤ 5 from 流星堂
  (前后 = both directions).
* **Effect:** the play is legal (outside that range it is not).
* **Frame:** n/a (gate only).
* **Rulings & OPEN:** 「前后5格内」 is read as absolute distance ≤ 5 (the
  same circular distance as 「献给远方的你」's example). Ties at exactly 5
  are inside.
* **Executable:** test `maze_playable_near_ryuseido` (pos at distance ≤ 5
  accepts the play; distance ≥ 6 refuses).

### C-PPP-maze-02

* **ID:** `C-PPP-maze-02`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动」
* **Trigger:** the card's activation (legal under (01)).
* **Condition:** the walk can encounter the rolled number of unowned
  buyable tiles.
* **Effect:** one d10 is rolled; the mover starts at 流星堂 and walks until
  it has [经过] exactly N unowned buyable tiles (N = the d10 face); this
  walk is the turn's main move.
* **Frame:** user `pos`.
* **Rulings & OPEN:** 「从流星堂开始移动」 -- the walk's origin is 流星堂
  (the mover is teleported there first), not the pre-play position. The
  direction is the normal clockwise main-move direction (the only walk
  direction the book defines for 主要移动).
* **Executable:** test `maze_walks_from_ryuseido` (loaded d10; count unowned
  buyable tiles passed).

### C-PPP-maze-03

* **ID:** `C-PPP-maze-03`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
* **Trigger:** a buy of a tile this turn, after this card's move.
* **Condition:** the buyer is the card's user this turn.
* **Effect:** the buy consumes 0 money; if the buy goes through, every house
  on that tile is demolished (houses = 0 after).
* **Frame:** buyer money unchanged by the purchase price; the bought tile's
  houses / owner.
* **Rulings & OPEN:** 「不[消耗]资金」 = the purchase price is 0, not a
  rebate after paying.
* **Executable:** not in this suite (see Coverage): `maze_buy_is_free_and_strips_houses` (unowned
  buyable tile with houses arranged; buy; money unchanged, houses 0).

### C-PPP-maze-04

* **ID:** `C-PPP-maze-04`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「如果[经过]“流星堂”则[强制停下]且[消耗]6000资金」
* **Trigger:** the (02) walk [经过] 流星堂.
* **Condition:** the path crosses 流星堂.
* **Effect:** the walk [强制停下] on 流星堂 and the user [消耗] 6000 money.
* **Frame:** user `pos` / money.
* **Rulings & OPEN:** OPEN-强制停下 -- only "stops there and pays 6000" is
  pinned.
* **Executable:** not in this suite (see Coverage): `maze_pass_ryuseido_force_stops_and_pays` (loaded
  d10 that crosses 流星堂).

---

## PPP:（多惠）寻找更好的声音

**Source:** `cards.json#PPP:（多惠）寻找更好的声音` (exclusive: 花园多惠)

> （多惠）寻找更好的声音：
>  传送到“江户川乐器店”并获得一个[火罐]

### C-PPP-better-sound-01

* **ID:** `C-PPP-better-sound-01`
* **Source:** `cards.json#PPP:（多惠）寻找更好的声音`
* **Quote:** 「传送到“江户川乐器店”并获得一个[火罐]」
* **Trigger:** this card's activation (play from hand).
* **Condition:** the user is 花园多惠 (exclusive card).
* **Effect:** (a) the user is [传送] to 江户川乐器店; (b) the user gains
  exactly 1 [火罐] (respecting the cap).
* **Frame:** user `pos` / fire-pot counter / played-card zones.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_better_sound_01` / test `better_sound_contract`
  (character set raw to 花园多惠; fire cap raised so the pot can hold one).

---

## PPP:（有咲）等等等一下

**Source:** `cards.json#PPP:（有咲）等等等一下` (exclusive: 市谷有咲)

> （有咲）等等等一下：
> [手]：
> 将此卡放置在[使用者]的[场地]。
> [持续]：
>
> （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。
>
> （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。

### C-PPP-wait-aoki-01

* **ID:** `C-PPP-wait-aoki-01`
* **Source:** `cards.json#PPP:（有咲）等等等一下` [手]
* **Quote:** 「将此卡放置在[使用者]的[场地]。」
* **Trigger:** the [手] body.
* **Condition:** the user is 市谷有咲.
* **Effect:** the card is on the user's field (not in hand/draw/discard).
* **Frame:** user's field / played-card zones.
* **Rulings & OPEN:** OPEN-场上 -- 「[场地]」 is the player's field, the same
  「游玩区域」 other cards name.
* **Executable:** not in this suite (see Coverage): `wait_aoki_places_on_field`.

### C-PPP-wait-aoki-c1

* **ID:** `C-PPP-wait-aoki-c1`
* **Source:** `cards.json#PPP:（有咲）等等等一下` [持续]（1）
* **Quote:** 「任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。」
* **Trigger:** any event-settle that this card did not cause.
* **Condition:** the card is on the owner's field.
* **Effect:** this card's crystals rise by exactly 1 per such settle.
* **Frame:** this card's crystals.
* **Rulings & OPEN:** 「非因为此卡导致」 -- the book does not define
  causation. The clause specifies only what is determinate: a settle that
  is not this card's own activation. Causation edges among other cards are
  not inventoried here (OPEN-wait-causality).
* **Executable:** not in this suite (see Coverage): `wait_aoki_crystal_on_foreign_settle` (a plain
  money-moving act that is not this card).

### C-PPP-wait-aoki-c2

* **ID:** `C-PPP-wait-aoki-c2`
* **Source:** `cards.json#PPP:（有咲）等等等一下` [持续]（2）
* **Quote:** 「事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。」
* **Trigger:** an event-card trigger, while this card holds ≥3 crystals.
* **Condition:** crystals ≥ 3 at the trigger.
* **Effect:** (a) exactly 3 crystals leave this card; (b) the triggered event
  is deferred to the owner's next turn start; (c) afterwards an extra event
  is drawn as if another player drew it at 流星堂.
* **Frame:** this card's crystals; the event-deck / event-active bookkeeping;
  the extra draw is an event-deck move.
* **Rulings & OPEN:** OPEN-wait-causality (「事件触发」 vs 「事件结算」
  timing). The deferred-resolution timing (「在[拥有者]回合开始时结算」) is
  determinate and is what the clause pins. The extra draw's player is
  「其他」 -- not the owner -- and its tile is treated as 流星堂.
* **Executable:** doc-only for the deferral -- arranging a deferred event
  card plus a turn-start resolve requires the event-unit's fixtures and is
  out of this unit's card set. Crystal spend of exactly 3 is checked in
  not in this suite (see Coverage): `wait_aoki_c2_spend_three` when a trigger is driven.

---

## PPP:（沙绫）总有一天要给这片天空命名

**Source:** `cards.json#PPP:（沙绫）总有一天要给这片天空命名` (exclusive: 山吹沙绫)

> （沙绫）总有一天要给这片天空命名：
>
> （1）将此卡放置在山吹面包房
>
> （2）[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上

### C-PPP-name-sky-01

* **ID:** `C-PPP-name-sky-01`
* **Source:** `cards.json#PPP:（沙绫）总有一天要给这片天空命名` （1）
* **Quote:** 「将此卡放置在山吹面包房」
* **Trigger:** the card's activation.
* **Condition:** the user is 山吹沙绫.
* **Effect:** the card is placed on tile 山吹面包房 (not on the player's
  field).
* **Frame:** the card's tile placement.
* **Rulings & OPEN:** none.
* **Executable:** test `name_sky_lands_on_yamabuki`.

### C-PPP-name-sky-02

* **ID:** `C-PPP-name-sky-02`
* **Source:** `cards.json#PPP:（沙绫）总有一天要给这片天空命名` （2）
* **Quote:** 「[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上」
* **Trigger:** the user [经过] this card's current tile; the effect lands at
  the following turn end.
* **Condition:** the card is still on that tile at the turn end.
* **Effect:** (a) the user gains exactly 1 [火罐] (respecting the cap); (b)
  3d20 is rolled and the card is placed on the tile whose **board index**
  equals the rolled sum (the book's 「投掷结果的格子」 -- a 3d20 sum is
  3..60, and the board is 60 tiles).
* **Frame:** the user's fire pot; the card's tile.
* **Rulings & OPEN:** 「投掷结果的格子」 is read as 「#S格」 where `S` is
  the 3d20 sum, i.e. board index `S-1` (the same 1-based reading as
  「#1，#16，#31，#46」). A sum of 60 is `#60格` = index 59.
* **Executable:** not in this suite (see Coverage): `name_sky_pass_grants_fire_and_relocates` (loaded
  3d20; assert the card's tile after the turn end).

---

## PPP:（里美）我的心就像巧克力螺

**Source:** `cards.json#PPP:（里美）我的心就像巧克力螺` (exclusive: 牛込里美)

> （里美）我的心就像巧克力螺：
> [手]：
> 立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。

### C-PPP-chocolate-01

* **ID:** `C-PPP-chocolate-01`
* **Source:** `cards.json#PPP:（里美）我的心就像巧克力螺`
* **Quote:** 「立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。」
* **Trigger:** the [手] body.
* **Condition:** the user is 牛込里美.
* **Effect:** (a) the user is in the move stage immediately; (b) the main
  move is a choice of any tile with absolute distance 1..=4 from the
  current tile, and that landing settles; (c) during this move the user is
  [不可阻挡].
* **Frame:** user `pos` / unstoppable status / landing settle.
* **Rulings & OPEN:** 「1到4格或以内」 is read as absolute distance in
  `1..=4` (the same circular distance as the far-away example). 「期间」
  scopes [不可阻挡] to this move only.
* **Executable:** test `chocolate_move_1_to_4` (answer the tile pick at
  distance 2; assert `pos`; unstoppable is a status key and is checked as
  the move not being redirected -- doc-level for the status key name).

---

## PPP:（香澄）大家我都喜欢哦

**Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` (exclusive: 户山香澄)

> （香澄）大家我都喜欢哦：
> [手]：
> 将此卡放置在“星之鼓动山丘”上。
> [持续]：
> 其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半。

### C-PPP-everyone-01

* **ID:** `C-PPP-everyone-01`
* **Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` [手]
* **Quote:** 「将此卡放置在“星之鼓动山丘”上。」
* **Trigger:** the [手] body.
* **Condition:** the user is 户山香澄.
* **Effect:** the card is placed on tile 星之鼓动山丘.
* **Frame:** the card's tile placement.
* **Rulings & OPEN:** none.
* **Executable:** test `everyone_lands_on_hill`.

### C-PPP-everyone-c1

* **ID:** `C-PPP-everyone-c1`
* **Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` [持续]
* **Quote:** 「其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]」
* **Trigger:** another player [经过] the card's tile, and that player's
  move end tile ≠ the card's tile.
* **Condition:** the card is on the hill; the passer ≠ the user.
* **Effect:** (a) the passer [强制停下] on the card's tile; (b) this card
  goes to the user's discard; (c) the user's 团卡 gains exactly 1
  [奇迹水晶].
* **Frame:** passer `pos`; the card's zone; the band-card crystals.
* **Rulings & OPEN:** OPEN-强制停下 (truncated definition) -- only the
  stop-on-this-tile outcome is pinned. The rent-halving follow-on is
  (c2).
* **Executable:** test `everyone_forces_stop_and_crystal` (passer walks
  past the hill; land off the hill; assert stop + discard + crystal).

### C-PPP-everyone-c2

* **ID:** `C-PPP-everyone-c2`
* **Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` [持续]
* **Quote:** 「那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半」
* **Trigger:** the forced-stop settle of (c1), if it pays 地租.
* **Condition:** the stop tile's settle would pay rent to some owner.
* **Effect:** that rent payment is half of the original (rounding: see
  OPEN).
* **Frame:** the rent leg's money.
* **Rulings & OPEN:** rounding of an odd half-rent is not specified
  (OPEN-half-rent). TEST-FINDINGS §6 #4 records the engine's stacking with
  another half-pay as ¼; the clause pins only "half of the original rent",
  not the stacking policy. Only even rents are asserted.
* **Executable:** not in this suite (see Coverage): `everyone_halves_rent` (even base rent).

---

## 户山香澄 -- 非凡之星

**Source:** `data/characters.json#户山香澄`

> （1）其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）
> （2）运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子且可选择盖房

### C-PPP-kasumi-skill-01

* **ID:** `C-PPP-kasumi-skill-01`
* **Source:** `characters.json#户山香澄` （1）
* **Quote:** 「其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）」
* **Trigger:** another player settles on 星之鼓动山丘.
* **Condition:** the skill is bound; the pot is below the cap 1.
* **Effect:** 香澄's [火罐] rises by exactly 1, never past 1; initial 0.
* **Frame:** the fire counter.
* **Rulings & OPEN:** none.
* **Executable:** test `kasumi_fire_on_hill_settle`.

### C-PPP-kasumi-skill-02

* **ID:** `C-PPP-kasumi-skill-02`
* **Source:** `characters.json#户山香澄` （2）
* **Quote:** 「运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子且可选择盖房」
* **Trigger:** the skill press in 运营.
* **Condition:** the user holds ≥1 [火罐]; the user owns ≥1 tile.
* **Effect:** (a) exactly 1 [火罐] is spent; (b) the user enters the move
  stage immediately; (c) the main move is a [传送] to any self-owned tile,
  which settles; a build there is optional.
* **Frame:** fire count / `pos` / landing settle.
* **Rulings & OPEN:** none.
* **Executable:** test `kasumi_skill_teleports_to_own` (1 fire, one owned
  tile).

---

## 花园多惠 -- 花园警察，出警！

**Source:** `data/characters.json#花园多惠`

> （1）每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是“星空齿科”则在结果对应的格子放置一个[多惠兔子]；当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）
> （2）在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个[火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金

### C-PPP-megumi-skill-01a

* **ID:** `C-PPP-megumi-skill-01a`
* **Source:** `characters.json#花园多惠` （1）前段
* **Quote:** 「每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是“星空齿科”则在结果对应的格子放置一个[多惠兔子]」
* **Trigger:** the user领取 [CiRCLE奖励].
* **Condition:** the rolled 3d20 tile is not 星空齿科.
* **Effect:** one [多惠兔子] mark is placed on that tile. If the tile is
  星空齿科, no rabbit is placed.
* **Frame:** the mark's tile.
* **Rulings & OPEN:** the 3d20 sum `S` is the printed 「#S格」, i.e. board
  index `S-1` (1-based, same as 「#1，#16，#31，#46」). A sum of 60 is
  `#60格` = index 59. (STAR BEAT's `mod 60` formula is a 0-based index
  because it is written as a raw number, not a `#N` label.)
* **Executable:** not in this suite (see Coverage): `megumi_rabbit_on_circle_reward` (loaded 3d20 away
  from 星空齿科).

### C-PPP-megumi-skill-01b

* **ID:** `C-PPP-megumi-skill-01b`
* **Source:** `characters.json#花园多惠` （1）后段
* **Quote:** 「当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）」
* **Trigger:** the user [经过] a tile holding ≥1 [多惠兔子].
* **Condition:** the pot has room (cap 4).
* **Effect:** every rabbit on that tile is removed; the user gains that many
  [火罐] (cap 4).
* **Frame:** the tile's marks / the user's fire counter.
* **Rulings & OPEN:** 「等量」 = one pot per rabbit removed.
* **Executable:** not in this suite (see Coverage): `megumi_rabbits_become_fire` (two rabbits on the
  passed tile; +2 fire, marks gone).

### C-PPP-megumi-skill-02

* **ID:** `C-PPP-megumi-skill-02`
* **Source:** `characters.json#花园多惠` （2）
* **Quote:** 「在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个[火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金」
* **Trigger:** another player's character/band [主] effect or a [手] card
  effect is activating (a counteraction window).
* **Condition:** 多惠 holds ≥4 [火罐].
* **Effect:** (a) 4 [火罐] are spent; (b) the target effect is negated; (c)
  多惠 gains 500; (d) the negated player gains 2000.
* **Frame:** both players' money / 多惠's fire.
* **Rulings & OPEN:** this is a [反击] -- see the global counteract area for
  window order. The book does not say what happens if 多惠 is the user
  (「其他玩家」 excludes self).
* **Executable:** not in this suite (see Coverage): `megumi_counteract_negates_for_4_fire` (a [手] play
  from another seat; accept the counteract prompt). The money split is
  asserted exactly.

---

## 牛込里美 -- 里美的决心

**Source:** `data/characters.json#牛込里美`

> （1）你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]（初始1，上限3）
> （2）运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房

### C-PPP-rimi-skill-01

* **ID:** `C-PPP-rimi-skill-01`
* **Source:** `characters.json#牛込里美` （1）
* **Quote:** 「你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]（初始1，上限3）」
* **Trigger:** during 里美's own turn, after [经过] a Live House–colour tile;
  the pot lands at the turn end.
* **Condition:** the pot is below the cap 3.
* **Effect:** at the turn end, [火罐] +1 (cap 3; initial 1).
* **Frame:** the fire counter at turn end.
* **Rulings & OPEN:** 「Live House颜色格子」 is the Live House colour group
  (the tiles sharing Live House's colour), not only the Live House tile
  itself. The exact colour set lives in `data/board.json` `group` and is
  not re-printed here.
* **Executable:** not in this suite (see Coverage): `rimi_fire_after_livehouse_pass` (pass a Live House
  colour tile this turn; turn end; +1 fire).

### C-PPP-rimi-skill-02

* **ID:** `C-PPP-rimi-skill-02`
* **Source:** `characters.json#牛込里美` （2）
* **Quote:** 「运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房」
* **Trigger:** the skill press in 运营.
* **Condition:** ≥3 [火罐]; there is at least one 地产商 tile.
* **Effect:** (a) 3 [火罐] are spent; (b) the user enters the move stage
  immediately; (c) the main move is a [传送] to a 地产商 tile at maximum
  absolute distance, which settles; a build is optional.
* **Frame:** fire / `pos` / landing settle.
* **Rulings & OPEN:** 「地产商」 is the property-agent tile class (see the
  global settle area); the farthest-tie rule is 「任意」 as in
  C-PPP-far-away-01.
* **Executable:** test `rimi_skill_teleports_to_farthest_agent` (3 fire;
  one farthest agent).

---

## 山吹沙绫 -- 焕然一新的天空中

**Source:** `data/characters.json#山吹沙绫`

> （1）其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]（初始3，上限5）；拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10（计算后小等于0则移动到下一个可购买格子）
> （2）[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。

### C-PPP-saaya-skill-01a

* **ID:** `C-PPP-saaya-skill-01a`
* **Source:** `characters.json#山吹沙绫` （1）前段
* **Quote:** 「其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]」
* **Trigger:** another player spends/pays ≥1000 in one money movement,
  while 沙绫 has no saaya标记.
* **Condition:** the optional 「可让」 is taken.
* **Effect:** the spender gains 200; 沙绫 gains 1 saaya标记 and 1 [火罐]
  (cap 5).
* **Frame:** both players' money / 沙绫's tokens and fire.
* **Rulings & OPEN:** the saaya标记 is a player token named `saaya`
  (engine vocabulary; the book's spelling is 「saaya标记」).
* **Executable:** not in this suite (see Coverage): `saaya_offer_on_big_spend` (accept the optional
  window; assert +200 / token 1 / fire +1).

### C-PPP-saaya-skill-01b

* **ID:** `C-PPP-saaya-skill-01b`
* **Source:** `characters.json#山吹沙绫` （1）后段
* **Quote:** 「拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10（计算后小等于0则移动到下一个可购买格子）」
* **Trigger:** the movement dice roll, while holding a saaya标记.
* **Condition:** saaya标记 ≥ 1.
* **Effect:** (a) the mark count falls by 1; (b) the roll total is reduced by
  1d10; (c) if the reduced total ≤ 0, the mover instead goes to the next
  buyable tile.
* **Frame:** the mark count / the walk.
* **Rulings & OPEN:** 「下一个可购买格子」 -- next in the clockwise main-move
  direction, first tile that is buyable. The 1d10 reduction die is a second
  die rolled at the same time as the movement dice.
* **Executable:** not in this suite (see Coverage): `saaya_mark_reduces_roll` (mark 1, loaded dice;
  assert mark 0 and the reduced walk).

### C-PPP-saaya-skill-02

* **ID:** `C-PPP-saaya-skill-02`
* **Source:** `characters.json#山吹沙绫` （2）
* **Quote:** 「[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。」
* **Trigger:** the owner's own [消耗] or [支付] money movement.
* **Condition:** the owner holds ≥5 [火罐].
* **Effect:** 5 [火罐] are spent and the money movement is reduced by 5000,
  floored at 0.
* **Frame:** the spend amount / fire count.
* **Rulings & OPEN:** same floor as Popipapapipopa (2); no rounding.
* **Executable:** not in this suite (see Coverage): `saaya_fire_reduces_spend` (5 fire, spend ≥ 5000).

---

## 市谷有咲 -- 盆栽爱好者

**Source:** `data/characters.json#市谷有咲`

> （1）每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]（初始0，上限2）
> （2）回合结束时如果本回合的[主要移动][经过]“流星堂”且[移动终点]不为“流星堂”时抽取一个视为在“流星堂”抽取的事件并获得一个[火罐]。
> （3）[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。

### C-PPP-aoki-skill-01

* **ID:** `C-PPP-aoki-skill-01`
* **Source:** `characters.json#市谷有咲` （1）
* **Quote:** 「每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]（初始0，上限2）」
* **Trigger:** another player draws an event at 流星堂.
* **Condition:** 有咲's pot < cap 2.
* **Effect:** [火罐] +1 (cap 2; initial 0).
* **Frame:** the fire counter.
* **Rulings & OPEN:** none.
* **Executable:** not in this suite (see Coverage): `aoki_fire_on_other_ryuseido_draw` (another seat's
  流星堂 event draw; +1 fire).

### C-PPP-aoki-skill-02

* **ID:** `C-PPP-aoki-skill-02`
* **Source:** `characters.json#市谷有咲` （2）
* **Quote:** 「回合结束时如果本回合的[主要移动][经过]“流星堂”且[移动终点]不为“流星堂”时抽取一个视为在“流星堂”抽取的事件并获得一个[火罐]。」
* **Trigger:** the owner's turn end.
* **Condition:** this turn's main move [经过] 流星堂 and its end tile is not
  流星堂.
* **Effect:** (a) the owner draws one event **as if** drawn at 流星堂; (b)
  [火罐] +1.
* **Frame:** the event-deck bookkeeping / the fire counter.
* **Rulings & OPEN:** 「视为在流星堂抽取」 -- the draw is attributed to
  流星堂 for every listener keyed on that tile (including (01) for other
  seats).
* **Executable:** not in this suite (see Coverage): `aoki_extra_event_at_turn_end` (main move past
  流星堂, end elsewhere; turn end; one event drawn + fire +1).

### C-PPP-aoki-skill-03

* **ID:** `C-PPP-aoki-skill-03`
* **Source:** `characters.json#市谷有咲` （3）
* **Quote:** 「[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。」
* **Trigger:** the owner [经过] 流星堂, optional spend.
* **Condition:** the owner holds ≥2 [火罐]; a 团卡 instance exists.
* **Effect:** 2 [火罐] are spent and the owner's 团卡 gains exactly 1
  [奇迹水晶].
* **Frame:** fire count / band-card crystals.
* **Rulings & OPEN:** none.
* **Executable:** test `aoki_fire_for_band_crystal` (2 fire, pass 流星堂,
  accept the optional spend).

---

## Poppin' Party band skill -- 星之鼓动

**Source:** `data/bands.json#Poppin' Party`

> （1）[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸，随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]，随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金（使用此卡[奇迹水晶]的效果为1回合1次）。
> （2）无法获取[CiRCLE奖励]
> （3）游戏开始时所有Poppin' Party角色视为同时拥有“星之鼓动山丘”，可正常抵押“星之鼓动山丘”，且非Poppin' Party角色对“星之鼓动山丘”的[结算]改为分摊支付给所有未抵押“星之鼓动山丘”的Poppin' Party角色。“星之鼓动山丘”不可被抵押双倍支付购买，只有全部Poppin' Party角色破产后才可被正常购买。
> （4）不能通过[主要移动]的[结算]盖房。

### C-PPP-band-01a

* **ID:** `C-PPP-band-01a`
* **Source:** `data/bands.json#Poppin' Party` （1）第一句
* **Quote:** 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
* **Trigger:** the skill owner [经过] board index 0, 15, 30, or 45
  (「#N格」 = index `N-1`).
* **Condition:** the band skill is bound.
* **Effect:** 星星贴纸 +1 per such pass.
* **Frame:** the sticker token.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_band_01a` / test `band_sticker_on_corner`.

### C-PPP-band-01b

* **ID:** `C-PPP-band-01b`
* **Source:** `data/bands.json#Poppin' Party` （1）第二句
* **Quote:** 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」
* **Trigger:** the band skill's press, after a corner sticker (or as its
  own press), optional.
* **Condition:** 星星贴纸 ≥ 2.
* **Effect:** 2 stickers are spent and this card gains exactly 1
  [奇迹水晶].
* **Frame:** stickers / this card's crystals.
* **Rulings & OPEN:** 「随后」 chains the three spends in printed order; the
  clause pins the sticker→crystal leg alone.
* **Executable:** test `band_stickers_for_crystal` (2 stickers on field;
  press; +1 crystal, −2 stickers).

### C-PPP-band-01c

* **ID:** `C-PPP-band-01c`
* **Source:** `data/bands.json#Poppin' Party` （1）第三句
* **Quote:** 「随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金（使用此卡[奇迹水晶]的效果为1回合1次）」
* **Trigger:** the band skill's press, optional, once per turn.
* **Condition:** this card's crystals ≥ 2.
* **Effect:** 2 crystals are spent, and the owner either draws exactly 1
  card or gains exactly 2000 money. Only once per turn.
* **Frame:** crystals / hand+draw **or** money.
* **Rulings & OPEN:** the once-per-turn gate is on 「使用此卡[奇迹水晶]的
  效果」 -- both the draw and the cash branch share the one-per-turn budget.
* **Executable:** test `band_crystals_for_cash` and
  not in this suite (see Coverage): `band_crystals_for_draw` (2 crystals; press twice -- the second must not
  fire).

### C-PPP-band-02

* **ID:** `C-PPP-band-02`
* **Source:** `data/bands.json#Poppin' Party` （2）
* **Quote:** 「无法获取[CiRCLE奖励]」
* **Trigger:** the owner passing / landing on CiRCLE where a reward would
  be granted.
* **Condition:** the band skill is bound.
* **Effect:** no [CiRCLE奖励] is granted (no draw, no money choice).
* **Frame:** the reward window is empty.
* **Rulings & OPEN:** the global settle area defines what a [CiRCLE奖励] is;
  this clause only forbids granting it.
* **Executable:** not in this suite (see Coverage): `band_no_circle_reward` (land on CiRCLE; no reward
  prompt / no money-or-draw grant).

### C-PPP-band-03a

* **ID:** `C-PPP-band-03a`
* **Source:** `data/bands.json#Poppin' Party` （3）第一句
* **Quote:** 「游戏开始时所有Poppin' Party角色视为同时拥有“星之鼓动山丘”，可正常抵押“星之鼓动山丘”」
* **Trigger:** game start, continuous afterwards.
* **Condition:** ≥1 Poppin' Party character is in the game.
* **Effect:** every such character is treated as a co-owner of 星之鼓动山丘;
  the hill may be mortgaged normally (by them).
* **Frame:** the hill's ownership view / mortgage flag.
* **Rulings & OPEN:** 「视为同时拥有」 is co-ownership for settle and
  mortgage purposes. How rent splits across co-owners is (3)'s 分摊 clause
  (OPEN-分摊).
* **Executable:** not in this suite (see Coverage): `band_hill_co_owned` (a PPP seat can mortgage the
  hill).

### C-PPP-band-03b

* **ID:** `C-PPP-band-03b`
* **Source:** `data/bands.json#Poppin' Party` （3）第二句
* **Quote:** 「非Poppin' Party角色对“星之鼓动山丘”的[结算]改为分摊支付给所有未抵押“星之鼓动山丘”的Poppin' Party角色」
* **Trigger:** a non-PPP character settling on the hill.
* **Condition:** there is ≥1 PPP character who has not mortgaged the hill.
* **Effect:** the settle payment is **分摊** among those characters (instead
  of a single landlord payment).
* **Frame:** the settle leg's money.
* **Rulings & OPEN:** OPEN-分摊 -- the book never defines 分摊 (equal parts?
  remainder?). The clause specifies only "the payees are exactly the
  unmortgaged PPP characters"; the split amounts are **not** asserted.
* **Executable:** doc-only for the split amounts. The payee *set* is
  checked in not in this suite (see Coverage): `band_hill_split_payees` (pay goes only to unmortgaged PPP
  seats).

### C-PPP-band-03c

* **ID:** `C-PPP-band-03c`
* **Source:** `data/bands.json#Poppin' Party` （3）第三句
* **Quote:** 「“星之鼓动山丘”不可被抵押双倍支付购买，只有全部Poppin' Party角色破产后才可被正常购买。」
* **Trigger:** an attempt to force-buy / double-pay-buy the hill, or a
  normal buy of the hill.
* **Condition:** ≥1 PPP character is still 存活.
* **Effect:** the hill cannot be bought by the double-pay force path; a
  normal buy is only legal once every PPP character is 破产.
* **Frame:** the buy attempt is refused (or the hill's owner stays).
* **Rulings & OPEN:** 「抵押双倍支付购买」 is the force-buy path of 基础[结算]
  (global settle area). 「正常购买」 is the ordinary buy.
* **Executable:** not in this suite (see Coverage): `band_hill_not_buyable_while_ppp_alive` (a non-PPP
  seat's buy of the hill is refused).

### C-PPP-band-04

* **ID:** `C-PPP-band-04`
* **Source:** `data/bands.json#Poppin' Party` （4）
* **Quote:** 「不能通过[主要移动]的[结算]盖房。」
* **Trigger:** a build offer that would arise from a **main-move** settle.
* **Condition:** the band skill is bound (and Returns has not neutralised
  this -- see C-PPP-returns-c1).
* **Effect:** that build is not available / not performed.
* **Frame:** the build window is empty.
* **Rulings & OPEN:** builds from other windows (e.g. 「可选择盖房」 on a
  teleport effect) are **not** blocked -- only 「[主要移动]的[结算]」.
* **Executable:** test `band_no_build_on_main_settle` (main-move land on an
  owned empty tile; no build offered).

---

## Open questions

| OPEN | text | notes |
|---|---|---|
| OPEN-乐队卡 | 乐队卡 | undefined as a card class; 团卡 reads as the band-skill instance |
| OPEN-场上 | [场地] / 游玩区域 | read as the player's field |
| OPEN-强制停下 | [强制停下] | definition truncated in `rulebook-doc.md:93`; clauses pin only the stop outcome |
| OPEN-结算定义 | [结算] | defined twice; 「不触发结算」 read as "no settle fires" |
| OPEN-分摊 | 分摊 | undefined; band (3) payee set is pinned, amounts are not |
| OPEN-wait-causality | 「非因为此卡导致的事件」 | causation undefined; only "not this card's own activation" is pinned |
| OPEN-half-rent | half-rent rounding | （香澄）大家我都喜欢哦 halves rent; odd halves unspecified |
| OPEN-door-hill | co-owned hill as 「[拥有者]拥有的格子」 | TEST-FINDINGS §6 #6, record-only |

## Findings

Classified LESS / MORE / WRONG per the second-pass addendum. Both stay
failing; no assertion is weakened.

### LESS (engine does less than the book)

* **C-PPP-returns-c4** -- 「[拥有者]的通用卡的[手]效果全部生效后获得1个
  星星贴纸」 -- after playing `通用:GREAT` with Returns on the field the
  owner's 星星贴纸 stays at 0. The event window shows the 通用 [手]
  resolving (`card-general.great_added`) and **no** sticker grant. The
  engine does less: the whole effect is missing.
  Test: `returns_sticker_after_general`.

### MORE (does more / leaks / fires when it should not)

(None recorded in this pass.)

### WRONG (different amount / target / timing)

* **C-PPP-rimi-skill-02** -- 「[传送]至任意与你绝对距离最远的地产商」 --
  the skill's pick prompt offers tile 1 (购物中心, a **property**), not a
  地产商 (`kind=agent`). From seat 0 at index 30 the farthest agent is
  高级住宅区 (index 59, distance 29); the engine lands on 购物中心 (also
  distance 29, but the wrong tile class). Target is wrong, not missing.
  Test: `rimi_skill_teleports_to_farthest_agent`.

ses:

* **团卡** = the band-skill field instance (`skill:<band>:<skill>`, `band_skill`).
  Crystal reads/writes land on that instance.
* **星星贴纸** is a player token named `星星贴纸`.
* **多惠兔子** is a tile mark; **saaya标记** is a player token.
* **[奇迹水晶]** is `FieldCard::crystals` on the named field card.
* **[移除]** = leave every private / public zone of the game (not the discard).

---

## PPP:Bang Dream!

**Source:** `cards.json#PPP:Bang Dream!`

> Bang Dream!：
> [手]：
> 为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房。

### C-PPP-bang-dream-01

* **ID:** `C-PPP-bang-dream-01`
* **Source:** `cards.json#PPP:Bang Dream!`
* **Quote:** 「为[使用者]的团卡添加一个[奇迹水晶]」
* **Trigger:** the [手] body of this card, played from hand.
* **Condition:** the user has a band-skill field instance (团卡).
* **Effect:** that instance's crystal count rises by exactly 1.
* **Frame:** only the user's band-card crystal count (and the played-card
  zones) may move on this leg; see `bang_dream_frame`.
* **Rulings & OPEN:** OPEN-乐队卡 does not apply -- 团卡 is the bound band
  skill instance, not a separate 「乐队卡」 class.
* **Executable:** `c_ppp_bang_dream_01` / test `bang_dream_contract`.

### C-PPP-bang-dream-02

* **ID:** `C-PPP-bang-dream-02`
* **Source:** `cards.json#PPP:Bang Dream!`
* **Quote:** 「然后[传送]至任意[使用者]拥有的格子且可选择盖房」
* **Trigger:** same [手] body, after (and only after) the crystal add.
* **Condition:** the user owns at least one tile.
* **Effect:** the user is [传送] to a tile they own. 「可选择盖房」 is optional
  and is not a requirement.
* **Frame:** the user's `pos` (and optionally a house on the destination)
  plus the played-card zones.
* **Rulings & OPEN:** none.
* **Executable:** test `bang_dream_contract` (tile pick answered to an owned
  tile; `pos` must equal that tile).

---

## PPP:Popipa

**Source:** `cards.json#PPP:Popipa`

> Popipa：
> [手]：
> 获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]

### C-PPP-popipa-01

* **ID:** `C-PPP-popipa-01`
* **Source:** `cards.json#PPP:Popipa`
* **Quote:** 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
* **Trigger:** [手] play of `PPP:Popipa`.
* **Condition:** user has room for the draw (or the engine's over-limit
  discard is a different clause, G-CARDS-01).
* **Effect:** (a) money +1000; (b) one `PPP:[衍生]Pipopa` enters the draw pile
  (multiset); (c) exactly one card is drawn; (d) `PPP:Popipa` is [移除] -- it is
  in neither hand, draw, nor discard.
* **Frame:** user money / hand / draw / discard only.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_popipa_01` / test `popipa_contract`.

---

## PPP:Returns

**Source:** `cards.json#PPP:Returns`

> Returns：
>  [特]：
>
> （1）如果此卡被加入初始卡组则卡组卡数添加8。
>
> （2）游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。
> [持续]：
>
> （1）无效[拥有者]Poppin' Party团卡的
> （4）效果。
>
> （2）[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。
>
> （3）使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。
>
> （4）[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。

### C-PPP-returns-01

* **ID:** `C-PPP-returns-01`
* **Source:** `cards.json#PPP:Returns` [特]（1）
* **Quote:** 「如果此卡被加入初始卡组则卡组卡数添加8。」
* **Trigger:** deck-construction / opening-deal window (before play).
* **Condition:** this card is in the starting deck.
* **Effect:** the deck size is 8 larger than the printed base.
* **Frame:** n/a (setup, not an observed transition).
* **Rulings & OPEN:** OPEN-结算定义 not involved.
* **Executable:** not driven as a Step -- deck construction is outside the
  observed-act window (`Table` cannot add to the starting-deck size). The
  opening deal in `Table::with_opening_draw` is the closest seam; left as a
  doc-only clause.

### C-PPP-returns-02

* **ID:** `C-PPP-returns-02`
* **Source:** `cards.json#PPP:Returns` [特]（2）
* **Quote:** 「游戏开始时此卡从卡组放置到拥有此卡的玩家的[场地]上并获得一个其他存活玩家的团卡。」
* **Trigger:** game start, after the opening deal.
* **Condition:** this card was in the owner's starting deck.
* **Effect:** the card sits on the owner's field (not in hand/draw/discard),
  and the owner holds a **borrowed** copy of another surviving player's band
  skill (an extra / copied 团卡 instance).
* **Frame:** the owner's field / private zones.
* **Rulings & OPEN:** 「团卡」 is the band-skill instance. OPEN-乐队卡 is about
  the undefined 「乐队卡」 class and does not cover this. Which band is borrowed
  is a player choice (prompt) and is not asserted.
* **Executable:** not in this suite (see Coverage): `returns_placed_and_borrows_band` (vanilla table with
  Returns forced into the opening draw via `with_opening_draw` is not yet a
  public Table API on `rules_spec`; the field placement is checked when the
  card is already on field and a second surviving seat's band is bound).

### C-PPP-returns-c1

* **ID:** `C-PPP-returns-c1`
* **Source:** `cards.json#PPP:Returns` [持续]（1）
* **Quote:** 「无效[拥有者]Poppin' Party团卡的（4）效果。」
* **Trigger:** continuous, while this card is on the owner's field.
* **Condition:** the owner has a Poppin' Party band skill with clause (4)
  「不能通过[主要移动]的[结算]盖房」.
* **Effect:** that (4) does **not** apply -- the owner may build through a
  main-move settle as if the band (4) were absent.
* **Frame:** the build decision itself is a different act; this clause only
  forbids (4) from blocking it.
* **Rulings & OPEN:** none.
* **Executable:** test `returns_neutralises_band_build_lock` (main-move settle
  offers a build while Returns is on field).

### C-PPP-returns-c2

* **ID:** `C-PPP-returns-c2`
* **Source:** `cards.json#PPP:Returns` [持续]（2）
* **Quote:** 「[拥有者]每回合开始时选择一个其他存活玩家的团卡，如果和当前因此卡获得的团卡不一样则替换并移除上面的所有[奇迹水晶]。」
* **Trigger:** the owner's turn start, while this card is on field.
* **Condition:** there is another surviving player.
* **Effect:** a prompt offers that player's 团卡; choosing a different one
  replaces the borrowed instance and clears **all** crystals on it.
* **Frame:** the borrowed band instance and its crystals.
* **Rulings & OPEN:** 「所有[奇迹水晶]」 is the crystals on the borrowed
  instance only.
* **Executable:** not in this suite (see Coverage): `returns_turn_start_borrow_prompt` (the turn start
  opens a prompt; answering a different band replaces the instance).

### C-PPP-returns-c3

* **ID:** `C-PPP-returns-c3`
* **Source:** `cards.json#PPP:Returns` [持续]（3）
* **Quote:** 「使用因此卡获得的团卡的主动效果时需要支付1星星贴纸。」
* **Trigger:** activating the **borrowed** band skill's active ([主]) effect.
* **Condition:** the borrowed instance is in use; the owner has ≥1 星星贴纸
  (or the activation cannot pay).
* **Effect:** exactly 1 星星贴纸 is spent as a cost of that activation.
* **Frame:** the owner's `星星贴纸` token count.
* **Rulings & OPEN:** none.
* **Executable:** doc-only for the tax in isolation -- the borrowed-skill
  activation path is the same `use_skill` as any band press and the tax is
  observable as a sticker drop when the press runs (covered indirectly by
  `returns_grants_sticker_after_general_card`'s sibling press tests). Not
  given its own forall: the press's body is another card's clause.

### C-PPP-returns-c4

* **ID:** `C-PPP-returns-c4`
* **Source:** `cards.json#PPP:Returns` [持续]（4）
* **Quote:** 「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。」
* **Trigger:** after every [手] effect of a 通用 card has fully resolved, for
  the owner of this card.
* **Condition:** this card is on the owner's field.
* **Effect:** the owner's 星星贴纸 rises by exactly 1.
* **Frame:** owner's 星星贴纸 (and the played 通用 card's own zones).
* **Rulings & OPEN:** 「全部生效后」 = after the whole [手] body, not between
  clauses of the body.
* **Executable:** `c_ppp_returns_c4` / test `returns_sticker_after_general`.

---

## PPP:STAR BEAT!

**Source:** `cards.json#PPP:STAR BEAT!`

> STAR BEAT!：
> [手]：[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”，然后进入移动阶段并选择以下操作之一：
> 1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算；
> 2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算。

### C-PPP-star-beat-01

* **ID:** `C-PPP-star-beat-01`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上进行一次盖房，随后减少1层”」
* **Trigger:** the [手] body, before the move-stage choice.
* **Condition:** none beyond a legal play.
* **Effect:** the user gains 1 layer of that status (a build-after-settle
  option within absolute distance 5 of a self-owned tile, consumed on use).
* **Frame:** the user's status counters.
* **Rulings & OPEN:** 「绝对距离」 is the circular distance already used by
  「献给远方的你」's worked example (#11 vs #1 = 10). The engine's key for
  this status is `star_beat.layers` (observed as a `state:P*` delta when the
  body runs); the book does not name a key, so the clause only requires a
  +1 layer of a status that gates a distance-5 build option.
* **Executable:** checked as a `state:P0:star_beat` frame allowance on the
  STAR BEAT contracts (the layer is granted by the same [手] body as the
  option choice). Not a standalone predicate -- the book does not name the
  status key.

### C-PPP-star-beat-02

* **ID:** `C-PPP-star-beat-02`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「然后进入移动阶段并选择以下操作之一」
* **Trigger:** after the status grant.
* **Condition:** none.
* **Effect:** the user is in the move stage and a prompt offers exactly the
  two printed options (sticker+teleport vs Nd10 walk).
* **Frame:** stage / prompt bookkeeping.
* **Rulings & OPEN:** none.
* **Executable:** not in this suite (see Coverage): `star_beat_offers_two_options` (prompt after play).

### C-PPP-star-beat-03

* **ID:** `C-PPP-star-beat-03`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「1. 获得2个星星贴纸，本回合的主要移动改为[传送]到(45×“资金数包含5的玩家数量+1”) mod 60格并结算」
* **Trigger:** choosing option 1.
* **Condition:** let `F` = number of players whose **money decimal
  representation contains the digit 5**.
* **Effect:** 星星贴纸 +2; the main move becomes a [传送] to tile index
  `(45 * (F + 1)) mod 60` (0-based board index; the book's 「#N格」 is
  index `N-1`, so the printed 「X格」 is read as board index X) **and that
  move settles**.
* **Frame:** user money is unchanged by the option itself (the settle may
  move money); user's sticker count +2; user's `pos` ends on the computed
  tile after the settle.
* **Rulings & OPEN:** 「资金数包含5」 is read as "the decimal representation
  of the money amount contains the digit 5" (e.g. 1500, 5000, 1050 all
  count). No OPEN -- the phrase is determinate enough to evaluate. Tile
  numbering: the board is 60 tiles 0..59 and the formula is `mod 60`, so
  the destination is the board index equal to the formula's value.
* **Executable:** `c_ppp_star_beat_03` / test `star_beat_option_1` (random
  money layouts via `forall`; F is computed in the test from the arranged
  money).

### C-PPP-star-beat-04

* **ID:** `C-PPP-star-beat-04`
* **Source:** `cards.json#PPP:STAR BEAT!`
* **Quote:** 「2. 本回合的主要移动改为移动(2×“资金数包含5的玩家数量+1”)d10格并结算」
* **Trigger:** choosing option 2.
* **Condition:** same `F` as (03).
* **Effect:** the main move becomes a walk of `(2 * (F + 1))` d10 dice,
  summed, and that move settles. No stickers are granted by this option.
* **Frame:** user money unchanged by the option itself; no sticker change;
  `pos` advances by the rolled total (mod 60 path).
* **Rulings & OPEN:** same `F` reading as (03).
* **Executable:** test `star_beat_option_2` (loaded dice; sum of the loaded
  faces is the walk length).

---

## PPP:Tomorrow's Door

**Source:** `cards.json#PPP:Tomorrow's Door`

> Tomorrow's Door：
>
> （1）此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”
>
> （2）将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡
> （1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域
>
> （3）此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。

### C-PPP-door-01

* **ID:** `C-PPP-door-01`
* **Source:** `cards.json#PPP:Tomorrow's Door` （1）
* **Quote:** 「此卡指定的序列（从前往后）为“流星堂”，“花咲川女子学院”，“Space”，“Live House Galaxy”，“武道馆”，“RiNG 1”，“RiNG 2”，“RiNG 3”，“RiNG 4”，“大阪中之岛公园”」
* **Trigger:** n/a -- this is the sequence definition the other clauses read.
* **Condition:** n/a.
* **Effect:** the ordered sequence is exactly those 10 named tiles.
* **Frame:** n/a.
* **Rulings & OPEN:** none.
* **Executable:** constant checked by `door_sequence()` in the test file.

### C-PPP-door-02

* **ID:** `C-PPP-door-02`
* **Source:** `cards.json#PPP:Tomorrow's Door` （2）
* **Quote:** 「将此卡放置在“流星堂”上，此卡使用者每次[经过]此卡所在的格子时把此卡放置到此卡（1）效果的序列中的下一个，如果已经所在为“大阪中之岛公园”则将此卡放置在此卡使用者的游玩区域」
* **Trigger:** the [手] body places the card; afterwards, the **user's** pass
  over the card's current tile.
* **Condition:** the card is in the sequence (or already at the last entry).
* **Effect:** placement starts on 流星堂. Each user pass advances the card to
  the next sequence entry. When the card is already on 大阪中之岛公园, the
  pass moves it to the user's play area (field).
* **Frame:** the card's tile (or field placement).
* **Rulings & OPEN:** 「游玩区域」 is the player's field. Only the **user's**
  passes advance it -- 「此卡使用者每次[经过]」.
* **Executable:** test `door_walks_the_sequence` (place, then user passes
  each intermediate tile; assert the card's tile follows the sequence).

### C-PPP-door-03

* **ID:** `C-PPP-door-03`
* **Source:** `cards.json#PPP:Tomorrow's Door` （3）
* **Quote:** 「此卡在自身游玩区域时[拥有者]以外的玩家在[拥有者]拥有的格子或梦开始的地方[结算]时额外支付[拥有者]星之鼓动山丘上房子数量×100的资金。」
* **Trigger:** another player (not the owner) settling on an owner-owned tile
  or on 梦开始的地方, while the card is in the owner's play area.
* **Condition:** the card is on the owner's field; the settler ≠ owner; the
  settle tile is owner-owned or 梦开始的地方.
* **Effect:** the settler pays an **extra** `houses(星之鼓动山丘) * 100` to
  the owner (on top of the base settle).
* **Frame:** settler and owner money.
* **Rulings & OPEN:** TEST-FINDINGS §6 #6 -- when 星之鼓动山丘 is co-owned by
  the PPP band's 「所有角色视为同时拥有」, does the hill count as
  「[拥有者]拥有的格子」? Record-only RULING, no decision in the book. The
  surcharge **amount** (`houses × 100`) is determinate and is what the
  clause pins; the co-owned-hill trigger is not asserted.
* **Executable:** doc-only for the co-owned-hill case. The amount formula is
  checked in not in this suite (see Coverage): `door_surcharge_amount` when the settle tile is an
  ordinary owner-owned tile (not the hill).

---

## PPP:[衍生]Pipopa

**Source:** `cards.json#PPP:[衍生]Pipopa`

> [衍生]Pipopa：
> [手]：
> 获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]

### C-PPP-pipopa-01

* **ID:** `C-PPP-pipopa-01`
* **Source:** `cards.json#PPP:[衍生]Pipopa`
* **Quote:** 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」
* **Trigger:** [手] play of `PPP:[衍生]Pipopa`.
* **Condition:** none beyond a legal play.
* **Effect:** (a) money +1000; (b) one `PPP:[衍生]Popipapapipopa` is placed on
  the user's field; (c) `PPP:[衍生]Pipopa` is [移除] (not in hand/draw/discard).
* **Frame:** user money / field / played-card zones.
* **Rulings & OPEN:** the printed name 「Popipapapipopa」 is the card
  `PPP:[衍生]Popipapapipopa`.
* **Executable:** `c_ppp_pipopa_01` / test `pipopa_contract`.

---

## PPP:[衍生]Popipapapipopa

**Source:** `cards.json#PPP:[衍生]Popipapapipopa`

> [衍生]Popipapapipopa：
> [持续]
>
> （1）此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。
>
> （2）消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。

### C-PPP-popipapapipopa-c1

* **ID:** `C-PPP-popipapapipopa-c1`
* **Source:** `cards.json#PPP:[衍生]Popipapapipopa` （1）
* **Quote:** 「此卡拥有者每次[经过]“东京外”，“江户川乐器店”，“山吹面包房”，“星之鼓动山丘”或“流星堂”时为此卡添加1个[奇迹水晶]（上限10个）。」
* **Trigger:** the card's owner [经过] one of the five named tiles.
* **Condition:** the card is on the owner's field; crystal count < 10.
* **Effect:** this card's crystals rise by exactly 1, never past 10.
* **Frame:** this card's crystals only (among card state).
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_popipapapipopa_c1` / test
  `popipapapipopa_crystal_on_pass`.

### C-PPP-popipapapipopa-c2

* **ID:** `C-PPP-popipapapipopa-c2`
* **Source:** `cards.json#PPP:[衍生]Popipapapipopa` （2）
* **Quote:** 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」
* **Trigger:** a [消耗] or [支付] the owner is making (the money-moving
  window of that spend).
* **Condition:** the card has ≥1 crystal; the spend is being reduced.
* **Effect:** spending `k` crystals reduces the money movement by `150 * k`,
  floored at 0; those `k` crystals are removed.
* **Frame:** the spend amount and this card's crystals.
* **Rulings & OPEN:** TEST-FINDINGS G26 -- does the reduction also apply to a
  rent the **owner receives**? The quote says 「消耗或支付」 (spend/pay), not
  收取; the clause covers the owner's own spend/pay legs only. The receive
  side is not asserted.
* **Executable:** `c_ppp_popipapapipopa_c2` / test
  `popipapapipopa_crystal_reduces_spend` (k crystals, spend ≥ 150k).

---

## PPP:[衍生]拍卖撤下来了

**Source:** `cards.json#PPP:[衍生]拍卖撤下来了`

> [衍生]拍卖撤下来了：
> [特]：此卡加入手牌时将此卡放置在自身场上，每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）
> [持续]：此卡拥有者火罐上限加1且不受任何其他效果影响。

### C-PPP-auction-off-01

* **ID:** `C-PPP-auction-off-01`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [特]
* **Quote:** 「此卡加入手牌时将此卡放置在自身场上」
* **Trigger:** this card enters a hand (draw / deal / any add-to-hand).
* **Condition:** none.
* **Effect:** it is immediately placed on that player's field (it does not
  stay in the hand).
* **Frame:** that player's field / hand.
* **Rulings & OPEN:** none.
* **Executable:** test `auction_off_places_itself_on_hand_add` (via
  `with_opening_draw` or `give`).

### C-PPP-auction-off-02

* **ID:** `C-PPP-auction-off-02`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [特]
* **Quote:** 「每回合开始时将自身场上的“仓库里的Random Star”[移除]（如果有，然后失去540资金）」
* **Trigger:** the owner's turn start, while this card is on field.
* **Condition:** a `PPP:仓库里的Random Star` is on the owner's field.
* **Effect:** that Random Star is [移除], then the owner loses exactly 540
  money.
* **Frame:** owner's field / money.
* **Rulings & OPEN:** the 「（如果有，然后失去540资金）」 parenthetical is
  read as: the 540 loss is part of this same removal (it fires when the
  Random Star was present and is removed).
* **Executable:** not in this suite (see Coverage): `auction_off_removes_random_star_and_loses_540`.

### C-PPP-auction-off-c1

* **ID:** `C-PPP-auction-off-c1`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [持续]
* **Quote:** 「此卡拥有者火罐上限加1」
* **Trigger:** continuous, while on field.
* **Condition:** the owner's fire-pot max is readable (`state` bounds).
* **Effect:** `max(火罐) = baseline + 1` while the card is present.
* **Frame:** the fire-pot max bound.
* **Rulings & OPEN:** the baseline is the character's printed cap (or the
  engine default 0 when no skill sets one).
* **Executable:** test `auction_off_raises_fire_cap` (max delta +1 vs a
  stripped baseline).

### C-PPP-auction-off-c2

* **ID:** `C-PPP-auction-off-c2`
* **Source:** `cards.json#PPP:[衍生]拍卖撤下来了` [持续]
* **Quote:** 「不受任何其他效果影响」
* **Trigger:** continuous.
* **Condition:** any other effect would touch this instance.
* **Effect:** that effect does not modify this instance (props, crystals,
  removal-by-other, etc.).
* **Frame:** this instance stays as placed.
* **Rulings & OPEN:** the engine's `FieldCard::immune` flag is the natural
  seat of this; the clause only requires the observable "unchanged by
  others" outcome.
* **Executable:** doc-only -- driving a specific "other effect" would require
  naming a second card's body, which is another unit's clause. The flag is
  checked indirectly (`immune` is a snapshot path) in
  `auction_off_raises_fire_cap`'s frame.

---

## PPP:仓库里的Random Star

**Source:** `cards.json#PPP:仓库里的Random Star`

> 仓库里的Random Star：
>
> （1）将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端
>
> （2）[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]

### C-PPP-random-star-01

* **ID:** `C-PPP-random-star-01`
* **Source:** `cards.json#PPP:仓库里的Random Star` （1）
* **Quote:** 「将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”放置在卡组底端」
* **Trigger:** the card's activation (play / body).
* **Condition:** none beyond a legal activation.
* **Effect:** (a) this card is on the owner's field; (b) the discard pile and
  hand are shuffled into the draw pile (multiset union, no other loss); (c)
  exactly one `PPP:[衍生]拍卖撤下来了` sits at the **bottom** of the draw
  pile.
* **Frame:** owner's field / hand / draw / discard.
* **Rulings & OPEN:** 「底端」 is the end opposite the draw face. The table
  exposes the pile top-first, so the auction card is the **last** element of
  `draw_pile()`.
* **Executable:** `c_ppp_random_star_01` / test `random_star_sweeps`.

### C-PPP-random-star-02

* **ID:** `C-PPP-random-star-02`
* **Source:** `cards.json#PPP:仓库里的Random Star` （2）
* **Quote:** 「[经过]“流星堂”时可使用2星星贴纸在“流星堂”强制停下并[结算]」
* **Trigger:** the owner [经过] 流星堂, while this card is on field.
* **Condition:** the owner holds ≥2 星星贴纸 (the cost is 「可使用」 --
  optional).
* **Effect:** spending 2 星星贴纸 force-stops the owner at 流星堂 and that
  tile settles.
* **Frame:** owner's sticker count and `pos`.
* **Rulings & OPEN:** OPEN-强制停下 is truncated in the global book; the
  clause only pins "stops on 流星堂 and settles", not the truncated
  remainder.
* **Executable:** not in this suite (see Coverage): `random_star_force_stop` (2 stickers, pass the tile,
  accept the force-stop).

---

## PPP:向着未来的路标

**Source:** `cards.json#PPP:向着未来的路标`

> 向着未来的路标：
> 本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金

### C-PPP-signpost-01

* **ID:** `C-PPP-signpost-01`
* **Source:** `cards.json#PPP:向着未来的路标`
* **Quote:** 「本回合的主要移动设为移动60格子并不触发结算」
* **Trigger:** this card's activation (its body).
* **Condition:** none beyond a legal activation.
* **Effect:** the turn's main move is a 60-tile walk that does **not**
  trigger 结算 along the way or at the end.
* **Frame:** the walk's `pos` change only (no settle money / no tile
  effects).
* **Rulings & OPEN:** OPEN-结算定义 -- 「不触发结算」 is read as "no
  基础[结算] and no tile settle", which is the only determinate reading; the
  double definition of [结算] is recorded as OPEN but does not change "no
  settle fires".
* **Executable:** test `signpost_sixty_no_settle`.

### C-PPP-signpost-02

* **ID:** `C-PPP-signpost-02`
* **Source:** `cards.json#PPP:向着未来的路标`
* **Quote:** 「回合结束时[失去]1000资金」
* **Trigger:** the owner's turn end, after the main move.
* **Condition:** none.
* **Effect:** the owner's money falls by exactly 1000 ([失去] = delete money).
* **Frame:** owner money only at the turn-end leg.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_signpost_02` / test `signpost_lose_1000_at_end`.

---

## PPP:抓到了

**Source:** `cards.json#PPP:抓到了`

> 抓到了：
> 移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。

### C-PPP-caught-01

* **ID:** `C-PPP-caught-01`
* **Source:** `cards.json#PPP:抓到了`
* **Quote:** 「移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。」
* **Trigger:** this card's activation.
* **Condition:** there is a player on a tile strictly ahead of the user
  (clockwise; 「前方」).
* **Effect:** the user moves to that player's tile; this counts as the turn's
  main move; a build is optional (not required).
* **Frame:** user `pos` (and optionally a house).
* **Rulings & OPEN:** 「你前方一名玩家」 -- when several players are ahead,
  which one is a player choice; the clause pins "the destination is some
  player's tile ahead". 「前方」 is the clockwise forward direction from the
  mover.
* **Executable:** test `caught_moves_to_ahead_player` (one player strictly
  ahead; assert `pos` equals that seat).

---

## PPP:献给远方的你

**Source:** `cards.json#PPP:献给远方的你`

> 献给远方的你：
> [手]：[传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]，然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖（例：[使用者]在#11号格子，一名玩家在#1号格子则绝对距离为10）。

### C-PPP-far-away-01

* **ID:** `C-PPP-far-away-01`
* **Source:** `cards.json#PPP:献给远方的你`
* **Quote:** 「[传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]」
* **Trigger:** the [手] body.
* **Condition:** at least one other player is standing somewhere.
* **Effect:** the user [传送] to a tile occupied by a player at maximum
  absolute distance from the user, and that landing settles.
* **Frame:** user `pos` and settle money / tile effects of the landing tile.
* **Rulings & OPEN:** 「绝对距离」 is circular: the example pins
  `min(|a-b|, 60-|a-b|)` -- #11 vs #1 = 10. Ties (several players equally
  far) make 「任意」 a free choice among them.
* **Executable:** test `far_away_teleports_to_farthest`.

### C-PPP-far-away-02

* **ID:** `C-PPP-far-away-02`
* **Source:** `cards.json#PPP:献给远方的你`
* **Quote:** 「然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖」
* **Trigger:** after the settle of (01).
* **Condition:** the user owns at least one buildable tile at maximum
  absolute distance.
* **Effect:** optionally, the user adds a house on one such tile. (Optional
  -- 「可以」.)
* **Frame:** that tile's house count.
* **Rulings & OPEN:** 「可盖房」 = the tile allows a further house under the
  global build caps; not re-specified here.
* **Executable:** not in this suite (see Coverage): `far_away_optional_house` (decline the build prompt;
  house counts unchanged -- the optional branch that *builds* is a choice,
  so the contract pins the offer, not a forced build).

---

## PPP:迷宫般的仓库

**Source:** `cards.json#PPP:迷宫般的仓库`

> 迷宫般的仓库：
> 位于“流星堂”前后5格内时，可打出此卡，投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动且本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋。如果[经过]“流星堂”则[强制停下]且[消耗]6000资金

### C-PPP-maze-01

* **ID:** `C-PPP-maze-01`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「位于“流星堂”前后5格内时，可打出此卡」
* **Trigger:** attempting to play this card.
* **Condition:** the user's position has absolute distance ≤ 5 from 流星堂
  (前后 = both directions).
* **Effect:** the play is legal (outside that range it is not).
* **Frame:** n/a (gate only).
* **Rulings & OPEN:** 「前后5格内」 is read as absolute distance ≤ 5 (the
  same circular distance as 「献给远方的你」's example). Ties at exactly 5
  are inside.
* **Executable:** test `maze_playable_near_ryuseido` (pos at distance ≤ 5
  accepts the play; distance ≥ 6 refuses).

### C-PPP-maze-02

* **ID:** `C-PPP-maze-02`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「投掷1d10，从“流星堂”开始移动直到[经过]投掷结果对应数量的无主可购买地，视为你的主要移动」
* **Trigger:** the card's activation (legal under (01)).
* **Condition:** the walk can encounter the rolled number of unowned
  buyable tiles.
* **Effect:** one d10 is rolled; the mover starts at 流星堂 and walks until
  it has [经过] exactly N unowned buyable tiles (N = the d10 face); this
  walk is the turn's main move.
* **Frame:** user `pos`.
* **Rulings & OPEN:** 「从流星堂开始移动」 -- the walk's origin is 流星堂
  (the mover is teleported there first), not the pre-play position. The
  direction is the normal clockwise main-move direction (the only walk
  direction the book defines for 主要移动).
* **Executable:** test `maze_walks_from_ryuseido` (loaded d10; count unowned
  buyable tiles passed).

### C-PPP-maze-03

* **ID:** `C-PPP-maze-03`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「本回合购买格子不[消耗]资金，如果购买则拆除那个格子上的所有房屋」
* **Trigger:** a buy of a tile this turn, after this card's move.
* **Condition:** the buyer is the card's user this turn.
* **Effect:** the buy consumes 0 money; if the buy goes through, every house
  on that tile is demolished (houses = 0 after).
* **Frame:** buyer money unchanged by the purchase price; the bought tile's
  houses / owner.
* **Rulings & OPEN:** 「不[消耗]资金」 = the purchase price is 0, not a
  rebate after paying.
* **Executable:** not in this suite (see Coverage): `maze_buy_is_free_and_strips_houses` (unowned
  buyable tile with houses arranged; buy; money unchanged, houses 0).

### C-PPP-maze-04

* **ID:** `C-PPP-maze-04`
* **Source:** `cards.json#PPP:迷宫般的仓库`
* **Quote:** 「如果[经过]“流星堂”则[强制停下]且[消耗]6000资金」
* **Trigger:** the (02) walk [经过] 流星堂.
* **Condition:** the path crosses 流星堂.
* **Effect:** the walk [强制停下] on 流星堂 and the user [消耗] 6000 money.
* **Frame:** user `pos` / money.
* **Rulings & OPEN:** OPEN-强制停下 -- only "stops there and pays 6000" is
  pinned.
* **Executable:** not in this suite (see Coverage): `maze_pass_ryuseido_force_stops_and_pays` (loaded
  d10 that crosses 流星堂).

---

## PPP:（多惠）寻找更好的声音

**Source:** `cards.json#PPP:（多惠）寻找更好的声音` (exclusive: 花园多惠)

> （多惠）寻找更好的声音：
>  传送到“江户川乐器店”并获得一个[火罐]

### C-PPP-better-sound-01

* **ID:** `C-PPP-better-sound-01`
* **Source:** `cards.json#PPP:（多惠）寻找更好的声音`
* **Quote:** 「传送到“江户川乐器店”并获得一个[火罐]」
* **Trigger:** this card's activation (play from hand).
* **Condition:** the user is 花园多惠 (exclusive card).
* **Effect:** (a) the user is [传送] to 江户川乐器店; (b) the user gains
  exactly 1 [火罐] (respecting the cap).
* **Frame:** user `pos` / fire-pot counter / played-card zones.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_better_sound_01` / test `better_sound_contract`
  (character set raw to 花园多惠; fire cap raised so the pot can hold one).

---

## PPP:（有咲）等等等一下

**Source:** `cards.json#PPP:（有咲）等等等一下` (exclusive: 市谷有咲)

> （有咲）等等等一下：
> [手]：
> 将此卡放置在[使用者]的[场地]。
> [持续]：
>
> （1）任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。
>
> （2）事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。

### C-PPP-wait-aoki-01

* **ID:** `C-PPP-wait-aoki-01`
* **Source:** `cards.json#PPP:（有咲）等等等一下` [手]
* **Quote:** 「将此卡放置在[使用者]的[场地]。」
* **Trigger:** the [手] body.
* **Condition:** the user is 市谷有咲.
* **Effect:** the card is on the user's field (not in hand/draw/discard).
* **Frame:** user's field / played-card zones.
* **Rulings & OPEN:** OPEN-场上 -- 「[场地]」 is the player's field, the same
  「游玩区域」 other cards name.
* **Executable:** not in this suite (see Coverage): `wait_aoki_places_on_field`.

### C-PPP-wait-aoki-c1

* **ID:** `C-PPP-wait-aoki-c1`
* **Source:** `cards.json#PPP:（有咲）等等等一下` [持续]（1）
* **Quote:** 「任何非因为此卡导致的事件结算时为此卡添加1个[奇迹水晶]。」
* **Trigger:** any event-settle that this card did not cause.
* **Condition:** the card is on the owner's field.
* **Effect:** this card's crystals rise by exactly 1 per such settle.
* **Frame:** this card's crystals.
* **Rulings & OPEN:** 「非因为此卡导致」 -- the book does not define
  causation. The clause specifies only what is determinate: a settle that
  is not this card's own activation. Causation edges among other cards are
  not inventoried here (OPEN-wait-causality).
* **Executable:** not in this suite (see Coverage): `wait_aoki_crystal_on_foreign_settle` (a plain
  money-moving act that is not this card).

### C-PPP-wait-aoki-c2

* **ID:** `C-PPP-wait-aoki-c2`
* **Source:** `cards.json#PPP:（有咲）等等等一下` [持续]（2）
* **Quote:** 「事件触发时如果此卡拥有至少3个[奇迹水晶]则移除此卡3个[奇迹水晶]，那个事件在[拥有者]回合开始时结算，此后额外抽取一个视为其他玩家在“流星堂”抽取的事件。」
* **Trigger:** an event-card trigger, while this card holds ≥3 crystals.
* **Condition:** crystals ≥ 3 at the trigger.
* **Effect:** (a) exactly 3 crystals leave this card; (b) the triggered event
  is deferred to the owner's next turn start; (c) afterwards an extra event
  is drawn as if another player drew it at 流星堂.
* **Frame:** this card's crystals; the event-deck / event-active bookkeeping;
  the extra draw is an event-deck move.
* **Rulings & OPEN:** OPEN-wait-causality (「事件触发」 vs 「事件结算」
  timing). The deferred-resolution timing (「在[拥有者]回合开始时结算」) is
  determinate and is what the clause pins. The extra draw's player is
  「其他」 -- not the owner -- and its tile is treated as 流星堂.
* **Executable:** doc-only for the deferral -- arranging a deferred event
  card plus a turn-start resolve requires the event-unit's fixtures and is
  out of this unit's card set. Crystal spend of exactly 3 is checked in
  not in this suite (see Coverage): `wait_aoki_c2_spend_three` when a trigger is driven.

---

## PPP:（沙绫）总有一天要给这片天空命名

**Source:** `cards.json#PPP:（沙绫）总有一天要给这片天空命名` (exclusive: 山吹沙绫)

> （沙绫）总有一天要给这片天空命名：
>
> （1）将此卡放置在山吹面包房
>
> （2）[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上

### C-PPP-name-sky-01

* **ID:** `C-PPP-name-sky-01`
* **Source:** `cards.json#PPP:（沙绫）总有一天要给这片天空命名` （1）
* **Quote:** 「将此卡放置在山吹面包房」
* **Trigger:** the card's activation.
* **Condition:** the user is 山吹沙绫.
* **Effect:** the card is placed on tile 山吹面包房 (not on the player's
  field).
* **Frame:** the card's tile placement.
* **Rulings & OPEN:** none.
* **Executable:** test `name_sky_lands_on_yamabuki`.

### C-PPP-name-sky-02

* **ID:** `C-PPP-name-sky-02`
* **Source:** `cards.json#PPP:（沙绫）总有一天要给这片天空命名` （2）
* **Quote:** 「[使用者][经过]此卡后在回合结束后获得1个[火罐]，然后投掷3d20将此卡放置在投掷结果的格子上」
* **Trigger:** the user [经过] this card's current tile; the effect lands at
  the following turn end.
* **Condition:** the card is still on that tile at the turn end.
* **Effect:** (a) the user gains exactly 1 [火罐] (respecting the cap); (b)
  3d20 is rolled and the card is placed on the tile whose **board index**
  equals the rolled sum (the book's 「投掷结果的格子」 -- a 3d20 sum is
  3..60, and the board is 60 tiles).
* **Frame:** the user's fire pot; the card's tile.
* **Rulings & OPEN:** 「投掷结果的格子」 is read as 「#S格」 where `S` is
  the 3d20 sum, i.e. board index `S-1` (the same 1-based reading as
  「#1，#16，#31，#46」). A sum of 60 is `#60格` = index 59.
* **Executable:** not in this suite (see Coverage): `name_sky_pass_grants_fire_and_relocates` (loaded
  3d20; assert the card's tile after the turn end).

---

## PPP:（里美）我的心就像巧克力螺

**Source:** `cards.json#PPP:（里美）我的心就像巧克力螺` (exclusive: 牛込里美)

> （里美）我的心就像巧克力螺：
> [手]：
> 立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。

### C-PPP-chocolate-01

* **ID:** `C-PPP-chocolate-01`
* **Source:** `cards.json#PPP:（里美）我的心就像巧克力螺`
* **Quote:** 「立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。」
* **Trigger:** the [手] body.
* **Condition:** the user is 牛込里美.
* **Effect:** (a) the user is in the move stage immediately; (b) the main
  move is a choice of any tile with absolute distance 1..=4 from the
  current tile, and that landing settles; (c) during this move the user is
  [不可阻挡].
* **Frame:** user `pos` / unstoppable status / landing settle.
* **Rulings & OPEN:** 「1到4格或以内」 is read as absolute distance in
  `1..=4` (the same circular distance as the far-away example). 「期间」
  scopes [不可阻挡] to this move only.
* **Executable:** test `chocolate_move_1_to_4` (answer the tile pick at
  distance 2; assert `pos`; unstoppable is a status key and is checked as
  the move not being redirected -- doc-level for the status key name).

---

## PPP:（香澄）大家我都喜欢哦

**Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` (exclusive: 户山香澄)

> （香澄）大家我都喜欢哦：
> [手]：
> 将此卡放置在“星之鼓动山丘”上。
> [持续]：
> 其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]，那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半。

### C-PPP-everyone-01

* **ID:** `C-PPP-everyone-01`
* **Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` [手]
* **Quote:** 「将此卡放置在“星之鼓动山丘”上。」
* **Trigger:** the [手] body.
* **Condition:** the user is 户山香澄.
* **Effect:** the card is placed on tile 星之鼓动山丘.
* **Frame:** the card's tile placement.
* **Rulings & OPEN:** none.
* **Executable:** test `everyone_lands_on_hill`.

### C-PPP-everyone-c1

* **ID:** `C-PPP-everyone-c1`
* **Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` [持续]
* **Quote:** 「其他玩家[经过]且[移动终点]不为此卡所在格子时那名玩家在此卡所在格子[强制停下]并将此卡放入[使用者]弃卡区且为[使用者]的团卡添加一个[奇迹水晶]」
* **Trigger:** another player [经过] the card's tile, and that player's
  move end tile ≠ the card's tile.
* **Condition:** the card is on the hill; the passer ≠ the user.
* **Effect:** (a) the passer [强制停下] on the card's tile; (b) this card
  goes to the user's discard; (c) the user's 团卡 gains exactly 1
  [奇迹水晶].
* **Frame:** passer `pos`; the card's zone; the band-card crystals.
* **Rulings & OPEN:** OPEN-强制停下 (truncated definition) -- only the
  stop-on-this-tile outcome is pinned. The rent-halving follow-on is
  (c2).
* **Executable:** test `everyone_forces_stop_and_crystal` (passer walks
  past the hill; land off the hill; assert stop + discard + crystal).

### C-PPP-everyone-c2

* **ID:** `C-PPP-everyone-c2`
* **Source:** `cards.json#PPP:（香澄）大家我都喜欢哦` [持续]
* **Quote:** 「那名玩家此次[结算]如果[支付]地租则地租只算作原本的一半」
* **Trigger:** the forced-stop settle of (c1), if it pays 地租.
* **Condition:** the stop tile's settle would pay rent to some owner.
* **Effect:** that rent payment is half of the original (rounding: see
  OPEN).
* **Frame:** the rent leg's money.
* **Rulings & OPEN:** rounding of an odd half-rent is not specified
  (OPEN-half-rent). TEST-FINDINGS §6 #4 records the engine's stacking with
  another half-pay as ¼; the clause pins only "half of the original rent",
  not the stacking policy. Only even rents are asserted.
* **Executable:** not in this suite (see Coverage): `everyone_halves_rent` (even base rent).

---

## 户山香澄 -- 非凡之星

**Source:** `data/characters.json#户山香澄`

> （1）其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）
> （2）运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子且可选择盖房

### C-PPP-kasumi-skill-01

* **ID:** `C-PPP-kasumi-skill-01`
* **Source:** `characters.json#户山香澄` （1）
* **Quote:** 「其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）」
* **Trigger:** another player settles on 星之鼓动山丘.
* **Condition:** the skill is bound; the pot is below the cap 1.
* **Effect:** 香澄's [火罐] rises by exactly 1, never past 1; initial 0.
* **Frame:** the fire counter.
* **Rulings & OPEN:** none.
* **Executable:** test `kasumi_fire_on_hill_settle`.

### C-PPP-kasumi-skill-02

* **ID:** `C-PPP-kasumi-skill-02`
* **Source:** `characters.json#户山香澄` （2）
* **Quote:** 「运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子且可选择盖房」
* **Trigger:** the skill press in 运营.
* **Condition:** the user holds ≥1 [火罐]; the user owns ≥1 tile.
* **Effect:** (a) exactly 1 [火罐] is spent; (b) the user enters the move
  stage immediately; (c) the main move is a [传送] to any self-owned tile,
  which settles; a build there is optional.
* **Frame:** fire count / `pos` / landing settle.
* **Rulings & OPEN:** none.
* **Executable:** test `kasumi_skill_teleports_to_own` (1 fire, one owned
  tile).

---

## 花园多惠 -- 花园警察，出警！

**Source:** `data/characters.json#花园多惠`

> （1）每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是“星空齿科”则在结果对应的格子放置一个[多惠兔子]；当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）
> （2）在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个[火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金

### C-PPP-megumi-skill-01a

* **ID:** `C-PPP-megumi-skill-01a`
* **Source:** `characters.json#花园多惠` （1）前段
* **Quote:** 「每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是“星空齿科”则在结果对应的格子放置一个[多惠兔子]」
* **Trigger:** the user领取 [CiRCLE奖励].
* **Condition:** the rolled 3d20 tile is not 星空齿科.
* **Effect:** one [多惠兔子] mark is placed on that tile. If the tile is
  星空齿科, no rabbit is placed.
* **Frame:** the mark's tile.
* **Rulings & OPEN:** the 3d20 sum `S` is the printed 「#S格」, i.e. board
  index `S-1` (1-based, same as 「#1，#16，#31，#46」). A sum of 60 is
  `#60格` = index 59. (STAR BEAT's `mod 60` formula is a 0-based index
  because it is written as a raw number, not a `#N` label.)
* **Executable:** not in this suite (see Coverage): `megumi_rabbit_on_circle_reward` (loaded 3d20 away
  from 星空齿科).

### C-PPP-megumi-skill-01b

* **ID:** `C-PPP-megumi-skill-01b`
* **Source:** `characters.json#花园多惠` （1）后段
* **Quote:** 「当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得等量的[火罐]（初始2，上限4）」
* **Trigger:** the user [经过] a tile holding ≥1 [多惠兔子].
* **Condition:** the pot has room (cap 4).
* **Effect:** every rabbit on that tile is removed; the user gains that many
  [火罐] (cap 4).
* **Frame:** the tile's marks / the user's fire counter.
* **Rulings & OPEN:** 「等量」 = one pot per rabbit removed.
* **Executable:** not in this suite (see Coverage): `megumi_rabbits_become_fire` (two rabbits on the
  passed tile; +2 fire, marks gone).

### C-PPP-megumi-skill-02

* **ID:** `C-PPP-megumi-skill-02`
* **Source:** `characters.json#花园多惠` （2）
* **Quote:** 「在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个[火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金」
* **Trigger:** another player's character/band [主] effect or a [手] card
  effect is activating (a counteraction window).
* **Condition:** 多惠 holds ≥4 [火罐].
* **Effect:** (a) 4 [火罐] are spent; (b) the target effect is negated; (c)
  多惠 gains 500; (d) the negated player gains 2000.
* **Frame:** both players' money / 多惠's fire.
* **Rulings & OPEN:** this is a [反击] -- see the global counteract area for
  window order. The book does not say what happens if 多惠 is the user
  (「其他玩家」 excludes self).
* **Executable:** not in this suite (see Coverage): `megumi_counteract_negates_for_4_fire` (a [手] play
  from another seat; accept the counteract prompt). The money split is
  asserted exactly.

---

## 牛込里美 -- 里美的决心

**Source:** `data/characters.json#牛込里美`

> （1）你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]（初始1，上限3）
> （2）运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房

### C-PPP-rimi-skill-01

* **ID:** `C-PPP-rimi-skill-01`
* **Source:** `characters.json#牛込里美` （1）
* **Quote:** 「你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]（初始1，上限3）」
* **Trigger:** during 里美's own turn, after [经过] a Live House–colour tile;
  the pot lands at the turn end.
* **Condition:** the pot is below the cap 3.
* **Effect:** at the turn end, [火罐] +1 (cap 3; initial 1).
* **Frame:** the fire counter at turn end.
* **Rulings & OPEN:** 「Live House颜色格子」 is the Live House colour group
  (the tiles sharing Live House's colour), not only the Live House tile
  itself. The exact colour set lives in `data/board.json` `group` and is
  not re-printed here.
* **Executable:** not in this suite (see Coverage): `rimi_fire_after_livehouse_pass` (pass a Live House
  colour tile this turn; turn end; +1 fire).

### C-PPP-rimi-skill-02

* **ID:** `C-PPP-rimi-skill-02`
* **Source:** `characters.json#牛込里美` （2）
* **Quote:** 「运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离最远的地产商并[结算]且可选择盖房」
* **Trigger:** the skill press in 运营.
* **Condition:** ≥3 [火罐]; there is at least one 地产商 tile.
* **Effect:** (a) 3 [火罐] are spent; (b) the user enters the move stage
  immediately; (c) the main move is a [传送] to a 地产商 tile at maximum
  absolute distance, which settles; a build is optional.
* **Frame:** fire / `pos` / landing settle.
* **Rulings & OPEN:** 「地产商」 is the property-agent tile class (see the
  global settle area); the farthest-tie rule is 「任意」 as in
  C-PPP-far-away-01.
* **Executable:** test `rimi_skill_teleports_to_farthest_agent` (3 fire;
  one farthest agent).

---

## 山吹沙绫 -- 焕然一新的天空中

**Source:** `data/characters.json#山吹沙绫`

> （1）其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]（初始3，上限5）；拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10（计算后小等于0则移动到下一个可购买格子）
> （2）[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。

### C-PPP-saaya-skill-01a

* **ID:** `C-PPP-saaya-skill-01a`
* **Source:** `characters.json#山吹沙绫` （1）前段
* **Quote:** 「其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名玩家获得200资金且自己获得1个saaya标记和1个[火罐]」
* **Trigger:** another player spends/pays ≥1000 in one money movement,
  while 沙绫 has no saaya标记.
* **Condition:** the optional 「可让」 is taken.
* **Effect:** the spender gains 200; 沙绫 gains 1 saaya标记 and 1 [火罐]
  (cap 5).
* **Frame:** both players' money / 沙绫's tokens and fire.
* **Rulings & OPEN:** the saaya标记 is a player token named `saaya`
  (engine vocabulary; the book's spelling is 「saaya标记」).
* **Executable:** not in this suite (see Coverage): `saaya_offer_on_big_spend` (accept the optional
  window; assert +200 / token 1 / fire +1).

### C-PPP-saaya-skill-01b

* **ID:** `C-PPP-saaya-skill-01b`
* **Source:** `characters.json#山吹沙绫` （1）后段
* **Quote:** 「拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10（计算后小等于0则移动到下一个可购买格子）」
* **Trigger:** the movement dice roll, while holding a saaya标记.
* **Condition:** saaya标记 ≥ 1.
* **Effect:** (a) the mark count falls by 1; (b) the roll total is reduced by
  1d10; (c) if the reduced total ≤ 0, the mover instead goes to the next
  buyable tile.
* **Frame:** the mark count / the walk.
* **Rulings & OPEN:** 「下一个可购买格子」 -- next in the clockwise main-move
  direction, first tile that is buyable. The 1d10 reduction die is a second
  die rolled at the same time as the movement dice.
* **Executable:** not in this suite (see Coverage): `saaya_mark_reduces_roll` (mark 1, loaded dice;
  assert mark 0 and the reduced walk).

### C-PPP-saaya-skill-02

* **ID:** `C-PPP-saaya-skill-02`
* **Source:** `characters.json#山吹沙绫` （2）
* **Quote:** 「[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。」
* **Trigger:** the owner's own [消耗] or [支付] money movement.
* **Condition:** the owner holds ≥5 [火罐].
* **Effect:** 5 [火罐] are spent and the money movement is reduced by 5000,
  floored at 0.
* **Frame:** the spend amount / fire count.
* **Rulings & OPEN:** same floor as Popipapapipopa (2); no rounding.
* **Executable:** not in this suite (see Coverage): `saaya_fire_reduces_spend` (5 fire, spend ≥ 5000).

---

## 市谷有咲 -- 盆栽爱好者

**Source:** `data/characters.json#市谷有咲`

> （1）每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]（初始0，上限2）
> （2）回合结束时如果本回合的[主要移动][经过]“流星堂”且[移动终点]不为“流星堂”时抽取一个视为在“流星堂”抽取的事件并获得一个[火罐]。
> （3）[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。

### C-PPP-aoki-skill-01

* **ID:** `C-PPP-aoki-skill-01`
* **Source:** `characters.json#市谷有咲` （1）
* **Quote:** 「每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]（初始0，上限2）」
* **Trigger:** another player draws an event at 流星堂.
* **Condition:** 有咲's pot < cap 2.
* **Effect:** [火罐] +1 (cap 2; initial 0).
* **Frame:** the fire counter.
* **Rulings & OPEN:** none.
* **Executable:** not in this suite (see Coverage): `aoki_fire_on_other_ryuseido_draw` (another seat's
  流星堂 event draw; +1 fire).

### C-PPP-aoki-skill-02

* **ID:** `C-PPP-aoki-skill-02`
* **Source:** `characters.json#市谷有咲` （2）
* **Quote:** 「回合结束时如果本回合的[主要移动][经过]“流星堂”且[移动终点]不为“流星堂”时抽取一个视为在“流星堂”抽取的事件并获得一个[火罐]。」
* **Trigger:** the owner's turn end.
* **Condition:** this turn's main move [经过] 流星堂 and its end tile is not
  流星堂.
* **Effect:** (a) the owner draws one event **as if** drawn at 流星堂; (b)
  [火罐] +1.
* **Frame:** the event-deck bookkeeping / the fire counter.
* **Rulings & OPEN:** 「视为在流星堂抽取」 -- the draw is attributed to
  流星堂 for every listener keyed on that tile (including (01) for other
  seats).
* **Executable:** not in this suite (see Coverage): `aoki_extra_event_at_turn_end` (main move past
  流星堂, end elsewhere; turn end; one event drawn + fire +1).

### C-PPP-aoki-skill-03

* **ID:** `C-PPP-aoki-skill-03`
* **Source:** `characters.json#市谷有咲` （3）
* **Quote:** 「[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。」
* **Trigger:** the owner [经过] 流星堂, optional spend.
* **Condition:** the owner holds ≥2 [火罐]; a 团卡 instance exists.
* **Effect:** 2 [火罐] are spent and the owner's 团卡 gains exactly 1
  [奇迹水晶].
* **Frame:** fire count / band-card crystals.
* **Rulings & OPEN:** none.
* **Executable:** test `aoki_fire_for_band_crystal` (2 fire, pass 流星堂,
  accept the optional spend).

---

## Poppin' Party band skill -- 星之鼓动

**Source:** `data/bands.json#Poppin' Party`

> （1）[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸，随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]，随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金（使用此卡[奇迹水晶]的效果为1回合1次）。
> （2）无法获取[CiRCLE奖励]
> （3）游戏开始时所有Poppin' Party角色视为同时拥有“星之鼓动山丘”，可正常抵押“星之鼓动山丘”，且非Poppin' Party角色对“星之鼓动山丘”的[结算]改为分摊支付给所有未抵押“星之鼓动山丘”的Poppin' Party角色。“星之鼓动山丘”不可被抵押双倍支付购买，只有全部Poppin' Party角色破产后才可被正常购买。
> （4）不能通过[主要移动]的[结算]盖房。

### C-PPP-band-01a

* **ID:** `C-PPP-band-01a`
* **Source:** `data/bands.json#Poppin' Party` （1）第一句
* **Quote:** 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
* **Trigger:** the skill owner [经过] board index 0, 15, 30, or 45
  (「#N格」 = index `N-1`).
* **Condition:** the band skill is bound.
* **Effect:** 星星贴纸 +1 per such pass.
* **Frame:** the sticker token.
* **Rulings & OPEN:** none.
* **Executable:** `c_ppp_band_01a` / test `band_sticker_on_corner`.

### C-PPP-band-01b

* **ID:** `C-PPP-band-01b`
* **Source:** `data/bands.json#Poppin' Party` （1）第二句
* **Quote:** 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」
* **Trigger:** the band skill's press, after a corner sticker (or as its
  own press), optional.
* **Condition:** 星星贴纸 ≥ 2.
* **Effect:** 2 stickers are spent and this card gains exactly 1
  [奇迹水晶].
* **Frame:** stickers / this card's crystals.
* **Rulings & OPEN:** 「随后」 chains the three spends in printed order; the
  clause pins the sticker→crystal leg alone.
* **Executable:** test `band_stickers_for_crystal` (2 stickers on field;
  press; +1 crystal, −2 stickers).

### C-PPP-band-01c

* **ID:** `C-PPP-band-01c`
* **Source:** `data/bands.json#Poppin' Party` （1）第三句
* **Quote:** 「随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金（使用此卡[奇迹水晶]的效果为1回合1次）」
* **Trigger:** the band skill's press, optional, once per turn.
* **Condition:** this card's crystals ≥ 2.
* **Effect:** 2 crystals are spent, and the owner either draws exactly 1
  card or gains exactly 2000 money. Only once per turn.
* **Frame:** crystals / hand+draw **or** money.
* **Rulings & OPEN:** the once-per-turn gate is on 「使用此卡[奇迹水晶]的
  效果」 -- both the draw and the cash branch share the one-per-turn budget.
* **Executable:** test `band_crystals_for_cash` and
  not in this suite (see Coverage): `band_crystals_for_draw` (2 crystals; press twice -- the second must not
  fire).

### C-PPP-band-02

* **ID:** `C-PPP-band-02`
* **Source:** `data/bands.json#Poppin' Party` （2）
* **Quote:** 「无法获取[CiRCLE奖励]」
* **Trigger:** the owner passing / landing on CiRCLE where a reward would
  be granted.
* **Condition:** the band skill is bound.
* **Effect:** no [CiRCLE奖励] is granted (no draw, no money choice).
* **Frame:** the reward window is empty.
* **Rulings & OPEN:** the global settle area defines what a [CiRCLE奖励] is;
  this clause only forbids granting it.
* **Executable:** not in this suite (see Coverage): `band_no_circle_reward` (land on CiRCLE; no reward
  prompt / no money-or-draw grant).

### C-PPP-band-03a

* **ID:** `C-PPP-band-03a`
* **Source:** `data/bands.json#Poppin' Party` （3）第一句
* **Quote:** 「游戏开始时所有Poppin' Party角色视为同时拥有“星之鼓动山丘”，可正常抵押“星之鼓动山丘”」
* **Trigger:** game start, continuous afterwards.
* **Condition:** ≥1 Poppin' Party character is in the game.
* **Effect:** every such character is treated as a co-owner of 星之鼓动山丘;
  the hill may be mortgaged normally (by them).
* **Frame:** the hill's ownership view / mortgage flag.
* **Rulings & OPEN:** 「视为同时拥有」 is co-ownership for settle and
  mortgage purposes. How rent splits across co-owners is (3)'s 分摊 clause
  (OPEN-分摊).
* **Executable:** not in this suite (see Coverage): `band_hill_co_owned` (a PPP seat can mortgage the
  hill).

### C-PPP-band-03b

* **ID:** `C-PPP-band-03b`
* **Source:** `data/bands.json#Poppin' Party` （3）第二句
* **Quote:** 「非Poppin' Party角色对“星之鼓动山丘”的[结算]改为分摊支付给所有未抵押“星之鼓动山丘”的Poppin' Party角色」
* **Trigger:** a non-PPP character settling on the hill.
* **Condition:** there is ≥1 PPP character who has not mortgaged the hill.
* **Effect:** the settle payment is **分摊** among those characters (instead
  of a single landlord payment).
* **Frame:** the settle leg's money.
* **Rulings & OPEN:** OPEN-分摊 -- the book never defines 分摊 (equal parts?
  remainder?). The clause specifies only "the payees are exactly the
  unmortgaged PPP characters"; the split amounts are **not** asserted.
* **Executable:** doc-only for the split amounts. The payee *set* is
  checked in not in this suite (see Coverage): `band_hill_split_payees` (pay goes only to unmortgaged PPP
  seats).

### C-PPP-band-03c

* **ID:** `C-PPP-band-03c`
* **Source:** `data/bands.json#Poppin' Party` （3）第三句
* **Quote:** 「“星之鼓动山丘”不可被抵押双倍支付购买，只有全部Poppin' Party角色破产后才可被正常购买。」
* **Trigger:** an attempt to force-buy / double-pay-buy the hill, or a
  normal buy of the hill.
* **Condition:** ≥1 PPP character is still 存活.
* **Effect:** the hill cannot be bought by the double-pay force path; a
  normal buy is only legal once every PPP character is 破产.
* **Frame:** the buy attempt is refused (or the hill's owner stays).
* **Rulings & OPEN:** 「抵押双倍支付购买」 is the force-buy path of 基础[结算]
  (global settle area). 「正常购买」 is the ordinary buy.
* **Executable:** not in this suite (see Coverage): `band_hill_not_buyable_while_ppp_alive` (a non-PPP
  seat's buy of the hill is refused).

### C-PPP-band-04

* **ID:** `C-PPP-band-04`
* **Source:** `data/bands.json#Poppin' Party` （4）
* **Quote:** 「不能通过[主要移动]的[结算]盖房。」
* **Trigger:** a build offer that would arise from a **main-move** settle.
* **Condition:** the band skill is bound (and Returns has not neutralised
  this -- see C-PPP-returns-c1).
* **Effect:** that build is not available / not performed.
* **Frame:** the build window is empty.
* **Rulings & OPEN:** builds from other windows (e.g. 「可选择盖房」 on a
  teleport effect) are **not** blocked -- only 「[主要移动]的[结算]」.
* **Executable:** test `band_no_build_on_main_settle` (main-move land on an
  owned empty tile; no build offered).

---

## Open questions

| OPEN | text | notes |
|---|---|---|
| OPEN-乐队卡 | 乐队卡 | undefined as a card class; 团卡 reads as the band-skill instance |
| OPEN-场上 | [场地] / 游玩区域 | read as the player's field |
| OPEN-强制停下 | [强制停下] | definition truncated in `rulebook-doc.md:93`; clauses pin only the stop outcome |
| OPEN-结算定义 | [结算] | defined twice; 「不触发结算」 read as "no settle fires" |
| OPEN-分摊 | 分摊 | undefined; band (3) payee set is pinned, amounts are not |
| OPEN-wait-causality | 「非因为此卡导致的事件」 | causation undefined; only "not this card's own activation" is pinned |
| OPEN-half-rent | half-rent rounding | （香澄）大家我都喜欢哦 halves rent; odd halves unspecified |
| OPEN-door-hill | co-owned hill as 「[拥有者]拥有的格子」 | TEST-FINDINGS §6 #6, record-only |

## Findings

* **C-PPP-returns-c4** (genuine, stays failing): 「[拥有者]的通用卡的[手]效果
  全部生效后获得1个星星贴纸」 -- after playing `通用:GREAT` with Returns on
  the field, the owner's 星星贴纸 stays at 0. The event window shows the
  通用 card's [手] resolving (`card-general.great_added`) with no sticker
  grant. Test: `returns_sticker_after_general`.
* **C-PPP-rimi-skill-02** (genuine, stays failing): 「[传送]至任意与你绝对
  距离最远的地产商」 -- the skill's pick prompt offers tile 1 (购物中心,
  a property), not a 地产商 (`kind=agent`) tile. From seat 0 at index 30 the
  farthest agent is 高级住宅区 (index 59, distance 29); the engine lands on
  购物中心 (distance 29 as well, but not an agent). Test:
  `rimi_skill_teleports_to_farthest_agent`.

## Coverage gaps

### Iff-covered (items 1-5: exact effect + exact frame + mutants)

These contracts have a pure verifier and a `*_verifier_rejects_mutants`
test that rejects LESS / MORE / WRONG mutants of a real engine window:

| clause | verifier | driver |
|---|---|---|
| C-PPP-popipa-01 | `verify_popipa` | `popipa_verifier_rejects_mutants` |
| C-PPP-pipopa-01 | `verify_pipopa` | `pipopa_verifier_rejects_mutants` |
| C-PPP-bang-dream-01/02 | `verify_bang_dream` | `bang_dream_verifier_rejects_mutants` |
| C-PPP-better-sound-01 | `verify_better_sound` | `better_sound_verifier_rejects_mutants` |
| C-PPP-signpost-01/02 | `verify_signpost` | `signpost_verifier_rejects_mutants` |
| C-PPP-star-beat-03 | `verify_star_beat_03` | `star_beat_03_verifier_rejects_mutants` |
| C-PPP-popipapapipopa-c1 | `verify_popipapapipopa_pass` | `popipapapipopa_pass_verifier_rejects_mutants` |
| C-PPP-band-01a | `verify_band_sticker` | `band_sticker_verifier_rejects_mutants` |

No-op / boundary drivers (item 3): `maze_outside_gate_is_noop`,
`bang_dream_without_land_is_refused`, `popipa_noop_on_unrelated_act`.

Driver-only contracts (exact numbers + frame in the `forall` body, but no
separate mutant table yet): caught, far-away, STAR BEAT (04),
Popipapapipopa spend, Random Star (1), name-sky (1), chocolate, everyone
(place + force-stop), kasumi skill (1)(2), rimi skill (2), aoki skill (3),
band (1b)(1c)(4), Returns (c4), door (1)(2), auction (c1), maze (1)(2).
Their assertions are exact; the mutant half of item 5 is the remaining
upgrade.

### Not armed (missing seam / OPEN / another unit's window)

The quote is specified above; the gap is one of: no harness seam, OPEN, or
another unit's window.

* **C-PPP-returns-01** -- **gap**: no seam to observe deck construction.
  A seam would need a pre-`quick_start` hook that counts the starting deck.
* **C-PPP-returns-02** -- **partial**: game-start placement is the fixture
  for `returns_sticker_after_general`; the borrowed-band pick (which other
  player's 团卡) is not asserted. A seam would need to expose the pick
  options' player ids.
* **C-PPP-returns-c2 / c3** -- the borrow prompt and the borrowed-skill
  sticker tax live on another skill's press body.
* **C-PPP-star-beat-01** -- the status key is not named in the book; the
  +1 layer is only a frame allowance.
* **C-PPP-door-03** -- surcharge amount and the co-owned-hill trigger
  (TEST-FINDINGS §6 #6 / OPEN-door-hill).
* **C-PPP-auction-off-01 / 02 / c2** -- hand-add [特] needs a hooked draw
  (the `give` seam skips it); the turn-start Random Star sweep and
  「不受任何其他效果影响」 need a named second effect.
* **C-PPP-random-star-02** -- OPEN-强制停下.
* **C-PPP-maze-03 / 04** -- free-buy + demolish and the 流星堂 force-stop
  need a multi-step walk + buy window; OPEN-强制停下 on (04).
* **C-PPP-wait-aoki-01 / c1 / c2** -- [场地] placement is arrange-seam
  only; 「非因为此卡导致」 is OPEN-wait-causality; the deferral needs an
  event-card fixture (unit 21).
* **C-PPP-name-sky-02** -- the turn-end 3d20 relocation.
* **C-PPP-everyone-c2** -- OPEN-half-rent.
* **C-PPP-megumi-skill-01a / 01b / 02** -- CiRCLE-reward rabbit placement,
  rabbit→fire, and the 4-fire [反击] need a reward/counteract window.
* **C-PPP-rimi-skill-01** -- turn-end fire after a Live House pass.
* **C-PPP-saaya-skill-01a / 01b / 02** -- the optional offer, the saaya
  mark roll reduction, and the 5-fire spend reduction.
* **C-PPP-aoki-skill-01 / 02** -- another seat's 流星堂 event draw and the
  turn-end extra event.
* **C-PPP-band-01c** once-per-turn gate (the second press) and the draw
  branch.
* **C-PPP-band-02 / 03a / 03b / 03c** -- CiRCLE-reward lockout, hill
  co-ownership / mortgage, OPEN-分摊 payee set, and the buy lockout.
  **Seams needed:** a CiRCLE-reward grant window; a mortgageable-tile list
  for co-owned hills; a payee ledger for a 分摊 leg; a force-buy gate.

**Coverage-gap summary (item 6).** Iff-covered (verifier + mutants): 8
clause groups. Driver-only (exact numbers + frame, no mutant table): 16
clause groups. Not armed (missing seam / OPEN / other unit): 25 clause
groups, each listed above with the seam that would close it. Clauses whose
Executable already names a `card_ppp.rs` test are the driver-only set.
