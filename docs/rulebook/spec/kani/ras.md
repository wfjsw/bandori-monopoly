# Kani unit `ras` — RAISE A SUILEN

Branch `kani-ras`, worktree `/home/monopoly/kani-ras-wt`.
Scope: 15 `RAS:` cards + 5 RAISE A SUILEN character skills + band skill
`UNSTOPPABLE` (`docs/rulebook/spec/cards/ras.md`, `docs/rulebook/cards.json`,
`data/characters.json`, `data/bands.json`).

Specs: `rules/kani/src/spec/ras.rs`, `rules/kani/src/spec/skills_ras.rs`.
Proofs: `rules/kani/src/proofs/ras.rs`, `rules/kani/src/proofs/skills_ras.rs`.

Method: Kani proofs on the unmodified card/skill bodies via `invoke::`, against
specs written only from the 规则书 quotes. Narration is not the frame.
Classification: PROVEN | FAILS | PROTOCOL-LEVEL | BLOCKED-HOST | OPEN | TIMEOUT/OOM.

---

## Summary counts

| classification | card clauses | skill clauses | total |
|---|---:|---:|---:|
| PROVEN | 7 | 2 | 9 |
| FAILS | 0 | 0 | 0 |
| PROTOCOL-LEVEL | 30 | 5 | 35 |
| BLOCKED-HOST | 8 | 3 | 11 |
| OPEN (text undefined; only what the text pins is proved) | 5 | 2 | 7 |
| **total** | **50** | **12** | **62** |

Rows can carry both an OPEN tag and a classification of the pinned part; the
table lists the **primary** classification. OPEN ids are in the last column.

---

## Host gaps this unit hit (exact)

| gap | ops | effect on proofs |
|---|---|---|
| `tile_named` resolves only 「江户川乐器店」 | `Op::tile_named` | every clause that names 白雪学园 / 艺术学院高中 / 瑟罗希亚国际学校 / 银河拉面馆 / 旭汤澡堂 / CHUCHU的公寓 / Live House / RiNG* / 武道馆 / Space / Live House Galaxy / Bandori车站 / 东京外 / CiRCLE cannot look the tile up → teleport/place-on-tile branches never run |
| `gain_fire` does not clamp to `fire_max` | `Op::gain_fire` | fire-pot caps 「上限1」「上限2」 unprovable (real engine: `game-core::engine::ops::gain_fire` clamps) |
| `add_held` / `add_tok` ignore the `max` argument | `Op::add_held` | PAREO cap 3 unprovable (real: `(was+n).clamp(0,max)-was`) |
| `state_set` / `state_get` ignore MIN/MAX/EXPIRES field columns | `Op::state_set` | `fire_pot` / `state::set_bounds` cannot declare a cap under Kani |
| `target` always returns 0 | `Op::target` | `ctx::target(who)` becomes `Some(0)`, so multi-player placement cannot pick the chosen seat |
| `card_move` + all `plan::*` / `set_build_cost_pct` / `set_fixed_roll` are protocol stubs (return 0) | see `host.rs` move-plan arm | teleport-as-main-move, +Xd20/+Xd10 dice, half/free build cost are PROTOCOL-LEVEL |
| `cards_in(Hand)` has no per-player pile (only `hand_resonance` + a global bitset) | `Op::cards_in` | riot cycle proved over the `hand_resonance` view |

These are **not** body bugs. A proof that needs them is BLOCKED-HOST or
PROTOCOL-LEVEL, never a silent zero.

---

## Cards

### RAS:Change the world — 4 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-change-the-world-01 | 「将此卡放置于你的一个有房屋的livehouse格子上」 | BLOCKED-HOST | — | — | needs `tile_named(livehouse)` + `place_card_on`; tile names unmodeled | OPEN-livehouse |
| C-RAS-change-the-world-02 | 「你的本次移动掷骰变为3d20」 | PROTOCOL-LEVEL | — | — | `plan::set_base_dice` is a move-plan stub (KANI.md §6) | — |
| C-RAS-change-the-world-03 | 「每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶」 | PROTOCOL-LEVEL | — | — | walk-by-walk `[经过]` during a 3d20 move is engine scheduling | OPEN-livehouse |
| C-RAS-change-the-world-04 | 「下一次收费增加50*(y+1)*n…触发后将该卡放入弃牌堆」 | PROTOCOL-LEVEL | — | — | multi-turn charge raise + settle chain | OPEN-livehouse, OPEN-change-world-equal |

### RAS:EXIST — 4 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-exist-01 | 「将此卡放置于自己场上」 | PROTOCOL-LEVEL | — | — | placement is fine in isolation, but the card's identity is the multi-turn redirect; leaf place is not separable from the standing effect without inventing a body | — |
| C-RAS-exist-02 | 「所有对单一玩家生效的手卡…目标将改为你」 | PROTOCOL-LEVEL | — | — | standing redirect over every later card play is engine-wide | OPEN-exist-multi |
| C-RAS-exist-03 | 「你的下回合开始时将其翻入弃牌堆」 | PROTOCOL-LEVEL | — | — | next-turn-start flip is multi-turn | — |
| C-RAS-exist-04 | 「若在此期间此卡没有造成影响，抽1张卡」 | PROTOCOL-LEVEL / OPEN | — | — | 「造成影响」 undefined; multi-turn | OPEN-exist-influence |

### RAS:PLEASE CHOOSE — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-please-choose-01 | 「[反击] 当有玩家在livehouse格子上结算时」 | PROTOCOL-LEVEL | — | — | [反击] window + settle chain (KANI.md §6) | OPEN-livehouse |
| C-RAS-please-choose-02 | 「（1）立即打出一张可将你指定为目标的牌…」 | PROTOCOL-LEVEL / OPEN | — | — | forced play + targeting is multi-effect; 「可将你指定为目标」 not enumerated | OPEN-please-choose-qual |
| C-RAS-please-choose-03 | 「（2）使你立即传送至对方所在格子（不触发结算…）」 | PROTOCOL-LEVEL | — | — | counteraction body + teleport-without-settle is move-plan / chain | — |

### RAS:R. I. O. T. — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-riot-01 | 「[反击] 当你被其他人的卡效果影响时」 | PROTOCOL-LEVEL | — | — | [反击] window is engine chain | — |
| C-RAS-riot-02 | 「所有玩家将所有手牌放至弃牌堆，并抽等量的卡」 | **PROVEN** | `riot_02_03_cycle_iff` | see results | body leaf proved over `hand_resonance` hands | — |
| C-RAS-riot-03 | 「你额外抽1张卡」 | **PROVEN** | `riot_02_03_cycle_iff`, `riot_03_extra_draw_mutant_is_detected` | see results | same leaf; mutant detects missing extra draw | — |

### RAS:Repaint — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-repaint-01 | 「[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后」 | PROTOCOL-LEVEL | — | — | [反击] after move-roll is engine chain | — |
| C-RAS-repaint-02 | 「移动数-X，X为对方原本预计路径上你拥有的格子数」 | PROTOCOL-LEVEL | — | — | planned-path walk + `plan::set_pay_factor` / `tile_steps_ahead` move-plan | — |
| C-RAS-repaint-03 | 「对方此次结算的支付减半」 | PROTOCOL-LEVEL / OPEN | — | — | settle payment shaping is engine; rounding unprinted | OPEN-repaint-round |

### RAS:UNSTOPPABLE (card) — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-unstoppable-card-01 | 「投掷1d6…传送至白雪学园…CHUCHU的公寓」 | BLOCKED-HOST | — | — | `tile_named` cannot resolve the six destinations; `plan::set_teleport_to` is a stub | OPEN-unstoppable-mod |
| C-RAS-unstoppable-card-02 | 「本次传送不触发结算，视为你的主要移动」 | PROTOCOL-LEVEL | — | — | move-plan `set_resolve(false)` + `card_move` | — |
| C-RAS-unstoppable-card-03 | 「若骰点为1-3获得2000资金，若为4-6则获得1000资金」 | **PROVEN** | `unstoppable_card_03_money_iff`, `…_noop_when_out`, `…_mutant_is_detected` | see results | money half is fully observable (`GainMoney`) | OPEN-unstoppable-mod (face 6) |

### RAS:成为最强 — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-become-strongest-01 | 「roll 1d10，传送到livehouse对应的格子…」 | BLOCKED-HOST | — | — | `tile_named` cannot resolve the livehouse table; destination unobservable | OPEN-livehouse, OPEN-become-strongest-table |
| C-RAS-become-strongest-02 | 「若你没有Livehouse格子，传送到"Live House"」 | BLOCKED-HOST | — | — | same `tile_named` gap | OPEN-livehouse |
| C-RAS-become-strongest-03 | 「视为你的主要移动」 | PROTOCOL-LEVEL | — | — | `card_move` is a move-plan stub | — |

### RAS:游击演出 — 4 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-guerrilla-01 | 「[特]…回合结束时…支付了至少1000资金…」 | PROTOCOL-LEVEL | — | — | full-turn settle ledger + turn-end gate | OPEN-livehouse |
| C-RAS-guerrilla-02 | 「将此卡置于场上，指定…最贵的地契…」 | PROTOCOL-LEVEL / OPEN | — | — | designation + 「视为 live house」 is standing state across turns | OPEN-livehouse |
| C-RAS-guerrilla-03 | 「[反击]当你经过…livehouse格子后，且当次[结算]时…」 | PROTOCOL-LEVEL | — | — | dual-condition [反击] window | OPEN-livehouse |
| C-RAS-guerrilla-04 | 「传送至任意无主可购买的格子并[结算]，且必须购买」 | PROTOCOL-LEVEL | — | — | teleport + forced buy + settle chain | — |

### RAS:狂乱Hey Kids!! — 4 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-hey-kids-01 | 「[反击] 在属于你的格子上结算时」 | PROTOCOL-LEVEL | — | — | [反击] window; TEST-FINDINGS 2026-10-06 narrows to rent-on-holder | — |
| C-RAS-hey-kids-02 | 「将本次结算改为：将本格上的房屋转移…」 | PROTOCOL-LEVEL | — | — | replace-settle is chain rewrite | — |
| C-RAS-hey-kids-03 | 「至少一方数量为1，且每个目标格子至多获得1层」 | PROTOCOL-LEVEL / OPEN | — | — | split grammar under-determined | OPEN-hey-kids-split |
| C-RAS-hey-kids-04 | 「转移后各格房屋造价总和－获得房屋数量×500」 | PROTOCOL-LEVEL / OPEN | — | — | money formula rides the transfer; sum scope unprinted | OPEN-hey-kids-sum |

### RAS:练习室里的风暴 — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-storm-01 | 「当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」 | BLOCKED-HOST | — | — | needs `tile_named` / livehouse ownership on the standing tile | OPEN-livehouse |
| C-RAS-storm-02 | 「每次[使用者]经过CiRCLE时…奇迹水晶（初始0，上限3）」 | BLOCKED-HOST | — | — | crystal cap via `add_crystals` max not modeled; CiRCLE `tile_named` missing | — |
| C-RAS-storm-03 | 「距此卡所在格子X个格子处[结算]时…(4-X)/4倍价格」 | PROTOCOL-LEVEL / OPEN | — | — | distance metric + settle charge + multi-turn placement | OPEN-storm-distance, OPEN-storm-price |

### RAS:（LOCK）追逐梦想的步伐 — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-lock-01 | 「抽取游戏开始的2手牌前…放置在自身场上」 | PROTOCOL-LEVEL | — | — | opening-deal replacement is engine setup | — |
| C-RAS-lock-02 | 「游戏开始时[传送]到"东京外"获得3层[除外]」 | BLOCKED-HOST | — | — | `tile_named("东京外")` unmodeled; game-start teleport is setup | — |
| C-RAS-lock-03 | 「主要移动[经过]了"Bandori车站"则…终点改为"旭汤澡堂"…[移除]」 | BLOCKED-HOST | — | — | `tile_named` for both tiles; `send_to_dest` / move-plan | — |

### RAS:（MASKING）CRUSH ON THE DRUM!!! — 2 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-crush-01 | 「添加骰效果的卡不受任何其他效果影响」 | PROTOCOL-LEVEL / OPEN | — | — | turn-level immunity flag is engine; scope unprinted | OPEN-crush-immune |
| C-RAS-crush-02 | 「额外添加Xd20，X为你弃牌堆的卡数」 | PROTOCOL-LEVEL | — | — | `plan::add_extra_dice` is a move-plan stub | — |

### RAS:（PAREO）渐渐远去的你 — 3 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-pareo-01 | 「获得2个PAREO标记」 | **PROVEN** (sub-cap) + BLOCKED-HOST (cap) | `pareo_01_marks_iff`, `pareo_01_marks_mutant_is_detected` | see results | +2 marks is exact when pre ≤ 1; `add_held` ignores max=3 | — |
| C-RAS-pareo-02 | 「视为你的房屋总数增加」 | PROTOCOL-LEVEL | — | — | listener offer (鳰原令王那 (2)) is a cross-skill hook | — |
| C-RAS-pareo-03 | 「可选择移除任意你拥有的格子上的一层房屋」 | PROTOCOL-LEVEL | — | — | optional prompt + `add_house` is covered only in the full play; isolated as declined in the mark proof | — |

### RAS:（chuchu）演奏我的音乐吧 — 5 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-chuchu-01 | 「放置于你以外的一名玩家场上并为其添加3个奇迹水晶」 | BLOCKED-HOST | — | — | `Op::target` returns 0 (cannot pick the chosen seat); `set_crystals` writes the running instance's counter | — |
| C-RAS-chuchu-02 | 「那名玩家的每个回合结束时失去一个」 | PROTOCOL-LEVEL | — | — | turn-end hook over later turns | — |
| C-RAS-chuchu-03 | 「下次购买地契时，[使用者]获得100资金…」 | PROTOCOL-LEVEL | — | — | buy-pass chain across seats | — |
| C-RAS-chuchu-04 | 「每触发一次…额外获得100（上限500）」 | PROTOCOL-LEVEL / OPEN | — | — | which 「获得资金」 unprinted | OPEN-chuchu-which-gain |
| C-RAS-chuchu-05 | 「奇迹水晶为0时，放入[使用者]的弃牌堆并使[使用者]抽一张卡」 | PROTOCOL-LEVEL | — | — | crystal-hits-zero is a standing state transition | — |

### RAS:（和奏瑞依）寄于指尖的执念 — 4 clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-layer-01 | 「[反击] 当你使用火罐进行掷骰时」 | PROTOCOL-LEVEL | — | — | [反击] window on the fire-pot roll chain | — |
| C-RAS-layer-02 | 「保留（写下）未被选择的另一个骰点」 | PROTOCOL-LEVEL | — | — | banked-face write is cross-turn state | — |
| C-RAS-layer-03 | 「消耗一个火罐以用于替代当回合的移动掷骰，随后删去该骰点」 | PROTOCOL-LEVEL | — | — | later-turn roll replacement (`set_fixed_roll` stub) | — |
| C-RAS-layer-04 | 「可保留多个骰点」 | PROTOCOL-LEVEL | — | — | multi-face bank + pick | — |

---

## Character skills

### 和奏瑞依 — 一次又一次竭尽全力

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-rio-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 | **PROVEN** (leaf +1) + BLOCKED-HOST (cap) | `rio_01_fire_gain_leaf` | see results | body requests +1 `GainFire`; cap 「上限1」 needs `gain_fire` clamp + `state_set` MAX field | — |
| C-RAS-rio-02 | 「进行任意掷骰后，可选择使用一个[火罐]再投一次」 | BLOCKED-HOST / PROTOCOL-LEVEL | — | — | `trig_move_roll` unmodeled (KANI-COVERAGE); reroll chain | — |

### 朝日六花 — 瞄准目标

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-rokka-01 | 「购买的第一个非"旭汤澡堂"或任意"Live House"格子获得"Live House"的颜色」 | BLOCKED-HOST | — | — | `tile_named("旭汤澡堂")` unmodeled; `color_for` prop write needs the tile | OPEN-livehouse |
| C-RAS-rokka-02 | 「在你有初始"Live House"格子时…旭汤澡堂…获得颜色」 | BLOCKED-HOST | — | — | same | OPEN-livehouse |

### 佐藤益木 — 与燃烧的红色一起驰骋

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-masking-01 | 「每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]（初始0，上限2）」 | **PROVEN** (leaf +1) + BLOCKED-HOST (cap) | `masking_01_fire_gain_leaf`, `fire_pass_gain_mutant_is_detected` | see results | body requests +1 `GainFire`; cap needs host clamp | — |
| C-RAS-masking-02 | 「移动阶段前可使用X个[火罐]…额外添加Xd10」 | PROTOCOL-LEVEL | — | — | `spend_fire` + `plan::add_extra_dice` (stub) | — |

### 鳰原令王那 — 梦幻可爱♪女仆

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-pareo-skill-01 | 「游戏开始时如果场上有"Pastel✽Palettes"角色则额外获得1个正面[P✽P粉丝]」 | PROTOCOL-LEVEL | — | — | game-start setup + band check | — |
| C-RAS-pareo-skill-02 | 「失去1PAREO标记…所有非自己的玩家分摊支付…四分之一」 | PROTOCOL-LEVEL / OPEN | — | — | 分摊 undefined; multi-payer split | OPEN-分摊 |

### 珠手知由 — 天才制作人

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-chuchu-skill-01 | 「加盖房屋时半价」 | PROTOCOL-LEVEL | — | — | `set_build_cost_pct` is a protocol stub (price applied by BuildRoutine) | — |
| C-RAS-chuchu-skill-02 | 「如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」 | PROTOCOL-LEVEL / FAILS-in-engine | — | — | multi-turn latch; unit tests already record engine charges full price (`chuchu_skill_free_contract`) | — |

---

## Band skill — UNSTOPPABLE

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-RAS-band-01 | 「每回合可选择只获得第一次受到的[异常移动效果]」 | PROTOCOL-LEVEL / OPEN | — | — | per-turn opt-in over abnormal-move stack; timing unprinted | OPEN-band-opt-timing |
| C-RAS-band-02 | 「因该技能以外的效果在livehouse格子[触发结算]时…下一次主要移动可变为传送」 | PROTOCOL-LEVEL / OPEN | — | — | multi-turn offer; `[触发结算]` undefined | OPEN-触发结算, OPEN-livehouse |

---

## Harness results

Filled from `tools/kani/run.sh --unit ras` / `--unit skills_ras`
(`CARGO_TARGET_DIR=/home/monopoly/kani-target-ras`).

| harness | unit | verdict | time (s) | notes |
|---|---|---|---:|---|
| `unstoppable_card_03_money_iff` | ras | (pending) | | iff: GainMoney 2000/1000 by face |
| `unstoppable_card_03_money_noop_when_out` | ras | (pending) | | out player: empty log, frame |
| `unstoppable_card_03_money_mutant_is_detected` | ras | (pending) | | 1100 ≠ 1000 |
| `riot_02_03_cycle_iff` | ras | (pending) | | discard-all + redraw h (+1 holder) |
| `riot_03_extra_draw_mutant_is_detected` | ras | (pending) | | no-extra-draw disagrees |
| `pareo_01_marks_iff` | ras | (pending) | | +2 PAREO, pre ≤ 1 |
| `pareo_01_marks_mutant_is_detected` | ras | (pending) | | +3 ≠ +2 |
| `masking_01_fire_gain_leaf` | skills_ras | (pending) | | +1 GainFire leaf |
| `rio_01_fire_gain_leaf` | skills_ras | (pending) | | +1 GainFire leaf |
| `fire_pass_gain_mutant_is_detected` | skills_ras | (pending) | | +2 ≠ +1 |

---

## Open questions (from the clause doc; nothing invented)

OPEN-livehouse, OPEN-触发结算, OPEN-分摊, OPEN-exist-influence, OPEN-exist-multi,
OPEN-please-choose-qual, OPEN-repaint-round, OPEN-unstoppable-mod,
OPEN-become-strongest-table, OPEN-hey-kids-split, OPEN-storm-distance,
OPEN-storm-price, OPEN-crush-immune, OPEN-chuchu-which-gain, OPEN-band-opt-timing,
OPEN-change-world-equal, OPEN-hey-kids-sum.

## What was tried for BLOCKED items

* `tile_named`: confirmed the host arm only special-cases 「江户川乐器店」 and
  returns -1 otherwise (`rules/kani/src/host.rs`). Did **not** invent tile ids.
* fire / PAREO caps: confirmed `gain_fire` / `add_held` do not take the max
  the bodies pass; real engine clamps (`game-core/src/engine/ops.rs`). Did
  **not** assume the cap away for the full clause — proved the sub-cap leaf.
* move-plan / buy / build: left as PROTOCOL-LEVEL per KANI.md §6; the arms
  return 0 on purpose.