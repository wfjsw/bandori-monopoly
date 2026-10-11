# G-SETTLE -- 基础[结算]

Unit 5 owns this file and `rules/spec/src/global/settle.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-SETTLE-01

* **ID:** `G-SETTLE-01`
* **Source:** `rulebook-doc.md:158`
* **Quote:** 「如果格子地契已抵押则无效果。」 (the owner-landed branch; the rent
  branch at `rulebook-doc.md:160` is the complement: 「如果格子地契未抵押则**[支付]拥有格子的玩家格子地契所标记的现等级地租。**」)
* **Trigger:** an observed input whose event window contains a `rent` / `pay`
  leg attributed to a landed settle on tile `T`.
* **Condition:** `T` is a player-owned buyable tile that is mortgaged in `pre`.
* **Effect:** no `rent` event for `T` in this window.
* **Frame:** n/a (read of the event window).
* **Rulings & OPEN:** the force-buy branch of a mortgaged tile
  (「可选择[支付]...两倍，从该玩家处强行购买」) is a `pay` that is *not* rent
  and is out of scope here -- unit 5 owns it. An event's tile is the `to`
  field on the money line when present; a `rent` with no tile is not a landed
  settle and is ignored.
* **Executable:** `rules_spec::global::settle::g_settle_01`.

---

## Open questions

* OPEN-结算定义 -- 「[结算]」 is defined twice (`rulebook-doc.md:61` and `:87`).
  Use the 专有名词 definition for "execute every effect on the tile"; the
  移动 one is the landing subset.
* OPEN-收费标价 -- 「收费标价」 undefined (Sweet Escape gate; TEST-FINDINGS §6).

## Findings

(Empty.)