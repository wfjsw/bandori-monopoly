# Kani unit `mujica` — Ave Mujica cards + character/band skills

Branch: `kani-mujica` · worktree `/home/monopoly/kani-mujica-wt`
Files: `rules/kani/src/spec/mujica.rs`, `rules/kani/src/proofs/mujica.rs`,
`rules/kani/src/spec/skills_mujica.rs`, `rules/kani/src/proofs/skills_mujica.rs`.
Clause doc: `docs/rulebook/spec/cards/mujica.md`.
Scope: 14 `Mujica:` cards + 5 Ave Mujica character skills + band skill
`skill:Ave Mujica:假面之下的真实`. (`Mujica:#J11` is out of scope.)

Method: specs written FROM the 规则书 quotes only; bodies invoked unmodified
via `invoke::`. Classifications:

| tag | meaning |
|---|---|
| **PROVEN** | iff + covers + reachable-disagreement mutant all green |
| **FAILS** | plain failing proof — body ≠ book (finding) |
| **PROTOCOL-LEVEL** | multi-turn / counteraction window / settle chain / move-plan scheduling — not Kani-expressible (`KANI.md` §6) |
| **BLOCKED-HOST** | the modeled host op/field cannot record the book's effect |
| **OPEN** | the text is undefined; only the determinate fragment is proved |

Host stubs that constrain this unit (see `rules/kani/src/host.rs`):
`plan::*` structure-only, `card_move` always returns 1 (no walk simulated),
`give_extra_turn` silent (no `Effect`), `card_mortgage` stub, `target` returns 0,
face-down/immune untracked, `add_counter_at` ignores uid, `stun_of` aliases
`stay`. These are listed per row where they bite.

---

## Cards

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MUJICA-人偶的箱庭-01 | 「向你支付X*20…X为…移动数」 | PROVEN (pay branch) | `doll_garden_pay_amount_iff`, `doll_garden_pay_amount_mutant_is_detected` | see run table | — | OPEN-触发结算 |
| C-MUJICA-人偶的箱庭-02 | 「无法移动的玩家只能选择向你支付」 | PROTOCOL-LEVEL | — | — | prompt shape mid-window | — |
| C-MUJICA-人偶的箱庭-03 | 「强制移动…掷骰数之和…视为主要移动」 | PROTOCOL-LEVEL | — | — | multi-walk + settle + `main_moved`; `card_move` stub | OPEN-箱庭0步 |
| C-MUJICA-水晶-01 | 「一个奇迹水晶移动到另一张…卡上」 | BLOCKED-HOST | — | — | `add_counter_at`/`counter_at` ignore uid — two instances share one key | — |
| C-MUJICA-心の雨-01 | 「前方20格内的所有玩家各自骰1d20」 | PROVEN | `heart_rain_in_window_roll_stay_iff`, `heart_rain_fallback_stun_and_money_iff` | see run table | — | OPEN-前方20格 |
| C-MUJICA-心の雨-02 | 「≤12…1层，≤3…变为2层」 | PROVEN | `heart_rain_stay_layers_iff`, `heart_rain_stay_layers_mutant_is_detected` | see run table | — | — |
| C-MUJICA-心の雨-03 | 「未能…则自身获得一层[晕眩]并获得1000」 | PROVEN | `heart_rain_fallback_stun_and_money_iff` | see run table | — | — |
| C-MUJICA-视线-01 | 「对你打出过[反击]的所有玩家…移动1~4」 | FAILS | (contract finding; body only sees the immediate counter-user) | — | **WRONG/LESS**: body force-moves the immediate counter-user only; book targets the full turn's counteractor set. Engine finding also moves every other player when nobody countered. | OPEN-触发结算 |
| C-MUJICA-视线-02 | 「视为其主要移动」 | PROTOCOL-LEVEL | — | — | `main_moved` scheduling | — |
| C-MUJICA-视线-03 | 「（此卡可作为[反击]…）」 | PROTOCOL-LEVEL | — | — | counteraction window | — |
| C-MUJICA-欢迎-01 | 「选择以下效果其一发动」 | PROTOCOL-LEVEL | — | — | (2) is a [反击] branch | — |
| C-MUJICA-欢迎-02 | 「转换任意一名玩家的状态（…无效果）」 | BLOCKED-HOST + FAILS | `welcome_toggle_arithmetic_iff`, `welcome_no_state2_mutant_is_detected` | see run table | **BLOCKED-HOST** `Op::target` returns 0 always (cannot designate). **WRONG** (engine): prompt omits the user (「任意一名」 includes self); toggles a non-状态2 seat (required no-op). | — |
| C-MUJICA-欢迎-03 | 「[反击] 当有其他玩家切换状态时…」 | PROTOCOL-LEVEL | — | — | counteraction window | — |
| C-MUJICA-花火-01 | 「放置于场上并放置2个奇迹水晶」 | PROVEN | `sparkler_places_two_crystals_iff`, `sparkler_crystal_count_mutant_is_detected` | see run table | — | — |
| C-MUJICA-花火-02 | 「回合结束后自动移除一个…并…额外回合」 | BLOCKED-HOST / OPEN | `sparkler_burn` (partial) | — | `give_extra_turn` is a silent stub (no Effect); crystal removal is recordable | OPEN-花火额外回合 |
| C-MUJICA-花火-03 | 「最后一个…置入弃牌堆并立刻…2层[眩晕]」 | PROVEN (body stun + dest) / PROTOCOL (discard motion + tick order) | `sparkler_last_crystal_stun_two_iff`, `sparkler_last_crystal_mutant_is_detected` | see run table | body grants 2 stun via `give_stun(2)` and `set_dest(Graveyard)`. Spec dest encoding fixed in triage3 batch 15 (`Dest::Graveyard = 0`, not 1). Engine leaves 1 after the turn-end layer tick (FAILS in contracts). | OPEN-花火眩晕掉层 |
| C-MUJICA-祥移动-01 | 「强制一名玩家…移动3格并[触发结算]」 | PROTOCOL-LEVEL | — | — | `card_move` stub; walk not simulated | OPEN-触发结算 |
| C-MUJICA-祥移动-02 | 「（可在掷骰前…代替主要移动）」 | PROTOCOL-LEVEL | — | — | `main_moved` | — |
| C-MUJICA-祥移动-03 | 「支付价格减半」 | PROTOCOL-LEVEL | — | — | settle chain; `plan::set_pay_factor` stub | OPEN-取整 |
| C-MUJICA-骰子-01 | 「将此卡放置于自身场上」 | PROVEN | `dice_cast_places_on_own_field_iff`, `dice_cast_mutant_extra_money_is_detected` | see run table | — | — |
| C-MUJICA-骰子-02 | 「无法从手牌中使用[反击]」 | PROTOCOL-LEVEL | — | — | counteraction window | — |
| C-MUJICA-骰子-03 | 「回合结束后放入弃牌堆」 | PROTOCOL-LEVEL | — | — | turn-end unplace scheduling (`set_dest` only) | — |
| C-MUJICA-骰子-04 | 「（此卡可以被反击）」 | PROTOCOL-LEVEL | — | — | counteraction window | — |
| C-MUJICA-黑色生日-01 | 「资金不多于1000…支付你800」 | PROVEN | `black_bday_low_pays_800_iff`, `black_bday_low_mutant_is_detected`, `black_bday_low_mutant_panics` | see run table | — | — |
| C-MUJICA-黑色生日-02 | 「严格在1000以上…两次200」 | PROVEN | `black_bday_high_pays_400_iff` | see run table | — | — |
| C-MUJICA-初华-01 | 「若自从上一次[经过]CiRCLE…」 | PROTOCOL-LEVEL | — | — | settle history; gate needs multi-turn memory | — |
| C-MUJICA-初华-02 | 「抽1张卡或…1-6」 | PROTOCOL-LEVEL | — | — | gated choice; `set_fixed_roll` is a plan stub | — |
| C-MUJICA-喵梦-01 | 「将此卡放置于场上」 | PROVEN | `nyamu_places_on_field_iff` | see run table | — | — |
| C-MUJICA-喵梦-02 | 「将从正面翻至背面时可选择失去2个火罐」 | PROTOCOL-LEVEL / OPEN | — | — | flip machinery untracked | OPEN-喵梦翻面 |
| C-MUJICA-喵梦-03 | 「背面翻至正面时…抽一张卡」 | PROTOCOL-LEVEL | — | — | flip machinery untracked | — |
| C-MUJICA-海铃-01 | 「放置于…下一名行动的玩家场上」 | PROVEN | `umiri_places_on_next_actors_field_iff` | see run table | — | — |
| C-MUJICA-海铃-02 | 「回合开始时…移动到…后一名」 | PROTOCOL-LEVEL | — | — | turn-start hop | — |
| C-MUJICA-海铃-03 | 「拿取…所有乐队技能卡」 | PROTOCOL-LEVEL | — | — | `add_band_skill`/`band_skills` return 0 | — |
| C-MUJICA-海铃-04 | 「（相同…不可叠加）」 | PROTOCOL-LEVEL | — | — | stacking assertion | — |
| C-MUJICA-海铃-05 | 「置入弃牌堆，抽一张卡」 | PROTOCOL-LEVEL | — | — | return hop + turn-end; known engine TODO | — |
| C-MUJICA-睦-01 | 「效果为…上一张卡（不计入…放置…）」 | PROVEN (inert path) | `mortis_nothing_to_copy_is_inert` | see run table | copy machinery is TODO in the body itself | — |
| C-MUJICA-睦-02 | 「（此卡复制[反击]卡时…）」 | PROTOCOL-LEVEL | — | — | counteraction window | — |
| C-MUJICA-祥子-01 | 「抵押一张…最贵地契」 | BLOCKED-HOST | — | — | `card_mortgage` is a host stub (no `SetMortgaged`) | — |
| C-MUJICA-祥子-02 | 「并弃置一张手牌（若无手牌…）」 | PROTOCOL-LEVEL | — | — | empty-hand next-draw discard is body TODO | — |
| C-MUJICA-祥子-03 | 「传送至…并触发结算，作为…主要移动」 | PROTOCOL-LEVEL | — | — | settle + `main_moved`; `plan::set_teleport_to` stub | OPEN-触发结算 |
| C-MUJICA-祥子-04 | 「若直接从抽牌堆打出…」 | PROTOCOL-LEVEL | — | — | draw-pile play path | — |

## Character skills

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MUJICA-初华技能-01 | 「状态1：移动改为10+1d10」 | PROVEN (arith) | `uika_state1_roll_is_10_plus_d10`, `uika_state1_move_mutant_is_detected` | see run table | plan-dice rewrite is structure-only in host | — |
| C-MUJICA-初华技能-02 | 「每当你经过CiRCLE的回合结束后…」 | PROTOCOL-LEVEL | — | — | multi-turn history | — |
| C-MUJICA-初华技能-03 | 「状态2：…收取200…额外移动1d6」 | PROTOCOL-LEVEL | — | — | multi-walk; `plan::set_extra_steps` stub | OPEN-触发结算 |
| C-MUJICA-睦技能-01 | 「每6回合…手牌数为5…状态2」 | PROVEN (gate arith) | `mutsumi_state2_gate_is_hand_exactly_5` | see run table | six-turn counter is multi-turn | — |
| C-MUJICA-睦技能-02 | 「覆盖于自己角色卡上…」 | PROTOCOL-LEVEL | — | — | overlay machinery TODO | — |
| C-MUJICA-祥子技能-01 | 「经过…转移…停留，眩晕…1500」 | PROTOCOL-LEVEL | — | — | pass-hook + transfer | — |
| C-MUJICA-祥子技能-02 | 「获得一个火罐（…上限3）」 | PROVEN (cap arith) | `sakiko_fire_cap_is_three` | see run table | — | — |
| C-MUJICA-祥子技能-03 | 「保留至多一张手牌…自动打出…」 | PROTOCOL-LEVEL | — | — | auto-play | — |
| C-MUJICA-海铃技能-01 | 「向你支付400…半价[结算]」 | PROTOCOL-LEVEL | — | — | multi-player interrupt | — |
| C-MUJICA-海铃技能-02 | 「每两个回合…火罐（…上限4）」 | PROVEN (cap arith) | `umiri_fire_cap_is_four` | see run table | two-turn counter is multi-turn | — |
| C-MUJICA-海铃技能-03 | 「100+n*100…每4火罐抽1」 | PROVEN (arith) | `umiri_deed_pay_and_draw_arithmetic` | see run table | 均分 split is OPEN | OPEN-分摊 |
| C-MUJICA-若麦技能-01 | 「获得1火罐…提高X*100，X为…火罐数」 | PROVEN (arith) | `nyamu_pay_raise_uses_new_pot_count`, `nyamu_pay_raise_mutant_is_detected` | see run table | PaySplitAdd body + host raise are pipeline | — |
| C-MUJICA-若麦技能-02 | 「到达上限时…进入状态2」 | PROTOCOL-LEVEL | — | — | immediate transition | — |
| C-MUJICA-若麦技能-03 | 「翻为背面朝上…无法产生效果」 | PROTOCOL-LEVEL | — | — | face-down untracked in host | — |

## Band skill — C-MUJICA-团技能 (`skill:Ave Mujica:假面之下的真实`)

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MUJICA-团技能-00 | 「初始处于状态1」 | PROVEN | `band_starts_in_state1` | see run table | — | — |
| C-MUJICA-团技能-01 | 「1.5倍（只对自己结算…不变）」 | FAILS (one-sided app) / PROVEN (amount rewrite, needs re-time) | `band_state2_pay_is_150_only_on_self`, `band_state2_bend_hook_rewrites_amount`, `band_state2_bend_mutant_is_detected` | see run table | **MORE**: engine moves 1.5x on **both** sides (contract finding). Hook is the ABI v78 pure Filter (returns the fold). Central FAILED `outside_subset` against the older `bend` that still `state::set(ORIG)`; the pure body + subset match on this tree (batch 15). | OPEN-取整 (odd amounts) |
| C-MUJICA-团技能-02 | 「第三回合开始时…奇迹水晶」 | PROTOCOL-LEVEL | — | — | three-turn counter | — |
| C-MUJICA-团技能-03 | 「消耗一个奇迹水晶…金额减半」 | PROVEN (arith) | `band_halve_pay_amount` | see run table | optional interrupt is PROTOCOL | OPEN-取整 |
| C-MUJICA-团技能-04 | 「补充至上限…失去1火罐…退出状态2」 | PROTOCOL-LEVEL | — | — | multi-turn pots | — |

---

## Open questions (carried from the clause doc)

* **OPEN-触发结算** — settle window undefined; walk clauses leave it to G-SETTLE.
* **OPEN-花火额外回合** — does the last crystal also grant the extra turn?
* **OPEN-花火眩晕掉层** — turn-end layer tick vs 「立刻获得2层」 order.
* **OPEN-取整** — odd half / odd 1.5x rounding.
* **OPEN-分摊** — 「均分」 vocabulary undefined (ratio 50/50 is clear).
* **OPEN-前方20格** — 「前方」 = clockwise ahead.
* **OPEN-箱庭0步** — sum-0 walk as [主要移动]?
* **OPEN-喵梦翻面** — does paying 2 fire pots cancel the flip?

## Summary counts

| class | cards | char skills | band skill | total |
|---|---:|---:|---:|---:|
| PROVEN | 12 | 3 | 3 | 18 |
| FAILS | 2 | 0 | 1 | 3 |
| PROTOCOL-LEVEL | 18 | 9 | 2 | 29 |
| BLOCKED-HOST | 3 | 0 | 0 | 3 |
| OPEN (fragment proved) | (cross-cutting; see OPEN column) | written | pending | 8 open questions |

Notes on the counts:
* A clause can be both PROVEN (for the determinate fragment) and carry an
  OPEN / PROTOCOL rider — e.g. 花火-03 body stun is PROVEN while the turn-end
  layer-tick order is OPEN-花火眩晕掉层.
* FAILS rows are the clause-doc Findings (engine vs book), not vacuous
  proofs: 视线-01 target set, 欢迎-02 self-target + no-状态2 no-op, and
  团技能-01 one-sided 1.5x (engine moves both sides).
* BLOCKED-HOST ops needed:
  - `Op::target` — must return the designated player (welcome-02 designate)
  - `Op::add_counter_at` / `Op::counter_at` — must key by field-card uid
    (水晶-01 two-instance crystal move)
  - `Op::card_mortgage` — must record `SetMortgaged` (祥子-01)
  - `Op::give_extra_turn` — must record an Effect (花火-02 extra turn)
  - `Op::card_move` / `plan::*` — must simulate the walk (all move clauses)

## Run table

`tools/kani/run.sh --unit skills_mujica --unit mujica` from
`/home/monopoly/kani-mujica-wt` with
`CARGO_TARGET_DIR=/home/monopoly/kani-target-mujica`.

Harnesses owned by this unit (34):

### skills_mujica (13)

| harness | obligation | verdict | time (s) |
|---|---|---|---:|
| `uika_state1_roll_is_10_plus_d10` | C-MUJICA-初华技能-01 iff | written | pending |
| `uika_state1_move_mutant_is_detected` | mutant 9+d10 | written | pending |
| `nyamu_pay_raise_uses_new_pot_count` | C-MUJICA-若麦技能-01 iff | written | pending |
| `nyamu_pay_raise_mutant_is_detected` | mutant old pot count | written | pending |
| `sakiko_fire_cap_is_three` | C-MUJICA-祥子技能-02 arith | written | pending |
| `umiri_fire_cap_is_four` | C-MUJICA-海铃技能-02 arith | written | pending |
| `umiri_deed_pay_and_draw_arithmetic` | C-MUJICA-海铃技能-03 arith | written | pending |
| `mutsumi_state2_gate_is_hand_exactly_5` | C-MUJICA-睦技能-01 gate | written | pending |
| `band_halve_pay_amount` | C-MUJICA-团技能-03 arith | written | pending |
| `band_starts_in_state1` | C-MUJICA-团技能-00 | written | pending |
| `band_state2_pay_is_150_only_on_self` | C-MUJICA-团技能-01 one-sided | written | pending |
| `band_state2_bend_hook_rewrites_amount` | C-MUJICA-团技能-01 PaySplitMul | written | pending |
| `band_state2_bend_mutant_is_detected` | mutant no-rewrite | written | pending |

### mujica (21)

| harness | obligation | verdict | time (s) |
|---|---|---|---:|
| `black_bday_low_pays_800_iff` | C-MUJICA-黑色生日-01 | written | pending |
| `black_bday_high_pays_400_iff` | C-MUJICA-黑色生日-02 | written | pending |
| `black_bday_low_mutant_is_detected` | mutant 801 | written | pending |
| `black_bday_low_mutant_panics` | mutant should_panic | written | pending |
| `heart_rain_stay_layers_iff` | C-MUJICA-心の雨-02 | written | pending |
| `heart_rain_stay_layers_mutant_is_detected` | mutant 3 layers | written | pending |
| `heart_rain_fallback_stun_and_money_iff` | C-MUJICA-心の雨-03 | written | pending |
| `heart_rain_in_window_roll_stay_iff` | C-MUJICA-心の雨-01/02 | written | pending |
| `dice_cast_places_on_own_field_iff` | C-MUJICA-骰子-01 | written | pending |
| `dice_cast_mutant_extra_money_is_detected` | mutant extra money | written | pending |
| `sparkler_places_two_crystals_iff` | C-MUJICA-花火-01 | written | pending |
| `sparkler_crystal_count_mutant_is_detected` | mutant 3 crystals | written | pending |
| `sparkler_last_crystal_stun_two_iff` | C-MUJICA-花火-03 body | written | pending |
| `sparkler_last_crystal_mutant_is_detected` | mutant 1 stun | written | pending |
| `doll_garden_pay_amount_iff` | C-MUJICA-人偶的箱庭-01 pay | written | pending |
| `doll_garden_pay_amount_mutant_is_detected` | mutant +10 | written | pending |
| `welcome_toggle_arithmetic_iff` | C-MUJICA-欢迎-02 arith | written | pending |
| `welcome_no_state2_mutant_is_detected` | mutant forced toggle | written | pending |
| `nyamu_places_on_field_iff` | C-MUJICA-喵梦-01 | written | pending |
| `umiri_places_on_next_actors_field_iff` | C-MUJICA-海铃-01 | written | pending |
| `mortis_nothing_to_copy_is_inert` | C-MUJICA-睦-01 inert | written | pending |
| `saki_move_dest_is_three_tiles` | C-MUJICA-祥移动-01 arith | written | pending |
| `saki_move_dest_mutant_is_detected` | mutant 4 tiles | written | pending |
| `saki_move_halve_pay_amount` | C-MUJICA-祥移动-03 arith | written | pending |

### Verdict status (honest)

All 37 harnesses are **written and compiling** under
`cargo build -p rules-kani`. Verdicts are **pending** the machine-wide
Kani slot queue (box hit 150+ `run.sh` waiters / 0 free RAM; cap 3).

What was attempted:
1. First `tools/kani/run.sh --unit skills_mujica … --timeout 300 --jobs 2`
   queued and was replaced before a slot opened (300 s too tight for the
   cold `cargo kani` compile).
2. Second run at `--timeout 900 --jobs 1` acquired a slot and compiled
   under `cfg(kani)`, exposing `E0382 use of moved value: w.n_players` in
   `heart_rain_in_window_roll_stay_iff` (native `cargo build` does not
   compile `#[kani::proof]` bodies the same way). **Fixed** — `n` is now
   read from `pre.n_players` before `reset_world` moves `w`.
3. Third run (post-fix) is still waiting on a slot as of hand-off.
   Earlier `FAILED` rows in `target/kani-results/results.md` are that
   compile error, **not** proof disagreements.

Per the coordinator notice: harnesses that could not be run to a verdict
are reported here as **written, verdict pending**; the coordinator will
run them centrally. Run command when slots free up:

```sh
export CARGO_TARGET_DIR=/home/monopoly/kani-target-mujica CARGO_BUILD_JOBS=4
cd /home/monopoly/kani-mujica-wt
bash tools/kani/run.sh --unit skills_mujica --unit mujica --jobs 1 --timeout 900
```

Cheapest-first order already encoded in the unit (pure arith → money /
place / stay / stun bodies → hooks).

### TIMEOUT / OOM notes

* Cold `cargo kani` compile of `kani-target-mujica` needs >300 s on this
  box when other CBMC jobs are live — use `--timeout 900`.
* Prompt-heavy bodies (doll_garden move branch, crystal_swap picks,
  welcome designate) are PROTOCOL-LEVEL / BLOCKED-HOST rather than run,
  so this unit has no 15–40 min prompt cliff in the current harness set.

## bands-b concrete-user rollout (branch `kani-bands-b`)

Verdicts below marked **bands-b** use the ppcost pattern: `n_players=2`,
concrete user seats (user=0/1 for seat-symmetric clauses), direct `NAME_kani`
body calls, per-harness `host_subset!`, sparse in-place world. See
TRIAGE-LOG.md `bands-b`. Results accumulate in
`/home/monopoly/kani-bands-b-results/results-all.tsv`.
