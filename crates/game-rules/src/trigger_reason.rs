//! The short **why** clause of a body's 「效果适用」 line.
//!
//! One [`Msg`] per [`Trigger`], derived host-side from the trigger's kind and
//! payload (who / target / tile / value / the chain link it answers). Bracketed
//! terms ([pass], overlap, settle, pay…) match the rulebook's wording, the way
//! existing `log.*` keys do. A kind with no dedicated phrase falls back to
//! `log.reason.{{kind}}`, so nothing ever renders blank.
//!
//! Player / tile / card names ride [`game_core::msg::Arg`] and resolve on the
//! client, so this is a pure function of the trigger.

use crate::world::Trigger;
use crate::Msg;
use crate::TriggerKind;

/// The short reason a body ran -- the `{{reason}}` of 「效果适用」.
///
/// Every `TriggerKind` a shipped entry listens to has a dedicated phrase; the
/// rest fall back to the kind name itself (`log.reason.*` keyed by the wire
/// spelling), so a new raise point still names itself until it gets a phrase.
pub fn trigger_reason(t: &Trigger) -> Msg {
    let who = t.player_id;
    let target = t.target;
    let tile = t.tile;
    let value = t.value;
    let card = t.card.clone();
    // A move-caused trigger reads its direction off the payload; a body that
    // is not about a move has no 「倒走」 half.
    let backward = t.mv.as_ref().is_some_and(|m| m.dir.as_i32() < 0);
    match t.kind {
        // ---- movement ----------------------------------------------------
        TriggerKind::Pass
        | TriggerKind::PassBefore
        | TriggerKind::PassTile
        | TriggerKind::MoveBefore
        | TriggerKind::MoveAfter
        | TriggerKind::MoveResolved => {
            let key = if backward {
                "log.reason.pass_back"
            } else {
                "log.reason.pass"
            };
            Msg::new(key).player_id("who", who).tile("tile", tile)
        }
        TriggerKind::PassPlayer => Msg::new("log.reason.pass_player")
            .player_id("who", who)
            .player_id("target", target),
        TriggerKind::DiceRollBefore
        | TriggerKind::DiceRollResult
        | TriggerKind::DiceRollAfter
        | TriggerKind::DiceRollValue => Msg::new("log.reason.roll")
            .player_id("who", who)
            .i("value", value),
        TriggerKind::Teleported => Msg::new("log.reason.teleport")
            .player_id("who", who)
            .tile("tile", tile),

        // ---- settle ------------------------------------------------------
        TriggerKind::Settle
        | TriggerKind::SettleBefore
        | TriggerKind::SettleAfter
        | TriggerKind::SettleBody
        | TriggerKind::TileResolved => Msg::new("log.reason.settle")
            .player_id("who", who)
            .tile("tile", tile),

        // ---- money -------------------------------------------------------
        TriggerKind::Pay
        | TriggerKind::Paid
        | TriggerKind::PayAfter
        | TriggerKind::PaySplitChoose
        | TriggerKind::PaySplitAdd
        | TriggerKind::PaySplitMul
        | TriggerKind::PaySplitAt
        | TriggerKind::PayCombinedAdd
        | TriggerKind::PayCombinedMul
        | TriggerKind::PayCombinedCancel
        | TriggerKind::PayCalcBefore
        | TriggerKind::RentAdd
        | TriggerKind::RentMul
        | TriggerKind::SettleAdd
        | TriggerKind::SettleMul => {
            if who < 0 {
                Msg::new("log.reason.gain")
                    .player_id("target", target)
                    .i("value", value)
            } else if target < 0 {
                Msg::new("log.reason.pay_bank")
                    .player_id("who", who)
                    .i("value", value)
            } else {
                Msg::new("log.reason.pay")
                    .player_id("who", who)
                    .player_id("target", target)
                    .i("value", value)
            }
        }

        // ---- turn flow ---------------------------------------------------
        TriggerKind::TurnStart
        | TriggerKind::TurnStartBefore
        | TriggerKind::TurnStartAfter
        | TriggerKind::DeckAtGameStart => Msg::new("log.reason.turn_start").player_id("who", who),
        TriggerKind::ManageAfter => Msg::new("log.reason.manage_end").player_id("who", who),
        TriggerKind::TurnEnd
        | TriggerKind::TurnEndBefore
        | TriggerKind::TurnEndAfter
        | TriggerKind::AtEnd
        | TriggerKind::PaySplitChooseBefore
        | TriggerKind::PayCombinedAddBefore
        | TriggerKind::PaySplitMulBefore
        | TriggerKind::EndTurnBefore
        | TriggerKind::SpendBefore
        | TriggerKind::EndTurnAfter => Msg::new("log.reason.turn_end").player_id("who", who),
        TriggerKind::DeckBeforeGame => Msg::new("log.reason.deck_before").player_id("who", who),

        // ---- cards in transit --------------------------------------------
        TriggerKind::Card | TriggerKind::CardAfter | TriggerKind::CardPlayed => match &card {
            Some(c) => Msg::new("log.reason.played")
                .player_id("who", who)
                .card("card", c.clone()),
            None => Msg::new("log.reason.played_no_card").player_id("who", who),
        },
        TriggerKind::Event | TriggerKind::EventAfter => match &card {
            Some(c) => Msg::new("log.reason.event").card("card", c.clone()),
            None => Msg::new("log.reason.event_no_card").player_id("who", who),
        },
        TriggerKind::Drawn | TriggerKind::Drew | TriggerKind::DrewBefore | TriggerKind::DrawOut => {
            match &card {
                Some(c) => Msg::new("log.reason.drew_card")
                    .player_id("who", who)
                    .card("card", c.clone()),
                None => Msg::new("log.reason.drew").player_id("who", who),
            }
        }
        TriggerKind::Discarded | TriggerKind::DiscardBefore | TriggerKind::DiscardAfter => {
            match &card {
                Some(c) => Msg::new("log.reason.discarded").card("card", c.clone()),
                None => Msg::new("log.reason.discarded_no_card").player_id("who", who),
            }
        }
        TriggerKind::Reshuffled => Msg::new("log.reason.reshuffled").player_id("who", who),

        // ---- property ----------------------------------------------------
        TriggerKind::Bought
        | TriggerKind::BuyBefore
        | TriggerKind::BuyAfter
        | TriggerKind::BuyGate
        | TriggerKind::BuyAdd
        | TriggerKind::BuyMul
        | TriggerKind::BuySet
        | TriggerKind::BuyAssign
        | TriggerKind::ForceBuyAdd
        | TriggerKind::ForceBuyMul
        | TriggerKind::ForceBuySet => Msg::new("log.reason.bought")
            .player_id("who", who)
            .tile("tile", tile),
        TriggerKind::BuildBefore | TriggerKind::BuildAfter => Msg::new("log.reason.build")
            .player_id("who", who)
            .tile("tile", tile),
        TriggerKind::Mortgage | TriggerKind::MortgageBefore => Msg::new("log.reason.mortgage")
            .player_id("who", who)
            .tile("tile", tile),

        // ---- status / abnormal -------------------------------------------
        TriggerKind::Abnormal | TriggerKind::AbnormalGuard => Msg::new("log.reason.abnormal")
            .player_id("who", who)
            .player_id("target", target)
            .i("value", value),
        TriggerKind::BeforeOut
        | TriggerKind::Bankrupt
        | TriggerKind::BankruptBefore
        | TriggerKind::BankruptResolved
        | TriggerKind::LeaveBefore
        | TriggerKind::LeaveAfter => Msg::new("log.reason.out").player_id("who", who),
        TriggerKind::State
        | TriggerKind::ActGate
        | TriggerKind::PayGate
        | TriggerKind::MoveGate
        | TriggerKind::ExclusiveUse
        | TriggerKind::TurnBonusTime
        | TriggerKind::TurnBaseTime => Msg::new("log.reason.state").player_id("who", who),

        // ---- targeting / gates -------------------------------------------
        TriggerKind::Target
        | TriggerKind::Targeted
        | TriggerKind::ImmuneAll
        | TriggerKind::Untargetable
        | TriggerKind::Redirect => Msg::new("log.reason.target")
            .player_id("who", who)
            .player_id("target", target),

        // ---- counters / marks --------------------------------------------
        TriggerKind::CounterChanged => Msg::new("log.reason.counter_changed")
            .msg("name", counter_name(t.name.as_deref().unwrap_or("")))
            .i("value", value),
        TriggerKind::MarkerSpend | TriggerKind::MarkerGain => Msg::new("log.reason.marker")
            .player_id("who", who)
            .i("value", value),
        TriggerKind::MarkerChanged => Msg::new("log.reason.marker_changed")
            .player_id("who", who)
            .msg("name", counter_name(t.name.as_deref().unwrap_or("")))
            .i("value", value),

        // ---- misc ---------------------------------------------------------
        TriggerKind::Counteracted => Msg::new("log.reason.counteracted")
            .player_id("who", who)
            .player_id("target", target),
        TriggerKind::Effect => match &card {
            Some(c) => Msg::new("log.reason.effect").card("card", c.clone()),
            None => Msg::new("log.reason.effect_no_card").player_id("who", who),
        },
        TriggerKind::TwoCards => Msg::new("log.reason.two_cards").player_id("who", who),

        // Generic fallback -- never blank. Names the wire kind so a new
        // raise point still says what it is until it gets a phrase.
        other => Msg::new("log.reason.generic")
            .player_id("who", who)
            .msg("kind", kind_name(other.spelling().as_ref())),
    }
}

/// The `{{reason}}` of a counteraction resolution: the answered link's card and what it
/// was doing. `answered_card` names the link's card; the link's own [`Trigger`]
/// supplies the rest.
pub fn counteract_reason(answered_card: &str, answered: &Trigger) -> Msg {
    let what = trigger_reason(answered);
    if answered_card.is_empty() {
        what
    } else {
        Msg::new("log.reason.answering")
            .card("card", answered_card.to_string())
            .msg("what", what)
    }
}

/// A named on-card counter, as its rulebook bracket. Unknown names become a
/// nested `log.counter.other` so the wire spelling is still quoted -- never a
/// raw `Text` arg (those are for player-entered names only).
pub fn counter_name(name: &str) -> Msg {
    if name.is_empty() {
        return Msg::new("log.counter.other").text("name", name);
    }
    Msg::new(format!("log.counter.{name}"))
}

/// A trigger-kind wire name, for the generic why fallback. Never raw.
///
/// Always the dedicated key `log.kind.{spelling}`: the locale file decides
/// whether a phrase exists. A kind with no phrase renders as its key (never
/// blank); `log.kind.wire` remains the raw-spelling shape for callers that
/// want the wire token quoted as data.
fn kind_name(kind: &str) -> Msg {
    Msg::new(format!("log.kind.{kind}"))
}

/// A keyed state item's display phrase. The engine has no key→phrase table:
/// the raw key is quoted as data (`log.state.other`) and the client maps it.
/// Scratch keys never reach here (`outcome_state` filters them out).
pub fn state_key_name(key: &str) -> Msg {
    match key {
        card_sdk::abi::state_key::HAND_LIMIT => Msg::new("log.state.hand_limit"),
        card_sdk::abi::state_key::START_HAND => Msg::new("log.state.start_hand"),
        card_sdk::abi::state_key::SKILL_STATE => Msg::new("log.state.skill_state"),
        other => Msg::new("log.state.other").text("name", other),
    }
}

/// A tile-mark kind. The kind string **is** the i18n key (one mechanism,
/// shared with the designation work): the tile-mark owner, `mark:designate`,
/// `skillBlock`, `ember`, or a card's own display name (怪盗标记). The view
/// resolves known keys and falls back to the literal, so a raw key never
/// reaches the player as a `Text` arg. Empty kind becomes `log.mark.none`.
pub fn mark_kind_name(kind: &str) -> Msg {
    if kind.is_empty() {
        return Msg::new("log.mark.none");
    }
    Msg::new(kind)
}
