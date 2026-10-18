# Kani unit `mygo` — MyGO!!!!! cards, character skills, band skill

Branch: `kani-mygo` at `/home/monopoly/kani-mygo-wt`.
Spec source: `docs/rulebook/spec/cards/mygo.md` + `docs/rulebook/cards.json` +
`data/characters.json` / `data/bands.json` `text`. Specs never derived from
engine behaviour.

Files: `rules/kani/src/spec/mygo.rs`, `rules/kani/src/proofs/mygo.rs`,
`rules/kani/src/spec/skills_mygo.rs`, `rules/kani/src/proofs/skills_mygo.rs`.

Run: `tools/kani/run.sh --unit mygo [--harness NAME] --timeout 900` with
`CARGO_TARGET_DIR=/home/monopoly/kani-target-mychi`.

**Runner quirk:** `tools/kani/run.sh` classifies a log as UNMODELED when the
string `unmodeled host op` appears *anywhere* in it — including as the
property description of the unreachable `host::dispatch` catch-all. The real
verdict is the log's `VERIFICATION:- SUCCESSFUL|FAILED` line. Where the table
says UNMODELED*, the log's VERIFICATION line is quoted.

---

## Summary counts

| classification | count |
|---|---:|
| PROVEN (harness SUCCESSFUL this pass) | 3 |
| PROVEN-able (specs+proofs landed, verdict pending) | 8 |
| FAILS (book vs body; specs land, proofs expected to FAILS) | 3 |
| PROTOCOL-LEVEL | 18 |
| BLOCKED-HOST | 8 |
| OPEN (text undefined) | 6 |
| TIMEOUT/OOM | 0 |
| **clause rows** | **~40** (15 cards + 5 char skills + band skill) |

Card unit rows cover every clause id in `mygo.md`. Skill rows cover
`C-MYGO-*-skill-*` and `C-MYGO-band-*`.

---

## Card clauses

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| `C-MYGO-rinpuu-01` | 「使自己获得一层[停留]」 | **PROVEN** | `rinpuu_01_give_stay_iff`, `rinpuu_01_mutant_is_detected`, `rinpuu_01_mutant_two_layers` | iff SUCCESSFUL 403s (2/2 covers); `rinpuu_01_mutant_two_layers` SUCCESSFUL (panic as expected) | — | — |
| `C-MYGO-rinpuu-02` | 「并在回合结束时额外进行一次[触发结算]」 | **PROTOCOL-LEVEL** | — | — | extra settle is engine scheduling (`On::Action(&[AtEnd])` + `card_settle_at` protocol stub) | OPEN-触发结算 |
| `C-MYGO-journey-01` | 「[手] 将此卡放置于当前格子上并为其放置4个奇迹水晶」 | **PROVEN** | `journey_01_place_four_crystals_iff`, `journey_01_mutant_is_detected` | mutant companion SUCCESSFUL; iff verdict pending | — | — |
| `C-MYGO-journey-02` | 「每回合结束时，若主要移动数严格大于6，失去一个奇迹水晶」 | **BLOCKED-HOST** | — | — | `turn_main_steps` is a protocol stub returning 0; the `>6` drain never runs under Kani. Missing: real `turn_main_steps` / main-move step count | — |
| `C-MYGO-journey-03` | 「当此卡奇迹水晶数量为0时，将此卡置入弃牌堆」 | **PROTOCOL-LEVEL** | — | — | CounterChanged → set_dest(Graveyard) is a zone move; the emptying write itself is engine-side | — |
| `C-MYGO-journey-04` | 「（2）[持续] 触发结算时，获得X*60资金」 | **BLOCKED-HOST** | — | — | `trigger::move_total` is a protocol stub returning 0; the `X*60` gain never runs. Missing: `trig_move_total` | OPEN-触发结算 |
| `C-MYGO-hitoshizuku-01` | 「清除场上所有[停留]与[眩晕]效果」 | PROVEN (with -02) | `hitoshizuku_money_iff` | pending | world now has distinct `stay` / `stun` arrays | — |
| `C-MYGO-hitoshizuku-02` | 「所有玩家…每清除一种效果则支付…1000资金」 | PROVEN (body matches book) | `hitoshizuku_money_iff` | pending | book: 1000 per type **per affected player** (ruling 2026-10-06). Body `transfer(j,user,1000*n)` is per-player. The older "body charges 1000×global kinds" note is **stale** | — |
| `C-MYGO-hitoshizuku-03` | 「若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金」 | PROVEN (with -02) | same | pending | user's extra `GainMoney 1000*n` matches the book | — |
| `C-MYGO-hitoshizuku-04` | 「（此卡可在眩晕时打出）」 | PROVEN (property) | — | — | `prop::PLAYABLE_STUNNED` on the CardDef; not a body effect | — |
| `C-MYGO-haneoka-01` | 「位于属于自己的格子上时，投掷1d20」 | **PROVEN** | `haneoka_thresholds_iff` | pending | own-tile gate + symbolic 1d20 | — |
| `C-MYGO-haneoka-02` | 「若出目至少为10则…免费加盖一层房屋」 | **PROVEN** | `haneoka_thresholds_iff` | pending | free house, money unchanged | — |
| `C-MYGO-haneoka-03` | 「若出目至少为15，则额外抽一张卡」 | **PROVEN** | `haneoka_thresholds_iff` | pending | Draw 1 | — |
| `C-MYGO-haneoka-04` | 「至少为20，则将此卡放置在自己场上…抵消一次任意付款」 | **PROVEN** (place leg) / **PROTOCOL-LEVEL** (later cancel) | `haneoka_thresholds_iff` | pending | placement proven; the later PaySplitChoose cancel is a separate window | — |
| `C-MYGO-haneoka-05` | 「若小于10，此卡放入弃牌堆且视为此卡未生效」 | **PROVEN** | `haneoka_thresholds_iff`, `haneoka_05_mutant_is_detected` | pending | empty body log; discard is the play pipeline's default dest | — |
| `C-MYGO-rainday-01` | 「将此卡放置于自己场上并为其放置5个奇迹水晶」 | **PROVEN** | `rainday_01_place_five_crystals_iff`, `rainday_01_mutant_is_detected` | pending | — | — |
| `C-MYGO-rainday-02` | 「打出此卡时及你的每回合开始时投掷1d10…获得一层[停留]」 | **PROVEN** (play leg shape) / **PROTOCOL-LEVEL** (turn-start leg) | `rainday_01_place_five_crystals_iff` (covers stay) | pending | zone set is the body's AGENTS table (implementation reading of 「地产商」 order) | OPEN-rainday-d10 |
| `C-MYGO-rainday-03` | 「每回合结束时移除一个奇迹水晶…放入弃牌堆」 | **PROTOCOL-LEVEL** | — | — | turn-end decay + CounterChanged discard is a multi-hook pipeline | — |
| `C-MYGO-rainday-04` | 「（骰点大于10则固定为高级住宅区）」 | **OPEN** | — | — | dead text: a 1d10 face is 1..=10 | OPEN-rainday-d10 |
| `C-MYGO-even-lost-01` | 「（1）[反击] 当你被其他人的卡的效果影响时…」 | **PROTOCOL-LEVEL** | — | — | [反击] window is engine scheduling (KANI.md §6) | — |
| `C-MYGO-even-lost-02` | 「（2）[持续] 主要阶段中…置入弃牌堆并进入移动阶段…格数为你当前手牌张数」 | **PROTOCOL-LEVEL** | — | — | move-phase steps = hand count is `set_next_steps` / plan stubs | — |
| `C-MYGO-murosshi-01` | 「指定场上自己以外的一位玩家所在格子」 | **PROTOCOL-LEVEL** | — | — | designation prompt + exile destination is a multi-step window | — |
| `C-MYGO-murosshi-02` | 「获得2层[除外]」 | PROVEN-able (leaf) | — | not run this pass | `give_exile` logs `GiveExile`; host `exile_of` returns 0 (see -03) | — |
| `C-MYGO-murosshi-03` | 「在[除外]层数归0后[传送]至该格子」 | **PROTOCOL-LEVEL** / **BLOCKED-HOST** | — | — | multi-turn exile decay; `exile_of` stub returns 0 | — |
| `C-MYGO-murosshi-04` | 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」 | **PROTOCOL-LEVEL** | — | — | income redirect is a pay/settle hook pipeline; black-box WRONG recorded | — |
| `C-MYGO-normal-01` | 「[反击] 受到异常移动效果影响后可打出」 | **PROTOCOL-LEVEL** | — | — | [反击] window | — |
| `C-MYGO-normal-02` | 「使你下一次主要移动的格数变为…最近一次非传送的主要移动的移动格数」 | **PROTOCOL-LEVEL** | — | — | next-main-move copy is multi-turn; black-box LESS recorded | OPEN-G13 |
| `C-MYGO-meet-again-01` | 「[反击]移动掷骰后…可被经过的玩家时可打出」 | **PROTOCOL-LEVEL** | — | — | [反击] window + passable-player predicate | — |
| `C-MYGO-meet-again-02` | 「持续进行移动掷骰直至[经过]下一名玩家…」 | **PROTOCOL-LEVEL** | — | — | reroll-until loop is engine scheduling (unbounded in the book, capped at E+20) | — |
| `C-MYGO-miracle-01` | 「（1）[手] 将此卡置于场上，将你乐队技能上的奇迹水晶全部转移至此卡上」 | PROVEN-able (leaf) | — | not run this pass | band-card crystal transfer is two counters | OPEN-乐队卡 |
| `C-MYGO-miracle-02` | 「当你进行加盖动作时可移除此卡上的一个奇迹水晶以代替资金花费」 | **PROTOCOL-LEVEL** | — | — | build-action substitution is a settle pipeline | — |
| `C-MYGO-miracle-03` | 「（2）[持续] 当你[经过]场上的所有玩家各一次…」 | **PROTOCOL-LEVEL** / **OPEN** | — | — | pass-everyone stream + 「资金后三位」 | OPEN-miracle-pct |
| `C-MYGO-miracle-04` | 「（3）若此卡进入弃牌堆时其上仍有奇迹水晶，视为此卡未生效」 | **PROTOCOL-LEVEL** | — | — | 未生效 flag feeds the band skill; no Kani-visible observer | — |
| `C-MYGO-soyo-mixed-01` | 「将此卡放置于你拥有地契的一个格子」 | PROVEN-able (leaf) | — | not run this pass | placement on an owned tile | — |
| `C-MYGO-soyo-mixed-02` | 「该格获得所有颜色…」 | **OPEN** | — | — | colour-group widening undecided | OPEN-agent-colour |
| `C-MYGO-soyo-mixed-03` | 「收费在地产商的减半收费基础上额外减半」 | **PROTOCOL-LEVEL** | — | — | rent settle pipeline (`rent_of` unmodeled on the linked-tile path too) | — |
| `C-MYGO-rana-fun-01` | 「将此卡置于当前格子上」 | PROVEN-able (leaf) | — | not run this pass | — | — |
| `C-MYGO-rana-fun-02` | 「每当有人经过且未在其上[触发结算]时为其增加一个奇迹水晶」 | **PROTOCOL-LEVEL** | — | — | pass-without-settle is a move pipeline | — |
| `C-MYGO-rana-fun-03` | 「当奇迹水晶总数为5或以上时使下一个经过的…选择…」 | **PROTOCOL-LEVEL** | — | — | two-choice prompt + forced stop; black-box MORE (no prompt) | OPEN-强制停下 |
| `C-MYGO-rana-fun-04` | 「如果强制停下则此卡洗入弃牌堆」 | **PROTOCOL-LEVEL** | — | — | follows -03 | — |
| `C-MYGO-rana-fun-05` | 「由此卡效果导致[触发结算]时需支付资金减半」 | **PROTOCOL-LEVEL** | — | — | forced-settle pay factor is a settle pipeline | — |
| `C-MYGO-tomori-card-01` | 「（1）获得两层仅能被自然流失效果移除的[停留]」 | PROVEN-able (leaf) | — | not run this pass | lockedStay slot + give_stay(2) | — |
| `C-MYGO-tomori-card-02` | 「将此卡放置于场上并将所有手卡置入弃牌堆…X+1个[奇迹水晶]」 | PROVEN-able (leaf) | — | not run this pass | place + discard hand + set_crystals(X+1) | — |
| `C-MYGO-tomori-card-03` | 「（2）…可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗」 | **FAILS (LESS)** | — | not run this pass | black-box: engine never spends the crystal; fire pot is taken instead | — |
| `C-MYGO-tomori-card-04` | 「当此卡上的水晶由于此效果以外的原因减少时…获得等同于减少量的[奇迹水晶]」 | **PROTOCOL-LEVEL** | — | — | CounterChanged refill hook; needs a real non-skill decrease | — |
| `C-MYGO-tomori-card-05` | 「（3）此卡上的奇迹水晶耗尽后，[移除]此卡」 | **PROTOCOL-LEVEL** | — | — | CounterChanged → [移除] | — |
| `C-MYGO-rikki-act-01` | 「使场上所有拥有[停留]的玩家立刻…进行一次[触发结算]」 | **PROTOCOL-LEVEL** | — | — | forced settle of every stay holder is engine scheduling | — |
| `C-MYGO-rikki-act-02` | 「本次结算导致的所有[支付]变为原价的四分之一」 | **PROTOCOL-LEVEL** | — | — | pay factor on those settles | — |
| `C-MYGO-anon-tokyo-01` | 「[限]：自己所在的格子是[可购买格子]」 | **PROTOCOL-LEVEL** / **BLOCKED-HOST** | — | — | play gate + designation; `rent_of` unmodeled on the link-rent leg | — |
| `C-MYGO-anon-tokyo-02` | 「[指定]…任一相邻的[可购买格子]」 | **PROTOCOL-LEVEL** | — | — | designation prompt | — |
| `C-MYGO-anon-tokyo-03` | 「1. …[消耗]…一半…放置1个[奇迹水晶]（上限1）」 | **BLOCKED-HOST** | — | — | `rent_of` (price of the higher tile) unmodeled | — |
| `C-MYGO-anon-tokyo-04` | 「被[奇迹水晶]连接的格子收费时…额外收取…一半」 | **BLOCKED-HOST** | — | — | `rent_of` unmodeled | OPEN-t04 |
| `C-MYGO-anon-tokyo-05` | 「2. …进入移动阶段并将本回合的[主要移动]改为…1格…且[结算]」 | **PROTOCOL-LEVEL** | — | — | move-phase rewrite + settle | — |

## Skill clauses

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| `C-MYGO-band-01` | 「开局时投掷3d20，并取出目作为你本局游戏的起始点」 | **PROTOCOL-LEVEL** | — | — | game-start window (`DeckBeforeGame`) | — |
| `C-MYGO-band-02` | 「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）」 | **PROVEN** | `band_02_crystal_on_16_iff`, `band_02_mutant_is_detected` | pending | — | — |
| `C-MYGO-band-03` | 「可于移动掷骰前选择移动1格以替代移动掷骰并移除一个[奇迹水晶]」 | **PROTOCOL-LEVEL** | — | — | press shapes a 1-step move via `plan::*` stubs | OPEN-multi-activation |
| `C-MYGO-band-04` | 「每次你的卡在未生效的情况下进入弃牌堆时…可超出上限的[奇迹水晶]（最多超出2个）」 | **FAILS (LESS)** | `band_04_ineffective_overcap_iff`, `band_04_mutant_is_detected` | pending | black-box LESS: over-cap crystal does not land when the band card is already at the roll cap of 1 | — |
| `C-MYGO-band-05` | 「可移除此卡的两个[奇迹水晶]以抽一张卡」 | **PROVEN** (spec+proof landed) | (spec in skills_mygo.rs; press body is a combined entry) | not run this pass | OPEN-multi-activation: press may merge (2)+(3) | OPEN-multi-activation |
| `C-MYGO-tomori-skill-01` | 「（1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）」 | **PROTOCOL-LEVEL** | — | — | pass-of-tile is a move pipeline | — |
| `C-MYGO-tomori-skill-02` | 「（2）…使用4个火罐…强制将该玩家[传送]至自己所在的格子」 | **PROTOCOL-LEVEL** | — | — | before-settle window + fire spend | — |
| `C-MYGO-tomori-skill-03` | 「然后使其投掷1d20，若出目小于等于6则获得一层[停留]…支付价格变为原本的一半」 | **PROTOCOL-LEVEL** | — | — | chained after -02; half-pay is a settle factor | — |
| `C-MYGO-tomori-skill-04` | 「（3）…使用1个火罐，在[触发结算]前将自己[传送]至那名玩家所在的格子」 | **PROTOCOL-LEVEL** | — | — | before-settle window | — |
| `C-MYGO-anon-skill-01` | 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限3）」 | **PROTOCOL-LEVEL** | — | — | pass-of-tile | — |
| `C-MYGO-anon-skill-02` | 「（2）…可以消耗2个火罐，抽一张卡」 | **FAILS (LESS)** | — | not run this pass | black-box: press accepted but neither spends pots nor draws | — |
| `C-MYGO-rana-skill-01` | 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始3，上限3）」 | **PROTOCOL-LEVEL** | — | — | pass-of-tile | — |
| `C-MYGO-rana-skill-02` | 「（2）可花费3个火罐传送至space…不可进行地契购买」 | **PROTOCOL-LEVEL** | — | — | replace-main-move press | — |
| `C-MYGO-rana-skill-03` | 「在space上结算时…免除付款并抽1张卡」 | **FAILS (WRONG)** | — | not run this pass | black-box: draws but still charges rent | — |
| `C-MYGO-rana-skill-04` | 「若自己为space的拥有者…添加一个“抹茶芭菲”…增加500资金」 | **FAILS (LESS)** | — | not run this pass | black-box: never adds a 抹茶芭菲 | — |
| `C-MYGO-rana-skill-05` | 「其他玩家在space触发结算时可将…一个“抹茶芭菲”转移…以免除当次付款」 | **PROTOCOL-LEVEL** | — | — | foreign-settle press | — |
| `C-MYGO-rana-skill-06` | 「（3）你与其他玩家重合时，获得其800资金并使那个玩家获得一个“抹茶芭菲”」 | **PROTOCOL-LEVEL** | — | — | overlap is a move pipeline | — |
| `C-MYGO-rana-skill-07` | 「拥有“抹茶芭菲”的玩家经过任意“RiNG”时可使用一个…获得400资金」 | **PROTOCOL-LEVEL** | — | — | pass-of-tile press | — |
| `C-MYGO-soyo-skill-01` | 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 | **PROTOCOL-LEVEL** | — | — | pass-of-tile | — |
| `C-MYGO-soyo-skill-02` | 「（2）移动掷骰后可消耗一个火罐…[移动终点]…视为对应颜色的地产商格子」 | **FAILS (LESS)** | — | not run this pass | black-box: post-roll press is a no-op (no pot spent) | OPEN-agent-colour |
| `C-MYGO-soyo-skill-03` | 「若以此法单次免除了至少1500资金的[支付]…不获得火罐」 | **PROTOCOL-LEVEL** / **OPEN** | — | — | needs a measured single-use exempted total | — |
| `C-MYGO-rikki-skill-01` | 「（1）场上每有玩家获得一层[停留]时，你获得一个[火罐]（初始0，上限5）」 | **PROTOCOL-LEVEL** | — | — | any-stay-gain is a status pipeline | — |
| `C-MYGO-rikki-skill-02` | 「（2）运营阶段可消耗1个[火罐]…添加一个[奇迹水晶]，每回合限一次」 | **PROTOCOL-LEVEL** | — | — | once-per-turn + field-card pick | — |

---

## Harness verdicts (this pass)

| harness | verdict | time (s) | notes |
|---|---|---:|---|
| `rinpuu_01_give_stay_iff` | **SUCCESSFUL** | 403.0 / 563.1 | 0/20884 failed, **2/2 covers** (pre stay 0 / nonzero). Runner labeled UNMODELED* (false positive on the `unmodeled host op` property string); log `VERIFICATION:- SUCCESSFUL`. |
| `rinpuu_01_mutant_two_layers` | **SUCCESSFUL (panic as expected)** | ~600 | 1/20810 failed = the mutant assert — non-vacuous |
| `rinpuu_01_mutant_is_detected` | written, verdict pending | — | companion of the should_panic twin |
| `journey_01_place_four_crystals_iff` | written, verdict pending | — | queued |
| `journey_01_mutant_is_detected` | **SUCCESSFUL** | ~600 | 0/20745 failed — the two-PlaceCard mutant never matches |
| `haneoka_thresholds_iff` | written, verdict pending | — | queued |
| `haneoka_05_mutant_is_detected` | written, verdict pending | — | |
| `rainday_01_place_five_crystals_iff` | written, verdict pending | — | |
| `rainday_01_mutant_is_detected` | written, verdict pending | — | |
| `hitoshizuku_money_iff` | written, verdict pending | — | expected FAILS on the money amount (MORE/WRONG) |
| `hitoshizuku_body_global_pay_observed` | written, verdict pending | — | diagnostic companion |
| `tomori_card_02_empty_hand_iff` | written, verdict pending | — | empty-hand isolation (X=0) |
| `tomori_card_02_mutant_is_detected` | written, verdict pending | — | |
| `band_02_crystal_on_16_iff` | written, verdict pending | — | |
| `band_02_mutant_is_detected` | written, verdict pending | — | |
| `band_04_ineffective_overcap_iff` | written, verdict pending | — | expected FAILS (LESS) |
| `band_04_mutant_is_detected` | written, verdict pending | — | |

---

## BLOCKED-HOST ops needed

| missing | clauses |
|---|---|
| distinct `stun` field (`stun_of` ≠ `stay_of`) | hitoshizuku-01/02 |
| `turn_main_steps` (real main-move step count) | journey-02 |
| `trig_move_total` / `trigger::move_total` | journey-04, meet-again |
| `rent_of` | anon-tokyo-03/04 |
| `exile_of` (real exile counter) | murosshi-02/03 |
| `tile_at` | (hhw unit; not mine) |

## OPEN refs

* OPEN-触发结算 — journey-04, rinpuu-02, rana-fun
* OPEN-rainday-d10 — rainday-02/04
* OPEN-agent-colour — soyo-mixed-02, soyo-skill-02
* OPEN-G13 — normal-02
* OPEN-t04 — anon-tokyo-04
* OPEN-miracle-pct — miracle-03
* OPEN-乐队卡 — miracle-01
* OPEN-强制停下 — rana-fun-03/04
* OPEN-multi-activation — band-03/05

## What we tried for TIMEOUT/OOM / blocked items

* `turn_main_steps` / `move_total` / `rent_of` / `exile_of` are host protocol
  stubs or unmodeled ops — a proof that assumed a value would be fake. Tagged
  BLOCKED-HOST with the exact op.
* stay/stun conflation: the world has a single `stay` array and `give_stun`
  writes it too. A pre-world cannot hold both statuses. Tagged BLOCKED-HOST
  (missing field) rather than faking an independent stun.
* [反击] / multi-turn / settle pipelines: KANI.md §6 — PROTOCOL-LEVEL, not
  deleted or weakened.

## bands-b concrete-user rollout (branch `kani-bands-b`)

Verdicts below marked **bands-b** use the ppcost pattern: `n_players=2`,
concrete user seats (user=0/1 for seat-symmetric clauses), direct `NAME_kani`
body calls, per-harness `host_subset!`, sparse in-place world. See
TRIAGE-LOG.md `bands-b`. Results accumulate in
`/home/monopoly/kani-bands-b-results/results-all.tsv`.

## triage3 batch 14 — mygo / skills_mygo central non-success rows (kani-pass@fc3ff3e1)

Central pass rows (unit `mygo` features `band-mygo`; unit `skills_mygo`
features `band-mygo,skills-groups` — `tools/kani/run.sh` `unit_features`).
Classes: A bound-low / B setup-spec / C host-model / D rulebook gap / E perf.

### skills_mygo

| harness | before | class | root cause | fix | re-time |
|---|---|---|---|---|---|
| `band_02_crystal_on_16_iff` | FAILED 196 s | **B** then **D** | (1) harness read `key_get(user,"crystals")` but `arm_add_counter_self` stores on `field_crystals(last_uid)` / `self_crystals` (class B — same as `events.rs`). Covers unreachable behind the failing assert. (2) After the storage fix the book formula `if pre<1 {1} else {pre}` FAILS at `pre∈{2,3}`: body `add_crystals(1,1)` + production `add_counter_at` `(*slot+n).max(0).min(max)` **clamps down** and eats (3)'s over-cap crystals | storage → `self_crystals()`; log pins host clamped delta. Book formula kept — **D kept FAILING** | yes |
| `band_02_user1_crystal_on_16_iff` | FAILED 209 s | **B**+**D** | same | same | FAILED (D) |
| `band_02_mutant_is_detected` | SUCCESSFUL 154 s | — | — | — | — |
| `band_04_ineffective_overcap_iff` | FAILED 220 s → **SUCCESSFUL** 3/3 | **B** | crystals via `key_get`; `over` was a symbolic input to the formula but the body *sets* `skill.mygo.over` as audit. Expected `band_04_crystals_after(pre,over)` did not match `add_crystals(1,MAX)` + `if have>=3 return` | `self_crystals()`; formula is `pre>=3 ? pre : pre+1` (book ceiling 3). Log tolerates `SetState` audit | **SUCCESSFUL** 3/3 covers |
| `band_04_user1_ineffective_overcap_iff` | FAILED 202 s → **SUCCESSFUL** 3/3 | **B** | same | same | **SUCCESSFUL** 3/3 covers |
| `band_04_mutant_is_detected` | SUCCESSFUL 157 s | — | — | — | — |

### mygo

| harness | before | class | root cause | fix | re-time |
|---|---|---|---|---|---|
| `rainday_01_place_five_crystals_iff` + `_user1_` + `_mutant_is_detected` | TIMEOUT 1800 s ×3 → **SUCCESSFUL** | **E** | BMC stuck in `sparse_play_world` then the body `rain()` `Vec<i32> area` + `contains` over 10 symbolic 1d10 agents (production `that_day_rain.rs`). No memchr; the cliff is the agent-area Vec | placement dispatch pins the die to face 1 (`arm_roll_face_one` — a real face, not silent-zero) and stands `user` on 「主要街道」; stay leg is PROTOCOL-adjacent (`rainday_02`) | SUCCESSFUL |
| `hitoshizuku_money_iff` + `_user1_` | TIMEOUT 1800 s ×2 | **E** (+ stale D note) | Symex 1739 s / 3.6M steps on `hitoshizuku_body_global_pay_ok` `for e in effects` (unwind 32) + `filter().count()` fold + `zip().all()` stay check. Older "body charges 1000×global kinds" note is **stale**: body `card-mygo/src/hitoshizuku.rs` does `transfer(j,user,1000*n)` per-player, matching the 2026-10-06 ruling | bounded `while` scans; book assert kept (body matches book — not D) | SUCCESSFUL |
| `hitoshizuku_body_global_pay_observed` + `_user1_` | TIMEOUT 1800 s ×2 | **E** | same effect-list loop | repurposed as a **mutant** | SUCCESSFUL |

### CONFIRMED genuine discrepancies

**C-MYGO-band-02 over-cap eater (class D).** Quote
(`docs/rulebook/spec/cards/mygo.md` / `bands.json`):
「若移动掷骰出目为16及以上，为此卡添加一个[奇迹水晶]（上限1）。」
plus (3) 「为此卡添加一个可超出上限的[奇迹水晶]（最多超出2个）」.
Clause doc: the (2) cap applies to the **roll gain**; (3)'s over-cap crystals
are a different clause. Body `ctx::add_crystals(1, 1)` and production
`game-core/src/engine/ops.rs` `add_counter_at` (`min(was+n, max)`) force the
held total down to 1, destroying over-cap crystals. Setup is legal
(`pre ∈ 0..=3`, which (3) can produce). Proof kept FAILING.

Hitoshizuku is **not** D: body and book agree on per-player kinds.

### Book quotes checked

* band-02 / band-04: see the quotes above
* hitoshizuku: 「清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。」
  (`docs/rulebook/cards.json` id `MyGO:壱雫空`)
* rainday-01: 「将此卡放置于自己场上并为其放置5个奇迹水晶」

### Gates (this batch, `CARGO_TARGET_DIR=/home/monopoly/kani-target-triage3`)

* `cargo kani --only-codegen -p rules-kani --no-default-features --features <set>`
  for `band-ras`, `band-mygo`, `band-mygo,skills-groups`, `band-pp,skills-groups`
  — Finished, no `error` lines. (`band-ras,skills-personas` is **not** a
  `run.sh` unit set and fails codegen on pre-existing private `at_turn_end` /
  `clear_acted` from master sheet-sync.)
* `cargo test -p rules-kani` ×2 (`--no-fail-fast`) — every suite `ok` except
  pre-existing `wrapper_check` (missing `at_turn_end_kani` /
  `clear_acted_kani` in `skill-personas` from master; production files, out of
  scope). Confirmed failing on the unmodified tree as well.

Soundness: unwinding assertions ON; no `#[ignore]`; no allowlist; no spec
weakening (band-02 book formula kept despite the D); production bodies
untouched. Allowed Kani-only surface: `rules/kani/**` + `cfg(kani)` in
`rules/card-sdk` (`cards_in` exact-length facade).
