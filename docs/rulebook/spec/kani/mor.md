# Kani unit `mor` — Morfonica cards + Morfonica skills

Branch `kani-mor` at `/home/monopoly/kani-mor-wt`.
Clause doc: `docs/rulebook/spec/cards/mor.md`. Specs: `rules/kani/src/spec/mor.rs`
+ `spec/skills_mor.rs`. Proofs: `rules/kani/src/proofs/mor.rs` + `proofs/skills_mor.rs`.

Method: Kani proofs on the **real, unmodified** card/skill bodies with symbolic
inputs, against a reference spec derived **only** from the 规则书 quotes.
IFF (post == spec(pre) ∧ effect log exactly ∧ frame) + NO-OP + covers + MUTANT
(reachable-disagreement companions). No `#[ignore]`. PROTOCOL-LEVEL rows are
tagged, not faked.

**Vacuity note.** `#[kani::unwind(24)]` is vacuous over `invoke::find`'s RULESET
scan (covers + assertions UNREACHABLE while CBMC still says SUCCESSFUL). Every
harness now uses `#[kani::unwind(80)]` and a `kani::cover!(true, "body returned")`
right after `unwrap_done`. A run is only trusted when that cover is SATISFIED.

**Runner note.** `tools/kani/run.sh` greps the log for `unmodeled host op` and
marks **UNMODELED** even when that CBMC check is `UNREACHABLE`. The real
verdict is CBMC's `VERIFICATION:- …` line.

**Host-model gaps** (shared `host.rs` / `board.rs`, not edited here):

1. Tile indexing split: `tile_owner` / `houses_of` / `set_owner` / `set_mortgaged`
   / `mortgaged_of` are 0-based (`tile_idx`); `tile_price` / `buy_price` /
   `is_agent` / `tile_group` use 1-based `board::tile_slot`. The real game is
   0-based. `owned_at` also returns 1-based `i+1`.
2. `buy_price` returns land price only — **houses ignored** (`house_total =
   buy_price - land` is always 0).
3. `rent_of` is an unmodeled `Op` (loud fail).
4. `tok_names` returns 0 (empty list). `placed_cards` returns a count, not a
   name list.
5. `gain_fire` ignores `fire_max` (always adds).
6. `trig_set_cancelled` writes `a0 != 0` for the no-arg call (`a0=0`) → always
   false.
7. `move_dir` / `card_settle_at` / `fixed_roll` / `trig_card_is` are protocol
   stubs returning 0.
8. `World::new().dest` starts at 0 = `Dest::Graveyard` (need a non-zero
   sentinel for dest assertions).

---

## Card clauses

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MOR-light-01 | 「当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡」 | PROTOCOL-LEVEL | — | — | move-plan gate; `tile_named("月之森")` = -1 under Kani | OPEN-移动起点 |
| C-MOR-light-02 | 「此次移动以"月之森女子学院"为起点（不触发起点地块效果）」 | PROTOCOL-LEVEL | — | — | `plan::set_start` scheduling | OPEN-移动起点 |
| C-MOR-again-01 | 「[反击]当…前一名玩家[消耗]或[支付]大于0资金后…并[消耗]等量资金」 | PROTOCOL-LEVEL | — | — | [反击] window (KANI.md §6) | — |
| C-MOR-again-02 | 「[持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」 | PROVEN | `again_02_cancel_iff`, `again_02_mutant_is_detected` | (pending) | SetPayAmount(0) + dest=Graveyard | OPEN-取消范围 |
| C-MOR-courage-01 | 「…所有者向你支付…购买价格+…房子的建造价格总额的一半」 | BLOCKED-HOST | `courage_01_pay_iff_blocked_host`, `courage_01_mutant_is_detected` | triage3: A loop fixed; then pay-vs-agent arm not partitioned — partitioned, re-time pending | `tile_price`/`buy_price` amount cannot be pinned | — |
| C-MOR-courage-02 | 「若为地产商地块，获得1000资金」 | BLOCKED-HOST | `courage_02_agent_iff_blocked_host` | (pending) | `is_agent` off-by-one (tile 5 property reads as agent) | — |
| C-MOR-courage-03 | unowned tile ⇒ nobody pays | PROVEN | `courage_03_unowned_iff`, `courage_03_empty_when_no_agent_misfire` | (pending) | empty-effect limb + money frame | — |
| C-MOR-summer-01 | 「直到下个自己的回合开始前，你只会被自己发动的效果指定」 | PROTOCOL-LEVEL | — | — | multi-turn designation window | OPEN-指定 |
| C-MOR-summer-02 | 「当此卡效果结束…则抽一张牌」 | PROTOCOL-LEVEL | — | — | window end + negation counter | — |
| C-MOR-centrifuge-01 | 「[反击]…第二次成为…目标时」 | PROTOCOL-LEVEL | — | — | [反击] + targeting count | OPEN-目标计数 |
| C-MOR-centrifuge-02 | 「直到下个你的回合开始时，无效化你受到的所有效果」 | PROTOCOL-LEVEL | — | — | multi-turn negate shield | OPEN-无效化 |
| C-MOR-rainbow-01 | 「当你向学妹或同级生支付时…此次支付金额减半」 | **FAILS (D GENUINE)** | `rainbow_01_half_iff`, `rainbow_01_mutant_is_detected` | keep FAILING | book 减半 (integer half); body C# `CeilTo(half, 10)`. Mutant collides on amounts like 18/38 | grade table ruling 2026-10-06 |
| C-MOR-rainbow-02 | 「当学姐或同级生向你支付…变成1.5倍」 | **FAILS (D GENUINE)** | `rainbow_02_boost_iff`, `rainbow_02_mutant_is_detected` | keep FAILING | book 1.5倍; body CeilTo-10 | grade table ruling 2026-10-06 |
| C-MOR-wing-01 | 「传送到移动方向20格后（不触发结算）」 | BLOCKED-HOST | `wing_01_teleport_iff_blocked_host`, `wing_01_dir_stub_gap` | (pending) | `Op::move_dir` stub = 0 | OPEN-移动方向 |
| C-MOR-wing-02 | 「立刻进行移动掷骰」 | PROTOCOL-LEVEL | — | — | `card_move` scheduling stub | — |
| C-MOR-starry-01 | 「支付X次1000…放置X个[奇迹水晶]」 | PROVEN | `starry_01_place_iff`, `starry_01_mutant_is_detected` | (pending) | X symbolic 1..=5 | OPEN-X, OPEN-场地中央 |
| C-MOR-starry-02 | 「此卡没有[奇迹水晶]时加入弃牌堆」 | PROVEN (dest write) | folded into `starry_06_payout_iff` | (pending) | discard commit = `set_dest(Graveyard)`; not an effect | — |
| C-MOR-starry-03 | 「可以放弃第一次的结果重骰一次」 | BLOCKED-HOST | — | — | DiceRollAfter arm needs unmodeled `trig_move_main` | OPEN-重骰次数 |
| C-MOR-starry-04 | 「当其他角色的移动掷骰结果是偶数时…」 | BLOCKED-HOST | — | — | same DiceRollAfter arm (`trig_move_main`, `trig_move_roll`) | — |
| C-MOR-starry-05 | 「经过CiRCLE时…移除一个[奇迹水晶]」 | PROVEN | `starry_05_circle_iff`, `starry_05_noop` | **SUCCESSFUL 306/299 s** (batch-12 B: `trig_tile=0`) | PassTile arm; `trig_tile=0` = CiRCLE (0-based `tile_slot`) | — |
| C-MOR-starry-06 | 「每个[奇迹水晶]移除时…获得1000资金」 | PROVEN | `starry_06_payout_iff`, `starry_06_mutant_is_detected` | (pending) | GainMoney 1000/removed | — |
| C-MOR-tritone-01 | 「[反击]…立刻获得此次失去的资金金额，此卡放置在场上」 | PROVEN (body) | `tritone_01_gain_place_iff` | **SUCCESSFUL 307/306 s** (batch-12 B: +AddCounter) | window is PROTOCOL; body GainMoney+PlaceCard+AddCounter(3)+SetState(owed) | 「任意时刻」 |
| C-MOR-tritone-02 | 「三回合后…弃置此卡并支付由此卡获得的资金」 | PROVEN (tick) + PROTOCOL (schedule) | `tritone_02_tick_iff`, `tritone_02_mutant_is_detected` | **SUCCESSFUL 335/351 s** (batch-12 B: owed=0) | 3-turn schedule is engine; `owed==0` pays nothing | OPEN-三全音收款人 |
| C-MOR-noble-01 | 「位于一块地契价值大于等于2200的地块时」 | BLOCKED-HOST | — | — | `tile_price` 0/1-based split | — |
| C-MOR-noble-02 | 「立刻进行一次触发结算。2.放置一个标记」 | PROVEN (mark) | `noble_02_mark_iff`, `noble_02_not_yours_noop`, `noble_02_mutant_is_detected` | (pending) | settle (1) undefined | OPEN-触发结算, OPEN-指定 |
| C-MOR-nnm-01 | 「弃置手中x枚角色标记，发动以下效果中的一个」 | BLOCKED-HOST + OPEN | — | — | `tok_names` empty; engine multi-activation gap | OPEN-角色标记, OPEN-多分支选择 |
| C-MOR-nnm-02 | 「抽x张卡…回合结束后将手牌弃置到五张」 | BLOCKED-HOST | — (spec ready) | — | hand-play unreachable without `tok_names` | — |
| C-MOR-nnm-03 | 「获得x次经过CiRCLE时的资金奖励」 | PROVEN (formula) | `nnm_03_amount_iff`, `nnm_03_mutant_is_detected` | **SUCCESSFUL 0.3/0.6 s** (batch-12 B) | body raises `PassRewardAffected`; Morfonica listener scales 1000/1500/2000 (CRYSTAL-04). Stale `x*2000` model retired | — |
| C-MOR-nnm-04 | 「放置x个奇迹水晶…视为发动你的（2）技能」 | BLOCKED-HOST | — (spec ready) | — | `placed_cards` is a count, not a name list | OPEN-借用技能 |
| C-MOR-nnm-05 | 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 | PROVEN (dest write) | `nnm_05_empty_discard_iff`, `nnm_05_press_empty_is_noop`, `nnm_05_atend_leaves_field_card` | **SUCCESSFUL 160/142 s** (batch-12 B) | (4) is CounterChanged (`on_crystals_changed` → `set_dest(Graveyard)`); empty-press is a correct no-op | — |
| C-MOR-rui-demon-01 | 「向全场玩家支付100…之后全场玩家向你支付300」 | PROVEN | `rui_demon_01_iff`, `rui_demon_01_mutant_is_detected` | **SUCCESSFUL 629/612 s** (class B post_ok legs swapped) | 100 leg face-value (immunity is pipeline) | — |
| C-MOR-rui-demon-02 | 「每当你的技能被触发后，立即将x设置为5」 | PROVEN candidate | `rui_demon_02_x_set_to_5` | (pending) | X ends at 5 | — |
| C-MOR-toko-swap-01 | 「交换…支付差价…免费赎回」 | PROVEN (no-pair) + BLOCKED-HOST (swap) | `toko_no_pair_noop`, `toko_swap_iff_blocked_host` | (pending) | index split wrecks pair enumeration; `buy_price` ignores houses | 「颜色」 = group |
| C-MOR-xiaobai-01 | 「此次支付改为失去同等的资金并令…对象失去…一半」 | PROVEN | `xiaobai_01_iff`, `xiaobai_01_mutant_is_detected` | see harness table | — | odd-half rounding |
| C-MOR-garden-01 | 「获得100资金。」 | PROVEN candidate | `garden_01_gain_iff`, `garden_01_mutant_is_detected` | (pending) | first effect is the gain | — |
| C-MOR-garden-02 | 「投掷1d6…传送到…前一格并视为主要移动」 | PROTOCOL-LEVEL | — | — | main-move scheduling | OPEN-行动条 |
| C-MOR-garden-03 | 「可以选择是否触发结算。」 | PROTOCOL-LEVEL | — | — | settle pipeline | OPEN-触发结算 |
| C-MOR-garden-04 | 「直到下个你的回合开始时，你无法被异常移动」 | PROTOCOL-LEVEL | — | — | multi-turn immunity window | OPEN-异常移动 |

## Character-skill clauses

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MOR-mashiro-01 | 「[主动移动]时移动掷骰变为2d20」 | PROTOCOL-LEVEL | — | — | DiceRollPlan plan-field stubs | OPEN-主动移动 |
| C-MOR-mashiro-02 | 「反方向移动」 | PROTOCOL-LEVEL | — | — | move direction is plan scheduling | — |
| C-MOR-touko-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 | PROVEN (below-cap) | `touko_01_fire_iff`, `touko_01_noop_not_circle`, `touko_01_mutant_is_detected` | (pending) | at-cap: `gain_fire` ignores `fire_max` → BLOCKED-HOST | — |
| C-MOR-touko-02 | 「消耗一个火罐并指定你的一个地块…支付你X」 | BLOCKED-HOST | — | — | body uses `rent_of` (unmodeled); `touko_02_x` arithmetic ready | OPEN-地租档 |
| C-MOR-nanami-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）」 | PROVEN (below-cap) | `nanami_01_fire_iff` | (pending) | at-cap BLOCKED-HOST (`fire_max`) | — |
| C-MOR-nanami-02 | 「当有玩家在使用角色技能时…得到一个该角色的标记」 | PROTOCOL-LEVEL / documented-only | — | — | foreign skill-press window | OPEN-角色标记, OPEN-被动CD |
| C-MOR-nanami-03 | 「使用手中的角色标记，视为使用该角色的技能」 | PROTOCOL-LEVEL / documented-only | — | — | borrowed-skill dispatch | OPEN-借用技能, OPEN-时机合适 |
| C-MOR-tsukushi-01 | 「改为指定场上除你以外的一个角色进行一次该动作」 | PROVEN (redirect arithmetic) | `tsukushi_01_redirect_iff`, `tsukushi_01_mutant_is_detected` | (pending) | mark + 单轮结算 PROTOCOL | OPEN-X标记 |
| C-MOR-tsukushi-02 | 「将对方的X标记翻面并代替其进行一次该动作」 | Spec pinned; harness deferred | — | — | mark flip OPEN; draw-tag PROTOCOL | OPEN-X标记 |
| C-MOR-tsukushi-03 | 「移除所有X标记…获得移除标记数量*800…」 | PROVEN (n×rate+decay) | `tsukushi_03_sweep_iff`, `tsukushi_03_mutant_is_detected` | (pending) | unseeded RATE maps 0→200 not first-fire 800 (legal play seeds 800) | OPEN-X标记 |
| C-MOR-rui-crit-01 | 「当复制者使x重置为0时，对方失去该技能（2）」 | OPEN / documented-only | — | — | copy mechanism undefined | OPEN-复制 |
| C-MOR-rui-crit-02 | 「roll1d20…获得资金翻倍或无需失去资金」 | PROVEN candidate | `rui_crit_02_roll_iff`, `rui_crit_02_noop`, `rui_crit_02_mutant_is_detected` | (pending) | watch: signedness of PaySplitChoose value | OPEN-双方影响 |

## Band-skill clauses

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MOR-band-01 | 「一次性失去2000以上资金时，抽一张卡」 | PROVEN candidate | `band_01_draw_iff`, `band_01_noop_below_or_gain`, `band_01_mutant_is_detected` | (pending) | 「2000以上」 inclusive | 「一次性」 |
| C-MOR-band-02 | 「[CiRCLE奖励]…1000，1500，2000的循环」 | PROVEN candidate (cycle+gain) | `band_02_cycle_iff`, `band_02_noop_card_option`, `band_02_mutant_is_detected` | (pending) | cancel leg BLOCKED-HOST (`trig_set_cancelled`) | — |

---

## Harness verdict table

A harness counts as **PROVEN** only when CBMC is SUCCESSFUL **and** the
`body returned` cover is SATISFIED (else vacuous).

### Ran (first pass, pre-unwind-80; vacuity checked)

| harness | CBMC verdict | time (s) | body-returned cover | notes |
|---|---|---:|---|---|
| `rui_demon_02_x_set_to_5` | SUCCESSFUL (0/20788) | 148.5 | (no cover in that rev) | X ends at 5; assertion was UNREACHABLE at unwind 24 → re-run pending |
| `rui_demon_01_mutant_is_detected` | SUCCESSFUL (0/20815) | ~250 | 0/1 — **VACUOUS** at unwind 24 | re-run with unwind 80 pending |
| `xiaobai_01_mutant_is_detected` | SUCCESSFUL (0/20903) | ~250 | **1/1 SATISFIED** | non-vacuous; mutant detected |
| `rainbow_01_half_iff` | SUCCESSFUL (0/20909) | 148.8 | 0/2 — **VACUOUS** at unwind 24 | re-run with unwind 80 pending |

### Written, verdict pending

All remaining harnesses (≈50) are written, `#[kani::unwind(80)]`, and
`cargo build -p rules-kani` clean. The shared 16 GB box saturated the 3
machine-wide CBMC slots; the unit was stopped with every harness **compiled
and committed** and a full classification table, per the runbook. Coordinator
can drain the queue centrally.

Harness families awaiting a verdict:
`rui_demon_01_iff` · `xiaobai_01_iff` · `rainbow_01/02_*` · `garden_01_*` ·
`courage_*` · `toko_*` · `starry_*` · `nnm_03_*` · `nnm_05_*` · `tritone_*` ·
`noble_02_*` · `again_02_*` · `wing_01_*` · `band_01/02_*` · `touko_01_*` ·
`nanami_01_*` · `rui_crit_02_*` · `tsukushi_01/03_*`

---

## Summary counts

| classification | count |
|---|---:|
| PROVEN (harness written; verdict pending unless noted) | 22 |
| — of which CBMC-confirmed non-vacuous | 1 (`xiaobai_01_mutant_is_detected`) |
| FAILS (harness written; witness listed) | 2 |
| PROTOCOL-LEVEL | 16 |
| BLOCKED-HOST | 12 |
| OPEN / documented-only | 2 |
| TIMEOUT/OOM | 0 (queue-saturated; verdicts deferred) |

## FAILS (book vs body)

None in this unit's batch-12 rows. Previously listed LESS (nnm-05) and WRONG
(nnm-03) were **class B proof-model bugs**, not book/body gaps:

1. **nnm-05** — 「（4）当奇迹水晶耗尽时，将此卡置入弃牌堆」 is a state clause
   on CounterChanged (`on_crystals_changed` → `set_dest(Graveyard)`), same
   shape as starry-02. The old iff drove the Play press with *already* empty
   crystals (a correct no-op) and claimed the body had no hook. The hook has
   been registered since before the central pass. `arm_set_dest` commits the
   dest write as world state.
2. **nnm-03** — sheet 2026-10-06 G7 wants x × the live CiRCLE money reward.
   CRYSTAL-04 fixed the body to raise `PassRewardAffected` per reward;
   Morfonica's `on_circle` rewrites to 1000/1500/2000 and cancels the default
   (test `nnm_option2_gains_x_times_circle_money_reward`). The old
   `nnm_03_amount_body(x) = x*2000` was a stale pre-CRYSTAL-04 model. Now a
   mutant of that hard-coding is detected.

Genuine D in this unit remains the rainbow-01/02 CeilTo-10 rows (keep FAILING).

## BLOCKED-HOST ops / fields needed

1. **Tile indexing consistency** — `tile_price` / `buy_price` / `is_agent` /
   `tile_group` must use the same 0-based index as `tile_owner` / `houses_of`
   (real game is 0-based). Blocks courage-01/02, noble-01, toko swap.
2. **`buy_price` must include houses** — currently land-only; blocks courage-01
   amount and toko 「地契+房屋价格」.
3. **`rent_of`** — unmodeled `Op`. Blocks `skill:桐谷透子` (2).
4. **`tok_names`** — must list hand character marks. Blocks NNM-01/02/03/04.
5. **`placed_cards`** — must return a name list, not a raw count. Blocks NNM-04.
6. **`trig_move_main` / `trig_move_roll`** — unmodeled. Blocks starry-03/04.
7. **`gain_fire` must honour `fire_max`** — at-cap fire legs (touko-01, nanami-01).
8. **`trig_set_cancelled` must set true** — currently always false. Blocks band-02 cancel.
9. **`move_dir` / `card_settle_at` / `fixed_roll` / `trig_card_is`** — protocol
   stubs returning 0; needed for wing-01, noble-02 settle, starry DiceRollAfter.

## OPEN refs

See the clause tables. Full list in `docs/rulebook/spec/cards/mor.md` § Open
questions: OPEN-移动起点, OPEN-取消范围, OPEN-指定, OPEN-无效化, OPEN-X,
OPEN-场地中央, OPEN-重骰次数, OPEN-三全音收款人, OPEN-触发结算,
OPEN-角色标记, OPEN-多分支选择, OPEN-借用技能, OPEN-地租档, OPEN-被动CD,
OPEN-行动条, OPEN-异常移动, OPEN-X标记, OPEN-复制, OPEN-双方影响,
OPEN-主动移动.

## bands-b concrete-user rollout (branch `kani-bands-b`)

Verdicts below marked **bands-b** use the ppcost pattern: `n_players=2`,
concrete user seats (user=0/1 for seat-symmetric clauses), direct `NAME_kani`
body calls, per-harness `host_subset!`, sparse in-place world. See
TRIAGE-LOG.md `bands-b`. Results accumulate in
`/home/monopoly/kani-bands-b-results/results-all.tsv`.
