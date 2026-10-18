# G-FLOW -- 阶段顺序 / 行动阶段 / [主要移动]次数

Unit 6 owns this file and `rules/spec/src/global/flow.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Note: the 回合流程 table rows 1-3 are corrupt in the book; use **行动阶段**
(`rulebook-doc.md:199+`) for the turn's action order. Record OPEN-回合流程.

---

## G-FLOW-01

* **ID:** `G-FLOW-01`
* **Source:** `rulebook-doc.md:132` / 行动阶段 row 7
* **Quote:** 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。[主要移动]和所需[结算]完成后进入结束阶段。」 / 「一回合只能触发一次［主要移动］效果,多次触发［主要移动］时无效」
* **Trigger:** an observed input that is a `roll` command, or any input that
  touches the `turn.main_moved` latch.
* **Condition:** `pre.turn.main_moved` is already true in the same turn.
* **Effect:** the latch does not reset mid-turn, and a second `roll` command
  does not move the player (the engine must refuse it / no-op it).
* **Frame:** n/a (read of the turn latch and `pos`).
* **Rulings & OPEN:** 「移动阶段中需进行1次」 is the "exactly one" that makes a
  second attempt illegal. Card-driven dice (a card's 「投掷3d10」) are **not**
  a `roll` command and are out of scope. Card-driven extra moves are
  「[强制移动]」 / 「[传送]」 and are unit 4 and unit 6's shared edge.
* **Executable:** `rules_spec::global::flow::g_flow_01`.

---

## G-FLOW-02

* **ID:** `G-FLOW-02`
* **Source:** `rulebook-doc.md:132`
* **Quote:** 「移动阶段中需进行1次[主要移动]。…[主要移动]和所需[结算]完成后进入结束阶段。」
* **Trigger:** an observed `end` command.
* **Condition:** the actor is in 运营 (`step == OPS`) with the main move still
  owed (`!turn.main_moved`) and the turn is not a skip (`!st.skip_move`).
* **Effect:** the `end` is refused (a pure no-op). Accepting it would enter
  结束阶段 without the required [主要移动].
* **Frame:** n/a.
* **Rulings & OPEN:** 「需进行1次」 is the "exactly one" that makes a pre-move
  `end` illegal. [晕眩] skips both 运营 and the move (G-FLOW-09) and never
  reaches this gate; [停留] sets `skip_move` (G-FLOW-10) and may `end` without
  rolling.
* **Executable:** `rules_spec::global::flow::g_flow_02`.

---

## G-FLOW-03

* **ID:** `G-FLOW-03`
* **Source:** `rulebook-doc.md:125`
* **Quote:** 「每局游戏开始的时候，如无技能或卡效果影响，每名玩家的初始格子为1号CiRCLE，拥有10000初始资金并抽取2张手卡。」
* **Trigger:** the match-start log (`log.match_start`).
* **Condition:** no start-position / start-money effect (the quote's 「如无技能或卡效果影响」).
* **Effect:** the log announces 10000 money; every seat's `pos` is tile 0
  (1号CiRCLE) and `money` is 10000.
* **Frame:** n/a (read of the opening state).
* **Rulings & OPEN:** 「1号CiRCLE」 is board index 0 (`data/board.json` name
  `CiRCLE`). A `deckBeforeGame` hook may move a start position -- those are
  skill/card effects and out of scope here.
* **Executable:** `rules_spec::global::flow::g_flow_03`.

---

## G-FLOW-04

* **ID:** `G-FLOW-04`
* **Source:** `rulebook-doc.md:125` / 开始游戏 rows 5-6
* **Quote:** 「并抽取2张手卡。仅一次，可将抽取的手卡全部洗回抽卡区并重抽2张。」 /
  「| 6 | 初始重抽 | 每名玩家可独立选择洗回自己初始抽卡获得的手卡并重新抽取2张手卡 |」
* **Trigger:** the opening deal (`log.match_start`) and a mulligan (`log.mulligan`).
* **Condition:** the opening hand size is the default 2 (a `deckBeforeGame`
  effect may cut it -- out of scope).
* **Effect:** after the deal each seat holds 2 cards. A `log.mulligan` line
  means that seat returned the opening hand and drew a fresh 2 -- and only
  once (`st.players[i].mulligan`).
* **Frame:** n/a.
* **Rulings & OPEN:** 「仅一次」 is the once-only mulligan. The redraw is of
  the *whole* opening hand (「全部洗回…重抽2张」), not a partial keep.
* **Executable:** `rules_spec::global::flow::g_flow_04`.

---

## G-FLOW-05

* **ID:** `G-FLOW-05`
* **Source:** `rulebook-doc.md:127`
* **Quote:** 「每回合的阶段顺序从头到尾为：开始阶段，运营阶段，移动阶段，结束阶段。」
* **Trigger:** any observed transition that stays inside one turn (no `turn`-typed
  boundary event in the window).
* **Condition:** `pre.st.turn == post.st.turn >= 0`.
* **Effect:** `post.st.step >= pre.st.step` -- the stage never runs backwards
  inside a turn. (Skips jump forward: 晕眩 开始 → 结束.)
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-回合流程 -- the 回合流程 table rows 1-3 are corrupt;
  the **行动阶段** table is the source of the turn's action order. A turn
  boundary (including an extra turn for the same seat) legitimately resets the
  stage to 开始 and is skipped here.
* **Executable:** `rules_spec::global::flow::g_flow_05`.

---

## G-FLOW-06

* **ID:** `G-FLOW-06`
* **Source:** `rulebook-doc.md:127`
* **Quote:** 「**每名玩家的回合结束后立刻进入行动顺序中下一位玩家的回合。**」
* **Trigger:** an observed window in which a turn ended (`log.end_turn`, or an
  auto-skip that calls `end_turn`).
* **Condition:** the match is still in play afterwards.
* **Effect:** the same settled window also contains the next `log.turn` /
  `log.turn_extra`, for the next [存活] seat in action order.
* **Frame:** n/a.
* **Rulings & OPEN:** 「立刻」 is the settle of one `end` input running on into
  the next turn -- no intervening player input. Extra turns (`log.turn_extra`)
  are 「额外回合」 and count as that seat's next turn. A turn-end hook that
  opens a decision (结束阶段后) holds the next turn until that window closes;
  the clause demands the next turn only when the step stopped at rest
  (`!post_prompt_open && !post.st.busy`).
* **Executable:** `rules_spec::global::flow::g_flow_06`.

---

## G-FLOW-07

* **ID:** `G-FLOW-07`
* **Source:** `rulebook-doc.md:98` / 行动阶段 row 1
* **Quote:** 「该效果生效时无法重复获得该效果，玩家的每回合开始前移除一层。」 /
  「| 1 | 回合开始前 | … | 负面效果：移除一层[除外] |」
* **Trigger:** a window in which player `i`'s turn begins (`log.turn` /
  `log.turn_extra` for `i`) and `pre` exile of `i` is already > 0.
* **Condition:** no card activation in the window that could layer more [除外]
  on top of the tick (those windows are skipped).
* **Effect:** `post` exile of `i` is exactly `pre - 1`.
* **Frame:** n/a.
* **Rulings & OPEN:** The tick is 「回合开始前」 -- before the turn's own 开始阶段
  effects. If the layer was the last one, the return [传送]
  (「状态结束后在玩家回合开始时要[传送]到的格子」) is unit 4 / G-STATUS.
* **Executable:** `rules_spec::global::flow::g_flow_07`.

---

## G-FLOW-08

* **ID:** `G-FLOW-08`
* **Source:** `rulebook-doc.md:96-97` / 行动阶段 row 18
* **Quote:** 「[停留]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。」 /
  「[晕眩]：…重复受到该效果时可叠加，玩家的每回合结束时移除一层。」 /
  「| 18 | 结束阶段 | 结束阶段 | 负面效果：移除一层[眩晕]/[停留] |」
* **Trigger:** a window in which player `i`'s turn ended.
* **Condition:** the layer was already > 0 in `pre`, and no card activation in
  the window (a hook may layer a fresh status on top of the tick).
* **Effect:** each of [停留] / [晕眩] is exactly `pre - 1`.
* **Frame:** n/a.
* **Rulings & OPEN:** The tick is 「每回合结束时」 -- 行动阶段 row 18, before
  row 19 (结束阶段后). `stunStart` (「本回合获得的晕眩」) ticks at the *next*
  turn's start and is G-STATUS's business, not this row.
* **Executable:** `rules_spec::global::flow::g_flow_08`.

---

## G-FLOW-09

* **ID:** `G-FLOW-09`
* **Source:** `rulebook-doc.md:97` / 行动阶段 row 4
* **Quote:** 「[晕眩]：处于该状态时[无法移动]，无法使用[主动]技能和手卡的[手]效果，无法收付款。」 /
  「| 4 | 经营/出牌阶段前 | … | 眩晕效果发动：跳过经营和主要移动阶段 |」
* **Trigger:** a window containing `log.stunned_skip` for player `i`.
* **Condition:** `i` is stunned at their turn start.
* **Effect:** no [主要移动] runs for `i` that turn (`turn.main_moved` stays
  false / no `roll` event), and the turn goes straight to its end.
* **Frame:** n/a.
* **Rulings & OPEN:** 行动阶段 row 4 names the skip of 经营 *and* 主要移动.
  [无法移动] is the standing definition; the skip is the turn-flow
  consequence.
* **Executable:** `rules_spec::global::flow::g_flow_09`.

---

## G-FLOW-10

* **ID:** `G-FLOW-10`
* **Source:** `rulebook-doc.md:96` / 行动阶段 row 7
* **Quote:** 「[停留]：处于该状态时[无法移动]。」 /
  「| 7 | 主要移动阶段前 | … | 停留效果发动：跳过主要移动阶段 |」
* **Trigger:** a window containing `log.stay_skip` for the turn player.
* **Condition:** the turn player has [停留] layers and is not [不可阻挡].
* **Effect:** the turn's [主要移动] is skipped (`st.skip_move`); a `roll` for
  that turn is refused; the player may still act in 运营 and then `end`.
* **Frame:** n/a.
* **Rulings & OPEN:** [不可阻挡] 「无视已存在的[停留]」 lifts the skip -- that
  path logs no `stay_skip` and is out of scope here (G-STATUS).
* **Executable:** `rules_spec::global::flow::g_flow_10`.

---

## G-FLOW-11

* **ID:** `G-FLOW-11`
* **Source:** `rulebook-doc.md:132`
* **Quote:** 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。」
* **Trigger:** an observed `roll` command (the input that enters 移动阶段).
* **Condition:** an effect is executing -- `pre.st.busy` (a routine is pending)
  or a prompt is open (`pre_prompt_open`).
* **Effect:** the `roll` does not run (no move, no `main_moved`).
* **Frame:** n/a.
* **Rulings & OPEN:** 「非执行任何效果的时候」 is read as "no pending routine
  and no open decision". A [反击] window is an open decision, so rolling
  through it is illegal. The scenario test `roll_refused_while_an_effect_is_executing`
  is only partial coverage (it needs a card that actually opens a prompt);
  the soak exercises the clause on random busy windows.
* **Executable:** `rules_spec::global::flow::g_flow_11`.

---

## G-FLOW-12

* **ID:** `G-FLOW-12`
* **Source:** `rulebook-doc.md:107`
* **Quote:** 「游玩人数为3到10人。」
* **Trigger:** the match-start log of a Casual (娱乐) match.
* **Condition:** `mode == Casual` and the window announces match start.
* **Effect:** the seat count is in `3..=10`.
* **Frame:** n/a.
* **Rulings & OPEN:** `MatchMode::Casual` is 娱乐; `MatchMode::Ranked` is 正规
  (G-FLOW-14). Solo is a practice table and is not the book's 娱乐 mode.
* **Executable:** `rules_spec::global::flow::g_flow_12`.

---

## G-FLOW-13

* **ID:** `G-FLOW-13`
* **Source:** `rulebook-doc.md:109`
* **Quote:** 「游戏开始时每名玩家投掷1d20，根据出目从大到小决定一局游戏的行动顺序（最大的先开始）。如果多名玩家出目一致则那些玩家再投一次1d20来决定在那些玩家中的顺序，直到排完。」
* **Trigger:** the order log (`log.order` / `log.order_rerolled`).
* **Condition:** the window announces the seat rolls.
* **Effect:** the announced rolls are strictly decreasing across seats (high to
  low, no ties left); each roll is in `1..=20`. A `log.order_rerolled` key
  means at least one tie re-roll happened.
* **Frame:** n/a.
* **Rulings & OPEN:** The engine logs only the *final* rolls per seat, so
  "only the tied players re-rolled" cannot be checked from the event stream
  alone -- the scenario tests pin the final order and uniqueness across seeds.
  OPEN-平局重投: the book's 「那些玩家再投一次」 among tied players is the
  requirement; the log shape cannot distinguish "everyone re-rolled" from
  "only the tied re-rolled". The `log.order` line is emitted inside
  `Match::new`, **before** any `observe` window, so the clause is vacuous on
  the soak; `g_flow::turn_order_1d20_high_to_low` /
  `turn_order_ties_resolve_to_unique_faces` pin the property.
* **Executable:** `rules_spec::global::flow::g_flow_13`.

---

## G-FLOW-14

* **ID:** `G-FLOW-14`
* **Source:** `rulebook-doc.md:117`
* **Quote:** 「游玩人数须在5到6人。」
* **Trigger:** the match-start log of a Ranked (正规) match.
* **Condition:** `mode == Ranked` and the window announces match start.
* **Effect:** the seat count is in `5..=6`.
* **Frame:** n/a.
* **Rulings & OPEN:** `MatchMode::Ranked` is 正规. The engine's Table harness
  allows 2..=10 seats for any mode; a Ranked match outside 5-6 is a book
  violation the scenario tests pin.
* **Executable:** `rules_spec::global::flow::g_flow_14`.

---

## G-FLOW-15

* **ID:** `G-FLOW-15`
* **Source:** `rulebook-doc.md:118-120` / 开始游戏 row 1
* **Quote:** 「选择角色前每名玩家有机会禁用最多一名角色，禁用的角色本局不可被任何玩家选择。」
  「禁用角色的顺数为行动顺序从后到前」 /
  「| 1 | 禁用角色 | 每名玩家依次选择禁用的角色 | 次阶段只有正规模式中使用 |」
* **Trigger:** a `ban` command (or the ban-phase log).
* **Condition:** `mode == Ranked`, phase is `ban`.
* **Effect:** bans are taken from the last seat forward (turn index counts
  down); each seat bans at most once (`ban_done`); a banned name goes on
  `st.bans` and cannot be picked later.
* **Frame:** n/a.
* **Rulings & OPEN:** 「最多一名」 includes zero (skip the ban). The engine
  records a skip as `log.no_ban` and an empty `ban` string.
* **Executable:** `rules_spec::global::flow::g_flow_15`.

---

## G-FLOW-16

* **ID:** `G-FLOW-16`
* **Source:** `rulebook-doc.md:120` / 开始游戏 rows 1-2
* **Quote:** 「选择角色的顺序为行动顺序从前到后。」 /
  「| 2 | 选角色 | 每名玩家依次选择游玩角色 |」
* **Trigger:** a `pick` command in the pick phase.
* **Condition:** phase is `pick`.
* **Effect:** picks are taken from the first seat forward (turn index counts
  up). Each seat picks exactly one character; no two seats share one.
* **Frame:** n/a.
* **Rulings & OPEN:** 开始游戏 row 1 (禁用角色) is Ranked-only; row 2 (选角色)
  runs in every mode. Casual goes straight to pick (no ban).
  Engine/UI (not from the book): a seat's last clicked character (`preselect`)
  is used on pick timeout if still available, else the pick is random.
  TODO(规则书): no book clause for the timeout fallback.
* **Executable:** `rules_spec::global::flow::g_flow_16`.

---

## G-FLOW-17

* **ID:** `G-FLOW-17`
* **Source:** `rulebook-doc.md:121`
* **Quote:** 「仅剩一名[存活]玩家时游戏结束，那名玩家获胜。」
* **Trigger:** the match-end log (`log.match_end`) with reason `last`.
* **Condition:** `mode == Ranked` and the end reason is `last`.
* **Effect:** the winner is the sole [存活] seat (`winner`); the match is `ended`.
* **Frame:** n/a.
* **Rulings & OPEN:** [存活] is 「非[破产]状态且未离开游戏的玩家」
  (`rulebook-doc.md:63`). 娱乐 mode ends by agreement (「可在所有玩家同意时结束
  游戏」) and lists 「仅剩一名[存活]玩家」 only as a *reference* time -- the
  engine auto-ends every mode at last standing. That auto-end in 娱乐 is
  recorded under Findings / OPEN rather than as a hard clause on Casual.
* **Executable:** `rules_spec::global::flow::g_flow_17`.

---

## G-FLOW-18

* **ID:** `G-FLOW-18`
* **Source:** `rulebook-doc.md:168-176`
* **Quote:** 「| 1 | 禁用角色 | 每名玩家依次选择禁用的角色 | 次阶段只有正规模式中使用 |」
  「| 2 | 选角色 | …」 「| 3 | 选卡 | …」
  「| 4 | 游戏开始前 | 发动游戏开始前发动的角色技能和卡的[特]效果 |」
  「| 5 | 初始抽卡 | 每名玩家抽取2张手卡 |」 「| 6 | 初始重抽 | …」
  「| 7 | 游戏开始时 | 发动游戏开始时发动的角色技能和卡的[特]效果 |」
* **Trigger:** the setup stream (ban / pick / deck / match_start events).
* **Condition:** the window carries one of the setup steps.
* **Effect:** phases appear in the table's order -- `ban` (Ranked only) before
  `pick` before `deck` before `log.match_start`. The two [特] points
  (`deckBeforeGame` / `deckAtGameStart`) bracket the deal.
* **Frame:** n/a.
* **Rulings & OPEN:** OPEN-回合流程 -- use **行动阶段** for the turn; 开始游戏's
  own table is intact and is this clause's source. The engine raises
  `deckBeforeGame` before the deal and `deckAtGameStart` after it (and after
  the mulligan), matching rows 4 and 7.
* **Executable:** `rules_spec::global::flow::g_flow_18`.

---

## Open questions

* OPEN-回合流程 -- 回合流程 rows 1-3 corrupt; use **行动阶段** (`rulebook-doc.md:199+`).
* OPEN-支付阶段7 -- where 支付阶段 step 7 sits is undecided (unit 1's window).
* OPEN-触发结算 -- 「[触发结算]」 undefined (行动阶段 row 15).
* OPEN-平局重投 -- the order log carries only final rolls, so "only the tied
  players re-rolled" (G-FLOW-13) cannot be checked from the event stream.
* OPEN-娱乐终局 -- 娱乐 mode ends 「可在所有玩家同意时」, listing last-standing
  only as a reference; the engine auto-ends at last standing in every mode
  (see G-FLOW-17 / Findings).

## Findings

* **G-FLOW-12** (fixed 2026-10-15): 娱乐 mode's book bound is
  「游玩人数为3到10人」. `MatchMode::seat_range` / `check_seats` carry the
  bound; the lobby entries (web-glue `SoloMatch::new`, rules-worker `new`)
  refuse out-of-range seat counts. `Match::new` stays a harness constructor.
  Test: `g_flow::casual_requires_3_to_10_players`.
* **G-FLOW-14** (fixed 2026-10-15): 正规 mode's book bound is
  「游玩人数须在5到6人」. Same `check_seats` / lobby refusal.
  Test: `g_flow::ranked_requires_5_to_6_players`.
