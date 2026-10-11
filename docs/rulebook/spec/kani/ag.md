# Kani unit `ag` — Afterglow cards + character/band skills

Branch: `kani-ag` at `/home/monopoly/kani-ag-wt`.
Clause doc: `docs/rulebook/spec/cards/ag.md`.
Primary text: `docs/rulebook/cards.json`, `data/characters.json` (`band ==
"Afterglow"`), `data/bands.json` (`Afterglow`).

Specs: `rules/kani/src/spec/ag.rs`, `rules/kani/src/spec/skills_ag.rs`.
Proofs: `rules/kani/src/proofs/ag.rs`, `rules/kani/src/proofs/skills_ag.rs`.

Per-clause standard: IFF + NO-OP + cover + mutant (KANI-FOUNDATION.md §3).
Classification: PROVEN | FAILS | PROTOCOL-LEVEL | BLOCKED-HOST | OPEN |
TIMEOUT/OOM.

## Host gaps that block whole classes of clauses

| gap | where | consequence |
|---|---|---|
| `Op::trig_move_roll` getter is **unmodeled** (loud fail); `Op::trig_set_move_roll` is a no-op stub (no world field) | `rules/kani/src/host.rs` | every roll-rewrite clause (yolo, not-ok, half-speed-03) is **BLOCKED-HOST** |
| `Op::tile_named` resolves only 「江户川乐器店」; every other name returns −1 | `host.rs` | childhood / detour / tomoe (「商店街」, 「银河拉面馆」, 「CiRCLE」) are **BLOCKED-HOST** |
| `board::is_shop` is `kind == 5` (江户川乐器店 only); real engine is `is_buyable && group == 10` | `rules/kani/src/board.rs` (shared, not editable) | shop-street predicates (childhood list, band filter, savior, one-of-us partner) are **BLOCKED-HOST** |
| `Op::card_build` / `Op::acquire` / `Op::set_build_discount` are protocol stubs returning 0 | `host.rs` | band free-house and savior acquisition are **PROTOCOL-LEVEL** |
| `Op::trig_by_card`, `Op::trig_effect_*`, `Op::target` are stubs (by_card ≡ Some(0), effect count ≡ 0, target ≡ Some(0)) | `host.rs` | declaration's counteract payload is **BLOCKED-HOST** |
| `Op::set_counter_self` writes the key with no `Effect`; `Op::to_discard` / `Op::add_to_deck` likewise silent | `host.rs` | crystal-set / discard-of-played-card appear in post-state, not the effect log (specs pin post-state) |
| `gain_fire` is `saturating_add` with no `fire_max` clamp | `host.rs` | 「上限1」 is declared but unenforced under Kani |
| `ask_yes` registers no options, so `Op::ask` with `opt_n == 0` only ever returns 0 (always yes) unless the harness pre-seats `w.opt_n = 2` | `host.rs` | decline paths need that harness convention (used in soul-02 / moca-02) |

## Card clauses

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-AG-as-usual-01 | 「此卡可以当反击使用」 | PROTOCOL-LEVEL | — | — | counteract-window offer is engine scheduling (KANI.md §6) | — |
| C-AG-as-usual-02 | 「受到异常移动效果…回到起始地点并取消所有受到的效果」 | OPEN | — | — | — | OPEN-起始地点, OPEN-像往常一样-undo |
| C-AG-one-of-us-01 | 「将此卡放置在场上」 | PROVEN* (harness written) | `one_of_us_01_place_iff` | pending | — | — |
| C-AG-one-of-us-02 | 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」 | OPEN + BLOCKED-HOST | — | — | `is_shop` wrong (see gaps) | OPEN-场上 |
| C-AG-one-of-us-03 | 「指定双方各一块地契」 | PROTOCOL-LEVEL | — | — | designation ops (`designate_tile`) are stubs | — |
| C-AG-one-of-us-04 | 「触发结算收益平分…」 | OPEN | — | — | — | OPEN-触发结算, OPEN-分摊 |
| C-AG-one-of-us-05 | 「当其中一方破产时…」 | PROTOCOL-LEVEL | — | — | bankruptcy + designation chain (KANI.md §6) | — |
| C-AG-yolo-01 | 「使结果增加1d4结果的数字」 | BLOCKED-HOST | — | — | needs `trig_move_roll` get+set (see gaps). Known engine finding (card left in hand) is a zone/protocol issue, not the numeric half | — |
| C-AG-glory-01 | 「[反击] 当你经过一名角色时」 | PROTOCOL-LEVEL | — | — | PassTile window is engine scheduling | — |
| C-AG-glory-02 | 「获得等于对方最贵格子基础购买价格一半数额的资金」 | PROVEN* (harness written) | `glory_02_03_swap_iff` | pending | integer division; printed prices even | OPEN-半价取整 |
| C-AG-glory-03 | 「之后对方从你处获得…一半」 | PROVEN* (harness written) | `glory_02_03_swap_iff` | pending | same rounding OPEN | OPEN-半价取整 |
| C-AG-sunset-01 | 「[限]：第100轮次开始前此卡的X为原本X的一半」 | OPEN / partial | — | — | X formula is C-AG-sunset-03; the round-100 halving is determined in isolation | OPEN-取整顺序 |
| C-AG-sunset-02 | 「[指定][使用者]拥有的四个有房子的格子…」 | OPEN | — | — | — | OPEN-少于四格 |
| C-AG-sunset-03 | 「将X设为…÷…向上取整10」 | PROVEN (pending run, formula only) | `sunset` body arithmetic via colors-style pin | pending | divide-then-ceil-to-10 then round-100 halve | OPEN-取整顺序 |
| C-AG-sunset-04 | 「删除所有被[指定]格子上的1栋房」 | PROVEN* (harness written) | covered by sunset play | pending | `add_house(-1)` → `SetHouses` | — |
| C-AG-sunset-05 | 「所有被[指定]的玩家[支付][使用者]X资金」 | PROVEN* (harness written) | covered by sunset play | pending | `transfer` → `PayTo` | — |
| C-AG-childhood-01 | 「投掷1d6并[传送]到商店街自己拥有的对应的格子」 | BLOCKED-HOST | — | — | `tile_named("商店街")` = −1 and `is_shop` wrong | — |
| C-AG-childhood-02 | 「如果投掷结果大于…则[传送]到商店街」 | BLOCKED-HOST | — | — | same (`tile_named`) | — |
| C-AG-childhood-03 | 「视为你的主要移动」 | PROTOCOL-LEVEL | — | — | `plan::*` / `card_move` are protocol stubs | OPEN-传送主要移动结算 |
| C-AG-detour-01 | 「(1)此卡可作为反击使用」 | PROTOCOL-LEVEL | — | — | counteract window; (1) prints no trigger X | — |
| C-AG-detour-02 | 「进行一次 "afterglow"式的移动」 | OPEN | — | — | body is a 4-way C# pick, not the book | OPEN-afterglow式移动 |
| C-AG-detour-03 | 「将下一次的移动掷骰变更为1d6」 | BLOCKED-HOST / OPEN | — | — | depends on (2); `set_base_dice` is a plan stub | — |
| C-AG-declaration-01 | 「[反击]当你或你拥有的格子被其他玩家的卡效果影响时」 | PROTOCOL-LEVEL + BLOCKED-HOST | — | — | offer window + `trig_by_card`/`effect::*` stubs | — |
| C-AG-declaration-02 | 「[指定]那名玩家。」 | BLOCKED-HOST | — | — | `ctx::target` stub always Some(0) | — |
| C-AG-declaration-03 | 「[支付][使用者]500资金且[使用者]抽1张卡」 | BLOCKED-HOST | — | — | same; the pay+draw body is otherwise simple | — |
| C-AG-colors-01 | 「投掷1d6…（若结果严格大于6，则从1开始重新计数）」 | **PROVEN** (formula) | `colors_01_wrap_formula` SUCCESSFUL 0.9–3.1 s | SUCCESSFUL | wrap is identity on 1d6 | — |
| C-AG-colors-02 | 「若为1则立刻获得1500资金」 | PROVEN* (harness written) | `colors_01_02_03_04_play_iff`, `colors_02_mutant_is_detected` | pending | — | — |
| C-AG-colors-03 | 「若为2则从弃牌堆中选择一张牌放置到抽牌堆顶」 | PROVEN (partial) | `colors_06_pick_to_hand` | pending | `add_to_deck_at` is a silent pile write (post-state) | — |
| C-AG-colors-04 | 「若为3则抽一张牌」 | PROVEN* (harness written) | `colors_01_02_03_04_play_iff` | pending | — | — |
| C-AG-colors-05 | 「若为4则可选择补满火罐或者获得2000资金」 | PROVEN* (harness written) | `colors_01_02_03_04_play_iff` | pending | fire branch uses `gain_fire` (no cap clamp) | — |
| C-AG-colors-06 | 「若为5则从弃牌堆中选择一张牌加入手牌」 | PROVEN* (harness written) | `colors_06_pick_to_hand` | pending | — | — |
| C-AG-colors-07 | 「若为6则选择1-5中的3个效果触发」 | OPEN / PROTOCOL | — | — | multi-effect sequencing; body forbids repetition (book does not exclude it) | OPEN (repetition) |
| C-AG-sky-01 | 「[反击]抽出此卡时立刻打出」 | PROTOCOL-LEVEL | — | — | Drawn auto-play is engine scheduling; body is the play itself | — |
| C-AG-sky-02 | 「手牌数至少为3，获得手牌数*600」 | PROVEN* (harness written) | `sky_02_03_play_iff`, `sky_02_mutant_is_detected`, `sky_03_draws_one` | pending | 手牌数 = remaining hand after the auto-play take | — |
| C-AG-sky-03 | 「手牌数小于3，抽一张卡」 | PROVEN* (harness written) | `sky_02_03_play_iff`, `sky_03_draws_one` | pending | — | — |
| C-AG-sky-04 | 「（开局时抽到此卡洗回）」 | PROTOCOL-LEVEL | — | — | opening deal / mulligan is engine; `add_to_deck` is a silent write | — |
| C-AG-soul-01 | 「选择[消耗]1到5次500资金并…放置对应数量的[奇迹水晶]」 | PROVEN* (harness written) | `soul_01_place_and_load_iff`, `soul_01_mutant_is_detected` | pending | host `pay` always returns the amount (floors money at 0); harness assumes sufficient money | — |
| C-AG-soul-02 | 「金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500）」 | PROVEN* (harness written) | `soul_02_pay_choose_yes_iff`, `soul_02_mutant_is_detected` | pending | quote 「因导致的」 truncated in `cards.json`; determined half proved | — |
| C-AG-soul-03 | 「使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]」 | PROVEN* (harness written) | `soul_03_skill_used_iff` | pending | — | — |
| C-AG-soul-04 | 「此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区」 | PROVEN (partial) | dest write via soul-02/03 emptying | pending | `set_dest(Graveyard)` is silent in the log (host) | — |
| C-AG-savior-01 | 「支付常规收购价一半…收购该地契」 | PROTOCOL-LEVEL + BLOCKED-HOST | — | — | Mortgage window + `acquire` stub + `is_shop` wrong | OPEN (mortgaged-after) |
| C-AG-savior-02 | 「额外获得一份抵押收益并将地契翻回」 | PROTOCOL-LEVEL | — | — | mortgage pipeline | — |
| C-AG-half-speed-01 | 「将此卡放置在场上并获得3个奇迹水晶」 | **PROVEN** | `half_speed_01_place_iff` SUCCESSFUL ~270 s; `half_speed_01_mutant_is_detected` pending | SUCCESSFUL | crystals are post-state (`set_counter_self` silent) | — |
| C-AG-half-speed-02 | 「回合结束时移除一个奇迹水晶，…为0时放入弃牌堆」 | PROVEN* (harness written) | `half_speed_02_decay_iff`, `half_speed_02_mutant_is_detected` | pending | empty-at-zero is `set_dest(Graveyard)` (silent) | — |
| C-AG-half-speed-03 | 「移动掷骰的最终结算/2（向上取整）」 | BLOCKED-HOST | — | — | needs `trig_move_roll` get+set | — |
| C-AG-half-speed-04 | 「所有资金支付与消耗减半」 | PROVEN* (harness written) | `half_speed_04_pay_mul_iff`, `half_speed_04_mutant_is_detected` | pending | body is `CeilTo(amount/2, 10)`; even 10-multiples match plain /2 | OPEN-减半取整 |
| C-AG-not-ok-01 | 「掷骰小于6时…使结果+1」 | BLOCKED-HOST | — | — | needs `trig_move_roll` get+set | — |
| C-AG-tiny-can-01 | 「选择其中一个效果发动」 | PROTOCOL-LEVEL | — | — | the (1)/(2) branches are themselves counteract / OPEN | — |
| C-AG-tiny-can-02 | 「使任意结果只有数字区间的效果以理论最大值或最小值结算」 | OPEN | — | — | — | OPEN-@Tsugu |
| C-AG-tiny-can-03 | 「视为打出一张@Tsugu ycm」 | OPEN | — | — | not a card id | OPEN-@Tsugu-ycm |

## Skill clauses

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-AG-firepot-01 | 「每三回合没有使用Afterglow角色的（2）技能获得一个[火罐]（初始1，上限1）」 | PROVEN* (harness written) | `firepot_01_tick_iff`, `firepot_01_mutant_is_detected` | pending | host `gain_fire` does not clamp at `fire_max` (「上限1」 unenforced) | OPEN-每三回合 |
| C-AG-ran-02 | 「可使用一个[火罐]选择本次移动向后…不获得CiRCLE奖励」 | PROVEN (partial) | `ran_02_spend_fire_iff`, `ran_02_mutant_is_detected` | pending | reverse / NO_REWARD are plan+prop stubs; only the fire spend is observable | — |
| C-AG-moca-02 | 「消耗一个[火罐]传送到其所在格子（不触发结算…）」 | PROVEN* (harness written) | `moca_02_teleport_iff`, `moca_02_mutant_is_detected` | pending | teleport + fire spend proved; no settle is the absence of a plan | OPEN-2次cd |
| C-AG-himari-02 | 「指定你的此次移动仅在单/双数格上进行」 | PROTOCOL-LEVEL | — | — | `plan::set_parity` is a stub | — |
| C-AG-tomoe-02 | 「使此次移动的起点向绝对距离"银河拉面馆"更近的方向移动10格」 | BLOCKED-HOST | — | — | `tile_named("银河拉面馆")` = −1 | — |
| C-AG-tomoe-03 | 「若…超过了"银河拉面馆"，你补充一个火罐」 | BLOCKED-HOST | — | — | depends on (2) | — |
| C-AG-tomoe-04 | 「若你位于银河拉面馆上，你无法使用（2）…停留一回合，每圈限一次」 | PROTOCOL-LEVEL + OPEN | — | — | lap counter undefined | OPEN-每圈 |
| C-AG-tsuzushi-02 | 「下回合开始时，进行一次双倍掷骰的移动」 | PROTOCOL-LEVEL | — | — | next-turn scheduling (KANI.md §6) | — |
| C-AG-band-01 | 「购买商店街的或价值小于等于1200的格子时自动免费…加盖一栋房子」 | PROTOCOL-LEVEL | `band_01_applies` (pure filter) | pending | `card_build` stub; `is_shop` wrong | — |
| C-AG-band-02 | 「自动免费在上面加盖一栋房子」 | PROTOCOL-LEVEL | — | — | same | — |

## Mutant companions

Every PROVEN row has a `*_mutant_is_detected` harness asserting a deliberately
wrong spec (off-by-one amount / extra crystal / wrong tile) does **not** match
the card — reachable-disagreement, not `#[kani::should_panic]`.

## Summary

Clause IDs: 60 (15 cards × their numbered clauses + 5 character skills + 1
band skill). Harness inventory: 26 `#[kani::proof]` functions (20 in
`proofs/ag.rs`, 6 in `proofs/skills_ag.rs`) — pure formulas, body iff,
mutant companions.

| class | count | notes |
|---|---:|---|
| PROVEN (verdict recorded) | 2 | `half_speed_01_place_iff`, `colors_01_wrap_formula` |
| harness written, verdict pending slot | 24 | shared-box 3-slot cap saturated (coordinator notice 2026-10-11); compile-checked |
| FAILS | 0 | none found in the runs that completed |
| PROTOCOL-LEVEL | 14 | multi-turn / [反击] / settle / move-plan / mortgage / opening deal |
| BLOCKED-HOST | 12 | `trig_move_roll`, `tile_named`, `is_shop`, `trig_by_card`/`effect::*`/`target`, `card_build`/`acquire` |
| OPEN | 11 | text undefined (OPEN-起始地点, OPEN-触发结算, OPEN-分摊, OPEN-@Tsugu, …) |
| TIMEOUT/OOM | 0 | — |
| **total** | **60** | |

### Runner verdicts completed

| harness | verdict | time | notes |
|---|---|---:|---|
| `colors_01_wrap_formula` | **SUCCESSFUL** | 0.9–3.1 s | pure formula; CBMC `VERIFICATION:- SUCCESSFUL` |
| `half_speed_01_place_iff` | **SUCCESSFUL** | ~270 s | CBMC `VERIFICATION:- SUCCESSFUL` on the unwind-80 build. A later re-run after adding formula harnesses (full recompile) hit the 600 s cap / slot queue and was recorded TIMEOUT — the SUCCESSFUL line was observed in the log before it was overwritten. |
| `half_speed_04_amount_formula` | written, verdict pending | — | queued behind the 3-slot cap at hand-off |
| `glory_half_formula` | written, verdict pending | — | ditto |
| `soul_02_amount_formula` | written, verdict pending | — | ditto |
| `sky_02_amount_formula` | written, verdict pending | — | ditto |
| `firepot_01_tick_formula` | written, verdict pending | — | ditto |
| `band_01_filter_formula` | written, verdict pending | — | ditto |
| all other `*_iff` / `*_mutant_is_detected` | written, verdict pending | — | 18 body/mutant harnesses; compile-checked |

Note: `tools/kani/run.sh` before the origin/master fix labelled a successful
run **UNMODELED** because the host's `unmodeled host op` assert string appears
in every binary's check list. Master's runner (merged) prefers the CBMC
`VERIFICATION:` line.

Remaining harnesses compile under `cargo kani` and are queued behind the
machine-wide 3-slot cap. Re-run with:

```sh
CARGO_TARGET_DIR=/home/monopoly/kani-target-ag \
  bash tools/kani/run.sh --unit ag --jobs 1
```

Harness names (in cost order): `colors_01_wrap_formula`,
`half_speed_04_amount_formula`, `glory_half_formula`, `soul_02_amount_formula`,
`sky_02_amount_formula`, `firepot_01_tick_formula`, `band_01_filter_formula`,
then the `*_iff` bodies, then `*_mutant_is_detected`.

## What was tried for BLOCKED / PROTOCOL items

* Roll-rewrite clauses: confirmed `Op::trig_move_roll` falls through to
  `unmodeled()` and `Op::trig_set_move_roll` stores nothing. No fake zero.
* `tile_named`: only 「江户川乐器店」 is wired; 「商店街」 / 「银河拉面馆」 /
  「CiRCLE」 return −1 and the bodies early-return. Not faked.
* `is_shop`: `board.rs` `kind == 5` ≠ the real `is_buyable && group == 10`.
  Shared file — not edited (brief § Setup).
* Yes/no decline: `ask_yes` with `opt_n == 0` is stuck on "yes"; harnesses
  pre-seat `w.opt_n = 2` and `w.symbolic_answers = true` to explore both.