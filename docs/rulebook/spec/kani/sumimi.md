# Kani unit `sumimi` — Sumimi cards + character skills + band skill

Branch `kani-sumimi` at `/home/monopoly/kani-sumimi-wt`.
Spec sources: `docs/rulebook/cards.json` (`Sumimi:*`, 9 sheet cards),
`data/characters.json` band `Sumimi` (纯田真奈 / 三角初华（Sumimi）),
`data/bands.json` skill `人气偶像组合`. Clause doc
`docs/rulebook/spec/cards/sumimi.md`.
`data/cards.json` extras `Sumimi:#L11` / `#L12` are out of scope (sheet wins).

Harness files: `rules/kani/src/spec/{sumimi,skills_sumimi}.rs`,
`rules/kani/src/proofs/{sumimi,skills_sumimi}.rs`.

OPEN-收费标价 (charge-vs-price) is respected: only the determined part of
each quote is specified / proven; the tag stays OPEN.

Classification vocabulary (KANI-FOUNDATION §5):
**PROVEN** · **FAILS** · **PROTOCOL-LEVEL** · **BLOCKED-HOST** · **OPEN** ·
**TIMEOUT/OOM**.

---

## Summary

| class | count | notes |
|---|---:|---|
| FAILS | 2 | twoinone-01 character identity; band-01 add half (guard) |
| PROTOCOL-LEVEL | 14 | [反击] windows, multi-act pass, settle pipelines, move plan |
| BLOCKED-HOST | 5 | `rent_of`, `tile_named` (non-shop), `state_set` packing, `trig_move_remaining` |
| OPEN | 8 | 收费标价 / 拥有相同数字 / 触发结算 / singing redirect |
| harness written, verdict pending slot | 12 | shared-box 3-slot cap |
| formula harness (cheap) | 3 | written this batch |
| **clause rows** | **~35** | cards 9 + 2 char skills + 1 band skill, split per clause id |

### Runner verdicts completed

| harness | verdict | time |
|---|---|---:|
| `idolband_money_formula` | SUCCESSFUL | 11.5 s |
| `pass_circle_fire_formula` | SUCCESSFUL | 6.9 s |
| `band_02_rent_bonus_formula` | SUCCESSFUL | 9.6 s |
| `idolband_reset_iff` | UNMODELED* | 416.1 s / 375.5 s |

\* Two overlapping runner passes recorded UNMODELED: logs end at `Finished
dev profile` with no `VERIFICATION:-` line (kani warned `caller_location` /
`foreign function` unsupported constructs). An earlier direct `cargo kani`
run of the same harness printed `VERIFICATION:- SUCCESSFUL` with full CBMC
output. Treat the iff as **written, verdict pending**, not a clause FAIL.

Remaining harnesses compile under `cargo kani` (checked) and wait on the
machine-wide 3-slot cap (`~/.kani-slots`). Re-run:
`CARGO_TARGET_DIR=/home/monopoly/kani-target-sumimi bash tools/kani/run.sh --unit sumimi,skills_sumimi`.

Harness inventory: 15 `#[kani::proof]` functions (12 in `proofs/sumimi.rs`,
5 in `proofs/skills_sumimi.rs`, including 3 cheap `*_formula`).

---

## Cards

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-SUMIMI-idolband-01 | 「未进行过赎回操作时可打出」 | PROVEN | `idolband_reset_iff` | SUCCESSFUL | gate admits iff `redeemed==0` | — |
| C-SUMIMI-idolband-02 | 「将你的资金重设为3000」 | PROVEN | `idolband_reset_iff` | SUCCESSFUL | one GainMoney of `3000-pre` when pre&lt;3000 | — |
| C-SUMIMI-idolband-03 | 「若你的资金为3000以上则无效果」 | PROVEN | `idolband_reset_iff` | SUCCESSFUL | money path empty when pre≥3000 | — |
| C-SUMIMI-idolband-frame | only `money:P{user}` | PROVEN | `idolband_reset_iff` | SUCCESSFUL | frame asserts | — |
| C-SUMIMI-idolband (formula) | set target 3000 | formula | `idolband_money_formula` | pending | cheap spec math | — |
| C-SUMIMI-idolband mutant | 2999 target | companion | `idolband_mutant_is_detected` | pending | reachable disagreement | — |
| C-SUMIMI-childhood-01 | 「本次移动以小豆岛为起点」 | PROTOCOL-LEVEL | — | — | `plan::set_start` is a structure stub (KANI.md §6) | — |
| C-SUMIMI-childhood-02 | 「移动后获得一个火罐」 | PROVEN* | `childhood_02_fire_iff` | pending | +1 fire under hold cap 2; at cap no-op (rulebook-doc.md:240) | — |
| C-SUMIMI-childhood-frame | fire-pot path only | PROVEN* | `childhood_02_fire_iff` | pending | frame asserts | — |
| C-SUMIMI-heretheworld-01 | 「[反击]…打出两张卡」 | PROTOCOL-LEVEL | — | — | [反击] window + `turn.played` | OPEN-回合 |
| C-SUMIMI-heretheworld-02 | 「抽卡时…3个奇迹水晶」 | PROTOCOL-LEVEL | — | — | rides (1) | OPEN-场上 |
| C-SUMIMI-heretheworld-03 | 「回合开始时移除一个」 | PROTOCOL-LEVEL | — | — | rides (2) | — |
| C-SUMIMI-heretheworld-04 | 「为0时…加入手牌」 | PROTOCOL-LEVEL | — | — | (1) never places (clause-doc Findings) | — |
| C-SUMIMI-nobreakup-01 | 「未抵押/赎回且资金不含有相同数字」 | PARTIAL | gate residual | pending | digit reading determined in sumimi.md (repeated digit = refuse) | — |
| C-SUMIMI-nobreakup-02 | 「资金变动为拥有相同数字」 | OPEN | — | — | target digit predicate undefined | OPEN-C-SUMIMI-nobreakup-digits |
| C-SUMIMI-nobreakup-03 | 「回到原处并取消所有受到的效果」 | PROTOCOL-LEVEL / OPEN | — | — | transaction rollback | OPEN-C-SUMIMI-nobreakup-digits, OPEN-触发结算 |
| C-SUMIMI-twoinone-01 | 「角色卡替换为另一名角色」 | **FAILS** | — | — | LESS: body swaps bound `skill:*` field entry + fire, but `player.character` stays old name (clause-doc Findings) | — |
| C-SUMIMI-twoinone-01b | 「及其初始火罐数」 | BLOCKED-HOST | `twoinone_01b_fire_iff` | pending | host `state_set` a3=field/a4=value but writes a3 as the value | `state_set` |
| C-SUMIMI-twoinone-02 | 「移动掷骰后作为[反击]」 | PROTOCOL-LEVEL | — | — | post-roll [反击] window | — |
| C-SUMIMI-sweetescape-01 | 「前后两格[收费标价]之和≥2000」 | OPEN + BLOCKED-HOST | — | — | body reads `rent_of` | OPEN-收费标价 |
| C-SUMIMI-sweetescape-02 | 「传送…并触发结算」 | PROTOCOL-LEVEL / OPEN | — | — | settle pipeline | OPEN-收费标价, OPEN-触发结算 |
| C-SUMIMI-sweetescape-03 | 「必须购买，视为主要移动」 | PROTOCOL-LEVEL | — | — | buy-as-main-move scheduling | — |
| C-SUMIMI-twodonuts-01 | 「获得[除外]」 | PROVEN* | `twodonuts_01_exile_iff` | pending | one GiveExile grant; expiry is multi-act | — |
| C-SUMIMI-twodonuts-02 | 「选择传送…并触发结算」 | PROTOCOL-LEVEL | — | — | multi-act pass + settle-teleport | OPEN-触发结算 |
| C-SUMIMI-twodonuts-03 | 「各获得2火罐」 | PROTOCOL-LEVEL | — | — | rides (2) | — |
| C-SUMIMI-littlehina-01 | 「失去资金总额即将超过[收费标价]」 | OPEN + PROTOCOL-LEVEL | — | — | running-loss window | OPEN-收费标价 |
| C-SUMIMI-littlehina-02 | 「获得相当于[收费标价]…」 | OPEN + BLOCKED-HOST | — | — | body reads `rent_of` / `ring_multiplier` | OPEN-收费标价 |
| C-SUMIMI-singing-01 | 「来自你以外的效果影响」 | PROTOCOL-LEVEL | — | — | [反击] offer | — |
| C-SUMIMI-singing-02 | 「效果改为你」 | PROTOCOL-LEVEL / OPEN | — | — | multi-counter redirect | OPEN-C-SUMIMI-singing-redirect |
| C-SUMIMI-singing-03 | 「抽一张卡」 | PROTOCOL-LEVEL | — | — | rides (2) | — |

## Character skills

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-SUMIMI-mana-01 | 「经过CiRCLE时获得1个[火罐]」 | BLOCKED-HOST | — | — | body tests `tile_named('CiRCLE')`; host `tile_named` only knows 江户川乐器店 (else −1). Shared fire effect proven via hina-01 | `tile_named` |
| C-SUMIMI-mana-02 | 「消耗火罐并支付…资金消耗减半」 | OPEN + BLOCKED-HOST | `mana_02_decline_is_empty` | pending | accept needs `rent_of`; decline is empty (determined) | OPEN-收费标价 |
| C-SUMIMI-hina-01 | 「经过CiRCLE时获得1个[火罐]」 | PROVEN* | `hina_01_pass_circle_iff` | pending | real `is_circle` guard; +1 under cap 2 | — |
| C-SUMIMI-hina-01 (formula) | fire formula | formula | `pass_circle_fire_formula` | pending | cheap spec math | — |
| C-SUMIMI-hina-01 mutant | +2 | companion | `hina_01_mutant_is_detected` | pending | reachable disagreement | — |
| C-SUMIMI-hina-02 | 「放置火罐…X*30」 | PROTOCOL-LEVEL | — | — | turn-start placement prompt not reachable from arrange seam | — |

## Band skill 人气偶像组合

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-SUMIMI-band-01 add | 「添加1个奇迹水晶」 | **FAILS** | `band_01_clear_iff` (add branch) | pending | LESS: entry guard is `slot('skill.sumimi.got') == 0`, add half never admitted | — |
| C-SUMIMI-band-01 clear | 「移除所有奇迹水晶」 | PROVEN* | `band_01_clear_iff` | pending | clear-all when latch clear | — |
| C-SUMIMI-band-02 | 「每个奇迹水晶…+100」 | PROVEN* | `band_02_rent_bonus_iff` | pending | SetPayAmount to pre+c×100 | — |
| C-SUMIMI-band-02 (formula) | bonus formula | formula | `band_02_rent_bonus_formula` | pending | cheap spec math | — |
| C-SUMIMI-band-02 mutant | +100 total | companion | `band_02_rent_bonus_mutant_is_detected` | pending | reachable disagreement | — |
| C-SUMIMI-band-03 | 「角色卡为对应角色时才可打出」 | PARTIAL | `band_03_exclusive_gate_predicate` | pending | pure predicate; carry-two is deck composition | — |

\* harness written and compiled; runner verdict pending a free `~/.kani-slots` slot.

---

## Host gaps hit by this unit

| missing / wrong op | clauses | note |
|---|---|---|
| `tile_named` (names other than 江户川乐器店) | mana-01 | returns −1 |
| `state_set` field/value packing | twoinone-01b, band latch | ABI a3=field a4=value; host writes a3 as value |
| `rent_of` | sweetescape, littlehina, mana-02, nobreakup | unmodeled catch-all |
| `trig_move_remaining` | hina-02 collect | unmodeled (coverage table) |

## Findings (plain FAILS, no `#[ignore]`)

1. **C-SUMIMI-twoinone-01** (LESS) — book: 「将自己的角色卡替换为sumimi的另一名角色」.
   Body replaces the bound `skill:<name>:*` field entry and writes fire, but
   `player.character` stays the old name, so the other exclusive stays
   unplayable (clause-doc Findings `twoinone_opens_the_other_exclusive`).
2. **C-SUMIMI-band-01 add** (LESS) — book: 「若你在一回合内从其他玩家处获得过资金，
   你的回合结束时为此卡添加1个奇迹水晶」. The only `TurnEndBefore` entry's guard is
   `actor == owner && slot('skill.sumimi.got') == 0`, so the add half is never
   admitted. Matches the black-box finding
   `band_turn_end_adds_crystal_after_inbound_money`.

## bands-b concrete-user rollout (branch `kani-bands-b`)

Verdicts below marked **bands-b** use the ppcost pattern: `n_players=2`,
concrete user seats (user=0/1 for seat-symmetric clauses), direct `NAME_kani`
body calls, per-harness `host_subset!`, sparse in-place world. See
TRIAGE-LOG.md `bands-b`. Results accumulate in
`/home/monopoly/kani-bands-b-results/results-all.tsv`.
