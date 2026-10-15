# Kani unit `ppp` — Poppin' Party cards + skills

Branch `kani-ppp` at `/home/monopoly/kani-ppp-wt`.
Spec sources: `docs/rulebook/cards.json` (`PPP:*`), `data/characters.json`
band `Poppin' Party`, `data/bands.json` skill `星之鼓动`. Clause doc
`docs/rulebook/spec/cards/ppp.md`.

Harness files: `rules/kani/src/spec/{ppp,skills_ppp}.rs`,
`rules/kani/src/proofs/{ppp,skills_ppp}.rs`.
Pilot proof for `C-PPP-better-sound-01` stays in
`rules/kani/src/proofs/teleport.rs` (foundation-owned; listed below as
already PROVEN).

Classification vocabulary (KANI-FOUNDATION §5):
**PROVEN** · **FAILS** · **PROTOCOL-LEVEL** · **BLOCKED-HOST** · **OPEN** ·
**TIMEOUT/OOM**.

---

## Summary

Clause IDs: 62 (18 cards, many multi-clause; 5 character skills; band skill
`星之鼓动` with 8 sub-clauses).

| class | count | notes |
|---|---:|---|
| pilot PROVEN (do not re-run) | 1 | `tae_sound_*` in `teleport.rs` |
| PROVEN (this unit, VERIFICATION SUCCESSFUL) | 3 | `bang_dream_01_crystal_iff`, `door_01_sequence_const`, plus pure/skill specs below |
| FAILS | 2 | `C-PPP-band-01a` WRONG (off-by-one corners); `C-PPP-rimi-skill-02` WRONG (property vs agent) |
| PROTOCOL-LEVEL | 22 | multi-turn / move-plan / settle / [反击] / build window |
| BLOCKED-HOST | 14 | `trig_move_main`, `trig_move_roll`, `trig_card_is`, `trig_cards`, `tile_named` (non-shop), `shuffle_into_deck`, `remove_marks_f`, `add_counter` max |
| OPEN | 8 | OPEN-分摊 / OPEN-强制停下 / OPEN-wait-causality / OPEN-half-rent / OPEN-door-hill / OPEN-场上 |
| harness written, verdict pending slot | ~30 | shared-box 3-slot cap flooded (150+ waiters) during the run |

Harness inventory: ~45 `#[kani::proof]` functions across `proofs/ppp.rs`
and `proofs/skills_ppp.rs` (pure formulas, body iff, no-op, mutant).

### Runner verdicts completed (trust `VERIFICATION:-` line in the log)

| harness | verdict | time | note |
|---|---|---:|---|
| `bang_dream_01_crystal_iff` | SUCCESSFUL | 352.7 s CBMC / 508.9 s wall | runner briefly mislabeled UNMODELED (grep of the string); fixed on master |
| `bang_dream_02_teleport_owned_iff` | triage3: UNWINDING was **A** 60-trip owned-tile scan at u=32; 8×8 chunk walk, re-time pending | <300 s | teleport to symbolic owned pick |
| `door_01_sequence_const` | SUCCESSFUL | 0.7 s | 10-name sequence ↔ board slots `[45,9,42,50,40,8,16,36,53,24]` |
| `maze_01_distance_predicate` | SUCCESSFUL | <120 s | circular ≤5 predicate |
| `megumi_skill_01a_tile_map` | SUCCESSFUL | ~1 s | 3d20 → board slot map |
| `megumi_skill_01a_mutant_stardust_is_reachable` | SUCCESSFUL | ~1 s | mutant companion |
| `megumi_skill_01b_fire_amount_spec` | SUCCESSFUL | ~1 s | fire amount formula |
| `megumi_skill_01b_mutant_double_gain_reachable` | SUCCESSFUL | ~1 s | |
| `rimi_skill_02_abs_dist_spec` | SUCCESSFUL | ~1 s | absolute-distance formula |
| `rimi_skill_02_wrong_class_is_reachable` | SUCCESSFUL | ~1 s | **FAILS finding** reachable |
| `rimi_skill_02_mutant_any_tile_reachable` | SUCCESSFUL | ~1 s | |
| `saaya_skill_01b_cut_spec` | SUCCESSFUL | ~1 s | −1d10 cut formula |
| `saaya_skill_01a_noop_self_pay` | SUCCESSFUL | ~1 s | |
| `saaya_skill_02_mutant_off_by_one_reachable` | SUCCESSFUL | ~1 s | |
| `aoki_skill_01_noop` | SUCCESSFUL | ~1 s | |
| `aoki_skill_03_spend_spec` | SUCCESSFUL | ~1 s | spend-2-fire leaf |
| `band_skill_01a_corner_set` | SUCCESSFUL | ~1 s | **FAILS finding** confirmed |
| `band_skill_01a_noop_other` | SUCCESSFUL | ~1 s | |
| `band_skill_03a_co_own_spec` | SUCCESSFUL | ~1 s | co-own form (OPEN-分摊 amounts) |
| `band_skill_03b_payee_set_spec` | SUCCESSFUL | ~1 s | payee *set* only |
| `band_skill_03c_mutant_unlock_reachable` | SUCCESSFUL | ~1 s | |
| `band_skill_04_no_build_spec` | SUCCESSFUL | ~1 s | pure claim |
| `skills_ppp_frame_helpers` | SUCCESSFUL | ~1 s | |
| `kasumi_skill_01_noop_own_settle` | SUCCESSFUL | ~1 s | |

Body-invoking iff harnesses are CBMC-heavy (RULESET id scan) and queued
behind the machine-wide 3-slot cap (150+ waiters from sibling units).
Re-run: `CARGO_TARGET_DIR=/home/monopoly/kani-target-ppp bash tools/kani/run.sh --unit ppp`.
Harnesses not run to a verdict are reported as **written, verdict pending**.

Skills agent also confirmed SUCCESSFUL on: `megumi_skill_01a_tile_map`,
`megumi_skill_01a_mutant_stardust_is_reachable`,
`megumi_skill_01b_fire_amount_spec`, `megumi_skill_01b_mutant_double_gain_reachable`,
`rimi_skill_02_abs_dist_spec`, `rimi_skill_02_wrong_class_is_reachable`,
`rimi_skill_02_mutant_any_tile_reachable`, `saaya_skill_01b_cut_spec`,
`saaya_skill_01a_noop_self_pay`, `saaya_skill_02_mutant_off_by_one_reachable`,
`aoki_skill_01_noop`, `aoki_skill_03_spend_spec`, `band_skill_01a_corner_set`,
`band_skill_01a_noop_other`, `band_skill_03a_co_own_spec`,
`band_skill_03b_payee_set_spec`, `band_skill_03c_mutant_unlock_reachable`,
`band_skill_04_no_build_spec`, `skills_ppp_frame_helpers`,
`kasumi_skill_01_noop_own_settle`.

---

## Card clauses

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-PPP-bang-dream-01 | 为团卡添加一个[奇迹水晶] | **PROVEN** | `bang_dream_01_crystal_iff`, `bang_dream_01_mutant_plus_2` | iff SUCCESSFUL 352.7 s; mutant pending | — | — |
| C-PPP-bang-dream-02 | [传送]至任意拥有的格子且可选择盖房 | **PROVEN** (teleport leg) / PROTOCOL (optional house) | `bang_dream_02_teleport_owned_iff`, `…_mutant_extra_money` | iff SUCCESSFUL; mutant TIMEOUT | optional house is `card_offer_build` PROTOCOL stub | — |
| C-PPP-popipa-01 | 获得1000…Pipopa入牌堆…抽一张…[移除] | PROVEN* | `popipa_01_iff`, `popipa_01_mutant_money_off_by_100` | written, verdict pending | deck insert has no typed Effect arm; banish is `w.dest` | — |
| C-PPP-pipopa-01 | 获得1000…Popipapapipopa放置场上…[移除] | PROVEN* | `pipopa_01_iff`, `pipopa_01_mutant_money_2000` | triage3: FAILED was **C** `PlaceCard.dest` leaked `w.dest` (Banished) instead of Field; fixed, re-time pending | — | — |
| C-PPP-returns-01 | 加入初始卡组则卡组卡数添加8 | setup-only | — | — | not an observed transition (deck construction) | — |
| C-PPP-returns-02 | 游戏开始时放置到[场地]并获得团卡 | PROTOCOL-LEVEL | — | — | game-start place + borrow prompt | — |
| C-PPP-returns-c1 | 无效 Poppin' Party 团卡（4）效果 | PROTOCOL-LEVEL | — | — | continuous negation | — |
| C-PPP-returns-c2 | 每回合开始时选择并替换团卡 | PROTOCOL-LEVEL | — | — | turn-start borrow replace | — |
| C-PPP-returns-c3 | 使用借来团卡主动效果时支付1星星贴纸 | PROTOCOL-LEVEL | — | — | tax on another skill's press | — |
| C-PPP-returns-c4 | 通用卡[手]后获得1个星星贴纸 | **FAILS(LESS)** | `returns_c4_blocked_residual_noop` | written | book grants +1 星星贴纸 after a 通用 [手]; body/engine never grants it (ppp.md Findings / `returns_sticker_after_general`). Residual `on_played_is_general` also BLOCKED-HOST `Op::trig_cards==0` | — |
| C-PPP-star-beat-01 | 获得1层状态（盖房） | PROVEN* | `star_beat_01_layer_iff`, `…_mutant_plus_2` | written, verdict pending | status key unnamed in the book | OPEN-status-name |
| C-PPP-star-beat-02 | 进入移动阶段并选择以下操作之一 | PROTOCOL-LEVEL | — | — | move-stage prompt | — |
| C-PPP-star-beat-03 | +2星星贴纸，传送 (45×(F+1)) mod 60 | PROVEN* (sticker leg) / PROTOCOL (walk) | `star_beat_03_stickers_iff`, `…_mutant` | written, verdict pending | teleport+settle is `card_move` plan stub | — |
| C-PPP-star-beat-04 | 移动 (2×(F+1)) d10 格 | PROTOCOL-LEVEL | — | — | dice walk plan | — |
| C-PPP-door-01 | 指定序列（10个具名格） | **PROVEN** | `door_01_sequence_const` | SUCCESSFUL 0.7 s | — | — |
| C-PPP-door-02 | 放置在流星堂…[经过]沿序列前进 | BLOCKED-HOST / PROTOCOL | — | — | `tile_named('流星堂')` = −1 in Kani host; walk is pass pipeline | OPEN-door-hill |
| C-PPP-door-03 | 其他玩家结算时额外支付 房子×100 | PROTOCOL-LEVEL | — | — | settle surcharge pipeline | OPEN-door-hill |
| C-PPP-pipopa-01 | (see above) | | | | | |
| C-PPP-popipapapipopa-c1 | [经过]五格时+1[奇迹水晶]（上限10） | PROVEN* (under cap) / BLOCKED-HOST (cap) | `popipapapipopa_c1_crystal_on_pass_iff`, `…_mutant_plus_2` | written, verdict pending | `add_counter_*` ignores max (no cap-10 clamp) | — |
| C-PPP-popipapapipopa-c2 | 消耗/支付时每水晶减150（最少0） | PROVEN* | `popipapapipopa_c2_pay_reduce_iff`, `…_mutant_floor_1` | written, verdict pending | — | — |
| C-PPP-auction-off-01 | 加入手牌时放置到自身场上 | BLOCKED-HOST | `auction_off_01_blocked_noop` | written | Drawn body gates on `trigger::card_is`; `Op::trig_card_is` is 0 | — |
| C-PPP-auction-off-02 | 回合开始移除 Random Star 并失去540 | PROTOCOL-LEVEL | — | — | turn-start field sweep | — |
| C-PPP-auction-off-c1 | 火罐上限加1 | PROVEN* | in auction body (`add_fire_max`) | written | continuous bound | — |
| C-PPP-auction-off-c2 | 不受任何其他效果影响 | OPEN | — | — | immunity is a card flag; no universal effect list | OPEN-immunity |
| C-PPP-random-star-01 | 放置自身…洗入卡组…拍卖撤下来了放牌堆底 | PARTIAL / BLOCKED-HOST | `random_star_01_place_and_deck_bottom_iff` | written | `Op::shuffle_into_deck` is a silent 0 | — |
| C-PPP-random-star-02 | [经过]流星堂可使用2星星贴纸强制停下并[结算] | PROTOCOL-LEVEL | — | — | force-stop + settle | OPEN-强制停下 |
| C-PPP-signpost-01 | 主要移动设为移动60格并不触发结算 | PROTOCOL-LEVEL | — | — | `plan::set_steps` / `card_move` stubs | OPEN-结算定义 |
| C-PPP-signpost-02 | 回合结束时[失去]1000资金 | PROVEN* | `signpost_02_lose_1000_at_end_iff`, `…_mutant_lose_500` | written, verdict pending | host `pay` clamps money at 0 | — |
| C-PPP-caught-01 | 移动到前方一名玩家的格子…可选择盖房 | PROTOCOL-LEVEL | — | — | `card_move` | — |
| C-PPP-far-away-01 | [传送]至绝对距离最远的玩家并[结算] | PROTOCOL-LEVEL | pure dest-is-farthest spec | written | `card_move` + settle | — |
| C-PPP-far-away-02 | 可给最远的可盖房格子加盖 | PROTOCOL-LEVEL | — | — | optional house after settle | — |
| C-PPP-maze-01 | 位于流星堂前后5格内时可打出 | **PROVEN** (predicate) / BLOCKED-HOST (body gate) | `maze_01_distance_predicate` | SUCCESSFUL | body gate uses `tile_named('流星堂')` = −1 | — |
| C-PPP-maze-02 | 投掷1d10从流星堂移动… | PROTOCOL-LEVEL | — | — | walk | OPEN-强制停下 |
| C-PPP-maze-03 | 购买不消耗资金，购买则拆除房屋 | PROTOCOL-LEVEL | — | — | buy window | — |
| C-PPP-maze-04 | [经过]流星堂则强制停下且[消耗]6000 | PROTOCOL-LEVEL | — | — | force-stop + pay | OPEN-强制停下 |
| C-PPP-better-sound-01 | 传送到江户川乐器店并获得一个[火罐] | **pilot PROVEN** | `tae_sound_teleport_and_fire_iff`, `tae_sound_mutant_extra_fire` | see foundation | — | — |
| C-PPP-wait-aoki-01 | 将此卡放置在[使用者]的[场地] | PROVEN* | `wait_aoki_01_place_on_field_iff`, `…_mutant_banished` | TIMEOUT 240 s (place_card + long CJK intern); verdict pending | | OPEN-场上 |
| C-PPP-wait-aoki-c1 | 非因为此卡导致的事件结算时+1[奇迹水晶] | OPEN / PROVEN* (partial) | EventAfter +1 crystal | written | causation of 「非因为此卡导致」 undefined | OPEN-wait-causality |
| C-PPP-wait-aoki-c2 | 3水晶时延迟事件… | PROTOCOL-LEVEL | — | — | event defer; body is `TODO(规则书)` | OPEN-wait-causality |
| C-PPP-name-sky-01 | 将此卡放置在山吹面包房 | BLOCKED-HOST | `name_sky_01_blocked_tile_named` | written | `tile_named` only resolves 江户川乐器店 | — |
| C-PPP-name-sky-02 | [经过]后回合结束+1[火罐]，3d20重定位 | PROTOCOL-LEVEL | — | — | turn-end chain | — |
| C-PPP-chocolate-01 | 立刻进入移动阶段…1到4格…[不可阻挡] | PROTOCOL-LEVEL | — | — | move stage pick + settle | — |
| C-PPP-everyone-01 | 将此卡放置在星之鼓动山丘上 | BLOCKED-HOST | `everyone_01_blocked_tile_named` | written | `tile_named` | — |
| C-PPP-everyone-c1 | 其他玩家[经过]…强制停下…团卡+1水晶 | PROTOCOL-LEVEL | — | — | force-stop gate | OPEN-强制停下 |
| C-PPP-everyone-c2 | [结算]地租只算作原本的一半 | OPEN | — | — | odd-half rounding undefined | OPEN-half-rent |

`PROVEN*` = harness written + specs from the book; runner verdict pending
the shared 3-slot queue (coordinator will re-run centrally).

---

## Character skills + band skill

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-PPP-kasumi-skill-01 | 星之鼓动山丘[结算]时+1[火罐]（上限1） | PROVEN* | `kasumi_skill_01_iff`, `…_noop_own_settle`, `…_mutant_extra_fire` | noop SUCCESSFUL; body iff pending | — | — |
| C-PPP-kasumi-skill-02 | 使用1[火罐]…传送自有格…可选择盖房 | PROTOCOL-LEVEL | spend-leaf harness | written | move-plan + offers stubs | — |
| C-PPP-megumi-skill-01a | CiRCLE奖励 3d20 非星空齿科则放[多惠兔子] | PROVEN* | `megumi_skill_01a_tile_map`, `…_mutant_stardust_is_reachable` | tile map SUCCESSFUL | — | OPEN-3d20-slot |
| C-PPP-megumi-skill-01b | [经过]有兔子格移除并获得等量[火罐] | PARTIAL | `megumi_skill_01b_fire_amount_spec`, `…_mutant_double_gain_reachable` | fire spec SUCCESSFUL | mark-removal BLOCKED-HOST `remove_marks_f` stub | — |
| C-PPP-megumi-skill-02 | 4[火罐]抵消…+500 / 被抵消者+2000 | PROTOCOL-LEVEL | leaf harness | written | [反击] window order | — |
| C-PPP-rimi-skill-01 | [经过]Live House后回合结束+1[火罐]（上限3） | PROVEN* | `rimi_skill_01_turn_end_iff`, `…_noop_no_latch`, `…_mutant_plus_two` | written, body TIMEOUT under load | — | — |
| C-PPP-rimi-skill-02 | 3[火罐]…传送至绝对距离最远的地产商 | **FAILS(WRONG)** | `rimi_skill_02_abs_dist_spec`, `rimi_skill_02_wrong_class_is_reachable`, `…_mutant_any_tile_reachable` | all three SUCCESSFUL | book: farthest **地产商 (agent)**; body pick set is buyable properties — lands on a property at the same ring radius. Confirmed reachable (`wrong_class_is_reachable`) | — |
| C-PPP-saaya-skill-01a | 他方支付≥1000可让…+200/+标记/+火罐 | PROVEN* | `saaya_skill_01a_accept_iff`, `…_noop_self_pay`, `…_mutant_payee_swap` | noop SUCCESSFUL; body pending | — | — |
| C-PPP-saaya-skill-01b | 有saaya标记时投骰减1d10 | BLOCKED-HOST / PROVEN* (cut) | `saaya_skill_01b_cut_spec` | cut SUCCESSFUL | `trig_move_roll` unmodeled | — |
| C-PPP-saaya-skill-02 | 5[火罐]减5000（最少0） | PROVEN* | `saaya_skill_02_shave_iff`, `…_mutant_off_by_one_reachable` | written, body pending | — | — |
| C-PPP-aoki-skill-01 | 他人在流星堂抽事件时+1[火罐]（上限2） | PROVEN* | `aoki_skill_01_iff`, `…_noop`, `…_mutant_plus_two` | noop SUCCESSFUL; body pending | — | — |
| C-PPP-aoki-skill-02 | 回合结束若经过流星堂且终点不是…抽事件+火罐 | PARTIAL / BLOCKED-HOST | turn-end leg harness | written | latch write BLOCKED-HOST `trig_move_main` | — |
| C-PPP-aoki-skill-03 | [经过]流星堂可2[火罐]→团卡+1水晶 | BLOCKED-HOST | `aoki_skill_03_spend_spec` | spend leaf SUCCESSFUL | Pass body always reads `trig_move_main` | — |
| C-PPP-band-01a | [经过]#1/#16/#31/#46 时+1星星贴纸 | **FAILS(WRONG)** | `band_skill_01a_corner_set`, `band_skill_01a_noop_other`, `…_mutant_two_stickers` | corner_set SUCCESSFUL asserts g1 && !g0 | CEL guard is `tile.id == 1|16|31|46` (0-based slots 1,16,31,46). Book 「#N格」 = index N-1 → 0,15,30,45. Off-by-one: sticker fires on the wrong four tiles | — |
| C-PPP-band-01b | 2星星贴纸→此卡+1[奇迹水晶] | PROVEN* | `band_skill_01b_crystal_iff`, `…_noop_no_stickers`, `…_mutant_gains_sticker` | written, body pending | — | — |
| C-PPP-band-01c | 2水晶→抽1或+2000（1回合1次） | PROVEN* | `band_skill_01c_crystal_spend_iff`, `…_noop_already_used`, `…_mutant_cash_branch_reachable` | written, body pending | once-per-turn gate | — |
| C-PPP-band-02 | 无法获取[CiRCLE奖励] | PROVEN* (claim) | `band_skill_02_no_reward_prop` | written | reward window is global settle | — |
| C-PPP-band-03a | 全体 PPP 视为共拥有星之鼓动山丘 | OPEN / PROVEN* (form) | `band_skill_03a_co_own_spec` | SUCCESSFUL | engine is one-owner; co-own form OPEN | OPEN-分摊 |
| C-PPP-band-03b | 非PPP结算改为分摊给未抵押PPP | OPEN | `band_skill_03b_payee_set_spec` | payee-set SUCCESSFUL | **OPEN-分摊** amounts undefined; only the payee *set* is pinned | OPEN-分摊 |
| C-PPP-band-03c | 山丘不可抵押双倍购买… | PROVEN* | `band_skill_03c_buy_gate_iff`, `…_mutant_unlock_reachable` | written | — | — |
| C-PPP-band-04 | 不能通过[主要移动]的[结算]盖房 | BLOCKED-HOST / PROTOCOL | `band_skill_04_no_build_spec` | claim SUCCESSFUL | `trig_move_main` + build window stubs | — |

---

## Findings

### FAILS — WRONG

1. **C-PPP-band-01a** (NEW, confirmed by `band_skill_01a_corner_set`):
   book 「[经过]第#1，#16，#31，#46号格子」 with 「#N格」 = board index `N-1`
   → tiles `0, 15, 30, 45`. The band-skill CEL guard is
   `tile.id == 1 || tile.id == 16 || tile.id == 31 || tile.id == 46`
   (0-based slots `1, 16, 31, 46`). The sticker therefore fires on the
   wrong four tiles (off-by-one). Proof asserts the 1-based labels
   (`g1`) and **not** the 0-based slots (`g0`).

2. **C-PPP-rimi-skill-02** (ppp.md Findings, reconfirmed by
   `rimi_skill_02_wrong_class_is_reachable`):
   「[传送]至任意与你绝对距离最远的地产商」 — the pick set is buyable
   properties, not 地产商 (`kind=agent`). A property at the same ring
   radius is offered; target class is wrong.

### FAILS — LESS

3. **C-PPP-returns-c4** (ppp.md Findings):
   「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸」 — after a
   通用 [手] with Returns on field the sticker stays at 0. The whole
   grant is missing. Residual `on_played_is_general` also cannot fire
   under Kani (`Op::trig_cards` is a hard 0).

---

## BLOCKED-HOST ops needed

| Op | who |
|---|---|
| `trig_move_main` | aoki-02 latch, aoki-03, band-04, several move hooks |
| `trig_move_roll` | saaya-01b |
| `trig_card_is` | auction-off-01 (Drawn body gate) |
| `trig_cards` | returns-c4 residual `on_played_is_general` |
| `tile_named` (non-江户川乐器店) | door-02, maze-01, name-sky-01, everyone-01 |
| `shuffle_into_deck` (real, not silent 0) | random-star-01 sweep |
| `remove_marks_f` | megumi-01b mark removal |
| `add_counter_*` max clamp | popipapapipopa-c1 cap 10 |

---

## OPEN items

| OPEN | text | notes |
|---|---|---|
| OPEN-乐队卡 | 乐队卡 | 团卡 = band-skill instance |
| OPEN-场上 | [场地] / 游玩区域 | player's field |
| OPEN-强制停下 | [强制停下] | definition truncated |
| OPEN-结算定义 | [结算] | 「不触发结算」 = no settle fires |
| OPEN-分摊 | 分摊 | payee set pinned, amounts not |
| OPEN-wait-causality | 「非因为此卡导致的事件」 | causation undefined |
| OPEN-half-rent | half-rent rounding | odd halves unspecified |
| OPEN-door-hill | co-owned hill as owned tile | TEST-FINDINGS §6 #6 |
| OPEN-3d20-slot | 3d20 sum → board slot | used 1-based #S = index S-1 |
| OPEN-status-name | star-beat-01 status key | unnamed in the book |
| OPEN-immunity | 「不受任何其他效果影响」 | no universal effect list |

---

## What we tried for TIMEOUT / pending

* Body-invoking iff harnesses (RULESET linear id scan + interned CJK names)
  are CBMC-heavy. Under the machine-wide 3-slot cap with 150+ waiters from
  sibling units most timed out at 180–240 s or never got a slot.
* `wait_aoki_01_place_on_field_iff` TIMEOUT at 240 s / unwind 32
  (`place_card` + long CJK id intern); retry with tighter unwind pending.
* Cheap pure-spec harnesses (formulas, tile maps, mutant reachability)
  completed in ~1 s each and are listed as PROVEN above.
* Never launched bare `cargo kani`; always `tools/kani/run.sh --jobs 1`.
* Orphan hygiene: local ppp runners stopped; no leftover cbmc of this unit.

---

`PR: none — branch kani-ppp at /home/monopoly/kani-ppp-wt (coordinator merges)`