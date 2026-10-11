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
| C-MUJICA-花火-03 | 「最后一个…置入弃牌堆并立刻…2层[眩晕]」 | PROVEN (body stun) / PROTOCOL (discard motion + tick order) | `sparkler_last_crystal_stun_two_iff`, `sparkler_last_crystal_mutant_is_detected` | see run table | body grants 2 stun via `give_stun(2)` and `set_dest(Graveyard)` (no `UnplaceCard` Effect). Engine leaves 1 after the turn-end layer tick (FAILS in contracts). | OPEN-花火眩晕掉层 |
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
| C-MUJICA-若麦技能-01 | 「获得1火罐…提高X*100，X为…火罐数」 | PROVEN (arith) | `nyamu_pay_raise_uses_new_pot_count`, `nyamu_pay_raise_mutant_is_detected` | see run table | PayAdd body + host raise are pipeline | — |
| C-MUJICA-若麦技能-02 | 「到达上限时…进入状态2」 | PROTOCOL-LEVEL | — | — | immediate transition | — |
| C-MUJICA-若麦技能-03 | 「翻为背面朝上…无法产生效果」 | PROTOCOL-LEVEL | — | — | face-down untracked in host | — |

## Band skill — C-MUJICA-团技能 (`skill:Ave Mujica:假面之下的真实`)

| ID | book quote (short) | class | harnesses | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-MUJICA-团技能-00 | 「初始处于状态1」 | PROVEN | `band_starts_in_state1` | see run table | — | — |
| C-MUJICA-团技能-01 | 「1.5倍（只对自己结算…不变）」 | FAILS (one-sided app) / PROVEN (amount rewrite) | `band_state2_pay_is_150_only_on_self`, `band_state2_bend_hook_rewrites_amount`, `band_state2_bend_mutant_is_detected` | see run table | **MORE**: engine moves 1.5x on **both** sides (contract finding). Hook rewrites one `pay_amount`; the host's `pay_to` applies it symmetrically. Book: other party unchanged. | OPEN-取整 (odd amounts) |
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

Filled in after the run table below.

## Run table

Filled in from `tools/kani/run.sh --unit mujica --unit skills_mujica`.
