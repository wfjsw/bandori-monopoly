# C-MUJICA -- Ave Mujica (Group Mujica)

Unit 17 owns this file and `rules/tests/tests/spec_card_mujica.rs`. Clause IDs are
`C-MUJICA-<slug>-<n>`.

Sources: live sheet (`docs/rulebook/cards.json` `Mujica:*`, the five
`data/characters.json` rows with `band == "Ave Mujica"`, and the
`data/bands.json` `Ave Mujica` row) > `docs/rulebook/rulebook-doc.md`.
`rulebook.txt` is not authoritative. TEST-FINDINGS §6 rulings unless newer
sheet text supersedes them.

Scope: 14 `Mujica:` cards, 5 character skills, 1 band skill. (The ruleset
also ships `Mujica:#J11`, which is **not** on the live sheet and is out of
scope.)

---

## Cards

### C-MUJICA-人偶的箱庭

* **ID:** `C-MUJICA-人偶的箱庭-01`
* **Source:** `cards.json#Mujica:人偶的箱庭`
* **Quote:** 「使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。”」
* **Trigger:** play of `Mujica:人偶的箱庭` from hand.
* **Condition:** every other [存活] player is offered the choice.
* **Effect:** each other player either walks their dice with no settle, or
  pays the user `X*20` where X is the clockwise tile count from them to the
  user's tile.
* **Frame:** money of the user and the payers / positions of the movers; the
  user's hand / draw / discard for the play itself.
* **Rulings & OPEN:** OPEN-触发结算 -- 「不[触发结算]」 is undefined beyond
  "do not run the landing settle". 「正常移动到你所在格子」 is read as the
  clockwise walk length (the board's only forward).
* **Executable:** `doll_garden_pay_amount` (pay branch).

* **ID:** `C-MUJICA-人偶的箱庭-02`
* **Source:** `cards.json#Mujica:人偶的箱庭`
* **Quote:** 「无法移动的玩家只能选择向你支付。」
* **Condition:** a designated player is [无法移动] (e.g. under [停留] /
  [晕眩] / [除外]).
* **Effect:** that player's prompt has only the pay branch.
* **Frame:** n/a (prompt shape).
* **Rulings & OPEN:** untested -- arranging [无法移动] mid-prompt is a
  multi-step setup the contract suite does not yet drive. Recorded so the
  clause is not lost.
* **Executable:** (none).

* **ID:** `C-MUJICA-人偶的箱庭-03`
* **Source:** `cards.json#Mujica:人偶的箱庭`
* **Quote:** 「然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。」
* **Condition:** the other players have resolved their choices.
* **Effect:** the user force-moves the sum of the others' dice; that walk is
  their [主要移动] for the turn (so it settles, and a later roll is closed).
* **Frame:** the user's `pos`, `turn.main_moved`, settle side-effects.
* **Rulings & OPEN:** if every other player paid (no dice), the sum is 0.
  The quote still says 「视为你本回合的主要移动」 for the sum walk, so a 0-step
  walk might still close the main move -- OPEN-花火额外回合 is a different
  gap; record this as **OPEN-箱庭0步**: does a sum-0 forced walk count as the
  [主要移动] (closing a later roll)? The suite only pins the pay branch.
  OPEN-触发结算 applies to the others' walks, not to this one.
* **Executable:** (partially -- the pay branch pins the sum-0 case in
  `doll_garden_pay_amount`).

---

### C-MUJICA-会被骗着买水晶的人

* **ID:** `C-MUJICA-水晶-01`
* **Source:** `cards.json#Mujica:会被骗着买水晶的人`
* **Quote:** 「将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。」
* **Trigger:** play of `Mujica:会被骗着买水晶的人`.
* **Condition:** a field card holds ≥1 crystal and another field card can
  hold crystals.
* **Effect:** exactly one crystal moves between those two instances.
* **Frame:** `crystals:*` / `field:*` of the two holders; the user's zones.
* **Rulings & OPEN:** 「可以放置奇迹水晶的卡」 is not defined as a class;
  this contract uses two cards the engine already treats as crystal-bearing.
* **Executable:** `crystal_swap_moves_one`.

---

### C-MUJICA-心の雨

* **ID:** `C-MUJICA-心の雨-01`
* **Source:** `cards.json#Mujica:心の雨`
* **Quote:** 「使你前方20格内的所有玩家各自骰1d20」
* **Trigger:** play of `Mujica:心の雨`.
* **Condition:** other players on the 20 tiles clockwise ahead of the user.
* **Effect:** each such player rolls 1d20. Players outside the window are
  untouched.
* **Frame:** `state:stay` of the in-range players (and the fail branch below).
* **Rulings & OPEN:** 「前方20格」 is the 20 tiles clockwise ahead (the
  board's only forward); the user's own tile is not 「前方」.
* **Executable:** `rain_stay_layers`, `rain_contract`.

* **ID:** `C-MUJICA-心の雨-02`
* **Source:** `cards.json#Mujica:心の雨`
* **Quote:** 「骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。」
* **Condition:** an in-range player's d20 result.
* **Effect:** roll ≤ 12 → +1 [停留]; roll ≤ 3 → **+2** (the 2 replaces the 1,
  it does not stack on top of it).
* **Frame:** only that player's `stay` counter.
* **Rulings & OPEN:** 「变为」 is read as "the gain is 2 layers, not 1+2".
* **Executable:** `rain_stay_amount`, `rain_contract`.

* **ID:** `C-MUJICA-心の雨-03`
* **Source:** `cards.json#Mujica:心の雨`
* **Quote:** 「若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金」
* **Condition:** after the rolls, no player gained any [停留] layer.
* **Effect:** the user gains 1 [晕眩] and [获得] 1000.
* **Frame:** the user's `stun` / `money`.
* **Rulings & OPEN:** the quote uses [晕眩]; the rulebook's status name is
  [晕眩] (`rulebook-doc.md:97`) and 「眩晕」 appears in the 花火 text for the
  same counter.
* **Executable:** `rain_miss_self_stun`, `rain_contract`.

---

### C-MUJICA-无法将视线移开

* **ID:** `C-MUJICA-视线-01`
* **Source:** `cards.json#Mujica:无法将视线移开`
* **Quote:** 「使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算]」
* **Trigger:** play of `Mujica:无法将视线移开` (as [手], or as [反击] -- see
  clause 03).
* **Condition:** the set of players who played a [反击] against the user
  earlier this turn. An empty set designates nobody.
* **Effect:** each set member force-moves 1–4 tiles in the user's chosen
  direction, and settles (OPEN-触发结算).
* **Frame:** the targets' `pos` / settle side-effects.
* **Rulings & OPEN:** OPEN-触发结算. The target set is exactly 「当前回合内
  对你打出过[反击]的所有玩家」 -- not "every other player".
* **Executable:** `look_away_only_moves_counteractors` (passing, but only its
  empty-set case: a normal hand play currently moves nobody at all -- see
  Findings / ABI-03).

* **ID:** `C-MUJICA-视线-02`
* **Source:** `cards.json#Mujica:无法将视线移开`
* **Quote:** 「若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动」
* **Condition:** the user is among the force-moved and it is the user's turn.
* **Effect:** that forced walk is the user's [主要移动].
* **Frame:** `turn.main_moved`.
* **Rulings & OPEN:** only reachable once clause 01's target set is honoured
  (the user is not in 「对你打出过[反击]」 unless they counteracted
  themselves). Not separately tested.
* **Executable:** (none).

* **ID:** `C-MUJICA-视线-03`
* **Source:** `cards.json#Mujica:无法将视线移开`
* **Quote:** 「（此卡可作为[反击]在有玩家对你使用[反击]后立即使用）」
* **Trigger:** a [反击] window that opens after another player's [反击]
  against the user.
* **Condition:** the card is in the user's hand and the window offers it.
* **Effect:** it may be played in that window (it is a [反击], so it resolves
  before the original X).
* **Frame:** counteraction bookkeeping.
* **Rulings & OPEN:** untested -- the harness's `counteract_offered` needs a
  counterable X with this card in hand; the suite has not yet driven that
  two-step window.
* **Executable:** (none).

---

### C-MUJICA-欢迎来到ave mujica的世界

* **ID:** `C-MUJICA-欢迎-01`
* **Source:** `cards.json#Mujica:欢迎来到ave mujica的世界`
* **Quote:** 「选择以下效果其一发动：」
* **Trigger:** play of the card.
* **Effect:** exactly one of （1） / （2） runs.
* **Frame:** whichever branch's paths only.
* **Rulings & OPEN:** the (2) branch is a [反击] (see clause 03); the (1)
  branch is the default [手].
* **Executable:** `welcome_switches_designated_state` (covers (1)).

* **ID:** `C-MUJICA-欢迎-02`
* **Source:** `cards.json#Mujica:欢迎来到ave mujica的世界`
* **Quote:** 「（1）转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）」
* **Condition:** a designated player who has 状态2 available (an Ave Mujica
  character under the band skill).
* **Effect:** that player's state toggles (状态1 ↔ 状态2). Designating a
  player without 状态2 does nothing.
* **Frame:** the designated player's `skillState`.
* **Rulings & OPEN:** 「状态」 here is the band skill's 状态1 / 状态2 pair.
  「任意一名玩家」 includes the user; 「不存在状态2的玩家则无效果」 is a
  required no-op. Both gaps are Findings (below).
* **Executable:** `welcome_switches_designated_state` (happy path);
  `welcome_offers_self_as_well`, `welcome_no_state2_is_noop` (**failing**;
  see Findings).

* **ID:** `C-MUJICA-欢迎-03`
* **Source:** `cards.json#Mujica:欢迎来到ave mujica的世界`
* **Quote:** 「（2）[反击] 当有其他玩家切换状态时，你与所有本回合切换了状态的玩家同时切换一次状态」
* **Trigger:** another player switches 状态, as a [反击] window.
* **Effect:** the user and every player who switched state this turn each
  switch once, simultaneously.
* **Frame:** `skillState` of those players.
* **Rulings & OPEN:** untested -- needs a [反击] window on a state switch.
* **Executable:** (none).

---

### C-MUJICA-燃尽前的线香花火

* **ID:** `C-MUJICA-花火-01`
* **Source:** `cards.json#Mujica:燃尽前的线香花火`
* **Quote:** 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
* **Trigger:** play of the card.
* **Effect:** the card is placed on the user's field with 2 crystals (cap 2).
* **Frame:** `field:P<user>` / `crystals:P<user>`.
* **Executable:** `sparkler_places_two_crystals`.

* **ID:** `C-MUJICA-花火-02`
* **Source:** `cards.json#Mujica:燃尽前的线香花火`
* **Quote:** 「你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合」
* **Trigger:** the user's turn ends while the card is on the field.
* **Effect:** one crystal is removed and an extra turn is granted.
* **Frame:** the card's crystal count; `extra_turns`.
* **Rulings & OPEN:** OPEN-花火额外回合 (TEST-FINDINGS §6 item 2) -- does the
  **last** crystal also grant the extra turn? The quote attaches the extra
  turn to every removal and the stun to the last; this suite checks the first
  removal's crystal drop and does not pin the last's extra turn.
* **Executable:** `sparkler_burn_and_last_stun` (first half).

* **ID:** `C-MUJICA-花火-03`
* **Source:** `cards.json#Mujica:燃尽前的线香花火`
* **Quote:** 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。」
* **Trigger:** the crystal count reaches 0.
* **Effect:** the card leaves play for the discard (or the reshuffled draw)
  and the user gains **2** [眩晕] layers, immediately.
* **Frame:** the card's zones; the user's `stun`.
* **Rulings & OPEN:** OPEN-花火眩晕掉层 -- if the same turn-end also ticks a
  [晕眩] layer (rulebook 「每回合结束时移除一层」), does the tick run before
  or after 「立刻获得2层」? The quote's 「立刻」 asks for 2 visible after the
  effect; this contract asserts 2 and stays red while the engine leaves 1.
* **Executable:** `sparkler_burn_and_last_stun` (**failing**; see Findings).

---

### C-MUJICA-祥，移动

* **ID:** `C-MUJICA-祥移动-01`
* **Source:** `cards.json#Mujica:祥，移动`
* **Quote:** 「强制一名玩家向你选择的方向移动3格并[触发结算]」
* **Trigger:** play of `Mujica:祥，移动`.
* **Condition:** a designated player (the prompt offers every player,
  including the user -- see clause 02).
* **Effect:** that player walks exactly 3 tiles in the chosen direction and
  settles.
* **Frame:** the target's `pos` and settle side-effects (rent / buy offer).
* **Rulings & OPEN:** OPEN-触发结算. 「移动3格」 is a fixed 3-tile walk, not a
  roll (the engine's `log.move_forward` confirms the shape, but the contract
  asserts the book's 3).
* **Executable:** `saki_move_forces_three_tiles`.

* **ID:** `C-MUJICA-祥移动-02`
* **Source:** `cards.json#Mujica:祥，移动`
* **Quote:** 「（可在掷骰前选择自己以代替主要移动）」
* **Condition:** the play happens before the user's main-move roll.
* **Effect:** the user may designate themself; that 3-tile walk replaces the
  [主要移动].
* **Frame:** `turn.main_moved` / `pos:P<user>`.
* **Rulings & OPEN:** untested as a main-move replacement (the suite plays
  the card from 运营 and does not assert `main_moved`).
* **Executable:** (none).

* **ID:** `C-MUJICA-祥移动-03`
* **Source:** `cards.json#Mujica:祥，移动`
* **Quote:** 「触发结算时进行的支付价格减半」
* **Condition:** the forced walk settles on a tile that causes a payment.
* **Effect:** that payment's price is halved.
* **Frame:** the payer's and payee's money.
* **Rulings & OPEN:** TEST-FINDINGS §6: 「支付价格」 covers shaped payments.
  Odd halves are OPEN-取整 (this contract uses an even rent, 320 → 160).
* **Executable:** `saki_move_halves_settle_pay`.

---

### C-MUJICA-骰子已经掷下

* **ID:** `C-MUJICA-骰子-01`
* **Source:** `cards.json#Mujica:骰子已经掷下`
* **Quote:** 「将此卡放置于自身场上」
* **Effect:** the card is placed on the user's field.
* **Frame:** `field:P<user>`.
* **Executable:** `dice_cast_places_and_goes_to_discard`.

* **ID:** `C-MUJICA-骰子-02`
* **Source:** `cards.json#Mujica:骰子已经掷下`
* **Quote:** 「本回合内所有其他玩家无法从手牌中使用[反击]」
* **Condition:** the card is on the field, this turn.
* **Effect:** other players cannot play a [反击] from hand.
* **Frame:** counteraction windows.
* **Rulings & OPEN:** untested -- needs a [反击] window while the card is up.
* **Executable:** (none).

* **ID:** `C-MUJICA-骰子-03`
* **Source:** `cards.json#Mujica:骰子已经掷下`
* **Quote:** 「回合结束后放入弃牌堆」
* **Effect:** at the user's turn end the card leaves the field for the discard.
* **Frame:** `field` / `discard` / `draw` of the user.
* **Executable:** `dice_cast_places_and_goes_to_discard`.

* **ID:** `C-MUJICA-骰子-04`
* **Source:** `cards.json#Mujica:骰子已经掷下`
* **Quote:** 「（此卡可以被反击）」
* **Effect:** the play opens a [反击] window.
* **Rulings & OPEN:** untested (window shape).
* **Executable:** (none).

---

### C-MUJICA-黑色生日

* **ID:** `C-MUJICA-黑色生日-01`
* **Source:** `cards.json#Mujica:黑色生日`
* **Quote:** 「每名你以外的资金不多于1000的玩家支付你800资金」
* **Condition:** another player's money `<= 1000` (inclusive) and they can
  pay 800.
* **Effect:** they pay the user exactly 800. A shortfall is not a skip: the
  book's 「支付你800」 still fires and G-BANKRUPT covers the bankruptcy. This
  clause pins solvent payers only.
* **Frame:** money of the user and the payers.
* **Rulings & OPEN:** TEST-FINDINGS §6 sheet-superseded: 「不多于1000」 is
  inclusive.
* **Executable:** `black_bday_low_pays_800`, `black_birthday_contract`.

* **ID:** `C-MUJICA-黑色生日-02`
* **Source:** `cards.json#Mujica:黑色生日`
* **Quote:** 「每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
* **Condition:** another player's money `> 1000` and they can pay 400.
* **Effect:** they pay 200 twice (total 400).
* **Frame:** as above.
* **Executable:** `black_bday_high_pays_400`, `black_birthday_contract`.

---

### C-MUJICA-（初华）我，无畏悲伤

*Exclusive to 三角初华 (`card.owner`).*

* **ID:** `C-MUJICA-初华-01`
* **Source:** `cards.json#Mujica:（初华）我，无畏悲伤`
* **Quote:** 「打出此卡时若自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]，你可选择抽1张卡或使你本回合的移动掷骰结果可定义为1-6以内的任何数字」
* **Condition:** since the last [经过]CiRCLE the user settled at least one
  [回忆地块] (the skill quote names 小豆岛 / 武道馆 / 旧古河庭园).
* **Effect:** the user chooses draw 1, or set this turn's roll to any 1–6.
* **Frame:** the user's hand / draw, or `turn.fixed_roll`.
* **Rulings & OPEN:** with the gate closed the card resolves inertly (no
  choice). The gate itself needs a settle history the arrange cannot fake;
  only the closed path is tested.
* **Executable:** `uika_no_memory_offers_no_choice`.

* **ID:** `C-MUJICA-初华-02`
* **Source:** `cards.json#Mujica:（初华）我，无畏悲伤`
* **Quote:** 「你可选择抽1张卡或使你本回合的移动掷骰结果可定义为1-6以内的任何数字」
* **Effect:** the choice's two branches (draw 1, or define the roll 1–6).
* **Rulings & OPEN:** untested (gate closed in the suite).
* **Executable:** (none).

---

### C-MUJICA-（喵梦）

*Exclusive to 祐天寺若麦.*

* **ID:** `C-MUJICA-喵梦-01`
* **Source:** `cards.json#Mujica:（喵梦）`
* **Quote:** 「（1）将此卡放置于场上」
* **Effect:** the card is placed on the user's field.
* **Frame:** `field:P<user>`.
* **Executable:** `nyamu_places_on_field`.

* **ID:** `C-MUJICA-喵梦-02`
* **Source:** `cards.json#Mujica:（喵梦）`
* **Quote:** 「1. 每当此卡将从正面翻至背面时可选择失去2个火罐。」
* **Trigger:** a flip of this instance face-down (e.g. 祐天寺若麦 状态2 entry).
* **Effect:** the owner may lose 2 fire pots (optional). The quote does not
  name a benefit -- **OPEN-喵梦翻面**: does paying the pots cancel the flip?
* **Frame:** the owner's `fire`.
* **Rulings & OPEN:** untested -- the flip is driven by another skill's
  state-2 entry, a multi-skill arrange the suite does not yet run.
* **Executable:** (none).

* **ID:** `C-MUJICA-喵梦-03`
* **Source:** `cards.json#Mujica:（喵梦）`
* **Quote:** 「2. 每当此卡将从背面翻至正面时此卡拥有者抽一张卡。」
* **Trigger:** a flip of this instance face-up (e.g. 状态2 exit).
* **Effect:** the owner draws 1 card.
* **Frame:** the owner's hand / draw.
* **Rulings & OPEN:** untested (same as 02).
* **Executable:** (none).

---

### C-MUJICA-（海铃）

*Exclusive to 八幡海铃.*

* **ID:** `C-MUJICA-海铃-01`
* **Source:** `cards.json#Mujica:（海铃）`
* **Quote:** 「（1）将此卡放置于在此卡使用者下一名行动的玩家场上」
* **Effect:** the card is placed on the next actor's field, not the user's.
* **Frame:** `field:*`.
* **Executable:** `umiri_places_on_next_actors_field`.

* **ID:** `C-MUJICA-海铃-02`
* **Source:** `cards.json#Mujica:（海铃）`
* **Quote:** 「轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。」
* **Trigger:** the user's turn start.
* **Effect:** the card slides to the next actor after its current holder.
* **Frame:** `field:*`.
* **Rulings & OPEN:** untested (needs a turn-start hop).
* **Executable:** (none).

* **ID:** `C-MUJICA-海铃-03`
* **Source:** `cards.json#Mujica:（海铃）`
* **Quote:** 「使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。」
* **Trigger:** the play, and the user's turn start.
* **Condition:** the current holder has band skill cards.
* **Effect:** the user takes those band skills; duplicates do not stack; the
  user does not count as a member of that band.
* **Frame:** `field:P<user>` (skill instances).
* **Executable:** `umiri_takes_holders_band_skill`.

* **ID:** `C-MUJICA-海铃-04`
* **Source:** `cards.json#Mujica:（海铃）`
* **Quote:** 「（相同乐队技能卡的效果不可叠加）」
* **Effect:** two copies of one band skill do not double its effect.
* **Rulings & OPEN:** untested as a stacking assertion.
* **Executable:** (none).

* **ID:** `C-MUJICA-海铃-05`
* **Source:** `cards.json#Mujica:（海铃）`
* **Quote:** 「（3）当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。」
* **Trigger:** the card returns to the user's field; at the user's turn end.
* **Effect:** this card and every taken band skill go to the discard; the
  user draws 1.
* **Frame:** `field` / `discard` / `draw` of the user.
* **Rulings & OPEN:** untested (needs the return hop).
* **Executable:** (none).

---

### C-MUJICA-（睦/mortis）表演的本能

*Exclusive to 若叶睦.*

* **ID:** `C-MUJICA-睦-01`
* **Source:** `cards.json#Mujica:（睦/mortis）表演的本能`
* **Quote:** 「此卡打出时效果为场上其他玩家打出的上一张卡（不计入效果带有放置于场上的卡）」
* **Condition:** another player's last played card exists and is not a
  place-on-field card.
* **Effect:** this play runs that card's effect (as a copy).
* **Frame:** whatever the copied card's own effect touches.
* **Rulings & OPEN:** with no prior copyable card the play is inert. The
  exclusion 「不计入效果带有放置于场上的卡」 is not separately tested.
* **Executable:** `mortis_nothing_to_copy_is_inert`.

* **ID:** `C-MUJICA-睦-02`
* **Source:** `cards.json#Mujica:（睦/mortis）表演的本能`
* **Quote:** 「（此卡复制[反击]卡时可在符合条件时打出）」
* **Effect:** when the copied card is a [反击], this card may be played in
  that [反击]'s timing window.
* **Rulings & OPEN:** untested (window).
* **Executable:** (none).

---

### C-MUJICA-（祥子）斩断留恋，忘却一切

*Exclusive to 丰川祥子.*

* **ID:** `C-MUJICA-祥子-01`
* **Source:** `cards.json#Mujica:（祥子）斩断留恋，忘却一切`
* **Quote:** 「抵押一张你拥有且未抵押的最贵地契」
* **Condition:** the user owns at least one un-mortgaged deed.
* **Effect:** the most expensive such deed (by printed purchase price) is
  mortgaged.
* **Frame:** `mortgaged:T<k>` and the mortgage's 50% money.
* **Rulings & OPEN:** 「最贵」 is read as the deed's printed price (tile 3
  price 3200 beats tile 1 price 3000 even with houses on tile 1).
* **Executable:** `sakiko_mortgages_most_expensive_and_teleports`.

* **ID:** `C-MUJICA-祥子-02`
* **Source:** `cards.json#Mujica:（祥子）斩断留恋，忘却一切`
* **Quote:** 「并弃置一张手牌（若无手牌则弃掉下一张抽到的牌）」
* **Effect:** one hand card is discarded; with an empty hand the next drawn
  card is discarded instead.
* **Frame:** the user's hand / discard / draw.
* **Executable:** `sakiko_mortgages_most_expensive_and_teleports` (hand
  branch).

* **ID:** `C-MUJICA-祥子-03`
* **Source:** `cards.json#Mujica:（祥子）斩断留恋，忘却一切`
* **Quote:** 「立刻传送至任意可购买或已拥有的格子并触发结算，作为你的主要移动。」
* **Effect:** the user [传送]s to any [可购买格子] or owned tile, settles,
  and that walk is their [主要移动].
* **Frame:** the user's `pos`, settle side-effects, `turn.main_moved`.
* **Rulings & OPEN:** OPEN-触发结算.
* **Executable:** `sakiko_mortgages_most_expensive_and_teleports`.

* **ID:** `C-MUJICA-祥子-04`
* **Source:** `cards.json#Mujica:（祥子）斩断留恋，忘却一切`
* **Quote:** 「若直接从抽牌堆打出，可不弃置手牌发动，或选择不发动此卡」
* **Condition:** the card is played straight from the draw pile.
* **Effect:** the user may skip the discard, or decline the card entirely.
* **Rulings & OPEN:** untested (draw-pile play path).
* **Executable:** (none).

---

## Character skills

### C-MUJICA-初华技能 -- 三角初华 / Imprisoned XII

* **ID:** `C-MUJICA-初华技能-01`
* **Source:** `characters.json#三角初华`
* **Quote:** 「状态1：移动改为10+1d10」
* **Condition:** the user is in 状态1.
* **Effect:** the main move is `10 + 1d10`, not 1d20.
* **Frame:** the user's `pos`.
* **Executable:** `uika_state1_roll_is_10_plus_d10`.

* **ID:** `C-MUJICA-初华技能-02`
* **Source:** `characters.json#三角初华`
* **Quote:** 「每当你经过CiRCLE的回合结束后，若你自从上一次经过CiRCLE后未在任何[回忆地块]（小豆岛，武道馆，旧古河庭园）触发结算则可在你的下回合开始时进入状态2。若满足前述条件，你可在你经过CiRCLE的回合结束后立刻获得一个新的回合。」
* **Trigger:** a turn in which the user [经过]CiRCLE ends.
* **Condition:** no [回忆地块] settle since the previous CiRCLE pass.
* **Effect:** the user may enter 状态2 at their next turn start, and may take
  an extra turn immediately.
* **Rulings & OPEN:** untested (multi-turn history).
* **Executable:** (none).

* **ID:** `C-MUJICA-初华技能-03`
* **Source:** `characters.json#三角初华`
* **Quote:** 「状态2：（火罐上限3）获得[不可阻挡]，并且在此状态下主动移动经过任何玩家都将向其收取200资金，每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）」
* **Condition:** 状态2.
* **Effect:** [不可阻挡]; on an active move, every player passed pays 200;
  after each successful collect the user may walk 1d6 more (≤4 times), before
  the landing settle.
* **Rulings & OPEN:** OPEN-触发结算. Untested (multi-walk).
* **Executable:** none. `uika_fire_cap_is_three` only checks the arrange
  setter (it does **not** read the skill's declared cap).

### C-MUJICA-睦技能 -- 若叶睦 / 天生的演员

* **ID:** `C-MUJICA-睦技能-01`
* **Source:** `characters.json#若叶睦`
* **Quote:** 「状态1：你的每6回合不打出任何手牌，在回合开始前抽1张牌。若你的回合开始时手牌数为5，可选择进入状态2。」
* **Condition:** 状态1; six turns with no hand play (for the draw); hand
  size 5 at turn start (for the state-2 gate).
* **Effect:** draw 1 before the turn start; optionally enter 状态2.
* **Rulings & OPEN:** the rulebook-doc line 「手牌数4或以上」 is superseded by
  the live sheet's 「手牌数为5」. Untested (six-turn counter).
* **Executable:** (none).

* **ID:** `C-MUJICA-睦技能-02`
* **Source:** `characters.json#若叶睦`
* **Quote:** 「状态2：进入该状态后，你从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖于自己角色卡上并获得其初始火罐，直至退出状态2；若回合结束时你的手牌小于等于1，退出状态2。」
* **Effect:** overlay the last hand-player's character card (and its starting
  fire pots); exit 状态2 at turn end with ≤1 card in hand.
* **Rulings & OPEN:** untested (overlay machinery).
* **Executable:** (none).

### C-MUJICA-祥子技能 -- 丰川祥子 / 请把你们的人生交给我

* **ID:** `C-MUJICA-祥子技能-01`
* **Source:** `characters.json#丰川祥子`
* **Quote:** 「状态1： 经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上（仍正常完成本次移动），若如此做，每获得一层停留，眩晕，你获得1500资金。」
* **Effect:** optional transfer of all [停留]/[晕眩] layers from a passed
  player; the walk completes; +1500 per layer taken.
* **Executable:** (none -- pass-hook).

* **ID:** `C-MUJICA-祥子技能-02`
* **Source:** `characters.json#丰川祥子`
* **Quote:** 「每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐（初始0，上限3），火罐数达到上限时可在回合开始时选择进入状态2。」
* **Effect:** +1 fire pot per applied [停留]/[晕眩]/[除外] (cap 3); at cap,
  状态2 is offered at turn start.
* **Executable:** none. `sakiko_fire_cap_is_three` only checks the arrange
  setter (it does **not** read the skill's declared cap).

* **ID:** `C-MUJICA-祥子技能-03`
* **Source:** `characters.json#丰川祥子`
* **Quote:** 「状态2：保留至多一张手牌并将其余置入弃牌堆。每回合开始时自动打出一张抽牌堆顶端的牌。」
* **Effect:** on 状态2 entry keep ≤1 hand card; each turn start auto-plays
  the top of the draw pile.
* **Executable:** (none).

### C-MUJICA-海铃技能 -- 八幡海铃 / 熟练的支援贝斯手

* **ID:** `C-MUJICA-海铃技能-01`
* **Source:** `characters.json#八幡海铃`
* **Quote:** 「（1）状态1：其他玩家可以在他们的回合开始时向你支付400资金指定一个属于你的格子，使其立即对前后一格内的一名玩家进行一次半价[结算]，并获得相当于你获取数额的资金；直到这两名玩家的下回合结束，若你仍在状态1，他们不会被该效果再次收款。」
* **Effect:** another player pays 400 to half-settle one of the user's tiles
  against a neighbour; that player also receives what the user collected;
  a per-pair cooldown lasts until both players' next turn end.
* **Rulings & OPEN:** the half rate is the [地产商] 「向上取整10」 style (G-SETTLE).
* **Executable:** (none -- multi-player interrupt).

* **ID:** `C-MUJICA-海铃技能-02`
* **Source:** `characters.json#八幡海铃`
* **Quote:** 「每两个回合没有玩家发动该技能，你在回合开始时获得1火罐（初始0，上限4），若达到上限，可在你的回合开始时选择进入状态2。」
* **Effect:** +1 fire pot per two idle turns (cap 4); at cap, 状态2 offered.
* **Executable:** none. `umiri_fire_cap_is_four` only checks the arrange
  setter (it does **not** read the skill's declared cap).

* **ID:** `C-MUJICA-海铃技能-03`
* **Source:** `characters.json#八幡海铃`
* **Quote:** 「（2） 状态2： 进入状态2时你可消耗X个火罐，指定其他人的X个地契，每张地契向对应玩家支付100+n*100资金（n为对应格子上的房屋数），直到状态2结束为止，你与对方均分那些地契收取的资金；每通过此法消耗4个火罐，你抽1张卡。」
* **Effect:** spend X fire pots to designate X foreign deeds; pay each owner
  `100 + n*100`; while 状态2 lasts, split those tiles' rent 50/50; draw 1 per
  4 pots spent.
* **Rulings & OPEN:** OPEN-分摊 (「均分」) -- the split ratio is clear (half
  each) but the engine's 分摊 vocabulary is undefined in the book. Untested.
* **Executable:** (none).

### C-MUJICA-若麦技能 -- 祐天寺若麦 / 大主播喵梦亲

* **ID:** `C-MUJICA-若麦技能-01`
* **Source:** `characters.json#祐天寺若麦`
* **Quote:** 「状态1：（1）每次其他玩家向你支付资金时，你立即获得1火罐（初始0，上限5），并使那次支付的金额提高X*100，X为你拥有的火罐数。」
* **Effect:** on every payment to the user, +1 fire pot, then the payment is
  raised by `X*100` with X the new pot count.
* **Executable:** none. `nyamu_fire_cap_is_five` only checks the arrange
  setter (it does **not** read the skill's declared cap).

* **ID:** `C-MUJICA-若麦技能-02`
* **Source:** `characters.json#祐天寺若麦`
* **Quote:** 「（2）任何时刻当你的火罐数到达上限时，你立即进入状态2。」
* **Effect:** hitting 5 pots enters 状态2 immediately.
* **Executable:** (none).

* **ID:** `C-MUJICA-若麦技能-03`
* **Source:** `characters.json#祐天寺若麦`
* **Quote:** 「状态2：（1）进入状态2时，你立即将场上全部正面朝上的手卡及事件卡翻为背面朝上，直到状态2结束为止，场上所有背面朝上的卡无法产生效果，且不受任何效果影响。你退出状态2后，被你翻面的卡自动翻回。」
* **Effect:** on 状态2 entry flip every face-up hand/event card face-down;
  face-down cards are inert and immune; on exit flip them back.
* **Executable:** (none -- flip machinery).

---

## Band skill -- C-MUJICA-团技能 (Ave Mujica / 假面之下的真实)

* **ID:** `C-MUJICA-团技能-00`
* **Source:** `bands.json#Ave Mujica`
* **Quote:** 「你拥有“状态1”和“状态2”两种状态，初始处于状态1。」
* **Effect:** the pair exists; the player starts in 状态1.
* **Executable:** `band_starts_in_state1`.

* **ID:** `C-MUJICA-团技能-01`
* **Source:** `bands.json#Ave Mujica`
* **Quote:** 「你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己结算，对应的其他玩家收支不变）」
* **Condition:** the user is in 状态2 and is one side of a [收取]/[支付].
* **Effect:** the user's own money movement is 1.5x; the other player's
  movement is the unhalfed printed amount.
* **Frame:** money of the two sides.
* **Rulings & OPEN:** the live sheet says **1.5倍**; `rulebook-doc.md:232`
  「翻倍」 is superseded. The parenthetical is the whole point: a symmetric
  1.5x transfer breaks the quote. Only the user-as-payer half is tested; the
  user-as-payee (「[收取]」) half is the same arithmetic mirrored.
* **Executable:** `band_state2_pay_is_150_only_on_self` (**failing**; see
  Findings).

* **ID:** `C-MUJICA-团技能-02`
* **Source:** `bands.json#Ave Mujica`
* **Quote:** 「处于状态2的第三回合开始时为此卡添加一个奇迹水晶。」
* **Effect:** at the third turn start while in 状态2, the band skill gains 1
  crystal.
* **Executable:** (none -- three-turn counter).

* **ID:** `C-MUJICA-团技能-03`
* **Source:** `bands.json#Ave Mujica`
* **Quote:** 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半。」
* **Effect:** optional: spend 1 crystal to halve one payment.
* **Executable:** (none -- optional interrupt; no harness seam).

* **ID:** `C-MUJICA-团技能-04`
* **Source:** `bands.json#Ave Mujica`
* **Quote:** 「若你的技能拥有火罐，进入状态2时将火罐补充至上限且每回合结束时失去1火罐，回合结束后若火罐为0则退出状态2；退出状态2时，失去所有剩余火罐。」
* **Effect:** on 状态2 entry pots refill to cap; each turn end −1 pot; at 0
  pots leave 状态2 (and any remainder is lost on exit).
* **Executable:** (none -- multi-turn).

---

## Open questions

* **OPEN-触发结算** -- undefined in the book (README index). Clauses that
  only say 「并[触发结算]」 / 「不[触发结算]」 specify the walk and leave the
  settle window to G-SETTLE.
* **OPEN-花火额外回合** (TEST-FINDINGS §6 item 2) -- does the **last** crystal
  of 燃尽前的线香花火 also grant 「一个额外回合」?
* **OPEN-花火眩晕掉层** -- if the last-crystal stun is applied in the same
  turn-end as 「每回合结束时移除一层」, which runs first? The quote's
  「立刻获得2层」 asks for 2 after the effect.
* **OPEN-取整** -- rounding of 「支付价格减半」 on an odd price (祥，移动).
* **OPEN-分摊** -- 「均分」 in 八幡海铃 状态2 (the 50/50 ratio is clear; the
  book's 分摊 vocabulary is not defined).
* **OPEN-前方20格** -- 「前方」 is read as clockwise ahead; the book does not
  define a counter-clockwise forward.
* **OPEN-箱庭0步** -- does a sum-0 forced walk of 人偶的箱庭 still count as
  the [主要移动] (closing a later roll)?
* **OPEN-喵梦翻面** -- does paying 2 fire pots cancel the face-down flip
  (「可选择失去2个火罐」 names a cost and no benefit)?

## Findings

Genuine engine-vs-book discrepancies. Each is a plain failing test in
`rules/tests/tests/spec_card_mujica.rs`. No `#[ignore]`, no allowlist.

* **C-MUJICA-团技能-01** (`band_state2_pay_is_150_only_on_self`) -- book says
  状态2 [支付]/[收取] is 1.5x **on the user's side only**
  (「对应的其他玩家收支不变」). Engine moves 1.5x on **both** sides: on a
  300-rent landing the user pays 450 **and** the owner receives 450. The
  owner should receive 300.
* **C-MUJICA-视线-01** (`look_away_only_moves_counteractors`) -- book targets
  only 「当前回合内对你打出过[反击]的所有玩家」. The counter entry moves the
  immediate counterer only; a normal hand play moves **nobody** (`run` returns
  on its guard), which is why the contract passes on the empty-target case.
* **C-MUJICA-花火-03** (`sparkler_burn_and_last_stun`) -- book says the last
  crystal 「立刻使你获得2层[眩晕]」. Engine leaves the user at 1 stun layer
  after the last crystal is removed.
* **C-MUJICA-欢迎-02** (self-target) -- 「转换任意一名玩家的状态」 includes
  the user; the engine's prompt only offers the other players.
* **C-MUJICA-欢迎-02** (no 状态2) (`welcome_no_state2_is_noop`) -- 「若指定
  了不存在状态2的玩家则无效果」 is a required no-op. Engine toggles a
  non-Ave-Mujica seat into 状态2.
* **C-MUJICA-海铃-05** -- 「将此卡与使用者拿取的所有乐队技能卡置入弃牌堆」:
  taken band skills are detached, not put into a discard pile (engine TODO);
  untested here, recorded as a known shortfall.
