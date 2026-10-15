# Consolidated open questions (OPEN-*)

Every book gap the clauses must not invent, deduplicated and grouped by the
same concept across units. Sources: `docs/rulebook/spec/README.md` index,
`global/*.md` / `cards/*.md` / `events.md` **Open questions** sections, and
`kani/*.md` OPEN columns. Where a unit poses no explicit question, a neutral
yes/no question is suggested for the rulebook owner (rulebook / sheet).

Snapshot 2026-10-12. A clause that depends on one of these records the OPEN id
in its **Rulings & OPEN** field and specifies only what the text determines.

## Shared / cross-unit

These concepts appear in many units. One question each; unit-local OPEN ids
that mean the same thing are folded in.

### OPEN-触发结算 — [触发结算] undefined

* **Text:** 「[触发结算]」 appears in 行动阶段 row 15; never defined.
* **Depends on:** G-FLOW; crychic grab-02/03, myown-01, sakilead-02,
  rikireroll-01; r NFO-04, live前的准备-02, 学生会的检查-01/05, 蓝玫瑰-03,
  来练习吧-06, 慈爱女神-03, 选择自己的舞台-02; mujica 「并/不[触发结算]」
  walk clauses; ras C-RAS-band-02; sumimi; mygo; hhw parade-01..05 / kkr-03;
  mor 高贵的微蓝 / 迷茫的庭园; ag ONE OF US (1); E-circle-rebuild, E-embers.
* **Also listed as:** OPEN-HHW-触发结算 (same concept).
* **Suggested question:** Is [触发结算] exactly "resolve the landing settle
  window as if the piece had just landed (rent, CP, trigger cards)"?

### OPEN-分摊 — 分摊 / 收益平分 undefined

* **Text:** 「分摊」 never defined. Only the 1000 total is pinned (前场队 /
  Pareo).
* **Depends on:** G-MONEY (generally); E-front-or-back-01..02; ag tiny-can /
  ONE OF US; crychic vocalist-01 (「一次性」 excludes it); ras
  C-RAS-pareo-skill-02; r NFO face 2; ppp band (3); pp chisato-mask-02/03;
  mujica 八幡海铃 状态2.
* **Suggested question:** When several players share one payment, is the
  amount split equally among the named payees, and does any remainder go to
  the first in turn order?

### OPEN-场上 — [场上] undefined

* **Text:** used as 「自己[场上]N个[CP点]」; never defined as a zone.
* **Depends on:** ag ONE OF US partner; ppp wait-aoki-01; sumimi 「对方场上」;
  r NFO face 5 (character card as crystal host).
* **Suggested question:** Does 「场上」 mean only that player's field cards
  (not hand / deck / discard / board tiles)?

### OPEN-强制停下 — [强制停下] definition truncated

* **Text:** `rulebook-doc.md:93` definition is truncated.
* **Depends on:** G-MOVE (path / end rewrite); mygo rana-fun-03/04; pp flower;
  ppp random-star-02 / maze-04; r.
* **Suggested question:** When a per-tile 强制停下 and a settle-before
  end-rewrite both want to change the destination, which wins? And does
  [强制停下] end the walk without [结算] of the stop tile unless the effect
  says so?

### OPEN-乐队卡 — 「乐队卡」 undefined as a card class

* **Text:** bankruptcy removes them; counteract filter mentions them.
* **Depends on:** G-BANKRUPT-05; G-COUNTER-06; mygo miracle-01; pp; ppp
  (团卡 = band-skill instance in specs).
* **Suggested question:** Is 「乐队卡」 exactly the identity/band card each
  player holds at setup (and not other card kinds)?

### OPEN-收费标价 — 「收费标价」 undefined

* **Text:** Sweet Escape gate (TEST-FINDINGS §6.5). Engine now reads tile
  **price** sum (8200); sumimi notes also try charge/rent (RiNG 基础乘数).
* **Depends on:** G-SETTLE (Sweet Escape gate); sumimi sweetescape gate /
  littlehina-02; tiles.
* **Suggested question:** Does 「收费标价」 mean the tile's purchase price
  (sum of prices), not the marked base rent?

### OPEN-结算定义 — [结算] defined twice

* **Text:** `rulebook-doc.md:61` and `:87`.
* **Depends on:** G-SETTLE-01+; ppp 「不触发结算」 (signpost etc.).
* **Suggested question:** When both definitions apply, is the 专有名词
  definition (execute every effect on the tile) the one that governs a full
  settle? Does 「不触发结算」 mean no settle fires at all?

### OPEN-回合流程 — 回合流程 rows 1-3 corrupt

* **Text:** use **行动阶段** (`rulebook-doc.md:199+`) instead.
* **Depends on:** G-FLOW-01+.
* **Suggested question:** Should the 回合流程 table be restored to match
  行动阶段 rows (so rows 1-3 are not corrupt)?

### OPEN-支付阶段7 — 支付阶段 step 7 placement

* **Text:** `rulebook-doc.md:235` 「合并到[支付后]?」.
* **Depends on:** G-MONEY (pay stages); G-FLOW; 振翅高飞的练习曲 (1) row-7
  trigger.
* **Suggested question:** Is 支付阶段 row 7 merged into 支付后 (one window),
  or is it a separate window after 支付后?

### OPEN-取整 family — rounding of halves / 1.5× / 60%

**RESOLVED — user ruling 2026-10-15.** Full write-up in
`docs/rulebook/spec/ROUNDING.md`.

**Ruling:** money rounding (up to a multiple of 10) is performed **once**,
**after** all add/mul modifiers of an amount have been processed. Modifier
cards transform the running amount exactly and must **not** CeilTo-10 inside
their own body. Printed 「向上取整10」 on a formula **is** that end-of-calc
rounding. An amount no modifier touched is left as the base formula produced
it.

| OPEN id | where | resolution |
|---|---|---|
| **OPEN-60%舍入** | G-MONEY-06 | **still OPEN** (valuation, not a running-amount modifier; all shipped prices × 0.6 are integers) |
| **OPEN-半价取整** / **OPEN-减半取整** | ag half-speed / glory / sunset; mujica 祥 移动 支付价格减半 | **RESOLVED for running modifiers:** fold is exact (integer `.5` up), then one `round10` at settle. Glory / sunset X are base formulas (unchanged; sunset's pre-100 half of X stays OPEN-取整顺序). |
| **OPEN-取整** | mujica 团技能 / 祥 | **RESOLVED:** 1.5× is an exact fold; one `round10` at settle. 祥's `set_pay_factor(500)` rides `scale_settle_payment` and gets the same end round. |
| **OPEN-ROUND10** | r louder / fire-bird | **RESOLVED for fire-bird:** ×1.5 fold + one end `round10`. louder is not a pay-amount modifier. blue_rose 20% is a **gain formula** (kept; OPEN as a printed-formula rounding question if ever non-integer). |
| **OPEN-half-rent** | ppp 香澄 everyone-c2 | **RESOLVED as factor path:** `set_rent_factor(500)` truncates the scale; the engine's one end `round10` applies when the scale rewrote the figure. Sole-modifier result matches `round10(exact half)`. |
| **OPEN-取整顺序** | ag sunset-01/03 | **still OPEN** (X is a printed base formula, not a running-amount modifier) |
| **OPEN-减半取整** vs **rainbow CeilTo-10** | mor `rainbow_01_half_iff` | **RESOLVED:** 「减半」 is an exact fold (no CeilTo in the card); `final = round10(fold)` (301 → 150.5 → 160). Kani pins updated. |
| **OPEN-抵押半价取整** | G-MONEY-05 / G-BANKRUPT-04 | **still OPEN** (mortgage credit is a valuation, not a running-amount modifier) |

**Rainbow-card specific (CANDIDATE from mor HOSTOPS):** **RESOLVED** —
「金额减半」 is the fold `amount/2` (integer `.5` up); the settled figure is
`round10(fold)` (301 → 150.5 → 160). See ROUNDING.md.

**Still OPEN in this family:** multi-fractional-stack precision (integer fold
can diverge from the rational fold when several halves / 1.5× stack);
OPEN-60%舍入; OPEN-取整顺序; OPEN-抵押半价取整; charity_show's printed
CeilTo when another modifier stacks after it.

### OPEN-同色 / OPEN-Agent同色 — colour-group widening

* **Text:** 「每种颜色拥有一个地产商格子」; does `extraColor` /
  「该格获得所有颜色」 widen the set?
* **Depends on:** G-MONEY agent half-charge; G-SETTLE-13..14; mygo
  soyo-mixed-02 / soyo-skill-02 (OPEN-agent-colour).
* **Suggested question:** Does a colour override (「该格获得所有颜色」) make
  that tile belong to every colour group for the agent's half-charge set?

### OPEN-视为房屋数 — 「房屋数视为…」 overrides

* **Text:** vs counted house count (Ringing Bloom (3) X; TEST-FINDINGS §6.3).
* **Depends on:** G-MONEY rent house count; G-SETTLE-10; r OPEN-RINGING-X.
* **Suggested question:** Does an effect that says 「房屋数视为 N」 change
  X / rent house-count reads to N (counted), or do those reads use the real
  house count?

---

## Global

### Money / purchase

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-RiNG地租** / **OPEN-RING-RENT** | RiNG deeds have `rent: []`; book only says 「所有RiNG不可升级」 | G-MONEY rent; G-SETTLE-10 | When a player settles on a RiNG owned by another player, is there any rent at all (yes/no), and if yes is it the engine formula 「RiNG数 × ringMultiplier × 1d20」? |
| **OPEN-强制购买支付钩** | may a 「写明改动强制购买」 effect retune the force-buy sum? Engine pays force-buy outside the money pipeline | G-MONEY-08; G-SETTLE-11 | May an effect that explicitly says it changes 强制购买 modify the force-buy amount? |
| **OPEN-AGENT-MORTGAGED-HALF** | agent half-charge sweep vs a mortgaged member (no rent) | G-SETTLE-13 | In the agent's all-owned half-charge settle, is a mortgaged member skipped entirely (no half force-buy)? |
| §6.11 RiNG mortgageability | engine refuses (`err.mortgage_ring`); book 游戏流程 5 says 「抵押拥有的地契」 with no exclusion | G-MONEY-05; G-BANKRUPT | Is a RiNG deed mortgageable? |
| §6.13 巴's 「收购」 vs 「购买」 + mortgage | engine: acquisition is a purchase; commits un-mortgage | G-SETTLE / purchase | Is a priced 「收购」 a 「购买」 for 「购买…时」 listeners, and does it clear the mortgage? |

### Cards / status / move / settle / flow / bankruptcy / counteract

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-CARDS-LIMIT-X** | `[限]X` play gate; AG:即使夕阳落山 prints X as a modifier | G-CARDS-06 | For 「即使夕阳落山」's `[限]` text, is the printed X halved (modifier) rather than used as a boolean play condition? |
| **OPEN-CARDS-START-HAND** | baseline start hand 2; `startHand` / `deckBeforeGame` can lower it | G-CARDS-09 | Is the opening hand always 2 unless a skill or card effect says otherwise? |
| **OPEN-晕眩-自得** | does 「无法收付款」 block a card's own 「获得」 to the player that same body just stunned? Engine: gain lands | G-STATUS-04 | After a card stuns a player in its own body, may that same body still 「获得」 money to that stunned player? |
| **OPEN-除外-归位** | is a [除外] grant with no named return tile legal? | G-STATUS-08 | Must every [除外] grant name a return tile (illegal without one)? |
| **OPEN-异常移动效果-封闭** | is the seven-item abnormal-move list closed (「包括」)? | G-STATUS-10 | Is the list of 异常移动效果 closed (only those seven kinds)? |
| **OPEN-停留-中途** | mid-move [停留] does not stop the current move (glossary 51) | G-STATUS-02; G-FLOW-10 | Does a [停留] gained after the main move has started stop that move immediately? |
| **OPEN-经过-0步** | 0-step move: book puts the tile in [路径] but 行动阶段 13 names only [重叠] | G-MOVE-02; G-MOVE-10 | Does a 0-step move also fire [经过] on the standing tile, or only [重叠]? |
| §6.1 Teleport-as-main | teleport that *is* the main move: settle like main landing or like any teleport? Engine: like any teleport | G-MOVE-09 / G-SETTLE-06 | Should a teleport used as the main move settle like a main-move landing (announce + end-step buy offer)? |
| **OPEN-平局重投** | order log only carries final rolls | G-FLOW-13 | When only some players tie, do only those tied players re-roll (not everyone)? |
| **OPEN-娱乐终局** | 娱乐 ends 「可在所有玩家同意时」 | G-FLOW-17; G-BANKRUPT last-alive | In 娱乐 mode, is the game **only** allowed to end by unanimous consent (last standing is not an automatic end)? |
| **OPEN-模式映射** | 正规 / 娱乐 vs engine Solo/Casual/Ranked | G-FLOW-12/14; G-BANKRUPT | Is 娱乐 the Casual mode and 正规 the Ranked mode (Solo is practice only)? |
| **OPEN-可折现资产** | 「可折现的资产」 set undefined (houses? deeds?) | G-BANKRUPT-03..04 | May houses on a bankrupt player's deeds be liquidated for cash (in addition to mortgaging deeds at 50%)? |
| **OPEN-棋子移出** | 「棋子移出游戏」; `World.pos` is the only piece slot | G-BANKRUPT-05 | When a player goes bankrupt, is their pawn removed from the board (position cleared / unoccupiable)? |
| **OPEN-可消耗资金** | auction eligibility gate (cash only? cash + mortgageable?) | G-BANKRUPT-07 | May a bidder with less cash than the bid still bid if they could mortgage deeds to cover it? |
| **OPEN-拍卖最高额** | 「最先喊价最高金额」 | G-BANKRUPT-07 | If two seats bid the same amount, does the first to bid that amount win? |
| **OPEN-拍卖抵押状态** | does a deed won at auction stay mortgaged? | G-BANKRUPT-07 | Does a deed transferred by auction keep its mortgage status? |
| **OPEN-拍卖房屋** | houses on an auctioned deed? | G-BANKRUPT-07 | Do houses on an auctioned deed stay on that deed for the winner? |
| **OPEN-拍卖无人出价** | nobody bids; book silent | G-BANKRUPT-07 | If nobody bids, does the deed stay with the leaving player's estate (or go unowned / to the bank)? |
| **OPEN-离开资金** | no cash-out for 「离开」 (unlike 破产) | G-BANKRUPT-08 | When a player leaves voluntarily, do they take no cash (unlike bankruptcy which pays the creditor)? |
| **OPEN-特** | 「特殊情况」 undefined (`rulebook-doc.md:81`) | G-COUNTER-13 | Is 「特殊情况」 limited to effects that apply before the game starts (plus other cases the book will name later)? |
| **OPEN-自我反击** | may a player counteract their own link? | G-COUNTER-02 | May the player whose action opened a [反击] window play a [反击] card into that same window? |

---

## Events

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-duobang-tie** | 对邦 multi highest/lowest split among extremes | E-duobang-01..02 | If several players share the highest (or lowest) roll, is the money split among all of them equally? |
| **OPEN-estate-tie** | 弦卷集团地产开发 equal high bids | E-estate-01..03 | If two players bid the same highest amount, is the deed awarded to the earlier seat in turn order? |
| **OPEN-estate-empty** | 「（如果有）」 no purchasable tile ⇒ no-op only | E-estate-02 | If there is no purchasable tile, does 弦卷集团地产开发 do nothing (no other effect)? |
| **OPEN-habinel-end** | 哈比内尔 「结束后」 teleport timing unbound | E-habinel-03 | Does 「结束后」 mean immediately after the event body (not after the exile countdown)? |
| **OPEN-tears-colour** | 泪水的含义 同色地契 multi-colour overlap | E-tears-01..02 | If a deed is in more than one colour group, does it count as same-colour for both players' checks? |
| **OPEN-kizuna-song** | 「卡名为歌名的卡」 not defined | E-kizuna-01 | Is a card a 「卡名为歌名的卡」 when its printed name is exactly a song title (no other words)? |
| **OPEN-chart-rush-or** | 「2000资金或抽一张卡」 chooser unspecified | E-chart-rush-02 | Does the ranked player choose whether to take 2000 funds or draw a card? |
| **OPEN-chart-rush-tie** | tie tiers: example pins 1,2,2,4; 5th+ unspecified | E-chart-rush-02 | If more than four players tie for the same rank tier, do all of them receive the same purse as the example's tied seats? |
| **OPEN-fix-note-count** | 「累计…加盖后」 how many builds unspecified | E-fix-note-02 | Does 修复公告's half-price apply to an unlimited number of builds while it is in play (not just one)? |
| **OPEN-heat-head-2** | second 「下回合结束前」 window bound ambiguous | E-heat-head-02..03 | Does the second phase's 「下回合结束前」 cover exactly the next full turn (including its end), the same as the first phase? |
| **OPEN-surprise-2turn** | 「下2回合开始时」 = 2nd-next reading | E-surprise-duel-02 | Does 「下2回合开始时」 expire at the start of the second next turn (not the first)? |
| **OPEN-marina-who** | 「那名玩家」 read as the passer | E-marina-box-02 | Is 「那名玩家」 the player who just passed CiRCLE (the one who paid the optional 500)? |
| **OPEN-marina-default** | 「可[消耗]」 default; TEST-FINDINGS default 「不消耗」 | E-marina-box-02 | When the optional spend is declined or unanswered, is the default 「不消耗」? |

---

## Cards / skills (unit-local)

Grouped by band. Shared concepts (触发结算 / 分摊 / 场上 / 强制停下 / 乐队卡 /
取整) are above; only unit-local ids remain.

### AG

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-起始地点** | 「起始地点」 of AG:(兰) 像往常一样 (2) | C-AG-as-usual-02 | Is 起始地点 CiRCLE, the opening seat tile, or the pre-move position? |
| **OPEN-像往常一样-undo** | 「取消所有受到的效果」 scope | C-AG-as-usual-02 | Does undo also reverse money already paid / cards already discarded? |
| **OPEN-少于四格** | sunset with fewer than four housed deeds | C-AG-sunset-02 | If fewer than four housed deeds exist, is the designation set all of them, or illegal? |
| **OPEN-传送主要移动结算** | teleport-as-main settle | childhood / §6.1 | Does a teleport that *is* the main move open a normal landing settle? |
| **OPEN-afterglow式移动** | 「初始Afterglow角色的（2）」 | detour-02 | Which character's (2) is "afterglow-style"? |
| **OPEN-@Tsugu / OPEN-@Tsugu-ycm** | garbled sheet mentions | tiny-can-01/02 | Are these typos for a real card id? |
| **OPEN-每三回合** | fire-pot 「每三回合没有使用」 | fire-pot | Whose turns count the three? |
| **OPEN-2次cd** | moca skill 「2次使用cd」 | moca skill | Per player / per skill / per card instance? |
| **OPEN-每圈** | tomoe (3) 「每圈限一次」 | tomoe skill (3) | How is a lap counted? |

### CRYCHIC

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-收款** | 「收款」 undefined | destiny-01 | Is 收款 only rent, or any inbound transfer? |
| **OPEN-行动序列后一名** | next-player tile search | destiny-01 | Clockwise next living player only? |
| **OPEN-春日影抽6** | each draw-to-6 iteration replaceable? | haruhikage-01 | Is every intermediate draw up to 6 a replaceable draw? |
| **OPEN-两个X差值** | 「两个X差值*120」 formula | tomoricry-01 | Is it abs(X1−X2)*120? |
| **OPEN-四到五** | 3→5 jump as 「从4增加至5」? | band-03 | Does the crystal only fire on landing exactly on 5 from 4? |

### HHW

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-HHW-DIR** | 1d4mod4 → 上/下/左/右; face 0; 「直线距离最远」 | C-HHW-hlsy-01 | How does face 0 map; is "farthest" Euclidean board distance? |
| **OPEN-HHW-SHORT-PILE** | draw pile < 2 on 出发！后台之旅！ | backstage | Draw fewer or reshuffle first? |
| **OPEN-HHW-BELIEVE-CHOOSER** | who chooses discarded card | believe-03 | The drawer or the opponent? |
| **OPEN-HHW-RESHUFFLE** | reshuffle of empty-remainder pile | deck clauses | Reshuffle discard into draw mid-effect? |
| **OPEN-HHW-PATROL-PRICE** | 「付款」 amount on 微笑巡逻队 | patrol-02 | Printed build cost, table cost, or free? |
| **OPEN-HHW-PATROL-3D20** | 3d20 as sum vs three tiles | patrol-02 | Is 3d20 one sum for one build, or three separate tiles? |
| **OPEN-HHW-BUILDABLE** | 「可建造格子」 | patrol-02 | Is it exactly [可购买格子]? |
| **OPEN-HHW-RETURN-AMOUNT / OPEN-HHW-RETURN-BOOK / OPEN-HHW-RETURN-RELIEF** | pay amount; crystal 「records」 a player; relief pre/post double | return-02..05 | Is relief half of the doubled amount or of the base? |
| **OPEN-HHW-CHARITY-RECOMPUTE** | 「初次经过」 after path recompute | charity-01 | Does recompute reset "first pass"? |
| **OPEN-HHW-KKR-OWNER** | 「所有者」 of CiRCLE-placed card | kkr-03 | The placer or the tile owner? |
| **OPEN-HHW-NEIGHBOUR** | 「相邻2格」 | misaki | Two tiles away or the two adjacent tiles? |
| **OPEN-HHW-HAGUMI-TIME** | 「投掷次数+1」 | hagumi-03/04 | +1 die or +1 to each roll? |
| **OPEN-HHW-BACKWARDS** | 「倒走」 | wacha-01 | Direction on a ring board? |
| **OPEN-HHW-CHARGE** | 「充能3，衰减1」 counter name | athletic | Shared or per-card counter? |
| **OPEN-HHW-KANON-NEG** | negative difference on 花音 (2) | kanon-02 | `simple` says signed; `text` does not — which wins? |
| **OPEN-HHW-BAND-RECORD** | band skill 「记录」 storage | band-01/02/03 | Is 「记录」 a sticky player set? |
| **OPEN-HHW-TEXT-VS-SIMPLE** | `characters.json` `text` vs `simple` conflicts | several HHW skills | Does `text` always beat `simple`? |

### MOR

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-移动起点** | 「此次移动」 replace main move or free move? | 你的光芒将照亮前路 | — |
| **OPEN-取消范围** | [持续] 「取消此次资金变动」 one-sided or both? | 再次牵起手来 | — |
| **OPEN-指定** | designation | 夏日合宿 / 高贵的微蓝 | Is 指定 the engine's designate API? |
| **OPEN-无效化** | 「无效化你受到的所有效果」 | 离心力 | Does it cancel only new effects or also ongoing? |
| **OPEN-X** | 蝴蝶飞舞的星月夜 X range | starry | Is X 1..=5? |
| **OPEN-场地中央** | board slot? | starry | — |
| **OPEN-重骰次数** | second result final? | starry | — |
| **OPEN-三全音收款人** | payee (bank vs original) | tritone-02 | — |
| **OPEN-角色标记 / OPEN-X标记** | mark namespace | NNM, 筑紫 | — |
| **OPEN-多分支选择** | NNM 「发动以下效果中的一个」 vs multi-activation | NNM-01 (TEST-FINDINGS §6.8 / C-Q6) | Must exactly one branch fire? |
| **OPEN-借用技能** | 「你的（2）技能」 / 「视为使用该角色的技能」 | nanami / rui | — |
| **OPEN-地租档** | 「此地的地租」 | 桐谷透子 (2) | Which rent step? |
| **OPEN-被动CD** | 3-turn CD of passive-skill mark claim | nanami | — |
| **OPEN-行动条 / OPEN-异常移动** | 行动条; [异常移动效果] set | garden-04 | — |
| **OPEN-复制** | skill copy | rui-crit-01 | — |
| **OPEN-双方影响** | 「收款或付款方同样受到」 | 八潮瑠唯 (2) | — |
| **OPEN-主动移动** | [主动移动] vs [主要移动] | 仓田真白 | Same term? |

### MUJICA

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-花火额外回合** | does **last** crystal also grant extra turn? | 花火-02/03 (TEST-FINDINGS §6.2 / C-Q26) | Does the last-crystal removal also grant an extra turn? |
| **OPEN-花火眩晕掉层** | stun layer vs turn-end remove-layer order | 花火-03 | Must 2 layers remain after both the grant and the turn-end tick? |
| **OPEN-前方20格** | 「前方」 = clockwise? | doll garden | — |
| **OPEN-箱庭0步** | sum-0 forced walk still [主要移动]? | 箱庭 | — |
| **OPEN-喵梦翻面** | does paying 2 fire cancel the face-down flip? | 喵梦 | Paying names cost only — any benefit? |

### MYGO

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-t04** | Anon Tokyo link + mortgaged partner 收费 | anon-tokyo-04 | What is charged on a mortgaged partner tile? |
| **OPEN-G13** | copied 移动格数 keeps sign? | normal-02 | — |
| **OPEN-miracle-pct** | 「资金后三位」 | miracle-03 | Is it `money % 1000`? |
| **OPEN-agent-colour** | 「该格获得所有颜色」 | soyo-mixed-02, soyo-skill-02 | (see OPEN-同色) |
| **OPEN-rainday-d10** | 「骰点大于10」 on 1d10 | rainday-04 | Needs a modifier, or wrong die? |

### PP

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-共鸣** | [共鸣] undefined | dream-ahead, together, flower, etc. | — |
| **OPEN-mark** | 单色/双色标记 + colour table | guide-02/06 | — |
| **OPEN-substitution / OPEN-substitution-effect** | which exclusive substituted; what 「改变」 replaces | hina-sound-01/02 | — |
| **OPEN-fan-face** | which fan face flipped when both exist | fan-flip cards | — |
| **OPEN-window2** | 「生效2次」 window | — | Until next payment or turn end? |
| **OPEN-ties** | fewest-money tie | — | All tied seats or one? |
| **OPEN-floor** | remove 4 crystals when fewer exist | — | Remove all remaining? |
| **OPEN-div** | 「粉丝数量」÷3 rounding | — | Integer division? |
| **OPEN-tiles** | 「bandori车站」 spelling | — | Same as board `Bandori车站`? |

### PPP

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-wait-causality** | 「非因为此卡导致的事件」 | wait-aoki-01 | — |
| **OPEN-door-hill** | co-owned hill as 「[拥有者]拥有的格子」 | door-03 (TEST-FINDINGS §6.6) | Does a co-owned tile count as owned by each co-owner? |
| **OPEN-3d20-slot** | 3d20 slot semantics | band / random-star | — |
| **OPEN-status-name** | status name keys | several | — |
| **OPEN-immunity** | 「不受任何其他效果影响」 | several | — |

### R

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-RING-BUILD** | card 「加盖一层房屋」 vs 「所有RiNG不可升级」 | 向着顶点-02 | Which text is live? |
| **OPEN-STUDENT-REFUND** | 「本卡原应拿到的资金」 | 学生会的检查-07 | Refund amount? |
| **OPEN-RING-MULT** | 「ring的价格基础乘数」 | louder | — |
| **OPEN-RINGING-X** | X after 「视为」 | Ringing Bloom (3) | (see OPEN-视为房屋数) |
| **OPEN-SAYO-TIMING** | 「时机合适时」 | 弹奏 skill | — |
| **OPEN-R1** | Fire bird 1.5× stacking | fire-bird | Stack or replace? |
| **OPEN-FIRE-X** | Fire bird 「X（自选）」 range | fire-bird | — |
| **OPEN-BAND-PRETEND** | band (2) 「视为 Live House」 | band-02 | — |

### RAS

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-livehouse** | 「livehouse 格子」 / Live House colour | almost every clause | Is it board `group == 6`? |
| **OPEN-exist-influence / OPEN-exist-multi** | 「造成影响」; multi/self targets | exist-02/04 | — |
| **OPEN-please-choose-qual** | 「可将你指定为目标的牌」 | please-choose-02 | — |
| **OPEN-repaint-round** | odd half | repaint-03 | (see OPEN-取整) |
| **OPEN-unstoppable-mod** | 「1d6mod6」 vs 「结果1-6」 | unstoppable-card-01 | — |
| **OPEN-become-strongest-table** | board-wide vs owned-only livehouse | become-strongest-01 | — |
| **OPEN-hey-kids-split / OPEN-hey-kids-sum** | house-split grammar; cost-sum scope | hey-kids-03/04 | — |
| **OPEN-storm-distance / OPEN-storm-price** | distance metric; 「普通[结算]的价格」 | storm-03 | — |
| **OPEN-crush-immune** | 「不受任何其他效果影响」 | crush-01 | — |
| **OPEN-chuchu-which-gain** | which 「获得资金」 +100 rides | chuchu-04 | — |
| **OPEN-band-opt-timing** | 「每回合可选择」 | band-01 | — |
| **OPEN-change-world-equal** | 「获得等量资金」 binding | change-the-world-04 | — |

### SUMIMI

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-C-SUMIMI-nobreakup-digits** | 「拥有相同数字」 | nobreakup | All digits equal, or any repeated digit? |
| **OPEN-C-SUMIMI-singing-redirect** | [反击] self-redirect substitution | singing | — |
| **OPEN-C-SUMIMI-heretheworld-facdown** | face-down storage | heretheworld | — |
| **OPEN-回合** | 「回合」 scope | several | — |

### GEN (generic cards)

| OPEN id | description | depends on | suggested question |
|---|---|---|---|
| **OPEN-GEN-recruit-once** | 「1回限定」 not in body | C-GEN-recruit-01 | Is the once-per-game limit enforced by the engine play gate? |
| **OPEN-GEN-fever-floor** | source `max(0, …)` vs printed 「提高」 | C-GEN-fever-02 | Does 「提高X」 ever floor at 0, or can it go negative? |
| **OPEN-GEN-budokan-Y** | Y printed, unused | C-GEN-budokan-01 | Is Y a leftover / does it modify X? |
| **OPEN-GEN-marina-extra** | 「正常进行而不受到其上的额外效果」 | C-GEN-marina-01 | Which extra effects are suppressed? |
| **OPEN-GEN-parking-houses** | excess houses | C-GEN-parking-01 | What happens to houses beyond the transfer cap? |
| **OPEN-GEN-net-which-target** | 「取消其对目标之一的[指定]」 | C-GEN-net-01 | Which target is cancelled when several are designated? |
| **OPEN-GEN-net-event** | printed text garbled | C-GEN-net-03 | Please re-print the event-card sentence. |
| **OPEN-GEN-clearcp-neighbour** | 「相邻」 CP spread | C-GEN-clearcp-03 | Adjacent tiles only, or also players on them? |
| **OPEN-GEN-clearcp-window** | 「下2回合开始时」 window | C-GEN-clearcp-03 | (see OPEN-surprise-2turn) |
| **OPEN-GEN-tsugu-teleport-main** | teleport vs main move + settle | C-GEN-tsugu-02/04 | (see §6.1 Teleport-as-main) |
| **OPEN-GEN-tsugu-3-order** | order of ≥26 / ≥28 / <26 branches | C-GEN-tsugu-03 | Are the thresholds checked top-down, highest first? |

---

## Related rulings (TEST-FINDINGS §6 / purchase surface)

Book often silent; "Engine now" is the recorded choice. Listed for the owner
to confirm or overturn (CANDIDATE where the source poses no question).

| item | summary | suggested question |
|---|---|---|
| §6.1 Teleport-as-main | see above | — |
| §6.2 燃尽前的线香花火 | last crystal extra turn? | see OPEN-花火额外回合 |
| §6.3 Ringing Bloom (3) X | counted vs real houses | see OPEN-视为房屋数 |
| §6.4 Two halvings | stack to ¼ or one window? | Do two different half-pay clauses on one settle stack to ¼? |
| §6.5 Sweet Escape | see OPEN-收费标价 | — |
| §6.6 Tomorrow's Door co-owned hill | see OPEN-door-hill | — |
| §6.7 Draw capture vs auto-play | capture-before-auto-play | When several effects respond to one draw, is the drawn effect card captured by one effect before any auto-play? |
| §6.8 Multi-activation | first `On::Play` only | Should a card with several On::Play entries prompt which activation to run? |
| §6.9 晕眩 self-gain | see OPEN-晕眩-自得 | — |
| §6.10 Agent 同色 | see OPEN-同色 | — |
| §6.11 RiNG mortgageability | see above | — |
| §6.12 Force-buy pay hook | see OPEN-强制购买支付钩 | — |
| §6.13 巴's 「收购」 | see above | — |
| Purchase R3–R8, R10 | mortgage-to-fund buy; agent pick; ANY_COLOR; force-buy as 「购买」; auction as 「购买格子」; 巴's 「常规收购价」; 「本回合」 linger | CANDIDATE each (TEST-FINDINGS Purchase surface TODOs) |

## See also

* `FINDINGS.md` — book-vs-implementation discrepancies (not book gaps)
* `README.md` § Open questions (index) — original short index
* Per-area **Open questions** / **Coverage gaps** sections
* `docs/rulebook/TEST-FINDINGS.md` §6 rulings
* `docs/rulebook/BACKLOG.md` C-Q1…C-Q43
