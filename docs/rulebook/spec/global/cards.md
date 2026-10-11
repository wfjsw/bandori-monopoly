# G-CARDS -- 卡牌区域与手牌上限

Unit 2 owns this file and `rules/spec/src/global/cards.rs`. Append clauses;
do not edit other areas.

Sources: live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
`data/bands.json`) > `docs/rulebook/rulebook-doc.md`. `rulebook.txt` is not
authoritative. TEST-FINDINGS §6 rulings unless superseded by newer sheet text.

Cards have **no instance identity** (`Hidden.hand` is `Vec<String>`). A name
may sit in several zones at once (two copies of one id), so "exactly one
zone" is only checkable as a **multiset** over `hand ∪ draw ∪ discard`. Field
cards and [移除] leave that multiset; draws / discards / reshuffles /
mulligans only rearrange it.

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
  and is not this act's fault. No prompt is open in `post`.
* **Effect:** the player's private hand length is at most the limit in `post`.
* **Frame:** n/a (pure read).
* **Rulings & OPEN:** 「立刻」 means the discard happens inside the same
  transition as the draw that overflowed. Mid-mulligan / mid-pick hands that
  a prompt is still holding are not `post` of an *answered* act.
* **Executable:** `rules_spec::global::cards::g_cards_01`
  (soak + `tests/g_cards.rs::g_cards_01_*`).

---

## G-CARDS-02

* **ID:** `G-CARDS-02`
* **Source:** `rulebook-doc.md:126`
* **Quote:** 「从抽卡区抽取手卡，使用后的非[持续]手卡进入弃卡区。当抽卡区抽光时将弃卡区洗卡并放回抽卡区。」
* **Trigger:** an observed input whose event window is **only** pile
  mechanics (`draw` / `discard` / `mulligan` / log `text`) -- no card body, no
  hand play, no event card.
* **Condition:** the pre-state's `hand ∪ draw ∪ discard` multiset of each
  player (name → count).
* **Effect:** every player's pile multiset is unchanged. Draw: pile → hand.
  Discard: hand → discard. Reshuffle: discard → draw. Mulligan: hand → draw
  → hand. All are permutations of the same names.
* **Frame:** n/a (pure read of the three piles).
* **Rulings & OPEN:** two copies of one name may sit in different zones
  legally -- the multiset, not a per-name zone, is the conserved object.
  Windows that create or [移除] a card are out of scope here (G-CARDS-03 and
  the scenario tests own those, with the exact card named).
* **Executable:** `rules_spec::global::cards::g_cards_02`
  (`tests/g_cards.rs::g_cards_02_pure_draw_conserves_the_pile_multiset`).
  **Not armed on the soak** (see the note in `cards.rs`): a "pure" draw
  window still runs `drawn` / `drew` hooks that may legally create cards, so
  the multiset is not conserved across a fuzz draw even though the book's
  mechanical moves are. The isolated scenario is the executable form.

---

## G-CARDS-03

* **ID:** `G-CARDS-03`
* **Source:** `rulebook-doc.md:126` + `rulebook-doc.md:80`
* **Quote:** 「使用后的非[持续]手卡进入弃卡区。」 /
  「[持续]：卡在任何玩家的[场地]或格子上时持续生效的效果。」
* **Trigger:** a hand play (`type == "play"` / `kind == play`) of a card that
  is **not** tagged [持续], settled to rest. 「手卡」 is the hand-card **zone**
  (as in 「抽取2张手卡」), not the [手] timing tag.
* **Condition:** the card was played from hand. Skip when the card's own text
  [移除]s it or returns it to hand (card effects override -- 特别注意 3), and
  skip when a prompt is still open.
* **Effect:** the card is in the player's discard in `post`, not in hand and
  not on the field.
* **Frame:** n/a (read of that card's zone).
* **Rulings & OPEN:** a negated play still leaves the hand. [持续] cards are
  the contrapositive: they stay on the 场地 / tile
  (`tests/g_cards.rs::g_cards_03_continuous_play_stays_on_the_field`).
* **Executable:** `rules_spec::global::cards::g_cards_03`
  (soak + `tests/g_cards.rs::g_cards_03_non_continuous_play_goes_to_discard`).

---

## G-CARDS-04

* **ID:** `G-CARDS-04`
* **Source:** `rulebook-doc.md:126`
* **Quote:** 「当抽卡区抽光时将弃卡区洗卡并放回抽卡区。」
* **Trigger:** an observed window that logs `log.reshuffle` for player `i`.
* **Condition:** the draw pile was empty (or was emptied by this window's
  draws) at the moment of the reshuffle, and there was a discard to shuffle.
* **Effect:** the reshuffle leaves the player's discard empty and the draw
  pile holding a permutation of the pre-discard.
* **Frame:** n/a.
* **Rulings & OPEN:** the engine also refills after a discard that finds the
  draw already empty; that still satisfies 「当抽卡区抽光时」.
* **Executable:** `rules_spec::global::cards::g_cards_04`
  (soak + `tests/g_cards.rs::g_cards_04_*`).

---

## G-CARDS-05

* **ID:** `G-CARDS-05`
* **Source:** `rulebook-doc.md:128`
* **Quote:** 「非[反击]的[手]效果只能在运营阶段使用，[反击]和[特]效果会描写发动时点。」
* **Trigger:** an accepted hand play (`Act(play)` whose window contains a
  `kind == play` activation) of a card tagged [手].
* **Condition:** the actor is the current player and `pre.st.step` is 运营
  (`stage::OPS`). [反击] windows are prompt answers (`kind == counter`) and
  are out of scope. Cards whose only timing is [特] (no [手] tag) are out of
  scope.
* **Effect:** the play did not happen outside 运营.
* **Frame:** n/a.
* **Rulings & OPEN:** a card tagged both [手] and [反击] is still a [手] play
  when pressed via `Act(play)`.
* **Executable:** `rules_spec::global::cards::g_cards_05`
  (soak + `tests/g_cards.rs::g_cards_05_*`).

---

## G-CARDS-06

* **ID:** `G-CARDS-06`
* **Source:** `rulebook-doc.md:82`
* **Quote:** 「[限]：带有“[限]X”的卡只有在X成立时才可从手中打出。」
* **Trigger:** an attempted or accepted hand play of a card tagged [限].
* **Condition:** X, as printed on the card. The global clause cannot evaluate
  an arbitrary X; the concrete X cases are scenario tests.
* **Effect:** a play of a [限] card outside X is refused (no `play`
  activation). An accepted play is left to the per-card X scenario.
* **Frame:** n/a.
* **Rulings & OPEN:** see OPEN-CARDS-LIMIT-X below. The soak half of this
  clause never *demands* a refusal; the Anon Tokyo scenario pins X.
* **Executable:** `rules_spec::global::cards::g_cards_06`
  (soak + `tests/g_cards.rs::g_cards_06_limit_gate_anon_tokyo`).

---

## G-CARDS-07

* **ID:** `G-CARDS-07`
* **Source:** `rulebook-doc.md:75` + `rulebook-doc.md:112`
* **Quote:** 「[衍生]：不能加入初始卡组，需通过特定效果获得。」 /
  「卡名带有[衍生]的卡不可加入初始卡组」
* **Trigger:** deck construction (选卡), before the opening deal. Not an
  observed transition -- `opening()` is setup -- so this clause is
  scenario-tested, not soak-checked.
* **Condition:** a card whose name carries [衍生] (`CardData::derived`).
* **Effect:** the card is not accepted into a starting deck (`deck::cant_play`
  refuses it; `deck::clean` drops it).
* **Frame:** n/a.
* **Rulings & OPEN:** none.
* **Executable:** `tests/g_cards.rs::g_cards_07_derived_cards_never_in_starting_deck`
  (+ the observational twin `g_cards_08_opening_piles_are_ten_distinct_non_derived`).

---

## G-CARDS-08

* **ID:** `G-CARDS-08`
* **Source:** `rulebook-doc.md:112`
* **Quote:** 「从通用卡组，角色所在乐队的卡组，和角色专属卡中挑选10张（不能重复）组成本局的初始卡组」
* **Trigger:** deck construction (选卡). Scenario-tested (same reason as
  G-CARDS-07).
* **Condition:** the submitted id list.
* **Effect:** a complete starting deck is exactly 10 cards, names not
  repeated.
* **Frame:** n/a.
* **Rulings & OPEN:** 「不能重复」 is read as distinct card names (the engine's
  `deck::clean` is a `HashSet` over ids, then `take(SIZE)`).
* **Executable:**
  `tests/g_cards.rs::g_cards_08_starting_deck_is_ten_distinct` +
  `g_cards_08_opening_piles_are_ten_distinct_non_derived`.

---

## G-CARDS-09

* **ID:** `G-CARDS-09`
* **Source:** `rulebook-doc.md:125` + `rulebook-doc.md:174`
* **Quote:** 「每名玩家的初始格子为1号CiRCLE，拥有10000初始资金并抽取2张手卡。」 /
  「初始抽卡 | 每名玩家抽取2张手卡」
* **Trigger:** the opening deal (开始游戏 row 5). Scenario-tested
  (`opening()` is not an observed transition).
* **Condition:** no skill / card effect has lowered `startHand` (the engine's
  `key::START_HAND`, default 2).
* **Effect:** each player's hand holds exactly 2 cards after the deal.
* **Frame:** n/a.
* **Rulings & OPEN:** effects may lower the opening hand
  (`deckBeforeGame` / 「初始手牌减1」); the book's baseline is 2.
* **Executable:** `tests/g_cards.rs::g_cards_09_opening_draw_is_two`.

---

## G-CARDS-10

* **ID:** `G-CARDS-10`
* **Source:** `rulebook-doc.md:125` + `rulebook-doc.md:175`
* **Quote:** 「仅一次，可将抽取的手卡全部洗回抽卡区并重抽2张。」 /
  「初始重抽 | 每名玩家可独立选择洗回自己初始抽卡获得的手卡并重新抽取2张手卡」
* **Trigger:** the opening mulligan prompt (开始游戏 row 6).
  Scenario-tested (`opening()` is not an observed transition).
* **Condition:** the player's opening hand. Optional: keep is always legal.
* **Effect:** at most one mulligan per player. On redo, the whole opening
  hand is washed back into the draw pile and exactly 2 are redrawn (the pile
  multiset is conserved). After either answer the offer is gone.
* **Frame:** n/a.
* **Rulings & OPEN:** 「全部洗回」 is the whole hand, not a subset. The redraw
  is 2 (the same `startHand` the deal used).
* **Executable:** `tests/g_cards.rs::g_cards_10_mulligan_keep_is_quiet_and_once`
  + `g_cards_10_mulligan_redraws_two_exactly_once`
  (local helper `open_at_mulligan` -- the `Table` constructor auto-answers
  the prompt).

---

## G-CARDS-11

* **ID:** `G-CARDS-11`
* **Source:** `rulebook-doc.md:153`
* **Quote:** 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」
* **Trigger:** an observed window containing an `event` draw.
* **Condition:** the drawn event id. Events live in the shared
  `event_deck` / `event_discard` / `event_removed` / `event_active` zones,
  never in `Hidden.hand`.
* **Effect:** no player's hand contains the drawn event id in `post`. The
  draw is logged (`log.event`) -- public by construction of the log.
* **Frame:** n/a.
* **Rulings & OPEN:** 「公开」 is the public log + the `eventTop` /
  `event_active` views.
* **Executable:** `rules_spec::global::cards::g_cards_11`
  (soak + `tests/g_cards.rs::g_cards_11_*`).

---

## G-CARDS-12

* **ID:** `G-CARDS-12`
* **Source:** `rulebook-doc.md:154`
* **Quote:** 「事件结算后进入事件弃卡区。」
* **Trigger:** an event draw that has fully resolved (the step is at rest).
* **Condition:** the window drew an event and no prompt is open. Skip when
  the event's own text places it on the board (「放置于场地中央」) or [移除]s
  it -- card-effect overrides.
* **Effect:** the drawn event id is in `event_discard` in `post` (or, for a
  stay-on-board event, in `event_active`).
* **Frame:** n/a.
* **Rulings & OPEN:** derived events are 「永久移除」 rather than recycled.
* **Executable:** `rules_spec::global::cards::g_cards_12`
  (soak + `tests/g_cards.rs::g_cards_11_clause_over_a_controlled_event_draw`).

---

## G-CARDS-13

* **ID:** `G-CARDS-13`
* **Source:** `rulebook-doc.md:154`
* **Quote:** 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
* **Trigger:** a window that logs `log.events_reshuffled`.
* **Condition:** `pre.event_deck` was empty and `pre.event_discard` was
  non-empty.
* **Effect:** the post event pile holds a permutation of the pre-discard
  (plus the just-filed event, if this window also drew one).
* **Frame:** n/a.
* **Rulings & OPEN:** the engine waits for the drawn event to fully resolve
  before reshuffling (ruling 2026-10-07, pinned by
  `crates/game-rules/tests/deck_refill_timing.rs`); the book's
  「如果没有事件可抽取」 is the empty pile. This clause only pins *when* a
  reshuffle is legal.
* **Executable:** `rules_spec::global::cards::g_cards_13`
  (soak + `tests/g_cards.rs::g_cards_13_event_discard_reshuffles_only_when_pile_empty`).

---

## Open questions

* **OPEN-CARDS-LIMIT-X** (`rulebook-doc.md:82` vs `AG:即使夕阳落山`): the
  definition reads 「带有“[限]X”的卡只有在X成立时才可从手中打出」 -- X is a
  play gate. `AG:即使夕阳落山` prints
  `[限]：第100轮次开始前此卡的X为原本X的一半`, which reads as a modifier on
  the card's own X rather than a boolean play condition. G-CARDS-06 specifies
  only the determined part (a [限] play outside its printed X is refused);
  the Anon Tokyo case (「自己所在的格子是[可购买格子]」) is pinned as the
  worked example. Do not invent a reading for 即使夕阳落山's X.
* **OPEN-CARDS-START-HAND** (`rulebook-doc.md:125`): 「如无技能或卡效果影响」
  leaves the baseline at 2, but the engine also exposes `startHand` and
  `deckBeforeGame` hooks that lower it (「初始手牌减1」). G-CARDS-09 pins the
  unmodified baseline only.

## Findings

* **G-CARDS-01** (soak, genuine discrepancy): the engine lets a human seat
  sit over `handLimit` after a draw (observed `5 -> 6` over limit 5, no prompt
  open). `draw_cards` always draws past the limit and only logs `over`; the
  auto-discard loop runs **only for AI seats** (`play.rs`, `if p.ai`), and a
  human is expected to discard manually (other acts are blocked with
  `err.over_hand`). The book says 「任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张」
  -- discard down immediately, no resting over the limit. Stays failing.
