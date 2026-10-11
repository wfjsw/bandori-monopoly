# Kani unit `sumimi` — clause table

Branch `kani-sumimi` at `/home/monopoly/kani-sumimi-wt`.
Sources: `docs/rulebook/spec/cards/sumimi.md`, `docs/rulebook/cards.json`,
`data/characters.json` `text`, `data/bands.json` `text`.
OPEN-收费标价 is respected: only the determined part of each quote is
specified / proven; the charge-vs-price tag stays OPEN.

## Summary counts

| classification | count |
|---|---:|
| PROVEN | (see verdicts) |
| FAILS | (see findings) |
| PROTOCOL-LEVEL | (see rows) |
| BLOCKED-HOST | (see rows) |
| OPEN | (see rows) |

## Cards

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-SUMIMI-idolband-01 | 「未进行过赎回操作时可打出」 | PROVEN (pending run) | `idolband_reset_iff` | — | gate admits iff `redeemed==0` | — |
| C-SUMIMI-idolband-02 | 「将你的资金重设为3000」 | PROVEN (pending run) | `idolband_reset_iff` | — | set-to-3000 when pre&lt;3000 | — |
| C-SUMIMI-idolband-03 | 「若你的资金为3000以上则无效果」 | PROVEN (pending run) | `idolband_reset_iff` | — | money path empty when pre≥3000 | — |
| C-SUMIMI-idolband-frame | only `money:P{user}` | PROVEN (pending run) | `idolband_reset_iff` | — | frame asserts | — |
| C-SUMIMI-idolband mutant | off-by-one 2999 | companion | `idolband_mutant_is_detected` | — | reachable disagreement | — |
| C-SUMIMI-childhood-01 | 「本次移动以小豆岛为起点」 | PROTOCOL-LEVEL | — | — | `plan::set_start` is a structure stub; move-plan scheduling is engine protocol (KANI.md §6) | — |
| C-SUMIMI-childhood-02 | 「移动后获得一个火罐」 | PROVEN (pending run) | `childhood_02_fire_iff` | — | +1 fire under hold cap (rulebook-doc.md:240) | — |
| C-SUMIMI-childhood-frame | fire-pot path only | PROVEN (pending run) | `childhood_02_fire_iff` | — | frame asserts | — |
| C-SUMIMI-heretheworld-01 | 「[反击]…同一回合内打出两张卡」 | PROTOCOL-LEVEL | — | — | [反击] window + turn.played bookkeeping | OPEN-回合 |
| C-SUMIMI-heretheworld-02 | 「下次抽卡时…放置3个奇迹水晶」 | PROTOCOL-LEVEL | — | — | rides (1); face-down storage engine-defined | OPEN-场上, OPEN-facdown |
| C-SUMIMI-heretheworld-03 | 「每个回合开始时移除一个」 | PROTOCOL-LEVEL | — | — | rides (2) | — |
| C-SUMIMI-heretheworld-04 | 「奇迹水晶数为0时…加入手牌」 | PROTOCOL-LEVEL | — | — | rides (2); (1) never places (clause-doc Findings) | — |
| C-SUMIMI-nobreakup-01 | 「未抵押/赎回且资金不含有相同数字」 | PARTIAL | gate residual `cant_play` | not harness-driven this batch | digit reading determined in sumimi.md (a digit repeats = refuse); mortgage/redeem latch via `state::get` | — |
| C-SUMIMI-nobreakup-02 | 「资金变动为拥有相同数字的传送」 | OPEN | — | — | target digit predicate undefined | OPEN-C-SUMIMI-nobreakup-digits |
| C-SUMIMI-nobreakup-03 | 「回到原处并取消所有受到的效果」 | PROTOCOL-LEVEL / OPEN | — | — | full transaction rollback + open target predicate | OPEN-C-SUMIMI-nobreakup-digits, OPEN-触发结算 |
| C-SUMIMI-twoinone-01 | 「角色卡替换为另一名角色」 | FAILS | — | — | LESS: body swaps the bound `skill:*` field entry and fire, but `player.character` stays the old name (clause-doc Findings `twoinone_opens_the_other_exclusive`) | — |
| C-SUMIMI-twoinone-01b | 「及其初始火罐数」 | BLOCKED-HOST | `twoinone_01b_fire_iff` | pending / host | host `state_set` packs `(player,key,field,value)` as a3=field but writes a3 as the value — `state::set(FIRE, to)` stores the field column | `state_set` a4 |
| C-SUMIMI-twoinone-02 | 「移动掷骰后作为[反击]使用」 | PROTOCOL-LEVEL | — | — | post-roll [反击] window | — |
| C-SUMIMI-sweetescape-01 | 「前后两格…[收费标价]之和≥2000」 | OPEN + BLOCKED-HOST | — | — | tag undefined; body reads `rent_of` (unmodeled) | OPEN-收费标价 |
| C-SUMIMI-sweetescape-02 | 「传送至…并触发结算」 | PROTOCOL-LEVEL / OPEN | — | — | settle pipeline + open tag | OPEN-收费标价, OPEN-触发结算 |
| C-SUMIMI-sweetescape-03 | 「必须购买，视为主要移动」 | PROTOCOL-LEVEL | — | — | buy-as-main-move scheduling | — |
| C-SUMIMI-twodonuts-01 | 「获得[除外]直至…经过」 | PROVEN (pending run) | `twodonuts_01_exile_iff` | — | one GiveExile grant; expiry window is multi-act | — |
| C-SUMIMI-twodonuts-02 | 「经过时…选择传送…并触发结算」 | PROTOCOL-LEVEL | — | — | multi-act pass + optional settle-teleport | OPEN-触发结算 |
| C-SUMIMI-twodonuts-03 | 「各获得2火罐（超出…500资金）」 | PROTOCOL-LEVEL | — | — | rides (2) | — |
| C-SUMIMI-idolband mutant | 2999 target | companion | `idolband_mutant_is_detected` | — | reachable disagreement | — |
| C-SUMIMI-littlehina-01 | 「失去资金总额即将超过[收费标价]」 | OPEN + PROTOCOL-LEVEL | — | — | running-loss window + open tag | OPEN-收费标价 |
| C-SUMIMI-littlehina-02 | 「获得相当于[收费标价]…（RiNG则为基础乘数）」 | OPEN + BLOCKED-HOST | — | — | body reads `rent_of` / `ring_multiplier`; tag undefined | OPEN-收费标价 |
| C-SUMIMI-singing-01 | 「即将受到来自你以外的效果影响」 | PROTOCOL-LEVEL | — | — | [反击] offer on outside effect | — |
| C-SUMIMI-singing-02 | 「反击卡中针对…自身的效果改为你」 | PROTOCOL-LEVEL / OPEN | — | — | multi-counter redirect | OPEN-C-SUMIMI-singing-redirect |
| C-SUMIMI-singing-03 | 「免于受到该影响…抽一张卡」 | PROTOCOL-LEVEL | — | — | rides (2) window close | — |

## Character skills

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-SUMIMI-mana-01 | 「经过CiRCLE时获得1个[火罐]（初始1，上限2）」 | BLOCKED-HOST | — | — | body tests `t == tile_named('CiRCLE')`; host `tile_named` resolves only 「江户川乐器店」 (−1 otherwise). Shared fire effect proven on hina-01 | `tile_named` |
| C-SUMIMI-mana-02 | 「消耗一个火罐并…支付…资金消耗减半」 | OPEN + BLOCKED-HOST | `mana_02_decline_is_empty` (structure only) | pending | accept path needs `rent_of` for 「原本收费」; half-window is a 2-layer TurnEnd state | OPEN-收费标价 |
| C-SUMIMI-hina-01 | 「经过CiRCLE时获得1个[火罐]（初始2，上限2）」 | PROVEN (pending run) | `hina_01_pass_circle_iff` | — | real `is_circle` guard; +1 fire under cap 2 (global hold-cap at/over cap is a no-op) | — |
| C-SUMIMI-hina-02 | 「放置一个火罐…X*30…移除那个火罐」 | PROTOCOL-LEVEL | — | — | turn-start placement prompt not reachable from the arrange seam (clause doc); collect rides PassTile | — |

## Band skill 人气偶像组合

| ID | book quote (short) | classification | harness | verdict + time | finding | OPEN |
|---|---|---|---|---|---|---|
| C-SUMIMI-band-01 (add) | 「从其他玩家处获得过资金…添加1个奇迹水晶」 | FAILS | `band_01_clear_iff` (add branch) | pending | LESS: entry guard is `slot('skill.sumimi.got') == 0`, so the add half is never admitted | — |
| C-SUMIMI-band-01 (clear) | 「否则，移除…所有奇迹水晶」 | PROVEN (pending run) | `band_01_clear_iff` | — | clear-all when latch clear | — |
| C-SUMIMI-band-02 | 「每个奇迹水晶…额外提高100资金」 | PROVEN (pending run) | `band_02_rent_bonus_iff` | — | SetPayAmount to pre+c*100 | — |
| C-SUMIMI-band-02 mutant | +100 total | companion | `band_02_rent_bonus_mutant_is_detected` | — | reachable disagreement | — |
| C-SUMIMI-band-03 | 「仅有在你的角色卡为对应角色时才可打出」 | PARTIAL | `band_03_exclusive_gate_predicate` | pending | pure predicate; carry-two is deck composition | — |

## Host gaps hit by this unit

| missing / wrong op | who needs it | note |
|---|---|---|
| `tile_named` for board names other than 江户川乐器店 | C-SUMIMI-mana-01, childhood start | returns −1 |
| `state_set` field/value packing | C-SUMIMI-twoinone-01b, band latch | ABI a3=field a4=value; host writes a3 as the value |
| `rent_of` | sweetescape, littlehina, mana-02, nobreakup predict | unmodeled catch-all |
| `trig_move_remaining` | hina-02 collect | unmodeled (coverage table) |

## Verdicts

Filled in from `tools/kani/run.sh --unit sumimi` (and `--unit skills_sumimi`).
See `target/kani-results/results.md` in the worktree for the live runner table.