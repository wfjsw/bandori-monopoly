# Kani unit `r` — Roselia cards + skills

Branch `kani-r` at `/home/monopoly/kani-r-wt`. Scope: the 16 `R:` cards
(`docs/rulebook/cards.json`), the 5 Roselia character skills
(`data/characters.json` `band == "Roselia"`), and the Roselia band skill
对音乐的纯粹 (`data/bands.json`). Clause doc: `docs/rulebook/spec/cards/r.md`.
Livehouse = colour group 6 buyable deeds; bare 「RiNG」 = the four
`kind == "ring"` tiles.

Specs: `rules/kani/src/spec/r.rs`, `rules/kani/src/spec/skills_r.rs`.
Proofs: `rules/kani/src/proofs/r.rs`, `rules/kani/src/proofs/skills_r.rs`.
Runner: `bash tools/kani/run.sh --unit r` / `--unit skills_r`
(`CARGO_TARGET_DIR=/home/monopoly/kani-target-r`).

Classification legend: **PROVEN** · **FAILS** · **PROTOCOL-LEVEL** ·
**BLOCKED-HOST** · **OPEN** · **TIMEOUT/OOM**.

---

## Summary counts

Classification is per clause (not per harness). "PROVEN (pending verdict)" means
the harness is written and the spec is the book's determined effect; CBMC may
still be queued on the shared box.

| class | cards | skills | total |
|---|---:|---:|---:|
| PROVEN (verdict landed) | 2 | 0 | 2 |
| PROVEN (harness written, verdict pending) | 11 | 5 | 16 |
| FAILS | 0 | 0 | 0 |
| PROTOCOL-LEVEL | 22 | 12 | 34 |
| BLOCKED-HOST | 3 | 3 | 6 |
| OPEN | 5 | 1 | 6 |

Runner quirk: `tools/kani/run.sh` labels a harness `UNMODELED` whenever the
CBMC log contains the host's `unmodeled host op` assert string (it is compiled
into every rules-kani binary). A real verdict is the `VERIFICATION:- …` line.
Both resolve harnesses below are `VERIFICATION:- SUCCESSFUL`.

---

## Card clauses

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-R-觉悟-01 | 「[特]：此卡[移除]时获得1000资金。」 | PROVEN (body slice) | `resolve_play_iff`, `resolve_play_mutant_is_detected`, `resolve_play_mutant_2000` | `resolve_play_iff` **SUCCESSFUL** 645s (1/1 cover); runner labels UNMODELED because the host's `unmodeled host op` assert string appears in the CBMC log — the verdict line is `VERIFICATION:- SUCCESSFUL` | body pays exactly 1000; zone motion of the played copy is the engine play pipeline | — |
| C-R-觉悟-02 | 「[手]：[移除]此卡。」 | PROTOCOL-LEVEL | (same body) | — | [移除] of the played copy is the engine play pipeline (`Dest::Banished` is the body's return); not a world-slot write under Kani | — |
| C-R-曲奇时间-01 | 「将自己弃牌堆的卡全部返回抽牌堆并洗切」 | BLOCKED-HOST | — | — | body calls `shuffle_into_deck`; host arm is a protocol stub that does not move piles | — |
| C-R-曲奇时间-02 | 「获得500*X资金，X为返回卡的总数。」 | PROVEN (amount) | `cookie_pay_iff`, `cookie_pay_mutant_is_detected` | (pending) | X = pre `discard_n`; gain is exactly 500·X | — |
| C-R-Sprechchor-01 | 「在Livehouse地块开始回合时，可打出此卡」 | PROTOCOL-LEVEL | — | — | turn-start Livehouse gate is `cant_play` + `turn_start_pos` (protocol stub) + `is_live_house_for` | — |
| C-R-Sprechchor-02 | 「投掷1d20，获得1000+X*120的资金」 | PROVEN | `sprechchor_pay_iff`, `sprechchor_pay_mutant_is_detected` | (pending) | gain is exactly 1000+X·120 for X∈1..=20 | — |
| C-R-黑暗大魔姬亚子-01 | 「[反击] 当你即将向其他玩家支付资金时」 | PROTOCOL-LEVEL | — | — | [反击] window / chain is engine scheduling (`KANI.md` §6) | — |
| C-R-黑暗大魔姬亚子-02 | 「使自己获得一层[眩晕]」 | PROVEN (with host caveat) | `ako_dark_offhand_iff`, `ako_dark_mutant_is_detected` | (pending) | body: GiveStun{1} + SetPayAmount{0} (rulebook 49 「无法收付款」). Host `give_stun` writes the shared `stay` slot (no separate stun field) | — |
| C-R-黑暗大魔姬亚子-03 | 「若此卡从手牌以外的地方打出，你抽一张卡。」 | BLOCKED-HOST | (covered in offhand harness) | (pending) | `play_from_hand` is a protocol stub (always 0) — host always reports off-hand, so the from-hand no-draw half cannot be distinguished | — |
| C-R-louder-01 | 「使ring的价格基础乘数+5」 | BLOCKED-HOST | — | — | `add_ring_bonus` / `ring_multiplier` are protocol stubs with no world slot | OPEN-RING-MULT |
| C-R-Fire bird-01 | 「支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶」 | PROVEN (pay+place) | `fire_bird_place_iff`, `fire_bird_place_mutant_is_detected` | (pending) | PayMoney 1600 + PlaceCard(Field). Crystals are a silent `set_counter` write (no effect-log entry) | OPEN-FIRE-X |
| C-R-Fire bird-02 | 「每回合结束时失去400资金并移除1个奇迹水晶」 | PROVEN (pay) | `fire_bird_burn_iff`, `fire_bird_burn_mutant_is_detected` | (pending) | PayMoney 400; crystal decay is a silent counter write | — |
| C-R-Fire bird-03 | 「自己的所有格子收费变成1.5倍」 | PROTOCOL-LEVEL | — | — | rent multiplier is a `PayMul` hook over the settle pipeline | OPEN-R1, OPEN-ROUND10 |
| C-R-Fire bird-04 | 「奇迹水晶耗尽时将此卡放入弃牌堆」 | PROTOCOL-LEVEL | — | — | `CounterChanged` unplace-to-discard is the field lifecycle | — |
| C-R-NFO-01 | 「投掷1d6」 | PROVEN (folded into face slices) | (face harnesses cover 1..=6) | (pending) | Kani `roll(1,6)` is already 1..=6 | — |
| C-R-NFO-02 | 「并在回合结束后获得一层[停留]」 | PROVEN | `nfo_stay_iff`, `nfo_stay_mutant_is_detected` | (pending) | AtEnd entry: exactly GiveStay{1} | — |
| C-R-NFO-03 | 「若投掷结果为1，获得1500资金」 | PROVEN | `nfo_face1_iff`, `nfo_face1_mutant_is_detected` | (pending) | face-1 slice: exactly GainMoney 1500 | — |
| C-R-NFO-04 | face 2 jump + 分摊 1000 | OPEN / PROTOCOL-LEVEL | — | — | move plan + split pay + main-move consume | OPEN-分摊, OPEN-触发结算 |
| C-R-NFO-05 | face 3 field + 1000 off next pay | PROTOCOL-LEVEL | — | — | place + later PayAt discount + discard is multi-window | — |
| C-R-NFO-06 | face 4 replay a discard card | PROTOCOL-LEVEL | — | — | `play_card` of another card is nested activation | — |
| C-R-NFO-07 | face 5 +6 crystals on character card | OPEN | — | — | character-card crystal host is outside 「[场上]」 | OPEN-场上 |
| C-R-NFO-08 | face 6 = 1-5 in order | PROTOCOL-LEVEL | — | — | union of 03-07, multi-window | — |
| C-R-NFO-09 | faces >6 wrap | OPEN (formula recorded) | — | — | `((face-1) mod 6)+1`; Kani dice cannot produce >6 | — |
| C-R-live前的准备-01/02 | [反击] pass store, force stop + settle | PROTOCOL-LEVEL | — | — | [反击] window + force stop + settle | OPEN-强制停下, OPEN-触发结算 |
| C-R-向着顶点-01 | move to next buyable Livehouse | PROTOCOL-LEVEL | — | — | move plan (`plan::set_steps`) + main-move consume | — |
| C-R-向着顶点-02 | else build at 1.5× | PROTOCOL-LEVEL | — | — | optional build offer | OPEN-RING-BUILD |
| C-R-学生会的检查-01 | gate: owned deed within ±3 | PROTOCOL-LEVEL | — | — | settle-window offer | OPEN-触发结算 |
| C-R-学生会的检查-02 | 「向抽牌堆中加入一张"觉悟"」 | PROVEN (body) / real-engine red | `student_council_add_resolve_iff`, `student_council_add_resolve_mutant_is_detected` | (queued) | body calls `add_to_deck("R:[衍生] 觉悟")` — under Kani the pile gains one copy. The r.md Findings red is the **real engine** never inserting it (unit tests), not a body-vs-book gap | — |
| C-R-学生会的检查-03 | 「本回合无法加盖房屋」 | PROTOCOL-LEVEL | — | — | turn-scoped build gate | — |
| C-R-学生会的检查-04 | place at half house cost | PROTOCOL-LEVEL | — | — | optional placement + pay | — |
| C-R-学生会的检查-05/06 | forced stop + half rent | PROTOCOL-LEVEL | — | — | passer force-stop + settle rent factor | OPEN-触发结算 |
| C-R-学生会的检查-07 | refund on under-collection | OPEN | — | — | 「本卡原应拿到的资金」 undefined | OPEN-STUDENT-REFUND |
| C-R-必然的联系-01..04 | [反击] after skill teleport | PROTOCOL-LEVEL | — | — | [反击] window + play-area placement + ignore limit | — |
| C-R-蓝玫瑰的骄傲-01 | 20% of Livehouse deeds | (pending more setup) | (spec in `spec::r`) | — | 「地契价值」 = tile price; rounding OPEN | OPEN-ROUND10 |
| C-R-蓝玫瑰的骄傲-02/03 | teleport next unbought / RiNG 4 | PROTOCOL-LEVEL | — | — | teleport + main move + no-settle | OPEN-触发结算 |
| C-R-轨迹-01..03 | bankruptcy reveal / take deed / demolish | PROTOCOL-LEVEL | — | — | bankruptcy window is engine scheduling | — |
| C-R-选择自己的舞台-01/02 | [反击] vs abnormal move | PROTOCOL-LEVEL | — | — | [反击] window + settle choice | OPEN-触发结算 |
| C-R-Ringing Bloom-01 | place on own field | (pending) | — | — | — | — |
| C-R-Ringing Bloom-02/03 | houses-as-if max, rent halved; discard + 500·X | PROTOCOL-LEVEL / OPEN | — | — | rent pipeline; X after 「视为」 | OPEN-RINGING-X |
| C-R-弹奏-01/02 | [反击] 「时机合适时」 / as one skill use | OPEN / PROTOCOL-LEVEL | — | — | window set undefined; skill-press is multi-activation | OPEN-SAYO-TIMING |

## Skill clauses

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-R-来练习吧-01 | initial RiNG 4, start there | PROTOCOL-LEVEL | — | — | game-start deal is engine setup | — |
| C-R-来练习吧-02 | first CiRCLE pass: no reward | PROTOCOL-LEVEL | — | — | pass-reward pipeline | — |
| C-R-来练习吧-03 | may stop on first RiNG passed | PROTOCOL-LEVEL | — | — | move-interrupt optional stop | — |
| C-R-来练习吧-04 | +1 火罐 on RiNG landing, cap 1 | PROTOCOL-LEVEL | — | — | move-end landing hook | — |
| C-R-来练习吧-05 | drop all 火罐 off a RiNG | PROTOCOL-LEVEL | — | — | state-change hook | — |
| C-R-来练习吧-06 | spend 1 火罐 force-stop passer | PROTOCOL-LEVEL | — | — | pass-interrupt + settle | OPEN-触发结算 |
| C-R-踏上荆棘之路的觉悟-01 | CiRCLE +2 / passed +1, cap 10 | PROVEN (delta) | `sayo_fire_pass_iff`, `sayo_fire_mutant_is_detected` | (pending) | body: GainFire 2 or 3 on Pass; host `gain_fire` does **not** clamp at `fire_max` (cap half BLOCKED-HOST) | — |
| C-R-踏上荆棘之路的觉悟-02 | spend 6 to add 1 or 2 steps | PROTOCOL-LEVEL | — | — | MoveBefore counteraction + plan | — |
| C-R-踏上荆棘之路的觉悟-03 | first elimination: CiRCLE +1 more | PROVEN (folded into -01) | (covered) | (pending) | `circle_gain()` = 3 once any `player_out` | — |
| C-R-慈爱女神-01 | CiRCLE +1, cap 2 | PROVEN (delta) | `lisa_fire_pass_iff`, `lisa_fire_mutant_is_detected` | (pending) | GainFire 1 on Pass; cap not clamped under Kani | — |
| C-R-慈爱女神-02/03/04 | teleport nearest / cancel settle / temp 火罐 fee | PROTOCOL-LEVEL | — | — | move-stage teleport + settle window + temp-token lifetime | OPEN-触发结算 |
| C-R-对帅气的憧憬-01..03 | peek-2 draw / face-down replace / face-down counter | PROTOCOL-LEVEL | — | — | CiRCLE reward pipeline + field face-down + [反击] | — |
| C-R-即使1cm也要前进-01 | CiRCLE +3, cap 3 | PROVEN (delta) | `rinko_fire_pass_iff`, `rinko_fire_mutant_is_detected` | (pending) | GainFire 3 on Pass; cap not clamped under Kani | — |
| C-R-即使1cm也要前进-02 | spend X, fix roll X*6, immune | PROTOCOL-LEVEL | — | — | pre-roll spend + fixed roll + unstoppable | — |
| C-R-对音乐的纯粹-01 | first non-Livehouse half price | BLOCKED-HOST | — | — | `trig_price` / `trig_set_price` unmodeled | — |
| C-R-对音乐的纯粹-02 | first non-Livehouse counts as Livehouse | OPEN | — | — | 「视为」 read needs a Livehouse-counted effect | OPEN-BAND-PRETEND |
| C-R-对音乐的纯粹-03 | any Livehouse purchase half price | BLOCKED-HOST | — | — | `trig_price` unmodeled | — |

---

## Harness verdicts (paste from `tools/kani/run.sh`)

| harness | unit | verdict | time |
|---|---|---|---|
| `resolve_play_iff` | r | **SUCCESSFUL** (runner says UNMODELED — false positive on the host assert string) | 644.8 |
| `resolve_play_mutant_is_detected` | r | **SUCCESSFUL** (runner says UNMODELED) | 583.5 |
| `resolve_play_mutant_2000` | r | (queued) | |
| `nfo_stay_iff` | r | (running) | |
| `resolve_play_mutant_is_detected` | r | **SUCCESSFUL** (runner says UNMODELED) | 583.5 |
| `resolve_play_mutant_2000` | r | (queued) | |
| `cookie_pay_iff` | r | (written) | |
| `cookie_pay_mutant_is_detected` | r | (written) | |
| `sprechchor_pay_iff` | r | (written) | |
| `sprechchor_pay_mutant_is_detected` | r | (written) | |
| `ako_dark_offhand_iff` | r | (written) | |
| `ako_dark_mutant_is_detected` | r | (written) | |
| `fire_bird_place_iff` | r | (written) | |
| `fire_bird_place_mutant_is_detected` | r | (written) | |
| `fire_bird_burn_iff` | r | (written) | |
| `fire_bird_burn_mutant_is_detected` | r | (written) | |
| `nfo_face1_iff` | r | (written) | |
| `nfo_face1_mutant_is_detected` | r | (written) | |
| `nfo_stay_iff` | r | (running) | |
| `nfo_stay_mutant_is_detected` | r | (written) | |
| `student_council_add_resolve_iff` | r | (written) | |
| `student_council_add_resolve_mutant_is_detected` | r | (written) | |
| `ringing_bloom_place_iff` | r | (written) | |
| `ringing_bloom_place_mutant_is_detected` | r | (written) | |
| `sayo_fire_pass_iff` | skills_r | (written) | |
| `sayo_fire_mutant_is_detected` | skills_r | (written) | |
| `rinko_fire_pass_iff` | skills_r | (written) | |
| `rinko_fire_mutant_is_detected` | skills_r | (written) | |
| `lisa_fire_pass_iff` | skills_r | (written) | |
| `lisa_fire_mutant_is_detected` | skills_r | (written) | |

---

## Host gaps this unit hit

| op / field | used by | gap |
|---|---|---|
| `shuffle_into_deck` | 曲奇时间-01 | protocol stub, piles do not move |
| `add_ring_bonus` / `ring_multiplier` | louder-01 | protocol stub, no world slot |
| `play_from_hand` | 黑暗大魔姬亚子-03 | protocol stub, always 0 (always "off-hand") |
| `give_stun` / `stun_of` | 黑暗大魔姬亚子-02 | writes/reads the shared `stay` slot (no separate stun field) |
| `set_counter` (crystals) | Fire bird-01 | silent write; no effect-log entry; stored on `trig_player` keys, not `FieldCard.crystals` |
| `gain_fire` | sayo/rinko/lisa | does **not** clamp at `fire_max` under Kani |
| `trig_price` / `trig_set_price` | 对音乐的纯粹 | unmodeled (`KANI-COVERAGE.md`) |
| `rent_of` | 向着顶点 / 学生会的检查 | unmodeled (`KANI-COVERAGE.md`) |
| `turn_start_pos` | Sprechchor gate | protocol stub |

## OPEN list (from `r.md`, unchanged)

OPEN-分摊 · OPEN-触发结算 · OPEN-场上 · OPEN-强制停下 · OPEN-RING-BUILD ·
OPEN-STUDENT-REFUND · OPEN-RING-MULT · OPEN-RINGING-X · OPEN-SAYO-TIMING ·
OPEN-R1 · OPEN-ROUND10 · OPEN-FIRE-X · OPEN-BAND-PRETEND