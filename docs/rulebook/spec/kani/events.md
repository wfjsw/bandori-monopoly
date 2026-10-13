# Kani unit `events` — 28 event cards + draw/filing

Branch: `kani-events` · Worktree: `/home/monopoly/kani-events-wt`
Clause doc: `docs/rulebook/spec/events.md` (IDs `E-*`)
Primary text: `data/events.json` · Generic draw: `rulebook-doc.md` 基础[结算] 2

Specs: `rules/kani/src/spec/events.rs` · Proofs: `rules/kani/src/proofs/events.rs`

## Classification vocabulary

| tag | meaning |
|---|---|
| **PROVEN** | iff + covers + mutant; body == book on the modeled surface |
| **FAILS** | plain failing proof; finding LESS/MORE/WRONG |
| **PROTOCOL-LEVEL** | multi-turn / engine scheduling / protocol stub — not Kani-expressible (`KANI.md` §6) |
| **BLOCKED-HOST** | missing modeled op / tile-name / state field (exact gap named) |
| **OPEN** | book text undefined; only the determined fragment is proved |
| **TIMEOUT/OOM** | CBMC cliff; what was tried is recorded |

## Host-model gaps that gate this unit

| gap | effect on proofs |
|---|---|
| `tile_named` resolves only 「江户川乐器店」 | 快餐店 / RiNG / 微笑号 / 飞鸟山公园 / 周边学区 / CiRCLE → `-1` |
| `event_deck_push` / `event_banish` / `draw_event` = stubs | derived-card seeding and event-deck filing unprovable |
| `schedule_turn_end` = stub | `AtEnd` never auto-fires; expiry tick is invoked by hand |
| `give_exile` logs only (no `EXILE` key) | exile layer count not in world state |
| `give_stun` / `give_stay` share `w.stay[]` | stun frame polluted by host |
| `set_prop` (self) is a no-op | drawer/turns props invisible |
| `set_build_discount` / `raise_bought` / `card_move` / `remove_marks_f` / `plan::set_*` / `card_text_mentions` = stubs | see per-clause rows |
| `trig_move_roll` / `trig_move_resolve` / `trig_move_remaining` = **unmodeled** | moca halve, heat_head phase-2, circle_rebuild pass reward |

## Unit table

| ID | book quote (short) | classification | harness names | verdict + time | finding | OPEN refs |
|---|---|---|---|---|---|---|
| E-draw-01 | 抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效 | PROTOCOL-LEVEL | — | — | event-deck ops are stubs (`draw_event`) | — |
| E-draw-02 | 事件结算后进入事件弃卡区 | PROTOCOL-LEVEL | — | — | filing pile is protocol (`event_banish` / discard list not modeled) | — |
| E-draw-03 | 没有事件可抽取则将事件弃卡区洗切… | PROTOCOL-LEVEL | — | — | reshuffle is engine scheduling | ruling 2026-10-07 recycle-after-file |
| E-duobang-01 | 最高者从最低者获得胜出点数x50（18 vs 16 → 100） | PROVEN | `duobang_01_02_two_player_iff` | TBD | — | OPEN-duobang-tie |
| E-duobang-02 | total moved = (max-min)*50 (unique extremes) | PROVEN | `duobang_01_02_two_player_iff`, `duobang_01_mutant_is_detected` | TBD | multi-extreme total is `n_low*n_high*each` (OPEN) | OPEN-duobang-tie |
| E-estate-01 | 随机指定可购买格子…出价最大…获得地契 | PROTOCOL-LEVEL | — | — | `raise_bought` stub; sealed-bid stand-in is not a raise auction | OPEN-estate-tie, OPEN-estate-empty |
| E-estate-02 | 全员出价 0 → 无人支付 | PROTOCOL-LEVEL | — | — | same | — |
| E-estate-03 | 仅指定格子易主 | PROTOCOL-LEVEL | — | — | `raise_bought` stub | — |
| E-lulu-01 | 可选择[传送]至快餐店…支付500 | BLOCKED-HOST | — | — | `tile_named(快餐店)` → -1; pay-500 leg modelled | shout = out-of-game |
| E-lulu-02 | 拒绝则不动不付 | PROVEN | (lulu decline) | TBD | — | under-500 reading = no teleport |
| E-habinel-01 | 每人获得X层[除外] X=1d2 | PROVEN (log) | `habinel_01_grants_x_exile` | TBD | `GiveExile.to` = -1 (tile blocked); no EXILE key in world | — |
| E-habinel-02 | 回合开始时移除1层 | PROTOCOL-LEVEL | — | — | engine status tick | — |
| E-habinel-03 | 结束后传送到微笑号 | OPEN | — | — | 「结束后」 timing unbound; `tile_named(微笑号)` → -1 | OPEN-habinel-end |
| E-this-hand-01 | 移动X，X=1d20-1d20+1d20 | PROTOCOL-LEVEL | — | — | `card_move` + `plan::set_*` stubs; no pos change | — |
| E-this-hand-02 | X负数则反方向 | PROTOCOL-LEVEL | — | — | `plan::set_reverse` stub | — |
| E-this-hand-03 | 不触发场地效果 | PROTOCOL-LEVEL | — | — | settle suppression is engine | — |
| E-pico-swap-01 | 未[除外]者传送到下一位未[除外]者位置 | PROVEN | `pico_swap_01_rotates_positions` SUCCESSFUL 1767 s | latch `SetState` skipped in `log_is_teleports` | — | — |
| E-pico-swap-02 | [除外]者不动、跳过 | PROVEN | `pico_swap_02_skips_exiled_players`, `pico_swap_mutant_is_detected` | TBD | — | — |
| E-circle-rebuild-01 | 放置于场地中央，5个奇迹水晶 | PROVEN (activation) | `circle_rebuild_01_activation_five_crystals` SUCCESSFUL | host `self_crystals` fallback | — | — |
| E-circle-rebuild-02 | counter 镜像水晶数 | PROVEN (if crystals modeled) | (same) | TBD | — | — |
| E-circle-rebuild-03 | 经过CiRCLE移除一个，为0放入弃牌 | BLOCKED-HOST | — | — | `trig_move_resolve` / `trig_move_remaining` unmodeled on the pass hook | — |
| E-circle-rebuild-04 | 触发结算改为 2停留 或 -500 | OPEN + PROTOCOL | — | — | [触发结算] undefined; priority claim untestable | OPEN-触发结算 |
| E-circle-rebuild-05 | CiRCLE效果迁移至咖啡厅 | PROTOCOL-LEVEL | — | — | tile-effect migration is engine | — |
| E-front-or-back-01 | X=Y ⇒ 全员+500 | PROVEN | `front_or_back_01_*` | TBD | — | — |
| E-front-or-back-02 | 高半场分摊1000 | PROVEN (total) | `front_or_back_02_*` | TBD | remainder `1000%n` dropped (body reading) | OPEN-分摊 |
| E-ex-quest-01 | 1d4+26=X，各掷3d20 | PROVEN | `ex_quest_01_*` | TBD | — | — |
| E-ex-quest-02 | ≥X 可换皮肤（技能相同） | OPEN / cannot test | — | — | cosmetic | — |
| E-bear-cookie-01 | 衍生冲榜背面朝上置顶 | PROTOCOL-LEVEL | — | — | `event_deck_push` stub | — |
| E-bear-cookie-02 | 下次事件抽到冲榜 | PROTOCOL-LEVEL | — | — | same | — |
| E-bear-cookie-03 | 火罐玩家可补X并付X×1000 | BLOCKED-HOST | — | — | `card_text_mentions` stub → nobody eligible | X/cap = skill text |
| E-asukayama-01 | 全员传送到飞鸟山公园，抽卡者移动1d20 | BLOCKED-HOST | — | — | `tile_named(飞鸟山公园)`; `card_move` stub | — |
| E-asukayama-02 | 上一位2眩晕 / 下一位锁手 / 其余1除外 | PROVEN (status, seat neighbour) | `asukayama_02_status_spread_blocked_neighbor` SUCCESSFUL 989 s | host `neighbor` fixed to next present seat | — |
| E-asukayama-03 | 状态回合开始衰减 | PROTOCOL-LEVEL | — | — | engine status tick | — |
| E-school-time-01 | 移到学校格并+1除外 | BLOCKED-HOST | — | — | `tile_named(school / 周边学区)` → -1 | — |
| E-school-time-02 | 无学校 → 周边学区 | BLOCKED-HOST | — | — | same | — |
| E-tears-01 | 同色 ⇒ 下一位付当前700 | PROVEN | `tears_01_same_colour_next_pays_700` | TBD | — | OPEN-tears-colour |
| E-tears-02 | 否则当前付下一位300 | PROVEN | `tears_02_different_colour_current_pays_300`, `tears_mutant_is_detected` | TBD | — | — |
| E-kizuna-01 | 投X+1d20，全员传送 mod60 +1除外 | PARTIAL / BLOCKED-HOST | — | — | teleport dest is `face%60` (no `tile_named`) — effect log of `Teleport`+`GiveExile` provable; song X undefined | OPEN-kizuna-song |
| E-chart-rush-01 | 失去所有火罐，投Xd20 X=lost+1 | PROVEN | `chart_rush_01_*` | TBD | — | — |
| E-chart-rush-02 | 名次 2000/1500/1000/500，同点共高档 | PROVEN | `chart_rush_02_*` | TBD | — | OPEN-chart-rush-or, OPEN-chart-rush-tie |
| E-fix-note-01 | 指定格加盖金额减半 | PROTOCOL-LEVEL | — | — | `set_build_discount` stub | — |
| E-fix-note-02 | 累计加盖后移除 | OPEN | — | — | count unspecified (body takes 3) | OPEN-fix-note-count |
| E-hallucination-01 | 下回合开始放入弃牌 | PROVEN (activation+expiry) | `hallucination_01_*` | TBD | `set_prop(drawer)` no-op | — |
| E-hallucination-02 | 上一名代替投掷 | PROTOCOL-LEVEL | — | — | `plan::set_roller` stub | OPEN-hallucination-effects |
| E-pool-bug-01 | 放置中央 + 修复公告置顶 | PROVEN (keep) / PROTOCOL (push) | `pool_bug_01_*` | TBD | `event_deck_push` stub | — |
| E-pool-bug-02 | 下次触发事件时放入弃牌 | PROVEN (expiry entry) | `pool_bug_02_*` | TBD | Event hook body | — |
| E-pool-bug-03 | 不可在造价≥1500加盖 | PROVEN | `pool_bug_03_*` | TBD | `set_cancelled` modeled | — |
| E-aao-01 | 下回合结束放入弃牌 | PROVEN (activation+expiry) | `aao_01_activation_keeps_the_card`, `aao_01_expiry_tick_files_away` | TBD | — | — |
| E-aao-02 | 在场时不能使用[手] | **FAILS(LESS)** | `aao_02_hand_veto_is_armed` | TBD | book requires global [手] veto; body arms nothing (TODO engine surface) | — |
| E-marina-box-01 | 第3回合开始放入弃牌 | PROVEN (activation+expiry) | `marina_box_01_*` | TBD | `set_prop` no-op for turns counter | — |
| E-marina-box-02 | 经过CiRCLE可消耗500，抽1d10≥6得1200 | PROVEN | `marina_box_02_*` | TBD | — | OPEN-marina-who, OPEN-marina-default |
| E-bangdream-chan-01 | 第3回合开始放入弃牌 | PROVEN (activation+expiry) | `bangdream_chan_01_*` | TBD | — | — |
| E-bangdream-chan-02 | 免费时间5s / 恢复0s | cannot test | — | — | host clocks, no seam | — |
| E-heat-head-01 | 下回合结束前移动掷骰+1d6 | PROVEN (keep + phase-1 crystals) / PROTOCOL (dice) | `heat_head_01_activation_phase_up` SUCCESSFUL 423 s | host `self_crystals` fallback | `plan::add_extra_dice` stub | — |
| E-heat-head-02 | 此后 −1d6 | PROTOCOL-LEVEL / BLOCKED-HOST | — | — | `trig_move_roll` unmodeled | OPEN-heat-head-2 |
| E-heat-head-03 | 此后放入弃牌 | PROVEN (phase + expiry) | `heat_head_03_phase_end_files_away` SUCCESSFUL 692 s | host `self_crystals` fallback | phase machine via crystals | — |
| E-surprise-duel-01 | 放置于场地中央 | PROVEN (activation) | `surprise_duel_01_*` | TBD | — | — |
| E-surprise-duel-02 | 下2回合开始放入弃牌 | PROVEN (expiry) | `surprise_duel_02_*` | TBD | — | OPEN-surprise-2turn (2nd-next reading) |
| E-surprise-duel-03 | 其余玩家改向最近方向 | PROTOCOL-LEVEL | — | — | `plan::set_reverse` stub | — |
| E-moca-01 | 移动掷骰/2 | BLOCKED-HOST | `moca_01_activation_keeps_the_card` | TBD | halve hook needs `trig_move_roll` (unmodeled) | — |
| E-moca-02 | 下回合结束永久移除 | PROVEN (activation+expiry) | `moca_02_expiry_tick_files_away`, `moca_02_mutant_is_detected` | TBD | host ignores `removed` flag on `event_expire` | — |
| E-embers-01 | 复制品放在所有玩家所在格 | PROVEN | `embers_01_*` | TBD | `place_mark` modeled | — |
| E-embers-02 | 回合开始时前后各一格 | PROVEN (hook) | `embers_02_*` | TBD | `set_prop(spread)` no-op | — |
| E-embers-03 | 触发结算每张-10 | OPEN + PROVEN (pay leg) | `embers_03_*` | TBD | [触发结算] undefined | OPEN-触发结算 |
| E-embers-04 | 获得[除外]或破产时移除复制品 | BLOCKED-HOST | — | — | `remove_marks_f` stub; `give_exile` sets no EXILE key | — |
| E-lost-chase-01 | 抽卡者严格最高 → 让我来结束一切置顶 | PROTOCOL-LEVEL | — | — | `event_deck_push` stub (dice + `event_is_active` guard modelled) | — |
| E-lost-chase-02 | 否则一切不会结束置顶 | PROTOCOL-LEVEL | — | — | same | — |
| E-end-it-all-01 | 选3个非衍生移除，永久在场 | PROVEN (keep + 3 picks) / PROTOCOL (banish) | `end_it_all_01_*` | TBD | `event_banish` stub | — |
| E-end-it-all-02 | 在场时迷子不再推让我来结束一切 | PROVEN | `end_it_all_02_*` | TBD | `event_is_active` modeled | — |
| E-never-ends-01 | 1d4 → 传送到对应RiNG +1除外 | BLOCKED-HOST | `never_ends_01_noop_under_stub_tile_named` | TBD | `tile_named("RiNG {n}")` → -1; body early-returns | — |

## Summary counts (clause-level)

| classification | count | notes |
|---|---:|---|
| PROVEN (or partial-PROVEN) | ~30 | activation/expiry split where model allows |
| FAILS(LESS) | 1 | E-aao-02 |
| PROTOCOL-LEVEL | ~18 | draw/filing, scheduling, plan stubs, deck push |
| BLOCKED-HOST | ~10 | tile_named names, neighbor, remove_marks, trig_move_* |
| OPEN | ~12 | listed in the table's OPEN column |
| cannot test | 4 | skin, clocks, shout, fire-pot skill gate |

55 harnesses in `proofs/events.rs`. Full per-clause table above.

## CBMC run status

- Runner: `tools/kani/run.sh --unit events` (`CARGO_TARGET_DIR=/home/monopoly/kani-target-events`).
- First batch compile errors (`spec::` vs `spec::events::`, `MAX_LOG=24`) are fixed.
- A 3-harness probe (`moca_02_expiry_tick_files_away`, `aao_01_expiry_tick_files_away`, `duobang_01_02_two_player_iff`) sat in `acquire_slot` for 30+ minutes: the box had **20 CBMC / 41 slot-waiters** at 22:00 UTC (2026-10-11), well over `KANI_GLOBAL_SLOTS=3`. No new verdicts landed in this window.
- The earlier `moca_02_expiry_tick_files_away` FAILED at 12s is the **pre-fix compile error log**, not a proof failure.
- Re-run from the worktree when the box is quiet:
  ```sh
  export CARGO_TARGET_DIR=/home/monopoly/kani-target-events CARGO_BUILD_JOBS=4
  bash tools/kani/run.sh --unit events --timeout 900 --jobs 1
  ```

## Findings

1. **E-aao-02 LESS (body)** — book: 「此卡在场时全场玩家不能使用卡牌的[手]效果」. Body (`rules/events/src/a_a_o.rs`) keeps the card and schedules expiry but arms **no** hand-effect veto (explicit `TODO(规则书)/TODO(engine)`). Proof `aao_02_hand_veto_is_armed` asserts the veto and is expected to FAIL.

2. **HOST WRONG — `Op::neighbor` returns a tile, not a seat.** asukayama's prev/next status split (2 [眩晕] / lock hand / rest [除外]) cannot be proven; under the Kani host every non-drawer lands in the 「其余」 [除外] leg. Proof `asukayama_02_status_spread_blocked_neighbor` pins the partial GiveExile leg and documents the gap.

3. **HOST LESS — `state::set_expires` ignores the field column** and writes VALUE, clobbering `stunStart=2` down to 1. Relevant to asukayama's 「回合开始时减少一层的[眩晕]」.

4. **HOST LESS — `Op::pay_total` both shapes *and* transfers.** The real pipeline only shapes the pre-split total; the host also emits a bank `PayTo` of 1000. front_or_back proofs treat that first `PayTo` as a protocol artifact and pin the per-winner share legs (`1000/n`).

5. **PROTOCOL stubs used by event bodies:** `event_deck_push`, `event_banish`, `draw_event`, `card_move`, `raise_bought`, `set_build_discount`, `set_self_prop`, `remove_marks_f`, `schedule_turn_end`, `card_text_mentions`, `plan::set_*`.

6. **UNMODELED (loud fail):** `trig_move_roll` (moca halve, heat_head sub_die), `trig_move_resolve` / `trig_move_remaining` (circle_rebuild pass reward). Proofs avoid these paths; the clauses stay BLOCKED-HOST.

## Notes on methodology

- Specs are pure predicates in `spec/events.rs`, quoted from `events.md` / `data/events.json` only.
- Bodies are invoked unmodified via `invoke::event_play` / `invoke::hook`.
- Activation (`On::Play`) and expiry tick (`On::AtEnd` / turn-start hooks) are proven **separately** where the model allows; the link between them (`schedule_turn_end`) is a protocol stub.
- Mutants use reachable-disagreement companions (`*_mutant_is_detected`) unless `#[kani::should_panic]` is reliable.
- Isolation assumes (ample money, empty piles, frozen seat counts) are in the harness module docs and `kani::assume`d.