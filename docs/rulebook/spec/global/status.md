# G-STATUS -- [停留] / [晕眩] / [除外] 及其层数

Unit 3 owns this file and `rules/spec/src/global/status.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-STATUS-01

* **ID:** `G-STATUS-01`
* **Source:** `rulebook-doc.md:96-98`
* **Quote:** 「[停留]：处于该状态时[无法移动]。重复受到该效果时可叠加，玩家的每回合结束时移除一层。」
  (and the parallel stacking sentences for [晕眩] / [除外])
* **Trigger:** every observed input, on the settled `post` world.
* **Condition:** the engine's `stay` / `stun` / `stunStart` / `exile` counters.
* **Effect:** every such counter is `>= 0`.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** "可叠加" + "移除一层" makes the counters layer counts; a
  layer count that went negative would mean a decay ran one too many times.
  `stunStart` is the [晕眩] variant that expires at turn start; same
  non-negativity. What each status *forbids* is unit 3's work.
* **Executable:** `rules_spec::global::status::g_status_01`.

---

## Open questions

(None yet.)

## Findings

(Empty.)