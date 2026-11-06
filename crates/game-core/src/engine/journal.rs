//! The HOST-01 request journal (design G10 §6.3).
//!
//! A host request can tear: it runs against live, opens a prompt inside, and
//! the routine halts. The drive re-runs the body from the saved snapshot; the
//! request's own answer was logged at a cursor the resumed pass had already
//! walked past, so the re-run consumed the *nested prompt's* answer as its own
//! and the request's effect never ran.
//!
//! The journal records, per in-flight request, the exact pre-state and the
//! payload. The resumed body re-applies the request at its own site and only
//! then goes on. [`JournalRequest`] is the serializable mirror of
//! `game_rules::HostRequest` (that enum holds no client types but lives in the
//! rules crate, which `game-core` cannot see).

use super::cx::GuestOverlay;
use super::move_ctx::MoveCtx;
use super::world::World;
use crate::msg::Msg;
use std::collections::BTreeMap;

/// One in-flight (or just-completed) host request.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HostJournalEntry {
    /// The answer cursor this request owns. Dynamic: every own-answer
    /// insertion shifts the deeper entries by one.
    pub slot: usize,
    /// The request payload, replayed verbatim on resume.
    pub request: JournalRequest,
    pub card: String,
    pub uid: i32,
    /// The seat the request was issued for.
    pub player: i32,
    /// Exact world before the request.
    pub condition: Box<World>,
    /// The card's turn-policy writes (`build_discount`, `lingering`, ...) at
    /// the call, which the blind re-apply cannot read back from the body.
    pub linger: BTreeMap<String, i32>,
    /// The guest state the body had already cleared when it called.
    pub guest: GuestOverlay,
    /// The drive run this entry was applied in (see [`GuestLog::fresh`]).
    #[serde(default)]
    pub generation: u32,
    /// Every seat's money right after the effect landed. A re-run reuses the
    /// answer without re-applying, so the reuse puts this figure back on live
    /// -- money is absolute-ordered, and a `world_restore` that came after
    /// the body's own `adjust_money` must not be undone by it.
    #[serde(default)]
    pub after_money: Option<Vec<i32>>,
    /// The mover's position right after a `Move` landed. The re-run rewinds
    /// the position to the request's pre-state (a body that derives its
    /// destination from `player_pos` would otherwise walk another step every
    /// pass), and the reuse puts the landing back before the body reads it
    /// again.
    #[serde(default)]
    pub after_pos: Option<i32>,
}

/// How the apply relates to the journal. Set by the guest pause point,
/// harvested by the drive and passed to the apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JournalStart {
    /// A brand-new request: push a new entry at the current cursor.
    Push,
    /// An in-flight entry is being re-applied in place.
    Resume(usize),
    /// A completed entry whose arguments the body recomputed: replace it.
    Replace(usize),
}

/// The per-call guest answer log. It travels as one value because every part
/// must move together: the answers, the cursor, the journal, the reapply
/// counts, the completed rows and the replace caps all describe one body run.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GuestLog {
    pub answers: Vec<i32>,
    pub cursor: usize,
    /// The slot this body run starts every pass at. `0` for a drive of its
    /// own; a nested drive continues its parent's cursor. A body *resume*
    /// restarts here rather than at the pause cursor, so the requests the run
    /// already answered keep their slots.
    #[serde(default)]
    pub start: usize,
    pub journal: Vec<HostJournalEntry>,
    /// cursor -> outstanding re-applications (an enclosing and a nested entry
    /// can share a cursor).
    pub reapply: BTreeMap<usize, usize>,
    /// Completed requests, by their slot, for the recompute comparison.
    pub last: BTreeMap<usize, HostJournalEntry>,
    /// Slots whose request this drive already re-applied under recomputed
    /// arguments. The body re-derives the arguments from a world it just
    /// changed, so the recomputation can keep differing from what was applied;
    /// the mark makes the drive re-apply at most once per slot and consume from
    /// then on.
    #[serde(skip)]
    pub reapplied: std::collections::BTreeSet<usize>,
    /// Slots whose logged request was already re-applied in the current pass.
    /// A pass re-runs its body from the top, so the second and later
    /// encounters of a request inside one pass can reuse the live effect
    /// instead of applying it again. Cleared at every pass start; not part of
    /// a save.
    #[serde(skip)]
    pub applied: std::collections::BTreeSet<usize>,
    /// Which body run the entries in `last` belong to. A fresh drive bumps it;
    /// a body resume keeps it. A completed row from an earlier run is only a
    /// re-run of *this* body when the generations match -- otherwise the
    /// sibling drive's world has already moved on and re-winding it to that
    /// row's pre-state would undo work the new run no longer redoes.
    #[serde(skip)]
    pub fresh: u32,
}

impl GuestLog {
    /// Move every entry, mark and completed row at or after `from` up by one
    /// (design G10 §6.3.4). Runs when an answer is inserted at `from`: the
    /// live cursor sits past it afterwards, and a later request must land
    /// where the cursor is, not where it was.
    pub fn shift_slots(&mut self, from: usize) {
        for e in &mut self.journal {
            if e.slot >= from {
                e.slot += 1;
            }
        }
        self.reapply = shift_map(&self.reapply, from);
        self.reapplied = self
            .reapplied
            .iter()
            .map(|&s| if s >= from { s + 1 } else { s })
            .collect();
        self.last = shift_map(&self.last, from);
        if self.cursor >= from {
            self.cursor += 1;
        }
        // Strictly after: an answer landing exactly on the start slot is the
        // run's own first request (the outermost raise), and the run still
        // starts there. Only an enclosing request's answer, which always sits
        // before the run, pushes the origin along.
        if self.start > from {
            self.start += 1;
        }
    }

    /// Seed one outstanding re-application per in-flight entry, for the first
    /// drive start after a halt (design G10 §6.3.3 step 1).
    pub fn seed_reapply(&mut self) {
        self.reapply.clear();
        for e in &self.journal {
            *self.reapply.entry(e.slot).or_default() += 1;
        }
    }
}

fn shift_map<V>(m: &BTreeMap<usize, V>, from: usize) -> BTreeMap<usize, V>
where
    V: Clone,
{
    m.iter()
        .map(|(&k, v)| (if k >= from { k + 1 } else { k }, v.clone()))
        .collect()
}

/// A serializable mirror of `game_rules::HostRequest`. Every variant is
/// journaled: the request enum holds only work that touches the world, and any
/// such work can be followed by a prompt in a nested drive.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum JournalRequest {
    Pay {
        from: i32,
        to: i32,
        amount: i32,
        src: Option<Msg>,
        runs_total_stage: bool,
    },
    PayCommand {
        from: i32,
        to: i32,
        amount: i32,
        spec: i32,
        src: Option<Msg>,
    },
    PayTotal {
        from: i32,
        to: i32,
        amount: i32,
        src: Option<Msg>,
    },
    Gate {
        player_id: i32,
        kind: i32,
    },
    Target {
        player_id: i32,
        tile: i32,
        single_target: bool,
    },
    Move {
        player_id: i32,
        plan: MoveCtx,
    },
    Teleport {
        player_id: i32,
        tile: i32,
    },
    TeleportAs {
        player_id: i32,
        tile: i32,
        settles: bool,
        is_main_move: bool,
    },
    SettleAt {
        player_id: i32,
        tile: i32,
        is_main_move: bool,
    },
    JoinOpen {
        at: i32,
    },
    Buy {
        player_id: i32,
        tile: i32,
        kind: i32,
    },
    Acquire {
        player_id: i32,
        from: i32,
        tile: i32,
        price: i32,
    },
    AgentOffer {
        player_id: i32,
        agent: i32,
        tile: i32,
        kind: i32,
    },
    Linger {
        player_id: i32,
        expires: i32,
    },
    Build {
        player_id: i32,
        tile: i32,
    },
    OfferBuild {
        player_id: i32,
        tiles: Vec<i32>,
    },
    Mortgage {
        player_id: i32,
        tile: i32,
    },
    ApplyMortgage {
        player_id: i32,
        tile: i32,
        amount: i32,
    },
    ApplyRedeem {
        player_id: i32,
        tile: i32,
        amount: i32,
    },
    PayBuy {
        player_id: i32,
        tile: i32,
        amount: i32,
        kind: i32,
    },
    PayBuild {
        player_id: i32,
        tile: i32,
        amount: i32,
    },
    AssignDeed {
        player_id: i32,
        tile: i32,
        owner: i32,
        houses: i32,
        mortgaged: i32,
    },
    Draw {
        player_id: i32,
        n: i32,
    },
    DrawEvent {
        player_id: i32,
    },
    DrawEventAs {
        player_id: i32,
        tile: i32,
    },
    EventDefer {
        event: String,
        player_id: i32,
        tile: i32,
    },
    WorldSnapshot,
    WorldRestore {
        key: i32,
    },
    WorldSnapshotDrop {
        key: i32,
    },
    PayRent {
        player_id: i32,
        tile: i32,
        half: bool,
    },
    ChargeRent {
        player_id: i32,
        tile: i32,
        milli_scale: i32,
    },
    OfferBuy {
        player_id: i32,
        tile: i32,
    },
    OfferForceBuy {
        player_id: i32,
        tile: i32,
    },
    OfferBuildOne {
        player_id: i32,
        tile: i32,
    },
    Roll {
        player_id: i32,
        count: i32,
        sides: i32,
        purpose: i32,
        source: i32,
        window: bool,
    },
    RollRecast {
        roll_id: i32,
        source: i32,
    },
    RollMove {
        player_id: i32,
        source: i32,
    },
    RaiseBought {
        player_id: i32,
        tile: i32,
    },
    Raise {
        player_id: i32,
        kind: String,
        value: i32,
        tile: i32,
        card: Option<String>,
        deal_owner: i32,
        deal_houses: i32,
        deal_mortgaged: i32,
        charge_skill: bool,
    },
    AiAgentChoice {
        player_id: i32,
        tiles: Vec<i32>,
    },
    GainTyped {
        player_id: i32,
        amount: i32,
        typ: String,
        text: Option<Msg>,
    },
    AdjustMoney {
        player_id: i32,
        delta: i32,
        src: Option<Msg>,
    },
    Marker {
        player_id: i32,
        name: String,
        delta: i32,
    },
}

/// One event a deferral parked, awaiting its owner's turn start.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DeferredEvent {
    /// The event card taken out of the deck, unresolved.
    pub event: String,
    /// The drawer whose event this was; the release resolves for it.
    pub player: i32,
    /// The tile the draw stood on; the release resolves there.
    pub tile: i32,
    /// The deferring instance's owner; only that seat's turn start is due.
    pub due_owner: i32,
    /// The deferring instance: release cause and the `eventResolved` subject.
    pub deferrer_uid: i32,
    /// The deferring card, on the release's `event` raise.
    pub deferrer_card: String,
    /// `by_card` on the release, kept beside the record rather than re-read.
    pub by_card: i32,
    /// Turn key when deferred; the release requires a strictly later one.
    pub after_turn_key: i32,
}

/// A [`World`] checkpoint (`world_snapshot`).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorldSnapshot {
    pub players: Vec<PlayerSnapshot>,
    pub owners: Vec<i32>,
    pub houses: Vec<i32>,
    pub mortgaged: Vec<bool>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlayerSnapshot {
    pub pos: i32,
    pub state: Vec<(String, i32)>,
    pub money: i32,
}
