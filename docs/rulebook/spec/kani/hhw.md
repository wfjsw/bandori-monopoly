# Kani unit `hhw` — Hello, Happy World!

Branch: `kani-hhw` at `/home/monopoly/kani-hhw-wt`.
Scope: 15 `HHW:` cards (`docs/rulebook/spec/cards/hhw.md`, `docs/rulebook/cards.json`),
5 Hello, Happy World! character skills (`data/characters.json`), band skill 传播笑容
(`data/bands.json`). Spec sources: `rules/kani/src/spec/hhw.rs` + `spec/skills_hhw.rs`.
Proof sources: `rules/kani/src/proofs/hhw.rs` + `proofs/skills_hhw.rs`.

Method: specs are pure predicates written FROM the clause doc quotes only.
Harnesses invoke the unmodified bodies via `invoke::`. Guards use `guard_id!` /
`guard!` against the lowered single-source IR. Narration is not the frame.

## Classification counts

| class | cards | character skills | band | total |
|---|---:|---:|---:|---:|
| PROVEN | (see table) | (see table) | (see table) | (see table) |
| FAILS (finding) | 0 | 2 (known, pre-registered) | 0 | 2 |
| BLOCKED-HOST | 3 rows | 1 row | 0 | 4 |
| PROTOCOL-LEVEL | many clauses | several | 1 clause | — |
| OPEN | several clauses | several | 1 clause | — |

The three card rows blocked on unmodeled ops (KANI-COVERAGE.md) are listed
**BLOCKED-HOST** and left there:

| row | blocked-by |
|---|---|
| `HHW:梦幻的回礼` | `rent_of` |
| `HHW:爱心义演` | `tile_at` |
| `HHW:黑衣人的补给` | `tile_at` |

Plus the skill row `skill:松原花音:真正的迷子` is blocked on `trig_move_roll`.

## Per-clause table

Legend: **P**=PROVEN, **F**=FAILS(finding), **B**=BLOCKED-HOST, **PROTO**=PROTOCOL-LEVEL,
**O**=OPEN, **—**=not Kani-expressible this pass (subclass of PROTO/O).

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-HHW-hlsy-01 | 「投掷1d4mod4…上，下，左，右…」 | O | — | — | — | OPEN-HHW-DIR (face→direction, face 0, geometry) |
| C-HHW-hlsy-02 | 「传送至直线距离最远的格子…主要移动」 | O | — | — | — | OPEN-HHW-DIR |
| C-HHW-backstage-01 | 「看牌堆顶2张…0~2张…放回…翻入弃牌堆」 | O / partial | `backstage_pays_1000_per_discard_iff` (count) | pending | — | OPEN-HHW-SHORT-PILE; order is player choice |
| C-HHW-backstage-02 | 「每翻入一张获得1000资金」 | P | `backstage_pays_1000_per_discard_iff` | pending | — | — |
| C-HHW-believe-01 | 「至少有另一张手牌时可发动」 | O (residual gate) | isolation assume | — | — | — |
| C-HHW-believe-02 | 「消耗800资金」 | P | `believe_cost_800_iff` | pending | — | — |
| C-HHW-believe-03 | 「…选择一张放入弃牌堆」 | P (count) | `believe_discard_one_iff` | pending | — | OPEN-HHW-BELIEVE-CHOOSER |
| C-HHW-believe-04 | 「…选择两张加入你的手牌，重洗」 | P (count) | `believe_discard_one_iff` | pending | — | OPEN-HHW-RESHUFFLE |
| C-HHW-patrol-01 | 「付款并在任意自己的格子加盖一层房屋」 | O | — | — | — | OPEN-HHW-PATROL-PRICE |
| C-HHW-patrol-02 | 「投掷3d20…额外免费加盖…（若为可建造格子）」 | O | — | — | — | OPEN-HHW-PATROL-3D20, OPEN-HHW-BUILDABLE |
| C-HHW-return-01 | 「[反击]…即将被不属于你的格子收费时」 | **B** + PROTO | — | BLOCKED-HOST | — | counter window + `rent_of` |
| C-HHW-return-02 | 「向…任意一名玩家的一个格子进行一次支付」 | **B** + O | — | BLOCKED-HOST | — | OPEN-HHW-RETURN-AMOUNT |
| C-HHW-return-03 | 「自动使用一次你的乐队技能进行双倍支付」 | **B** + PROTO | — | BLOCKED-HOST | — | — |
| C-HHW-return-04 | 「添加两个…奇迹水晶」 | **B** + O | — | BLOCKED-HOST | — | OPEN-HHW-RETURN-BOOK |
| C-HHW-return-05 | 「减免相当于你支付金额的数额」 | **B** + O | — | BLOCKED-HOST | — | OPEN-HHW-RETURN-RELIEF |
| C-HHW-balloon-01 | 「投掷4次3d20mod60并记录」 | P (shape) | `balloon_teleports_to_a_chosen_result_iff` | pending | — | wrap ruling 0≡60 |
| C-HHW-balloon-02 | 「选择其中之一，传送…主要移动」 | P | `balloon_teleports_to_a_chosen_result_iff` | pending | — | — |
| C-HHW-charity-01 | 「每初次经过一个属于你的格子…+2」 | **B** | — | BLOCKED-HOST (`tile_at`) | — | OPEN-HHW-CHARITY-RECOMPUTE |
| C-HHW-charity-02 | 「付款减半（向上取整10）」 | **B** (row-level) | — | BLOCKED-HOST (`tile_at`) | — | formula pinned in spec doc |
| C-HHW-charity-03 | 「若弦卷心已将专属卡置于CiRCLE上…」 | **B** | — | BLOCKED-HOST (`tile_at`) | — | — |
| C-HHW-parade-01 | 「[反击]经过弦卷集团…放置…3个奇迹水晶」 | PROTO | — | — | — | OPEN-HHW-触发结算 |
| C-HHW-parade-02 | 「每回合结束时移除一个…为0时…弃牌堆」 | PROTO | — | — | — | OPEN-HHW-触发结算 (card known stack-overflow) |
| C-HHW-parade-03 | 「[持续] [触发结算]后可将…放置于[移动终点]」 | PROTO | — | — | — | OPEN-HHW-触发结算 |
| C-HHW-parade-04 | 「那格视为与弦卷集团交换位置」 | PROTO | — | — | — | tile-swap standing effect |
| C-HHW-parade-05 | 「[触发结算]后此卡放入弃牌堆」 | PROTO | — | — | — | OPEN-HHW-触发结算 |
| C-HHW-athletic-01 | 「放置于自己场上并放置3个奇迹水晶」 | P | `athletic_place_three_crystals_iff` | pending | — | — |
| C-HHW-athletic-02 | 「每回合结束时失去一个，为0时置入弃牌堆」 | P / F? | `athletic_turn_end_decay_iff`, `athletic_empty_discards` | pending | body may only `set_dest(Graveyard)` without unplace → LESS | — |
| C-HHW-athletic-03 | 「每次掷骰获得一次资金，起始为700…下限100」 | P (spec) | (spec only this pass) | — | host `gain_fire` ignores cap (n/a here) | — |
| C-HHW-athletic-04 | 「重骰…直至结果至少为10」 | P (spec) | (spec only) | — | inclusive floor ruling | — |
| C-HHW-supply-01 | 「[反击] 经过CiRCLE…放置一个奇迹水晶」 | **B** | — | BLOCKED-HOST (`tile_at`) | — | — |
| C-HHW-supply-02 | 「该格获得CiRCLE格子的全部效果」 | **B** + PROTO | — | BLOCKED-HOST (`tile_at`) | — | OPEN-HHW-CIRCLE-FX |
| C-HHW-supply-03 | 「经过弦卷集团后，移除那格的一个奇迹水晶」 | **B** | — | BLOCKED-HOST (`tile_at`) | — | — |
| C-HHW-kkr-01 | 「支付10000资金（视为买地花费）」 | P | `kkr_pay_10000_iff` | pending | — | — |
| C-HHW-kkr-02 | 「将此卡置于CiRCLE上」 | P (weak) | `kkr_place_on_circle_iff` | pending | host `place_card_on` arg layout may drop the tile | — |
| C-HHW-kkr-03 | 「若其他玩家…[触发结算]…支付6000」 | O / PROTO | — | — | — | OPEN-HHW-触发结算, OPEN-HHW-KKR-OWNER |
| C-HHW-misaki-01 | 「[反击] 使用火罐进行移动掷骰后…」 | PROTO | — | — | — | counter window |
| C-HHW-misaki-02 | 「消耗所有火罐…传送…触发结算…主要移动」 | PROTO + O | — | — | — | OPEN-HHW-触发结算 |
| C-HHW-misaki-03 | 「那格及相邻2格…支付你500」 | PROTO + O | — | — | — | OPEN-HHW-NEIGHBOUR |
| C-HHW-hagumi-01 | 「投掷2次4d20」 | P (shape) | `hagumi_over_60_gains_and_draws_iff` | pending | — | — |
| C-HHW-hagumi-02 | 「若…大于60，获得1000资金并抽一张卡」 | P | `hagumi_over_60_gains_and_draws_iff` | pending | — | — |
| C-HHW-hagumi-03 | 「经过育美标记时…强制停下并获得2000」 | PROTO | — | — | — | multi-act force-stop |
| C-HHW-hagumi-04 | 「[反击]…回合外收到资金的回合结束时」 | PROTO | — | — | — | OPEN-HHW-HAGUMI-TIME |
| C-HHW-wacha-01 | 「[场]…倒走时此卡获得一个水母标记」 | O | — | — | — | OPEN-HHW-BACKWARDS |
| C-HHW-wacha-02 | 「至少9个水母标记…传送…弃牌堆」 | P (spec) | (spec only) | — | — | — |
| C-HHW-phantom-01 | 「放置于场上（充能3，衰减1）」 | P (spec) | (spec only) | — | — | OPEN-HHW-CHARGE |
| C-HHW-phantom-02 | 「向一名玩家场上放置一个怪盗标记」 | P (spec) | (spec only) | — | own field is OPEN | — |
| C-HHW-phantom-03 | 「经过该玩家时使自己强制停下并触发结算」 | PROTO | — | — | — | OPEN-HHW-触发结算 |
| C-HHW-phantom-04 | 「主要移动改为投掷1d2…1d10」 | PROTO | — | — | — | multi-act path rewrite |
| C-HHW-kokoro-01 | 「开局时获得1000资金」 | P | `kokoro_01_opening_1000_iff` | **running / pending** | — | 「开局时」 latched at first TurnStartBefore |
| C-HHW-kokoro-02 | 「[经过]CiRCLE时额外获得1500资金」 | P | `kokoro_02_circle_pass_1500_iff` | pending | — | — |
| C-HHW-kaoru-01 | 「不存在[怪盗标记]…获得1火罐…存在…移除一个」 | F (known) | `kaoru_01_pot_arm_iff` | pending | **FINDING**: body's mark-existence check does not see the counter the card plants; 3-step walk granted +3 fire and left the mark (TEST-FINDINGS) | — |
| C-HHW-kaoru-02 | 「消耗7火罐…放置3个[怪盗标记]」 | F (known) | (not scripted this pass) | — | **FINDING**: press spends 7 fire and places **0** marks | — |
| C-HHW-kaoru-03 | 「非传送的主要移动不会经过…格子」 | PROTO | — | — | — | whole-walk shape |
| C-HHW-hagumi-skill-01 | 「每个回合在北泽精肉店生成一个可乐饼」 | PROTO | — | — | — | multi-turn spawn |
| C-HHW-hagumi-skill-02 | 「经过…可乐饼…转移到自己场上」 | PROTO | — | — | — | multi-act transfer |
| C-HHW-hagumi-skill-03 | 「其他角色…支付50*X」 | PROTO | — | — | — | multi-act + X is post-transfer count |
| C-HHW-hagumi-skill-04 | 「可用其替换掉一个…标记」 | PROTO | — | — | — | — |
| C-HHW-hagumi-skill-05 | 「每拥有一个可乐饼，多投掷1个1d2」 | P (spec) | (spec only) | — | — | — |
| C-HHW-kanon-01 | 「游戏开始起点为#30弦卷豪宅」 | **B** (row) | (spec `kanon_01_start_ok`) | BLOCKED-HOST (`trig_move_roll` on the row) | — | — |
| C-HHW-kanon-02 | 「两次移动掷骰并将它们的结果相减」 | **B** | — | BLOCKED-HOST (`trig_move_roll`) | — | OPEN-HHW-KANON-NEG |
| C-HHW-kanon-03 | 「三个回合内没有拿过Circle奖励…1d20+1d4」 | **B** + PROTO | — | BLOCKED-HOST + multi-turn clock | — | — |
| C-HHW-misaki-skill-01 | 「经过弦卷集团…获得2火罐（上限2）」 | P | `misaki_skill_01_pass_fire_iff` | pending | host `gain_fire` has no cap clamp — iff may FAIL (MORE) | — |
| C-HHW-misaki-skill-02 | 「可将弦卷集团额外视为CiRCLE」 | PROTO | — | — | — | tile-kind override missing |
| C-HHW-misaki-skill-03 | 「消耗一个火罐…额外1d10…视为[传送]」 | PROTO | — | — | — | — |
| C-HHW-misaki-skill-04 | 「获得一层…[除外]」 | P (spec) / PROTO | (Play body) | — | — | — |
| C-HHW-band-01 | 「可选择支付双倍价格…奇迹水晶」 | P | `band_01_declined_is_noop`, `band_01_accepted_doubles_iff` | pending | — | OPEN-HHW-BAND-RECORD |
| C-HHW-band-02 | 「移除一个奇迹水晶，记录该次收款的价格」 | P (spec) | (spec only) | — | — | OPEN-HHW-BAND-RECORD |
| C-HHW-band-03 | 「从盖房花费中减去所记录的数字…归零」 | O / PROTO | — | — | — | needs a legal build window |

## Harness verdicts (runner)

Filled in as `tools/kani/run.sh --unit hhw|skills_hhw --harness <name>` completes.
Machine-wide slot cap: 3 concurrent CBMC; queueing is normal.

| harness | unit | verdict | time |
|---|---|---|---|
| `kokoro_01_opening_1000_iff` | skills_hhw | pending | — |
| `kokoro_01_mutant_500` | skills_hhw | pending | — |
| `kokoro_01_mutant_is_detected` | skills_hhw | pending | — |
| `kokoro_01_noop_when_latch_set` | skills_hhw | pending | — |
| `kokoro_02_circle_pass_1500_iff` | skills_hhw | pending | — |
| `kokoro_02_noop_non_circle` | skills_hhw | pending | — |
| `kokoro_02_mutant_2000` | skills_hhw | pending | — |
| `kokoro_02_mutant_is_detected` | skills_hhw | pending | — |
| `misaki_skill_01_pass_fire_iff` | skills_hhw | pending | — |
| `misaki_skill_01_noop_other_tile` | skills_hhw | pending | — |
| `misaki_skill_01_mutant_is_detected` | skills_hhw | pending | — |
| `kaoru_01_pot_arm_iff` | skills_hhw | pending | — |
| `kaoru_01_mutant_is_detected` | skills_hhw | pending | — |
| `band_01_declined_is_noop` | skills_hhw | pending | — |
| `band_01_accepted_doubles_iff` | skills_hhw | pending | — |
| `band_01_mutant_is_detected` | skills_hhw | pending | — |
| `kkr_pay_10000_iff` | hhw | pending | — |
| `kkr_place_on_circle_iff` | hhw | pending | — |
| `kkr_mutant_pay_1000` | hhw | pending | — |
| `kkr_mutant_is_detected` | hhw | pending | — |
| `athletic_place_three_crystals_iff` | hhw | pending | — |
| `athletic_turn_end_decay_iff` | hhw | pending | — |
| `athletic_decay_to_zero_discards` | hhw | pending | — |
| `athletic_empty_discards` | hhw | pending | — |
| `believe_cost_800_iff` | hhw | pending | — |
| `believe_discard_one_iff` | hhw | pending | — |
| `believe_mutant_is_detected` | hhw | pending | — |
| `backstage_pays_1000_per_discard_iff` | hhw | pending | — |
| `backstage_mutant_is_detected` | hhw | pending | — |
| `balloon_teleports_to_a_chosen_result_iff` | hhw | pending | — |
| `balloon_mutant_is_detected` | hhw | pending | — |
| `hagumi_over_60_gains_and_draws_iff` | hhw | pending | — |

## BLOCKED-HOST ops needed

| Op | rows | note |
|---|---|---|
| `rent_of` | C-HHW-return-* (`HHW:梦幻的回礼`) | unmodeled in `host.rs` |
| `tile_at` | C-HHW-charity-*, C-HHW-supply-* | unmodeled |
| `trig_move_roll` | C-HHW-kanon-* (`skill:松原花音:真正的迷子`) | unmodeled |

(KANI-COVERAGE.md lists 3 HHW card rows blocked; the skill row is a fourth
blocked-by `trig_move_roll`. Listed BLOCKED-HOST and left alone per the brief.)

## Findings (real, from the clause doc + body)

1. **C-HHW-kaoru-01** (TEST-FINDINGS §6, pre-registered): with a live
   [怪盗标记] planted by `HHW:（薰）怪盗hello happy`, a 3-step own walk granted
   +3 火罐 and left the mark untouched. The skill's existence check does not
   see the counter the printed card plants.
2. **C-HHW-kaoru-02** (TEST-FINDINGS §6, pre-registered): the press spends the
   7 火罐 and answers the nearest-player pick, but places **0** [怪盗标记].
3. **C-HHW-athletic-02** (candidate, harness `athletic_empty_discards`): the
   crystals-changed hook only `set_dest(Graveyard)` + log; if it does not
   unplace / discard the card, 「为0时置入弃牌堆」 is LESS.
4. **C-HHW-misaki-skill-01** (candidate): book 「上限2」; host `gain_fire` has
   no cap clamp and the body calls `gain_fire(2)` directly — a pre-fire of 1
   would become 3. Iff proof may FAIL (MORE).
5. **Host `place_card_on` arg layout** (host-model note, not a card body bug):
   `Op::place_card_on` is invoked as `[player, tile, cp, cl, ptr, len]` but
   `host.rs` reads the card name from `(a1, a2) = (tile, cp)`. This can make
   `C-HHW-kkr-02` land on `Dest::Field` instead of CiRCLE in the model. The
   harness covers both outcomes and pins the weaker “a PlaceCard happens”.

## What was tried for BLOCKED / OOM

* BLOCKED rows: not proven. Exact missing Op listed above. No silent zeros.
* The kani runner is queued behind other units' CBMC jobs (machine-wide slot
  cap 3). Harness times are recorded as they complete.

## Spec inventory (pure functions)

`spec/hhw.rs`: backstage_02_*, believe_02/03/04_*, kkr_01/02_*, athletic_01–04_*,
balloon_02_*, hagumi_02_*, wacha_02_*, kokoro_01/02_*, misaki_skill_01_*,
band_01/02/03_*, log_empty, money_frame_unchanged, piles_unchanged + mutants.

`spec/skills_hhw.rs`: re-exports of the skill clauses above + kaoru_01/02_*,
hagumi_skill_05_*, kanon_01/02_*, band_03_* + mutants.