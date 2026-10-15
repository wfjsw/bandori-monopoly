# Consolidated findings — book vs implementation

Every genuine rulebook-vs-engine discrepancy found by the formal-verification
effort, deduplicated by clause ID. Sources: red `rules/tests/tests/spec_*.rs`
regression tests, Kani unit-table **FAILS** rows, and the clause docs'
`## Findings` sections. Not invented: rows whose source is unclear are
**CANDIDATE**.

Snapshot 2026-10-12. A discrepancy is a plain failing test (no `#[ignore]`).

## Totals

### By class

| class | count |
|---|---:|
| **LESS** (engine does less than the book) | 24 |
| **MORE** (engine does more than the book) | 8 |
| **WRONG** (engine does something else) | 11 |
| **all** | **43** |

(Primary class per row. Dual-tag rows — e.g. G-STATUS-02 WRONG+MORE — are
counted under the first tag; the dual is named in the row.)

### By evidence status

| status | count | meaning |
|---|---:|---|
| **CONFIRMED-by-Kani** | 3 | a landed Kani verdict (SUCCESSFUL counterexample / FAILED book assert) confirms the gap |
| **REGRESSION-ONLY** | 38 | a named red `spec_*.rs` test is the evidence (Kani harness may be written but pending) |
| **CANDIDATE** | 2 | only a pending Kani FAILS row exists, or the source marks the row uncertain |
| **all** | **43** | |

---

## Global

### G-CARDS — hand limit

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| G-CARDS-01 | cards | LESS | 「基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。」 (`rulebook-doc.md:126`) | `draw_cards` draws past the limit and only auto-discards for `p.ai`. A human seat sits at 5→6 over limit 5 with no discard prompt. | `g_cards_01_human_over_draw_must_discard_down` (green contrast `g_cards_01_bot_discards_down_to_the_limit`) | REGRESSION-ONLY |

### G-STATUS — statuses

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| G-STATUS-02 | status | WRONG (also MORE) | 「[主要移动]的[移动终点]视为当前所在格子且不[结算]」; 行动阶段 row 4 「眩晕效果发动：跳过经营和主要移动阶段」 | A [晕眩]/[除外] in force at the `roll` still walks to another tile and settles. `skip_move` is latched only for [停留] at turn start. | `stun_in_force_at_roll_does_not_walk` | REGRESSION-ONLY |
| G-STATUS-06 | status | MORE | 「[除外]：…无法收付款」 | [除外] payee is not gated (only [晕眩] is). Rent of 760 lands on an `exile=1` owner. | `exile_owner_cannot_receive_rent` (green contrast `stun_owner_cannot_receive_rent`) | REGRESSION-ONLY |

### G-SETTLE — buy / build timing

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| G-SETTLE-06 | settle | LESS | 「无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。」 (`rulebook-doc.md:155`) | Main landing only *announces* (`log.land_unowned`) and defers the optional buy to the end step. The book makes the optional buy **the settle**. | `g_settle_06_unowned_buy_is_the_settle` | REGRESSION-ONLY |
| G-SETTLE-07 | settle | LESS | 「如果格子地契未抵押则可选择[消耗]格子地契所标注的房屋建筑费进行升级建造…」 (`rulebook-doc.md:157`) | Same deferral for the own-land build offer (`log.land_own` announce only). | `g_settle_07_own_build_is_the_settle` | REGRESSION-ONLY |

### G-FLOW — seat bounds

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| G-FLOW-12 | flow | MORE | 「游玩人数为3到10人。」 (`rulebook-doc.md:107`) | **FIXED 2026-10-15**: `MatchMode::check_seats` carries the bound; web-glue `SoloMatch::new` and rules-worker `new` refuse. `Match::new` stays a harness constructor (unit tests hand-build small tables). | `casual_requires_3_to_10_players` | was REGRESSION-ONLY |
| G-FLOW-14 | flow | MORE | 「游玩人数须在5到6人。」 (`rulebook-doc.md:117`) | **FIXED 2026-10-15**: same `check_seats` / lobby refusal. | `ranked_requires_5_to_6_players` | was REGRESSION-ONLY |

---

## Cards / skills by band

### AG — Afterglow

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-AG-yolo-01 | ag | WRONG (zone) | 「…使结果增加1d4结果的数字」 + 「使用后的非[持续]手卡进入弃卡区」 | +1d4 lands, but the used non-[持续] card stays in hand (discard empty). Intermittent in `forall` (some seeds take a [反击] path that consumes it). | `yolo_adds_1d4` | REGRESSION-ONLY |

### CRYCHIC

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-CRYCHIC-ifonly-02 | crychic | LESS | 「若你的手牌大于等于7，此卡立即置入弃牌堆」 | Body only `set_dest(Graveyard)` + narration; never unplaces the card into discard. | Kani `ifonly_02_discard_at_seven_iff` (FAILS/LESS, verdict pending) | CANDIDATE |
| C-CRYCHIC-band-01b | crychic | MORE | 「手牌数大于等于6时…无法从手中打出任何牌」 | No play-gate body (TODO in `skill-bands/src/crychic.rs`); engine accepts plays at hand 6 and runs the body. | `crychic_band_blocks_play_at_six` | REGRESSION-ONLY |
| C-CRYCHIC-band-03 | crychic | LESS | 「每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶（上限10）」 | Logic is book-correct when `SEEN==4`, but SEEN only tracks draws (non-draw 4→5 missed) and host `state_set` value drop means live games never see 4. | `crychic_band_crystal_on_four_to_five` | REGRESSION-ONLY |

### HHW — Hello, Happy World!

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-HHW-kaoru-01 | hhw | WRONG | 「若场上不存在[怪盗标记]，获得1火罐…；若场上存在[怪盗标记]，则移除场上的一个[怪盗标记]」 | With a live [怪盗标记] planted by `HHW:（薰）怪盗hello happy`, a 3-step walk granted +3 火罐 and left the mark untouched (existence check misses the counter). | `kaoru_removes_a_phantom_mark_instead_of_a_pot` (TEST-FINDINGS §6) | REGRESSION-ONLY |
| C-HHW-kaoru-02 | hhw | LESS | 「消耗7火罐…放置3个[怪盗标记]」 | Press spends 7 fire pots, answers the pick, places **0** marks. | `kaoru_skill_plants_three_marks_for_seven_fire` | REGRESSION-ONLY |

### MOR — Morfonica

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-MOR-nnm-03 | mor | WRONG | 「获得x次经过CiRCLE时的资金奖励」 (sheet 2026-10-06 G7: x × current cycle reward 1000/1500/2000) | Body hard-codes `x * 2000` (`TODO(规则书)`); disagrees whenever the cycle reward is 1000 or 1500. | Kani FAILS row `nnm_03_amount_iff_fails_wrong` / `nnm_03_wrong_is_reachable` (verdict pending) | CANDIDATE |
| C-MOR-nnm-05 | mor | LESS | 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」 | Spending the last crystal via the (3) press leaves the card on the field with 0 crystals; no discard hook. | `nnm_last_crystal_discards_the_card`; Kani `nnm_05_empty_discard_iff_fails_less` (pending) | REGRESSION-ONLY |

### MUJICA — Ave Mujica

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-MUJICA-视线-01 | mujica | WRONG (also LESS) | 「对你打出过[反击]的所有玩家…移动1~4」 | Body force-moves the immediate counter-user only (book: full turn's counteractor set). Engine also moves every other player when nobody countered. | `look_away_only_moves_counteractors` | REGRESSION-ONLY |
| C-MUJICA-欢迎-02 | mujica | WRONG | 「转换任意一名玩家的状态（…无效果）」 (「任意一名」 includes self; no 状态2 ⇒ no effect) | Prompt omits the user; toggles a non-状态2 seat instead of no-opping. | `welcome_offers_self_as_well` (LESS self), `welcome_no_state2_is_noop` (MORE) | REGRESSION-ONLY |
| C-MUJICA-团技能-01 | mujica | MORE | 「1.5倍（只对自己结算，对应的其他玩家收支不变）」 | Engine moves 1.5x on **both** sides (owner receives 450 on a 300 rent). | `band_state2_pay_is_150_only_on_self` | REGRESSION-ONLY |
| C-MUJICA-花火-03 | mujica | WRONG | 「最后一个奇迹水晶移除后…立刻使你获得2层[眩晕]」 | Engine leaves the user at 1 stun after the last crystal (turn-end layer tick). OPEN-花火眩晕掉层. | `sparkler_burn_and_last_stun` (pins 2; red at 1) | REGRESSION-ONLY |

### MYGO

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-MYGO-hitoshizuku-02 | mygo | MORE (also WRONG) | 「所有玩家…每清除一种效果则支付…1000资金」 (ruling 2026-10-06: 1000 per type **per affected player**) | Body charges `1000 * global kinds` from **every** living player (observed 3000 on stay+stun). | `hitoshizuku_clear_charge_and_stun_play` | REGRESSION-ONLY |
| C-MYGO-tomori-card-03 | mygo | LESS | 「可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗」 | Engine never spends the crystal; fire pot is taken instead. | `tomori_card_crystal_pays_fire_and_removes` | REGRESSION-ONLY |
| C-MYGO-band-04 | mygo | LESS | 「每次你的卡在未生效的情况下进入弃牌堆时…可超出上限的[奇迹水晶]（最多超出2个）」 | Over-cap crystal does not land when the band card is already at the roll cap of 1. | `band_ineffective_card_adds_over_cap_crystal` | REGRESSION-ONLY |
| C-MYGO-anon-skill-02 | mygo | LESS | 「可以消耗2个火罐，抽一张卡」 | Press accepted but neither spends pots nor draws. | `anon_skill_draw_after_abnormal_resolves` | REGRESSION-ONLY |
| C-MYGO-rana-skill-03 | mygo | WRONG | 「在space上结算时…免除付款并抽1张卡」 | Draws but still charges rent. | `rana_skill_space_and_overlap` | REGRESSION-ONLY |
| C-MYGO-rana-skill-04 | mygo | LESS | 「若自己为space的拥有者…添加一个“抹茶芭菲”…增加500资金」 | Never adds the 抹茶芭菲. | `rana_skill_parfait_on_space` | REGRESSION-ONLY |
| C-MYGO-soyo-skill-02 | mygo | LESS | 「移动掷骰后可消耗一个火罐…[移动终点]…视为对应颜色的地产商格子」 | Post-roll press is a no-op (no pot spent). OPEN-agent-colour. | `soyo_skill_endpoint_as_coloured_agent` | REGRESSION-ONLY |
| C-MYGO-normal-02 | mygo | LESS | 「使你下一次主要移动的格数变为…最近一次非传送的主要移动的移动格数」 | Copy does not run (observed 9, expected 4). | `normal_copies_last_main_move_steps` | REGRESSION-ONLY |
| C-MYGO-rana-fun-03 | mygo | MORE | 「使下一个经过的你以外的玩家选择」 (抹茶芭菲 vs 强制停下) | No choice prompt; engine auto-resolves 强制停下. | `rana_fun_choice_and_force_stop` | REGRESSION-ONLY |
| C-MYGO-murosshi-04 | mygo | WRONG | 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」 | Foreign settler pays 0 (rent vanishes) instead of redirecting to the designated player. | `murosshi_income_goes_to_designated_player` | REGRESSION-ONLY |

### PP — Pastel*Palettes

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-PP-first-accident-02 | pp | MORE | 「[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能」 | `skillBlock` token is written, but 丸山彩's (2) prompt still opens and the pay-discount runs. | `first_accident_blocks_pp_skill_2` | REGRESSION-ONLY |
| C-PP-band-idol-02 | pp | LESS | 「X大于拥有的反面的[P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]」 (cap 10) | No flip-effect trigger; overflow never becomes crystals. | `band_overflow_to_crystals` | REGRESSION-ONLY |
| C-PP-hina-lottery-02 | pp | LESS | turn start 1d4, borrow that character's (2) | `skill.hinaLottery.borrowed` stays -1 (never rolls / never borrows). | `hina_skill_2_borrows_skill` | REGRESSION-ONLY |
| C-PP-shine-again-01 | pp | LESS | 「同色地契的购买价格」÷5 **and** 「1个正面[P✽P粉丝]」 | Money leg lands (+600) but the positive fan is never granted. | `shine_again_rescue_on_short_pay` | REGRESSION-ONLY |

### PPP — Poppin' Party

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-PPP-band-01a | ppp | WRONG | 「[经过]#1，#16，#31，#46号格子时」 +1星星贴纸 | CEL guard is `tile.id == 1\|16\|31\|46` (0-based). Book 「#N格」 = index N-1 → 0,15,30,45. Off-by-one: sticker fires on the wrong four tiles. | Kani `band_skill_01a_corner_set` SUCCESSFUL (asserts g1 && !g0) | CONFIRMED-by-Kani |
| C-PPP-rimi-skill-02 | ppp | WRONG | 「[传送]至任意与你绝对距离最远的地产商」 | Pick set is buyable properties, not 地产商 (`kind=agent`); lands on a property at the same ring radius. | Kani `rimi_skill_02_wrong_class_is_reachable` SUCCESSFUL; `rimi_skill_teleports_to_farthest_agent` | CONFIRMED-by-Kani |
| C-PPP-returns-c4 | ppp | LESS | 「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」 | After a 通用 [手] with Returns on field the sticker stays at 0. | `returns_sticker_after_general`; Kani `returns_c4_blocked_residual_noop` | REGRESSION-ONLY |

### R — Roselia

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-R-学生会的检查-02 | r | LESS | 「向抽牌堆中加入一张“觉悟”」 | Counteract activation is accepted in the right window but no `R:[衍生] 觉悟` is inserted anywhere. | `student_council_contract` | REGRESSION-ONLY |

### RAS — RAISE A SUILEN

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-RAS-chuchu-skill-02 | ras | LESS | 「如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」 | Second build charged **full** house cost (1500), not 0 and not the standing half (750). Multi-turn latch missing. | `chuchu_skill_free_contract` (control `chuchu_skill_half_on_a_later_turn` green) | REGRESSION-ONLY |

### SUMIMI

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| C-SUMIMI-twoinone-01 | sumimi | LESS | 「将自己的角色卡替换为sumimi的另一名角色」 | Replaces the bound `skill:<name>:*` field entry and writes fire, but `player.character` stays the old name — the other exclusive stays unplayable. | `twoinone_opens_the_other_exclusive` | REGRESSION-ONLY |
| C-SUMIMI-band-01 | sumimi | LESS | 「若你在一回合内从其他玩家处获得过资金，你的回合结束时为此卡添加1个奇迹水晶」 | Else-half (clear crystals) runs; inbound [支付] does not add a crystal at turn end (guard never admits the add). | `band_turn_end_adds_crystal_after_inbound_money` | REGRESSION-ONLY |
| C-SUMIMI-band-02 | sumimi | LESS | 「此卡上的每个奇迹水晶可使你在向其他人收费时让金额额外提高100资金」 | Rent collection pays table rent; crystals do not raise the amount. | `band_rent_bonus_100_per_crystal` | REGRESSION-ONLY |
| C-SUMIMI-heretheworld-01 | sumimi | LESS | 「（1）[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。」 | Two [手] plays in one turn never open the [反击] window; card never moves. | `heretheworld_places_on_two_play_turn` | REGRESSION-ONLY |

---

## Events

| clause ID | unit | class | what the book says | what the implementation does | evidence | status |
|---|---|---|---|---|---|---|
| E-aao-02 | events | LESS | 「此卡在场时全场玩家不能使用卡牌的[手]效果」 | Body keeps the card and schedules expiry but arms **no** hand-effect veto (explicit `TODO(规则书)/TODO(engine)`). | Kani `aao_02_hand_veto_is_armed` **FAILED** (central `events/`); regression `aao_blocks_hand_effects` | CONFIRMED-by-Kani |

---

## Candidate / adjacent rows (detail)

| clause ID | unit | note |
|---|---|---|
| C-HHW-athletic-02 | hhw | 「每回合结束时失去一个，为0时置入弃牌堆」 — body may only `set_dest(Graveyard)` without unplace → expected LESS. Kani class P / F?. |
| C-HHW-misaki-skill-01 | hhw | 「经过弦卷集团…获得2火罐（上限2）」 — host `gain_fire` may lack a cap clamp → expected MORE. |
| C-MOR-rainbow-01 | mor | HOSTOPS-ROUND2 re-run `rainbow_01_half_iff` FAILED: spec wants `amount/2` but card is `CeilTo(amount/2, 10)`. Rounding question (see OPEN-QUESTIONS). Not folded into mor.md Summary. |
| C-MUJICA-海铃-05 | mujica | Taken band skills are detached, not 「置入弃牌堆」. Recorded shortfall; `umiri_takes_holders_band_skill` notes the TODO. |
| G-SETTLE-06 / G-SETTLE-07 (Kani) | tiles | Expected FAILS(LESS) once `trig_move_main` / `offer_buy` / `offer_build` are unblocked. Same LESS as the regression rows above. |
| events `pico_swap_01` / `pico_swap_02` | events | Central FAILED while the unit table listed them as covered — unit-vs-central disagreement, not a book finding. |
| `teleport` `tae_sound_teleport_and_fire_iff` | ppp pilot | FAILED in `teleport/`, SUCCESSFUL in `verify-fix1/` after the fix — superseded, not a live finding. |
| C-CRYCHIC-elegant-01 | crychic | Setup gap (no live-sheet card makes another player draw), **not** an engine finding. |
| C-R-向着顶点-02 | r | 「加盖一层房屋」 vs 「所有RiNG不可升级」 — newer sheet text taken as live; conflict is OPEN-RING-BUILD, not a red test. |

## Out of scope (older suite, not `spec_*` reds)

`docs/rulebook/TEST-FINDINGS.md` / `docs/rulebook/BACKLOG.md` still name
engine gaps encoded only in the older black-box suite (`#[ignore = "DISCREPANCY"]`):
multi-activation first-`On::Play`-only (C-Q6), mid-turn [停留] (C-Q7 /
OPEN-停留-中途), 安可 vs 重叠的声音, 笑容大游行 stack overflow, fuzz latent
shapes, and several `gap_*` / `cross_*` items. Those stay in TEST-FINDINGS /
BACKLOG unless a `spec_*` red or Kani FAILS row also names them.

## See also

* `OPEN-QUESTIONS.md` — book gaps the clauses must not invent
* `KANI-STATUS.md` — coverage dashboard
* Per-area Findings sections in `global/*.md`, `cards/*.md`, `events.md`,
  `kani/*.md`
* `docs/rulebook/TEST-FINDINGS.md` — older suite + §6 rulings
