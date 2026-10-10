# Settle stages — where each settle-adjacent effect belongs

Design audit, 2026-10-07. **Status: implemented (ABI 43).** The stage model
(§7) and the §4 re-homings are in; each item below is marked with what landed.
The `TODO(规则书) PIPELINE-AUDIT Q7` marker is gone — Q7 is ruled and
implemented.

Terminology (project rule): the [反击] mechanism is **counteraction**
(counteraction window / ring). Passive automatic listeners are **hooks**. The
external spec says "reaction" — translate. Never say "reaction" for our engine.

Reference data (external project, **not** a spec for us):
`target/scratch/specs/TILE_RESOLUTION_PIPELINE.md`,
`EFFECT_ACTIVATION_RESOLUTION_RESPONSIBILITY.md`,
`PLAYER_MOVEMENT_LOGIC.md`, `PLAYER_MOVEMENT_RESPONSIBILITY.md`.
Ground truth: `data/rules.txt` and the 规则书 timing tables in
`docs/rulebook/rulebook-doc.md` (时点流程 / 行动阶段). C# is not a spec.

---

## 1. The rulebook's stages (ground fact)

行动阶段 (`rulebook-doc.md` rows 12–16) is the whole movement/settle timing
spec. Map every card's timing words onto it:

| 行动阶段 | 适用字段 (timing words in card text) | What happens | Example in the table |
| :---: | --- | --- | --- |
| 12 | 「移动时」/「[经过]」 | walk steps; [经过] per path tile; 传送 fires [经过] at the destination | 移动起点不触发[经过], 移动终点触发[经过] |
| 13 | 「移动后」/「[重叠]」 | the move is over; [重叠] at 移动终点 | 要乐奈（3）「你与其他玩家重合时」 |
| 14 | 「[结算]前」/「[触发结算]前」/「[移动终点]」 | pre-settle window | 高松灯（2）「在该玩家[触发结算]前强制将该玩家[传送]…」 |
| 15 | 「触发结算」/「[结算]时」/「在…格子上[结算]时」 | **执行格子上的所有效果** — base *and* card-driven (专有名词 5) | 落点效果 |
| 16 | 「[结算]后」/「[触发结算]后」/「主要移动阶段后」 | after the settle / after the main-move phase | — |

Two more book facts that decide the model:

* **L6 「技能和卡的效果优先于规则书中的所有规则」** — a card effect overrides
  the book's own rules. The settle's effect list is therefore *replaceable*,
  not "default first, then specials".
* **专有名词 5 「[结算]：执行格子上的所有效果，包括基础效果以及技能或卡所导致
  的效果」** — [结算] is a **list** of effects on the tile, not a default
  function followed by a responder window.
* **其他规则注意事项 1.2 「技能或卡所导致的[主要移动]会表明到达终点后是否
  [结算]」** — 「是否[结算]」 gates only the settle (stages 14–15), not the
  movement tail (12–13) and not stage 16.
* Stage 12's note: a [主要移动] effect that says 「跳过/不触发结算」 jumps to
  stage 16. Read against E14 (「移动终点触发[重叠] 原地[传送]/移动(移动0格)时
  触发[重叠]」) this skips the *settle* stages (14–15), not 重叠 — see
  [Ruling R1](#rulings).

## 2. Our stages today (`crates/game-core/src/engine/play.rs`)

> **STACK-01 (2026-10-10):** the stages below are unchanged, but they now run
> as a **work-stack state machine** (`step_settle` / `engine/work.rs`) so a
> nested `card_settle_at` suspends and resumes instead of recursing. Order is
> the old synchronous nest (the nested settle finishes before the outer
> settle's next stage). See `docs/ENGINE.md` "Settle work stack".


```
walk, per step:            passBefore → passTile → pass          (行动阶段 12)
  teleport (resolve):      passTile → passPlayer → after_walk
  teleport (!resolve):     teleported → moveResolved              (nothing else)
after the walk:            passPlayer  (重叠, one per other player)
after_walk:
  if resolve:              settleBefore                            (14)
                           settle        (declaration + [反击] window) (15 opening)
                           settleBody    (chain link: field replace
                                          + tile rule instances)    (15 body)
                           settleAfter                             (16's 「结算后」 half)
                           tileResolved  (terminal, also on cancel)
  always:                  moveResolved                            (16's 「主要移动阶段后」 half)
```

`settle_at` (`play.rs:1323-1360`) threads one `at` through the whole pipeline.
`settle` cancel → no body, no `settleAfter`, but `tileResolved` still fires
(complete-as-nothing). `settleBody` cancel → skip the body only, `settleAfter`
still runs. There is **no `moveAfter`** kind; `moveResolved` fires *after* the
settle, not at the move's end. `tileResolved` / `moveResolved` currently have
**zero** rules wired to them.

Tile bodies (`rules/tiles/src/*`, `On::Settle`): `tile:property` / `tile:ring`
= the property default (purchase / build / rent / force-buy);
`tile:agent` = the agent sweep; `tile:circle` / `tile:edogawa` = the landing
draw; `tile:event` = draw + event. `tile:circle` also has a **Pass entry**
(`On::Hook(&[HookKind::PassTile])`) for the [经过] CiRCLE reward — that one is
a 经过 effect and correctly outside the settle.

## 3. Inventory — every rule on a settle-adjacent kind

`wired` = the kind it declares now (file:line of the `On::` registration).
`text names` = the stage its 规则书 words actually point at.
Flags: **MOVE** = really a move-end effect; **BODY** = really part of the
settle's effect list (or a body replace); **AT** = 「…[结算]时」 but wired
after; **PASS** = 经过-vs-重叠 confusion; **OK** = wired where its text names.

### 3.1 `settleBefore` (行动阶段 14 「[触发结算]前」)

| rule | quote (规则书 timing words) | wired | text names | flag |
| --- | --- | --- | --- | --- |
| `skill:高松灯:诗超绊` （2）（3） | 「当其他玩家的[移动终点]位于您前后5格内时…在该玩家[触发结算]前强制将该玩家[传送]至自己所在的格子」/「在[触发结算]前将自己[传送]至那名玩家所在的格子」 | `HookKind::SettleBefore` `tomori_poem.rs:36` | 14 「[触发结算]前」 (condition anchored on 「[移动终点]」) | OK |
| `skill:丰川祥子（CRYCHIC）` （2）（3） | 「其他人的主要移动结束时，[触发结算]前若在你的前后5格内…」/「你的主要移动结束时，[触发结算]前…」 | `HookKind::SettleBefore` `saki_crychic.rs:30` | 14, but the anchor is 13 「主要移动结束时」 | OK* |
| `RAS:（LOCK）追逐梦想的步伐` （2） | 「…则在触发结算前将行动终点改为"旭汤澡堂"」 | `HookKind::SettleBefore` `lock_dream.rs:28` | 14 | OK |
| `PP:[若宫伊芙]属于我的武士道！` （2） | 「[结算]前如果此卡上有[奇迹水晶]且…」 | `HookKind::SettleBefore` `eve_bushido.rs:28` | 14 | OK |
| `skill:学生会的检查` (council_check) [反击] | 「移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出」 | `ChainKind::SettleBefore` `council_check.rs:20` | 14 (condition 「移动结束后」 = 13) | OK |
| `CRYCHIC:想要抓住...` （2） | 「当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]」 | `HookKind::SettleBefore` `want_to_grab.rs:30` (+ `PassPlayer` `:29`) | 14 window is right; the trigger 「经过你」 is 12, not 13 | PASS |
| `CRYCHIC:我的问题呢` (my_own_problem) （1） | 「主要移动结束时，[触发结算]前打出此卡，使自己额外远离…一格」 | `ChainKind::SettleBefore` `my_own_problem.rs:19` | 14 (anchor 13) | OK |
| `CRYCHIC:春日影` （2） | 「（此卡可作为[反击]打出）使用一次Crychic角色的技能」 (C# `settleBefore` branch) | `ChainKind::SettleBefore` `haruhikage.rs:25` | generic [反击] at the settle's pre-window | OK |
| `skill:三角初华:Imprisoned XII` 状态2 | 「每次成功收取后可选择在[触发结算]前额外移动1d6（最多4次）」 | `HookKind::SettleBefore` `uika_imprisoned.rs:43` | 14 | OK |
| `skill:要乐奈:投币式停车场的猫` （2） | 「在space上结算时，若space已属于其他玩家，免除付款并抽1张卡；若自己为space的拥有者，则可选择**将该次结算改为**在space格子上添加一个"抹茶芭菲"」 | `HookKind::SettleBefore` `rana_parking.rs:35` | 15 「[结算]时」 + a **body replace** (「将该次结算改为」) | **BODY/AT** |
| `event:火种燃尽之后会怎么样呢？` | 「**触发结算时**，格子上每有一张此卡的复制品，那名玩家失去10资金」 | `HookKind::SettleBefore` `embers.rs:27` | 15 「触发结算时」 | **AT** |
| `Sumimi:儿时玩伴的鼓励` | 「可在移动掷骰前打出此卡…并在**移动后**获得一个火罐」 | `HookKind::{SettleBefore,SettleAfter,Teleported}` `childhood_cheer.rs:22-27` | 13 「移动后」 | **MOVE** |
| `skill:Poppin' Party:星之鼓动` | （2）「无法获取[CiRCLE奖励]」（4）「不能通过[主要移动]的[结算]盖房」 | `HookKind::SettleBefore` `poppin.rs:32` | （4）is a body/build-gate fact; （2）is a standing veto (arm on state change) | partly BODY |

### 3.2 `settle` (行动阶段 15 opening — declaration + [反击] window)

| rule | quote | wired | text names | flag |
| --- | --- | --- | --- | --- |
| `skill:RAISE A SUILEN` （2） | 「因该技能以外的效果在livehouse格子**[触发结算]时**」 | `HookKind::Settle` `ras.rs:28` | 15 | OK |
| `skill:户山香澄:非凡之星` （1） | 「其他玩家"星之鼓动山丘"上**[结算]时**获得一个[火罐]」 | `HookKind::Settle` `extraordinary_star.rs:44` | 15 | OK |
| `skill:三角初华:Imprisoned XII` 状态1 | 「未在任何[回忆地块]…**触发结算**」 (latch) | `HookKind::Settle` `uika_imprisoned.rs:41` | 15 | OK |
| `skill:长崎素世:通透的颜色` （2） | 「使自己本回合的**[移动终点]**对你视为对应颜色的地产商格子」 | `HookKind::Settle` `soyo_clear.rs:39` | 14 「[移动终点]」 (armed at 掷骰后); applied as a body colour | OK-ish |
| `RAS:PLEASE CHOOSE` [反击] | 「当有玩家在livehouse格子上**结算时**，使对方选择…」（2）「立即传送至对方所在格子（**不触发结算**但视为可触发乐队技能）」 | `ChainKind::Settle` `please_choose.rs:17` | 15 | OK |

### 3.3 `settleBody` (the settle's effect list / body-replace gesture)

| rule | quote | wired | text names | flag |
| --- | --- | --- | --- | --- |
| `通用:…Parking Space` (parking_space) | 「将本次结算改为…」 (`docs/TILES.md`) | `HookKind::SettleBody` `parking_space.rs:31` | body replace | OK |
| `HHW:笑容大游行` （1） | 「当次移动的移动终点视为"弦卷集团"地产商」 | `HookKind::SettleBody` `smile_parade.rs:21` | body replace (tile swap) | OK |
| `RAS:Hey Kids` | 「将本次结算改为…」 | `ChainKind::SettleBody` `hey_kids.rs:21` | body replace | OK |
| `event:circle_rebuild` | (event's settle-body entry) | `HookKind::SettleBody` `circle_rebuild.rs:22` | body | OK |

### 3.4 Tile bodies (`On::Settle`, `rules/tiles/src/*`)

| rule | quote | what it is | flag |
| --- | --- | --- | --- |
| `tile:property` / `tile:ring` | 基础[结算]规则 100–106 (+「所有RiNG不可升级」) | the **property default** — purchase / build / rent / force-buy | OK (this is the only "default function" the spec's taxonomy needs) |
| `tile:agent` | 107 | the agent sweep / choose | OK |
| `tile:circle` | 95 「CiRCLE的[结算]是：抽取一张手卡」 | special-tile behaviour, **is** the tile's [结算] | OK — do **not** demote to a post-default responder |
| `tile:circle` Pass entry | 96 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得[CiRCLE奖励]」 | a 经过 effect, correctly on `PassTile` | OK |
| `tile:edogawa` | 95 | landing draw | OK |
| `tile:event` | 97–99 | draw + event | OK |
| `mark:cp` (`rules/tile_marks/src/cp.rs`) | 「在拥有[CP]点的格子上**[结算]时**移除格子上的个[CP点]和自己[场上]1个[CP点]，[获得]800资金」 | 「[结算]时」 clause | **AT** — wired `HookKind::SettleAfter` (`cp.rs:81`) |

### 3.5 `settleAfter` (「[结算]后」/「[触发结算]后」)

| rule | quote | wired | text names | flag |
| --- | --- | --- | --- | --- |
| `PPP:STAR BEAT!` | 「"下次**结算后**可选择在绝对距离5格以内自己拥有的格子上进行一次盖房"」 | `HookKind::SettleAfter` `star_beat.rs:17` | 16 「结算后」 | OK |
| `HHW:笑容大游行` （2）（3） | 「[触发结算]**后**可将…放置于[移动终点]格子上」/「任何玩家在此卡放置的格子上[触发结算]**后**此卡放入弃牌堆」 | `HookKind::SettleAfter` `smile_parade.rs:25` | 16 | OK |
| `Sumimi:两个甜甜圈` （2） | 「可在那名玩家**触发结算后**选择传送至…并触发结算」 | `HookKind::SettleAfter` `two_donuts.rs:27` | 16 | OK |
| `MyGO:哪怕这旅程没有终点` （2） | 「[持续] **触发结算时**，获得X*60资金」 | `HookKind::SettleAfter` `endless_journey.rs:20` | 15 「触发结算时」 | **AT** |
| `PP:梦在前方，结彩当下` （3） | 「[拥有者]以外的玩家在[拥有者]拥有的格子**[结算]时**额外[支付]…」 | `HookKind::SettleAfter` `dream_ahead.rs:35` | 15 (「[结算]时」) — an extra charge that belongs in the settle's effect list | **AT/BODY** |
| `PPP:Tomorrow's Door` （3） | 「…在[拥有者]拥有的格子或梦开始的地方**[结算]时**额外支付…」 | `HookKind::SettleAfter` `tomorrows_door.rs:28` (+ `PayAdd`) | 15 | **AT/BODY** |
| `RAS:练习室里的风暴` （2） | 「[使用者]以外的玩家在距此卡所在格子X个格子处**[结算]时**将此卡放入弃牌堆且对那个玩家进行一次…收费」 | `HookKind::SettleAfter` `studio_storm.rs:21` | 15 「[结算]时」 (a remote settle-shaped charge) | **AT** |
| `HHW:黑衣人的补给` | 「该格上拥有奇迹水晶时，该格**获得"CiRCLE"格子的全部效果**」 (landing draw 1) | `HookKind::SettleAfter` `black_suits.rs:17` | this *is* the tile's [结算] (「获得CiRCLE格子的全部效果」) | **BODY** |
| `HHW:（kkr）前往笑容集结的地方！` | 「若其他玩家在该格**[触发结算]**则向所有者支付6000资金，**视为格子的收款**」 | `HookKind::SettleAfter` `kokoro_circle.rs:18` | 15; 「视为格子的收款」 = a settle-body entry (`docs/TILES.md` already says so) | **BODY** |
| `skill:凑友希那:来练习吧` （3） | 「**移动终点**为任意"RiNG"时，获得一个火罐」 | `HookKind::SettleAfter` `kokoro_practice.rs:40` | 13/14 「移动终点」, not 「结算后」 | **MOVE** |
| `skill:学生会的检查` (council_check) | 「因此卡强制停下的玩家的**结算地租价格为原价格一半**」 (the settleAfter entry clears the mark) | `HookKind::SettleAfter` `council_check.rs:23` | the rent half is a pay-stage scale; the after entry is clean-up | OK-ish |
| `Sumimi:儿时玩伴的鼓励` | 「**移动后**获得一个火罐」 | `HookKind::SettleAfter` `childhood_cheer.rs:22-27` | 13 | **MOVE** |

### 3.6 `passPlayer` (行动阶段 13 「[重叠]」) — 经过-vs-重叠

| rule | quote | wired | text names | flag |
| --- | --- | --- | --- | --- |
| `skill:濑田薰:梦幻的王子殿下` （1） | 「每次**[经过]或被[经过]**时…」 | `HookKind::PassPlayer` `kaoru_prince.rs:34` | 12 「[经过]」 (both directions) | **PASS** |
| `skill:冰川纱夜:踏上荆棘之路的觉悟` （1） | 「每次**被别的玩家[经过]**时获得1个[火罐]」 | `HookKind::PassPlayer` `sayo_thorns.rs:32` | 12 「被[经过]」 | **PASS** |
| `skill:丰川祥子:请把你们的人生交给我` 状态1 | 「**经过其他玩家**时，可将其所有层数的停留，眩晕转移至自己身上」 | `HookKind::PassPlayer` `sakiko_life.rs:35` | 12 「经过其他玩家」 | **PASS** |
| `skill:凑友希那:来练习吧` （3） | 「当其他玩家**移动[经过]您**时，您可以选择使用一个[火罐]令该玩家强制停下并触发结算」 | `HookKind::PassPlayer` `kokoro_practice.rs:39` | 12; body guards `move_remaining() <= 0 → return`, which is **always** true at `passPlayer` (it fires at move end) — so this never fires today | **PASS** (latent bug) |
| `AG:刻入天穹傲岸的烈光` [反击] | 「当你**经过一名角色**时，你可以打出此卡…」 | `ChainKind::PassPlayer` `proud_light.rs:15` | 12 「经过一名角色」 | **PASS** |
| `MyGO:难以复刻的奇迹` （2） | 「当你**[经过]场上的所有玩家各一次**…」 | `HookKind::PassPlayer` `miracle.rs:24` | 12 | **PASS** |
| `CRYCHIC:想要抓住...` （2） | 「当第一位其他玩家**经过你**…」 | `HookKind::PassPlayer` `want_to_grab.rs:29` | 12 | **PASS** |
| `skill:要乐奈:投币式停车场的猫` （3） | 「你与其他玩家**重合**时，获得其800资金」 | `HookKind::PassPlayer` `rana_parking.rs:36` | 13 「[重叠]」/「重合」 | OK |
| `skill:三角初华` 状态2 | 「主动移动**经过任何玩家**都将向其收取200资金」 | `HookKind::PassPlayer` `uika_imprisoned.rs:42` | 12 「经过任何玩家」 | **PASS** |
| `HHW:怪盗` (kaoru_thief), `MyGO:奇迹` etc. | 「[重叠]」/「移动终点触发[重叠]」 shaped clauses | `HookKind::PassPlayer` | 13 | OK |

### 3.7 Terminals

`tileResolved` (87) / `moveResolved` (88): **no rule is wired to either today.**
Texts that want them and are not wired: `AG:one_of_us` 「先在地契原主人方**结算
完成**，之后被分享方资金直接增加」 (→ `tileResolved`); `Sumimi:no_breakup` 「若传送
并**触发结算后**未能使资金变为拥有相同数字，回到原处并取消所有受到的效果」
(→ `settleAfter` or `tileResolved` — see [Ruling R3](#rulings)).

---

## 4. Mismatch classes

Status per class: **M1 done** (moveAfter; childhood_cheer single hook,
kokoro_practice (3) on moveAfter; saki_crychic / my_own_problem stay on
settleBefore reading `move_is_main` as the move-end fact). **M2 done**
(settleBody + `trigger::cancelled()`; embers, endless_journey (2),
dream_ahead (3), tomorrows_door (3), studio_storm (2), mark:cp). **M3 done**
(black_suits attaches a `tile:circle` instance; kokoro_circle on settleBody;
rana_parking (2) split per R4). **M4 done** (off `passPlayer` onto `passTile` /
`ChainKind::PassTile`: kaoru_prince (1), sayo_thorns (1), sakiko_life 状态1,
proud_light, miracle (2), uika_imprisoned 状态2, kokoro_practice (3);
EXCLUDED want_to_grab (R5 pending) and no_breakup (R3 pending) — left with
`TODO(规则书)` markers). **M5 done** (one_of_us on `tileResolved`). **M6a done**
(a no-settle teleport fires `passTile` + `passPlayer` at its destination).

### M1 — 「移动后」/「主要移动结束时」/「移动终点」 effects on a settle stage

These name 行动阶段 13 (or the 「[移动终点]」 half of 14), but hang off
`settleBefore` / `settleAfter`. They must fire when the move ends **whether or
not it settles** (其他规则注意事项 1.2: 「是否[结算]」 gates only the settle) —
and must not fire *after* a settle they have nothing to do with.

* `Sumimi:儿时玩伴的鼓励` — 「并在**移动后**获得一个火罐」. Today triple-hooked
  (`settleBefore` + `settleAfter` + `teleported`) to paper over the missing
  move-end point; a 「不触发结算」 walk grants nothing.
* `skill:凑友希那` （3） — 「**移动终点**为任意"RiNG"时，获得一个火罐」 on
  `settleAfter`: misses no-settle moves, and runs after the body for no reason.
* `skill:丰川祥子（CRYCHIC）` （2）（3） and `CRYCHIC:我的问题呢` （1） name
  「主要移动结束时」 as the anchor; the window itself is correctly
  「[触发结算]前」. Keep on `settleBefore`, but the *condition* should read a
  move-end fact, not "a settle is happening".

### M2 — 「[结算]时」/「触发结算时」 effects on `settleAfter`

行动阶段 15 vs 16. These should sit **in** the settle's effect list (`settle`
declaration / `settleBody`), not after it. Observable today: they still run
when a field card **replaces** the body (`settleBody` cancel → `settleAfter`
still runs), so e.g. a Parking-Space replacement does not stop
`endless_journey`'s 「触发结算时」 payout, `mark:cp`'s CP spend, or
`black_suits`'s borrowed CiRCLE draw.

* `event:火种燃尽之后会怎么样呢？` — 「**触发结算时**，格子上每有一张此卡的复
  制品，那名玩家失去10资金」 on `settleBefore` (one stage *early*).
* `MyGO:哪怕这旅程没有终点` （2） — 「**触发结算时**，获得X*60资金」.
* `PP:梦在前方` （3） / `PPP:Tomorrow's Door` （3） — 「…**[结算]时**额外[支付]」.
* `RAS:练习室里的风暴` （2） — 「在距此卡所在格子X个格子处**[结算]时**…收费」.
* `mark:cp` — 「在拥有[CP]点的格子上**[结算]时**移除…[获得]800资金」.

### M3 — special tile behaviour living on `settleAfter` instead of the body

Per 专有名词 5, 「执行格子上的所有效果，包括基础效果以及技能或卡所导致的效果」
— a card that makes a tile *gain another tile's effect* is writing an entry in
that tile's effect list, not an after-hook.

* `HHW:黑衣人的补给` — 「该格上拥有奇迹水晶时，该格**获得"CiRCLE"格子的全部
  效果**」. `docs/TILES.md` already prescribes attaching a `tile:circle`
  instance; today it is a `settleAfter` hook that draws 1. A body replace then
  leaves the borrowed draw running.
* `HHW:（kkr）前往笑容集结的地方！` — 「**视为格子的收款**」. TILES.md: "the
  instance's settle-body entry, not a card hook". Today `settleAfter`.
* `skill:要乐奈:投币式停车场的猫` （2） — 「将该次结算改为在space格子上添加一
  个"抹茶芭菲"」 is a **body replace** (「将该次结算改为」, the same gesture as
  Hey Kids' `ChainKind::SettleBody`), but it runs at `settleBefore` and does it
  with `set_pay_amount(0)` + `add_mark` instead of replacing the body. The
  sibling clause 「若space已属于其他玩家，**免除付款**并抽1张卡」 is a pay-stage
  cancel/zero, not a pre-settle write.

### M4 — 「移动后」/「[经过]」 effects on `passPlayer` (重叠)

行动阶段 12 「[经过]」 vs 13 「[重叠]」. `passPlayer` fires once per other
player on the **final** tile (`play.rs:1272-1292`); 「经过」 fires per path
step. A 经过 effect on `passPlayer` misses every intermediate tile and fires
on an end-tile 重叠 that its text never names. `skill:凑友希那` （3） is the
sharp case: its body even guards `move_remaining() <= 0 → return`, which is
always true at `passPlayer`, so the force-stop never fires.

Affected: `kaoru_prince` （1）, `sayo_thorns` （1）, `sakiko_life` 状态1,
`kokoro_practice` （3）, `proud_light` [反击], `miracle` （2）,
`want_to_grab` （2）, `uika_imprisoned` 状态2.
Correct on `passPlayer`: `rana_parking` （3） 「重合」, and the genuine 重叠
clauses (「移动终点触发[重叠]」 shaped).

### M5 — `settleAfter` vs the terminal (`tileResolved`)

行动阶段 16 lumps 「[结算]后」 and 「主要移动阶段后」; we already split it into
`settleAfter` (only if the settle happened) and `moveResolved` (always, after
the whole routine). `tileResolved` is a further terminal for 「结算完成时」 —
it fires even when the settle was cancelled (complete-as-nothing). Today
nothing uses it; `AG:one_of_us`'s 「先在地契原主人方**结算完成**」 wants it.

### M6 — movement-side (from the movement specs)

See §6.

---

## 5. The spec's three constructs — what belongs where

**Do not copy these names.** They are the external project's taxonomy and it
fights the 规则书.

| spec construct | what it actually is | where it belongs for us |
| --- | --- | --- |
| `BeforeDefaultTileResolution` | a counteraction window before the tile's effect runs | our **`settle`** (行动阶段 15 opening — the declaration + [反击] window). **Not** our `settleBefore`, which is 行动阶段 14 and has no counterpart in their pipeline. |
| `TileController.ServerTryResolveTileFunction` ("default, exactly once") | *only* the general property function (purchase / development / acquisition / rent); the other six tile types "explicitly unavailable" | our `tile:property` / `tile:ring` / `tile:agent` bodies. This is one entry of the settle's effect list, not a stage that always runs first. |
| `ActivateSpecialResolution` | a post-default window where "special tile behaviour is NOT the default" and effects respond one at a time | **this split is wrong for us.** The 规则书 names special-tile behaviour *as* the tile's [结算] (「CiRCLE的[结算]是：抽取一张手卡」). It is a body entry beside the property default, not a post-default responder ring. |
| `EffectReactionController` (round-robin, one effect at a time) | the responder ring | this is the **[反击] ring** we already run per chain link (其他规则注意事项 3: from the next player in action order). It is not a second, special-only window. The settle's body list runs in instance order — the book does not order multiple effects on one tile beyond L6. |
| `AfterTileResolution` / `TileResolved` | after window / terminal | our `settleAfter` / `tileResolved`. Keep both; keep the split from `moveResolved`. |
| "position re-read before each stage; relocation never re-runs the default" | fresh snapshots | adopt the *re-read*, but not "never re-runs": see Q7 below. The default body still runs exactly once per `settle_at`. |

**Answer to "does some of these effects belong to other hooks?"** — yes, in
three directions: (1) move-end effects (M1) belong on a move-end point that
exists even when the move does not settle; (2) 「[结算]时」 effects and
"the tile gains another tile's effect" clauses (M2/M3) belong **in** the
settle's effect list, not after it; (3) 经过 effects (M4) belong on the pass
stages, not on 重叠. The spec's default-first / special-after split is the
wrong remedy: L6 + 专有名词 5 say the settle is one replaceable list.

## 6. Movement model — the two movement specs vs ours

`PLAYER_MOVEMENT_LOGIC.md` / `PLAYER_MOVEMENT_RESPONSIBILITY.md` draw a
cleaner **boundary** than our `after_walk`, even though their names and their
controller split are not ours.

| their stage | theirs | ours (`play.rs` / `move_ctx.rs`) | verdict |
| --- | --- | --- | --- |
| move end vs tile resolution | `MovementFinished` → `AfterMoving` → `MovementEnd` → `MovementEnded`, **then** an eligibility gate, **then** the tile-resolution pipeline | `after_walk` raises `settleBefore`/`settle`… and only then `moveResolved` — the move's terminal is *after* the settle | **adopt the boundary**: the move is over at 行动阶段 13, before any settle stage. Introduce `moveAfter` (see §7). |
| 「不触发结算」 | eligibility gate: skip tile resolution entirely; `PlayerLandedOnTile` still fires | `m.resolve == false` skips `settleBefore`/`settle`/… but still raises `passPlayer` + `moveResolved`; a no-settle **teleport** skips `passTile`/`passPlayer` too | the walk side is right (1.2 「是否[结算]」 gates only the settle); the teleport side is wrong — see M6a. |
| arrival vs settle | `PlayerLandedOnTile` fires even when tile resolution is prohibited | the last step's `passTile` is the arrival; no separate landed point | fine — `passTile` + `moveAfter` cover it. Do not add a third. |
| pass-through (经过) | one `PlayerPassedTile` per entered tile, carrying prev/dest indexes + remaining spaces (0 = destination pass; teleport carries 0) | `passBefore` → `passTile` → `pass` per step; `m.remaining` distinguishes intermediate vs destination | equivalent. Keep our three (they are 12's three phrases: declaration / step / the generic 「[经过]」). |
| teleport vs walk | teleport: `PlayerTeleported` → dest `PlayerPassedTile` (remaining 0) → `PlayerLandedOnTile` — **pass and landing always fire** | `teleported` is raised **after** the settle (C# 24390) for a settling teleport, and alone for a no-settle one; B41 「依次触发[经过],[重叠],和[结算]」 names the order as 经过→重叠→结算 and does not name `teleported` | keep our `teleported` placement (C#-carried, no book conflict), but **M6a**: a 「不[触发结算]」 teleport must still fire `passTile` + `passPlayer` at the destination. `move_ctx.rs:101-103`'s "a teleport with this clear resolves nothing" over-reads 「是否[结算]」. |
| zero-step move | raises pass + landed | raises `passPlayer` only (0-move); `passTile` is a `TODO(规则书)` at `play.rs:1063-1069` | E14 names only 重叠 for the 0-move. Keep ours; the spec's extra landed event is not needed. |
| redirects / relocation | LIFO relocation stack, same-player **replacement** (newest same-player op stops the older) | effects just write `pos` / the plan; no stack | not required by the book. Adopt only if a card needs nested move replacement. |
| mid-settle relocation | re-read position before every lifecycle stage; "later relocation does not rerun default" | `settle` re-reads `pos` (so a `settleBefore` relocation redirects); later stages keep the original `at` | see Q7 below — we go further than both. |

**M6a (movement-side fix worth making):** a 「不[触发结算]」 walk already fires
经过/重叠; a 「不[触发结算]」 teleport fires neither. 专名词 9 gives the teleport
a [路径] of just the endpoint, and B41 fires 经过/重叠/结算 at that endpoint —
1.2 then removes only the 结算. Fire `passTile` + `passPlayer` for a no-settle
teleport.

## 7. Proposed stage model (our names)

```
-- movement head (every move: walk or teleport, main or card, settle or not) --
moveBefore         **NEW**       「移动前」 -- the move is planned (kind, start,
                                 steps / destination known), nothing walked yet.
                                 Counteractions that cancel or alter the move go
                                 here (rollPlan / moveRoll stay the roll-specific
                                 points inside a main move's roll).

-- movement tail (every completed move: walk or teleport, settle or not) --
passBefore / passTile / pass     行动阶段 12  「[经过]」           (keep)
passPlayer                       行动阶段 13  「[重叠]」/「重合」   (keep)
moveAfter          **NEW**       行动阶段 13  「移动后」/「主要移动结束时」
                                 also the 「[移动终点]」 condition anchor

-- settle pipeline (only when the move settles, or a card forces a settle) --
settleBefore                     行动阶段 14  「[结算]前」/「[触发结算]前」
                                 Q7: relocation here redirects the settle and
                                 re-runs this window at the new tile
settle                           行动阶段 15 opening  「[结算]时」/「触发结算」
                                 declaration + [反击] window
                                 cancel = "the settle never happened"
settleBody                       行动阶段 15 body — the settle's **effect list**
                                 (a) field replace (「将本次结算改为…」)
                                 (b) tile rule instances in instance order
                                     = property default AND special-tile
                                       behaviour AND card 「[结算]时」 clauses
settleAfter                      「[结算]后」/「[触发结算]后」  (only if it happened)
tileResolved                     「结算完成时」 terminal (also on cancel)

-- move tail --
moveResolved                     行动阶段 16  「主要移动阶段后」  (always)
```

### Kind mapping

| existing kind | verdict |
| --- | --- |
| `passBefore` / `passTile` / `pass` | **keep.** Three phrases of 行动阶段 12. |
| `passPlayer` | **keep** (13 「[重叠]」). Re-home the M4 经过 effects off it. |
| *(none)* → `moveBefore` | **add** (user, 2026-10-07: a move has both a before and an after). Fires for every move — walk or teleport, main or card-driven, settling or not — once its plan is fixed and before the first step / the teleport. `teleport` (pre-teleport) stays as the teleport-specific point. |
| *(none)* → `moveAfter` | **add.** 13 「移动后」/「主要移动结束时」; fires after `passPlayer`, before `settleBefore`, for every completed move. Paired with `moveBefore`; `teleported` stays the teleport-specific point. |
| `settleBefore` | **keep** (14). Re-home the M1 move-end effects and the M2 「触发结算时」 effects off it. |
| `settle` | **keep** (15 opening). This is the spec's BeforeDefault — do not rename it to that. |
| `settleBody` | **keep.** This is the settle's effect list — *not* "the default". Pull the M2/M3 clauses into it (as tile-instance entries where `docs/TILES.md` already says so). |
| `settleAfter` | **keep** (16's 「结算后」 half). Only for text that says 「结算后」/「[触发结算]后」. |
| `tileResolved` | **keep** (terminal, complete-as-nothing). Start using it for 「结算完成时」 (`AG:one_of_us`). |
| `moveResolved` | **keep** (16's 「主要移动阶段后」 half). Always fires; stays *after* the settle pipeline. Not a synonym for `moveAfter`. |
| `teleported` | **keep** where it is (after the settle for a settling teleport). Not a settle stage. |

No kind is renamed or split. The structural change is the new `moveBefore` / `moveAfter` pair
and the re-homing in §4.

### How `settleBody` fits

`settleBody` is **the** answer to the spec's "default vs special" question:
the book does not split them. The property default is just the entry
`tile:property` declares; CiRCLE's draw is the entry `tile:circle` declares;
「该格获得CiRCLE格子的全部效果」 is a card attaching `tile:circle`'s entry to
another tile; 「视为格子的收款」 is a card adding a collect entry. All of them
are one list, run in instance order, each of them an effect link a [反击] can
answer. A field card may replace the whole list (`settleBody` cancel) — that
is L6 「技能和卡的效果优先」.

What the spec's "one effect at a time, round-robin" *does* buy us is already
covered: every chain link (including each body entry) opens its own [反击]
ring, and that ring is the 规则书's 其他规则注意事项 3 order. We do not need a
second, special-only ring.

### Q7 — relocation during the pre-settle window

**Ruled and implemented (user 2026-10-07).** A relocation during the pre-settle
window means **the settle did not happen at the old tile and proceeds at the
new tile**; we re-run the pre-settle window at the new tile, guarded against
loops. The `TODO(规则书) PIPELINE-AUDIT Q7` marker is removed. At most
`MAX_SETTLE_REDIRECTS = 8` redirects per settlement (log and settle where
standing after that). Test: `rb_settle_stages::q7_settle_before_relocation_settles_at_the_new_tile`.

* `moveAfter` / `settleBefore` relocation → the settle targets the new tile;
  re-run `settleBefore` at the new tile. Guard: cap the re-runs (same spirit
  as `MAX_NESTING = 8`) and skip tiles already visited in this settle chain.
* `settle` / `settleBody` relocation → the originally named tile finishes
  (「落点效果」, 行动阶段 15). The default body runs exactly once per
  `settle_at` — the spec's "never re-runs the default" holds *here*.
* `settleAfter` / `tileResolved` relocation → nothing re-runs; the resolve
  already happened.
* A relocation inside a no-settle move's tail only moves `moveAfter` /
  `moveResolved`'s tile; there is no settle to redirect.

Interaction to watch: a `settleBefore` relocation that lands on a tile whose
own `settleBefore` relocates again is exactly what the loop guard is for. And
because `moveAfter` now precedes `settleBefore`, a move-end effect that relocates
must do so *before* `settleBefore` opens — otherwise the two windows disagree
about which tile is being settled.

## 8. Rulebook first — where the spec conflicts

| spec claim | 规则书 | verdict |
| --- | --- | --- |
| "default function runs first, then a special window" | L6 「技能和卡的效果优先于规则书中的所有规则」; 专有名词 5 「执行格子上的**所有**效果，包括基础效果以及技能或卡所导致的效果」 | **reject.** Keep the replaceable body list. (Already pinned: `docs/rulebook/PIPELINE-AUDIT.md` T1b.) |
| "special tile behaviour is NOT the default" | 基础[结算]规则 95–99 names CiRCLE / 江户川 / 咖啡厅 / 流星堂 behaviour **as** their [结算] | **reject** the taxonomy. The behaviour is the body. |
| "one effect at a time in round-robin" for specials | 其他规则注意事项 3 orders **[反击]**, not body entries | adopt only for [反击] (already done). Body entries run in instance order. |
| "relocation never re-runs the default" | 行动阶段 15 「触发地块/**落点**效果」 | adopt **after** the pre-settle window; before it, Q7 says redirect + re-run. |
| "the other six tile types explicitly unavailable" | our board has all six kinds specified (基础[结算]规则) | N/A — implement all, as we do. |

## 9. Rulings {#rulings}

* **R1 — does 「移动后」/「主要移动结束时」 fire for a 「不触发结算」 move?**
  **Ruled yes (user 2026-10-07).** 13's move tail (重叠 + `moveAfter`) fires for
  every completed move; the jump to 16 skips only the settle stages (14–15).
  Implemented. Test: `rb_settle_stages::m1_no_settle_move_still_grants_the_move_after_fire`.
* **R2 — no-settle teleport: 经过/重叠?** **Ruled yes (M6a).** Implemented; a teleport's [路径] is just the endpoint.
  B41 + 专名词 9 + 1.2 read together say the path and its 经过 still exist and
  only the settle is gated; `move_ctx.rs`'s comment says otherwise.
* **R3 — `Sumimi:no_breakup` 「若传送并触发结算后未能使资金变为拥有相同数字，
  回到原处并取消所有受到的效果」**: is 「触发结算后」 `settleAfter` or
  `tileResolved`? "取消所有受到的效果" (undo everything the settle did) is
  easier at `tileResolved` (complete-as-nothing) than after the body has run.
  **Pending.** Left as-is with a `TODO(规则书) R3` marker.
* **R4 — `skill:要乐奈` （2）'s 「免除付款并抽1张卡」** when Space is another
  player's: is 「免除付款」 a pay-stage cancel (so a 「支付减半」 etc. never
  sees it) or a body replace (「将本次结算改为…」 for the owner branch only)?
  The text uses 「免除付款」 for one branch and 「将该次结算改为」 for the other —
  **Ruled (user 2026-10-07):** 「免除付款」 is a payment-stage cancel (`payTotalCancel`); 「将该次结算改为」 is a settleBody replace. Implemented in `rana_parking.rs`.
* **R5 — `want_to_grab` 「当第一位其他玩家经过你」**: 经过 (passTile, first
  passer mid-move) or 重叠 (passPlayer)? **Pending.** Left as-is with a
  `TODO(规则书) R5` marker. The C# GrabFx used PassPlayer. The force-move is 「在那名玩家[触发结算]前」, which only exists if
  that move settles — so a mid-move 经过 reading is the one that can force-stop
  before the settle.

---

## 10. Where things live

| what | where |
| --- | --- |
| the settle pipeline | `crates/game-core/src/engine/play.rs` `after_walk` (Q7 loop) / `settle` / `settle_at` |
| the walk / teleport | `play.rs` `walk` / `teleport_as` / `move_after` / `move_resolved`; `move_ctx.rs` (`MoveCtx::resolve`) |
| kind lists | `rules/card-sdk/src/abi.rs` `TriggerKind` / `ChainKind` / `HookKind` / `OnKind::Settle` |
| tile bodies | `rules/tiles/src/*`, `docs/TILES.md` |
| tile-mark rules (`mark:*`) | `rules/tile_marks/src/*`, `docs/TILES.md` → 「Board marks」 |
| prior audit (Q6/Q7 origin) | `docs/rulebook/PIPELINE-AUDIT.md` §4, §5.4 |
| rulebook ground truth | `data/rules.txt`; timing tables `docs/rulebook/rulebook-doc.md` |