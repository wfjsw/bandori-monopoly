# Kani unit `mor` — Morfonica cards + Morfonica skills

Branch `kani-mor` at `/home/monopoly/kani-mor-wt`.
Clause doc: `docs/rulebook/spec/cards/mor.md`. Specs: `rules/kani/src/spec/mor.rs`
+ `spec/skills_mor.rs`. Proofs: `rules/kani/src/proofs/mor.rs` + `proofs/skills_mor.rs`.

Method: Kani proofs on the **real, unmodified** card/skill bodies with symbolic
inputs, against a reference spec derived **only** from the 规则书 quotes.
IFF (post == spec(pre) ∧ effect log exactly ∧ frame) + NO-OP + covers + MUTANT.
No `#[ignore]`. PROTOCOL-LEVEL rows are tagged, not faked.

Runner note: `tools/kani/run.sh` greps the log for `unmodeled host op` and
marks **UNMODELED** even when that CBMC check is `UNREACHABLE`. The real
verdict is CBMC's `VERIFICATION:- …` line. Rows below record the CBMC verdict
and note the runner false-positive when it fires.

Host-model gap (shared, not edited here): `tile_owner` / `houses_of` /
`set_owner` index tiles 0-based (`tile_idx`), while `tile_price` / `buy_price` /
`is_agent` / `tile_group` use 1-based `board::tile_slot`. The real game
(`game_rules::wasm_rules`) is 0-based throughout. Clauses that need price /
agent / group on the same tile as owner are **BLOCKED-HOST** on this bug.

---

## Card clauses

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MOR-light-01 | 「当你在"月之森女子学院"格子前后20格之内，可以从手牌中打出此卡」 | PROTOCOL-LEVEL | — | — | — | OPEN-移动起点 |
| C-MOR-light-02 | 「此次移动以"月之森女子学院"为起点（不触发起点地块效果）」 | PROTOCOL-LEVEL | — | — | move-plan scheduling (`plan::set_start` is a protocol stub) | OPEN-移动起点 |
| C-MOR-again-01 | 「[反击]当…前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金」 | PROTOCOL-LEVEL | — | — | [反击] window is engine scheduling (KANI.md §6) | — |
| C-MOR-again-02 | 「[持续]：[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」 | (see proofs) | (pending) | (pending) | — | OPEN-取消范围 |
| C-MOR-courage-01 | 「…所有者向你支付该地块的购买价格+地块已有房子的建造价格总额的一半」 | BLOCKED-HOST | `courage_*` (pending) | (pending) | host `tile_price`/`buy_price` 1-based vs body 0-based | — |
| C-MOR-courage-02 | 「若为地产商地块，获得1000资金」 | BLOCKED-HOST | (pending) | (pending) | host `is_agent` 1-based vs body 0-based | — |
| C-MOR-courage-03 | unowned tile ⇒ nobody pays | (pending) | (pending) | (pending) | — | — |
| C-MOR-summer-01 | 「直到下个自己的回合开始前，你只会被自己发动的效果指定」 | PROTOCOL-LEVEL | — | — | multi-turn designation window | OPEN-指定 |
| C-MOR-summer-02 | 「当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌」 | PROTOCOL-LEVEL | — | — | window end + negation counter is engine-internal | — |
| C-MOR-centrifuge-01 | 「[反击]当你在两个你的回合之间…第二次成为…目标时」 | PROTOCOL-LEVEL | — | — | [反击] window + targeting count | OPEN-目标计数 |
| C-MOR-centrifuge-02 | 「直到下个你的回合开始时，无效化你受到的所有效果」 | PROTOCOL-LEVEL | — | — | multi-turn negate window | OPEN-无效化 |
| C-MOR-rainbow-01 | 「当你向学妹或同级生支付时…此次支付金额减半」 | (see proofs) | `rainbow_01_half_iff`, `rainbow_01_mutant_is_detected` | (pending) | watch: body CeilTo-10 vs clause integer half | grade table ruling 2026-10-06 |
| C-MOR-rainbow-02 | 「当学姐或同级生向你支付的时候…变成1.5倍」 | (see proofs) | `rainbow_02_boost_iff`, `rainbow_02_mutant_is_detected` | (pending) | watch: body CeilTo-10 vs clause integer 1.5× | grade table ruling 2026-10-06 |
| C-MOR-wing-01 | 「传送到移动方向20格后（不触发结算）」 | PROTOCOL-LEVEL | — | — | move-direction + settle skip is scheduling | OPEN-移动方向 |
| C-MOR-wing-02 | 「立刻进行移动掷骰」 | PROTOCOL-LEVEL | — | — | follow-on roll is engine scheduling | — |
| C-MOR-starry-01 | 「支付X次1000…放置X个[奇迹水晶]」 | (pending) | (pending) | (pending) | — | OPEN-X, OPEN-场地中央 |
| C-MOR-starry-02 | 「此卡没有[奇迹水晶]时加入弃牌堆」 | (pending) | (pending) | (pending) | — | — |
| C-MOR-starry-03 | 「可以放弃第一次的结果重骰一次」 | PROTOCOL-LEVEL | — | — | re-roll window is move scheduling | OPEN-重骰次数 |
| C-MOR-starry-04 | 「当其他角色的移动掷骰结果是偶数时可消耗1000…移除一个[奇迹水晶]」 | (pending) | (pending) | (pending) | — | — |
| C-MOR-starry-05 | 「此卡拥有者经过CiRCLE时此卡移除一个[奇迹水晶]」 | (pending) | (pending) | (pending) | — | — |
| C-MOR-starry-06 | 「每个[奇迹水晶]移除时此卡拥有者获得1000资金」 | (pending) | (pending) | (pending) | — | — |
| C-MOR-tritone-01 | 「[反击]…立刻获得此次失去的资金金额，此卡放置在场上」 | PROTOCOL-LEVEL (window) / body (pending) | (pending) | (pending) | — | 「任意时刻」 |
| C-MOR-tritone-02 | 「三回合后…弃置此卡并支付由此卡获得的资金」 | PROTOCOL-LEVEL | — | — | multi-turn decay + payback | OPEN-三全音收款人 |
| C-MOR-noble-01 | 「位于一块地契价值大于等于2200的地块时，可以打出此卡」 | BLOCKED-HOST | (pending) | (pending) | host `tile_price` 1-based vs body 0-based | — |
| C-MOR-noble-02 | 「立刻进行一次触发结算。2.在该地块上放置一个标记」 | partial | `noble_02_mark_ok` spec | (pending) | mark leg pinned; forced settle not defined | OPEN-触发结算, OPEN-指定 |
| C-MOR-nnm-01 | 「弃置手中x枚角色标记，发动以下效果中的一个」 | (pending) | (pending) | (pending) | clause doc records engine multi-activation gap | OPEN-角色标记, OPEN-多分支选择 |
| C-MOR-nnm-02 | 「抽x张卡（可超过上限），回合结束后将手牌弃置到五张」 | (pending) | (pending) | (pending) | — | — |
| C-MOR-nnm-03 | 「获得x次经过CiRCLE时的资金奖励」 | (pending) | (pending) | (pending) | sheet 2026-10-06 G7 reword | — |
| C-MOR-nnm-04 | 「放置x个奇迹水晶，你可以移除一个…视为发动你的（2）技能」 | (pending) | (pending) | (pending) | — | OPEN-借用技能 |
| C-MOR-nnm-05 | 「当奇迹水晶耗尽时，将此卡置入弃牌堆」 | FAILS candidate (LESS) | (pending) | (pending) | clause doc: last-crystal press leaves card on field | — |
| C-MOR-rui-demon-01 | 「向全场玩家支付100…之后全场玩家向你支付300」 | see harness | `rui_demon_01_iff`, `rui_demon_01_mutant_is_detected` | see harness table | — | — |
| C-MOR-rui-demon-02 | 「每当你的技能被触发后，立即将x设置为5」 | see harness | `rui_demon_02_x_set_to_5` | see harness table | — | — |
| C-MOR-toko-swap-01 | 「交换…地契+房屋价格更低的玩家向更高的玩家支付差价…免费赎回」 | (pending) | (pending) | (pending) | group lookup is 1-based — may BLOCK | 「颜色」 = group |
| C-MOR-xiaobai-01 | 「此次支付改为失去同等的资金并令…对象失去此次金额一半的资金」 | see harness | `xiaobai_01_iff`, `xiaobai_01_mutant_is_detected` | see harness table | — | odd-half rounding |
| C-MOR-garden-01 | 「获得100资金。」 | see harness | `garden_01_gain_iff`, `garden_01_mutant_is_detected` | see harness table | — | — |
| C-MOR-garden-02 | 「投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动」 | PROTOCOL-LEVEL | — | — | main-move scheduling | OPEN-行动条 |
| C-MOR-garden-03 | 「可以选择是否触发结算。」 | PROTOCOL-LEVEL | — | — | settle pipeline | OPEN-触发结算 |
| C-MOR-garden-04 | 「直到下个你的回合开始时，你无法被异常移动」 | PROTOCOL-LEVEL | — | — | multi-turn immunity window | OPEN-异常移动 |

## Character-skill clauses

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MOR-mashiro-01 | 「（1）[主动移动]时移动掷骰变为2d20」 | PROTOCOL-LEVEL | — | — | RollPlan plan-field stubs | OPEN-主动移动 |
| C-MOR-mashiro-02 | 「（2）反方向移动」 | PROTOCOL-LEVEL | — | — | move direction is plan scheduling | — |
| C-MOR-touko-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 | (pending) | (pending) | (pending) | at-cap may need `fire_max` honouring | — |
| C-MOR-touko-02 | 「消耗一个火罐并指定你的一个地块…支付你X」 | BLOCKED-HOST | — | — | body uses `rent_of` (unmodeled) | OPEN-地租档 |
| C-MOR-nanami-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始2，上限2）」 | (pending) | (pending) | (pending) | — | — |
| C-MOR-nanami-02 | 「当有玩家在使用角色技能时…得到一个该角色的标记」 | PROTOCOL-LEVEL / documented-only | — | — | needs foreign skill-press window | OPEN-角色标记, OPEN-被动CD |
| C-MOR-nanami-03 | 「使用手中的角色标记，视为使用该角色的技能」 | PROTOCOL-LEVEL / documented-only | — | — | borrowed-skill dispatch | OPEN-借用技能, OPEN-时机合适 |
| C-MOR-tsukushi-01 | 「改为指定场上除你以外的一个角色进行一次该动作」 | (pending) | (pending) | (pending) | — | OPEN-X标记 |
| C-MOR-tsukushi-02 | 「将对方的X标记翻面并代替其进行一次该动作」 | (pending) | (pending) | (pending) | — | OPEN-X标记 |
| C-MOR-tsukushi-03 | 「移除所有X标记，你获得移除标记数量*800…」 | (pending) | (pending) | (pending) | — | OPEN-X标记 |
| C-MOR-rui-crit-01 | 「当复制者使x重置为0时，对方失去该技能（2）」 | OPEN / documented-only | — | — | copy mechanism undefined | OPEN-复制 |
| C-MOR-rui-crit-02 | 「roll1d20，若结果小于等于X…此次获得资金翻倍或无需失去资金」 | (pending) | (pending) | (pending) | — | OPEN-双方影响 |

## Band-skill clauses

| ID | book quote (short) | classification | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MOR-band-01 | 「当你一次性失去2000以上资金时，抽一张卡」 | (pending) | (pending) | (pending) | 「2000以上」 inclusive | 「一次性」 |
| C-MOR-band-02 | 「[CiRCLE奖励]…设资金量为1000，1500，2000的循环」 | (pending) | (pending) | (pending) | cycle is per player | — |

---

## Harness verdict table

Filled from `target/kani-results` after each runner pass.

| harness | verdict | time (s) | notes |
|---|---|---:|---|
| `rui_demon_02_x_set_to_5` | SUCCESSFUL (CBMC 0/20788; runner false-UNMODELED) | 148.5 | X ends at 5 |
| `rui_demon_01_iff` | (pending) | | |
| `rui_demon_01_mutant_is_detected` | (pending) | | |
| `xiaobai_01_iff` | (pending) | | |
| `xiaobai_01_mutant_is_detected` | (pending) | | |
| `rainbow_01_half_iff` | (pending) | | |
| `rainbow_01_mutant_is_detected` | (pending) | | |
| `rainbow_02_boost_iff` | (pending) | | |
| `rainbow_02_mutant_is_detected` | (pending) | | |
| `garden_01_gain_iff` | (pending) | | |
| `garden_01_mutant_is_detected` | (pending) | | |

---

## Summary counts

(filled at hand-off)

| classification | count |
|---|---:|
| PROVEN | |
| FAILS | |
| PROTOCOL-LEVEL | |
| BLOCKED-HOST | |
| OPEN | |
| TIMEOUT/OOM | |

## BLOCKED-HOST ops / fields needed

1. `rent_of` (unmodeled `Op`) — `skill:桐谷透子` (2) `C-MOR-touko-02`.
2. **Tile indexing consistency** — `tile_price` / `buy_price` / `is_agent` /
   `tile_group` must use the same 0-based index as `tile_owner` / `houses_of`
   / `set_owner` (real game is 0-based). Blocks `C-MOR-courage-01/02`,
   `C-MOR-noble-01`, and toko's group matching.
3. `gain_fire` should honour `fire_max` (cap) — at-cap fire legs for
   `C-MOR-touko-01` / `C-MOR-nanami-01`.
4. `trig_set_cancelled` should set the cancelled flag (currently always false
   for the no-arg ABI call) — `C-MOR-band-02` cancel leg.

## OPEN refs

See the clause table. Full list is in `docs/rulebook/spec/cards/mor.md`
§ Open questions (OPEN-移动起点, OPEN-取消范围, OPEN-指定, OPEN-无效化,
OPEN-X, OPEN-场地中央, OPEN-重骰次数, OPEN-三全音收款人, OPEN-触发结算,
OPEN-角色标记, OPEN-多分支选择, OPEN-借用技能, OPEN-地租档, OPEN-被动CD,
OPEN-行动条, OPEN-异常移动, OPEN-X标记, OPEN-复制, OPEN-双方影响,
OPEN-主动移动).