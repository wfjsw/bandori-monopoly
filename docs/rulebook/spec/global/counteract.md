# G-COUNTER -- [反击] 窗口 / 顺序 / 无效

Unit 8 owns this file and `rules/spec/src/global/counteract.rs`. Append
clauses; do not edit other areas.

Terminology: "counteraction" for [反击] windows, "hook" for passive listeners.
Never "reaction".

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-COUNTER-01

* **ID:** `G-COUNTER-01`
* **Source:** `rulebook-doc.md:79`
* **Quote:** 「[反击]：带有“[反击]X：Y”效果在X发生时打出并触发效果Y，且结算优先于X（例：X可以是被其他玩家的卡[指定]时）。」
* **Trigger:** an observed input whose event window contains a `"card"`
  activation marked `negated` (a counteraction cancelled it).
* **Condition:** the negated activation's card body would have moved money /
  status / position (any of `pay` / `rent` / `gain` / `lose` / `mortgage` /
  `redeem` / `move` children).
* **Effect:** no such child events hang under the negated activation
  (`parent == that event's id`). The negation's own log line may.
* **Frame:** n/a (read of the event window).
* **Rulings & OPEN:** 「结算优先于X」 is why the negation can cancel X outright:
  Y resolves first, and a negating Y leaves X's body unrun. The counteraction
  *order* (「从行动顺序上在触发[反击]时点的玩家的下一位玩家开始」) is unit 8's
  separate clause. A `negated` activation still flashes and logs -- only the
  body is empty.
* **Executable:** `rules_spec::global::counteract::g_counter_01`.

---

## Open questions

(None yet.)

## Findings

(Empty.)