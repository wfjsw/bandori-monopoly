//! Worked example of a per-card contract: `通用:10次招募（1回限定）`.
//!
//! This is the **template** units 9-21 copy. The contract is derived from the
//! card's own text in `docs/rulebook/cards.json`, never from implementation
//! behaviour.
//!
//! Card text (roster key `通用:10次招募（1回限定）`):
//!
//! ```text
//! [手]：
//! [消耗]1500资金，抽1张卡。
//! ```
//!
//! So playing it from hand, at rest afterwards:
//!
//! * the player's money fell by exactly 1500 ([消耗] = delete money);
//! * exactly one card was drawn (hand net: -1 played +1 drawn = 0, draw pile
//!   -1 or a reshuffle);
//! * the played card left the hand and (being non-[持续]) entered the discard;
//! * **frame:** nothing outside that player's money / hand / draw / discard
//!   moved (bookkeeping aside).
//!
//! ## How to write your own
//!
//! 1. Quote the card text in this file's header (and in
//!    `docs/rulebook/spec/cards/<group>.md`).
//! 2. Pick the clause IDs that cover it (`C-GENERAL-10次招募-01`, ...).
//! 3. `forall` over randomized pre-states (money above the cost, a non-empty
//!    draw pile), drive **one** transition, assert exact numbers, then
//!    [`rules_spec::frame`] the rest.
//! 4. A discrepancy is a plain failing test. No `#[ignore]`.

use rules_spec::{forall, frame, Clause, Step, Table};

const CARD: &str = "通用:10次招募（1回限定）";

/// C-GENERAL-10次招募-01 -- [消耗]1500资金.
///
/// * **ID:** `C-GENERAL-10次招募-01`
/// * **Source:** `roster#通用:10次招募（1回限定）`
/// * **Quote:** 「[消耗]1500资金，抽1张卡。」
/// * **Trigger:** the [手] effect of this card, played from hand.
/// * **Condition:** the user has at least 1500 money (the engine mortgages /
///   bankrupts otherwise -- that is a different clause).
/// * **Effect:** user's money falls by exactly 1500.
/// * **Frame:** see [`card_10draw_frame`].
/// * **Rulings & OPEN:** 「1回限定」 is not defined in the rulebook body; the
///   once-per-game gate is OPEN (see the area doc). This contract only covers
///   the printed [手] body.
/// * **Executable:** [`g_general_10draw_money`].
pub fn g_general_10draw_money(step: &Step<'_>) -> Result<(), String> {
    // Only fire on a play of this card. `kind == "play"` is
    // `game_core::state::card_trigger::PLAY`; an empty kind is a bare
    // activation with no trigger tag.
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == CARD && (e.kind == "play" || e.kind.is_empty()));
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != -1500 {
        return Err(format!(
            "user {who} money delta {delta}, expected -1500 ([消耗]1500)"
        ));
    }
    Ok(())
}

/// Executable frame for the play: only the user's money and card zones move.
pub fn card_10draw_frame(step: &Step<'_>) -> Result<(), String> {
    let played = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == CARD);
    if !played {
        return Ok(());
    }
    let who = step.player_of_act();
    let allowed = [
        format!("money:P{who}"),
        format!("hand:P{who}"),
        format!("draw:P{who}"),
        format!("discard:P{who}"),
        format!("hand_n:P{who}"),
        format!("draw_n:P{who}"),
        format!("discard_pub:P{who}"),
        // The (1回限定) latch the play records.
        format!("state:P{who}:gacha10_used"),
    ];
    let allowed: Vec<&str> = allowed.iter().map(String::as_str).collect();
    frame(step.pre, step.post, &allowed)
}

/// The clause set a unit-owned card test would arm on `Table::observe`.
pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-GENERAL-10次招募-01",
        "roster#通用:10次招募（1回限定）",
        "[消耗]1500资金，抽1张卡。",
        g_general_10draw_money,
    ),
    Clause::new(
        "C-GENERAL-10次招募-frame",
        "roster#通用:10次招募（1回限定）",
        "[消耗]1500资金，抽1张卡。",
        card_10draw_frame,
    ),
];

/// The contract, as a `forall` over randomized pre-states. This is the shape
/// a unit copies into `tests/card_<group>.rs`.
#[test]
fn ten_draw_contract() {
    forall(8, 0xC0FFEE, |rng, _i| {
        let mut t = Table::vanilla(2);
        // Randomize the pre-state the contract cares about: money above the
        // cost, a draw pile that can pay the draw.
        let money = 1500 + rng.range(0, 40) * 100;
        t.set_money(0, money);
        t.set_draw(0, &["通用:GREAT", "通用:安可", "通用:尽力后的收获"]);
        t.give(0, &[CARD]);

        let pre = t.snapshot_world();
        // Arm the clauses *after* arranging -- the arranged state is not a
        // transition.
        t.observe(CLAUSES);

        t.play(0, CARD).map_err(|e| format!("play rejected: {e}"))?;

        // Exact numbers, not just "something happened".
        let delta = t.money(0) as i64 - money as i64;
        if delta != -1500 {
            return Err(format!("money delta {delta}, expected -1500"));
        }
        // The played card left the hand; one card was drawn.
        let hand = t.hand(0);
        if hand.iter().any(|c| c == CARD) {
            return Err(format!("played card still in hand: {hand:?}"));
        }
        if hand.len() != 1 {
            return Err(format!(
                "hand should hold the one drawn card (played one left), got {hand:?}"
            ));
        }
        if !t.discard(0).iter().any(|c| c == CARD) {
            return Err(format!(
                "non-[持续] card should be in the discard, got {:?}",
                t.discard(0)
            ));
        }

        // Frame: nothing else moved.
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                // The (1回限定) latch the play records.
                "state:P0:gacha10_used",
            ],
        )
    })
    .expect("10次招募 contract");
}
