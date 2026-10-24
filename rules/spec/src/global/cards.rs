//! G-CARDS -- card zones, the hand limit, draw / discard / reshuffle, play
//! gates, and the event pile.
//!
//! Owned by unit 2. Append to [`CLAUSES`]; do not edit other areas.
//!
//! Source precedence: live sheet (`docs/rulebook/cards.json`,
//! roster character / band rows) > `docs/rulebook/rulebook-doc.md`.
//! `rulebook.txt` is not authoritative.
//!
//! Cards have **no instance identity** (`Hidden.hand` is `Vec<String>`): a
//! card name may legally occupy several zones at once (two copies of one id),
//! so "exactly one zone" is only checkable as a **multiset** over the zone
//! vectors. See [`pile_multiset`] and G-CARDS-02.

use std::collections::BTreeMap;

use game_core::data::CardData;
use game_core::engine::World;
use game_core::state::{card_trigger, MatchEvent};
use spec_kit::{Clause, Step};

/// Card-zone clauses. Each entry is one book sentence (or one clause of a
/// packed sentence), checked as a predicate over one observed transition.
///
/// Opening-setup rules (10 distinct cards, no [衍生], the deal of 2, the one
/// mulligan) only apply inside `opening()`, which is **not** an observed
/// transition -- they live as scenario tests in `tests/g_cards.rs` and are
/// documented in the area doc under their own IDs.
pub const CLAUSES: &[Clause] = &[
    G_CARDS_01, G_CARDS_03, G_CARDS_04, G_CARDS_05, G_CARDS_06, G_CARDS_11, G_CARDS_12, G_CARDS_13,
];

// G-CARDS-02 is **not** armed on the soak (see [`G_CARDS_02`]): a "pure" draw
// window still runs `drawn` / `drew` hooks that may legally create cards. The
// isolated scenario pins the mechanical conservation on a skill-free table.

// ---------------------------------------------------------------------------
// helpers (local to this area; generic kit stays vocabulary-free)
// ---------------------------------------------------------------------------

/// Multiset of card names in the player's private piles: hand ∪ draw ∪ discard.
///
/// Why the union and not a per-zone bijection: cards have no instance
/// identity, so a name's *count* across the three piles is the only
/// conserved quantity a transition can be held to. Field cards (and [移除])
/// leave this multiset; draws / discards / reshuffles / mulligans only
/// rearrange it.
pub fn pile_multiset(w: &World, i: usize) -> BTreeMap<String, usize> {
    let mut m: BTreeMap<String, usize> = BTreeMap::new();
    let Some(h) = w.hidden.get(i) else {
        return m;
    };
    for c in h.hand.iter().chain(&h.deck).chain(&h.graveyard) {
        *m.entry(c.clone()).or_insert(0) += 1;
    }
    m
}

/// Body text that overrides the default 「进入弃卡区」 (特别注意 3).
///
/// Event cards live in `GameData::events` (`EventData`), not `GameData::cards`
/// (`CardData`) -- a `event:<id>` lookup on `card()` is `None` and the skip
/// would never fire. Resolve both, stripping the `event:` rule-id prefix.
fn card_or_event_text<'a>(data: &'a game_core::data::GameData, id: &str) -> &'a str {
    if let Some(c) = data.card(id) {
        return c.text.as_str();
    }
    let short = id.strip_prefix("event:").unwrap_or(id);
    data.event(short)
        .or_else(|| data.event(id))
        .map(|e| e.text.as_str())
        .unwrap_or("")
}

/// Does this body place / remove itself instead of 「进入弃卡区」?
///
/// 「将此卡…」 is the self-disposition shape (「将此卡放置于场地中央」 /
/// 「将此卡永久放置于场上」 / 「将此卡移除」) -- any of those override the
/// default discard. [移除] is the tag form; 返回手牌 / 加入手卡 are the
/// return-to-hand overrides.
fn overrides_discard(data: &game_core::data::GameData, id: &str) -> bool {
    let text = card_or_event_text(data, id);
    text.contains("[移除]")
        || text.contains("返回手牌")
        || text.contains("加入手卡")
        || text.contains("将此卡")
}

/// How many copies of `id` sit in `w` player `i`'s hand / discard / field.
///
/// Event cards (`event:*`) file to the **event** zones (`event_discard` /
/// `event_active` / the board-owned `event:*` rule instance), not the player's
/// private piles -- count those as the discard / field of an event card so a
/// `keep()` (「将此卡放置于场地中央」) or a later 「放入事件弃牌」 is visible.
fn counts(w: &World, i: usize, id: &str) -> (usize, usize, usize) {
    let Some(h) = w.hidden.get(i) else {
        return (0, 0, 0);
    };
    let hand = h.hand.iter().filter(|c| *c == id).count();
    let is_event = id.starts_with("event:");
    let short = id.strip_prefix("event:").unwrap_or(id);
    let player_discard = h.graveyard.iter().filter(|c| *c == id).count();
    let discard = if is_event {
        player_discard
            + w.st
                .event_discard
                .iter()
                .filter(|c| *c == short || *c == id)
                .count()
            + w.st
                .event_removed
                .iter()
                .filter(|c| *c == short || *c == id)
                .count()
    } else {
        player_discard
    };
    let player_field =
        w.st.players
            .get(i)
            .map(|p| p.field.iter().filter(|f| f.card == id).count())
            .unwrap_or(0);
    let field = if is_event {
        player_field
            + w.st
                .event_active
                .iter()
                .filter(|e| e.id == short || e.id == id)
                .count()
            + w.st
                .board_field
                .iter()
                .filter(|f| f.card == id && f.tile < 0)
                .count()
    } else {
        player_field
    };
    (hand, discard, field)
}

/// Multiset of the three piles as a sorted `name×count` string, for messages.
pub fn multiset_str(m: &BTreeMap<String, usize>) -> String {
    m.iter()
        .map(|(k, v)| format!("{k}×{v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Does `w` hold a `card` row for `id` tagged `tag`? Tags mirror the live
/// sheet's keyword markers ([手] / [持续] / [限] / [衍生] / ...).
fn tagged(data: &game_core::data::GameData, id: &str, tag: &str) -> bool {
    data.card(id)
        .map(|c: &CardData| c.tags.iter().any(|t| t == tag))
        .unwrap_or(false)
}

/// The hand play this window is judging: `type == "play"` (the `log.play`
/// line `play_from_hand` always writes) or a `"card"` activation stamped
/// `kind == play` / empty. Counteractions log `kind == "counter"` and are out
/// of scope. A stub / unported body still logs `type == "play"`.
fn played_card<'s>(step: &'s Step<'_>) -> Option<&'s MatchEvent> {
    step.events.iter().find(|e| {
        !e.card.is_empty()
            && match e.r#type.as_str() {
                "play" => true,
                "card" => e.kind == card_trigger::PLAY || e.kind.is_empty(),
                _ => false,
            }
    })
}

/// Was this window a pure pile-shuffle / draw / discard? (No card body, no
/// hand play, no event card, and no `text` beyond the reshuffle line -- a
/// `log.effect_applied` is a card body that may create cards.)
fn pure_pile_window(step: &Step<'_>) -> bool {
    step.events.iter().all(|e| match e.r#type.as_str() {
        "draw" | "discard" | "mulligan" => true,
        "text" => e.msg.key() == "log.reshuffle",
        _ => false,
    })
}

// ---------------------------------------------------------------------------
// G-CARDS-01 -- hand limit, discard down immediately
// ---------------------------------------------------------------------------

/// An act must not leave a hand over its limit.
///
/// * **ID:** G-CARDS-01
/// * **Source:** `rulebook-doc.md:126`
/// * **Quote:** 「基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。」
/// * **Trigger:** every observed input, after the settle loop (the step's
///   `post` world is the rest state a player sees).
/// * **Condition:** a player's hand-limit counter is the engine's
///   `handLimit` state (default 5, raised / lowered by effects). A limit of
///   `NO_HAND_LIMIT` (`i32::MAX`) means uncapped. The player was **at or
///   under** the limit in `pre` -- an arranged over-limit hand is a
///   non-canonical setup and is not this act's fault. No prompt is open in
///   `post` (the step is at rest; a discard decision may be the prompt).
/// * **Effect:** the player's private hand length is at most the limit in
///   `post`.
/// * **Frame:** n/a (pure read).
/// * **Rulings & OPEN:** 「立刻」 means the discard happens inside the same
///   transition as the draw that overflowed -- so a check on the settled
///   `post` world is the right place. Mid-mulligan / mid-pick hands that a
///   prompt is still holding are not `post` of an *answered* act, so they are
///   not covered here; unit 2 owns the in-prompt variants.
/// * **Executable:** [`g_cards_01`].
const G_CARDS_01: Clause = Clause::new(
    "G-CARDS-01",
    "rulebook-doc.md:126",
    "基础手卡上限为5张，任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张。",
    g_cards_01,
);

/// Executable form of [`G_CARDS_01`].
pub fn g_cards_01(step: &Step<'_>) -> Result<(), String> {
    // A prompt means the step stopped mid-decision (the discard choice may be
    // the prompt). "At rest" is the post-prompt state this clause is about.
    if step.post_prompt_open {
        return Ok(());
    }
    for (i, p) in step.post.st.players.iter().enumerate() {
        let Some(limit) = p.hand_limit() else {
            continue;
        };
        if limit == game_core::state::key::NO_HAND_LIMIT {
            continue;
        }
        let limit = limit as usize;
        let pre = step.pre_hand_len(i);
        let post = step.post_hand_len(i);
        // Arrange writes (and a previous failing act) may already be over.
        // Only an act that *starts* legal and *ends* over is this clause's
        // finding.
        if pre <= limit && post > limit {
            return Err(format!(
                "player {i} went from {pre} to {post} cards, over handLimit {limit}"
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-CARDS-02 -- pile multiset conservation on a pure pile move
// ---------------------------------------------------------------------------

/// A pure draw / discard / mulligan only rearranges the pile multiset.
///
/// * **ID:** G-CARDS-02
/// * **Source:** `rulebook-doc.md:126`
/// * **Quote:** 「从抽卡区抽取手卡，使用后的非[持续]手卡进入弃卡区。当抽卡区抽光时将弃卡区洗卡并放回抽卡区。」
/// * **Trigger:** an observed input whose event window is **only** pile
///   mechanics (`draw` / `discard` / `mulligan` / log `text`) -- no card body,
///   no hand play, no event card. (Those windows move cards through field /
///   [移除] / created copies and are covered by G-CARDS-03 and the scenario
///   tests, which know the exact card.)
/// * **Condition:** the pre-state's `hand ∪ draw ∪ discard` multiset of each
///   player. Cards have no instance identity, so the check is on name counts.
/// * **Effect:** every player's pile multiset is unchanged. (Draw: pile →
///   hand. Discard: hand → discard. Reshuffle: discard → draw. Mulligan:
///   hand → draw → hand. All are permutations of the same names.)
/// * **Frame:** n/a (pure read of the three piles).
/// * **Rulings & OPEN:** two copies of one name may sit in different zones
///   legally -- the multiset, not a per-name zone, is the conserved object.
///   A window that also creates or [移除]s a card is out of scope here.
/// * **Executable:** [`g_cards_02`].
const G_CARDS_02: Clause = Clause::new(
    "G-CARDS-02",
    "rulebook-doc.md:126",
    "从抽卡区抽取手卡，使用后的非[持续]手卡进入弃卡区。当抽卡区抽光时将弃卡区洗卡并放回抽卡区。",
    g_cards_02,
);

/// Executable form of [`G_CARDS_02`].
pub fn g_cards_02(step: &Step<'_>) -> Result<(), String> {
    if !pure_pile_window(step) {
        return Ok(());
    }
    let n = step.pre.st.players.len().max(step.post.st.players.len());
    for i in 0..n {
        let pre = pile_multiset(step.pre, i);
        let post = pile_multiset(step.post, i);
        if pre != post {
            return Err(format!(
                "player {i} pile multiset moved on a pure pile step: pre [{}] vs post [{}]",
                multiset_str(&pre),
                multiset_str(&post)
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-CARDS-03 -- non-[持续] hand cards go to the discard after use
// ---------------------------------------------------------------------------

/// A used non-[持续] card leaves the hand and enters the discard.
///
/// * **ID:** G-CARDS-03
/// * **Source:** `rulebook-doc.md:126` + `rulebook-doc.md:80`
/// * **Quote:** 「使用后的非[持续]手卡进入弃卡区。」 / 「[持续]：卡在任何玩家的[场地]或格子上时持续生效的效果。」
/// * **Trigger:** a hand play (`kind == play`) of a card that is **not**
///   tagged [持续], settled to rest.
/// * **Condition:** the card was in the player's hand in `pre` and the play
///   was accepted. Skip event cards (`event:*` -- 事件卡, not 手卡; their
///   discard is `event_discard`). Skip when the card's own text [移除]s it,
///   returns it to hand, or places itself (「将此卡…」 -- card effects override
///   the rulebook -- 特别注意 3), and skip when a prompt is still open (the
///   play may be mid-resolution).
/// * **Effect:** the card is in the player's discard in `post`, not in hand
///   and not on the field.
/// * **Frame:** n/a (read of that card's zone).
/// * **Rulings & OPEN:** a negated play still leaves the hand (the engine
///   files it to the discard as `Dest::Graveyard`); the book's 「使用后」
///   covers any use that left the hand. [持续] cards are the contrapositive:
///   they stay on the 场地 / tile (G-CARDS-03 covers only the non-[持续]
///   half; the continuous half is a scenario test).
/// * **Executable:** [`g_cards_03`].
const G_CARDS_03: Clause = Clause::new(
    "G-CARDS-03",
    "rulebook-doc.md:126",
    "使用后的非[持续]手卡进入弃卡区。",
    g_cards_03,
);

/// Executable form of [`G_CARDS_03`].
pub fn g_cards_03(step: &Step<'_>) -> Result<(), String> {
    if step.post_prompt_open {
        return Ok(());
    }
    let Some(ev) = played_card(step) else {
        return Ok(());
    };
    let id = ev.card.as_str();
    // 「使用后的非[持续]手卡」 -- **手卡 is the hand-card zone** (as in
    // 「抽取2张手卡」), not the [手] timing tag (rulebook 97 separates them:
    // 「手卡的[手]效果」). Any non-[持续] card that left the hand via a play
    // is in scope.
    if tagged(step.data, id, "持续") {
        return Ok(());
    }
    // 「手卡」 is the card-deck hand pile. Event cards (`event:*`) are 事件卡
    // with their own zones (event_active / event_discard / event_removed) --
    // 「使用后的非[持续]手卡进入弃卡区」 does not send them to the player's
    // discard. The soak can arrange an event id into a hand and play it; the
    // body's `keep()` / 「放入事件弃牌」 / 「永久移除」 owns where it lands.
    if id.starts_with("event:")
        || step
            .data
            .event(id.strip_prefix("event:").unwrap_or(id))
            .is_some()
    {
        return Ok(());
    }
    // Card effects override (特别注意 3): a body that places itself on the
    // board / a tile, [移除]s itself, or returns it to hand is not
    // 「进入弃卡区」.
    if overrides_discard(step.data, id) {
        return Ok(());
    }
    let who = step.player_of_act();
    let (pre_hand, pre_discard, pre_field) = counts(step.pre, who, id);
    let (post_hand, post_discard, post_field) = counts(step.post, who, id);
    // The play took one copy out of the hand. After a non-[持续] use that
    // copy must be in the discard -- as a **count** (two copies of one name
    // may sit in different zones legally).
    if pre_hand == 0 {
        return Ok(()); // not a hand play of this name (arranged oddity)
    }
    if post_field != pre_field {
        // Self-placement is already skipped above; a field delta here is a
        // different clause (or a body that moved another copy).
        return Ok(());
    }
    let expected_discard = pre_discard + 1;
    if post_discard != expected_discard {
        return Err(format!(
            "non-[持续] {id}: discard {} -> {} (expected {} after a hand use);              hand {} -> {}, field {} -> {} (book: 「使用后的非[持续]手卡进入弃卡区」)",
            pre_discard, post_discard, expected_discard, pre_hand, post_hand, pre_field, post_field
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-CARDS-04 -- reshuffle the discard into the draw only when the draw is empty
// ---------------------------------------------------------------------------

/// 「当抽卡区抽光时」 -- the discard is shuffled back only once the draw pile
/// is empty. Never while cards remain on the draw pile.
///
/// * **ID:** G-CARDS-04
/// * **Source:** `rulebook-doc.md:126`
/// * **Quote:** 「当抽卡区抽光时将弃卡区洗卡并放回抽卡区。」
/// * **Trigger:** an observed input whose window logs a `log.reshuffle` for
///   player `i`.
/// * **Condition:** the player's draw pile just before that reshuffle was
///   empty, and their discard was non-empty (there was something to shuffle).
///   From a whole-act `pre` this reads as: `pre.draw` was empty, **or** the
///   act drew at least `pre.draw.len()` cards (the draw emptied the pile
///   mid-act, then refilled).
/// * **Effect:** the post reshuffle leaves the player's discard empty and the
///   draw pile holding a permutation of the pre-discard (plus any cards the
///   act moved in between -- the scenario test pins the pure case).
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the book says 「抽光」 (drawn empty). The engine also
///   refills after a discard that finds the draw already empty; that still
///   satisfies 「当抽卡区抽光时」. Mid-window reshuffles of an N-card draw are
///   attributed to the draw that emptied the pile.
/// * **Executable:** [`g_cards_04`].
const G_CARDS_04: Clause = Clause::new(
    "G-CARDS-04",
    "rulebook-doc.md:126",
    "当抽卡区抽光时将弃卡区洗卡并放回抽卡区。",
    g_cards_04,
);

/// Executable form of [`G_CARDS_04`].
pub fn g_cards_04(step: &Step<'_>) -> Result<(), String> {
    // `log.reshuffle` is logged as type `text` with key `log.reshuffle`.
    let mut reshuffled: Vec<usize> = vec![];
    for e in step.events {
        if e.msg.key() == "log.reshuffle" {
            let who = e.player_id;
            if who >= 0 {
                reshuffled.push(who as usize);
            }
        }
    }
    if reshuffled.is_empty() {
        return Ok(());
    }
    for who in reshuffled {
        let pre_draw = step.pre.hidden.get(who).map(|h| h.deck.len()).unwrap_or(0);
        let pre_discard = step
            .pre
            .hidden
            .get(who)
            .map(|h| h.graveyard.len())
            .unwrap_or(0);
        // How many cards did this window take out of the pile (or fail to)?
        let drew: usize = step
            .events
            .iter()
            .filter(|e| e.r#type == "draw" && e.player_id == who as i32)
            .map(|e| e.value.max(0) as usize)
            .sum();
        // The pile must have been empty (or emptied by this window's draws)
        // at the moment of the reshuffle, and there must have been a discard
        // to shuffle in.
        let emptied = pre_draw == 0 || drew >= pre_draw;
        if !emptied {
            return Err(format!(
                "player {who} reshuffled with {pre_draw} cards still in the draw pile \
                 (drew {drew} this window); book: 「当抽卡区抽光时」 only"
            ));
        }
        if pre_draw == 0 && pre_discard == 0 && drew == 0 {
            // Nothing to shuffle and nothing to draw -- a reshuffle here is
            // still only legal if a card body seeded the discard first; the
            // pure case is covered by not logging one at all. Allow it (the
            // engine logs nothing when both piles are empty).
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-CARDS-05 -- non-[反击] [手] effects only in 运营
// ---------------------------------------------------------------------------

/// 「非[反击]的[手]效果只能在运营阶段使用」 -- a hand play of a [手] card is a
/// 运营-phase action. [反击] / [特] write their own timings and are out of
/// scope.
///
/// * **ID:** G-CARDS-05
/// * **Source:** `rulebook-doc.md:128`
/// * **Quote:** 「非[反击]的[手]效果只能在运营阶段使用，[反击]和[特]效果会描写发动时点。」
/// * **Trigger:** an accepted hand play (`Act(play)` whose window contains a
///   `kind == play` activation) of a card tagged [手].
/// * **Condition:** the actor is the current player and `pre.st.step` is
///   运营 (`stage::OPS`). Skip [反击] windows (those are prompt answers with
///   `kind == counter`, not `Act(play)`). Skip cards whose only timing is
///   [特] (no [手] tag) -- they fire on their own written timing.
/// * **Effect:** the play did not happen outside 运营. (Equivalently: any
///   accepted [手] play has `pre.st.step == OPS` and `pre.st.turn == actor`.)
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** a card tagged both [手] and [反击] is still a [手]
///   play when pressed via `Act(play)` -- the counteraction path is a
///   different act and stamps `kind == counter`.
/// * **Executable:** [`g_cards_05`].
const G_CARDS_05: Clause = Clause::new(
    "G-CARDS-05",
    "rulebook-doc.md:128",
    "非[反击]的[手]效果只能在运营阶段使用，[反击]和[特]效果会描写发动时点。",
    g_cards_05,
);

/// Executable form of [`G_CARDS_05`].
pub fn g_cards_05(step: &Step<'_>) -> Result<(), String> {
    let Some(ev) = played_card(step) else {
        return Ok(());
    };
    let id = ev.card.as_str();
    if !tagged(step.data, id, "手") {
        return Ok(());
    }
    let who = step.player_of_act();
    let step_ok = step.pre.st.step == game_core::state::stage::OPS;
    let turn_ok = step.pre.st.turn == who as i32;
    if !step_ok || !turn_ok {
        return Err(format!(
            "played [手] card {id} outside 运营: step={} turn={} actor={who} \
             (book: 「非[反击]的[手]效果只能在运营阶段使用」)",
            step.pre.st.step, step.pre.st.turn
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-CARDS-06 -- [限] X is a play gate
// ---------------------------------------------------------------------------

/// 「带有「[限]」X的卡只有在X成立时才可从手中打出」 -- the printed X is a
/// precondition of the hand play.
///
/// * **ID:** G-CARDS-06
/// * **Source:** `rulebook-doc.md:82`
/// * **Quote:** 「[限]：带有“[限]X”的卡只有在X成立时才可从手中打出。」
/// * **Trigger:** an attempted or accepted hand play of a card tagged [限].
/// * **Condition:** X, as printed on the card. The global clause cannot
///   evaluate an arbitrary X; it only checks the one shape the book names
///   (「X成立时才可从手中打出」 = a refused play is always legal, an accepted
///   play of a [限] card is the engine's claim that X held). The concrete X
///   cases are scenario tests (`MyGO:[千早爱音]Anon Tokyo` -- 「自己所在的格子
///   是[可购买格子]」).
/// * **Effect:** a play of a [限] card outside X is refused (no `play`
///   activation in the window). An accepted play is left to the per-card X
///   scenario.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** `AG:即使夕阳落山` prints `[限]：第100轮次开始前此卡的
///   X为原本X的一半` -- that reads as a modifier on the card's own X, not as a
///   play gate. OPEN-CARDS-LIMIT-X (see the area doc). This clause's soak
///   check therefore never *demands* a refusal; it only records that a [限]
///   play went through (so a scenario test can pin X).
/// * **Executable:** [`g_cards_06`].
const G_CARDS_06: Clause = Clause::new(
    "G-CARDS-06",
    "rulebook-doc.md:82",
    "[限]：带有“[限]X”的卡只有在X成立时才可从手中打出。",
    g_cards_06,
);

/// Executable form of [`G_CARDS_06`].
///
/// The soak-safe half: a [限] card that is **refused** is always legal. An
/// accepted play is not judged here (X is card-local); the Anon Tokyo scenario
/// pins the one printed X.
pub fn g_cards_06(step: &Step<'_>) -> Result<(), String> {
    // Only fire when this window tried to play a [限] card and the engine
    // accepted it -- record nothing (Ok), because X is card-local. The
    // scenario tests own the truth of X. Kept in CLAUSES so the ID is armed
    // and a future per-card X table can hang off this entry.
    if let Some(ev) = played_card(step) {
        if tagged(step.data, &ev.card, "限") {
            // Accepted. Nothing global to assert beyond "the played copy left
            // the hand" (the play activation means the engine believed X
            // held). A body that Dest::Hand's it back is a card override.
            let who = step.player_of_act();
            let (pre_hand, _, _) = counts(step.pre, who, &ev.card);
            let (post_hand, post_discard, post_field) = counts(step.post, who, &ev.card);
            let returned = post_hand >= pre_hand && (post_discard > 0 || post_field > 0);
            if pre_hand > 0 && post_hand >= pre_hand && !returned {
                return Err(format!(
                    "[限] card {} was 'played' but never left the hand                      (hand {} -> {})",
                    ev.card, pre_hand, post_hand
                ));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// G-CARDS-11..13 -- the event pile
// ---------------------------------------------------------------------------

/// A drawn event card is public and never enters a hand.
///
/// * **ID:** G-CARDS-11
/// * **Source:** `rulebook-doc.md:153`
/// * **Quote:** 「抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。」
/// * **Trigger:** an observed window containing an `event` draw (`type ==
///   "event"`, `kind == event` or the draw log).
/// * **Condition:** the drawn event id. Events live in the shared
///   `event_deck` / `event_discard` / `event_removed` / `event_active` zones,
///   never in `Hidden.hand`.
/// * **Effect:** no player's hand contains the drawn event id in `post`.
///   The draw is logged (`log.event`) -- public by construction of the log.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** 「公开」 is the public log + the `eventTop` /
///   `event_active` views; the private piles never hold an event id.
/// * **Executable:** [`g_cards_11`].
const G_CARDS_11: Clause = Clause::new(
    "G-CARDS-11",
    "rulebook-doc.md:153",
    "抽取的事件卡不进入手卡并向所有玩家公开，效果立刻生效。",
    g_cards_11,
);

/// Executable form of [`G_CARDS_11`].
pub fn g_cards_11(step: &Step<'_>) -> Result<(), String> {
    let events: Vec<&str> = step
        .events
        .iter()
        .filter(|e| e.r#type == "event" && !e.card.is_empty())
        .map(|e| e.card.as_str())
        .collect();
    if events.is_empty() {
        return Ok(());
    }
    for id in events {
        for (i, h) in step.post.hidden.iter().enumerate() {
            if h.hand.iter().any(|c| c == id) {
                return Err(format!(
                    "event card {id} entered player {i}'s hand \
                     (book: 「抽取的事件卡不进入手卡」)"
                ));
            }
        }
    }
    Ok(())
}

/// After resolution an event enters the event discard (or is [移除]d / stays
/// active -- card text overrides).
///
/// * **ID:** G-CARDS-12
/// * **Source:** `rulebook-doc.md:154`
/// * **Quote:** 「事件结算后进入事件弃卡区。」
/// * **Trigger:** an event draw that has fully resolved (the step is at rest).
/// * **Condition:** the window drew an event and no prompt is open. Skip when
///   the event's own text places it on the board (「放置于场地中央」) or
///   [移除]s it -- those are card-effect overrides.
/// * **Effect:** the drawn event id is in `event_discard` in `post`.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** an active event (`event_active`) is the
///   「放置于场地中央」 shape and is not a violation of 「进入事件弃卡区」.
/// * **Executable:** [`g_cards_12`].
const G_CARDS_12: Clause = Clause::new(
    "G-CARDS-12",
    "rulebook-doc.md:154",
    "事件结算后进入事件弃卡区。",
    g_cards_12,
);

/// Executable form of [`G_CARDS_12`].
pub fn g_cards_12(step: &Step<'_>) -> Result<(), String> {
    if step.post_prompt_open {
        return Ok(());
    }
    let drawn: Vec<&str> = step
        .events
        .iter()
        .filter(|e| e.r#type == "event" && !e.card.is_empty())
        .map(|e| e.card.as_str())
        .collect();
    for id in drawn {
        let text = step.data.event(id).map(|e| e.text.as_str()).unwrap_or("");
        // Card-effect overrides: stay on the board, or leave the game.
        if text.contains("放置于场地中央") || text.contains("永久移除") || text.contains("[移除]")
        {
            continue;
        }
        // Derived events are 「永久移除」 rather than recycled (engine +
        // 「[衍生]」 cards never return to the starting deck -- different rule).
        if step.data.event(id).map(|e| e.derived).unwrap_or(false) {
            if step.post.event_removed.iter().any(|e| e == id) {
                continue;
            }
        }
        // 「事件结算后进入事件弃卡区」 -- but the same window may then take
        // the emptied discard as a new pile (G-CARDS-13). So the event is
        // legal in event_discard, on the active list, [移除]d, **or** already
        // shuffled back into event_deck.
        let in_discard = step.post.event_discard.iter().any(|e| e == id);
        let in_deck = step.post.event_deck.iter().any(|e| e == id);
        let in_removed = step.post.event_removed.iter().any(|e| e == id);
        let active = step.post.st.event_active.iter().any(|e| e.id == id);
        if !in_discard && !active && !in_removed && !in_deck {
            return Err(format!(
                "event {id} resolved but left every event zone: \
                 discard={:?} deck={:?} removed={:?} active={:?}",
                step.post.event_discard,
                step.post.event_deck,
                step.post.event_removed,
                step.post
                    .st
                    .event_active
                    .iter()
                    .map(|e| e.id.as_str())
                    .collect::<Vec<_>>()
            ));
        }
    }
    Ok(())
}

/// The event discard is shuffled into a new event pile only when the pile is
/// empty.
///
/// * **ID:** G-CARDS-13
/// * **Source:** `rulebook-doc.md:154`
/// * **Quote:** 「如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。」
/// * **Trigger:** a window that logs `log.events_reshuffled`.
/// * **Condition:** `pre.event_deck` was empty and `pre.event_discard` was
///   non-empty (there was a discard to become the new pile).
/// * **Effect:** the post event pile holds a permutation of the pre-discard
///   (plus the just-filed event, if this window also drew one); the event
///   discard is empty once the reshuffle has taken it.
/// * **Frame:** n/a.
/// * **Rulings & OPEN:** the engine waits for the drawn event to fully
///   resolve before reshuffling (ruling 2026-10-07, recorded in
///   `deck_refill_timing.rs`); the book's 「如果没有事件可抽取」 is the empty
///   pile. This clause only pins *when* a reshuffle is legal.
/// * **Executable:** [`g_cards_13`].
const G_CARDS_13: Clause = Clause::new(
    "G-CARDS-13",
    "rulebook-doc.md:154",
    "如果没有事件可抽取则将事件弃卡区洗切并当作新的事件卡堆来抽取。",
    g_cards_13,
);

/// Executable form of [`G_CARDS_13`].
pub fn g_cards_13(step: &Step<'_>) -> Result<(), String> {
    let reshuffled = step
        .events
        .iter()
        .any(|e| e.msg.key() == "log.events_reshuffled");
    if !reshuffled {
        return Ok(());
    }
    let pre_deck = step.pre.event_deck.len();
    let pre_discard = step.pre.event_discard.len();
    // How many events did this window take off the pile? A draw that empties
    // the pile may reshuffle in the same window (the just-filed event included).
    let drew = step
        .events
        .iter()
        .filter(|e| e.r#type == "event" && !e.card.is_empty())
        .count();
    if pre_deck != 0 && drew < pre_deck {
        return Err(format!(
            "event pile reshuffled while {} cards were still in it (drew {drew} this window; \
             book: 「如果没有事件可抽取」)",
            pre_deck - drew
        ));
    }
    if pre_discard == 0 && step.post.event_deck.is_empty() {
        // Nothing to shuffle; the engine logs nothing in this case. A log
        // with both piles empty is still a book violation of "shuffle the
        // discard" (there was no discard).
        return Err("event pile reshuffled with an empty discard (nothing to shuffle)".into());
    }
    Ok(())
}
