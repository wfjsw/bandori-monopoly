//! The live state of one movement routine.
//!
//! Runtime-only: it holds the roll tables the wire never carries. The broadcast
//! summary the UI reads is [`MovePlan`] ([`MoveCtx::to_plan`]), which is what
//! `MatchState::plan` holds.

use serde::{Deserialize, Serialize};

use game_kinds::move_plan::{is_staging, PlanField};
use game_kinds::{Dice, Seat, Tile};

use crate::msg::Msg;
use crate::state::MovePlan;

// `MoveKind` lives in `game-kinds` -- one definition shared with the guest
// (`card-sdk`) and the host. Re-exported here so engine call sites keep their
// path.
pub use game_kinds::MoveKind;
// `PlanKind` likewise: the guest names the shape of a frame, the engine stores
// it.
pub use game_kinds::move_plan::PlanKind;

// Settling is not its own axis beside [`MoveKind`]: the kind already says
// *where* resolution happens -- a walk resolves the route it travels and its
// landing, a teleport only its destination. [`MoveCtx::settles`] is the one
// remaining boolean: *whether* the move settles where it lands, and a card
// effect can clear it to prevent settle at all.

/// One term the movement rolls into its total (the `base` / `dice` tables).
///
/// `dice.count`d`dice.sides`, summed. **`sides == 0` is a flat `count`** --
/// that is how a 「+2 to the roll」 effect is expressed, instead of a separate
/// `bonus` field; the roll total is just the sum of the terms. `reason` is the
/// log's parenthetical for this term.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Roll {
    pub dice: Dice,
    pub reason: String,
}

impl Roll {
    /// `count`d`sides` (or a flat `count` when `sides == 0`).
    pub fn new(count: i32, sides: i32, reason: impl Into<String>) -> Self {
        Self {
            dice: Dice { count, sides },
            reason: reason.into(),
        }
    }

    /// `count`d`sides` (or a flat `count` when `sides == 0`).
    pub fn from_dice(dice: Dice, reason: impl Into<String>) -> Self {
        Self {
            dice,
            reason: reason.into(),
        }
    }

    pub fn count(&self) -> i32 {
        self.dice.count
    }

    pub fn sides(&self) -> i32 {
        self.dice.sides
    }

    /// The value this term contributes.
    pub fn value(&self, faces: &[i32]) -> i32 {
        if self.dice.sides <= 0 {
            self.dice.count
        } else {
            faces.iter().sum()
        }
    }
}

/// `MoveCtx` -- the live state of one movement, from plan to landing.
///
/// This is the routine's working state: it holds what the walk loop and the
/// settle step actually read, plus what a card needs to shape the move. The
/// broadcast summary the UI reads is [`MovePlan`] ([`MoveCtx::to_plan`]),
/// which is what `MatchState::plan` holds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MoveCtx {
    // -- who ---------------------------------------------------------------
    pub player_id: usize,
    /// Who rolled, when that is not the player (shown on the log line).
    /// `None` until someone claims it -- `run_planned_move` then defaults it to the
    /// mover, and a substitution (幻觉来了 「上一名玩家代替进行此次投掷」)
    /// writes the stand-in through `plan::set_roller`.
    pub roller: Option<Seat>,
    /// The turn's main move, as opposed to a card's side move.
    pub is_main_move: bool,
    /// An event-driven forced move (「移动X」 / 「移动1d20」): not a main move, so
    /// it goes through even when the turn's main move is already spent
    /// (`docs/EVENTS.md`). Set by the host when the running rule is `event:*`
    /// or by a card (`plan::set_forced`).
    pub forced: bool,
    /// This forced move is main anyway (「视为你的主要移动」): `plan::
    /// set_main_override`. [`is_main_move`](game_kinds::move_plan::is_main_move)
    /// reads it.
    #[serde(default)]
    pub main_override: bool,
    /// The move was shaped or started by a rule whose card id starts with
    /// `skill:` (generic source tag; cond reads `move.by_skill`).
    #[serde(default)]
    pub by_skill: bool,
    /// Why the move happened (a card / effect), shown in parentheses on the
    /// log line.
    pub reason: Option<Msg>,

    // -- what kind of move ---------------------------------------------------
    /// How the player gets there: one walk or one teleport.
    pub kind: MoveKind,
    /// The teleport's destination. `None` = a teleport with no named
    /// destination derives it from the roll.
    pub teleport_to: Option<Tile>,
    /// This teleport replaces the pending walk instead of following it
    /// (`plan::void_walk`). With `main_override`, inside a move's own roll
    /// window, the next `run_planned_move` replaces the walk in place.
    #[serde(default)]
    pub void_walk: bool,
    /// Does the move settle where it lands (`settleBefore` -> `settle` -> `land`
    /// -> `settleAfter`)? Clear it to prevent settle at all -- the player still
    /// moves. 其他规则注意事项 1.2: 「是否settle」 gates **only** the settle
    /// stages (14–15). A walk still [pass]-resolves the tiles it travels, and
    /// (R2 / M6a, `SETTLE-STAGES.md` §6) a teleport still raises `passTile` +
    /// `passPlayer` at its destination -- 专名词 9 gives the teleport a [路径]
    /// of just the endpoint, and B41 raises [pass],overlap there before the
    /// (gated) settle. The movement tail (`moveAfter` / `moveResolved`) runs
    /// either way.
    pub settles: bool,

    // -- the dice ------------------------------------------------------------
    /// The dice the roll starts from (default 1d20).
    pub base: Vec<Roll>,
    /// Extra dice added by effects (summed into the roll).
    pub dice: Vec<Roll>,
    /// The face the walk uses, after the counteractions have rewritten it.
    pub roll: i32,
    /// A die was cast for this walk (a `steps` walk and a teleport are `false`;
    /// a `next_steps` override still casts, so it stays `true`). The charity
    /// condition and `trigger::move_rolled` read it.
    #[serde(default)]
    pub rolled: bool,
    /// Floor applied to the final face (only when the roll is unsigned).
    /// `None` = no floor.
    pub min_roll: Option<i32>,
    /// A negative `roll` walks backwards instead of clamping.
    pub signed: bool,

    // -- the walk ------------------------------------------------------------
    /// Planned length. `None` before the roll (the roll sets it).
    pub steps: Option<i32>,
    /// The main move's stored override, copied from `hidden[i]` at a Main
    /// frame seed and never a guest write; the staging plan leaves it `None`.
    #[serde(default)]
    pub next_steps: Option<i32>,
    /// Steps added mid-walk (extends the walk as it runs).
    pub extra_steps: i32,
    pub from: i32,
    pub to: i32,
    /// The tile the walk begins on. `None` = the player's own tile.
    pub start: Option<Tile>,
    pub start_reason: String,
    /// The walk goes backwards ([`Self::dir`] is -1).
    pub reverse: bool,
    /// The tiles the walk visits, in order.
    pub path: Vec<i32>,
    pub total: i32,
    pub remaining: i32,
    /// Players standing on tiles the walk passed.
    pub passed_players: Vec<i32>,
    /// Forced stop tile. `None` = no forced stop.
    pub stop_at: Option<Tile>,
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
    // A suspended body is re-executed from its start on the resume (STACK-01)
    // and would compose its factor twice; `TurnCtx::factor_applied` keeps the
    // writes that already landed so the replay's copy is skipped.
    /// Settle on another player's behalf.
    pub settle_as_agent: bool,
    /// This settle was initiated by a card (`settle_tile_at`), not by the
    /// player's own move landing. The engine sets it on the nested move
    /// `card_settle_move` builds; it is **never** a plan write (`plan::set_*`)
    /// and the plan's own flag remains default `false` so a later move cannot
    /// inherit it. Readable from a hook as
    /// `card_sdk::ctx::trigger::is_settle_redirect()`.
    #[serde(default)]
    pub settle_redirect: bool,
    /// An abnormal landed somewhere in this walk's span.
    #[serde(default)]
    pub abnormal: bool,
    /// Whether this move can build on the tile it lands on. `false` is the
    /// "no build" case (`ctx::plan::set_no_build` folds into this, HOST-08) --
    /// one flag, not two.
    pub can_build: bool,
    /// Whether this move may build away from the landing
    /// (`ctx::plan::set_build_anywhere`, HOST-08). Was: lifts the END-step
    /// build's "only where the move settled" check. The END-step build is
    /// gone (settle-only buy/build); kept for plan/record shape.
    pub build_anywhere: bool,
    /// A queued second walk, in steps (the follow-up walk once the first
    /// settles).
    pub more_steps: i32,
    /// Passing the start square pays nothing on this walk.
    /// The landing cannot be bought (「该次传送不可进行地契购买」).
    pub no_buy: bool,

    // -- counteractions ------------------------------------------------------------
    /// A counteraction cancelled the movement.
    pub cancelled: bool,
    /// A card has claimed this move at the pre-dice pass; later offers at the
    /// same pass stand down. One-shot: it lives on the move's plan frame and
    /// dies with it (`World::end_plan`).
    #[serde(default)]
    pub claimed: bool,
    /// Card-owned per-move state, keyed by name. The engine never reads it --
    /// it is scratch space for the card that armed the effect (e.g. a charge-roll
    /// counter the card tracks itself).
    pub tags: Vec<(String, i32)>,
    /// 「使你的下次主要移动结果对那些玩家一起执行」 -- players who replay this
    /// move's result after the mover settles. The engine walks each one the
    /// same way, in the order recorded here (「你先触发结算，此后其他玩家按
    /// 行动顺序依次触发结算」), carrying the same plan (steps / kind /
    /// destination / `pay_factor`).
    pub followers: Vec<i32>,
}

/// One live plan frame: the plan a move runs on, the staging it restores, and
/// who owns it. Pushed by `World::begin_plan`, popped by `World::end_plan`
/// (`spec/abi/G04-move-plan.md` §3.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanFrame {
    pub plan: MoveCtx,
    /// The staging plan at the command's entry; the restore target of a side
    /// or refused frame.
    pub saved: MoveCtx,
    pub kind: PlanKind,
    pub owner: usize,
}

/// One activation body's plan scope.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanScope {
    pub commands: Vec<PlanCommand>,
    pub moves: u32,
    /// The frame contents when the current roll window opened, innermost last.
    #[serde(default)]
    pub window_before: Vec<MoveCtx>,
}

/// The snapshot a command scope takes at its entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanCommand {
    pub staging_before: MoveCtx,
    /// The plan target when the body entered (a frame clone, or staging).
    pub plan_before: MoveCtx,
    pub moves_at_entry: u32,
}

impl Default for MoveCtx {
    /// `Option` / `bool` defaults are correct for every sentinel field. The
    /// remaining non-default bits: `settles = true`, `base = 1d20`,
    /// `pay_factor` / `rent_factor = 1.0`, `can_build = true`.
    fn default() -> Self {
        Self {
            player_id: 0,
            roller: None,
            is_main_move: false,
            forced: false,
            main_override: false,
            by_skill: false,
            reason: None,
            kind: MoveKind::Walk,
            teleport_to: None,
            void_walk: false,
            settles: true,
            base: vec![Roll {
                dice: Dice {
                    count: 1,
                    sides: 20,
                },
                reason: String::new(),
            }],
            dice: Vec::new(),
            roll: 0,
            rolled: false,
            min_roll: None,
            signed: false,
            steps: None,
            next_steps: None,
            extra_steps: 0,
            from: 0,
            to: 0,
            start: None,
            start_reason: String::new(),
            reverse: false,
            path: Vec::new(),
            total: 0,
            remaining: 0,
            passed_players: Vec::new(),
            stop_at: None,
            stopped: false,
            parity: -1,
            settle_tile: -1,
            pay_factor: 1.0,
            rent_factor: 1.0,
            settle_as_agent: false,
            settle_redirect: false,
            abnormal: false,
            can_build: true,
            build_anywhere: false,
            more_steps: 0,
            no_buy: false,
            cancelled: false,
            claimed: false,
            tags: Vec::new(),
            followers: Vec::new(),
        }
    }
}

/// Generate the per-field copy / compare pair over [`PlanField`]. The match is
/// exhaustive, so a new `PlanField` variant is a compile error here.
macro_rules! plan_field_ops {
    ($($variant:ident.$field:ident),* $(,)?) => {
        fn assign(dst: &mut MoveCtx, src: &MoveCtx, f: PlanField) {
            match f {
                $(PlanField::$variant => dst.$field = src.$field.clone()),*
            }
        }
        fn same(a: &MoveCtx, b: &MoveCtx, f: PlanField) -> bool {
            match f {
                $(PlanField::$variant => a.$field == b.$field),*
            }
        }
    };
}

plan_field_ops! {
    Steps.steps,
    Reverse.reverse,
    Parity.parity,
    Kind.kind,
    TeleportTo.teleport_to,
    Start.start,
    StartReason.start_reason,
    StopAt.stop_at,
    SettleTile.settle_tile,
    SettleRedirect.settle_redirect,
    Resolve.settles,
    PayFactor.pay_factor,
    RentFactor.rent_factor,
    Followers.followers,
    ExtraSteps.extra_steps,
    NextSteps.next_steps,
    MinimumRoll.min_roll,
    Signed.signed,
    Base.base,
    Dice.dice,
    MoreSteps.more_steps,
    NoBuy.no_buy,
    CanBuild.can_build,
    BuildAnywhere.build_anywhere,
    SettleAsAgent.settle_as_agent,
    Tags.tags,
    BySkill.by_skill,
    Forced.forced,
    MainOverride.main_override,
    VoidWalk.void_walk,
    Rolled.rolled,
    Reason.reason,
    Roller.roller,
    Cancelled.cancelled,
    IsMainMove.is_main_move,
    From.from,
    To.to,
    Path.path,
    Total.total,
    Remaining.remaining,
    PassedPlayers.passed_players,
    Stopped.stopped,
}

impl MoveCtx {
    /// A fresh walk for `player_id`.
    pub fn new(player_id: usize) -> Self {
        Self {
            player_id,
            roller: Some(Seat(player_id as i32)),
            ..Default::default()
        }
    }

    /// Fix the walk length, so the move walks exactly `n` steps and does not
    /// roll (a `Some` `steps` suppresses the roll; the walk then reads `steps`
    /// instead of `roll`). The sign of the current roll is kept so a reverse
    /// move remains reverse.
    pub fn set_steps(&mut self, n: i32) {
        let n = n.max(0);
        self.steps = Some(n);
        self.roll = if self.roll < 0 { -n } else { n };
    }

    /// The length this move actually takes: a fixed count, else the stored
    /// `next_steps` count, else the face.
    pub fn effective_steps(&self) -> i32 {
        self.steps.or(self.next_steps).unwrap_or(self.roll)
    }

    /// Build the seed of a side frame: every `Command` field the command
    /// changed relative to `plan_before`, and every `Turn` / `NextMainMove`
    /// field from `staging` (those stay for the next main). Iterates
    /// [`PlanField`] and consults [`is_staging`], so a new field cannot be
    /// forgotten.
    ///
    /// A field the caller itself changed (relative to `plan_before`) always
    /// keeps the caller's value. `target` is the body's captured plan -- the
    /// copy its plan writes land on while it is paused -- and the live
    /// `staging` does not see those, so letting `staging` win would drop a
    /// teleport / steps / kind the body set right before the call
    /// (`G04-move-plan.md` §3.2).
    pub fn staging_seed(plan_before: &MoveCtx, staging: &MoveCtx, target: &MoveCtx) -> MoveCtx {
        let mut seed = target.clone();
        for &f in PlanField::ALL {
            if same(plan_before, target, f) && (is_staging(f) || !same(plan_before, staging, f)) {
                assign(&mut seed, staging, f);
            }
        }
        seed
    }

    /// +1 forwards, -1 backwards.
    pub fn dir(&self) -> i32 {
        if self.reverse {
            -1
        } else {
            1
        }
    }

    /// The direction the current roll actually walks (a negative roll flips
    /// [`Self::dir`]).
    pub fn walk_dir(&self) -> i32 {
        if self.roll >= 0 {
            self.dir()
        } else {
            -self.dir()
        }
    }

    /// Write card-owned per-move state.
    pub fn set_tag(&mut self, key: &str, value: i32) {
        match self.tags.iter_mut().find(|(k, _)| k == key) {
            Some(e) => e.1 = value,
            None => self.tags.push((key.to_string(), value)),
        }
    }

    /// Read card-owned per-move state (0 when unset).
    pub fn tag(&self, key: &str) -> i32 {
        self.tags
            .iter()
            .find(|(k, _)| k == key)
            .map_or(0, |(_, v)| *v)
    }

    /// The broadcast summary of this walk (`MatchState::plan`).
    ///
    /// `reach` is the path the walk visits; `steps` is how far along it the
    /// player has got, and `landing()` is where it ends.
    pub fn to_plan(&self) -> MovePlan {
        MovePlan {
            player_id: self.player_id as i32,
            from: self.from,
            steps: (self.total - self.remaining).max(0),
            started: self.start.is_some(),
            reach: self.path.clone(),
            can_build: self.can_build,
            build_anywhere: self.build_anywhere,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MoveCtx, PlanKind};
    use crate::engine::world::World;
    use crate::rng::Rng;
    use crate::state::MatchState;
    use game_kinds::Tile;

    /// A bare two-seat world: enough for the frame stack and the staging
    /// buffer, with no board or match flow.
    fn bare_world() -> World {
        World::new(MatchState::default(), 2, Rng::new(1))
    }

    /// G04 §3.2: a side command that drove a move restores the staging it found
    /// (its `steps` / `reverse` / `pay_factor` are gone at command end), while
    /// the move itself ran on the command's shape.
    #[test]
    fn g04_side_move_restores_staging() {
        let mut w = bare_world();
        w.enter_plan_command(false);
        w.plan_mut().set_steps(5);
        w.plan_mut().reverse = true;
        w.plan_mut().pay_factor = 0.5;
        let target = w.plan().clone();
        w.begin_plan(0, PlanKind::SideMove, target);
        w.turn.plan_scope.moves += 1;
        // The move sees the command's shape...
        assert_eq!(w.plan().steps, Some(5));
        assert!(w.plan().reverse);
        assert_eq!(w.plan().pay_factor, 0.5);
        w.end_plan(true);
        w.leave_plan_command();
        // ...and the command's writes are gone with it.
        assert!(w.turn.plan.steps.is_none(), "steps restored");
        assert!(!w.turn.plan.reverse, "reverse restored");
        assert_eq!(w.turn.plan.pay_factor, 1.0, "factor restored");
        assert!(w.turn.plan_stack.is_empty());
        assert!(w.turn.plan_scope.commands.is_empty());
    }

    /// G04 §3.2 restore table: a Main frame that ran consumes the staging (the
    /// next move starts clean) and latches nothing back.
    #[test]
    fn g04_main_consumes_staging() {
        let mut w = bare_world();
        w.turn.plan.set_steps(3);
        w.set_teleport_to(Some(Tile(4)));
        let target = w.turn.plan.clone();
        w.begin_plan(0, PlanKind::MainMove, target);
        assert_eq!(w.plan().steps, Some(3), "the Main seed takes the staging");
        w.end_plan(true);
        assert_eq!(
            w.turn.plan,
            MoveCtx::default(),
            "a ran Main move empties staging"
        );
        assert!(w.turn.plan_stack.is_empty());
    }

    /// G04 R12: a body raised inside a body pushes its own `PlanCommand`; the
    /// nested leave touches nothing and the outer snapshot survives intact.
    #[test]
    fn g04_nested_frames_restore_in_order() {
        let mut w = bare_world();
        w.enter_plan_command(false);
        w.plan_mut().set_steps(2);
        let target = w.plan().clone();
        w.begin_plan(0, PlanKind::SideMove, target);
        w.turn.plan_scope.moves += 1;
        w.plan_mut().reverse = true;
        let outer = w.turn.plan_scope.commands.last().unwrap().clone();

        // A nested body opens its own command and frame.
        w.enter_plan_command(false);
        assert_eq!(w.turn.plan_scope.commands.len(), 2);
        let nested = w.plan().clone();
        w.begin_plan(0, PlanKind::SideMove, nested);
        assert_eq!(
            w.turn.plan_scope.commands.last().unwrap().plan_before.steps,
            Some(2),
            "the nested snapshot is the outer frame's shape"
        );
        assert!(w.plan().reverse, "the nested seed carries the outer frame");
        w.plan_mut().set_steps(9);
        w.plan_mut().pay_factor = 0.25;
        w.end_plan(true);
        w.leave_plan_command();

        // The nested frame and command are gone; the outer snapshot is intact.
        assert_eq!(w.turn.plan_scope.commands.len(), 1);
        assert_eq!(
            *w.turn.plan_scope.commands.last().unwrap(),
            outer,
            "the outer snapshot is not overwritten"
        );
        assert_eq!(w.plan().steps, Some(2), "outer frame shape untouched");
        assert!(w.plan().reverse);
        assert_eq!(w.plan().pay_factor, 1.0);

        // The outer command ends: its (single) move drop restores staging.
        w.end_plan(true);
        w.leave_plan_command();
        assert!(w.turn.plan.steps.is_none());
        assert!(w.turn.plan_stack.is_empty());
        assert!(w.turn.plan_scope.commands.is_empty());
    }

    /// HOST-08: `set_no_build` folds into `can_build = !on`; `build_anywhere`
    /// is its own plan flag (landing-only when false).
    #[test]
    fn no_build_folds_into_can_build() {
        let mut w = crate::engine::world::World::new(
            crate::state::MatchState::default(),
            2,
            crate::rng::Rng::new(1),
        );
        w.set_no_build(true);
        assert!(
            !w.turn.plan.can_build,
            "set_no_build(true) => can_build=false"
        );
        w.set_no_build(false);
        assert!(
            w.turn.plan.can_build,
            "set_no_build(false) => can_build=true"
        );
        w.set_build_anywhere(true);
        assert!(w.turn.plan.build_anywhere);
        w.set_build_anywhere(false);
        assert!(!w.turn.plan.build_anywhere);
    }

    #[test]
    fn set_steps_keeps_the_sign_of_a_reverse_walk() {
        let mut m = MoveCtx {
            roll: -6,
            ..Default::default()
        };
        m.set_steps(4);
        assert_eq!(m.roll, -4, "a reverse roll remains negative");
        m.roll = 6;
        m.set_steps(0);
        assert_eq!(m.roll, 0);
    }

    #[test]
    fn walk_dir_flips_for_a_negative_roll() {
        let m = MoveCtx {
            roll: -3,
            ..Default::default()
        };
        assert_eq!(m.dir(), 1);
        assert_eq!(m.walk_dir(), -1, "a negative roll walks backwards");
    }

    #[test]
    fn plan_projects_the_walk() {
        let m = MoveCtx {
            player_id: 2,
            from: 7,
            start: Some(Tile(7)),
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
