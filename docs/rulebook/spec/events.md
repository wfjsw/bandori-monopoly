# E-Events -- 事件卡

Unit E-Events owns this file and `rules/tests/tests/spec_events.rs`. Clause IDs
`E-<slug>-<n>`.

Sources: live sheet (`data/events.json`) > `docs/rulebook/rulebook-doc.md`
(basic draw rules). `rulebook.txt` is not authoritative. Classification
vocabulary (one-shot / active / derived / permanent) is from `docs/EVENTS.md`
only; every contract quotes the event's own `text`.

Draw / filing rules come from `rulebook-doc.md` 基础[结算] 2 (lines 152–154):

> · CiRCLE咖啡厅和流星堂的的[结算]是：抽取一张手卡，然后抽取一个事件卡。
> 　· 抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。
> 　· 事件结算后进入事件弃卡区。如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。

A discrepancy is a plain failing test. No `#[ignore]`.

---

## Draw / filing

### E-draw-01

* **ID:** `E-draw-01`
* **Source:** `rulebook-doc.md:153`
* **Quote:** 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」
* **Trigger:** the event half of CiRCLE 咖啡厅 / 流星堂 [结算], or any
  「抽取一个事件卡」.
* **Condition:** the event deck held a card to draw.
* **Effect:** the drawn id is logged publicly; it never enters any hand; its
  printed effect runs before the draw's settle ends.
* **Frame:** filing piles / active list / the effect's own paths.
* **Rulings & OPEN:** none.
* **Executable:** `e_draw_01_public_not_hand` / `draw_reveals_and_files_the_one_shot`.

### E-draw-02

* **ID:** `E-draw-02`
* **Source:** `rulebook-doc.md:154`
* **Quote:** 「事件结算后进入事件弃卡区。」
* **Trigger:** a drawn event that does not stay in play.
* **Condition:** the draw's settle has finished.
* **Effect:** the id sits in `event_discard`, not in the deck or on the active
  list (active events leave this leg until their own 「放入事件弃牌」; the
  derived permanent one never enters it).
* **Frame:** filing piles.
* **Rulings & OPEN:** none.
* **Executable:** `e_draw_02_filed`.

### E-draw-03

* **ID:** `E-draw-03`
* **Source:** `rulebook-doc.md:154`
* **Quote:** 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
* **Trigger:** a draw that would take from an empty deck while the discard is
  non-empty.
* **Condition:** empty `event_deck`, non-empty `event_discard`.
* **Effect:** the discard becomes the new deck (shuffled) and the draw takes
  from it. `event_removed` is never recycled.
* **Frame:** the two piles swap roles.
* **Rulings & OPEN:** user ruling 2026-10-07 (「Wait for everything to resolve,
  only then reshuffle」) -- recycle after the current event is fully filed,
  matching 「事件结算后进入事件弃卡区」 first.
* **Executable:** `e_draw_03_reshuffle` / `draw_reshuffles_the_discard_when_the_deck_is_empty`.

---

## 对邦 (one-shot)

**Source:** `data/events.json#对邦`
**Quote:** 「所有人立刻进行一次1d20骰子拼点，所有点数最高者从所有点数最低者获得胜出点数x50的钱（例：最高点数18，最低点数16，则需支付 (18-16)x50=100）」

### E-duobang-01

* **ID:** `E-duobang-01`
* **Trigger:** the event resolves.
* **Condition:** every living player rolls 1d20.
* **Effect:** highest roller(s) receive from lowest roller(s) the amount
  `(max - min) * 50`. The sheet example pins 18 vs 16 → 100.
* **Frame:** `money:` plus the generic filing.
* **Rulings & OPEN:** OPEN-duobang-tie -- multiple highest / lowest is
  「所有…最高者从所有…最低者」; the split among the extremes is not specified.
* **Executable:** `duobang_sheet_example_18_vs_16_is_100`.

### E-duobang-02

* **ID:** `E-duobang-02`
* **Trigger:** same.
* **Condition:** a second face pair (e.g. 7 / 20 / 3).
* **Effect:** total money moved equals `(max - min) * 50`.
* **Frame:** `money:`.
* **Executable:** `duobang_spread_times_50`.

---

## 弦卷集团地产开发 (one-shot)

**Source:** `data/events.json#弦卷集团地产开发`
**Quote:** 「随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付选择数量的资金并获得该格子的地契」

### E-estate-01

* **ID:** `E-estate-01`
* **Trigger:** the event resolves.
* **Condition:** at least one 可购买格子 exists (else the card is a no-op).
* **Effect:** a purchasable tile is designated; every player bids; the highest
  bidder pays their bid and gains that deed.
* **Frame:** `money:` of the winner, `owner:T<estate>`, filing.
* **Rulings & OPEN:** OPEN-estate-tie -- tie-breaking among equal high bids is
  not specified. OPEN-estate-empty -- 「如果有」 with no purchasable tile is a
  no-op (determined only as "nothing else moves").
* **Executable:** `estate_auction_grants_a_deed`.

### E-estate-02

* **ID:** `E-estate-02`
* **Trigger:** same, every bid 0.
* **Effect:** nobody pays (a 0 bid costs 0).
* **Frame:** `money:` unchanged.
* **Executable:** `estate_all_pass_leaves_no_new_deed`.

### E-estate-03

* **ID:** `E-estate-03`
* **Trigger:** same, a completed auction.
* **Effect:** only the designated tile changes hands.
* **Frame:** `owner:` of every other tile unchanged.
* **Executable:** `estate_grants_only_the_auctioned_tile`.

---

## 很噜的感觉 (one-shot)

**Source:** `data/events.json#很噜的感觉`
**Quote:** 「所有玩家可选择[传送]至快餐店（不触发场地效果），如果选择[传送]需支付500资金并喊出“噜噜噜！”（只要玩家嗓子允许，必须喊出“噜噜噜！”）」

### E-lulu-01

* **ID:** `E-lulu-01`
* **Trigger:** the event resolves.
* **Condition:** each player may choose the teleport.
* **Effect:** an accepting player [传送]s to 快餐店, pays 500, and the landing
  triggers no tile effect.
* **Frame:** that player's `pos:` / `money:` plus filing.
* **Rulings & OPEN:** the shout 「噜噜噜！」 is out-of-game flavour -- **cannot
  test** (no match-state seam).
* **Executable:** `lulu_teleport_is_optional_and_costs_500`.

### E-lulu-02

* **ID:** `E-lulu-02`
* **Trigger:** same, a decline.
* **Effect:** the player stays put and pays nothing.
* **Frame:** `pos:` / `money:` unchanged for the decliner.
* **Executable:** `lulu_decline_leaves_everyone_put`.

---

## 前往哈比内尔王国旅游 (one-shot + timed [除外])

**Source:** `data/events.json#前往哈比内尔王国旅游`
**Quote:** 「每个玩家获得X层[除外]，每名玩家回合开始时移除1层[除外]，结束后传送到微笑号（不触发场地效果）X=1d2（每个玩家独立投掷1d2）」

### E-habinel-01

* **ID:** `E-habinel-01`
* **Trigger:** the event resolves.
* **Condition:** each player independently rolls 1d2 for X.
* **Effect:** each gains X layers of [除外].
* **Frame:** `state:P*:exile` plus filing.
* **Executable:** `habinel_grants_x_exile_per_player`.

### E-habinel-02

* **ID:** `E-habinel-02`
* **Trigger:** a player's turn start while they hold [除外].
* **Effect:** one layer is removed.
* **Frame:** that player's `state:P*:exile`.
* **Executable:** `habinel_exile_ticks_away_at_turn_start`.

### E-habinel-03 (OPEN)

* **ID:** `E-habinel-03`
* **Quote:** 「结束后传送到微笑号（不触发场地效果）」
* **Status:** **OPEN-habinel-end** -- 「结束后」 is not bound (after the event
  body? after the exile countdown?). The destination 微笑号 and the
  「不触发场地效果」 rider are determined; the timing is not. Specified only
  as: whenever the teleport fires, it is a bare [传送] to 微笑号.
* **Executable:** not scripted (timing gap).

---

## 这只手我不会放开 (one-shot move)

**Source:** `data/events.json#这只手我不会放开`
**Quote:** 「所有玩家移动X，X=1d20-1d20+1d20，如果X是负数则向自身移动方向的反方向移动（每个玩家独立投掷三个1d20进行计算，不触发场地效果）」

### E-this-hand-01

* **ID:** `E-this-hand-01`
* **Trigger:** the event resolves.
* **Condition:** each player rolls 3d20; `X = d20 - d20 + d20`.
* **Effect:** every player (including the drawer) moves X, or |X| in the
  reverse direction when X < 0.
* **Frame:** `pos:` plus filing.
* **Executable:** `this_hand_moves_other_players_by_the_formula`.

### E-this-hand-02

* **ID:** `E-this-hand-02`
* **Quote:** 「如果X是负数则向自身移动方向的反方向移动」
* **Effect:** a negative X walks the board counter-clockwise.
* **Executable:** covered by the P1 leg of `this_hand_moves_other_players_by_the_formula`.

### E-this-hand-03

* **ID:** `E-this-hand-03`
* **Quote:** 「不触发场地效果」
* **Effect:** the event's move settles nothing (no buy / rent / etc.).
* **Frame:** `owner:` / `houses:` unchanged by the move.
* **Executable:** `this_hand_move_triggers_no_tile_settle`.

---

## PICO灵魂交换 (one-shot teleport)

**Source:** `data/events.json#PICO灵魂交换`
**Quote:** 「所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置（不触发场地效果，例：第一位行动的玩家[传送]到第二位行动的玩家的位置并以此类推，最后行动的玩家[传送]到第一位行动的玩家的位置）」

### E-pico-swap-01

* **ID:** `E-pico-swap-01`
* **Trigger:** the event resolves.
* **Condition:** the ring of players without [除外], in action order.
* **Effect:** each takes the next ring member's square (last takes the first's).
* **Frame:** `pos:` of the ring; tile effects do not fire.
* **Executable:** `pico_swap_rotates_positions`.

### E-pico-swap-02

* **ID:** `E-pico-swap-02`
* **Effect:** a [除外] player is not moved and is skipped in the ring.
* **Executable:** `pico_swap_skips_exiled_players`.

---

## 协助CiRCLE重建 (active)

**Source:** `data/events.json#协助CiRCLE重建`
**Quote:** 「将此卡放置于场地中央，为其放置5个奇迹水晶，每次有人经过CiRCLE时移除一个，为0时放入事件弃牌，期间CiRCLE的[触发结算]改为选择获得2层[停留]或失去500资金（该改变[触发结算]的效果优先于其他任何改变[触发结算]的效果），CiRCLE原本的所有效果迁移至CiRCLE咖啡厅并覆盖其原本效果」

### E-circle-rebuild-01

* **ID:** `E-circle-rebuild-01`
* **Trigger:** the draw resolves.
* **Effect:** the card stays in play (「放置于场地中央」) carrying 5 crystals.
* **Frame:** active list / event instance crystals.
* **Executable:** `circle_rebuild_plays_with_five_crystals`.

### E-circle-rebuild-02

* **ID:** `E-circle-rebuild-02`
* **Effect:** the public `ActiveEvent::counter` mirrors the crystal count.
* **Executable:** `circle_rebuild_counter_mirrors_the_crystals`.

### E-circle-rebuild-03

* **ID:** `E-circle-rebuild-03`
* **Trigger:** a player [经过] CiRCLE while the card is up.
* **Effect:** one crystal is removed; at 0 the card is filed (「放入事件弃牌」).
* **Executable:** `circle_rebuild_counts_down_on_passes_then_discards`.

### E-circle-rebuild-04

* **ID:** `E-circle-rebuild-04`
* **Effect:** while up, CiRCLE's [触发结算] becomes a choice of 2 [停留] or
  lose 500.
* **Rulings & OPEN:** OPEN-触发结算 ([触发结算] undefined in the book body).
  The 优先权 claim (「优先于其他任何改变[触发结算]的效果」) is not testable
  without a second replacement effect -- **cannot test** the priority, only
  the replacement itself.
* **Executable:** `circle_rebuild_replaces_circle_settle`.

### E-circle-rebuild-05

* **ID:** `E-circle-rebuild-05`
* **Effect:** CiRCLE's original effects move to CiRCLE 咖啡厅 and override its
  own. (The [经过] reward rides that migration.)
* **Status:** the [经过] reward half is now pinned directly, and it runs as
  `tile:circle` via the cross-card `"settleReward"` message rather than a call
  into that module (`docs/CARDS.md`). The veto that suppresses the reward on
  the real CiRCLE dies with the event. What is still open is the *rest* of
  「所有效果」 riding along automatically -- that needs a real tile-rule swap,
  BACKLOG **ABI-13**. A card keyed *on* CiRCLE (charge replenish) must not
  ride the migration; see the constraint on ABI-13.
* **Executable:** `circle_rebuild_pays_the_reward_on_a_cafe_pass`,
  `circle_rebuild_moves_the_pass_reward_off_circle`,
  `circle_rebuild_restores_the_reward_after_it_expires`,
  `circle_rebuild_leaves_charge_replenish_on_the_real_circle`.

---

## 前场队还是后场队？ (one-shot money)

**Source:** `data/events.json#前场队还是后场队？`
**Quote:** 「所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加定位X且站在格子序号31到60的玩家的投掷结果相加定位Y；如果X=Y则所有玩家获得500资金，如果X>Y则站在格子序号1到30的玩家分摊获得1000资金，如果Y>X则站在格子序号31到60的玩家分摊获得1000资金」

Board indices: 「#N格」 = index `N-1`, so X is tiles 0–29 and Y is 30–59.

### E-front-or-back-01

* **ID:** `E-front-or-back-01`
* **Effect:** X = Y ⇒ every player gains exactly 500.
* **Executable:** `front_or_back_equal_halves_give_500_each`.

### E-front-or-back-02

* **ID:** `E-front-or-back-02`
* **Effect:** the higher half shares a 1000 total (sole member takes 1000).
* **Rulings & OPEN:** OPEN-分摊 (分摊 undefined) -- the split among several
  members of the winning half is not specified; only the 1000 total is.
* **Executable:** `front_or_back_higher_half_gets_1000_total`.

---

## EX任务挑战 (one-shot)

**Source:** `data/events.json#EX任务挑战`
**Quote:** 「抽出此卡的玩家投掷1d4并记录结果和26相加为X，然后所有玩家各自投掷3d20，结果至少为X的玩家可选择更换角色皮肤（技能相同）」

### E-ex-quest-01

* **ID:** `E-ex-quest-01`
* **Effect:** the drawer rolls 1d4 (X = result + 26); every player rolls 3d20;
  the card is a one-shot and files away.
* **Executable:** `ex_quest_resolves_and_files_away`.

### E-ex-quest-02

* **ID:** `E-ex-quest-02`
* **Effect:** a total ≥ X may change character skin (skills unchanged).
* **Status:** the skin is cosmetic -- **cannot test** the change. The
  determined part (the draw completes without blocking) is
  `ex_quest_high_roll_completes`.
* **Rulings & OPEN:** 「技能相同」 is a constraint on the cosmetic swap, not a
  match-state fact.

---

## 发送熊饼表情 (one-shot + derived push)

**Source:** `data/events.json#发送熊饼表情`
**Quote:** 「所有技能中含有火罐的玩家可选择补充X个火罐（不能超过上限），并支付X次1000资金。随后，将一张“[衍生]冲榜”背面朝上放置于事件牌堆顶部。」

### E-bear-cookie-01

* **ID:** `E-bear-cookie-01`
* **Effect:** a derived 冲榜 is pushed face-down onto the event deck top.
* **Executable:** `bear_cookie_pushes_chart_rush_face_down`.

### E-bear-cookie-02

* **ID:** `E-bear-cookie-02`
* **Effect:** the next event draw takes that face-down 冲榜.
* **Executable:** `bear_cookie_next_draw_is_chart_rush`.

### E-bear-cookie-03

* **ID:** `E-bear-cookie-03`
* **Effect:** players whose skill text contains 火罐 may refill X pots (≤ cap)
  and pay X×1000.
* **Status:** the offer is character-gated; the refill amount X and the cap
  are skill-defined (not event text). The determined part (optional, paid
  per pot) is pinned by the quote; a dedicated contract needs a fire-pot
  character on the seat -- left as a follow-up (see Untestable).
* **Executable:** not scripted in this batch.

---

## 飞鸟山之战 (one-shot + status spread)

**Source:** `data/events.json#飞鸟山之战`
**Quote:** 「所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20，行动序列上在抽到此卡的玩家的上一位玩家获得2层回合开始时减少一层的[眩晕]，行动序列上在抽到此卡的玩家的下一位玩家在下个自己的回合结束前不可使用手牌，其余玩家获得1层在回合开始时移除的[除外]」

### E-asukayama-01

* **ID:** `E-asukayama-01`
* **Effect:** everyone [传送]s to 飞鸟山公园; the drawer then moves 1d20.
* **Executable:** `asukayama_drawer_moves_1d20_after_the_teleport`.

### E-asukayama-02

* **ID:** `E-asukayama-02`
* **Effect:** the previous seat takes 2 [眩晕] (1/turn start); the next seat is
  locked out of hand cards until the end of their next turn; the rest take 1
  [除外] (removed at turn start).
* **Executable:** `asukayama_assigns_the_status_spread`.

### E-asukayama-03

* **ID:** `E-asukayama-03`
* **Quote:** 「回合开始时减少一层的[眩晕]」 / 「在回合开始时移除的[除外]」
* **Effect:** those statuses tick at the holder's turn start.
* **Executable:** covered by `habinel_exile_ticks_away_at_turn_start` for the
  [除外] half; [眩晕] decay is the engine's status tick (same 「回合开始时」
  rider).

---

## 上学时间 (one-shot teleport)

**Source:** `data/events.json#上学时间`
**Quote:** 「全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。」

School tiles come from the table in `rules/events/src/school_time.rs` (fallback 周边学区).

### E-school-time-01

* **ID:** `E-school-time-01`
* **Effect:** each player moves to their character's school tile and gains 1
  [除外].
* **Executable:** `school_time_moves_to_school_and_exiles`.

### E-school-time-02

* **ID:** `E-school-time-02`
* **Effect:** a character with no school entry goes to 周边学区.
* **Executable:** `school_time_fallback_to_surrounding_district`.

---

## 泪水的含义 (one-shot money)

**Source:** `data/events.json#泪水的含义`
**Quote:** 「所有玩家依次和行动序列里的下一位玩家进行比较：当前玩家和下一位玩家拥有同色地契则下一位玩家[支付]当前玩家700资金，否则当前玩家[支付]下一位玩家300资金。」

### E-tears-01

* **ID:** `E-tears-01`
* **Effect:** for each seat vs the next in action order: same-colour deeds ⇒
  the next pays the current 700.
* **Rulings & OPEN:** 「同色地契」 is read as *some* shared colour between the
  two holders. Multi-colour overlaps are not specified (OPEN-tears-colour).
* **Executable:** `tears_same_colour_next_pays_700`.

### E-tears-02

* **ID:** `E-tears-02`
* **Effect:** otherwise the current pays the next 300.
* **Executable:** `tears_different_colour_current_pays_300`.

---

## Kizuna Music (one-shot)

**Source:** `data/events.json#Kizuna Music`
**Quote:** 「触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的格子并获得1层[除外]，X为所有玩家卡组里卡名为歌名的卡数量的总数」

### E-kizuna-01

* **ID:** `E-kizuna-01`
* **Effect:** the drawer rolls X+1d20 (X = total song-named cards in all
  decks); everyone [传送]s to `roll mod 60` and gains 1 [除外].
* **Rulings & OPEN:** OPEN-kizuna-song -- 「卡名为歌名的卡」 is not defined in
  the rulebook body (`data/song_cards.json` is a sheet annex). The mod-60
  teleport and the [除外] layer are determined and tested with X = 0.
* **Executable:** `kizuna_teleports_all_to_roll_mod_60`.

---

## 冲榜 (derived one-shot)

**Source:** `data/events.json#冲榜`
**Quote:** 「所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一，点数最高的人获得2000资金或抽一张卡，点数第二的人获得1500资金，点数第三的人获得1000资金，点数第四的人获得500资金（如果两个人点数相同则同时获得较高的 那一档，例：获得奖励名次为1，2，2，4）」

### E-chart-rush-01

* **ID:** `E-chart-rush-01`
* **Effect:** every player loses all 火罐 and rolls Xd20, X = (lost) + 1.
* **Executable:** `chart_rush_empties_fire_pots_and_rolls`.

### E-chart-rush-02

* **ID:** `E-chart-rush-02`
* **Effect:** ranks 2 / 3 / 4 take 1500 / 1000 / 500; rank 1 takes 2000 or a
  card draw; ties share the higher tier (1,2,2,4).
* **Rulings & OPEN:** OPEN-chart-rush-or -- 「获得2000资金或抽一张卡」 does not
  say who chooses. OPEN-chart-rush-tie -- the example pins the tier skip; the
  purse for a 5th+ tie is not spelled out.
* **Executable:** `chart_rush_pays_the_ranked_purses` (pins 2nd/3rd purses and
  that 1st does not lose money).

---

## 修复公告 (derived active)

**Source:** `data/events.json#修复公告`
**Quote:** 「前2个房屋造价为2000的格子和前1个房屋造价为1500的格子的加盖金额减半，且累计在上述格子上加盖后将此卡移除。」

### E-fix-note-01

* **ID:** `E-fix-note-01`
* **Effect:** while up, the first two house-cost-2000 tiles and first
  house-cost-1500 tile (board order) build at half price.
* **Executable:** `fix_note_halves_the_designated_build_costs`.

### E-fix-note-02

* **ID:** `E-fix-note-02`
* **Effect:** after the designated tiles have been built on, the card is
  removed.
* **Rulings & OPEN:** OPEN-fix-note-count -- 「累计在上述格子上加盖后」 does not
  say how many builds (one per tile? three total?). Specified only as: the
  card leaves once the builds have accumulated.
* **Executable:** not scripted (count gap).

---

## 幻觉来了 (active)

**Source:** `data/events.json#幻觉来了`
**Quote:** 「将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃牌。任何玩家进行投掷前在行动顺序的上一名玩家代替进行此次投掷（所有影响投掷的效果服从于原本进行投掷的玩家所收影响）。」

### E-hallucination-01

* **ID:** `E-hallucination-01`
* **Effect:** the card stays until the drawer's next turn start, then files to
  the event discard.
* **Executable:** `hallucination_stays_until_next_turn_start`.

### E-hallucination-02

* **ID:** `E-hallucination-02`
* **Effect:** any player's roll is performed by the previous seat in action
  order; dice-affecting effects still key off the original roller.
* **Status:** the substitution is an ownership-of-the-roll fact; the
  「所收影响」 rider is OPEN-hallucination-effects. Stay/expiry is pinned;
  a substitution contract needs a second seat to observe the face -- left as
  a follow-up.
* **Executable:** not scripted in this batch.

---

## 卡池BUG (active)

**Source:** `data/events.json#卡池BUG`
**Quote:** 「将此卡放置于场地中央并将一张“[衍生]修复公告”背面朝上放置于事件牌堆顶部，下次触发事件时将此卡放入事件弃牌。此卡在场时不可在造价1500及以上的格子上加盖房屋。」

### E-pool-bug-01

* **ID:** `E-pool-bug-01`
* **Effect:** stays in play; a derived 修复公告 is pushed face-down on top.
* **Executable:** `pool_bug_pushes_fix_note_and_stays`.

### E-pool-bug-02

* **ID:** `E-pool-bug-02`
* **Effect:** the next event trigger files this card away.
* **Executable:** `pool_bug_expires_on_the_next_event_draw`.

### E-pool-bug-03

* **ID:** `E-pool-bug-03`
* **Effect:** no house may be added on a tile whose house cost is ≥ 1500.
* **Executable:** `pool_bug_blocks_expensive_builds`.

---

## A！A！O！ (active)

**Source:** `data/events.json#A！A！O！`
**Quote:** 「将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃牌。此卡在场时全场玩家不能使用卡牌的[手]效果。」

### E-aao-01

* **ID:** `E-aao-01`
* **Effect:** stays until the drawer's next turn END (not the turn start),
  then files away.
* **Executable:** `aao_stays_until_next_turn_end`.

### E-aao-02

* **ID:** `E-aao-02`
* **Effect:** while up, no player may use a card's [手] effect.
* **Executable:** `aao_refuses_the_hand_play` / `aao_blocks_hand_effects`.

---

## 麻里奈小姐的礼物箱 (active)

**Source:** `data/events.json#麻里奈小姐的礼物箱`
**Quote:** 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。所有玩家[经过]CiRCLE时可[消耗]一次500资金，抽到的玩家投掷一次1d10（不受任何其他效果影响），如果投掷结果至少为6，那名玩家[获得]1200资金。」

### E-marina-box-01

* **ID:** `E-marina-box-01`
* **Effect:** stays until the drawer's 3rd turn start (the draw turn counts as
  #1), then files away.
* **Executable:** `marina_box_stays_until_third_turn_start`.

### E-marina-box-02

* **ID:** `E-marina-box-02`
* **Effect:** a player [经过] CiRCLE may [消耗] 500 to have the drawer roll 1d10
  (unaffected); on 6+ the passer [获得]s 1200. Net for a 6+ accept with the
  CiRCLE 奖励: +2000 − 500 + 1200.
* **Rulings & OPEN:** OPEN-marina-who -- 「那名玩家」 is read as the passer (the
  one who paid). OPEN-marina-default -- the optional spend's default polarity
  is a host choice (TEST-FINDINGS: default is 「不消耗」).
* **Executable:** `marina_box_spend_on_circle_pass_can_pay_1200`.

---

## 元祖！邦多利酱 (active)

**Source:** `data/events.json#元祖！邦多利酱`
**Quote:** 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。所有玩家的回合免费时间变为5秒，且回合恢复时间变为0秒。」

### E-bangdream-chan-01

* **ID:** `E-bangdream-chan-01`
* **Effect:** stays until the drawer's 3rd turn start, then files away.
* **Executable:** `bangdream_chan_stays_until_third_turn_start`.

### E-bangdream-chan-02

* **ID:** `E-bangdream-chan-02`
* **Effect:** free time 5s / recovery 0s for every player.
* **Status:** host clocks -- **cannot test** (no match-state seam; matches
  the RULING note in TEST-FINDINGS § Event rules).

---

## 超燃甩头 (active, two-phase)

**Source:** `data/events.json#超燃甩头`
**Quote:** 「将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，此后抽到的玩家的下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃牌。」

### E-heat-head-01

* **ID:** `E-heat-head-01`
* **Effect:** until the drawer's next turn end, every movement die gains 1d6.
* **Executable:** `heat_head_stays_and_adds_1d6`.

### E-heat-head-02

* **ID:** `E-heat-head-02`
* **Effect:** the next window subtracts 1d6 from movement dice.
* **Rulings & OPEN:** OPEN-heat-head-2 -- the quote reuses 「下回合结束前」 for
  both windows, so the second window's bound is ambiguous (the engine reads
  it as the turn end after the first). Phase-2 arithmetic is pinned;
  the window bound is not.
* **Executable:** `heat_head_phase_two_subtracts_1d6`.

### E-heat-head-03

* **ID:** `E-heat-head-03`
* **Effect:** after both windows the card files to the event discard.
* **Executable:** `heat_head_eventually_files_away`.

---

## 意外的对邦 (active)

**Source:** `data/events.json#意外的对邦`
**Quote:** 「将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌。所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动（如果距离一样则向正常方向移动）。」

### E-surprise-duel-01

* **ID:** `E-surprise-duel-01`
* **Effect:** the card stays in play while the steering is up.
* **Executable:** `surprise_duel_stays_and_steers_others`.

### E-surprise-duel-02

* **ID:** `E-surprise-duel-02`
* **Effect:** expiry at the drawer's 2nd subsequent turn start.
* **Rulings & OPEN:** OPEN-surprise-2turn -- 「下2回合开始时」 could mean "the
  start of each of the next 2 turns" (which would expire at the first) or
  "the start of the 2nd next turn". Specified as the 2nd (EVENTS.md reading).
* **Executable:** `surprise_duel_expires_at_second_turn_start`.

### E-surprise-duel-03

* **ID:** `E-surprise-duel-03`
* **Effect:** other players move toward the direction that minimises absolute
  distance to the drawer; a tie keeps the normal direction.
* **Status:** the direction rewrite is pinned by the quote; a contract needs
  a controlled walk -- left as a follow-up (the tie-break is determined).
* **Executable:** not scripted in this batch.

---

## Forbidden Moca (active + permanent removal)

**Source:** `data/events.json#Forbidden Moca`
**Quote:** 「直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除此事件！」

### E-moca-01

* **ID:** `E-moca-01`
* **Effect:** until the drawer's next turn end, every movement die is halved;
* **Executable:** `moca_halves_dice_and_is_banished`.

### E-moca-02

* **ID:** `E-moca-02`
* **Effect:** at that turn end the event is permanently removed (「永久移除」 →
  `event_removed`, never recycled).
* **Executable:** same test (the removed-pile half).

---

## 火种燃尽之后会怎么样呢？ (active + tile copies)

**Source:** `data/events.json#火种燃尽之后会怎么样呢？`
**Quote:** 「（1）将此卡的复制品放置于所有玩家所在格子上，触发事件的玩家回合开始时将此卡的复制品放置于所有玩家的前后各一格，触发结算时，格子上每有一张此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡的复制品。」

### E-embers-01

* **ID:** `E-embers-01`
* **Effect:** a copy (tile mark) sits on every player's square.
* **Executable:** `embers_places_copies_on_every_player_tile`.

### E-embers-02

* **ID:** `E-embers-02`
* **Effect:** at the drawer's turn start, copies also go on the square before
  and after each player.
* **Status:** the front/back placement is pinned by the quote; a dedicated
  contract needs a turn-start observe -- left as a follow-up.
* **Executable:** not scripted in this batch.

### E-embers-03

* **ID:** `E-embers-03`
* **Effect:** on [触发结算], each copy on a tile makes that player lose 10.
* **Rulings & OPEN:** OPEN-触发结算.
* **Executable:** `embers_copy_costs_10_on_settle`.

### E-embers-04

* **ID:** `E-embers-04`
* **Effect:** any player gaining [除外] (or going bankrupt) removes every copy.
* **Executable:** `embers_copies_clear_on_exile`.

---

## 迷子的追逐 (derived one-shot)

**Source:** `data/events.json#迷子的追逐`
**Quote:** 「所有玩家各投5d20。如果抽到此卡的玩家的点数严格大于所有其他玩家的点数则将一张“[衍生]让我来结束一切”背面朝上放置于事件牌堆顶部，否则将一张“[衍生]一切不会结束”背面朝上放置于事件牌堆顶。」

### E-lost-chase-01

* **ID:** `E-lost-chase-01`
* **Effect:** each player rolls 5d20; a strictly-solo-high drawer pushes
  让我来结束一切 face-down on top.
* **Executable:** `lost_chase_strict_high_pushes_end_it_all`.

### E-lost-chase-02

* **ID:** `E-lost-chase-02`
* **Effect:** otherwise 一切不会结束 is pushed face-down on top.
* **Executable:** `lost_chase_not_high_pushes_never_ends`.

---

## 让我来结束一切 (derived, permanent)

**Source:** `data/events.json#让我来结束一切`
**Quote:** 「触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上，此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆顶部」

### E-end-it-all-01

* **ID:** `E-end-it-all-01`
* **Effect:** the drawer picks 3 non-derived events to permanently remove
  (`event_removed`); this card stays in play permanently.
* **Executable:** `end_it_all_stays_permanently_and_banishes_three`.

### E-end-it-all-02

* **ID:** `E-end-it-all-02`
* **Effect:** while it is up, 迷子的追逐 does not push another
  让我来结束一切.
* **Executable:** `end_it_all_blocks_a_second_push`.

---

## 一切不会结束 (derived one-shot)

**Source:** `data/events.json#一切不会结束`
**Quote:** 「抽出此卡的玩家投掷1d4，所有玩家[传送]到对应数字的RiNG且获得1层在回合开始时移除的[除外]。」

「对应数字的RiNG」 is board 「RiNG 1..4」 (tiles 8 / 16 / 36 / 53).

### E-never-ends-01

* **ID:** `E-never-ends-01`
* **Effect:** the drawer rolls 1d4; everyone [传送]s to RiNG N and gains 1
  [除外] (removed at turn start).
* **Executable:** `never_ends_teleports_to_ring_n`.

---

## Open questions

| OPEN | text | notes |
|---|---|---|
| OPEN-duobang-tie | 对邦 multi highest/lowest | 「所有…最高者从所有…最低者」; split among the extremes unspecified |
| OPEN-estate-tie | 弦卷集团地产开发 equal high bids | tie-break unspecified |
| OPEN-estate-empty | 「（如果有）」 | no purchasable tile ⇒ no-op only |
| OPEN-habinel-end | 哈比内尔 「结束后」 | teleport timing unbound |
| OPEN-分摊 | 分摊 | undefined; only the 1000 total is pinned (前场队) |
| OPEN-触发结算 | [触发结算] | undefined in the book body (协助CiRCLE重建, 火种) |
| OPEN-tears-colour | 同色地契 | multi-colour overlap unspecified |
| OPEN-kizuna-song | 卡名为歌名的卡 | not defined in the rulebook body |
| OPEN-chart-rush-or | 2000资金或抽一张卡 | chooser unspecified |
| OPEN-chart-rush-tie | tie tiers | example pins 1,2,2,4; 5th+ unspecified |
| OPEN-fix-note-count | 累计…加盖后 | how many builds unspecified |
| OPEN-heat-head-2 | second 「下回合结束前」 | window bound ambiguous |
| OPEN-surprise-2turn | 下2回合开始时 | 2nd-next reading (EVENTS.md) |
| OPEN-marina-who | 那名玩家 | read as the passer |
| OPEN-marina-default | 可[消耗] default | host choice; TEST-FINDINGS default 「不消耗」 |

---

## Untestable

* 很噜的感觉's shout (out of game).
* EX任务挑战's skin change (cosmetic; skills unchanged).
* 元祖！邦多利酱's host clocks (免费时间 / 恢复时间).
* 协助CiRCLE重建's 优先权 claim (needs a second [触发结算] replacement).
* 发送熊饼表情's fire-pot refill (character-gated X / cap; skill text, not
  event text).

---

## Findings

(Empty -- every executed contract is green against the current engine. Gaps
listed above are OPEN / untestable, not discrepancies.)

---

## Coverage map

| event | kind | clauses | executed |
|---|---|---|---|
| draw / filing | global | E-draw-01..03 | 3 |
| 对邦 | one-shot | E-duobang-01..02 | 2 |
| 弦卷集团地产开发 | one-shot | E-estate-01..03 | 3 |
| 很噜的感觉 | one-shot | E-lulu-01..02 | 2 |
| 前往哈比内尔王国旅游 | one-shot | E-habinel-01..03 | 2 (+1 OPEN) |
| 这只手我不会放开 | one-shot | E-this-hand-01..03 | 3 |
| PICO灵魂交换 | one-shot | E-pico-swap-01..02 | 2 |
| 协助CiRCLE重建 | active | E-circle-rebuild-01..05 | 4 (+1 partial) |
| 前场队还是后场队？ | one-shot | E-front-or-back-01..02 | 2 (分摊 OPEN) |
| EX任务挑战 | one-shot | E-ex-quest-01..02 | 1 (+1 cosmetic) |
| 发送熊饼表情 | one-shot | E-bear-cookie-01..03 | 2 (+1 follow-up) |
| 飞鸟山之战 | one-shot | E-asukayama-01..03 | 2 (+1 tick) |
| 上学时间 | one-shot | E-school-time-01..02 | 2 |
| 泪水的含义 | one-shot | E-tears-01..02 | 2 |
| Kizuna Music | one-shot | E-kizuna-01 | 1 (song X OPEN) |
| 冲榜 | derived | E-chart-rush-01..02 | 2 (or/tie OPEN) |
| 修复公告 | derived active | E-fix-note-01..02 | 1 (+1 count OPEN) |
| 幻觉来了 | active | E-hallucination-01..02 | 1 (+1 follow-up) |
| 卡池BUG | active | E-pool-bug-01..03 | 3 |
| A！A！O！ | active | E-aao-01..02 | 2 |
| 麻里奈小姐的礼物箱 | active | E-marina-box-01..02 | 2 |
| 元祖！邦多利酱 | active | E-bangdream-chan-01..02 | 1 (+1 clocks) |
| 超燃甩头 | active | E-heat-head-01..03 | 3 |
| 意外的对邦 | active | E-surprise-duel-01..03 | 2 (+1 follow-up) |
| Forbidden Moca | active | E-moca-01..02 | 1 (both halves) |
| 火种燃尽之后会怎么样呢？ | active | E-embers-01..04 | 3 (+1 follow-up) |
| 迷子的追逐 | derived | E-lost-chase-01..02 | 2 |
| 让我来结束一切 | derived permanent | E-end-it-all-01..02 | 2 |
| 一切不会结束 | derived | E-never-ends-01 | 1 |
