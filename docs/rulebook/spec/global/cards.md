# G-CARDS -- 卡牌区域与手牌上限

Unit 2 owns this file and `rules/spec/src/global/cards.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

---

## G-CARDS-01

* **ID:** `G-CARDS-01`
* **Source:** `rulebook-doc.md:126`
* **Quote:** 「基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。」
* **Trigger:** every observed input, after the settle loop (the step's `post`
  world is the rest state a player sees).
* **Condition:** a player's hand-limit counter is the engine's `handLimit`
  state (default 5, raised / lowered by effects). A limit of
  `NO_HAND_LIMIT` (`i32::MAX`) means uncapped.
* **Effect:** no player's private hand length exceeds their limit.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** 「立刻」 means the discard happens inside the same
  transition as the draw that overflowed -- so a check on the settled `post`
  world is the right place. Mid-mulligan / mid-pick hands that a prompt is
  still holding are not `post` of an *answered* act and are not covered here;
  unit 2 owns the in-prompt variants.
* **Executable:** `rules_spec::global::cards::g_cards_01`.

---

## Open questions

(None yet.)

## Findings

(Empty.)