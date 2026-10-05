//! The boundary where card content plugs into the match shell.
//!
//! The shell calls these hooks at the same points the C# does; an implementation
//! decides what cards and events actually do. `game-rules` will implement this over
//! the WASM card modules. [`StubRules`] gives every card and event no effect, which
//! leaves a complete, playable game of plain BanG Dream Monopoly.

use super::cx::{Cx, Flow};
use crate::msg::Msg;

/// Where a played card goes afterwards (C# `PlayCtx.Dest`).
///
/// The port's names for the fates: Graveyard (discard pile, 弃卡区), Hand,
/// Field (stays in play, 场上), Banished (「[移除]」, out of the game).
/// Planned: the draw-pile fates `DeckTop` / `DeckBottom` / `DeckRandom` (C#
/// `c.Dest = "deck"` -> `H.AddToDeck(seat, card, where)` with `where` =
/// `"top"` / `"bottom"` / `"shuffle"`; the C# default is **shuffle**).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dest {
    /// The discard pile (弃卡区) -- the default.
    #[default]
    Graveyard,
    /// Back to the hand (手牌).
    Hand,
    /// Stays in play (场上) -- a persistent effect.
    Field,
    /// 「[移除]」 -- out of the game entirely.
    Banished,
}

/// Points in the flow where reaction cards may answer (C# `Trigger.Kind`).
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub kind: &'static str,
    pub seat: i32,
    pub target: i32,
    pub tile: i32,
    pub card: String,
    pub value: i32,
}

impl Trigger {
    pub fn new(kind: &'static str, seat: usize) -> Self {
        Self { kind, seat: seat as i32, target: seat as i32, tile: -1, card: String::new(), value: 0 }
    }
}

pub trait CardRules: Send + Sync {
    /// `Card.Normal` -- may be played from hand in the operation phase.
    fn normal(&self, _card: &str) -> bool {
        true
    }

    /// `Card.WhyNot` -- extra card-specific reason it can't be played right now.
    fn why_not(&self, _cx: &Cx, _seat: usize, _card: &str) -> Option<Msg> {
        None
    }

    /// `Card.AiPlay` -- would a bot play it now?
    fn ai_play(&self, _cx: &Cx, _seat: usize, _card: &str) -> bool {
        true
    }

    /// `Card.Play` -- the effect. Returns where the card goes afterwards.
    fn play(&self, cx: &mut Cx, seat: usize, card: &str) -> Flow<Dest>;

    /// Resolve a drawn event card. Returns `true` if the event stays in play
    /// (otherwise it is discarded).
    fn event(&self, cx: &mut Cx, seat: usize, id: &str) -> Flow<bool>;

    /// Reaction window at `t` (C# `React(trigger)`); may raise prompts.
    ///
    /// `t` is mutable: a reaction may rewrite the move roll (`t.value` /
    /// `t.Move.Roll`) and the engine then uses the new face (C# shares the
    /// `MoveCtx` with the reactions).
    fn react(&self, _cx: &mut Cx, _t: &mut Trigger) -> Flow<()> {
        Ok(())
    }

    /// Short note shown on a card in hand (`HandNotesOf`).
    fn hand_note(&self, _cx: &Cx, _seat: usize, _card: &str) -> Msg {
        Msg::default()
    }
}

/// Every card and event has no effect yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubRules;

impl CardRules for StubRules {
    fn play(&self, cx: &mut Cx, seat: usize, card: &str) -> Flow<Dest> {
        cx.log(seat as i32, Msg::new("log.card_not_ported").card("card", card));
        Ok(Dest::Graveyard)
    }

    fn event(&self, cx: &mut Cx, seat: usize, id: &str) -> Flow<bool> {
        cx.log(seat as i32, Msg::new("log.event_not_ported").event("event", id));
        Ok(false)
    }
}
