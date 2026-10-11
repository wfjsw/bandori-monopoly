# C-GEN -- 通用 (general cards)

Unit 20 / `c-general` owns this file and `rules/spec/tests/card_general.rs`.
`通用:10次招募（1回限定）` is covered by the shared template
`rules/spec/tests/example_card.rs` (do not edit); its clause is listed here
with that executable.

Sources: live sheet (`docs/rulebook/cards.json`, keys `通用:*`) >
`docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not authoritative.
TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Clause IDs: `C-GEN-<slug>-<n>`. Executables live in
`rules/spec/tests/card_general.rs` unless noted.

---

## C-GEN-recruit-01

* **ID:** `C-GEN-recruit-01`
* **Source:** `cards.json#通用:10次招募（1回限定）`
* **Quote:** 「[手]：[消耗]1500资金，抽1张卡。」
* **Trigger:** the [手] effect, played from hand.
* **Condition:** the user has at least 1500 money (shortfall is a bankruptcy
  path -- a different clause).
* **Effect:** user's money falls by exactly 1500; exactly one card is drawn;
  the played card (non-[持续]) enters the discard.
* **Frame:** only the user's money / hand / draw / discard move.
* **Rulings & OPEN:** 「1回限定」 is not defined in the rulebook body and the
  card carries no [限] tag -- see `OPEN-GEN-recruit-once`. The contract covers
  only the printed [手] body.
* **Executable:** `rules/spec/tests/example_card.rs`
  (`g_general_10draw_money`, `ten_draw_contract`). Clause id there is
  `C-GENERAL-10次招募-01` (the template's own naming); it is the executable for
  this entry.

---

## C-GEN-great-01 / 02 / 03

* **ID:** `C-GEN-great-01`, `C-GEN-great-02`, `C-GEN-great-03`
* **Source:** `cards.json#通用:GREAT`
* **Quote:** 「依次进行以下操作：1. [移除]此卡；2. 将一张“PERFECT“加入抽卡区并洗切；3. [获得]2000资金。」
* **Trigger:** the [手] effect of this card.
* **Condition:** the card is in the user's hand.
* **Effect:**
  1. the card leaves every zone (「[移除]：将卡移除出游戏」);
  2. exactly one `通用:[衍生]PERFECT` is in the user's draw pile (shuffle
     order is not contracted);
  3. user's money rises by exactly 2000.
* **Frame:** `money/hand/draw/discard` of the user only (bookkeeping aside).
* **Rulings & OPEN:** the quote names 「PERFECT」; the live sheet id is
  `通用:[衍生]PERFECT`.
* **Executable:** `c_gen_great_01` / `c_gen_great_02` / `c_gen_great_03` /
  `c_gen_great_frame`; test `great_contract`.

---

## C-GEN-perfect-01 / 02 / 03

* **ID:** `C-GEN-perfect-01`, `C-GEN-perfect-02`, `C-GEN-perfect-03`
* **Source:** `cards.json#通用:[衍生]PERFECT`
* **Quote:** 「依次进行以下操作：1. [移除]此卡；2. 将一张“FEVER!“加入弃卡区；3. [获得]3000资金。」
* **Trigger:** the [手] effect of this card.
* **Condition:** the card is in the user's hand (a [衍生] card -- it cannot be
  in the starting deck, but may be obtained by effect).
* **Effect:**
  1. the card leaves every zone ([移除]);
  2. exactly one `通用:[衍生]FEVER!` enters the user's discard (if the draw
     pile is empty the engine may reshuffle the discard into the draw -- the
     card still *entered* the discard);
  3. user's money rises by exactly 3000.
* **Frame:** `money/hand/draw/discard` of the user only.
* **Rulings & OPEN:** the quote names 「FEVER!」; the live sheet id is
  `通用:[衍生]FEVER!`.
* **Executable:** `c_gen_perfect_01` / `02` / `03` / `c_gen_perfect_frame`;
  test `perfect_contract`.

---

## C-GEN-fever-01 / 02 / 03 / 04

* **ID:** `C-GEN-fever-01`, `C-GEN-fever-02`, `C-GEN-fever-03`, `C-GEN-fever-04`
* **Source:** `cards.json#通用:[衍生]FEVER!`
* **Quote:**
  「[手]：将此卡放置在[使用者]的[场地]。」
  「[持续]：（1）[拥有者]被[支付]或[获得]资金时将金额额外提高X；X为600，[拥有者]场上每拥有一张卡则X降低200（可小于0）。（2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。」
* **Trigger / Condition / Effect:**
  * **01** -- [手] play: a field instance sits on the user's [场地]
    (owner == user == player).
  * **02** -- any [获得] **to** the [拥有者] while the card is on their field:
    with `n` = number of cards on that field (including this instance),
    `X = 600 - 200 * n` (may be < 0); the printed amount is raised by X.
  * **03** -- any [支付] **to** the [拥有者]: the transferred amount is raised
    by X (payer loses and owner gains `amount + X` -- a pure transfer).
  * **04** -- the owner's turn start: the instance leaves the [场地] and one
    `通用:[衍生]FEVER!` enters the [使用者]'s discard.
* **Frame:** the user's money / zones / field (and the counterparty's money on
  a transfer).
* **Rulings & OPEN:** 「场上每拥有一张卡」 counts every field card of the
  owner. 「将金额额外提高X」 on a [支付] is read as raising the payment
  amount, not as a separate printed gain. Owner vs user are the same seat on
  the normal play; if they differ, （2） sends the card to the **user**'s
  discard.
* **Executable:** `c_gen_fever_01` / `c_gen_fever_frame`; tests
  `fever_gain_boost`, `fever_payment_boost`,
  `fever_x_negative_lowers_the_gain`, `fever_leaves_at_owner_turn_start`
  (02/03/04 are multi-step -- the tests are the executables).

---

## C-GEN-tsugu-01 / 02 / 03 / 04

* **ID:** `C-GEN-tsugu-01`, `C-GEN-tsugu-02`, `C-GEN-tsugu-03`, `C-GEN-tsugu-04`
* **Source:** `cards.json#通用:@Tsugu ycm`
* **Quote:** 「投掷3d10并根据结果依次进行以下操作：1. 结果至少为22则抽1张卡；2. 结果至少为26则进进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并不[结算]，且可选择购买任意无主的[可购买格子]；3. 结果至少为28则本回合购买格子时[消耗]资金时降低1500（最低0）；4. 结果小于26则选择[获得]1000资金或进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”并[结算]。」
* **Trigger:** the [手] effect, after the 3d10 roll. The branches are applied
  「依次」 (in order) and may stack (≥28 also implies ≥26 and ≥22).
* **Condition / Effect:**
  * **01** -- `sum ≥ 22`: draw exactly one card.
  * **02** -- `sum ≥ 26`: the user is on 「bandori车站」 (board 「Bandori车站」,
    index 35); that tile did **not** [结算]; an optional buy of any unowned
    [可购买格子] is offered.
  * **03** -- `sum ≥ 28`: this turn, money spent buying a tile is reduced by
    1500, floored at 0.
  * **04** -- `sum < 26`: a binary choice -- (a) [获得]1000资金, or (b)
    teleport to 「bandori车站」 **and** [结算]. Does not stack with (2).
* **Frame:** money / card zones of the user, their position, board
  ownership/houses on a buy, and the turn-long buy modifier.
* **Rulings & OPEN:** the sheet writes 「bandori车站」 (lowercase); the board
  name is 「Bandori车站」. 「不[结算]」 means no ordinary landing settle (the
  card's own buy offer is separate). OPEN-GEN-tsugu-teleport-main (TEST-FINDINGS
  §6.1) records that a teleport-as-main-move's settle shape is still open;
  this contract only checks the printed 不[结算] / 并[结算] outcomes.
* **Executable:** `c_gen_tsugu_01`..`04` / `c_gen_tsugu_frame`; tests
  `tsugu_ge22_draws`, `tsugu_ge26_teleports`, `tsugu_ge28_buy_discount`,
  `tsugu_lt26_choice`.

---

## C-GEN-party-01 / 02 / 03

* **ID:** `C-GEN-party-01`, `C-GEN-party-02`, `C-GEN-party-03`
* **Source:** `cards.json#通用:CiRCLE THANKS PARTY!`
* **Quote:** 「[手]：所有其他玩家可选择[消耗]500资金，你消耗500资金，将X设为因此卡[消耗]资金的玩家数量加1。根据X进行以下操作之一：1. X至少为2则[使用者]投掷Xd20，如果结果至少为35则[使用者][获得]3000资金且其他因此卡[消耗]资金的玩家[获得]1500资金；2. X等于1则[使用者]的本回合结束后获得一个额外回合。」
* **Trigger:** the [手] effect of this card.
* **Condition / Effect:**
  * **01** -- the user always [消耗]s 500; each other player may accept
    (−500) or decline. `X = (others who spent) + 1` (user ruling 2026-10-06:
    the 「加1」 supplies the user; X does not count the user in the base).
  * **02** -- `X ≥ 2`: the user rolls Xd20; on sum **≥ 35** (inclusive) the
    user [获得]s 3000 and every *other* spender [获得]s 1500. Below 35 nobody
    gains.
  * **03** -- `X == 1`: after the user's current turn ends they receive one
    extra turn.
* **Frame:** money of every seat; `extra_turns` on branch 03; the played
  card's zones.
* **Rulings & OPEN:** X definition is the 2026-10-06 ruling in TEST-FINDINGS
  §6 ("X does NOT count the user"). 「至少为35」 is inclusive (sheet A12).
* **Executable:** `c_gen_party_01`..`03` / `c_gen_party_frame`; tests
  `party_x1_extra_turn`, `party_x2_roll_pays`.

---

## C-GEN-budokan-01

* **ID:** `C-GEN-budokan-01`
* **Source:** `cards.json#通用:登上武道馆`
* **Quote:** 「将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，被[指定]的玩家[支付][使用者]X资金。」
* **Trigger:** the [手] effect of this card.
* **Condition:** `alive_others` = number of [存活] players other than the user.
* **Effect:** `X = ceil_to_10(2000 / alive_others)` (2000/3 → 670); every
  other [存活] player [支付]s the user X (a pure transfer).
* **Frame:** only money among the players (plus the played card's zones);
  `targeted` records the [指定] set.
* **Rulings & OPEN:** `Y` is printed but never used by the effect body --
  `OPEN-GEN-budokan-Y`. 「向上取整10」 is read as round **up** to a multiple
  of 10.
* **Executable:** `c_gen_budokan_01` / `c_gen_budokan_frame`; test
  `budokan_contract` (2..=5 players).

---

## C-GEN-marina-01 / exclusive

* **ID:** `C-GEN-marina-01`, `C-GEN-marina-exclusive`
* **Source:** `cards.json#通用:[月岛麻里奈]今天也要加油工作喔` +
  `rulebook-doc.md:112`
* **Quote:**
  「[反击][使用者][经过]#1格子且#1格子受到其他效果影响时：[使用者]本次对#1格子的[经过]或[结算]正常进行而不受到其上的额外效果。」
  「角色专属卡在卡名里会有角色名（例：[丸山彩] 憧憬的前方）。」
* **Trigger / Condition / Effect:**
  * **exclusive** -- the card name carries 「[月岛麻里奈]」; a non-月岛 seat
    cannot play it (`err.play_exclusive`).
  * **01** -- a [反击] window when the user is [经过]ing #1格子 (engine index
    0, CiRCLE) and that tile carries another effect: the [经过] / [结算] of #1
    proceeds normally without the extra effect.
* **Frame:** the counteraction spends the card from hand.
* **Rulings & OPEN:** the contract arranges a [CP点] tile-mark (该清CP了's
  settle clause) as the 「其他效果」. Which effects count as 「受到其他效果
  影响」 is otherwise open -- `OPEN-GEN-marina-extra`.
* **Executable:** `c_gen_marina_01` / `c_gen_marina_exclusive`; tests
  `marina_counters_extra_effect`, `marina_exclusive_refused`.

---

## C-GEN-parking-01 / 02 / 03

* **ID:** `C-GEN-parking-01`, `C-GEN-parking-02`, `C-GEN-parking-03`
* **Source:** `cards.json#通用:[都筑诗船]Parking Space` +
  `rulebook-doc.md:112`
* **Quote:**
  「[手]：将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）。」
  「[持续]：（1）此卡所在格子的[结算]改为回合结束后获得一层[停留]。（2）位于此卡所在格子上的玩家无法使用角色及乐队技能。」
* **Trigger / Condition / Effect:**
  * **exclusive** -- the name carries 「[都筑诗船]」; a non-都筑 seat cannot
    play it.
  * **01** -- the card sits on the Space tile (index 42); houses that were on
    Space move to other tiles the user owns, at most +1 level each.
  * **02** -- a player who [结算]s that tile does not run the ordinary settle;
    at that player's turn end they gain one layer of [停留].
  * **03** -- a player standing on that tile cannot use character or band
    skills (the play is refused).
* **Frame:** the user's zones / field; `houses:*` / `owner:*`; `mark:*` (the
  card also drops a skill-block tile mark); `state:*` for the [停留].
* **Rulings & OPEN:** excess houses beyond the +1-capacity of the user's other
  tiles are unspecified -- `OPEN-GEN-parking-houses`.
* **Executable:** `c_gen_parking_01`..`03` / `c_gen_parking_frame`; tests
  `parking_moves_houses`, `parking_replaces_settle_and_blocks_skills`.

---

## C-GEN-encore-01

* **ID:** `C-GEN-encore-01`
* **Source:** `cards.json#通用:安可` + `rulebook-doc.md:100`
* **Quote:** 「[反击][使用者]即将因任何原因受到[异常移动效果]影响时：无效此次[异常移动效果]和其导致的所有效果。」
* **Trigger:** a [反击] window opened because the user is about to receive an
  [异常移动效果].
* **Condition:** the effect is one of the [异常移动效果] set
  (「包括[传送]，[停留]，[晕眩]，[除外]，[强制移动]，[强制停下]，反方向移动」).
* **Effect:** that effect and everything it would cause are negated for the
  user.
* **Frame:** the counteraction spends the card from hand.
* **Rulings & OPEN:** the contract arranges a [停留] from 「雨啊，快点来吧」
  as the incoming effect.
* **Executable:** `c_gen_encore_01`; test `encore_negates_stay`.

---

## C-GEN-harvest-01

* **ID:** `C-GEN-harvest-01`
* **Source:** `cards.json#通用:尽力后的收获`
* **Quote:** 「[手]：立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。」
* **Trigger:** the [手] effect of this card.
* **Condition:** none beyond the play.
* **Effect:** the user enters the move phase immediately; the [主要移动]
  becomes a move of any integer in 1..=6, and the landing tile [结算]s.
* **Frame:** money / card zones / position of the user; board purchases from
  the settle.
* **Rulings & OPEN:** 「任意整数」 is the player's choice among 1..=6 (six
  options). Wrap / reverse follow the ordinary [路径] rules.
* **Executable:** `c_gen_harvest_01` / `c_gen_harvest_frame`; test
  `harvest_moves_and_settles`.

---

## C-GEN-net-01 / 02

* **ID:** `C-GEN-net-01`, `C-GEN-net-02`
* **Source:** `cards.json#通用:网络链接异常`
* **Quote:** 「[反击]1张手卡的[手]效果或事件卡的效果生效前：根据卡和效果的类型依次进行以下操作：1. 手卡的[手]效果且有[指定]目标则取消其对目标之一的[指定]；2. 手卡的[手]效果且没有[指定]目标则抵消其所有的效果；3. 事件卡的效果手牌则则抵消其所有的效果。」
* **Trigger:** a [反击] window before a hand card's [手] effect (or an event
  card's effect) resolves.
* **Condition / Effect:**
  * **01** -- the target effect designates targets: exactly one designation is
    cancelled (not all).
  * **02** -- the target effect has no [指定]: every effect of that [手] is
    negated.
  * **03** -- event-card branch: the printed text is garbled (「事件卡的效果
    手牌则则」) -- see `OPEN-GEN-net-event`. Not contracted.
* **Frame:** the counteraction spends the card from hand.
* **Rulings & OPEN:** which of several targets is cancelled is unspecified --
  `OPEN-GEN-net-which-target`.
* **Executable:** `c_gen_net_01` / `c_gen_net_02`; tests
  `net_negates_untargeted` (GREAT fully negated),
  `net_cancels_one_target` (武道馆 keeps exactly one payer).

---

## C-GEN-clearcp-01 / 02 / 03 / 04

* **ID:** `C-GEN-clearcp-01`, `C-GEN-clearcp-02`, `C-GEN-clearcp-03`,
  `C-GEN-clearcp-04`
* **Source:** `cards.json#通用:该清CP了` + TEST-FINDINGS §6 (2026-10-07)
* **Quote:**
  「（1）[使用者]使用此卡后的下2回合开始时，此卡在格子上添加的[CP点]及其产物将在相邻的没有[CP点]的格子添加1个[CP点]。」
  「[手]：在任意一个没有角色和[CP点]的格子上添加1个[CP点]并在自己[场上]添加6个[CP点]。在拥有[CP]点的格子上[结算]时移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金。」
* **Trigger / Condition / Effect:**
  * **01** -- [手]: one tile-mark [CP点] on a tile with no character and no
    [CP点]; six on-card [CP点] on this instance (`FieldCard::cp`).
  * **02** -- any player's [结算] on a tile carrying a [CP点]: one tile [CP点]
    and one on-card [CP点] (from the mark's `src` card) are spent; the
    **settler** [获得]s 800.
  * **03** -- each of the next two turn starts after the play: every extant
    mark of this card (「及其产物」) adds one [CP点] on an adjacent tile with
    no [CP点].
  * **04** -- when the on-card count hits 0, the card leaves the [场地] into
    the owner's 弃卡区 (user ruling 2026-10-07).
* **Frame:** money / card zones / field / `mark:*`.
* **Rulings & OPEN:** two kinds of [CP点] (TEST-FINDINGS §6, 2026-10-07):
  tile marks (`category = "cp"`, `owner = -1`, provenance `src`) and on-card
  counters on the instance. 「自己[场上]」 is the card's own count. The
  printed 「移除格子上的个[CP点]」 omits the numeral; read as **1**. The
  settler is the subject of the gain. Which neighbour is chosen when both are
  empty is unspecified -- `OPEN-GEN-clearcp-neighbour`. A [特] gate
  ("cannot be played while the special is pending") is a 2026-10-07 ruling
  not present in the sheet body; not contracted here beyond the sheet text.
* **Executable:** `c_gen_clearcp_01`..`04` / `c_gen_clearcp_frame`; tests
  `clearcp_places_cp`, `clearcp_settle_pays`, `clearcp_spreads`,
  `clearcp_graveyards_at_zero`.

---

## C-GEN-rain-01

* **ID:** `C-GEN-rain-01`
* **Source:** `cards.json#通用:雨啊，快点来吧`
* **Quote:** 「[手]：投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。」
* **Trigger:** the [手] effect of this card.
* **Condition:** X = sum of 2d2 (in 2..=4); the player designates X distinct
  players, and the set must include the user.
* **Effect:** each designated player gains one layer of [停留].
* **Frame:** `state:*` (the [停留] layers) plus the played card's zones;
  `targeted` records the [指定] set.
* **Rulings & OPEN:** the user is a mandatory target (「其中必须包括[使用者]」);
  the remaining `X - 1` are free choices among the other players.
* **Executable:** `c_gen_rain_01` / `c_gen_rain_frame`; test `rain_stays_x`.

---

## Open questions

* **OPEN-GEN-recruit-once** -- 「1回限定」 on `通用:10次招募（1回限定）` is
  not defined in the rulebook body; the card carries no [限] tag. The
  once-per-game gate is unspecified.
* **OPEN-GEN-budokan-Y** -- `Y设为[存活]玩家数量减1` is printed and never
  used by the effect body.
* **OPEN-GEN-parking-houses** -- excess Space houses beyond the +1-capacity
  of the user's other tiles: dropped? left on Space? unspecified.
* **OPEN-GEN-net-event** -- branch 3 「事件卡的效果手牌则则抵消其所有的效果」
  is garbled; the event-card negation shape is unspecified.
* **OPEN-GEN-net-which-target** -- which of several [指定] targets branch 1
  cancels is unspecified.
* **OPEN-GEN-clearcp-neighbour** -- which adjacent tile receives a product
  mark when both neighbours are empty is unspecified.
* **OPEN-GEN-marina-extra** -- which board extras count as 「#1格子受到其他
  效果影响」 beyond a [CP点] mark is unspecified.
* **OPEN-GEN-tsugu-teleport-main** -- (from TEST-FINDINGS §6.1) whether a
  teleport that *is* the main move settles like a main-move landing is still
  an open ruling; this unit only checks the card's printed 不[结算] / 并[结算].
* **OPEN-GEN-tsugu-3-order** -- 「依次进行以下操作」 puts branch 3 (the
  −1500 buy discount) after branch 2's own 「可选择购买」. Whether the
  in-effect buy is already discounted, or only later buys this turn are, is
  unspecified. The contract pins the turn-long modifier on a **later** buy
  and does not lock the step-2 buy's price.
* **OPEN-GEN-clearcp-window** -- 「下2回合开始时」 is either the next two
  chronological turn starts or the user's next two. The contract checks the
  mark count after the spread window (seed + 1 + 2 products = 4) which both
  readings give on a 2-player loop; which two starts fire is not pinned.

## Findings

(Empty -- no engine discrepancy surfaced. All 29 tests are green.

Caveats that are *not* findings: several multi-step clause IDs document a
printed rule whose single-act `Clause` predicate is a no-op stub (the body
lives in the named `#[test]` driver instead -- multi-prompt / multi-turn).
`C-GEN-marina-01`'s suppression of 「其上的额外效果」 is asserted as "the
CiRCLE reward paid and no CP +800 landed"; a richer reading of 「其他效果」
is `OPEN-GEN-marina-extra`.)
