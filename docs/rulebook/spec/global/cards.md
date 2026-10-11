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
  `NO_HAND_LIMIT` (`i32::MAX`) means uncapped. The player was **at or under**
  the limit in `pre` -- an arranged over-limit hand is a non-canonical setup
  and is not this act's fault.
* **Effect:** the player's private hand length is at most the limit in `post`.
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

* **G-CARDS-01** (soak, genuine discrepancy): the engine lets a human seat
  sit over `handLimit` after a draw (observed `5 -> 6` over limit 5, no prompt
  open). `draw_cards` always draws past the limit and only logs `over`; the
  auto-discard loop runs **only for AI seats** (`play.rs`, `if p.ai`), and a
  human is expected to discard manually (other acts are blocked with
  `err.over_hand`). The book says 「任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张」
  -- discard down immediately, no resting over the limit. Stays failing.