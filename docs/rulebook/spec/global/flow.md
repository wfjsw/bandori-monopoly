# G-FLOW -- 阶段顺序 / 行动阶段 / [主要移动]次数

Unit 6 owns this file and `rules/spec/src/global/flow.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Note: the 回合流程 table rows 1-3 are corrupt in the book; use **行动阶段**
(`rulebook-doc.md:199+`) for the turn's action order.

---

## G-FLOW-01

* **ID:** `G-FLOW-01`
* **Source:** `rulebook-doc.md:132`
* **Quote:** 「玩家在运营阶段可在非执行任何效果的时候进入移动阶段。移动阶段中需进行1次[主要移动]。基础[主要移动]为**投掷1d20**，在地图上顺时针移动投掷结果数量的格子，到达[移动终点]后需进行[结算]。[主要移动]和所需[结算]完成后进入结束阶段。」
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

## Open questions

* OPEN-回合流程 -- 回合流程 rows 1-3 corrupt; use **行动阶段**.
* OPEN-支付阶段7 -- where 支付阶段 step 7 sits is undecided.
* OPEN-触发结算 -- 「[触发结算]」 undefined (行动阶段 row 15).

## Findings

(Empty.)