//! Roll transactions: one cast, its windows and its single settlement.
//!
//! A roll is a [`RollTransaction`] on [`World::open_rolls`], carrying the
//! roller, the purpose, the source and every cast. Each cast runs one fixed
//! ladder (design `docs/rulebook/spec/abi/G03-roll-and-prompts.md`):
//!
//! ```text
//! Cast -> diceRollCast -> diceRollBefore -> diceRollAdd -> diceRollValue
//!      -> [diceRollAfter -> diceRollResult] -> Settle -> diceRollSettled
//! ```
//!
//! `diceRollAdd` and `diceRollValue` fold **once**, on the chosen cast, after
//! the last window; the result is cached on the cast. A window write to the
//! running face clears the cache and is treated as a new raw value, so the
//! settle runs at most twice per cast. `diceRollSettled` fires exactly once
//! per transaction.
//!
//! Three call shapes exist: a *join* (a recast on an open move transaction), a
//! *preview* (a raw face for a later move, karaoke's candidates), and a
//! *standalone* move roll (Doll Garden's victim). Effect rolls always open a
//! child transaction and stop at the settlement -- the After/Result pair is
//! Move-only.
//!
//! The transaction stays **in the stack** throughout, because a window body can
//! join it (`roll_recast`) or pick another cast (`roll_pick`). Stages therefore
//! mutate through the stack by id between raises instead of holding a borrow
//! across `self.raise`.

use super::cx::{Cx, Flow};
use super::play::Move;
use super::rules::{RollSource, Trigger};
use super::world::{FixedFace, RollHistoryEntry, World};
use serde::{Deserialize, Serialize};

/// `RollPurpose::Effect` (card / skill `count`d`sides`).
pub const PURPOSE_EFFECT: i32 = 0;
/// `RollPurpose::Move` (the move plan's dice tables).
pub const PURPOSE_MOVE: i32 = 1;

/// One cast of a transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollCast {
    /// The face as it stands (a window write replaces it).
    pub raw: i32,
    /// `diceRollAdd` + `diceRollValue` applied once. Cleared by a later write
    /// to `raw`.
    pub settled: Option<i32>,
    /// The source this cast was made with.
    pub source: RollSource,
    /// The face came from a fixed roll, not a throw.
    pub fixed: bool,
}

/// Where a transaction is in the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollStage {
    Plan,
    Cast,
    Before,
    Add,
    Value,
    After,
    Result,
    /// The `diceRollSettled` window is open.
    Settling,
    /// Done; kept on the record only.
    Settled,
}

/// A roll in progress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollTransaction {
    pub id: i32,
    pub roller: usize,
    /// [`PURPOSE_EFFECT`] / [`PURPOSE_MOVE`].
    pub purpose: i32,
    /// The effect producer's dice counts.
    pub count: i32,
    pub sides: i32,
    /// The highest-ranked source seen.
    pub source: RollSource,
    pub stage: RollStage,
    pub casts: Vec<RollCast>,
    /// Index into `casts` of the face that will settle.
    pub chosen_index: usize,
    /// The face the latest pick did not take; `-1` when none.
    pub unused_face: i32,
    /// `diceRollAfter` / `diceRollResult` belong to this ladder.
    pub has_result_window: bool,
    /// The innermost open transaction at open time, or `-1`.
    pub parent: i32,
    /// The transaction is the turn's own main move (`move.main` on a
    /// synthesized view; false for a side move, a preview or a victim roll).
    #[serde(default)]
    pub main: bool,
    /// The current cast is a fixed face; no die is thrown.
    pub fixed: bool,
    /// The running face was written after its settlement; the settle re-runs.
    pub dirty: bool,
    /// The settled face, recorded at the commit.
    pub final_face: Option<i32>,
}

impl RollTransaction {
    fn new(id: i32, roller: usize, purpose: i32, count: i32, sides: i32, parent: i32) -> Self {
        Self {
            id,
            roller,
            purpose,
            count,
            sides,
            source: RollSource::Unattributed,
            stage: RollStage::Plan,
            casts: Vec::new(),
            chosen_index: 0,
            unused_face: -1,
            has_result_window: purpose == PURPOSE_MOVE,
            parent,
            main: false,
            fixed: false,
            dirty: false,
            final_face: None,
        }
    }

    /// The running face of the chosen cast.
    pub fn face(&self) -> i32 {
        self.casts
            .get(self.chosen_index)
            .map(|c| c.settled.unwrap_or(c.raw))
            .unwrap_or(0)
    }

    /// Take a window answer as the new raw face (clears the settle cache).
    ///
    /// The comparison is against the running face (the settled value once one
    /// is cached), not the raw cast: a window that writes back the folded face
    /// unchanged must not re-fold it (I3 -- once per cast).
    fn write_face(&mut self, value: i32) {
        let Some(c) = self.casts.get(self.chosen_index) else {
            return;
        };
        let v = value.max(0);
        if v == c.settled.unwrap_or(c.raw) {
            return;
        }
        let c = self
            .casts
            .get_mut(self.chosen_index)
            .expect("checked above");
        c.raw = v;
        c.settled = None;
        self.dirty = true;
    }
}

/// Transaction accessors on the **world** itself. The card rules read them
/// through the run's world copy (`CardWorld for Run`), so the lookups cannot
/// live on [`Cx`] alone; the `Cx` methods below delegate.
impl World {
    /// The innermost open transaction id, `-1` when none.
    pub fn open_roll_id(&self) -> i32 {
        self.open_rolls.last().map(|r| r.id).unwrap_or(-1)
    }

    /// A transaction by id.
    pub fn roll_by_id(&self, id: i32) -> Option<&RollTransaction> {
        self.open_rolls.iter().find(|r| r.id == id)
    }

    /// The innermost open move transaction id, `-1` when none.
    pub fn open_move_roll_id(&self) -> i32 {
        self.open_rolls
            .iter()
            .rev()
            .find(|r| r.has_result_window && r.stage != RollStage::Settling)
            .map(|r| r.id)
            .unwrap_or(-1)
    }

    /// The synthesized move view of a transaction, for a trigger raised
    /// outside a live move frame (a join recast or a preview): a walk that does
    /// not resolve -- the transaction's caller owns the walk and the settle.
    pub fn roll_move_view(&self, id: i32, fallback_roller: usize) -> Move {
        let roller = self.roll_by_id(id).map(|r| r.roller).unwrap_or(fallback_roller);
        let mut m = Move::default();
        m.player_id = roller;
        m.roller = Some(game_kinds::Seat(roller as i32));
        m.main = self.roll_by_id(id).map(|r| r.main).unwrap_or(false);
        m.resolve = false;
        m
    }

    /// May a plain `roll` join a transaction? Only a windowed (real) move
    /// transaction that is not already settling -- an effect roll is always
    /// its own child, and a preview is not a join target.
    pub fn can_join(&self) -> bool {
        self.open_rolls
            .iter()
            .any(|r| r.has_result_window && r.stage != RollStage::Settling)
    }

    /// Pick a cast; the enclosing commit settles it.
    pub fn roll_pick(&mut self, id: i32, index: i32) {
        let Some(r) = self.open_rolls.iter_mut().find(|r| r.id == id) else {
            return;
        };
        let n = r.casts.len();
        if n == 0 {
            return;
        }
        let idx = (index.max(0) as usize).min(n - 1);
        // I5: the keep point is the cast of the latest pair the pick did not
        // take, so it is recorded even when the pick re-keeps the face that
        // was already chosen. A single cast has nothing unchosen.
        if n >= 2 {
            let other = if idx == n - 1 { n - 2 } else { n - 1 };
            r.unused_face = r.casts[other].raw;
        }
        if idx == r.chosen_index {
            return;
        }
        r.chosen_index = idx;
        r.dirty = true;
    }

    /// The face the player did not keep, `-1` when none.
    pub fn roll_unused_face(&self, id: i32) -> i32 {
        self.roll_by_id(id).map(|r| r.unused_face).unwrap_or(-1)
    }

    /// More than one cast on the transaction?
    pub fn roll_rerolled(&self, id: i32) -> bool {
        self.roll_by_id(id).map(|r| r.casts.len() > 1).unwrap_or(false)
    }

    /// Every face `player` has cast this turn.
    pub fn rolls_by_player(&self, player: usize) -> Vec<i32> {
        self.turn
            .roll_history
            .iter()
            .filter(|e| e.roller == player)
            .map(|e| e.value)
            .collect()
    }
}

impl Cx<'_> {
    // ---- stack ---------------------------------------------------------

    fn next_roll_id(&mut self) -> i32 {
        let id = self.w.next_roll_id;
        self.w.next_roll_id = id + 1;
        id
    }

    /// The innermost open transaction id, `-1` when none.
    pub fn open_roll_id(&self) -> i32 {
        self.w.open_roll_id()
    }

    /// A transaction by id.
    pub fn roll_by_id(&self, id: i32) -> Option<&RollTransaction> {
        self.w.roll_by_id(id)
    }

    /// The innermost open move transaction id, `-1` when none.
    pub fn open_move_roll_id(&self) -> i32 {
        self.w.open_move_roll_id()
    }

    /// The synthesized move view of a transaction, for a trigger raised
    /// outside a live move frame (a join recast or a preview): a walk that does
    /// not resolve -- the transaction's caller owns the walk and the settle.
    pub fn roll_move_view(&self, id: i32, fallback_roller: usize) -> Move {
        self.w.roll_move_view(id, fallback_roller)
    }

    /// May a plain `roll` join a transaction? Only a windowed (real) move
    /// transaction that is not already settling -- an effect roll is always
    /// its own child, and a preview is not a join target.
    pub fn can_join(&self) -> bool {
        self.w.can_join()
    }

    /// Open a move transaction at the top of the stack without casting yet
    /// (the caller raises `diceRollPlan` against the fresh move first, and may
    /// still abandon it for a teleport rewrite).
    pub fn open_move_roll(&mut self, roller: usize, source: RollSource) -> i32 {
        let id = self.next_roll_id();
        let parent = self.open_roll_id();
        let mut r = RollTransaction::new(id, roller, PURPOSE_MOVE, 0, 0, parent);
        r.source = source;
        self.w.open_rolls.push(r);
        id
    }

    /// Drop a transaction without a settlement (a `diceRollPlan` teleport
    /// rewrite casts no dice).
    pub fn abandon_roll(&mut self, id: i32) -> Option<RollTransaction> {
        self.drop_roll(id)
    }

    fn tx_mut(&mut self, id: i32) -> Option<&mut RollTransaction> {
        self.w.open_rolls.iter_mut().find(|r| r.id == id)
    }

    fn drop_roll(&mut self, id: i32) -> Option<RollTransaction> {
        let at = self.w.open_rolls.iter().position(|r| r.id == id)?;
        Some(self.w.open_rolls.remove(at))
    }

    // ---- raises --------------------------------------------------------

    /// Build and run a roll trigger. `m` is the move view (a Move transaction
    /// only); the payload carries the transaction.
    pub fn raise_roll(
        &mut self,
        kind: &str,
        id: i32,
        m: Option<&Move>,
        value: i32,
    ) -> Flow<Trigger> {
        let Some(r) = self.roll_by_id(id) else {
            return Ok(Trigger::new(kind, 0));
        };
        let mut t = Trigger::new(kind, r.roller);
        if let Some(m) = m {
            t.with_move(m);
        }
        t.with_roll(r);
        t.value = value;
        self.raise(t)
    }

    // ---- stages --------------------------------------------------------

    /// Throw one face and log it. Returns the raw face.
    fn cast(&mut self, id: i32, m: Option<&Move>, source: RollSource) -> Flow<i32> {
        let Some(r) = self.roll_by_id(id) else {
            return Ok(0);
        };
        let (purpose, roller) = (r.purpose, r.roller);
        let value = if purpose == PURPOSE_MOVE {
            let m = m.expect("a move roll carries its move");
            self.roll_tables(m)
        } else {
            let (count, sides) = (r.count, r.sides);
            self.card_roll(roller, count, sides)
        };
        let Some(r) = self.tx_mut(id) else {
            return Ok(value);
        };
        if source.rank() > r.source.rank() {
            r.source = source;
        }
        let source = r.source;
        r.casts.push(RollCast {
            raw: value,
            settled: None,
            source,
            fixed: false,
        });
        r.chosen_index = r.casts.len() - 1;
        r.unused_face = -1;
        r.stage = RollStage::Cast;
        let at = r.chosen_index;
        let entry = RollHistoryEntry {
            roller: r.roller,
            value,
            purpose: r.purpose,
            source: r.source,
            fixed: r.fixed,
        };
        self.w.turn.roll_history.push(entry);
        self.w.turn.turn_rolls.push(value);
        let t = self.raise_roll("diceRollCast", id, m, value)?;
        let Some(r) = self.tx_mut(id) else {
            return Ok(t.value.max(0));
        };
        r.casts[at].raw = t.value.max(0);
        Ok(r.casts[at].raw)
    }

    /// Push a fixed face; no die is thrown and no `diceRollCast` opens.
    fn cast_fixed(&mut self, id: i32, fixed: FixedFace) {
        let Some(r) = self.tx_mut(id) else {
            return;
        };
        if fixed.source.rank() > r.source.rank() {
            r.source = fixed.source;
        }
        r.casts.push(RollCast {
            raw: fixed.value.max(0),
            settled: None,
            source: fixed.source,
            fixed: true,
        });
        r.chosen_index = r.casts.len() - 1;
        r.unused_face = -1;
        r.fixed = true;
        let value = fixed.value.max(0);
        let entry = RollHistoryEntry {
            roller: r.roller,
            value,
            purpose: r.purpose,
            source: r.source,
            fixed: r.fixed,
        };
        self.w.turn.roll_history.push(entry);
        self.w.turn.turn_rolls.push(value);
    }

    /// The additive + final-settlement fold, once per cast, on the chosen
    /// face. A cached face that is not dirty is left alone.
    fn settle_cast(&mut self, id: i32, m: Option<&Move>) -> Flow<()> {
        let Some(r) = self.roll_by_id(id) else {
            return Ok(());
        };
        let idx = r.chosen_index;
        if !r.dirty && r.casts[idx].settled.is_some() {
            return Ok(());
        }
        let raw = r.casts[idx].raw;
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Add;
        }
        let mut v = self.fold_roll("diceRollAdd", id, m, raw)?;
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Value;
        }
        v = self.fold_roll("diceRollValue", id, m, v)?;
        if let Some(r) = self.tx_mut(id) {
            r.casts[idx].settled = Some(v.max(0));
            r.dirty = false;
        }
        Ok(())
    }

    fn fold_roll(
        &mut self,
        kind: &str,
        id: i32,
        m: Option<&Move>,
        value: i32,
    ) -> Flow<i32> {
        let t = self.raise_roll(kind, id, m, value)?;
        Ok(t.value.max(0))
    }

    /// Raise `diceRollSettled`, re-fold a listener write, and finish.
    fn commit_roll(&mut self, id: i32, m: Option<&Move>) -> Flow<i32> {
        let face = match self.roll_by_id(id) {
            Some(r) => r.face(),
            None => return Ok(0),
        };
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Settling;
            r.final_face = Some(face);
        }
        let t = self.raise_roll("diceRollSettled", id, m, face)?;
        if let Some(r) = self.tx_mut(id) {
            let v = t.value;
            r.write_face(v);
        }
        if self.roll_by_id(id).map(|r| r.dirty).unwrap_or(false) {
            self.settle_cast(id, m)?;
        }
        let Some(mut r) = self.drop_roll(id) else {
            return Ok(t.value.max(0));
        };
        let face = r.face();
        r.final_face = Some(face);
        r.stage = RollStage::Settled;
        Ok(face)
    }

    // ---- shapes --------------------------------------------------------

    /// The full ladder of a move roll that already owns its transaction (the
    /// turn's main move or a card-driven move). The caller opened it with
    /// [`Self::open_move_roll`] and raised `diceRollPlan` on the fresh move.
    pub fn run_move_roll(
        &mut self,
        id: i32,
        m: &Move,
        fixed: Option<FixedFace>,
    ) -> Flow<i32> {
        if let Some(r) = self.tx_mut(id) {
            r.main = m.main;
        }
        if fixed.is_some() {
            let f = fixed.expect("checked");
            self.cast_fixed(id, f);
        } else {
            let source = self.roll_by_id(id).map(|r| r.source).unwrap_or_default();
            self.cast(id, Some(m), source)?;
        }
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Before;
        }
        let face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
        let t = self.raise_roll("diceRollBefore", id, Some(m), face)?;
        if let Some(r) = self.tx_mut(id) {
            r.write_face(t.value);
        }
        self.settle_cast(id, Some(m))?;
        let face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::After;
        }
        let t = self.raise_roll("diceRollAfter", id, Some(m), face)?;
        if let Some(r) = self.tx_mut(id) {
            r.write_face(t.value);
        }
        if self.roll_by_id(id).map(|r| r.dirty).unwrap_or(false) {
            self.settle_cast(id, Some(m))?;
        }
        let face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Result;
        }
        let t = self.raise_roll("diceRollResult", id, Some(m), face)?;
        if let Some(r) = self.tx_mut(id) {
            r.write_face(t.value);
        }
        self.commit_roll(id, Some(m))
    }

    /// A child effect roll: the whole effect ladder, always its own
    /// transaction, even inside a move window.
    pub fn run_effect_roll(
        &mut self,
        roller: usize,
        count: i32,
        sides: i32,
        source: RollSource,
    ) -> Flow<i32> {
        let id = self.next_roll_id();
        let parent = self.open_roll_id();
        let mut r = RollTransaction::new(id, roller, PURPOSE_EFFECT, count, sides, parent);
        r.source = source;
        r.has_result_window = false;
        self.w.open_rolls.push(r);

        self.cast(id, None, source)?;
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Before;
        }
        let face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
        let t = self.raise_roll("diceRollBefore", id, None, face)?;
        if let Some(r) = self.tx_mut(id) {
            r.write_face(t.value);
        }
        self.settle_cast(id, None)?;
        self.commit_roll(id, None)
    }

    /// A preview move roll: cast + `diceRollBefore`, committed raw. No
    /// settlement and no `diceRollSettled` -- the real move settles the face.
    pub fn run_preview_roll(
        &mut self,
        roller: usize,
        m: &Move,
        source: RollSource,
    ) -> Flow<i32> {
        let id = self.next_roll_id();
        let parent = self.open_roll_id();
        let mut r = RollTransaction::new(id, roller, PURPOSE_MOVE, 0, 0, parent);
        r.source = source;
        r.has_result_window = false;
        r.main = m.main;
        self.w.open_rolls.push(r);

        self.cast(id, Some(m), source)?;
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Before;
        }
        let face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
        let t = self.raise_roll("diceRollBefore", id, Some(m), face)?;
        if let Some(r) = self.tx_mut(id) {
            r.write_face(t.value);
        }
        let mut face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
        // Kept on the stack for the rest of the turn: `roll_reroll`'s
        // no-open-transaction fallback picks one of these candidates by id.
        if let Some(r) = self.tx_mut(id) {
            face = r.face();
            r.final_face = Some(face);
            r.stage = RollStage::Settled;
        }
        Ok(face)
    }

    /// A standalone move roll for another seat (Doll Garden's victim): its own
    /// top-level transaction, the whole Move ladder, the plan raise against
    /// the victim's view of the shared plan.
    pub fn roll_move(
        &mut self,
        roller: usize,
        m: &Move,
        source: RollSource,
    ) -> Flow<i32> {
        let id = self.open_move_roll(roller, source);
        self.raise_roll("diceRollPlan", id, Some(m), -1)?;
        self.run_move_roll(id, m, None)
    }

    /// A standalone victim move roll (`ruleset_roll_move`). The caller names
    /// only the seat, so the view is a plain non-main walk that never
    /// resolves -- the card drives the walk itself with the returned face.
    pub fn run_standalone_roll(&mut self, roller: usize, source: RollSource) -> Flow<i32> {
        let mut m = Move::default();
        m.player_id = roller;
        m.roller = Some(game_kinds::Seat(roller as i32));
        m.resolve = false;
        self.roll_move(roller, &m, source)
    }

    /// A recast on an open transaction (a reroll). Raises its own
    /// `diceRollBefore` and, on a windowed move, `diceRollResult`, both with
    /// the transaction source. Returns the new raw face.
    pub fn roll_recast(
        &mut self,
        id: i32,
        source: RollSource,
        m: Option<&Move>,
    ) -> Flow<i32> {
        let Some(roll) = self.roll_by_id(id) else {
            return Ok(0);
        };
        let windowed = roll.has_result_window;
        let face = self.cast(id, m, source)?;
        if let Some(r) = self.tx_mut(id) {
            r.stage = RollStage::Before;
        }
        let t = self.raise_roll("diceRollBefore", id, m, face)?;
        if let Some(r) = self.tx_mut(id) {
            r.write_face(t.value);
        }
        // Every cast folds its own Add / Value (`settle_cast`): a rerolled
        // face settles through the same 「最终结算」 modifiers as its first
        // throw (G03 §2.14), and the pick only chooses *between* settled
        // faces. Position matches `run_move_roll` (Before -> settle -> After).
        self.settle_cast(id, m)?;
        if windowed {
            let face = self.roll_by_id(id).map(|r| r.face()).unwrap_or(0);
            if let Some(r) = self.tx_mut(id) {
                r.stage = RollStage::Result;
            }
            let t = self.raise_roll("diceRollResult", id, m, face)?;
            if let Some(r) = self.tx_mut(id) {
                r.write_face(t.value);
            }
        }
        Ok(self.roll_by_id(id).map(|r| r.face()).unwrap_or(0))
    }

    /// Pick a cast; the enclosing commit settles it.
    pub fn roll_pick(&mut self, id: i32, index: i32) {
        self.w.roll_pick(id, index);
    }

    /// The face the player did not keep, `-1` when none.
    pub fn roll_unused_face(&self, id: i32) -> i32 {
        self.w.roll_unused_face(id)
    }

    /// A windowless cast (`ruleset_roll_direct`): the die is thrown and logged
    /// with its source, but no window, no fold and no settlement run. On a
    /// move it joins the open transaction when there is one and otherwise
    /// burns a throwaway preview; an effect cast does the same with a
    /// throwaway transaction, so the source still reaches a later
    /// `diceRollSettled`.
    pub fn run_direct_roll(
        &mut self,
        roller: usize,
        purpose: i32,
        count: i32,
        sides: i32,
        source: RollSource,
    ) -> Flow<i32> {
        if purpose == PURPOSE_MOVE {
            if let Some(id) = (self.open_move_roll_id() >= 0).then(|| self.open_move_roll_id()) {
                let view = self.roll_move_view(id, roller);
                return self.cast(id, Some(&view), source);
            }
        }
        let id = self.next_roll_id();
        let parent = self.open_roll_id();
        let mut r = RollTransaction::new(id, roller, purpose, count, sides, parent);
        r.source = source;
        r.has_result_window = false;
        self.w.open_rolls.push(r);
        let face = if purpose == PURPOSE_MOVE {
            let view = self.roll_move_view(id, roller);
            self.cast(id, Some(&view), source)?
        } else {
            self.cast(id, None, source)?
        };
        self.drop_roll(id);
        Ok(face)
    }

    /// Every face this player has cast this turn, in cast order.
    pub fn rolls_by_player(&self, player: usize) -> Vec<i32> {
        self.w
            .turn
            .roll_history
            .iter()
            .filter(|e| e.roller == player)
            .map(|e| e.value)
            .collect()
    }
}
