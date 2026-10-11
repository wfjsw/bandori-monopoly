# C-PP -- Pastel✽Palettes

Unit C-PP owns this file and `rules/tests/tests/spec_card_pp.rs`. Clause IDs
`C-PP-<slug>-<n>`.

Sources: live sheet (`docs/rulebook/cards.json` keys `PP:*`,
`data/characters.json` band `Pastel✽Palettes`, `data/bands.json` skill
`与偶像一起`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Fan tokens: `P✽P粉丝(正)` / `P✽P粉丝(反)`. 「Pastel✽Palettes乐队卡」 crystal
pool = the band-skill field card `skill:Pastel✽Palettes:与偶像一起` (see
OPEN-乐队卡).

Notation: `up` / `down` = the player's positive / reverse fan counts;
`owned` = `up + down`. `[共鸣]` clauses only fire when the resonance window is
accepted (OPEN-共鸣).

---

## PP:TITLE IDOL

* **ID:** `C-PP-title-idol-01`
* **Source:** `cards.json#PP:TITLE IDOL`
* **Quote:** 「1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；」
* **Trigger:** [手] played from hand.
* **Condition:** the user has a Pastel✽Palettes band card (the crystal pool).
* **Effect:** that pool rises by exactly 2 from this step.
* **Frame:** only the user's card zones / band-card crystals may move.
* **Rulings & OPEN:** OPEN-乐队卡.
* **Executable:** `title_idol_band_plus_2` / `title_idol_crystals`.

* **ID:** `C-PP-title-idol-02`
* **Source:** `cards.json#PP:TITLE IDOL`
* **Quote:** 「2. 为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。」
* **Trigger:** same play, after step 1.
* **Condition:** the user's field cards whose printed effect contains
  「[奇迹水晶]」 (the band card itself qualifies).
* **Effect:** each such card gains 1 crystal, or 2 with [共鸣].
* **Frame:** as above.
* **Rulings & OPEN:** OPEN-共鸣 (the 共鸣 branch); OPEN-乐队卡.
* **Executable:** `title_idol_crystals` / `title_idol_echo_adds_two`.

---

## PP:[丸山彩]憧憬的前方

* **ID:** `C-PP-aya-haruka-01`
* **Source:** `cards.json#PP:[丸山彩]憧憬的前方`
* **Quote:** 「[手]：将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。」
* **Trigger:** [手] played by 丸山彩 (exclusive).
* **Condition:** the discard holds at least one card.
* **Effect:** the card is on the user's field; exactly one discard card is in hand.
* **Frame:** user's field / hand / discard only.
* **Executable:** `aya_haruka_hand`.

* **ID:** `C-PP-aya-haruka-02`
* **Source:** `cards.json#PP:[丸山彩]憧憬的前方`
* **Quote:** 「（1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。」
* **Trigger:** any [消耗] / [支付] the owner makes while uniquely poorest.
* **Condition:** owner's money is strictly least among living players; `owned >= 10` raises X.
* **Effect:** the amount is reduced by X, floored at 0.
* **Frame:** only the payment amount / money legs of that transfer.
* **Rulings & OPEN:** ties for "least" are not specified (OPEN-ties).
* **Executable:** not yet scripted (requires a uniquely-poorest pre-state plus a
  [消耗]; left as a follow-up -- the formula is pinned by the quote).
  *Cannot test in this batch:* no dedicated hook lands only on the
  uniquely-poorest gate without also colliding with the shared pay-modifier
  window; see `## Untestable` below.

* **ID:** `C-PP-aya-haruka-03`
* **Source:** `cards.json#PP:[丸山彩]憧憬的前方`
* **Quote:** 「（2）[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。」
* **Trigger:** a [共鸣][反击] window's [消耗] / [支付].
* **Condition:** [共鸣] accepted (OPEN-共鸣).
* **Effect:** amount reduced by 1500, floored at 0.
* **Frame:** that payment only.
* **Executable:** not scripted (OPEN-共鸣 timing).

---

## PP:[冰川日菜]会发出怎样的声音呢？

* **ID:** `C-PP-hina-sound-01`
* **Source:** `cards.json#PP:[冰川日菜]会发出怎样的声音呢？`
* **Quote:** 「（1）此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡。」
* **Trigger:** an exclusive-card play gate of one of those four characters.
* **Condition:** this card is in hand and the substituted exclusive's own use
  condition is met.
* **Effect:** the play is allowed as if the exclusive card were in hand.
* **Frame:** the play's own effects.
* **Rulings & OPEN:** which exclusive is being substituted when more than one
  is legal is not specified (OPEN-substitution).
* **Executable:** *cannot test* -- no harness API plays a card "as" another;
  see Untestable.

* **ID:** `C-PP-hina-sound-02`
* **Source:** `cards.json#PP:[冰川日菜]会发出怎样的声音呢？`
* **Quote:** 「（2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。」
* **Trigger:** [共鸣] while this card is on the field and is substituting.
* **Condition:** OPEN-共鸣.
* **Effect:** the substituted exclusive's effect is changed. The new text is
  not printed on this card (OPEN-substitution-effect).
* **Executable:** *cannot test* -- the replacement effect is undefined.

---

## PP:[大和麻弥]可能性为∞

* **ID:** `C-PP-maya-inf-01`
* **Source:** `cards.json#PP:[大和麻弥]可能性为∞`
* **Quote:** 「[特]：观看卡组并观看到此卡时将此卡展示给所有玩家并将其放置在自己[场上]（次效果优先于其他后续效果，比如观看卡组后将一张牌加入手牌）。」
* **Trigger:** any deck-watch that reveals this card.
* **Condition:** this card is among the watched cards.
* **Effect:** it is shown and placed on the owner's field, before any later
  watch-resolution effect.
* **Frame:** that watch resolution.
* **Executable:** *cannot test cleanly* -- needs a multi-card watch with this
  card inside; the priority clause is order-sensitive (see Untestable).

* **ID:** `C-PP-maya-inf-02`
* **Source:** `cards.json#PP:[大和麻弥]可能性为∞`
* **Quote:** 「（1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。」
* **Trigger:** after the owner completes a draw action.
* **Condition:** crystals < 4 vs >= 4.
* **Effect:** either +1 crystal on this card, or clear them and +1 on the band card.
* **Frame:** those two crystal counters / the draw.
* **Executable:** `maya_inf_crystal_per_draw` (the <4 leg, via
  `通用:10次招募`'s draw).

* **ID:** `C-PP-maya-inf-03`
* **Source:** `cards.json#PP:[大和麻弥]可能性为∞`
* **Quote:** 「（2）[共鸣]交换[拥有者]的抽卡区和弃卡区。」
* **Trigger:** [共鸣] accepted while this card is on the field.
* **Effect:** the owner's draw pile and discard pile swap contents.
* **Frame:** those two zones only.
* **Executable:** OPEN-timing (see C-PP-maya-inf-02). Not pinned.

---

## PP:[白鹭千圣]微笑的铁假面

* **ID:** `C-PP-chisato-mask-01`
* **Source:** `cards.json#PP:[白鹭千圣]微笑的铁假面`
* **Quote:** 「[手]：将此卡放置在[使用者]的[场地]，其他玩家[分摊][支付][使用者]2000资金。」
* **Trigger:** [手] played by 白鹭千圣.
* **Effect:** card on the user's field; the other players collectively pay the
  user 2000 (OPEN-分摊: only the 2000 total is determined).
* **Frame:** money of every player + the user's field.
* **Executable:** `chisato_mask_hand_split`.

* **ID:** `C-PP-chisato-mask-02`
* **Source:** `cards.json#PP:[白鹭千圣]微笑的铁假面`
* **Quote:** 「（1）[拥有者]的弃卡区洗入抽卡区时其他玩家[分摊][支付][拥有者]500资金。」
* **Trigger:** the owner's discard is reshuffled into the draw pile.
* **Effect:** others collectively pay 500 (OPEN-分摊).
* **Executable:** *not scripted this batch* (needs a forced reshuffle).

* **ID:** `C-PP-chisato-mask-03`
* **Source:** `cards.json#PP:[白鹭千圣]微笑的铁假面`
* **Quote:** 「（2）[共鸣]其他玩家[分摊][支付][拥有者]1500资金。」
* **Trigger:** [共鸣] accepted.
* **Effect:** others collectively pay 1500 (OPEN-分摊).
* **Executable:** *not scripted* (OPEN-共鸣 / OPEN-分摊).

---

## PP:[若宫伊芙]属于我的武士道！

* **ID:** `C-PP-eve-bushido-01`
* **Source:** `cards.json#PP:[若宫伊芙]属于我的武士道！`
* **Quote:** 「[手]：将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。」
* **Trigger:** [手] played by 若宫伊芙.
* **Effect:** card on the user's field; money += (sum of 12d4) * 60.
* **Frame:** money / field of the user.
* **Executable:** `eve_bushido_twelve_d4` / `eve_bushido_played`.

* **ID:** `C-PP-eve-bushido-04`
* **Source:** `cards.json#PP:[若宫伊芙]属于我的武士道！`
* **Quote:** 「（1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。」
* **Trigger:** after the user draws.
* **Effect:** +1 crystal, cap 3.
* **Executable:** `eve_bushido_crystal_per_draw`.

* **ID:** `C-PP-eve-bushido-05`
* **Source:** `cards.json#PP:[若宫伊芙]属于我的武士道！`
* **Quote:** 「（2）[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家则使用1个[奇迹水晶]并依次进行以下操作：1. 地契主人投掷1d6；2. [拥有者]投掷3d4；3. 投掷点数低的玩家[支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），如果[共鸣]则[支付]金额改为“投掷点差”×50。」
* **Trigger:** before [结算] on a tile whose deed owner is another player.
* **Condition:** the card holds >= 1 crystal.
* **Effect:** spend 1 crystal; 1d6 vs 3d4; loser pays winner
  `(houses + 1) * 100`, or 0 on a tie; with [共鸣] the amount is
  `|1d6 - 3d4| * 50`.
* **Frame:** those dice / one payment / one crystal.
* **Executable:** *not scripted this batch* (multi-roll duel; the formula is
  pinned by the quote).

---

## PP:[衍生]共鸣

* **ID:** `C-PP-der-echo-01`
* **Source:** `cards.json#PP:[衍生]共鸣`
* **Quote:** 「[特]：拥有此卡的玩家的卡可发动[共鸣]时将此卡放入弃卡区，发动那张卡的[共鸣]效果并为拥有此卡的玩家的Pastel✽Palettes乐队卡添加2[奇迹水晶]。」
* **Trigger:** another card is about to activate [共鸣].
* **Condition:** this card is in the owner's hand (or otherwise available --
  the text says 「拥有此卡」; hand is the live reading).
* **Effect:** this card goes to the discard; the other card's [共鸣] branch
  runs; the band card gains 2 crystals.
* **Frame:** discard / band crystals / the other card's [共鸣] effects.
* **Rulings & OPEN:** OPEN-共鸣.
* **Executable:** exercised by every `accept_echo` test; the +2 leg is pinned
  in `ix`-style setup inside `title_idol_echo_adds_two` (dream/echo share the
  pool).

---

## PP:[衍生]明天见

* **ID:** `C-PP-seeyou-01`
* **Source:** `cards.json#PP:[衍生]明天见`
* **Quote:** 「（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。」
* **Trigger:** the [特] play gate.
* **Condition:** `up <= down`.
* **Effect:** the play is refused when `up > down`, allowed otherwise.
* **Executable:** `seeyou_gate`.

* **ID:** `C-PP-seeyou-02`
* **Source:** `cards.json#PP:[衍生]明天见`
* **Quote:** 「（2）[共鸣]无视此卡的[特]效果（1）。」
* **Trigger:** [共鸣] accepted while playing this card.
* **Effect:** the gate (1) is ignored.
* **Executable:** *not scripted* (OPEN-共鸣); the gate itself is `seeyou_gate`.

* **ID:** `C-PP-seeyou-03`
* **Source:** `cards.json#PP:[衍生]明天见`
* **Quote:** 「[手]：将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。」
* **Trigger:** [手] played (gate met or bypassed).
* **Effect:** flip `min(owned, down)` reverse fans positive.
* **Executable:** `seeyou_flip_owned_count`.

---

## PP:[衍生]重叠的声音

* **ID:** `C-PP-overlap-01`
* **Source:** `cards.json#PP:[衍生]重叠的声音`
* **Quote:** 「[手]：…1. [移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；」
* **Trigger:** [手] played.
* **Effect:** this card and any 初次演出事故 on the user's field leave play.
* **Executable:** `overlap_play_and_turn_end` (accident half).

* **ID:** `C-PP-overlap-02`
* **Source:** `cards.json#PP:[衍生]重叠的声音`
* **Quote:** 「2. 进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]；」
* **Trigger:** same [手], step 2.
* **Effect:** the main move becomes a teleport to Bandori车站; no settle
  (no buy / rent).
* **Executable:** `overlap_play_and_turn_end`.

* **ID:** `C-PP-overlap-03`
* **Source:** `cards.json#PP:[衍生]重叠的声音`
* **Quote:** 「3. 回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。」
* **Trigger:** the turn ends after this play.
* **Effect:** stay += 1; positive fan += 1; one 明天见 added to hand.
* **Executable:** `overlap_play_and_turn_end`.

* **ID:** `C-PP-overlap-04`
* **Source:** `cards.json#PP:[衍生]重叠的声音`
* **Quote:** 「[特]：此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上的“初次演出事故”（如果有）。」
* **Trigger:** this card enters the owner's discard (any path).
* **Effect:** 初次演出事故 is removed from the owner's field.
* **Executable:** *not scripted* (needs a discard-not-play path).

---

## PP:[衍生]魔法战队Pastel✽Ranger

* **ID:** `C-PP-ranger-00`
* **Source:** `cards.json#PP:[衍生]魔法战队Pastel✽Ranger`
* **Quote:** 「[特]：仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用。」
* **Trigger:** the [特] play gate.
* **Condition:** some tile the player owns with a house lies within absolute
  board distance `30 - owned`.
* **Effect:** play is refused otherwise.
* **Executable:** `ranger_gate`.

* **ID:** `C-PP-ranger-01`
* **Source:** `cards.json#PP:[衍生]魔法战队Pastel✽Ranger`
* **Quote:** 「…依次进行以下操作，如果[共鸣]则视为数量加1：1. 数量至少为1则[获得]500资金；」
* **Trigger:** [手] played. `count` = number of cards the user owns on the
  field (the Ranger itself counts once it is played), +1 with [共鸣].
* **Effect:** if `count >= 1`, money += 500.
* **Executable:** `ranger_steps_by_count`.

* **ID:** `C-PP-ranger-02`
* **Source:** `cards.json#PP:[衍生]魔法战队Pastel✽Ranger`
* **Quote:** 「2. 数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]；」
* **Effect:** if `count >= 3`, band pool += 3.
* **Executable:** `ranger_steps_by_count`.

* **ID:** `C-PP-ranger-03`
* **Source:** `cards.json#PP:[衍生]魔法战队Pastel✽Ranger`
* **Quote:** 「3. 数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡；」
* **Effect:** if `count >= 4`, band pool -= 4 (from 0 is unspecified --
  OPEN-floor) and draw 1.
* **Executable:** *not scripted* (needs `count >= 4`).

* **ID:** `C-PP-ranger-04`
* **Source:** `cards.json#PP:[衍生]魔法战队Pastel✽Ranger`
* **Quote:** 「4. 数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），盖房后减少1层”；」
* **Effect:** if `count >= 5`, gain that 1-layer status.
* **Executable:** *not scripted* (status key is engine-specific).

* **ID:** `C-PP-ranger-05`
* **Source:** `cards.json#PP:[衍生]魔法战队Pastel*=*=Ranger`
* **Quote:** 「5.数量至少为6则抽1张卡。」
* **Effect:** if `count >= 6`, draw 1.
* **Executable:** *not scripted*.

---

## PP:不要背负期待

* **ID:** `C-PP-no-expect-01`
* **Source:** `cards.json#PP:不要背负期待`
* **Quote:** 「[特]：此卡不受任何其他效果影响。」
* **Trigger:** any other effect that would touch this card.
* **Effect:** the card is immune (`FieldCard::immune`).
* **Executable:** `no_expect_hand` (immune flag).

* **ID:** `C-PP-no-expect-02`
* **Source:** `cards.json#PP:不要背负期待`
* **Quote:** 「[手]：将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。」
* **Trigger:** [手] played.
* **Effect:** card on the field; one 共鸣 added to the draw pile; draw 1.
* **Executable:** `no_expect_hand`.

* **ID:** `C-PP-no-expect-03`
* **Source:** `cards.json#PP:不要背负期待`
* **Quote:** 「（1）以下效果对[拥有者]持续生效：非回合开始时进行投掷的投掷结果减少2（如果是移动投掷则最终结果最小为0，如果为0则此次移动不[结算]）；手卡上限数量减1。」
* **Trigger:** continuous while on the field.
* **Effect:** non-turn-start rolls -2 (move floored at 0; 0 does not settle);
  hand limit -1.
* **Executable:** `no_expect_roll_minus_2` / `no_expect_hand` (hand limit).

* **ID:** `C-PP-no-expect-04`
* **Source:** `cards.json#PP:不要背负期待`
* **Quote:** 「（2）以下效果此卡放入[场地]或每次[拥有者]弃卡区洗入抽卡区时对[拥有者]生效2次：[支付]资金时金额提高100；[收取]资金时金额减少100（最低0）。」
* **Trigger:** on placement, and on every discard-reshuffle.
* **Effect:** for the next window, pay +100 and receive -100, applied 2 times
  (i.e. +200 / -200 on the first payment/receipt of that window).
* **Executable:** *not scripted this batch* (window lifetime of 「生效2次」 is
  itself OPEN -- see OPEN-window2).

---

## PP:再次闪耀

* **ID:** `C-PP-shine-again-00`
* **Source:** `cards.json#PP:再次闪耀`
* **Quote:** 「[特]：此卡不受除拥有此卡的玩家以外的玩家的效果影响。」
* **Effect:** the instance is immune to other players' effects (`immune`).
* **Executable:** `shine_again_places_immune`.

* **ID:** `C-PP-shine-again-01`
* **Source:** `cards.json#PP:再次闪耀`
* **Quote:** 「（1）[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡未记录且[拥有者]拥有同色地契的颜色，[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。」
* **Trigger:** [反击] when the owner's [消耗]/[支付] cannot be covered.
* **Effect:** record an unrecorded colour of a same-colour deed the owner has;
  owner gains `(that deed's purchase price) / 5` and 1 positive fan.
* **Executable:** `shine_again_rescue_on_short_pay`.

* **ID:** `C-PP-shine-again-02`
* **Source:** `cards.json#PP:再次闪耀`
* **Quote:** 「（2）[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]。」
* **Trigger:** [共鸣] accepted.
* **Effect:** record a colour; owner gains 1 positive fan.
* **Executable:** *not scripted* (OPEN-共鸣).

---

## PP:初次演出事故

* **ID:** `C-PP-first-accident-01`
* **Source:** `cards.json#PP:初次演出事故`
* **Quote:** 「[手]：将此卡放置在[使用者]的[场地]并将1张“重叠的声音”加入抽卡区并洗切，然后抽1张卡。」
* **Trigger:** [手] played.
* **Effect:** card on the field; 重叠的声音 shuffled into the draw pile; draw 1.
* **Executable:** `first_accident_hand`.

* **ID:** `C-PP-first-accident-02`
* **Source:** `cards.json#PP:初次演出事故`
* **Quote:** 「[持续]：[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能。」
* **Trigger:** continuous.
* **Effect:** no PP character's (2) skill may be used by the owner.
* **Executable:** `first_accident_blocks_pp_skill_2`.

---

## PP:可爱又强壮的花朵

* **ID:** `C-PP-flower-01`
* **Source:** `cards.json#PP:可爱又强壮的花朵`
* **Quote:** 「[手]：[指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]。」
* **Trigger:** [手] played.
* **Effect:** the card sits on the designated owned tile; band pool += 2.
* **Executable:** `flower_place_on_tile`.

* **ID:** `C-PP-flower-02`
* **Source:** `cards.json#PP:可爱又强壮的花朵`
* **Quote:** 「[使用者][经过]此卡所在格子时依次进行以下操作：1. [强制停下]，此次移动变为[结算]；」
* **Trigger:** the user passes the flower's tile.
* **Effect:** force stop; the move becomes a settle. OPEN-强制停下 (definition
  truncated) -- only "the pass is not a clean pass" is specified.
* **Executable:** `flower_pass_discards_and_echo_pays` (card consumed).

* **ID:** `C-PP-flower-03`
* **Source:** `cards.json#PP:可爱又强壮的花朵`
* **Quote:** 「2. 如果[共鸣]则获得1500资金；3. 此卡放入[使用者]弃卡区。」
* **Effect:** +1500 with [共鸣]; the card goes to the user's discard.
* **Executable:** `flower_pass_discards_and_echo_pays`.

---

## PP:同一个梦想

* **ID:** `C-PP-same-dream-01`
* **Source:** `cards.json#PP:同一个梦想`
* **Quote:** 「1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；」
* **Trigger:** [手] played.
* **Effect:** band pool += 3.
* **Executable:** `same_dream_three_steps` / `same_dream_band_plus_3`.

* **ID:** `C-PP-same-dream-02`
* **Source:** `cards.json#PP:同一个梦想`
* **Quote:** 「2. 将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金；」
* **Effect:** every positive fan flips reverse; money += `up * 100`.
* **Executable:** `same_dream_three_steps`.

* **ID:** `C-PP-same-dream-03`
* **Source:** `cards.json#PP:同一个梦想`
* **Quote:** 「3. 将自己的所有反面[P✽P粉丝]变正，如果[共鸣]则此效果对所有Pastel✽Palettes角色生效。」
* **Effect:** all of the user's reverse fans flip positive; with [共鸣] every
  PP character's reverse fans flip positive.
* **Executable:** `same_dream_three_steps` / `same_dream_echo_all_pp_characters`.

---

## PP:找回珍妮弗

* **ID:** `C-PP-jennifer-01`
* **Source:** `cards.json#PP:找回珍妮弗`
* **Quote:** 「[手]：将此卡放置在除[使用者]以外的一名玩家的[场地]。」
* **Trigger:** [手] played.
* **Effect:** the card sits on another player's field.
* **Executable:** `jennifer_place_and_agency_pass`.

* **ID:** `C-PP-jennifer-02`
* **Source:** `cards.json#PP:找回珍妮弗`
* **Quote:** 「[拥有者][经过]“偶像经纪公司”时依次进行以下操作：1. [使用者][支付][拥有者]400资金；2. [使用者]获得1个正面的[P✽P粉丝]，[拥有者]将一个[P✽P粉丝]变为正面；」
* **Trigger:** the owner passes 偶像经纪公司.
* **Effect:** user pays owner 400; user +1 positive fan; owner flips one fan
  positive (which face is flipped is not specified -- OPEN-fan-face).
* **Executable:** `jennifer_place_and_agency_pass`.

* **ID:** `C-PP-jennifer-03`
* **Source:** `cards.json#PP:找回珍妮弗`
* **Quote:** 「3. 此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡。」
* **Effect:** this card is removed; a Ranger joins the user's hand.
* **Executable:** `jennifer_place_and_agency_pass`.

---

## PP:有你与我在这里共度

* **ID:** `C-PP-together-01`
* **Source:** `cards.json#PP:有你与我在这里共度`
* **Quote:** 「[手]：将X个反面[P✽P粉丝]变正；X为5，如果[使用者]拥有的正面[P✽P粉丝]数量少于反面[P✽P粉丝]数量则X额外添加反面和正面[P✽P粉丝]数量的差的一半（向上取整），」
* **Trigger:** [手] played.
* **Effect:** flip `min(X, down)` reverse positive, `X = 5` or
  `5 + ceil((down - up) / 2)` when `up < down`.
* **Executable:** `together_x_formula`.

* **ID:** `C-PP-together-02`
* **Source:** `cards.json#PP:有你与我在这里共度`
* **Quote:** 「如果[共鸣]则X减少5并抽1张卡。」
* **Effect:** with [共鸣], X -= 5 (floored at 0 by "flip X of them") and draw 1.
* **Executable:** `together_echo`.

---

## PP:梦在前方，结彩当下

* **ID:** `C-PP-dream-ahead-00`
* **Source:** `cards.json#PP:梦在前方，结彩当下`
* **Quote:** 「[特]：游戏开始前将此卡放置在[使用者]的[场地]且初始手牌减1。」
* **Trigger:** game start.
* **Effect:** the card is on the user's field before play; opening hand is 1
  (the default deal is 2).
* **Executable:** `dream_ahead_auto_place`.

* **ID:** `C-PP-dream-ahead-01`
* **Source:** `cards.json#PP:梦在前方，结彩当下`
* **Quote:** 「（1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的[奇迹水晶]就为此卡的X加1（X初始0）。」
* **Trigger:** every owner draw.
* **Effect:** +1 crystal to a cap of 5; each would-be overflow increments X.
* **Executable:** `dream_ahead_crystal_per_draw` / `dream_ahead_extra_rent` (X=1).

* **ID:** `C-PP-dream-ahead-02`
* **Source:** `cards.json#PP:梦在前方，结彩当下`
* **Quote:** 「（2）[拥有者]不可盖房且手卡上限数量减1。」
* **Effect:** build is refused; hand limit -1.
* **Executable:** `dream_ahead_no_build_and_hand_limit`.

* **ID:** `C-PP-dream-ahead-03`
* **Source:** `cards.json#PP:梦在前方，结彩当下`
* **Quote:** 「（3）[拥有者]以外的玩家在[拥有者]拥有的格子[结算]时额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。」
* **Trigger:** another player settles on an owner-owned tile.
* **Effect:** extra payment `owned * X + MIN(X * 30, 300)`.
* **Executable:** `dream_ahead_extra_rent`.

* **ID:** `C-PP-dream-ahead-04`
* **Source:** `cards.json#PP:梦在前方，结彩当下`
* **Quote:** 「（4）[共鸣][反击][拥有者]回合结束时且在[拥有者]所在格子前后3格以内有无主的[可购买格子]：使用此卡上3个[奇迹水晶]，[拥有者]购买任意[拥有者]所在格子前后3格以内的无主的[可购买格子]。」
* **Trigger:** owner's turn end, [共鸣][反击] accepted, such a tile exists.
* **Effect:** spend 3 crystals; owner buys any such tile.
* **Executable:** *not scripted* (OPEN-共鸣 + [可购买格子] undefined as a set).

---

## PP:练习生解密指南

* **ID:** `C-PP-guide-01`
* **Source:** `cards.json#PP:练习生解密指南`
* **Quote:** 「[手]：为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的[场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]，」
* **Trigger:** [手] played.
* **Effect:** band pool += 3; card on the field carrying `owned / 3` crystals
  (integer division is the live reading of 「÷」; OPEN-div).
* **Executable:** `guide_hand_places_crystals`.

* **ID:** `C-PP-guide-02`
* **Source:** `cards.json#PP:练习生解密指南`
* **Quote:** 「然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记，如果[共鸣]则[消耗]500资金并添加任意2个标记。」
* **Trigger:** same [手], after the crystals.
* **Effect:** add one 单色/双色 mark keyed on the draw pile's colour count;
  with [共鸣] pay 500 and add any 2 marks. OPEN-mark (the mark vocabulary and
  the colour-count table are not defined on the card).
* **Executable:** *cannot test* -- OPEN-mark.

* **ID:** `C-PP-guide-03`
* **Source:** `cards.json#PP:练习生解密指南`
* **Quote:** 「（1）手卡上限数量减1。」
* **Executable:** `guide_hand_places_crystals`.

* **ID:** `C-PP-guide-04`
* **Source:** `cards.json#PP:练习生解密指南`
* **Quote:** 「（2）回合结束时添加1个[奇迹水晶]。」
* **Trigger:** the owner's turn ends.
* **Effect:** +1 crystal on this card.
* **Executable:** `guide_turn_end_crystal`.

* **ID:** `C-PP-guide-05`
* **Source:** `cards.json#PP:练习生解密指南`
* **Quote:** 「（3）此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区：1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），盖房后减少1层”；2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正。」
* **Trigger:** crystals reach >= 5.
* **Effect:** apply each mark, then the card goes to the discard.
* **Executable:** *cannot test* (depends on OPEN-mark).

---

## Character skills (Pastel✽Palettes)

Shared (1) on every PP character:

* **ID:** `C-PP-<slug>-01` (one per character: `aya-with`, `hina-lottery`,
  `chisato-frank`, `maya-dawn`, `eve-unify`)
* **Source:** `characters.json#<name>`
* **Quote:** 「（1）游戏开始后获得5个正面[P✽P粉丝]，所有非Pastel✽Palettes玩家获得<角色>的（2）技能」
* **Trigger:** game start.
* **Effect:** the character gains 5 positive fans; every non-PP player also
  gains that character's (2) skill.
* **Executable:** `character_start_five_fans` (the 5-fan half). The
  "non-PP gains (2)" half is *not scripted* (needs a non-PP seat using a
  borrowed (2) -- see Untestable).

(2) skills, one clause each:

* **ID:** `C-PP-aya-with-02`
* **Source:** `characters.json#丸山彩`
* **Quote:** 「（2）自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的分摊前资金减少Y×100（最少0）；如果自己是Pastel✽Palettes角色则将Y个其他乐队玩家拥有的反面[P✽P粉丝]变正，否则将所有Pastel✽Palettes角色的1个反面[P✽P粉丝]变正」
* **Effect:** optional; flip Y positive reverse; pay -= Y*100 (floor 0);
  PP: flip Y reverse of other-band players; non-PP: flip 1 reverse of each PP.
* **Executable:** `aya_skill_2_pay_discount` (PP branch).

* **ID:** `C-PP-hina-lottery-02`
* **Source:** `characters.json#冰川日菜`
* **Quote:** 「（2）每回合开始时必须进行1次投掷1d4，根据最后投掷的结果在下回合开始前获得以下角色的（2）技能：结果为1是丸山彩；结果为2是白鹭千圣；结果为3是若宫伊芙；结果为4是大和麻弥。」
* **Effect:** mandatory 1d4 at each turn start; borrow that character's (2)
  until the next turn start.
* **Executable:** `hina_skill_2_borrows_skill`.

* **ID:** `C-PP-chisato-frank-02`
* **Source:** `characters.json#白鹭千圣`
* **Quote:** 「（2）自己每次收取资金时可选择将自己Y个正面[P✽P粉丝]变反，此次获得的分摊前数量增加Y×100；…」
* **Effect:** optional; flip Y; receive += Y*100; same fan-side flip as 丸山彩.
* **Executable:** `chisato_skill_2_income_boost`.

* **ID:** `C-PP-maya-dawn-02`
* **Source:** `characters.json#大和麻弥`
* **Quote:** 「（2）自己抽卡时时可选择将自己Y个正面[P✽P粉丝]变反，此次抽卡改为观看卡组顶端Y+1张卡（卡组数量不足Y+1则观看全部卡组），然后选择一张牌（不公开）加入手牌（此次加手视为抽卡动作），剩余观看的牌洗回卡组；…」
* **Effect:** optional; flip Y; the draw becomes watch Y+1, pick 1 (counts as
  a draw), rest shuffled back.
* **Executable:** `maya_skill_2_watch_pick`.

* **ID:** `C-PP-eve-unify-02`
* **Source:** `characters.json#若宫伊芙`
* **Quote:** 「（2）当你因任意原因投掷前可选择将自己Y个正面[P✽P粉丝]变反并为此次投掷结果增加Yd4；…」
* **Effect:** optional; flip Y; the roll gains +Yd4.
* **Executable:** `eve_skill_2_adds_yd4`.

---

## Band skill -- Pastel✽Palettes「与偶像一起」

* **ID:** `C-PP-band-idol-01`
* **Source:** `bands.json#Pastel✽Palettes`
* **Quote:** 「（1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张Pastel✽Palettes乐队卡时此效果不重复发动）。」
* **Trigger:** game start.
* **Effect:** each non-PP character gains 1 reverse fan; the effect fires once
  even with several PP band cards in play.
* **Executable:** `band_start_reverse_fan` (the 1-fan half). The non-repeat
  half is *not scripted*.

* **ID:** `C-PP-band-idol-02`
* **Source:** `bands.json#Pastel✽Palettes`
* **Quote:** 「（2）你因任意原因受到“将X个反面[P✽P粉丝]变正”的效果且X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）。」
* **Trigger:** a flip-to-positive effect with `X > down`.
* **Effect:** the band card gains `X - down` crystals, cap 10.
* **Executable:** `band_overflow_to_crystals`.

* **ID:** `C-PP-band-idol-03`
* **Source:** `bands.json#Pastel✽Palettes`
* **Quote:** 「（3）[经过]CiRCLE时不获得[CiRCLE奖励]。」
* **Trigger:** passing CiRCLE.
* **Effect:** no CiRCLE reward (no money, no draw).
* **Executable:** `band_no_circle_reward`.

* **ID:** `C-PP-band-idol-04`
* **Source:** `bands.json#Pastel✽Palettes`
* **Quote:** 「（4）回合开始时此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。」
* **Trigger:** the owner's turn start.
* **Effect:** +1 crystal; optional spend 5 to draw 1.
* **Executable:** `band_turn_start_crystal_and_spend`.

---

## Untestable (this batch)

* Exclusive **substitution** (`C-PP-hina-sound-01/02`): no harness API plays a
  card as another; the replacement effect is unprinted.
* **Watch-deck placement priority** (`C-PP-maya-inf-01`): needs a watch that
  reveals the card among others; ordering is OPEN.
* **分摊 splits** (`C-PP-chisato-mask-02/03`): OPEN-分摊.
* **单色/双色标记** (`C-PP-guide-02/06`): OPEN-mark.
* **[共鸣][反击] turn-end buy** (`C-PP-dream-ahead-04`): OPEN-共鸣 plus an
  undefined 「[可购买格子]」 set.
* **Non-PP borrowed (2)**: the (1) 「所有非Pastel✽Palettes玩家获得…（2）技能」
  half needs a non-PP seat driving a PP (2); not scripted.

---

## Open questions

* **OPEN-共鸣** -- [共鸣] is undefined in the book body. The engine offers a
  resonance window; clauses that read 「如果[共鸣]」 only pin the accepted
  branch. `rules/tests/tests/spec_card_pp.rs` uses `accept_echo`.
* **OPEN-分摊** -- 分摊 is undefined. Only collective totals are specified.
* **OPEN-乐队卡** -- 「Pastel✽Palettes乐队卡」 is named, not defined as a class.
  This unit writes the band-skill field card's `crystals` as that pool.
* **OPEN-强制停下** -- `rulebook-doc.md:93` truncates the definition. Flower (1)
  only pins "the pass is not clean".
* **OPEN-mark** -- 单色/双色标记 vocabulary and the colour-count table are not
  printed on 练习生解密指南.
* **OPEN-substitution** / **OPEN-substitution-effect** -- which exclusive is
  substituted, and what 「改变」 replaces it with, are unprinted.
* **OPEN-fan-face** -- 「将一个[P✽P粉丝]变为正面」 does not say which face is
  flipped when both exist.
* **OPEN-window2** -- 「生效2次」's window (until the next payment? until turn
  end?) is unprinted.
* **OPEN-ties** -- 「资金数是所有存活玩家中最少」 on a tie is unprinted.
* **OPEN-floor** -- removing 4 crystals when fewer than 4 exist is unprinted.
* **OPEN-div** -- 「粉丝数量」÷3 rounding (integer division is assumed).
* **OPEN-tiles** -- 「bandori车站」 maps to board tile `Bandori车站`; the card's
  lowercase spelling is treated as the same tile.

---

## Findings

Genuine engine / book discrepancies. These tests stay red.

* **C-PP-first-accident-02** -- book: 「[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能」.
  Engine: a `skillBlock` token is written, but 丸山彩's (2) prompt still opens
  and the pay-discount runs. Test: `first_accident_blocks_pp_skill_2`.
* **C-PP-band-idol-02** -- book: overflow of a fan-flip becomes crystals on
  the band card (「X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]」).
  Engine: no flip-effect trigger, so the overflow never lands. Test:
  `band_overflow_to_crystals`. (Also recorded in TEST-FINDINGS §6
  "Fan-flip trigger".)
* **C-PP-hina-lottery-02** -- book: each turn start rolls 1d4 and borrows that
  character's (2) until the next turn start. Engine: `skill.hinaLottery.borrowed`
  stays -1. Test: `hina_skill_2_borrows_skill`.
* **C-PP-shine-again-01** -- book: the [反击] rescue grants
  「同色地契的购买价格」÷5 **and** 「1个正面[P✽P粉丝]」.
  Engine: the money leg lands (+600 on a 3000-price deed) but the positive
  fan is never granted. Test: `shine_again_rescue_on_short_pay`.
