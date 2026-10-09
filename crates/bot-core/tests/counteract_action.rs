//! The [反击] offer's action surface (`docs/BOT.md` §3.4): one searched
//! decision per **offered card** (the option label names it -- `MatchPrompt::card`
//! is the answered link, not the offer), plus the fallback's skip, and each one
//! maps back to the right `answer` option index. The search must be able to
//! branch on *which* card to declare; "the first declaration" is not it.

use bot_core::action::{self, Action};
use bot_core::view::AiAnswer;
use game_core::msg::Msg;
use game_core::state::{MatchPrompt, MatchState};

/// A [反击] offer naming two cards plus the 「不打」 skip, waiting on seat 1.
fn counteract_prompt() -> MatchPrompt {
    let mut p = MatchPrompt::default();
    p.id = 7;
    p.kind = "choice".into();
    p.title = Msg::new("ask.counteract.title");
    p.text = Msg::new("ask.counteract.text");
    p.options = vec![
        Msg::new("ask.counteract.play").card("card", "TEST:probe"),
        Msg::new("ask.counteract.play").card("card", "TEST:deny"),
        Msg::new("ask.counteract.skip"),
    ];
    p.fallback = 2;
    p.players = vec![1];
    p.answers = vec![-1];
    p
}

fn state_with(p: MatchPrompt) -> MatchState {
    let mut st = MatchState::default();
    st.phase = "play".into();
    st.players.resize(2, Default::default());
    st.prompt = p;
    st
}

#[test]
fn a_counteract_offer_lists_one_action_per_offered_card_plus_the_skip() {
    let st = state_with(counteract_prompt());
    let acts = action::legal_actions(&Default::default(), &st, &[], &[], 1);
    assert_eq!(
        acts,
        vec![
            Action::Counteract {
                card: Some("TEST:probe".into())
            },
            Action::Counteract {
                card: Some("TEST:deny".into())
            },
            Action::Counteract { card: None },
        ],
        "each offered card is its own decision; the fallback is the skip"
    );
}

#[test]
fn each_declaration_maps_back_to_its_own_option_index() {
    let st = state_with(counteract_prompt());
    for (card, want) in [("TEST:probe", 0i32), ("TEST:deny", 1i32)] {
        let msg = action::to_net_message(
            &Action::Counteract {
                card: Some(card.into()),
            },
            &st,
            1,
        );
        assert_eq!(msg.value, want, "{card} must answer its own offer");
        assert_eq!(msg.prompt, 7);
    }
    let msg = action::to_net_message(&Action::Counteract { card: None }, &st, 1);
    assert_eq!(msg.value, 2, "the skip is the fallback");
}

/// The engine's `aiAnswer` (an option index) maps onto the card that option
/// names -- `MatchPrompt::card` is empty on a [反击] offer and must not be
/// mistaken for the declaration.
#[test]
fn the_heuristic_answer_names_the_offered_card_it_picked() {
    let st = state_with(counteract_prompt());
    let hand = ["TEST:probe".to_string(), "TEST:deny".to_string()];
    // The heuristic picked option 1 (TEST:deny), not the skip.
    let ai = AiAnswer {
        answer: 1,
        picked: vec![],
        worth: 0,
    };
    let acts = action::legal_actions(&Default::default(), &st, &hand, &[], 1);
    let priors = action::action_priors(
        &Default::default(),
        &st,
        &hand,
        &[],
        Some(&ai),
        1,
        &acts,
    );
    let deny = Action::Counteract {
        card: Some("TEST:deny".into()),
    };
    let probe = Action::Counteract {
        card: Some("TEST:probe".into()),
    };
    let skip = Action::Counteract { card: None };
    let at = |a: &Action| priors[acts.iter().position(|x| x == a).unwrap()];
    assert_eq!(at(&deny), 1.0, "the heuristic's card is the anchor");
    assert!(at(&probe) < 1.0, "the other declaration is not the anchor");
    assert!(at(&skip) < 1.0, "the skip is not the anchor when it declares");
}

/// A skip answer anchors the skip.
#[test]
fn the_heuristic_skip_anchors_the_fallback() {
    let st = state_with(counteract_prompt());
    let hand = ["TEST:probe".to_string(), "TEST:deny".to_string()];
    let ai = AiAnswer {
        answer: 2,
        picked: vec![],
        worth: 0,
    };
    let acts = action::legal_actions(&Default::default(), &st, &hand, &[], 1);
    let priors = action::action_priors(
        &Default::default(),
        &st,
        &hand,
        &[],
        Some(&ai),
        1,
        &acts,
    );
    let skip = Action::Counteract { card: None };
    let at = |a: &Action| priors[acts.iter().position(|x| x == a).unwrap()];
    assert_eq!(at(&skip), 1.0);
}