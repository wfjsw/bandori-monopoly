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

// `MoveFlags` is gone: a move is exactly one [`super::move_ctx::MoveKind`] and
// what it resolves is `super::move_ctx::Settle` -- two categories, not one flag
// soup. Card-owned move state (a [火罐] roll, say) rides on the move's tags.

/// Points in the flow where reaction cards may answer (C# `Trigger.Kind`).
///
/// The kind name itself encodes pre/post (`"settleBefore"` vs `"settle"`,
/// `"buyBefore"` vs `"buyAfter"`), so there is no separate `when` field.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub kind: &'static str,
    pub player_id: i32,
    pub target: i32,
    pub tile: i32,
    pub card: String,
    pub value: i32,
    /// The turn step (0/1/2/3) active when this trigger fired. Stamped by
    /// `Cx::react`, not by the raise site. Only meaningful for kinds that
    /// aren't step-specific (e.g. `mortgage` can fire during both step 1
    /// and step 3).
    pub step: i32,
    /// The player whose card caused this trigger (C# `t.ByCard`), or `None` when
    /// it was not card-caused (board-driven: rent, buy, build, turn flow). Set
    /// at the raise site from the card's own player. `Some(by) if by != player_id` is
    /// C# `H.HitByOtherCard(t, seat)`.
    pub by_card: Option<i32>,
    /// `t.Pay.IsRent` on `pay`/`paid` triggers -- is this payment rent (C# `t.Pay.kind == "rent"`).
    pub pay_is_rent: bool,
    /// `t.Move` -- how the move that caused this trigger got there (C#
    /// `m.Teleport`); `None` when the move did not cause it.
    pub move_kind: Option<super::move_ctx::MoveKind>,
    /// `t.Move.Resolve` -- does that move settle where it lands?
    pub move_resolve: bool,
    /// `t.Move.Tags` -- free-form per-card counters on that move (a [火罐] roll
    /// is card-owned state, tagged by whoever armed it).
    pub move_tags: Vec<(String, i32)>,
    /// `t.Move.Main` -- was this the turn's main move (C# `MoveCtx.main`)?
    pub move_main: bool,
    /// `t.Move.Dir` -- 1 forward, -1 backward. Only meaningful when `move_flags.is_move()`.
    pub move_dir: i32,
    /// `Trigger.Cancelled` -- a reaction negated this trigger's effect. The
    /// effect body is skipped; the Before/After hooks still fire.
    pub cancelled: bool,
    /// `t.Move.Remaining` -- steps the move has left to walk.
    pub move_remaining: i32,
    /// `t.Move.Total` -- the move's path length (C# `m.Path.Count`).
    pub move_total: i32,
    /// The cards the trigger is about when there are several (`drew`).
    pub cards: Vec<String>,
}

impl Trigger {
    pub fn new(kind: &'static str, player_id: usize) -> Self {
        Self {
            kind,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            card: String::new(),
            value: 0,
            step: 0,
            by_card: None,
            pay_is_rent: false,
            move_kind: None,
            move_resolve: false,
            move_tags: Vec::new(),
            move_main: false,
            move_dir: 1,
            cancelled: false,
            move_remaining: 0,
            move_total: 0,
            cards: Vec::new(),
        }
    }

    /// Copy the move context (C# `t.Move`) onto this trigger.
    pub(crate) fn with_move(&mut self, m: &super::play::Move) -> &mut Self {
        self.move_kind = Some(m.kind);
        self.move_resolve = m.resolve;
        self.move_tags = m.tags.clone();
        self.move_main = m.main;
        self.move_dir = m.dir();
        self.move_remaining = m.remaining;
        self.move_total = m.total;
        self
    }
}

/// Raise a reaction trigger: build a [`Trigger`], apply the optional field
/// overrides, and run it through `Cx::raise` (which stamps `step` centrally and
/// delegates to the card rules). Every raise site should go through this so the
/// shape of a trigger stays uniform.
///
/// `mv = <move>` copies the move context (C# `t.Move`) on from a `Move`, and
/// must come before any other field overrides.
///
/// The result is the trigger as `raise` left it, so a reaction that rewrites a
/// field (e.g. `moveRoll`'s reroll) can read it back:
///
/// ```ignore
/// raise!(self, "passBefore", i, @m m, tile = next)?;
/// raise!(self, "roll", i, value = -1)?;
/// raise!(self, "card", i, card = id.to_string())?;
/// let t = raise!(self, "moveRoll", i, @m m, value = m.roll)?; // keep `t` to read back
/// ```
macro_rules! raise {
    ($cx:expr, $kind:expr, $player_id:expr $(, @m $m:expr)? $(, $field:ident = $value:expr)* $(,)?) => {{
        let mut __trigger = Trigger::new($kind, $player_id);
        $( __trigger.with_move(&$m); )?
        $( __trigger.$field = $value; )*
        $cx.raise(__trigger)
    }};
}
pub(crate) use raise;

pub trait CardRules: Send + Sync {
    /// `Card.Normal` -- may be played from hand in the operation phase.
    fn normal(&self, _card: &str) -> bool {
        true
    }

    /// `Card.WhyNot` -- extra card-specific reason it can't be played right now.
    fn cant_play(&self, _cx: &Cx, _player: usize, _card: &str) -> Option<Msg> {
        None
    }

    /// `Card.AiPlay` -- would a bot play it now?
    fn ai_play(&self, _cx: &Cx, _player: usize, _card: &str) -> bool {
        true
    }

    /// `Card.Play` -- the effect. Returns where the card goes afterwards.
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest>;

    /// Resolve a drawn event card. Returns `true` if the event stays in play
    /// (otherwise it is discarded).
    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool>;

    /// Reaction window at `t` (C# `React(trigger)`); may raise prompts.
    ///
    /// `t` is mutable: a reaction may rewrite the move roll (`t.value` /
    /// `t.Move.Roll`) and the engine then uses the new face (C# shares the
    /// `MoveCtx` with the reactions).
    fn react(&self, _cx: &mut Cx, _t: &mut Trigger) -> Flow<()> {
        Ok(())
    }

    /// Short note shown on a card in hand (`HandNotesOf`).
    fn hand_note(&self, _cx: &Cx, _player: usize, _card: &str) -> Msg {
        Msg::default()
    }
}

/// Every card and event has no effect yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubRules;

impl CardRules for StubRules {
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest> {
        cx.log(player_id as i32, Msg::new("log.card_not_ported").card("card", card));
        Ok(Dest::Graveyard)
    }

    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool> {
        cx.log(player_id as i32, Msg::new("log.event_not_ported").event("event", id));
        Ok(false)
    }
}
