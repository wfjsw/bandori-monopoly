# G-MOVE -- [路径] / [经过] / [传送] / [主要移动]

Unit 4 owns this file and `rules/spec/src/global/movement.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-MOVE-01

* **ID:** `G-MOVE-01`
* **Source:** `rulebook-doc.md:90`
* **Quote:** 「[路径]：[移动起点]与[移动终点]之间的路径所包含的格子，[路径]不包括[移动起点]但包括[移动终点]，如果两点相同则包括」
* **Trigger:** every observed input, on the settled `post` world.
* **Condition:** the board has `n` tiles (indices `0..n`).
* **Effect:** every player's `pos` is in `0..n`.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** the quote fixes what a [路径] *contains*; a full path
  check needs the move's start / end / direction out of the event stream and
  is unit 4's work. This example is the standing index invariant every path
  statement presupposes.
* **Executable:** `rules_spec::global::movement::g_move_01`.

---

## Open questions

* OPEN-强制停下 -- 「[强制停下]」 definition is truncated
  (`rulebook-doc.md:93`). Clauses that mention it specify only what remains.

## Findings

(Empty.)