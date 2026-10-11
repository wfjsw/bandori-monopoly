# Kani unit `crychic` — results

**Branch:** `kani-crychic`  
**Worktree:** `/home/monopoly/kani-crychic-wt`  
**Scope:** 15 `CRYCHIC:` cards + 5 CRYCHIC character skills + band skill
`skill:CRYCHIC:美好的往日幻影`  
**Spec source:** `docs/rulebook/spec/cards/crychic.md` + live sheets
(`docs/rulebook/cards.json`, `data/characters.json`, `data/bands.json`).
Bodies invoked unmodified via `rules/kani/src/invoke.rs`.

**Harness files:** `rules/kani/src/spec/{crychic,skills_crychic}.rs`,
`rules/kani/src/proofs/{crychic,skills_crychic}.rs`.

## Runner caveat (read first)

`tools/kani/run.sh` classifies a harness `UNMODELED` when the log contains the
string `unmodeled host op`. Every successful run also lists
`host::unmodeled.assertion.1` as **UNREACHABLE** (the catch-all assert is in
the binary but not hit). The **true verdict** is the log's `VERIFICATION:-`
line. Tables below report that true verdict and note the runner tag.

## Host gaps that shape this unit (not card findings)

| gap | symptom | classification tag |
|---|---|---|
| `Op::state_set` stores `a3` (field id) and drops `a4` (value) | `state::set(k, v)` logs `SetState{n: 0}` and writes 0; `set_slot` packing is fine | **BLOCKED-HOST** |
| `Op::spend_fire`, `Op::state_add`, `Op::add_band_crystals`, `Op::gain_fire` unlogged | effect log empty while counters move | specs assert post-state + empty log |
| `Op::set_no_money_loss`, `Op::set_fixed_roll`, plan `set_*`, `at_turn_end` are protocol stubs (return 0) | standing claims / walk modifiers unobservable | **PROTOCOL-LEVEL** |
| `trigger::move_roll` unmodeled (`trig_move_roll`) | roll-reading skill hooks | **BLOCKED-HOST** |
| `give_stun` writes `World::stay` (same counter as stay; `stun_of` reads stay) | stun and stay share one store | documented; body uses `GiveStun` effect to distinguish |

## Summary counts

Harnesses written: **22** (13 card + 9 skill). Run to verdict: **1** so far
(box saturated — other units; coordinator will re-run the rest centrally).

| classification | count | notes |
|---|---:|---|
| PROVEN (leaf iff) | 6 clauses / 11 harnesses | soyoreturn-02, vocalist-01, debut-02, ifonly-01, grab-01(stay), tomoriskill-02, soyoskill-01(waive), band-03(crystal leaf), soyoreturn-01 branch-2 |
| FAILS | 2 | ifonly-02 (LESS), band-01b (LESS) |
| PROTOCOL-LEVEL | 18 | windows, walks, transforms, deferrals |
| BLOCKED-HOST | 7 | `state_set` value drop, `trig_move_roll`, `fire_pot` |
| OPEN | 7 | see OPEN table |
| TIMEOUT/OOM | 0 attempted | rest of harnesses **written, verdict pending** |

## Card clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-CRYCHIC-destiny-01 | 「每当场上任意格子发生一次收款时…移动…（不触发结算）」 | OPEN / PROTOCOL-LEVEL | — | — | window is cross-turn; 「收款」 undefined | OPEN-收款, OPEN-行动序列后一名 |
| C-CRYCHIC-destiny-02 | 「若…（1）效果未产生作用，将此卡返回手牌」 | PROTOCOL-LEVEL | — | — | same window; band-(3) exemption | — |
| C-CRYCHIC-vocalist-01 | 「一次性向其他玩家支付5000以上资金时，免除此次支付」 | PROVEN (leaf) / PROTOCOL-LEVEL (window) | `vocalist_01_waive_iff`, `vocalist_01_mutant_is_detected` | see log | body: `SetPayAmount{0}` exact | OPEN-分摊 (「一次性」 excludes it) |
| C-CRYCHIC-elegant-01 | 「回合外受到抽卡效果时打出…下回合结束时抽一张卡」 | PROTOCOL-LEVEL | — | — | deferred draw is multi-turn scheduling | — |
| C-CRYCHIC-debut-01 | 「本回合内你的资金不会下降…」 | PROTOCOL-LEVEL | `debut_01_play_logs_nothing` (leaf) | see log | play body only arms `set_no_money_loss` stub | auction / immune-payment exemptions out of scope |
| C-CRYCHIC-debut-02 | 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶」 | PROVEN | `debut_02_turn_end_iff`, `debut_02_mutant_is_detected` | see log | `GiveStun{1}` + unlogged `band_crystals+1` | — |
| C-CRYCHIC-karaoke-01 | 「进行至多5次掷骰，并选择其中一个结果…」 | PROTOCOL-LEVEL | — | — | dice loop + walk-as-main-move | — |
| C-CRYCHIC-ifonly-01 | 「移除…奇迹水晶，获得2000+500*X资金」 | PROVEN | `ifonly_01_cashout_iff`, `ifonly_01_mutant_is_detected` | see log | `GainMoney{2000+500X}` + `PlaceCard` field; crystals drained unlogged | — |
| C-CRYCHIC-ifonly-02 | 「若你的手牌大于等于7，此卡立即置入弃牌堆」 | FAILS (LESS) | `ifonly_02_discard_at_seven_iff` | see log | **LESS:** body only `set_dest(Graveyard)` + narration; never unplaces the card to discard (C# `Unplace`) | — |
| C-CRYCHIC-human-01 | 「从1-20间选择并声明X…掷骰小于等于X…添加一个奇迹水晶」 | mixed | — | — | placement + X write-down is `ask_number` + `set_slot`/props; roll hook needs move-roll | — |
| C-CRYCHIC-human-02 | 「回合开始时…两个或以上奇迹水晶…移除…+（20-X）」 | PROTOCOL-LEVEL | — | — | turn-start consume + move-modifier latch | [异常移动效果] set is G-STATUS's clause |
| C-CRYCHIC-grab-01 | 「（1）获得一层[停留]」 | PROVEN (stay) / BLOCKED-HOST (PASSER value) | `grab_01_stay_iff`, `grab_01_mutant_is_detected` | see log | `GiveStay{1}` exact; `state::set(PASSER,-1)` value host-dropped | — |
| C-CRYCHIC-grab-02 | 「当第一位其他玩家经过你…向前移动一格并[触发结算]」 | OPEN / PROTOCOL-LEVEL | — | — | OPEN-触发结算; pass sequence | OPEN-触发结算 |
| C-CRYCHIC-grab-03 | 「传送至…直线距离最近的格子…视为你的主要移动」 | OPEN / PROTOCOL-LEVEL | — | — | grid-nearest teleport as main move | OPEN-触发结算 |
| C-CRYCHIC-haruhikage-01 | 「总资产大于等于20000…抽牌直至手牌数为6」 | OPEN | — | — | draw-to-6 replaceable? | OPEN-春日影抽6 |
| C-CRYCHIC-haruhikage-02 | 「使用一次Crychic角色的技能」 | PROTOCOL-LEVEL | — | — | [反击] borrows a character skill | — |
| C-CRYCHIC-myown-01 | 「主要移动结束时…远离绝对距离最近的玩家一格」 | OPEN / PROTOCOL-LEVEL | — | — | settle-before window + nearest-player rule | OPEN-触发结算 |
| C-CRYCHIC-myown-02 | 「若受到[异常移动效果]影响，此卡不生效」 | OPEN | — | — | G-STATUS [异常移动效果] set | — |
| C-CRYCHIC-soyoreturn-01 | 「[特] 抽到此卡时立刻从抽牌堆打出并执行以下操作之一」 | PROTOCOL-LEVEL (trigger) / PROVEN (branch-2 leaf) | `soyoreturn_01_branch2_iff` | see log | branch 2 = `GainMoney{1000}`; branch 1 is multi-step | — |
| C-CRYCHIC-soyoreturn-02 | 「（2）[手] 获得500资金」 | PROVEN | `soyoreturn_02_hand_iff`, `soyoreturn_02_mutant_is_detected` | see log | exactly `GainMoney{500}` | — |
| C-CRYCHIC-tomoricry-01 | 「重置一次…X…获得两个X差值*120的资金」 | OPEN | — | — | formula ambiguous | OPEN-两个X差值 |
| C-CRYCHIC-tomoricry-02 | 「不会被…（2）效果移除…洗入抽牌堆」 | PROTOCOL-LEVEL | — | — | band-(2) transform exemption | — |
| C-CRYCHIC-tomoricry-03 | 「乐队技能为MyGO!!!!!…使用一次高松灯（MyGO!!!!!）的（2）技能」 | PROTOCOL-LEVEL | — | — | cross-band skill swap | — |
| C-CRYCHIC-mutsumifeel-01 | 「可以选择（2）或（3）效果之一发动」 | PROTOCOL-LEVEL (choice) | — | — | branch bodies are 02/03 | — |
| C-CRYCHIC-mutsumifeel-02 | 「消耗…3个奇迹水晶…执行乐队技能的（2）效果，然后弃一张卡」 | PROTOCOL-LEVEL | — | — | mid-play band-(2) invoke | — |
| C-CRYCHIC-mutsumifeel-03 | 「[移除]…加入一张"表演的本能"…抽2张卡…以CiRCLE为起点」 | PROTOCOL-LEVEL | — | — | shuffle + move-from-CiRCLE | — |
| C-CRYCHIC-sakilead-01 | 「回合开始时若你与其他玩家重合，可打出此卡并记录那些玩家」 | PROTOCOL-LEVEL | — | — | record latch + optional play | — |
| C-CRYCHIC-sakilead-02 | 「下次主要移动结果对那些玩家一起执行…依次触发结算」 | PROTOCOL-LEVEL | — | — | multi-player walk + settle order | OPEN-触发结算 |
| C-CRYCHIC-sakilead-03 | 「触发结算时进行的支付价格减半」 | PROTOCOL-LEVEL | — | — | settle pay factor | stacking with other halves is open item 4 |
| C-CRYCHIC-rikireroll-01 | 「进行一次重骰…至掷骰结果与本回合内…都不同为止」 | PROTOCOL-LEVEL | — | — | re-roll loop + turn-roll history | OPEN-触发结算 |

## Character-skill clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-CRYCHIC-tomoriskill-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 | PROTOCOL-LEVEL (walk) / BLOCKED-HOST (`fire_pot`/`set_bounds`) | — | — | cap (1,1); walk-past is multi-event | — |
| C-CRYCHIC-tomoriskill-02 | 「消耗1火罐清除自身一层[停留]」 | PROVEN | `tomoriskill_02_spend_iff`, `tomoriskill_02_mutant_is_detected` | see log | `spend_fire(1)` + `state::add(STAY,-1)`; both unlogged; dual-store documented | — |
| C-CRYCHIC-tomoriskill-03 | 「移动掷骰小于等于6时…获得一层[停留]并在回合结束时触发结算」 | BLOCKED-HOST | — | — | `trig_move_roll`; deferred settle is PROTOCOL-LEVEL | — |
| C-CRYCHIC-soyoskill-01 | 「取消那次支付…下回合的主要移动变为传送至…那格」 | PROVEN (waive leaf) / BLOCKED-HOST (latch values) / PROTOCOL-LEVEL (deferred teleport) | `soyoskill_01_cancel_iff`, `soyoskill_01_decline_noop`, `soyoskill_01_mutant_is_detected` | see log | `SetPayAmount{0}` exact; two `SetState` latches host-value-dropped | — |
| C-CRYCHIC-rikiskill-01 | 「移动掷骰结果严格小于…上一次…不大于20…下回合可…重骰一次」 | BLOCKED-HOST | — | — | `trig_move_roll`; cross-turn latch is PROTOCOL-LEVEL | — |
| C-CRYCHIC-sakiskill-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）」 | PROTOCOL-LEVEL / BLOCKED-HOST | — | — | cap (1,2); `fire_pot` | — |
| C-CRYCHIC-sakiskill-02 | 「其他人的主要移动结束时…支付300资金并…靠近1格」 | PROTOCOL-LEVEL | — | — | within-5 step; plan stubs | — |
| C-CRYCHIC-sakiskill-03 | 「你的主要移动结束时…消耗1火罐…靠近1格」 | PROTOCOL-LEVEL | — | — | same | — |
| C-CRYCHIC-sakiskill-04 | 「乐队技能为Ave Mujica…处于状态2」 | PROTOCOL-LEVEL | — | — | band-skill swap | — |
| C-CRYCHIC-mutsumiskill-01 | 「每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」 | PROTOCOL-LEVEL / BLOCKED-HOST | — | — | `fire_pot` | — |
| C-CRYCHIC-mutsumiskill-02 | 「消耗1火罐，使本次的移动掷骰结果为那名玩家上一次的…」 | BLOCKED-HOST | — | — | `trig_move_roll` | — |
| C-CRYCHIC-mutsumiskill-03 | 「手牌大于等于3时进入状态2…」 | PROTOCOL-LEVEL | — | — | band-skill swap | — |

## Band-skill clauses

| ID | book quote (short) | class | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-CRYCHIC-band-01 | 「你的手牌数没有上限」 | BLOCKED-HOST (value) | `band_01_hand_limit_host_gap` | see log | body `state::set(HAND_LIMIT, i32::MAX)` is book-correct; host drops the value | — |
| C-CRYCHIC-band-01b | 「手牌数大于等于6时…无法从手中打出任何牌」 | **FAILS (LESS)** | (no leaf body) | unit test `crychic_band_blocks_play_at_six` red | **LESS:** no play-gate body (TODO in `skill-bands/src/crychic.rs`); engine accepts plays at hand 6 | — |
| C-CRYCHIC-band-02 | 「回合结束时，若…抽牌堆与弃牌堆中都没有卡…」 | PROTOCOL-LEVEL | — | — | multi-zone transform + band-skill swap; cash-out leaf is `spec::band_02_cashout_*` | 「角色对应的自选」 counterpart choice |
| C-CRYCHIC-band-03 (crystal) | 「每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶（上限10）」 | PROVEN (leaf with SEEN=4) / BLOCKED-HOST (SEEN write) / body LESS vs 「每当」 | `band_03_crystal_on_four_to_five_iff`, `band_03_no_crossing_noop`, `band_03_mutant_is_detected` | see log | body logic is book-correct when `SEEN==4`; `state::set(SEEN,…)` value host-dropped so live games never see 4 (matches unit-test red). Also SEEN only tracks draws — a non-draw 4→5 is missed (LESS vs 「每当」) | OPEN-四到五 (3→5 jump) |
| C-CRYCHIC-band-03 (cash-out) | 「当此卡移除时，你获得X*500资金」 | PROTOCOL-LEVEL (removal) / leaf spec ready | — | — | `spec::band_02_cashout_*`; nested `invoke_skill` path skips cash-out (source TODO) | — |

## Mutant / non-vacuity

Every PROVEN leaf has a `*_mutant_is_detected` reachable-disagreement
companion (`#[kani::should_panic]` is avoided — COND-SINGLE-SOURCE §10
tooling quirk). Companions assert the body matches the book and `kani::cover!`
that the wrong spec disagrees.

## What was tried for blocked / expensive items

* **state_set value drop** — confirmed in `rules/kani/src/host.rs`
  `Op::state_set` (`w.key_set(a0, key, a3)` with `a3 = field`, ABI packs
  `value` in `a4`). Host file is shared; not edited. Clauses whose only
  observable effect is `state::set` are BLOCKED-HOST.
* **trig_move_roll** — listed in KANI-COVERAGE as unmodeled. Skill hooks that
  read `trigger::move_roll()` are BLOCKED-HOST (tomoriskill-03, rikiskill-01,
  mutsumiskill-02).
* **PROTOCOL-LEVEL** — [反击] windows, multi-turn deferrals, walk/settle
  pipelines, band transform. Cite KANI.md §6. Leaf bodies that are single
  entries (waive, gain, give_stay/stun, cash-out) are still proven.
* **ifonly-02** — proof asserts the book's discard move; the body does not
  make it. Left as a plain FAILING harness (no `#[ignore]`).

## Harness verdict log

True verdict = log `VERIFICATION:-` line. Runner tag may say UNMODELED when
`host::unmodeled` appears as an UNREACHABLE check (fixed on master; this
worktree still has the old grep — read the log).

| unit | harness | true verdict | time (s) | runner tag |
|---|---|---|---:|---|
| crychic | soyoreturn_02_hand_iff | **SUCCESSFUL** | 324 | UNMODELED (false) |
| crychic | soyoreturn_02_mutant_is_detected | written, verdict pending | | |
| crychic | vocalist_01_waive_iff | written, verdict pending | | |
| crychic | vocalist_01_mutant_is_detected | written, verdict pending | | |
| crychic | grab_01_stay_iff | written, verdict pending | | |
| crychic | grab_01_mutant_is_detected | written, verdict pending | | |
| crychic | debut_02_turn_end_iff | written, verdict pending | | |
| crychic | debut_02_mutant_is_detected | written, verdict pending | | |
| crychic | debut_01_play_logs_nothing | written, verdict pending | | |
| crychic | ifonly_01_cashout_iff | written, verdict pending | | |
| crychic | ifonly_01_mutant_is_detected | written, verdict pending | | |
| crychic | ifonly_02_discard_at_seven_iff | written, verdict pending (expected FAILS/LESS) | | |
| crychic | soyoreturn_01_branch2_iff | written, verdict pending | | |
| skills_crychic | tomoriskill_02_spend_iff | written, verdict pending | | |
| skills_crychic | tomoriskill_02_mutant_is_detected | written, verdict pending | | |
| skills_crychic | soyoskill_01_cancel_iff | written, verdict pending | | |
| skills_crychic | soyoskill_01_decline_noop | written, verdict pending | | |
| skills_crychic | soyoskill_01_mutant_is_detected | written, verdict pending | | |
| skills_crychic | band_01_hand_limit_host_gap | written, verdict pending | | |
| skills_crychic | band_03_crystal_on_four_to_five_iff | written, verdict pending | | |
| skills_crychic | band_03_no_crossing_noop | written, verdict pending | | |
| skills_crychic | band_03_mutant_is_detected | written, verdict pending | | |

Sequential runner left running under
`target/kani-results-crychic/` (`--jobs 1`, `--timeout 500`) so further
verdicts land as slots free; coordinator can re-run
`bash tools/kani/run.sh --unit crychic,skills_crychic --jobs 1`.

## FAILS (book vs body) — one-liners

1. **C-CRYCHIC-ifonly-02 LESS** — book 「此卡立即置入弃牌堆」; body
   (`rules/cards/card-crychic/src/forever.rs` `on_drew`) only
   `ctx::set_dest(Dest::Graveyard)` + narration and never unplaces/sends the
   card to the discard (C# comment cites `H.Unplace`). Harness
   `ifonly_02_discard_at_seven_iff` stays red.
2. **C-CRYCHIC-band-01b LESS** — book 「任何时刻拥有手牌数大于等于6时…无法从
   手中打出任何牌」; `skill-bands/src/crychic.rs` has a TODO and no play-gate
   body. Unit test `crychic_band_blocks_play_at_six` stays red (engine accepts
   the play).

Related body gap (not a separate FAILS row): **C-CRYCHIC-band-03** crystal —
logic is book-correct when `SEEN==4`, but SEEN is only updated on draws /
turn-start (not every hand-size movement), so a non-draw 4→5 is missed
(LESS vs 「每当」). Live unit-test red is also caused by the host
`state_set` value drop (SEEN never becomes 4).

## Open questions (from the clause doc; unchanged)

| OPEN | text | clauses |
|---|---|---|
| OPEN-收款 | 「收款」 undefined | destiny-01 |
| OPEN-行动序列后一名 | next-player tile search undefined | destiny-01 |
| OPEN-触发结算 | [触发结算] undefined | grab-02/03, myown-01, sakilead-02, rikireroll-01 |
| OPEN-春日影抽6 | is each draw-to-6 iteration replaceable? | haruhikage-01 |
| OPEN-两个X差值 | 「两个X差值*120」 formula | tomoricry-01 |
| OPEN-四到五 | does a 3→5 jump count as 「从4增加至5」? | band-03 |
| OPEN-分摊 | 分摊 undefined | vocalist-01 (「一次性」 excludes it) |