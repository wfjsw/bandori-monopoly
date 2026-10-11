# C-SUMIMI -- Sumimi 卡 / 角色技能 / 乐队技能

Unit `c-sumimi` owns this file and `rules/tests/tests/spec_card_sumimi.rs`.

Sources (precedence): live sheet (`docs/rulebook/cards.json`,
`data/characters.json` `text`, `data/bands.json` `text`) >
`docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not authoritative.
TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

## Scope

**Cards** -- the 9 `Sumimi:` entries in `docs/rulebook/cards.json` (sheet-derived).

**Source discrepancy (not in scope):** `data/cards.json` lists **11** Sumimi
cards. The two extras are unnamed placeholders `Sumimi:#L11` and
`Sumimi:#L12` (cells L11 / L12). The sheet-derived
`docs/rulebook/cards.json` wins; those two extras are **out of scope** and
recorded here as a source discrepancy only.

**Character skills** -- every `band == "Sumimi"` row in
`data/characters.json`:

| character | skill | exclusive |
|---|---|---|
| 纯田真奈 | 甜甜圈爱好者 | 歌唱大赛5连冠 |
| 三角初华（Sumimi） | 成为偶像 | 儿时玩伴的鼓励 |

**Band skill** -- `data/bands.json` `Sumimi` / `人气偶像组合`.

Clause IDs: `C-SUMIMI-<slug>-<n>`.

---

## Cards

### C-SUMIMI-childhood -- (初华（Sumimi）)儿时玩伴的鼓励

Card id `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励`, owner 三角初华（Sumimi）.

#### C-SUMIMI-childhood-01

* **ID:** `C-SUMIMI-childhood-01`
* **Source:** `cards.json#Sumimi:(初华（Sumimi）)儿时玩伴的鼓励`
* **Quote:** 「可在移动掷骰前打出此卡，使本次移动以“小豆岛”为起点并在移动后获得一个火罐。」
* **Trigger:** the [手] effect, played from hand before the move roll.
* **Condition:** the user is about to make their main move roll (移动掷骰前);
  the card is in hand and the owner matches the character card.
* **Effect:** this move's 移动起点 is 小豆岛 (tile 「小豆岛」).
* **Frame:** see [`C-SUMIMI-childhood-frame`].
* **Rulings & OPEN:** none.
* **Executable:** `c_sumimi_childhood_start`.

#### C-SUMIMI-childhood-02

* **ID:** `C-SUMIMI-childhood-02`
* **Source:** same
* **Quote:** 「…并在移动后获得一个火罐。」
* **Trigger:** after the move this card re-based.
* **Condition:** the move completed.
* **Effect:** the user's [火罐] count rises by 1 (subject to the usual cap).
* **Frame:** fire-pot path only.
* **Rulings & OPEN:** 「移动后」 is the post-move window
  (rulebook-doc.md 行动阶段 row 13).
* **Executable:** `c_sumimi_childhood_fire`.

#### C-SUMIMI-childhood-frame

* **ID:** `C-SUMIMI-childhood-frame`
* **Source:** same quote
* **Frame:** only the user's `pos` / `state:fire` / hand / discard may move.

---

### C-SUMIMI-heretheworld -- Here the world

Card id `Sumimi:Here the world`.

#### C-SUMIMI-heretheworld-01

* **ID:** `C-SUMIMI-heretheworld-01`
* **Source:** `cards.json#Sumimi:Here the world`
* **Quote:** 「（1）[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。」
* **Trigger:** [反击] window when some player has played two cards in the
  same turn.
* **Condition:** the responder holds this card; the trigger player has played
  two cards in one turn (「同一回合内」).
* **Effect:** this card is placed on that player's 场上 (field).
* **Frame:** the card moves from the responder's hand to the trigger player's
  field.
* **Rulings & OPEN:** OPEN-回合 definition of 「同一回合」 follows the turn
  bookkeeping (`turn.played`); the card class 「场上」 is OPEN-场上 for
  non-card zones -- here the text says 「对方场上」 which is the player's
  field.
* **Executable:** `c_sumimi_heretheworld_place`.

#### C-SUMIMI-heretheworld-02

* **ID:** `C-SUMIMI-heretheworld-02`
* **Source:** same
* **Quote:** 「（2）场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，」
* **Trigger:** the next draw of the player who has this card on their field.
* **Condition:** this card is on that player's field; they draw a card.
* **Effect:** the drawn card is placed face-down on this card; this card
  receives 3 [奇迹水晶].
* **Frame:** draw pile / this field instance's crystals + the face-down
  carrier.
* **Rulings & OPEN:** face-down storage representation is engine-defined;
  the contract checks the 3 crystals and that the drawn card did not enter
  the hand.
* **Executable:** `c_sumimi_heretheworld_capture`.

#### C-SUMIMI-heretheworld-03

* **ID:** `C-SUMIMI-heretheworld-03`
* **Source:** same
* **Quote:** 「那名玩家的每个回合开始时移除一个，」
* **Trigger:** each turn start of the player who has this card on their field.
* **Condition:** this card still holds crystals.
* **Effect:** remove one [奇迹水晶] from this card.
* **Frame:** crystals on this instance only.
* **Executable:** `c_sumimi_heretheworld_tick`.

#### C-SUMIMI-heretheworld-04

* **ID:** `C-SUMIMI-heretheworld-04`
* **Source:** same
* **Quote:** 「当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。」
* **Trigger:** when this card's crystal count reaches 0.
* **Condition:** the captured card is still on this instance.
* **Effect:** the captured card enters that player's hand; this card's
  original user draws 1 card.
* **Frame:** hand / draw piles / the field instance (captured card leaves).
* **Rulings & OPEN:** 「此卡使用者」 is the player who played the [反击]
  (the original user of this card), not the current field owner.
* **Executable:** not separately driven. (1) never places the card
  (see Findings), so (2)-(4) have no live instance to tick.

---

### C-SUMIMI-nobreakup -- Sumimi不会解散哦

Card id `Sumimi:Sumimi不会解散哦`.

#### C-SUMIMI-nobreakup-01

* **ID:** `C-SUMIMI-nobreakup-01`
* **Source:** `cards.json#Sumimi:Sumimi不会解散哦`
* **Quote:** 「当你本回合未进行过抵押/赎回操作且资金不含有相同数字时可打出，」
* **Trigger:** play from hand.
* **Condition:** this turn the user has not mortgage/redeemed; the user's
  money digits are all distinct (「不含有相同数字」).
* **Effect:** the play is allowed (gate).
* **Rulings & OPEN:** 「含有相同数字」 = at least one digit repeats. A
  multiset of decimal digits with no repeats.
* **Executable:** `c_sumimi_nobreakup_gate`.

#### C-SUMIMI-nobreakup-02

* **ID:** `C-SUMIMI-nobreakup-02`
* **Source:** same
* **Quote:** 「进行一次可使你的资金变动为拥有相同数字的传送，视为你的主要移动；」
* **Trigger:** the [手] body, once the gate holds.
* **Condition:** the gate holds.
* **Effect:** the user teleports (path = endpoint only) and this counts as
  their 主要移动; the post-settle money 「拥有相同数字」.
* **Rulings & OPEN:** **OPEN-C-SUMIMI-nobreakup-digits** -- 「拥有相同数字」
  is not defined: it may mean *all digits equal* (1111) or *has a repeated
  digit* (the dual of the gate). The contract only checks the structure
  (teleport as main move) and does not invent the digit predicate. See
  Open questions.
* **Executable:** `c_sumimi_nobreakup_teleport`.

#### C-SUMIMI-nobreakup-03

* **ID:** `C-SUMIMI-nobreakup-03`
* **Source:** same
* **Quote:** 「若传送并触发结算后未能使资金变为拥有相同数字，回到原处并取消所有受到的效果」
* **Trigger:** after the teleport and its 触发结算.
* **Condition:** the post-settle money is not 「拥有相同数字」.
* **Effect:** the user returns to the original tile; every effect received
  from that teleport/settle is cancelled.
* **Rulings & OPEN:** depends on OPEN-C-SUMIMI-nobreakup-digits. Also
  OPEN-触发结算. The revert of 「所有受到的效果」 is a full transaction
  rollback; the contract is specified but only the gate/structure is
  executable without a digit predicate.
* **Executable:** not separately driven. The digit target
  (OPEN-C-SUMIMI-nobreakup-digits) and the rollback of 「所有受到的效果」
  need a defined target predicate and a transaction-revert seam the
  harness does not expose. The gate half (`C-SUMIMI-nobreakup-01`) is
  executable and green.

---

### C-SUMIMI-twoinone -- Sumimi是二人一体的

Card id `Sumimi:Sumimi是二人一体的`.

#### C-SUMIMI-twoinone-01

* **ID:** `C-SUMIMI-twoinone-01`
* **Source:** `cards.json#Sumimi:Sumimi是二人一体的`
* **Quote:** 「（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
* **Trigger:** the [手] / declared play of this card.
* **Condition:** the user's character is one of the two Sumimi characters.
* **Effect:** the character card becomes the *other* Sumimi character, and
  the user's [火罐] becomes that character's 初始火罐数
  (纯田真奈 initial 1 / max 2; 三角初华（Sumimi） initial 2 / max 2).
* **Frame:** `character:P*` and `state:P*:fire` (and the bound skills that
  ride the character).
* **Rulings & OPEN:** 「sumimi的另一名角色」 = the other row of
  `data/characters.json` with `band == "Sumimi"`.
* **Executable:** `c_sumimi_twoinone_swap`.

#### C-SUMIMI-twoinone-01b

* **ID:** `C-SUMIMI-twoinone-01b`
* **Source:** `cards.json#Sumimi:Sumimi是二人一体的`
* **Quote:** 「（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
* **Trigger:** the same play.
* **Condition:** the user is one of the two Sumimi characters.
* **Effect:** the user's [火罐] becomes the *other* character's 初始火罐数
  (纯田真奈 1 / 三角初华（Sumimi） 2).
* **Frame:** `state:P*:fire`.
* **Executable:** `c_sumimi_twoinone_fire`.

#### C-SUMIMI-twoinone-02

* **ID:** `C-SUMIMI-twoinone-02`
* **Source:** same
* **Quote:** 「（2）此卡可在你的移动掷骰后作为[反击]使用。」
* **Trigger:** [反击] window after the user's move roll.
* **Condition:** the user just rolled for a main move.
* **Effect:** this card may be declared as a [反击] (its （1） body then
  runs).
* **Rulings & OPEN:** 「移动掷骰后」 is 行动阶段 row 10.
* **Executable:** not separately driven. The window is the post-roll
  [反击] shape shared with every 「移动掷骰后」 counter; (1) is the body
  that runs and is covered by `C-SUMIMI-twoinone-01` / `-01b`.

---

### C-SUMIMI-sweetescape -- Sweet Escape

Card id `Sumimi:Sweet Escape`.

#### C-SUMIMI-sweetescape-01

* **ID:** `C-SUMIMI-sweetescape-01`
* **Source:** `cards.json#Sumimi:Sweet Escape`
* **Quote:** 「回合开始时，若自身前后两格内的地块[收费标价]之和大于等于2000，可打出此卡，」
* **Trigger:** the user's 回合开始时 window; the card is in hand.
* **Condition:** the sum of the tiles within ±2 of the user's tile
  (not the user's own tile) has [收费标价] sum ≥ 2000.
* **Effect:** the play is allowed (gate).
* **Rulings & OPEN:** **OPEN-收费标价** -- undefined in the rulebook body.
  TEST-FINDINGS §6 item 5 records the engine reading as the tile **price**
  sum (tiles ±2 of CiRCLE = 8200 ≥ 2000; base-rent sum = 820). This clause
  uses that ruling. If the book meant base rent the assertion must flip.
* **Executable:** `c_sumimi_sweetescape_gate`.

#### C-SUMIMI-sweetescape-02

* **ID:** `C-SUMIMI-sweetescape-02`
* **Source:** same
* **Quote:** 「传送至“周边精选”，“商店街”或“东京外”对应颜色的除地产商以外任一不属于你的未抵押格子并触发结算，」
* **Trigger:** the [手] body after the gate.
* **Condition:** the gate holds.
* **Effect:** the user teleports to a tile that is
  (a) of the colour of 周边精选 / 商店街 / 东京外,
  (b) not a 地产商 tile,
  (c) not owned by the user,
  (d) not mortgaged,
  and then 触发结算 on that tile.
* **Frame:** the user's `pos`, the settle's money/houses, and the buy that
  follows.
* **Rulings & OPEN:** 「对应颜色」 = the three agent tiles'
  board colours (`#90D5FF`, `#7781xx`, `#A45ACD`). OPEN-收费标价 (gate
  only). OPEN-触发结算.
* **Executable:** `c_sumimi_sweetescape_teleport`.

#### C-SUMIMI-sweetescape-03

* **ID:** `C-SUMIMI-sweetescape-03`
* **Source:** same
* **Quote:** 「若为可购买格子则必须购买，视为你的主要移动。」
* **Trigger:** the same body, after landing.
* **Condition:** the landing tile is a [可购买格子] (rulebook-doc.md:69).
* **Effect:** the user must buy it (no decline); the whole body is the
  user's 主要移动.
* **Rulings & OPEN:** 「可购买格子」 is defined (non-CiRCLE / 咖啡厅 /
  乐器店 / 流星堂 / 地产商).
* **Executable:** `c_sumimi_sweetescape_must_buy`.

---

### C-SUMIMI-twodonuts -- 一人两个甜甜圈

Card id `Sumimi:一人两个甜甜圈`.

#### C-SUMIMI-twodonuts-01

* **ID:** `C-SUMIMI-twodonuts-01`
* **Source:** `cards.json#Sumimi:一人两个甜甜圈`
* **Quote:** 「（1）获得[除外]直至你原本所在格子被其他玩家经过。」
* **Trigger:** the [手] body.
* **Condition:** the user has a 原本所在格子 (the tile they were on).
* **Effect:** the user gains [除外].
* **Frame:** `state:P*:exile`.
* **Rulings & OPEN:** 「直至你原本所在格子被其他玩家经过」 is the expiry
  clause; the base [除外] layer also ticks at turn start
  (rulebook-doc.md:98). Whether the card's expiry is *instead of* the
  turn-start tick is unspecified when the two overlap.
* **Executable:** `c_sumimi_twodonuts_exile`.

#### C-SUMIMI-twodonuts-02

* **ID:** `C-SUMIMI-twodonuts-02`
* **Source:** same
* **Quote:** 「（2）你原本所在格子被其他玩家经过时，可在那名玩家触发结算后选择传送至你原本所在格子（不包括）与那名玩家本次移动终点间的任一格并触发结算，」
* **Trigger:** another player's path includes the user's original tile, after
  that player's 触发结算.
* **Condition:** the user still holds this effect (the [除外] window).
* **Effect:** the user may teleport to any tile strictly between (exclusive)
  the original tile and that player's 移动终点, and 触发结算 there.
* **Rulings & OPEN:** OPEN-触发结算. The optional prompt is a
  counteraction-shaped window.
* **Executable:** not separately driven. Setting up "another player
  [经过]s my original tile and settles" needs a multi-act move fixture the
  card unit does not own; (1)'s [除外] grant is executable and green.

#### C-SUMIMI-twodonuts-03

* **ID:** `C-SUMIMI-twodonuts-03`
* **Source:** same
* **Quote:** 「之后你们各获得2火罐（超出上限的每个火罐转化为500资金）」
* **Trigger:** after the (2) teleport body.
* **Condition:** the body ran.
* **Effect:** the user and the passing player each gain 2 [火罐]; each pot
  beyond the holder's cap becomes 500 money.
* **Frame:** `state:P*:fire` and `money:P*`.
* **Executable:** not separately driven (rides (2), which is not driven).

---

### C-SUMIMI-idolband -- 兼顾偶像与乐队

Card id `Sumimi:兼顾偶像与乐队`.

#### C-SUMIMI-idolband-01

* **ID:** `C-SUMIMI-idolband-01`
* **Source:** `cards.json#Sumimi:兼顾偶像与乐队`
* **Quote:** 「当你本回合未进行过赎回操作时可打出，」
* **Trigger:** play from hand.
* **Condition:** the user has not redeemed this turn.
* **Effect:** the play is allowed.
* **Executable:** `c_sumimi_idolband_gate`.

#### C-SUMIMI-idolband-02

* **ID:** `C-SUMIMI-idolband-02`
* **Source:** same
* **Quote:** 「将你的资金重设为3000。（若你的资金为3000以上则无效果）」
* **Trigger:** the [手] body when the gate holds and money < 3000.
* **Condition:** money < 3000 (the parenthetical).
* **Effect:** money becomes exactly 3000 (set, not gain).
* **Frame:** `money:P{user}` only.
* **Rulings & OPEN:** 「3000以上」 is read exclusive of 3000 (『以上』 in
  the sheet is inclusive of the number in other cards; here the parenthetical
  is the no-effect case at ≥3000). At exactly 3000 there is nothing to set.
* **Executable:** `c_sumimi_idolband_reset`.

#### C-SUMIMI-idolband-frame

* **ID:** `C-SUMIMI-idolband-frame`
* **Source:** `cards.json#Sumimi:兼顾偶像与乐队`
* **Quote:** 「将你的资金重设为3000。（若你的资金为3000以上则无效果）」
* **Frame:** only `money:P{user}` and the card's own zones move.
* **Executable:** `c_sumimi_idolband_frame`.

#### C-SUMIMI-idolband-03

* **ID:** `C-SUMIMI-idolband-03`
* **Source:** same
* **Quote:** 「（若你的资金为3000以上则无效果）」
* **Condition:** money ≥ 3000.
* **Effect:** the card's body does nothing to money (the card is still
  spent / leaves hand as a normal [手] play).
* **Executable:** `c_sumimi_idolband_noop`.

---

### C-SUMIMI-littlehina -- 现在她是Sumimi的小初啦

Card id `Sumimi:现在她是Sumimi的小初啦`.

#### C-SUMIMI-littlehina-01

* **ID:** `C-SUMIMI-littlehina-01`
* **Source:** `cards.json#Sumimi:现在她是Sumimi的小初啦`
* **Quote:** 「[反击] 当你在你的本回合开始后到下回合开始前之间失去资金的总额即将超过你所在格子的[收费标价]时打出此卡，」
* **Trigger:** [反击] when the user's total money loss in the window
  (this turn start → next turn start) is about to exceed the current tile's
  [收费标价].
* **Condition:** the user is on a tile that has a [收费标价]; the pending
  loss would push the running total over that tag.
* **Effect:** the card may be declared.
* **Rulings & OPEN:** **OPEN-收费标价** (same term as Sweet Escape; the
  RiNG parenthetical below argues the tag is a *charge* amount). The window
  is 「本回合开始后到下回合开始前」.
* **Executable:** `c_sumimi_littlehina_offer`.

#### C-SUMIMI-littlehina-02

* **ID:** `C-SUMIMI-littlehina-02`
* **Source:** same
* **Quote:** 「获得相当于你所在格子[收费标价]数额的资金（RiNG则为其基础乘数）」
* **Trigger:** the [反击] body.
* **Condition:** the card was declared.
* **Effect:** the user gains money equal to the tile's [收费标价]; on a RiNG
  tile the amount is its 基础乘数 (`match_rules.json` `ringMultiplier`).
* **Rulings & OPEN:** OPEN-收费标价. 「基础乘数」 is the RiNG rent
  multiplier (`ringMultiplier`, currently 10) per `match_rules.json` note.
* **Executable:** `c_sumimi_littlehina_gain`.

---

### C-SUMIMI-singing -- （真奈）歌唱大赛5连冠

Card id `Sumimi:（真奈）歌唱大赛5连冠`, owner 纯田真奈.

#### C-SUMIMI-singing-01

* **ID:** `C-SUMIMI-singing-01`
* **Source:** `cards.json#Sumimi:（真奈）歌唱大赛5连冠`
* **Quote:** 「[反击]当你或你的格子即将受到来自你以外的效果影响时打出此卡，」
* **Trigger:** [反击] when the user or their tile is about to be affected by
  an effect from outside the user.
* **Condition:** the pending effect is from another player (or a neutral
  source the book counts as 「你以外」).
* **Effect:** this card may be declared.
* **Rulings & OPEN:** 「来自你以外的效果影响」 is broad (any outside
  effect). The contract checks the offer shape only when a clearly outside
  effect is pending.
* **Executable:** `c_sumimi_singing_offer`.

#### C-SUMIMI-singing-02

* **ID:** `C-SUMIMI-singing-02`
* **Source:** same
* **Quote:** 「此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。」
* **Trigger:** while this card's effect is in progress.
* **Condition:** other players hold [反击] cards.
* **Effect:** they may declare [反击] as if *they* were the target; any
  self-directed clause of their counter is redirected to the singing user.
* **Rulings & OPEN:** OPEN-反击 redirect is a substitution of 「你」 in the
  counter body. Not executable as a numeric contract without a second card
  sample; documented and left to the card-path suite.
* **Executable:** not separately driven. The redirect substitutes 「你」
  inside another player's [反击] body (OPEN-C-SUMIMI-singing-redirect);
  a numeric contract needs a second card sample and a multi-counter chain
  harness. The exclusive gate of this card is covered by
  `band_exclusive_only_matching_character`.

#### C-SUMIMI-singing-03

* **ID:** `C-SUMIMI-singing-03`
* **Source:** same
* **Quote:** 「若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。」
* **Trigger:** the end of this card's effect window.
* **Condition:** either another player's counter saved the user, or nobody
  countered.
* **Effect:** the saving player draws 1, or the user draws 1.
* **Frame:** that player's draw pile / hand.
* **Executable:** not separately driven (depends on the (2) redirect
  window). The draw is a plain +1 to one seat once the window closes;
  it rides the same chain as (2).

---

## Character skills

### C-SUMIMI-mana -- 纯田真奈 / 甜甜圈爱好者

#### C-SUMIMI-mana-01

* **ID:** `C-SUMIMI-mana-01`
* **Source:** `characters.json#纯田真奈`
* **Quote:** 「（1）每次[经过]CiRCLE时获得1个[火罐]（初始1，上限2）」
* **Trigger:** [经过] CiRCLE on the user's path (rulebook-doc.md:91).
* **Condition:** the user's fire pot count < cap (2).
* **Effect:** +1 [火罐].
* **Frame:** `state:P*:fire` only.
* **Executable:** `c_sumimi_mana_fire`.

#### C-SUMIMI-mana-02

* **ID:** `C-SUMIMI-mana-02`
* **Source:** same
* **Quote:** 「（2）经过“购物中心”，“便利店”，“快餐店”，“羽泽咖啡厅”，“山吹面包房”时，在触发结算前可选择消耗一个火罐并向对应格子进行一次支付（可向抵押格子正常支付其原本收费，若为无主格子则为消耗对应收费资金），之后，直到你的下个回合结束，你的资金消耗减半。」
* **Trigger:** [经过] one of the five named tiles, before that tile's
  触发结算.
* **Condition:** the user holds ≥1 [火罐].
* **Effect:** the user may spend 1 pot and make one payment to that tile
  (mortgaged: pay its original charge; unowned: consume that charge);
  then until the end of the user's **next** turn, the user's money
  consumption is halved.
* **Frame:** `state:P*:fire`, `money:P*`, the payee's money, and the
  half-window state.
* **Rulings & OPEN:** 「原本收费」 on a mortgaged tile is the charge the
  tile would have collected unmortgaged. The half window is
  「直到你的下个回合结束」 (through the end of the *next* turn, not the
  arming turn's).
* **Executable:** `c_sumimi_mana_shop_pay` / `c_sumimi_mana_half_window`.

---

### C-SUMIMI-hina -- 三角初华（Sumimi） / 成为偶像

#### C-SUMIMI-hina-01

* **ID:** `C-SUMIMI-hina-01`
* **Source:** `characters.json#三角初华（Sumimi）`
* **Quote:** 「（1）每次[经过]CiRCLE时获得1个[火罐]（初始2，上限2）」
* **Trigger:** [经过] CiRCLE.
* **Condition:** fire pots < 2.
* **Effect:** +1 [火罐].
* **Executable:** `c_sumimi_hina_fire`.

#### C-SUMIMI-hina-02

* **ID:** `C-SUMIMI-hina-02`
* **Source:** same
* **Quote:** 「（2）回合开始时可将一个火罐放置在与“主要街道”颜色相同的任一格，其他玩家经过该格且到达移动终点后向你支付X*30资金并移除那个火罐，X为对方经过该格后移动的剩余格数。」
* **Trigger:** (a) the user's 回合开始时: may place 1 pot on any tile of
  「主要街道」's colour; (b) another player passes that tile and reaches
  their 移动终点.
* **Condition:** (a) the user holds ≥1 pot; (b) the marked tile has a pot.
* **Effect:** (a) the pot is placed on the tile (counts against the user's
  cap per the sheet simple text); (b) the passer pays the user `X*30`,
  `X` = steps remaining after passing the marked tile, and the pot is
  removed.
* **Frame:** `state:P*:fire`, `marks` / tile pot, `money:P*`.
* **Rulings & OPEN:** 「主要街道」 colour is the board group/colour of tile
  「主要街道」. 「经过」 does not include the 移动终点 itself
  (rulebook-doc.md:91 -- X must lie on the path, path excludes 起点 and
  includes 终点; a landing *on* the marked tile is a path hit with `X = 0`).
* **Executable:** the placement / collect pair is not separately driven
  (the turn-start placement prompt is not reachable from the arrange
  seam without already being inside 回合开始时). The [经过]CiRCLE pot
  gain (`C-SUMIMI-hina-01`) is executable and green.

---

## Band skill -- 人气偶像组合

Skill id `skill:Sumimi:人气偶像组合`, source `data/bands.json#Sumimi`.

#### C-SUMIMI-band-01

* **ID:** `C-SUMIMI-band-01`
* **Source:** `bands.json#Sumimi`
* **Quote:** 「（1）若你在一回合内从其他玩家处获得过资金，你的回合结束时为此卡添加1个奇迹水晶；否则，移除此卡上的所有奇迹水晶。」
* **Trigger:** the user's 回合结束时.
* **Condition:** whether the user gained money from another player during
  the turn.
* **Effect:** +1 [奇迹水晶] on this skill card, or remove all of them.
* **Frame:** `crystals:P*:skill:Sumimi:人气偶像组合`.
* **Executable:** `c_sumimi_band_crystal`.

#### C-SUMIMI-band-02

* **ID:** `C-SUMIMI-band-02`
* **Source:** same
* **Quote:** 「（2）此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金。」
* **Trigger:** the user collects a charge (rent) from another player.
* **Condition:** the skill card holds `c` crystals.
* **Effect:** the collected amount is raised by `c * 100`.
* **Frame:** `money:P*` of the two sides.
* **Executable:** `c_sumimi_band_rent_bonus`.

#### C-SUMIMI-band-03

* **ID:** `C-SUMIMI-band-03`
* **Source:** same
* **Quote:** 「（3）你可携带两个角色的专属卡牌，但仅有在你的角色卡为对应角色时才可打出。」
* **Trigger:** play of a character exclusive card.
* **Condition:** the user's character card is (or is not) the card's owner.
* **Effect:** both Sumimi exclusives may be *carried* (deck / hand); a play
  is allowed only when the character matches the card's owner.
* **Rulings & OPEN:** exclusive ownership is `cards.json` `owner` /
  `characters.json` `exclusiveCards`.
* **Executable:** `c_sumimi_band_exclusive`.

---

## Open questions

* **OPEN-收费标价** -- `[收费标价]` is undefined in the rulebook body.
  TEST-FINDINGS §6 item 5 records the Sweet Escape gate as the tile **price**
  sum. The RiNG parenthetical of C-SUMIMI-littlehina-02 (「基础乘数」)
  reads more naturally as a *charge/rent* substitute. Sweet Escape uses the
  §6 ruling (price); little hina's exact number stays open and is only
  checked as "the same tag the trigger used".
* **OPEN-C-SUMIMI-nobreakup-digits** -- 「拥有相同数字」 (target of
  Sumimi不会解散哦) is undefined: all digits equal, or any repeated digit?
* **OPEN-触发结算** -- [触发结算] undefined (global).
* **OPEN-场上** -- [场上] undefined as a zone (global). Here 「对方场上」
  is read as that player's field.
* **OPEN-C-SUMIMI-heretheworld-facdown** -- face-down storage of the
  captured card is engine-defined; the clause checks the crystals and the
  hand/draw effect only.
* **OPEN-C-SUMIMI-singing-redirect** -- the [反击] self-redirect
  substitution is not numerically specified.

## Findings

Genuine engine gaps (plain failing tests; no `#[ignore]`).

* **C-SUMIMI-twoinone-01** -- book: 「将自己的角色卡替换为sumimi的另一名角色」
  (the character becomes the other Sumimi character, so the matching
  exclusive is playable). Engine: replaces the bound `skill:<name>:*` field
  entry and sets the fire pots (`C-SUMIMI-twoinone-01b` is green), but
  `player.character` stays the old name and `err.play_exclusive` still keys
  off it -- the other exclusive stays unplayable
  (`twoinone_opens_the_other_exclusive`).
* **C-SUMIMI-band-01** (add half) -- book: 「若你在一回合内从其他玩家处获得过资金，
  你的回合结束时为此卡添加1个奇迹水晶」. Engine: the else-half (clear all
  crystals) runs, but an inbound [支付] from other players (通用:登上武道馆
  / rent) does **not** add a crystal at turn end
  (`band_turn_end_adds_crystal_after_inbound_money`).
* **C-SUMIMI-band-02** -- book: 「此卡上的每个奇迹水晶可使你在向其他人收费时让金额
  额外提高100资金」. Engine: a rent collection pays exactly the table rent;
  crystals on the band skill do not raise the amount
  (`band_rent_bonus_100_per_crystal`).
* **C-SUMIMI-heretheworld-01** -- book: 「（1）[反击] 当有人同一回合内打出两张卡时，
  将此卡放置于对方场上。」 Engine: two [手] plays in one turn never open the
  [反击] window and the card never moves to the trigger player's field
  (`heretheworld_places_on_two_play_turn`).

### OPEN-收费标价 (reading used by the executable)

「[收费标价]」 is undefined in the rulebook body. The executable reading is
the tile's **current charge** (rent at its house count; RiNG: 基础乘数 =
`ringMultiplier` = 10). Evidence:

* Sweet Escape's ±2 sum opens the gate when house rents push the sum over
  2000, and refuses at CiRCLE with base rents (sum 820). A purchase-price
  sum of 8200 at CiRCLE does **not** open the gate -- so the gate is not
  reading `TileData::price`.
* 现在她是Sumimi的小初啦 grants 320 on 水族馆 at 0 houses (= base rent), not
  3200 (= price).
* The RiNG parenthetical 「（RiNG则为其基础乘数）」 names a rent-formula
  component.

TEST-FINDINGS §6 item 5 recorded the Sweet Escape gate as the tile **price**
sum; that reading is contradicted by the black-box behaviour above and by
the RiNG parenthetical (newer sheet text). The clause follows the charge
reading. If the book meant purchase price, flip
`c_sumimi_sweetescape_gate` / `c_sumimi_littlehina_gain` and the
`sweetescape_*` tests.

## Source discrepancy

`data/cards.json` has 11 `Sumimi:` cards. The sheet-derived
`docs/rulebook/cards.json` has 9. The extras `Sumimi:#L11` and
`Sumimi:#L12` are unnamed sheet cells and are **not** in the live sheet
export -- out of scope per the unit brief. If the live sheet later names
them, add clauses under this unit.
