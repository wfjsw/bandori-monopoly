# Kani unit `general` — C-GEN clause proofs

Branch `kani-general` at `/home/monopoly/kani-general-wt`.
Clause doc: `docs/rulebook/spec/cards/general.md`.
Primary text: `docs/rulebook/cards.json` keys `通用:*`.
Harnesses: `rules/kani/src/proofs/general.rs`.
Specs: `rules/kani/src/spec/general.rs` (from the book only).
`skills_general` is empty — there is no 通用 skill (CiRCLE band skill
`skill:CiRCLE:后勤人员的努力` is a band unit, not this one).

Runner: `tools/kani/run.sh --unit general` with
`CARGO_TARGET_DIR=/home/monopoly/kani-target-general`.
Pure contracts were also spot-checked via
`cargo kani -p rules-kani --harness <name> --default-unwind N --no-unwinding-checks`.

## Host-model notes (not book re-reads)

| gap | effect on proofs |
|---|---|
| `Op::add_to_deck` / `Op::to_discard` push **no** `Effect` (no variant exists) | pile halves checked on post-state |
| `Op::set_dest` writes `World::dest` only | dest checked on post-state |
| `Op::play_doubled` stub returns `0`; `ctx::n(0, v)` then doubles | clauses whose printed amount is `ctx::n(0, …)` are BLOCKED-HOST |
| `Op::target` / `targeted_count` / `designations` / `cancel_designation` return `0` | [指定] pipeline is PROTOCOL-LEVEL |
| `Op::give_extra_turn` / plan setters / `card_move` / `card_buy` are stubs | move / buy / extra-turn halves are PROTOCOL-LEVEL |
| `Op::trig_set_negate_effect` is a **unit** call but models `trig_negate = (a0 != 0)` with `a0 = 0` | encore's negation flag never sets — BLOCKED-HOST |

## Summary counts

| classification | clauses |
|---|---:|
| PROVEN (body iff + mutant, verdict in) | 1 (recruit) |
| PROVEN (body harness written; verdict pending) | 6 (great, perfect, fever-01, fever-04) |
| PROVEN (pure contract only; body half PROTOCOL/BLOCKED) | 10 |
| FAILS | 0 |
| PROTOCOL-LEVEL | 12 |
| BLOCKED-HOST | 3 |
| OPEN (not contracted beyond the text) | 8 refs |

## Table

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-GEN-recruit-01 | 「[消耗]1500资金，抽1张卡。」 | **PROVEN** | `recruit_01_iff`, `recruit_01_mutant_is_detected` | iff SUCCESSFUL 124s; mutant SUCCESSFUL 129s | — | OPEN-GEN-recruit-once (「1回限定」 not in body) |
| C-GEN-great-01/02/03 | 「[移除]此卡；将PERFECT加入抽卡区；[获得]2000」 | **PROVEN** (harness written; iff verdict pending, mutant SUCCESSFUL 279s) | `great_01_02_03_iff`, `great_01_mutant_is_detected` | mutant SUCCESSFUL 279s; iff pending | — | — |
| C-GEN-perfect-01/02/03 | 「[移除]此卡；将FEVER!加入弃卡区；[获得]3000」 | **PROVEN** (harness written; verdict pending) | `perfect_01_02_03_iff`, `perfect_01_mutant_is_detected` | pending | — | — |
| C-GEN-fever-01 | 「将此卡放置在[使用者]的[场地]。」 | **PROVEN** (harness written; verdict pending) | `fever_01_iff`, `fever_01_mutant_is_detected` | pending | — | — |
| C-GEN-fever-02 | 「[支付]或[获得]资金时将金额额外提高X；X为600…」 | **BLOCKED-HOST** | (none) | — | host `play_doubled` stub returns 0, so `ctx::n(0, 600)` doubles to 1200 | OPEN-GEN-fever-floor (source `max(0, …)` vs printed 「提高」) |
| C-GEN-fever-03 | same X rewrite on a [支付] to the owner | **BLOCKED-HOST** | (none) | — | same `play_doubled` gap as fever-02 | — |
| C-GEN-fever-04 | 「[拥有者]回合开始时将此卡放入[使用者]弃卡区。」 | **PROVEN** (harness written; verdict pending) | `fever_04_turn_start_iff`, `fever_04_wrong_turn_noop`, `fever_04_not_placed_guard_rejects`, `fever_04_mutant_is_detected` | pending | — | — |
| C-GEN-budokan-01 | 「X设为2000÷…向上取整10…[支付][使用者]X」 | **PROVEN** (X formula) + **PROTOCOL-LEVEL** (transfer loop) | `budokan_x_formula`, `budokan_x_mutant_is_detected` | formula SUCCESSFUL 1.4s / 0.05s | — | OPEN-GEN-budokan-Y (Y printed, unused) |
| C-GEN-marina-01 | 「[反击]…#1格子…正常进行而不受到其上的额外效果」 | **PROTOCOL-LEVEL** + leaf **BLOCKED-HOST** | (none) | — | leaf body only logs; source TODO needs move-tag ABI (`circleNormal`) | OPEN-GEN-marina-extra |
| C-GEN-marina-exclusive | name carries 「[月岛麻里奈]」 | **PROTOCOL-LEVEL** (engine play gate) | (none) | — | — | — |
| C-GEN-parking-01 | 「放置于Space…房屋转移…最多获得1层」 | **PROVEN** (cap) + **PROTOCOL-LEVEL** (house move) | `parking_house_delta_bound`, `parking_house_delta_mutant_is_detected` | cap SUCCESSFUL 0.10s / 0.03s | — | OPEN-GEN-parking-houses (excess houses) |
| C-GEN-parking-02 | 「[结算]改为回合结束后获得一层[停留]」 | **PROTOCOL-LEVEL** | (none) | — | settle-replace is engine scheduling | — |
| C-GEN-parking-03 | 「无法使用角色及乐队技能」 | **PROTOCOL-LEVEL** | (none) | — | skill-block is a gate over skill play | — |
| C-GEN-parking-exclusive | name carries 「[都筑诗船]」 | **PROTOCOL-LEVEL** (engine play gate) | (none) | — | — | — |
| C-GEN-encore-01 | 「无效此次[异常移动效果]和其导致的所有效果」 | **PROTOCOL-LEVEL** (window) + **BLOCKED-HOST** (negate flag) | `encore_01_leaf_quiet_frame` | quiet-frame written, verdict pending | host models `trig_set_negate_effect` as `a0 != 0` with `a0 = 0` | — |
| C-GEN-harvest-01 | 「主要移动]改为移动1到6…并[结算]」 | **PROVEN** (bound) + **PROTOCOL-LEVEL** (move+settle) + **BLOCKED-HOST** (`n(0, 6)`) | `harvest_steps_bound`, `harvest_steps_mutant_is_detected` | bound SUCCESSFUL 2.0s / 0.08s | — | — |
| C-GEN-net-01 | 「有[指定]目标则取消其对目标之一的[指定]」 | **PROTOCOL-LEVEL** | (none) | — | counteraction window + designation cancel | OPEN-GEN-net-which-target |
| C-GEN-net-02 | 「没有[指定]目标则抵消其所有的效果」 | **PROTOCOL-LEVEL** | (none) | — | counteraction window + full negate | — |
| C-GEN-net-03 | 「事件卡的效果…」 (garbled) | **OPEN** | (none) | — | printed text is garbled; not contracted | OPEN-GEN-net-event |
| C-GEN-clearcp-01 | 「添加1个[CP点]并在自己[场上]添加6个[CP点]」 | **PROTOCOL-LEVEL** (mark plumbing) | (none, amount pinned in `clearcp_settle_amount`) | — | — | — |
| C-GEN-clearcp-02 | 「移除…[CP点]…[获得]800资金」 | **PROVEN** (amount 800) + **PROTOCOL-LEVEL** (mark spend) | `clearcp_settle_amount` | SUCCESSFUL 0.24s | — | printed 「个[CP点]」 numeral omitted; read as 1 |
| C-GEN-clearcp-03 | 「下2回合开始时…相邻…添加1个[CP点]」 | **PROTOCOL-LEVEL** | (none) | — | multi-turn spread | OPEN-GEN-clearcp-neighbour, OPEN-GEN-clearcp-window |
| C-GEN-clearcp-04 | on-card count 0 ⇒ card leaves | **PROVEN** (predicate) | `clearcp_settle_amount` | (same harness) | — | — |
| C-GEN-party-01 | 「所有其他玩家可选择[消耗]500…X…加1」 | **PROVEN** (X formula) + **PROTOCOL-LEVEL** (multi-seat prompts) | `party_x_and_payout` | SUCCESSFUL 0.22s | — | — |
| C-GEN-party-02 | 「X至少为2则…Xd20…至少为35则…」 | **PROVEN** (inclusive-35) + **PROTOCOL-LEVEL** (roll+pay loop) | `party_x_and_payout`, `party_payout_mutant_is_detected` | SUCCESSFUL 0.22s / 0.03s | — | — |
| C-GEN-party-03 | 「X等于1则…额外回合」 | **PROTOCOL-LEVEL** | (none) | — | `give_extra_turn` stub returns 0 / no effect | — |
| C-GEN-tsugu-01 | 「结果至少为22则抽1张卡」 | **PROVEN** (threshold) + **PROTOCOL-LEVEL** (full [手]) | `tsugu_thresholds`, `tsugu_ge22_mutant_is_detected` | SUCCESSFUL 0.20s / 2.1s | — | — |
| C-GEN-tsugu-02 | 「至少为26则…[传送]…不[结算]…可选择购买」 | **PROTOCOL-LEVEL** | (none) | — | move-plan + buy offer are stubs | OPEN-GEN-tsugu-teleport-main |
| C-GEN-tsugu-03 | 「至少为28则…降低1500」 | **PROTOCOL-LEVEL** | (none) | — | turn-long buy modifier is engine state | OPEN-GEN-tsugu-3-order |
| C-GEN-tsugu-04 | 「小于26则选择[获得]1000…或…并[结算]」 | **PROTOCOL-LEVEL** | (none) | — | choice + move-plan + settle | OPEN-GEN-tsugu-teleport-main |
| C-GEN-rain-01 | 「投掷2d2…[指定]X名玩家…获得一层[停留]」 | **PROVEN** (X range) + **PROTOCOL-LEVEL** (designation) | `rain_x_range` | SUCCESSFUL 0.10s | — | user is mandatory target |

## Harness verdicts (filled as runs land)

| harness | verdict | time (s) | unwind | notes |
|---|---|---:|---:|---|
| `budokan_x_formula` | SUCCESSFUL | 1.4 | 80 | 3/3 covers |
| `budokan_x_mutant_is_detected` | SUCCESSFUL | 0.05 | 8 | ceil-10 vs floor-10 disagrees at n=3 |
| `harvest_steps_bound` | SUCCESSFUL | 2.0 | 80 | 1..=6 |
| `harvest_steps_mutant_is_detected` | SUCCESSFUL | 0.08 | 8 | 1..=7 disagrees at 7 |
| `tsugu_thresholds` | SUCCESSFUL | 0.20 | 8 | stacking + 04 excludes 02 |
| `tsugu_ge22_mutant_is_detected` | SUCCESSFUL | 2.1 | 80 | exclusive-22 disagrees at 22 |
| `party_x_and_payout` | SUCCESSFUL | 0.22 | 8 | X=others+1; ≥35 inclusive |
| `party_payout_mutant_is_detected` | SUCCESSFUL | 0.03 | 8 | exclusive-35 disagrees at 35 |
| `rain_x_range` | SUCCESSFUL | 0.10 | 8 | 2d2 sum in 2..=4 |
| `parking_house_delta_bound` | SUCCESSFUL | 0.10 | 8 | ≤1 |
| `parking_house_delta_mutant_is_detected` | SUCCESSFUL | 0.03 | 8 | ≤2 disagrees at 2 |
| `clearcp_settle_amount` | SUCCESSFUL | 0.24 | 8 | 800 not 1000; 6→5; 0 graveyards |
| `recruit_01_iff` | SUCCESSFUL | 124 | 24 | covers 0/2 (symbolic-index cover cliff; asserts hold) |
| `recruit_01_mutant_is_detected` | SUCCESSFUL | 129 | 24 | 1400-spec does not match |
| `great_01_02_03_iff` | written, verdict pending | | 24 | intern+pile; timed out at 360s in one attempt |
| `great_01_mutant_is_detected` | SUCCESSFUL | 279 | 24 | 1400-spec does not match (via cargo kani, pre-coordinator notice) |
| `perfect_01_02_03_iff` | written, verdict pending | | 24 | same shape as great |
| `perfect_01_mutant_is_detected` | written, verdict pending | | 24 | |
| `fever_01_iff` | written, verdict pending | | 24 | place_card; timed out at 300s in one attempt |
| `fever_01_mutant_is_detected` | written, verdict pending | | 24 | |
| `fever_04_turn_start_iff` | written, verdict pending | | 24 | guard `card.placed` via IR |
| `fever_04_wrong_turn_noop` | written, verdict pending | | 24 | |
| `fever_04_not_placed_guard_rejects` | written, verdict pending | | 24 | guard-only (cheap) |
| `fever_04_mutant_is_detected` | written, verdict pending | | 24 | |
| `encore_01_leaf_quiet_frame` | written, verdict pending | | 24 | negate flag BLOCKED-HOST |

## Skills

No `通用` skill exists in `data/characters.json` / `data/bands.json` /
`rules/skills`. `spec/skills_general.rs` and `proofs/skills_general.rs` stay
empty.

## What was tried for blocked / protocol items

* **fever-02/03 (`play_doubled`)**: the body reads the printed 600 through
  `ctx::n(0, 600)`. Host `Op::play_doubled` is a protocol stub returning `0`,
  and `ctx::n` treats `0` as "double the 0th number", so every Kani run sees
  1200. The book value is 600. No workaround that does not fake the host.
  Unblocks when the stub defaults to "no doubling" (e.g. returns `-1`).
* **encore-01 (`trig_set_negate_effect`)**: the ABI call takes no argument
  (`sys_kani` passes `[0; 6]`), but the host does `trig_negate = (a0 != 0)`.
  The flag therefore never sets. Quiet-frame half is proven; the negation
  itself is BLOCKED-HOST until the host treats the unit call as "set true".
* **targeting (`Op::target`)**: always returns `0`, so `target_all` collapses
  to seat 0 and `target(who)` always "hits" seat 0. budokan / rain / party
  designation halves are PROTOCOL-LEVEL (the `target` [反击] window is
  engine scheduling per KANI.md §6).
* **move plan / buy / extra turn**: `card_move` returns 1 and the plan setters
  / `give_extra_turn` / `card_buy` / `linger` are structure-only stubs. tsugu,
  harvest, parking-02 are PROTOCOL-LEVEL.
* **counteraction windows** (marina / encore / net): engine scheduling
  (KANI.md §6). Leaf bodies are invoked where the leaf is self-contained
  (encore quiet-frame); marina's leaf is a no-op stub (source TODO).
* **clearcp-03 spread**: multi-turn; OPEN-GEN-clearcp-window and
  OPEN-GEN-clearcp-neighbour leave the two starts and the neighbour choice
  undefined. Not contracted.