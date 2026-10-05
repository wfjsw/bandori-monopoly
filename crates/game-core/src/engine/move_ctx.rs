//! `MatchHost.MoveCtx` -- the live state of one movement routine.
//!
//! Runtime-only: it holds roll tables and (in the C#) closures the wire never
//! carries. The broadcast summary the UI reads is [`MovePlan`]
//! ([`MoveCtx::to_plan`]), which is what `MatchState::plan` holds.

use crate::msg::Msg;
use crate::state::MovePlan;

/// How the player gets there: exactly one per move (C# `MoveCtx.Teleport` vs the
/// walk loop). Mirrors `card_sdk::abi::MoveKind` bit-for-bit.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MoveKind {
    #[default]
    Walk,
    Teleport,
}

impl MoveKind {
    pub fn from_i32(v: i32) -> Option<Self> {
        match v {
            0 => Some(Self::Walk),
            1 => Some(Self::Teleport),
            _ => None,
        }
    }
}

// Settling is not its own axis beside [`MoveKind`]: the kind already says
// *where* resolution happens -- a walk resolves the route it travels and its
// landing, a teleport only its destination. `MoveCtx::resolve` (C#
// `m.Resolve`) is the one remaining boolean: *whether* the move settles where
// it lands, and a card effect can clear it to prevent settle at all.

/// One term the movement rolls into its total (`MoveCtx.Base` / `Dice`).
///
/// `count`d`sides`, summed. **`sides == 0` is a flat `count`** -- that is how a
/// 「+2 to the roll」 effect is expressed, instead of a separate `bonus` field;
/// the roll total is just the sum of the terms. `why` is the log's parenthetical
/// for this term.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Roll {
    pub count: i32,
    pub sides: i32,
    pub why: String,
}

impl Roll {
    /// `count`d`sides` (or a flat `count` when `sides == 0`).
    pub fn new(count: i32, sides: i32, why: impl Into<String>) -> Self {
        Self { count, sides, why: why.into() }
    }

    /// The value this term contributes.
    pub fn value(&self, faces: &[i32]) -> i32 {
        if self.sides <= 0 {
            self.count
        } else {
            faces.iter().sum()
        }
    }
}

/// `MoveCtx` -- the live state of one movement, from plan to landing.
///
/// This is the routine's working state, not a mirror of the C# class: it holds
/// what the walk loop and the settle step actually read, plus what a card needs
/// to shape the move. The broadcast summary the UI reads is [`MovePlan`]
/// ([`MoveCtx::to_plan`]), which is what `MatchState::plan` holds.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveCtx {
    // -- who ---------------------------------------------------------------
    pub player_id: usize,
    /// Who rolled, when that is not the player (shown on the log line).
    pub roller: usize,
    /// The turn's [main move](C# `Main`), as opposed to a card's side move.
    pub main: bool,
    /// Why the move happened (a card / effect), shown in parentheses on the
    /// log line.
    pub why: Option<Msg>,

    // -- what kind of move ---------------------------------------------------
    /// How the player gets there: one walk or one teleport (C# `m.Teleport`).
    pub kind: MoveKind,
    /// The teleport's destination, or `-1` (a teleport with no named
    /// destination derives it from the roll).
    pub teleport_to: i32,
    /// Does the move settle where it lands (`settleBefore` -> `settle` -> `land`
    /// -> `settleAfter`)? Clear it to prevent settle at all -- the player still
    /// moves. A walk still [经过]-resolves the tiles it travels; a teleport with
    /// this clear resolves nothing.
    pub resolve: bool,

    // -- the dice ------------------------------------------------------------
    /// The dice the roll starts from (default 1d20).
    pub base: Vec<Roll>,
    /// Extra dice added by effects (summed into the roll).
    pub dice: Vec<Roll>,
    /// The face the walk uses, after the reactions have rewritten it.
    pub roll: i32,
    /// Floor applied to the final face (only when the roll is unsigned).
    pub min_roll: i32,
    /// A negative `roll` walks backwards instead of clamping.
    pub signed: bool,

    // -- the walk ------------------------------------------------------------
    /// Planned length, or `-1` before the roll.
    pub steps: i32,
    /// Steps added mid-walk (extends the walk as it runs).
    pub extra_steps: i32,
    pub from: i32,
    pub to: i32,
    /// The tile the walk begins on, or `-1` for the player's own tile.
    pub start: i32,
    pub start_why: String,
    /// The walk goes backwards (`Dir` is -1).
    pub reverse: bool,
    /// The tiles the walk visits, in order.
    pub path: Vec<i32>,
    pub total: i32,
    pub remaining: i32,
    /// Players standing on tiles the walk passed.
    pub passed_players: Vec<i32>,
    /// Forced stop tile, or `-1`.
    pub stop_at: i32,
    /// Set mid-walk to stop here and settle (the 「强制停下」 cards).
    pub stopped: bool,
    /// Restrict the walk to odd/even tiles (`-1` = either).
    pub parity: i32,

    // -- settlement -----------------------------------------------------------
    /// Settle at this tile instead of the landing (`-1` = the landing). Lets a
    /// walk settle at a tile it passes without stopping there.
    pub settle_tile: i32,
    /// Scale money paid for this walk. Milli-units (500 = x0.5): the cards scope
    /// a payment modifier to *this* move (「此次支付的分摊前资金减少…」), so it
    /// lives on the move rather than on the payment step.
    pub pay_factor: f64,
    /// Scale rent paid for this walk, same units.
    pub rent_factor: f64,
    /// Settle on another player's behalf.
    pub settle_as_agent: bool,
    /// May build away from the landing (not just on it). Distinct from
    /// `no_build`, which denies building at all.
    pub build_anywhere: bool,
    /// A queued second walk, in steps (the C# `MoreSteps` follow-up phase).
    pub more_steps: i32,
    /// Passing CiRCLE pays nothing on this walk.
    pub no_circle_reward: bool,
    /// The landing cannot be bought (「该次传送不可进行地契购买」).
    pub no_buy: bool,
    /// The landing cannot be built on.
    pub no_build: bool,

    // -- reactions ------------------------------------------------------------
    /// A reaction cancelled the movement.
    pub cancelled: bool,
    /// Card-owned per-move state, keyed by name (C# held fire-roll counters on
    /// the engine; the cards track that themselves now). The engine never reads
    /// it -- it is scratch space for the card that armed the effect.
    pub tags: Vec<(String, i32)>,
}

impl Default for MoveCtx {
    /// The sentinels a derived `Default` would get wrong: `Resolve = true`,
    /// `TeleportTo/Start/StopAt/Parity/Steps = -1`, `Base = 1d20`. Getting these
    /// wrong is silent -- the walk just never settles, or a teleport goes to
    /// tile 0.
    fn default() -> Self {
        Self {
            player_id: 0,
            roller: 0,
            main: false,
            why: None,
            kind: MoveKind::Walk,
            teleport_to: -1,
            resolve: true,
            base: vec![Roll { count: 1, sides: 20, why: String::new() }],
            dice: Vec::new(),
            roll: 0,
            min_roll: 0,
            signed: false,
            steps: -1,
            extra_steps: 0,
            from: 0,
            to: 0,
            start: -1,
            start_why: String::new(),
            reverse: false,
            path: Vec::new(),
            total: 0,
            remaining: 0,
            passed_players: Vec::new(),
            stop_at: -1,
            stopped: false,
            parity: -1,
            settle_tile: -1,
            pay_factor: 1.0,
            rent_factor: 1.0,
            settle_as_agent: false,
            build_anywhere: false,
            more_steps: 0,
            no_circle_reward: false,
            no_buy: false,
            no_build: false,
            cancelled: false,
            tags: Vec::new(),
        }
    }
}

impl MoveCtx {
    /// A fresh walk for `player_id` (C# `new MoveCtx { Player = player_id }`).
    pub fn new(player_id: usize) -> Self {
        Self { player_id, roller: player_id, ..Default::default() }
    }

    /// `SetSteps` -- set the walk length, keeping the sign of the current roll
    /// so a reverse move stays reverse.
    pub fn set_steps(&mut self, n: i32) {
        let n = n.max(0);
        self.roll = if self.roll < 0 { -n } else { n };
    }

    /// `Dir` -- +1 forwards, -1 backwards.
    pub fn dir(&self) -> i32 {
        if self.reverse { -1 } else { 1 }
    }

    /// `WalkDir` -- the direction the current roll actually walks (a negative
    /// roll flips `Dir`).
    pub fn walk_dir(&self) -> i32 {
        if self.roll >= 0 { self.dir() } else { -self.dir() }
    }

    /// `SetTag` -- write card-owned per-move state.
    pub fn set_tag(&mut self, key: &str, value: i32) {
        match self.tags.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value,
            None => self.tags.push((key.to_string(), value)),
        }
    }

    /// `Tag` -- read card-owned per-move state (0 when unset).
    pub fn tag(&self, key: &str) -> i32 {
        self.tags.iter().find(|(k, _)| k == key).map_or(0, |(_, v)| *v)
    }

    /// `MovePlan` -- the broadcast summary of this walk (`State.plan`).
    ///
    /// `reach` is the path the walk visits; `steps` is how far along it the
    /// player has got, and `landing()` is where it ends.
    pub fn to_plan(&self) -> MovePlan {
        MovePlan {
            player_id: self.player_id as i32,
            from: self.from,
            steps: (self.total - self.remaining).max(0),
            started: self.start >= 0,
            reach: self.path.clone(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::MoveCtx;

    #[test]
    fn set_steps_keeps_the_sign_of_a_reverse_walk() {
        let mut m = MoveCtx { roll: -6, ..Default::default() };
        m.set_steps(4);
        assert_eq!(m.roll, -4, "a reverse roll stays negative");
        m.roll = 6;
        m.set_steps(0);
        assert_eq!(m.roll, 0);
    }

    #[test]
    fn walk_dir_flips_for_a_negative_roll() {
        let m = MoveCtx { roll: -3, ..Default::default() };
        assert_eq!(m.dir(), 1);
        assert_eq!(m.walk_dir(), -1, "a negative roll walks backwards");
    }

    #[test]
    fn plan_projects_the_walk() {
        let m = MoveCtx {
            player_id: 2,
            from: 7,
            start: 7,
            total: 5,
            remaining: 2,
            path: vec![8, 9, 10, 11, 12],
            ..Default::default()
        };
        let p = m.to_plan();
        assert_eq!(p.player_id, 2);
        assert_eq!(p.from, 7);
        assert_eq!(p.steps, 3);
        assert!(p.started);
        assert_eq!(p.landing(), 10, "reach[steps-1]");
    }
}