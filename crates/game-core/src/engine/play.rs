//! Match routines: turns, movement, landing, money, property, cards, events, scoring.
//!
//! Ported from the non-content parts of `MatchHost.cs` (method names in comments).
//! Card/skill/band hooks (`Fx`), active events and per-player card variables are card
//! content and enter through [`super::rules::CardRules`]; the C# branches that only
//! exist for specific cards or events are left out here.

use super::cx::{Ask, Cx, Flow, Halt};
pub(crate) use super::move_ctx::MoveCtx as Move;
use super::move_ctx::MoveKind;
use super::rules::{raise, Dest, Trigger};
use crate::state::{key, Tick};
use super::world::{Signal, CIRCLE_MONEY, HAND_LIMIT, START_HAND, START_MONEY};
use crate::msg::{Arg, Msg};

/// A payment (C# `PayCtx`, the fields the shell uses).
#[derive(Debug, Clone)]
pub(crate) struct Pay {
    pub from: Option<usize>,
    pub to: Option<usize>,
    pub amount: i32,
    pub kind: &'static str,
    /// Event type; derived from `kind`/direction when `None`.
    pub typ: Option<&'static str>,
    pub tile: Option<usize>,
    /// Mandatory: raise funds (mortgage, then bankruptcy) if short.
    pub must: bool,
    /// Log line; gets the amount as the `amount` argument.
    pub text: Option<Msg>,
    /// Key naming where the money came from (`src.*`), shown in parentheses.
    pub source: &'static str,
    /// The player whose card caused this payment (C# `t.ByCard`), or `None` when
    /// the payment is board-driven (rent, buy, build). Stamped onto the `pay` /
    /// `paid` triggers so `H.HitByOtherCard` can tell the two apart.
    pub by_card: Option<i32>,
}

impl Pay {
    pub fn new(amount: i32, kind: &'static str) -> Self {
        Self {
            from: None,
            to: None,
            amount,
            kind,
            typ: None,
            tile: None,
            must: false,
            text: None,
            source: "",
            by_card: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Paid {
    pub paid: bool,
    pub loss: i32,
}

impl Cx<'_> {
    // =============================================================== turn flow

    /// `BeginPlay` (the part without prompts; the host then runs `opening`).
    pub(crate) fn begin_play(&mut self) {
        let n = self.data.tiles.len();
        let st = &mut self.w.st;
        st.phase = "play".into();
        st.owners = vec![-1; n];
        st.houses = vec![0; n];
        st.mortgaged = vec![false; n];
        st.embers = vec![0; n];
        for i in 0..st.players.len() {
            st.players[i].money = START_MONEY;
            st.players[i].pos = 0;
            let mut draw = std::mem::take(&mut self.w.hidden[i].draw);
            self.w.rng.shuffle(&mut draw);
            self.w.hidden[i].draw = draw;
        }
        self.setup_event_deck();
        let st = &mut self.w.st;
        st.round = 1;
        st.turn = -1;
        st.step = 0;
        self.w.log("text", -1, Msg::new("log.match_start").n("money", START_MONEY).i("hand", START_HAND as i64));
    }

    /// `Opening` -- deal opening hands, offer humans one mulligan.
    pub(crate) fn opening(&mut self) -> Flow<()> {
        // C# `CardsBeforeGame` (MatchHost.cs:20575, called at 27204): every
        // distinct card id in each draw pile gets `deckBeforeGame` on that card
        // alone, before the opening hands are drawn. Up to 5 passes, so cards a
        // hook adds to the pile get their turn too.
        for i in 0..self.w.player_count() {
            let mut seen: Vec<String> = Vec::new();
            for _ in 0..5 {
                let mut fresh: Vec<String> = Vec::new();
                for id in &self.w.hidden[i].draw {
                    if !seen.contains(id) && !fresh.contains(id) {
                        fresh.push(id.clone());
                    }
                }
                if fresh.is_empty() {
                    break;
                }
                for id in fresh {
                    seen.push(id.clone());
                    raise!(self, "deckBeforeGame", i, card = id)?;
                }
            }
        }
        for i in 0..self.w.player_count() {
            self.draw(i, START_HAND, false)?;
        }
        self.wait(3.0);
        let humans: Vec<usize> = (0..self.w.player_count()).filter(|&p| !self.w.st.players[p].ai).collect();
        if !humans.is_empty() {
            self.w.log("text", -1, Msg::new("log.mulligan_offer").i("n", START_HAND as i64));
            let ask = Ask::choice(
                humans.clone(),
                Msg::new("ask.mulligan.title"),
                Msg::new("ask.mulligan.text").i("n", START_HAND as i64),
                vec![Msg::new("ask.mulligan.keep").i("n", START_HAND as i64), Msg::new("ask.mulligan.redo")],
                0,
                20.0,
            )
            .with_kind("mulligan");
            let r = self.ask(ask)?;
            let redo: Vec<usize> = humans.into_iter().filter(|&p| r.of(p) == 1).collect();
            for &p in &redo {
                self.mulligan(p)?;
            }
            if redo.is_empty() {
                self.w.log("text", -1, Msg::new("log.mulligan_none"));
            }
            self.wait(0.8);
        }
        // C# `CardsAtGameStart` (20604, called at 27237): every distinct id in
        // each draw pile and hand gets `deckAtGameStart`, after the mulligan.
        for i in 0..self.w.player_count() {
            let mut ids: Vec<String> = Vec::new();
            for id in self.w.hidden[i].draw.iter().chain(self.w.hidden[i].hand.iter()) {
                if !ids.contains(id) {
                    ids.push(id.clone());
                }
            }
            for id in ids {
                raise!(self, "deckAtGameStart", i, card = id)?;
            }
        }
        self.w.next_turn_pending = true;
        Ok(())
    }

    /// `Mulligan`
    fn mulligan(&mut self, i: usize) -> Flow<()> {
        let h = &mut self.w.hidden[i];
        let hand = std::mem::take(&mut h.hand);
        h.draw.extend(hand);
        let mut draw = std::mem::take(&mut self.w.hidden[i].draw);
        self.w.rng.shuffle(&mut draw);
        self.w.hidden[i].draw = draw;
        self.w.st.players[i].mulligan = true;
        self.draw(i, START_HAND, false)?;
        self.w.log("mulligan", i as i32, Msg::new("log.mulligan").player_id("who", i).i("n", START_HAND as i64));
        Ok(())
    }

    /// `NextTurnRoutine` + `TurnStart`.
    pub(crate) fn next_turn(&mut self) -> Flow<()> {
        self.w.next_turn_pending = false;
        let n = self.w.player_count();
        let turn = self.w.st.turn;
        let mut next = turn;
        let extra = turn >= 0 && self.w.extra_turns.contains(&(turn as usize)) && !self.out(turn as usize);
        if extra {
            let t = turn as usize;
            self.w.extra_turns.retain(|&x| x != t);
        } else {
            for k in 1..=n as i32 {
                let c = ((turn + k) % n as i32 + n as i32) % n as i32;
                if !self.out(c as usize) {
                    next = c;
                    break;
                }
            }
            if turn >= 0 && next <= turn {
                self.w.st.round += 1;
            }
        }
        if next < 0 {
            return Ok(()); // everyone is out; game-over handling already ran
        }
        let i = next as usize;
        let st = &mut self.w.st;
        st.turn = next;
        st.step = 1;
        st.bought = false;
        st.built = false;
        st.skip_move = false;
        st.landed = -1;
        st.roller = next;
        self.w.turn = super::world::TurnCtx { player_id: i, extra, ..Default::default() };
        self.w.signals.push(Signal::TurnBegan);
        let text = Msg::new(if extra { "log.turn_extra" } else { "log.turn" }).i("round", self.w.st.round).player_id("who", i);
        self.w.log("turn", next, text);
        self.turn_start(i)
    }

    /// `TurnStartInner` -- status effects tick, then the clock starts.
    fn turn_start(&mut self, i: usize) -> Flow<()> {
        if self.out(i) {
            return Ok(());
        }
        // C# `_targeted[i] = 0` -- the between-turns target counter.
        if let Some(n) = self.w.targeted.get_mut(i) {
            *n = 0;
        }
        // `turnStartBefore` -- before any exile/stun status ticks resolve, so a
        // reaction sees the turn exactly as it was left last turn.
        raise!(self, "turnStartBefore", i, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        if self.w.st.players[i].exile() > 0 {
            self.w.st.players[i].state_add(key::EXILE, -1);
            let left = self.w.st.players[i].exile();
            if left > 0 {
                self.w.log("status", i as i32, Msg::new("log.exiled_skip").player_id("who", i).i("left", left));
                self.wait(1.0);
                return self.end_turn(i);
            }
            let to = self.w.st.players[i].exile_to();
            self.w.st.players[i].state_set(key::EXILE_TO, -1);
            self.w.log("status", i as i32, Msg::new("log.exile_over").player_id("who", i));
            if to >= 0 {
                self.teleport(i, to as usize, false, None)?;
            }
        }
        let s = &mut self.w.st.players[i];
        // Timed counters due now -- the item says so, not the engine naming
        // `stunStart`.
        s.tick_state(Tick::TurnStart);
        if s.no_hand() == 2 {
            s.state_set(key::NO_HAND, 1);
        }
        raise!(self, "turnStart", i, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        self.w.signals.push(Signal::StartTimer(i));
        if self.w.st.players[i].stunned() {
            self.w.log("status", i as i32, Msg::new("log.stunned_skip").player_id("who", i));
            self.w.st.step = 3;
            self.wait(1.2);
            return self.end_turn(i);
        }
        if self.w.st.players[i].stay() > 0 {
            self.w.st.skip_move = true;
            self.w.log("status", i as i32, Msg::new("log.stay_skip").player_id("who", i));
        }
        self.w.st.roller = i as i32;
        self.wait(1.2);
        Ok(())
    }

    /// `EndTurn` -- the player's "end turn" command.
    pub(crate) fn end_turn_cmd(&mut self, i: usize) -> Flow<()> {
        // `endTurnBefore` -- the player chose to end the turn. (Auto-skips --
        // stun, exile -- go straight to [`Self::end_turn`] and so raise only
        // `endTurnAfter`, which is the "the turn ended" signal.)
        raise!(self, "endTurnBefore", i)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        self.w.log("text", i as i32, Msg::new("log.end_turn").player_id("who", i));
        self.end_turn(i)
    }

    /// `EndTurnRoutine` -- status effects wear off, next turn queued.
    pub(crate) fn end_turn(&mut self, i: usize) -> Flow<()> {
        self.w.st.step = 3;
        // C# `EndTurnRoutine` (27493): TurnEndBefore -> `AtEnd` callbacks ->
        // status wear-off -> TurnEnd -> TurnEndAfter -> `AfterEnd` callbacks.
        // `turnEndBefore` also runs the `before_turn_end` callbacks (`AtEnd`).
        raise!(self, "turnEndBefore", i)?;
        if self.out(i) || !self.playing() {
            // C# 27541: an out player / ended match skips the rest.
            self.w.next_turn_pending = true;
            return Ok(());
        }
        {
            let s = &mut self.w.st.players[i];
            // Timed counters due now (C# decrements `stay`/`stun` here). The
            // item carries its own expiry, so this names no keys.
            s.tick_state(Tick::TurnEnd);
            if s.no_hand() == 1 {
                s.state_set(key::NO_HAND, 0);
            }
        }
        self.w.next_turn_pending = true;
        // `turnEnd` (Fx hook point) -- every placed card decays/acts here.
        raise!(self, "turnEnd", i)?;
        // `turnEndAfter` (Fx) -- also runs the `at_turn_end` callbacks (C#
        // `AfterEnd`), so a [停留] they grant survives this turn's wear-off.
        raise!(self, "turnEndAfter", i)?;
        // `endTurnAfter` -- the turn is over and the next one is queued. Raised
        // for every turn end, including auto-skips.
        raise!(self, "endTurnAfter", i)?;
        Ok(())
    }

    // =============================================================== movement

    /// `H.CardMove(c, m)` -> `MainMoveAs` -- run a card-shaped movement **now**,
    /// as the player's main move. This is the 「立刻进入移动阶段」/「视为你的主要移动」
    /// case: the card has already shaped `plan` (via the `World` plan ops) and
    /// this consumes the turn's main move and walks it immediately, with the
    /// normal raise points.
    ///
    /// Returns `Ok(())` without moving when the turn's main move is already
    /// spent (the C# logs 「这回合已经进行过 [主要移动]，这次移动无效」).
    pub fn card_move(&mut self, player_id: usize, mut plan: Move) -> Flow<()> {
        let i = player_id;
        let is_turn = self.w.st.turn == i as i32;
        if self.w.turn.main_moved && is_turn {
            self.w.log("text", i as i32, Msg::new("log.main_move_used").player_id("who", i));
            return Ok(());
        }
        if is_turn {
            self.w.turn.main_moved = true;
        }
        plan.player_id = i;
        plan.roller = i;
        plan.main = is_turn;
        self.w.st.step = 2;
        if plan.teleport_to >= 0 {
            let to = plan.teleport_to as usize;
            let resolve = plan.resolve;
            let why = plan.why.clone();
            self.teleport(i, to, resolve, why)?;
        } else {
            if plan.steps < 0 {
                // No fixed step count: roll it now, with the same raise points.
                self.w.turn.plan = plan.clone();
                raise!(self, "rollPlan", i, @m plan)?;
                plan = self.w.turn.plan.clone();
                plan.roll = match self.w.turn.fixed_roll {
                    Some(n) => n,
                    None => self.w.rng.d(20),
                };
                let t = raise!(self, "rollAfter", i, @m plan, value = plan.roll)?;
                plan.roll = t.value.max(0);
                let t = raise!(self, "moveRoll", i, @m plan, value = plan.roll)?;
                plan.roll = t.value.max(0);
                if let Some(steps) = self.w.hidden[i].next_steps.take() {
                    plan.roll = steps.max(0);
                }
                if !plan.signed {
                    plan.roll = plan.roll.max(plan.min_roll);
                }
            }
            if !plan.cancelled && !self.out(i) {
                self.walk(&mut plan)?;
            }
        }
        // `NoteWalk` / `LastMain` -- the C# writes `lastWalk` from `Total`.
        if is_turn {
            self.w.turn.main_steps = if plan.kind == MoveKind::Teleport { 0 } else { plan.total };
        }
        self.w.st.plan = plan.to_plan();
        if plan.kind != MoveKind::Teleport {
            self.w.set_slot(i as i32, "lastWalk", plan.total + 1);
        }
        if is_turn {
            self.w.st.step = 3;
        }
        self.wait(1.2);
        Ok(())
    }

    /// `MainMove` -- the turn's main roll-and-move.
    pub(crate) fn main_move(&mut self, i: usize, roller: usize) -> Flow<()> {
        self.w.st.step = 2;
        if self.w.turn.main_moved {
            self.w.log("text", i as i32, Msg::new("log.main_move_used").player_id("who", i));
            self.w.st.step = 3;
            return Ok(());
        }
        self.w.turn.main_moved = true;
        let mut m = Move::new(i);
        m.roller = roller;
        m.main = true;
        if !self.out(i) {
            // `roll` (pre) -- before the d20 is cast. `value = -1` is the
            // sentinel meaning "no roll yet": the bridge would otherwise
            // derive `move_roll` from `value` and let roll-reacting cards
            // (which match `Roll | MoveRoll`) fire before any dice exist.
            raise!(self, "roll", i, @m m, value = -1)?;
            if self.out(i) || !self.playing() {
                self.w.st.step = 3;
                return Ok(());
            }
        }
        if !self.out(i) {
            // `rollPlan` -- C# `RollMove`: every placed card's `On::RollPlan`
            // shapes the move before the dice (RollPlan -> roll -> RollAfter ->
            // the moveRoll [反击] window).
            //
            // The card bodies shape `TurnCtx::plan` (that is what the `World`
            // plan ops write), so the walk has to take the move back afterwards.
            // Seed first so a body that only reads sees the fresh move.
            self.w.turn.plan = m.clone();
            raise!(self, "rollPlan", i, @m m)?;
            m = self.w.turn.plan.clone();
            if self.out(i) || !self.playing() {
                self.w.st.step = 3;
                return Ok(());
            }
            // C# `TurnCtx.Plan.FixedRoll` replaces the d20 when a card set it.
            m.roll = match self.w.turn.fixed_roll {
                Some(n) => n,
                None => self.w.rng.d(20), // MoveCtx.Base = 1d20
            };
            // `rollAfter` (Fx) -- C# `Each(RollAfter)` runs on the fresh roll,
            // *before* the moveRoll [反击] window; a field card may rewrite it
            // (`set_move_roll`), which is how a stored boost lands.
            let t = raise!(self, "rollAfter", i, @m m, value = m.roll)?;
            m.roll = t.value.max(0);
            let t = raise!(self, "moveRoll", i, @m m, value = m.roll)?;
            // A [反击] may have rerolled the dice (C# shares `t.Move` with the
            // reactions): the face the walk uses is the one left on the trigger.
            m.roll = t.value.max(0);
            // C# `NextStepsFx.MoveBefore` -- a stored step count overrides the
            // roll for this one main move.
            if let Some(steps) = self.w.hidden[i].next_steps.take() {
                m.roll = steps.max(0);
            }
            // C# `RollMove`: `if (!m.Signed) m.Roll = max(m.MinRoll, m.Roll)` --
            // the clamp applies to the final face after the reactions, so a
            // card that pushes the roll down still respects the floor.
            if !m.signed {
                m.roll = m.roll.max(m.min_roll);
            }
            self.walk(&mut m)?;
            // C# `TurnCtx.LastMain`, and `NoteWalk`: `SetV(player_id, "lastWalk",
            // steps + 1)` after a non-teleport main walk (0 means "none").
            self.w.turn.main_steps = m.total;
            // `State.plan` -- the broadcast summary of this walk.
            self.w.st.plan = m.to_plan();
            if m.kind != MoveKind::Teleport {
                self.w.set_slot(i as i32, "lastWalk", m.total + 1);
            }
        }
        self.w.st.step = 3;
        self.wait(1.2);
        Ok(())
    }

    /// `WalkMove` -- step tile by tile; passing CiRCLE pays the reward.
    fn walk(&mut self, m: &mut Move) -> Flow<()> {
        let i = m.player_id;
        let n = self.data.tiles.len() as i32;
        let steps = m.roll;
        let head = |still: bool| {
            let base = if m.main {
                Msg::new("log.roll").opt("by", (m.roller != i).then(|| Msg::new("log.part.rolled_by").player_id("who", m.roller)))
            } else {
                Msg::new(if m.dir() < 0 { "log.move_back" } else { "log.move_forward" }).opt("why", m.why.clone().map(|w| Msg::new("log.part.why").msg("why", w)))
            };
            base.player_id("who", i)
                .i("n", steps)
                .opt("nores", (!m.resolve).then(|| Msg::new("log.part.no_resolve")))
                .opt("still", still.then(|| Msg::new("log.part.no_move")))
        };
        let kind = if m.main { "roll" } else { "move" };
        let pos = self.w.st.players[i].pos;
        if steps <= 0 {
            let e = self.w.log(kind, i as i32, head(true));
            e.from = pos;
            e.to = pos;
            e.dice = if m.main { steps.max(0) } else { 0 };
            self.wait(if m.main { 1.2 } else { 0.3 });
            return self.after_walk(m);
        }
        let (mut seg_from, mut seg_steps, mut first) = (pos, 0, true);
        // C# `WalkMoveSteps` bounds the walk by `steps + m.ExtraSteps`, re-read
        // each step: a card that adds steps mid-walk lengthens it. `MoreSteps`
        // is a second walk phase (see the tail below).
        let mut k = 0usize;
        while k < (steps.max(0) as usize) + m.extra_steps.max(0) as usize {
            let cur = self.w.st.players[i].pos;
            let next = ((cur + m.dir()) % n + n) % n;
            seg_steps += 1;
            let remaining = (steps.max(0) as usize) + m.extra_steps.max(0) as usize - k - 1;
            let at = next as usize;
            let passes_circle = self.tile(at).kind == "circle";
            let last = remaining == 0;
            if passes_circle || last {
                // `passBefore` -- same tiles `pass` fires at, but before the
                // player's position updates to `next` (it has not arrived yet).
                raise!(self, "passBefore", i, @m m, tile = next)?;
                if self.out(i) || !self.playing() {
                    return Ok(());
                }
                if self.w.st.players[i].pos != cur {
                    break; // moved by a passBefore reaction
                }
            }
            self.w.st.players[i].pos = next;
            // `passTile` (Fx) -- the player has stepped onto this tile.
            raise!(self, "passTile", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            if passes_circle || last {
                let text = if first { head(false) } else { Msg::new("log.move_on").player_id("who", i) };
                let ek = if first && m.main { "roll" } else { "move" };
                let e = self.w.log(ek, i as i32, text);
                e.from = seg_from;
                e.to = next;
                e.value = seg_steps * m.dir();
                e.dice = if first && m.main { m.roll } else { 0 };
                let pace = if ek == "roll" { 1.5 } else { 0.3 };
                self.wait(pace + seg_steps as f32 * 0.15);
                first = false;
                seg_from = next;
                seg_steps = 0;
                if passes_circle {
                    self.circle_reward(i, m.resolve && last)?;
                }
                raise!(self, "pass", i, @m m, tile = next)?;
                if self.out(i) || !self.playing() {
                    return Ok(());
                }
                if self.w.st.players[i].pos != next {
                    break; // moved by an effect
                }
            }
            k += 1;
        }
        self.after_walk(m)
    }

    /// `TeleportMove` / `Teleport`.
    ///
    /// C# `TeleportMove`: the walk bookkeeping is `Path = [to]`, `Total = 1`,
    /// and the pass handling runs **once at the destination** -- not per step,
    /// and not at all for a teleport that lands where it started (the log says
    /// 「原地，不算 [经过]」) unless the walk was a `TeleportWalk`.
    pub(crate) fn teleport(&mut self, i: usize, to: usize, resolve: bool, why: Option<Msg>) -> Flow<()> {
        if to >= self.data.tiles.len() || self.out(i) {
            return Ok(());
        }
        let from = self.w.st.players[i].pos;
        self.w.st.players[i].pos = to as i32;
        let mut m = Move::new(i);
        m.resolve = resolve;
        m.kind = MoveKind::Teleport;
        m.teleport_to = to as i32;
        m.from = from;
        m.path = vec![to as i32];
        m.total = 1;
        let same_tile = from == to as i32;
        let text = Msg::new("log.teleport")
            .player_id("who", i)
            .tile("to", to)
            .opt("why", why.map(|w| Msg::new("log.part.why").msg("why", w)))
            .opt("nores", (!resolve).then(|| Msg::new("log.part.no_resolve")));
        let e = self.w.log("teleport", i as i32, text);
        e.from = from;
        e.to = to as i32;
        self.wait(0.5);
        if !resolve {
            m.to = to as i32;
            // C# 24371: a teleport that does not settle still raises Teleported.
            if !self.out(i) && self.playing() {
                raise!(self, "teleported", i, @m m, tile = to as i32)?;
            }
            return Ok(());
        }
        // C#: `if (m.From != to || m.TeleportWalk)` -- the destination is a
        // [经过] when the teleport actually moves the player. With `TeleportWalk`
        // gone (「视为 [传送]（只触发终点）」 is just a teleport to the
        // roll-derived destination), that is the whole rule.
        if !same_tile {
            raise!(self, "passTile", i, @m m, tile = to as i32)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            let players: Vec<usize> = (0..self.w.st.players.len())
                .filter(|&s| s != i && self.w.st.players[s].pos == to as i32 && !self.out(s))
                .collect();
            for o in players {
                m.passed_players.push(o as i32);
                raise!(self, "passPlayer", i, @m m, tile = to as i32, target = o as i32)?;
                if self.out(i) || !self.playing() {
                    return Ok(());
                }
            }
        }
        if self.tile(to).kind == "circle" {
            self.circle_reward(i, true)?;
        }
        m.to = self.w.st.players[i].pos as i32;
        self.after_walk(&mut m)?;
        // C# 24390: `Teleported` after the settlement.
        if !self.out(i) && self.playing() {
            raise!(self, "teleported", i, @m m, tile = m.to)?;
        }
        Ok(())
    }

    /// `AfterWalk`
    fn after_walk(&mut self, m: &mut Move) -> Flow<()> {
        let i = m.player_id;
        if self.out(i) || !self.playing() || !m.resolve {
            return Ok(());
        }
        raise!(self, "settleBefore", i, @m m, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        self.settle(i, m)
    }

    /// `Settle` -> `Land`
    fn settle(&mut self, i: usize, m: &Move) -> Flow<()> {
        let at = self.w.st.players[i].pos as usize;
        let owner = self.w.st.owners.get(at).copied().unwrap_or(-1);
        let t = raise!(self, "settle", i, @m m, tile = at as i32, target = owner)?;
        // C# 24448: a cancelled settle ends here -- no Land, no SettleAfter.
        if self.out(i) || t.cancelled {
            return Ok(());
        }
        // `settleInstead` (Fx) -- C# `SettleInstead`: the first field card that
        // replaces the tile's effect does its own thing and calls
        // `trigger::set_cancelled()`; a later one should check `cancelled()`.
        let si = raise!(self, "settleInstead", i, @m m, tile = at as i32, target = owner)?;
        if !si.cancelled {
            self.land(i, m.main)?;
        }
        // C# 24493: SettleAfter only while the player is in and the match plays.
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        // `settleAfter` -- the landed tile is fully resolved. (Not raised when
        // `land` halts on an unanswered prompt; it fires on the successful
        // replay instead, alongside the rest of the routine.)
        raise!(self, "settleAfter", i, @m m, tile = at as i32, target = owner)?;
        Ok(())
    }

    // =============================================================== tiles

    /// `CircleReward` -- 2,000 money or one card.
    fn circle_reward(&mut self, i: usize, landing: bool) -> Flow<()> {
        if self.w.st.players[i].exile() > 0 {
            return Ok(());
        }
        let pick = if self.w.st.players[i].stunned() {
            self.w.log("text", i as i32, Msg::new("log.circle_card_only").player_id("who", i));
            1
        } else {
            let ask = if landing {
                Ask::choice(
                    vec![i],
                    Msg::new("ask.circle.title"),
                    Msg::new("ask.circle.text_landed").player_id("who", i),
                    vec![Msg::new("ask.circle.money_landed").n("money", CIRCLE_MONEY), Msg::new("ask.circle.card_landed")],
                    0,
                    10.0,
                )
            } else {
                Ask::choice(
                    vec![i],
                    Msg::new("ask.circle.title"),
                    Msg::new("ask.circle.text").player_id("who", i),
                    vec![Msg::new("ask.circle.money").n("money", CIRCLE_MONEY), Msg::new("ask.circle.card")],
                    0,
                    10.0,
                )
            };
            self.ask(ask)?.of(i)
        };
        if pick == 1 {
            self.w.log("text", i as i32, Msg::new("log.circle_card").player_id("who", i));
            self.draw_r(i, 1, "src.circle")?;
        } else {
            let mut p = Pay::new(CIRCLE_MONEY, "gain");
            p.to = Some(i);
            p.typ = Some("pass");
            p.source = "src.circle";
            p.text = Some(Msg::new("log.circle_money").player_id("who", i));
            self.money(p)?;
        }
        self.wait(0.4);
        Ok(())
    }

    /// `Land` -- resolve the tile a player stopped on.
    fn land(&mut self, i: usize, main: bool) -> Flow<()> {
        let at = self.w.st.players[i].pos as usize;
        if at >= self.data.tiles.len() || self.w.st.players[i].exile() > 0 || self.out(i) {
            return Ok(());
        }
        let tile = self.tile(at);
        if main {
            self.w.st.landed = at as i32;
        }
        if tile.is_buyable() {
            let owner = self.w.st.owners[at];
            if owner < 0 {
                if main {
                    let houses = self.w.st.houses[at];
                    let extra = (houses > 0).then(|| Msg::new("log.part.with_houses").i("h", houses));
                    self.w.log("text", i as i32, Msg::new("log.land_unowned").player_id("who", i).tile("tile", at).n("price", self.buy_price(at)).opt("extra", extra));
                } else {
                    self.offer_buy(i, at)?;
                }
            } else if owner as usize != i {
                self.pay_rent(i, at, false)?;
            } else if main {
                let note = if self.w.st.mortgaged[at] {
                    Some(Msg::new("log.part.mortgaged"))
                } else if self.why_not_build(i, at).is_none() {
                    Some(Msg::new("log.part.can_build"))
                } else {
                    None
                };
                self.w.log("text", i as i32, Msg::new("log.land_own").player_id("who", i).tile("tile", at).opt("note", note));
            } else {
                self.offer_build(i, at)?;
            }
            return Ok(());
        }
        match tile.kind.as_str() {
            "circle" => {
                self.w.log("text", i as i32, Msg::new("log.land_circle").player_id("who", i));
                self.draw_r(i, 1, "src.land_circle")?;
                self.wait(0.4);
            }
            "edogawa" => {
                self.w.log("text", i as i32, Msg::new("log.land_draw").player_id("who", i).tile("tile", at));
                self.draw_r(i, 1, "src.land_edogawa")?;
                self.wait(0.4);
            }
            "cafe" | "ryuseido" => {
                self.w.log("text", i as i32, Msg::new("log.land_event").player_id("who", i).tile("tile", at));
                self.draw_r(i, 1, "src.land_event")?;
                self.wait(0.6);
                self.draw_event(i)?;
            }
            "agent" => self.agent_landing(i, at)?,
            _ => {
                self.w.log("text", i as i32, Msg::new("log.land").player_id("who", i).tile("tile", at));
            }
        }
        Ok(())
    }

    /// `AgentLanding` -- buy or build once in the agent's colour group.
    pub fn agent_landing(&mut self, i: usize, agent: usize) -> Flow<()> {
        let g = self.tile(agent).group;
        let same: Vec<usize> = (0..self.data.tiles.len()).filter(|&t| self.tile(t).is_buyable() && self.tile(t).group == g).collect();
        if same.is_empty() {
            self.w.log("text", i as i32, Msg::new("log.agent_none").player_id("who", i).tile("agent", agent));
            return Ok(());
        }
        if same.iter().all(|&t| self.w.st.owners[t] >= 0 && self.w.st.owners[t] as usize != i) {
            self.w.log("text", i as i32, Msg::new("log.agent_all_owned").player_id("who", i).tile("agent", agent).i("n", same.len() as i64));
            self.wait(0.6);
            for t in same {
                if self.out(i) || !self.playing() {
                    break;
                }
                self.pay_rent(i, t, true)?;
            }
            return Ok(());
        }
        let money = self.w.st.players[i].money;
        let mut options = vec![];
        let mut labels = vec![];
        for &t in &same {
            if self.w.st.owners[t] < 0 {
                if self.can_pay(i) && money >= self.buy_price(t) {
                    let houses = self.w.st.houses[t];
                    options.push(t);
                    labels.push(
                        Msg::new("ask.agent.buy")
                            .tile("tile", t)
                            .n("price", self.buy_price(t))
                            .opt("extra", (houses > 0).then(|| Msg::new("ask.part.incl_houses").i("h", houses))),
                    );
                }
            } else if self.w.st.owners[t] as usize == i && self.why_not_build_on(i, t).is_none() && money >= self.build_cost(t) {
                options.push(t);
                labels.push(Msg::new("ask.agent.build").tile("tile", t).i("nth", self.w.st.houses[t] + 1).n("cost", self.build_cost(t)));
            }
        }
        if options.is_empty() {
            self.w.log("text", i as i32, Msg::new("log.agent_nothing").player_id("who", i).tile("agent", agent));
            return Ok(());
        }
        let ai = self.ai_agent_choice(i, &options);
        let ask = Ask::tile(
            i,
            Msg::new("ask.agent.title").tile("agent", agent),
            Msg::new("ask.agent.text").player_id("who", i),
            &options,
            labels,
        )
        .with_ai(|_| ai);
        let pick = self.ask(ask)?.of(i);
        let Some(&t) = usize::try_from(pick).ok().and_then(|p| options.get(p)) else {
            self.w.log("text", i as i32, Msg::new("log.agent_skip").player_id("who", i));
            return Ok(());
        };
        if self.w.st.owners[t] < 0 {
            if self.can_pay(i) && self.w.st.players[i].money >= self.buy_price(t) {
                self.buy(i, t)?;
            }
        } else if self.w.st.owners[t] as usize == i && self.why_not_build_on(i, t).is_none() && self.w.st.players[i].money >= self.build_cost(t) {
            self.build(i, t)?;
        }
        self.wait(0.6);
        Ok(())
    }

    /// `PayRent` -- property rent from the rent table; RiNG rent is
    /// rings owned x multiplier x 1d20; agents charge half.
    fn pay_rent(&mut self, i: usize, t: usize, half: bool) -> Flow<()> {
        let owner = self.w.st.owners[t];
        if owner < 0 || owner as usize == i || self.out(i) || self.out(owner as usize) {
            return Ok(());
        }
        let owner = owner as usize;
        if self.w.st.mortgaged[t] {
            return self.offer_force_buy(i, t);
        }
        let tile = self.tile(t);
        let (mut amount, detail) = if tile.kind == "ring" {
            let d = self.w.rng.d(20);
            let rings = self.count_rings(owner).max(1);
            let mult = self.data.match_rules.ring_multiplier.max(1);
            (rings * mult * d, Some(Msg::new("log.part.rent_ring").i("rings", rings).i("mult", mult).i("d", d)))
        } else {
            let h = self.w.st.houses[t];
            let rent = if tile.rent.is_empty() { 0 } else { tile.rent[(h as usize).min(tile.rent.len() - 1)] };
            (rent, (h > 0).then(|| Msg::new("log.part.rent_houses").i("h", h)))
        };
        let mut half_part = None;
        if half {
            let full = amount;
            amount = ((full as f64 / 2.0 / 10.0).ceil() as i32) * 10;
            half_part = Some(Msg::new("log.part.rent_half").n("full", full).n("cut", amount));
        }
        let note = match (detail, half_part) {
            (Some(d), h) => Some(d.opt("half", h)),
            (None, Some(h)) => Some(Msg::new("log.part.rent_half_only").msg("half", h)),
            (None, None) => None,
        };
        let mut p = Pay::new(amount, "rent");
        p.from = Some(i);
        p.to = Some(owner);
        p.typ = Some("rent");
        p.tile = Some(t);
        p.must = true;
        p.source = "src.rent";
        p.text = Some(Msg::new("log.rent").player_id("who", i).tile("tile", t).player_id("owner", owner).opt("note", note));
        self.money(p)?;
        self.wait(0.9);
        Ok(())
    }

    /// `OfferForceBuy` -- mortgaged land charges no rent but may be bought out at
    /// twice its value.
    fn offer_force_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        let owner = self.w.st.owners[t] as usize;
        let price = self.force_buy_price(t);
        if !self.can_pay(i) || self.w.st.players[i].money < price {
            self.w.log("text", i as i32, Msg::new("log.land_mortgaged").player_id("who", i).player_id("owner", owner).tile("tile", t).n("price", price));
            return Ok(());
        }
        let money = self.w.st.players[i].money;
        let ask = Ask::choice(
            vec![i],
            Msg::new("ask.force_buy.title"),
            Msg::new("ask.force_buy.text").player_id("who", i).player_id("owner", owner).tile("tile", t).n("price", price),
            vec![Msg::new("ask.force_buy.yes").n("price", price), Msg::new("ask.no_buy")],
            1,
            15.0,
        )
        .with_ai(|_| if money - price < 4000 { 1 } else { 0 })
        .with_tile(t);
        if self.ask(ask)?.of(i) == 0 && self.w.st.owners[t] == owner as i32 && !self.out(owner) && !self.out(i) && self.w.st.players[i].money >= price {
            self.w.st.players[i].money -= price;
            self.w.st.players[owner].money += price;
            self.w.st.owners[t] = i as i32;
            let text = Msg::new("log.force_buy").player_id("who", i).player_id("owner", owner).n("price", price).tile("tile", t);
            let e = self.w.log("forcebuy", i as i32, text);
            e.other = owner as i32;
            e.value = price;
            e.to = t as i32;
            self.wait(0.9);
        }
        Ok(())
    }

    /// `OfferBuy` (when a non-main move lands on unowned land).
    fn offer_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        let price = self.buy_price(t);
        if !self.can_pay(i) || self.w.st.players[i].money < price {
            self.w.log("text", i as i32, Msg::new("log.land_unowned_poor").player_id("who", i).tile("tile", t));
            return Ok(());
        }
        let houses = self.w.st.houses[t];
        let wants = self.ai_wants_buy(i, t);
        let ask = Ask::choice(
            vec![i],
            Msg::new("ask.buy.title"),
            Msg::new("ask.buy.text")
                .player_id("who", i)
                .tile("tile", t)
                .n("price", price)
                .opt("extra", (houses > 0).then(|| Msg::new("ask.part.incl_tile_houses").i("h", houses))),
            vec![Msg::new("ask.buy.yes"), Msg::new("ask.no_buy")],
            1,
            15.0,
        )
        .with_ai(|_| if wants { 0 } else { 1 })
        .with_tile(t);
        if self.ask(ask)?.of(i) == 0 && self.w.st.owners[t] < 0 && self.w.st.players[i].money >= self.buy_price(t) && !self.out(i) {
            self.buy(i, t)?;
        }
        Ok(())
    }

    /// `OfferBuild` (when a non-main move lands on own land).
    fn offer_build(&mut self, i: usize, t: usize) -> Flow<()> {
        if self.why_not_build_on(i, t).is_some() || self.w.st.players[i].money < self.build_cost(t) {
            let note = self.w.st.mortgaged[t].then(|| Msg::new("log.part.mortgaged"));
            self.w.log("text", i as i32, Msg::new("log.land_own").player_id("who", i).tile("tile", t).opt("note", note));
            return Ok(());
        }
        let cost = self.build_cost(t);
        let wants = self.ai_wants_build(i, t);
        let ask = Ask::choice(
            vec![i],
            Msg::new("ask.build.title"),
            Msg::new("ask.build.text").player_id("who", i).tile("tile", t).n("cost", cost),
            vec![Msg::new("ask.build.yes"), Msg::new("ask.build.no")],
            1,
            15.0,
        )
        .with_ai(|_| if wants { 0 } else { 1 })
        .with_tile(t);
        if self.ask(ask)?.of(i) == 0 && self.why_not_build_on(i, t).is_none() && self.w.st.players[i].money >= self.build_cost(t) {
            self.build(i, t)?;
        }
        Ok(())
    }

    // =============================================================== property

    /// `BuyPrice` -- land price plus houses already standing on it.
    pub(crate) fn buy_price(&self, t: usize) -> i32 {
        let tile = self.tile(t);
        tile.price + self.w.st.houses.get(t).copied().unwrap_or(0) * tile.house
    }

    pub(crate) fn build_cost(&self, t: usize) -> i32 {
        self.tile(t).house.max(0)
    }

    pub(crate) fn mortgage_value(&self, t: usize) -> i32 {
        self.tile(t).price / 2
    }

    /// 60% of the land price.
    pub(crate) fn redeem_cost(&self, t: usize) -> i32 {
        (self.tile(t).price as f64 * 0.6).round_ties_even() as i32
    }

    fn force_buy_price(&self, t: usize) -> i32 {
        let tile = self.tile(t);
        2 * (tile.price + self.w.st.houses[t] * tile.house)
    }

    fn count_rings(&self, player_id: usize) -> i32 {
        (0..self.data.tiles.len()).filter(|&t| self.tile(t).kind == "ring" && self.w.st.owners[t] == player_id as i32).count() as i32
    }

    /// Deeds a player could mortgage (not RiNG, not already mortgaged).
    pub(crate) fn mortgageable(&self, player_id: usize) -> Vec<usize> {
        (0..self.data.tiles.len())
            .filter(|&t| self.w.st.owners[t] == player_id as i32 && self.tile(t).is_buyable() && self.tile(t).kind != "ring" && !self.w.st.mortgaged[t])
            .collect()
    }

    /// `MortgageOrder` -- bare land first, cheapest first.
    fn mortgage_order(&self, mut deeds: Vec<usize>) -> Vec<usize> {
        deeds.sort_by_key(|&t| (self.w.st.houses[t] > 0, self.tile(t).price, t));
        deeds
    }

    /// `AutoMortgage` -- the AI's (and the time-out) selection.
    fn auto_mortgage(&self, player_id: usize, need: i32) -> Vec<String> {
        let mut got = 0;
        let mut out = vec![];
        for t in self.mortgage_order(self.mortgageable(player_id)) {
            if got >= need {
                break;
            }
            out.push(t.to_string());
            got += self.mortgage_value(t);
        }
        out
    }

    /// `BuyableHere` -- after a main move onto unowned land, before buying.
    pub(crate) fn buyable_here(&self, i: usize) -> bool {
        let st = &self.w.st;
        let pos = st.players[i].pos;
        st.step == 3
            && st.turn == i as i32
            && !st.bought
            && st.landed == pos
            && self.data.tiles.get(pos as usize).is_some_and(|t| t.is_buyable())
            && st.owners[pos as usize] < 0
            && self.can_pay(i)
    }

    pub(crate) fn can_buy_here(&self, i: usize) -> bool {
        self.buyable_here(i) && self.w.st.players[i].money >= self.buy_price(self.w.st.players[i].pos as usize)
    }

    pub(crate) fn can_build_here(&self, i: usize) -> bool {
        let st = &self.w.st;
        let pos = st.players[i].pos as usize;
        st.step == 3
            && st.turn == i as i32
            && !st.built
            && !st.bought
            && st.landed == pos as i32
            && self.why_not_build(i, pos).is_none()
            && st.players[i].money >= self.build_cost(pos)
    }

    /// `WhyNotBuild` -- building as part of resolving the main move.
    pub(crate) fn why_not_build(&self, i: usize, t: usize) -> Option<Msg> {
        let st = &self.w.st;
        if !self.data.tiles.get(t).is_some_and(|x| x.is_buyable()) || st.owners[t] != i as i32 {
            return Some(Msg::new("err.build_not_on_own"));
        }
        if st.turn == i as i32 && self.playing() && st.step == 3 && (st.landed != t as i32 || st.bought) {
            return Some(Msg::new("err.build_only_settling"));
        }
        self.why_not_build_on(i, t)
    }

    /// `WhyNotBuildOn`
    pub(crate) fn why_not_build_on(&self, i: usize, t: usize) -> Option<Msg> {
        let Some(tile) = self.data.tiles.get(t) else { return Some(Msg::new("err.build_not_own")) };
        let st = &self.w.st;
        if !tile.is_buyable() || st.owners[t] != i as i32 {
            return Some(Msg::new("err.build_not_own"));
        }
        if tile.kind == "ring" || tile.rent.len() < 2 {
            return Some(Msg::new("err.build_ring"));
        }
        if st.mortgaged[t] {
            return Some(Msg::new("err.build_mortgaged"));
        }
        if st.houses[t] as usize >= tile.rent.len() - 1 {
            return Some(Msg::new("err.build_full"));
        }
        if !self.can_pay(i) {
            return Some(Msg::new("err.cannot_spend"));
        }
        None
    }

    /// `BuyRoutine`
    pub(crate) fn buy(&mut self, i: usize, t: usize) -> Flow<()> {
        // `buyBefore` -- before any guard, so it is a real pre-hook (fires even
        // on the no-op/already-owned path).
        raise!(self, "buyBefore", i, tile = t as i32)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        if self.w.st.owners[t] >= 0 {
            return Ok(());
        }
        let mut price = self.buy_price(t);
        let houses = self.w.st.houses[t];
        if price > 0 {
            let mut p = Pay::new(price, "buy");
            p.from = Some(i);
            p.typ = Some("lose");
            p.tile = Some(t);
            p.source = "src.buy";
            p.text = Some(Msg::new("log.paid_for").player_id("who", i).tile("tile", t));
            let paid = self.money(p)?;
            if !paid.paid {
                return Ok(());
            }
            price = paid.loss;
        }
        self.w.st.owners[t] = i as i32;
        self.w.st.mortgaged[t] = false;
        let text = Msg::new("log.buy")
            .player_id("who", i)
            .tile("tile", t)
            .n("price", price)
            .opt("extra", (houses > 0).then(|| Msg::new("log.part.incl_houses").i("h", houses)));
        let e = self.w.log("buy", i as i32, text);
        e.value = 0;
        e.to = t as i32;
        // `bought` (Fx) -- C# `Each(Bought)`: the owner is set.
        raise!(self, "bought", i, tile = t as i32)?;
        // `buyAfter` -- the deed has changed hands.
        raise!(self, "buyAfter", i, tile = t as i32)?;
        Ok(())
    }

    /// `BuildRoutine`
    pub(crate) fn build(&mut self, i: usize, t: usize) -> Flow<()> {
        // `buildBefore` -- before any guard or payment, so it is a real pre-hook.
        raise!(self, "buildBefore", i, tile = t as i32)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        let mut cost = self.build_cost(t);
        if cost > 0 {
            let mut p = Pay::new(cost, "build");
            p.from = Some(i);
            p.typ = Some("lose");
            p.tile = Some(t);
            p.source = "src.build";
            p.text = Some(Msg::new("log.paid_for_house").player_id("who", i).tile("tile", t));
            let paid = self.money(p)?;
            if !paid.paid {
                return Ok(());
            }
            cost = paid.loss;
        }
        let full = self.tile(t).rent.len().saturating_sub(1) as i32;
        if self.w.st.owners[t] != i as i32 || self.w.st.houses[t] >= full {
            if cost > 0 && !self.out(i) {
                self.w.st.players[i].money += cost;
                let key = if self.w.st.owners[t] != i as i32 { "log.build_refund_not_owned" } else { "log.build_refund_full" };
                self.w.log("text", i as i32, Msg::new(key).player_id("who", i).tile("tile", t).n("cost", cost));
            }
            return Ok(());
        }
        self.w.st.houses[t] += 1;
        let h = self.w.st.houses[t];
        let text = Msg::new("log.build").player_id("who", i).tile("tile", t).i("nth", h).n("cost", cost);
        let e = self.w.log("build", i as i32, text);
        e.value = 0;
        e.to = t as i32;
        e.other = h;
        // `buildAfter` -- only after the house actually commits (not on the
        // refund path above).
        raise!(self, "buildAfter", i, tile = t as i32)?;
        Ok(())
    }

    /// `WhyNotMortgage`
    pub(crate) fn why_not_mortgage(&self, i: usize, t: i32, asking: bool) -> Option<Msg> {
        let st = &self.w.st;
        if !self.playing() {
            return Some(Msg::new("err.not_playing"));
        }
        let Some(tile) = usize::try_from(t).ok().and_then(|t| self.data.tiles.get(t)) else { return Some(Msg::new("err.not_your_deed")) };
        let t = t as usize;
        if !tile.is_buyable() || st.owners[t] != i as i32 {
            return Some(Msg::new("err.not_your_deed"));
        }
        if tile.kind == "ring" {
            return Some(Msg::new("err.mortgage_ring"));
        }
        if st.mortgaged[t] {
            return Some(Msg::new("err.already_mortgaged"));
        }
        if st.turn != i as i32 {
            return Some(Msg::new("err.mortgage_own_turn"));
        }
        if asking {
            return Some(Msg::new("err.busy"));
        }
        if st.step == 2 {
            return Some(Msg::new("err.mortgage_moving"));
        }
        if st.step == 3 && !self.pending_purchase(i) {
            return Some(Msg::new("err.mortgage_after_roll"));
        }
        None
    }

    /// `PendingPurchase` -- standing where a buy or build is still possible.
    fn pending_purchase(&self, i: usize) -> bool {
        let pos = self.w.st.players[i].pos;
        if self.w.st.landed != pos {
            return false;
        }
        if self.buyable_here(i) {
            return true;
        }
        !self.w.st.built && !self.w.st.bought && self.why_not_build(i, pos as usize).is_none()
    }

    /// `WhyNotRedeem`
    pub(crate) fn why_not_redeem(&self, i: usize, t: i32, asking: bool) -> Option<Msg> {
        let st = &self.w.st;
        if !self.playing() {
            return Some(Msg::new("err.not_playing"));
        }
        let Some(tile) = usize::try_from(t).ok().and_then(|t| self.data.tiles.get(t)) else { return Some(Msg::new("err.not_your_deed")) };
        let t = t as usize;
        if !tile.is_buyable() || st.owners[t] != i as i32 {
            return Some(Msg::new("err.not_your_deed"));
        }
        if !st.mortgaged[t] {
            return Some(Msg::new("err.not_mortgaged"));
        }
        if st.turn != i as i32 || st.step != 1 {
            return Some(Msg::new("err.redeem_phase"));
        }
        if asking {
            return Some(Msg::new("err.busy"));
        }
        if st.players[i].money < self.redeem_cost(t) {
            return Some(Msg::new("err.redeem_poor").n("cost", self.redeem_cost(t)));
        }
        None
    }

    /// `DoMortgage`
    fn do_mortgage(&mut self, i: usize, t: usize, why: Option<Msg>) {
        let v = self.mortgage_value(t);
        self.w.st.mortgaged[t] = true;
        self.w.st.players[i].money += v;
        let text = Msg::new("log.mortgage").player_id("who", i).tile("tile", t).n("n", v).opt("why", why.map(|w| Msg::new("log.part.why").msg("why", w)));
        let e = self.w.log("mortgage", i as i32, text);
        e.value = v;
        e.to = t as i32;
    }

    /// `MortgageRoutine`
    pub(crate) fn mortgage(&mut self, i: usize, t: usize, why: Option<Msg>) -> Flow<()> {
        // `mortgageBefore` -- fires before any guard, so it is a real pre-hook
        // (a reaction can block the mortgage). The post half below only fires
        // when the mortgage actually applied.
        raise!(self, "mortgageBefore", i, tile = t as i32)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        if self.w.st.owners[t] == i as i32 && !self.w.st.mortgaged[t] {
            self.do_mortgage(i, t, why);
            raise!(self, "mortgage", i, tile = t as i32)?;
        }
        Ok(())
    }

    /// `DoRedeem`
    pub(crate) fn redeem(&mut self, i: usize, t: usize) {
        let cost = self.redeem_cost(t);
        self.w.st.mortgaged[t] = false;
        self.w.st.players[i].money -= cost;
        let text = Msg::new("log.redeem").player_id("who", i).n("cost", cost).tile("tile", t);
        let e = self.w.log("redeem", i as i32, text);
        e.value = cost;
        e.to = t as i32;
    }

    // =============================================================== money

    /// `Money` -- move money between players and/or the bank. A mandatory payment
    /// the payer can't cover triggers `RaiseFunds` (and possibly bankruptcy).
    pub(crate) fn money(&mut self, p: Pay) -> Flow<Paid> {
        if p.amount <= 0 || (p.from.is_some() && p.from == p.to) {
            return Ok(Paid::default());
        }
        if p.from.is_some_and(|f| self.out(f)) || p.to.is_some_and(|t| self.out(t)) {
            return Ok(Paid::default());
        }
        // C# `TurnCtx.NoMoneyLoss` -- the payer's money cannot drop this turn.
        if let Some(f) = p.from.filter(|&f| self.w.money_locked(f as i32)) {
            self.w.log("text", f as i32, Msg::new("log.money_locked").player_id("who", f).n("amount", p.amount));
            return Ok(Paid::default());
        }
        if p.kind != "forcebuy" {
            if let Some(x) = p.from.filter(|&f| !self.can_pay(f)).or(p.to.filter(|&t| !self.can_pay(t))) {
                let text = Msg::new(if p.from.is_some() { "log.blocked_pay" } else { "log.blocked_gain" })
                    .player_id("who", x)
                    .msg("status", Msg::new(self.blocked(x)))
                    .n("amount", p.amount);
                self.w.log("text", x as i32, text);
                return Ok(Paid::default());
            }
        }
        // C# `Money` (25128-25234): the amount runs PayAdd -> PayMul -> PayChoose
        // -> PayAt -> the `pay` [反击] window *before* any money moves or funds
        // are raised. `player_id` is the payer and `target` the payee (-1 = the
        // bank); any of them may rewrite the amount (`set_pay_amount`).
        let payer = p.from.map_or(-1, |f| f as i32);
        let side = p.from.or(p.to).expect("a payment has a side");
        let rent = p.kind == "rent";
        let mut amount = p.amount;
        let mut to = p.to;
        for kind in ["payAdd", "payMul", "payChoose", "payAt"] {
            if amount <= 0 {
                break;
            }
            let t = raise!(self, kind, side, player_id = payer, target = to.map_or(-1, |t| t as i32), value = amount, by_card = p.by_card, pay_is_rent = rent, tile = p.tile.map_or(-1, |t| t as i32))?;
            amount = t.value.max(0);
        }
        // `pay` (pre) -- the [反击] window, on the payer. A reaction may rewrite
        // the amount (0 cancels) and/or the payee (`set_pay_target`, -1 = bank).
        if let Some(f) = p.from.filter(|_| amount > 0) {
            let t = raise!(self, "pay", f, target = to.map_or(-1, |t| t as i32), value = amount, by_card = p.by_card, pay_is_rent = rent, tile = p.tile.map_or(-1, |t| t as i32))?;
            amount = t.value.max(0);
            to = (t.target >= 0).then_some(t.target as usize);
        }
        // Cancelled: nothing moves and it is not paid (C# 25164 -- PayAfter
        // still runs, with `p.paid == false`).
        if amount <= 0 {
            raise!(self, "payAfter", side, player_id = payer, target = to.map_or(-1, |t| t as i32), value = 0, by_card = p.by_card, pay_is_rent = rent, tile = p.tile.map_or(-1, |t| t as i32))?;
            return Ok(Paid::default());
        }
        let mut loss = if p.from.is_some() { amount } else { 0 };
        if let Some(f) = p.from {
            if self.w.st.players[f].money < loss {
                if p.must {
                    self.raise_funds(f, loss, to)?;
                    if self.out(f) {
                        return Ok(Paid::default());
                    }
                } else {
                    loss = self.w.st.players[f].money.max(0);
                }
            }
            self.w.st.players[f].money -= loss;
        }
        // The payee is credited what the payer actually paid.
        let gain = if p.from.is_some() { loss } else { amount };
        if let Some(t) = to {
            if !self.out(t) {
                self.w.st.players[t].money += gain;
            }
        }
        self.log_money(&p, loss, gain);
        // `payAfter` (Fx) -- every settled payment (C# 25232), then the `paid`
        // [反击] window only when the payer lost money (25234).
        let moved = if p.from.is_some() { loss } else { gain };
        raise!(self, "payAfter", side, player_id = payer, target = to.map_or(-1, |t| t as i32), value = moved, by_card = p.by_card, pay_is_rent = rent, tile = p.tile.map_or(-1, |t| t as i32))?;
        if let Some(f) = p.from.filter(|_| loss > 0) {
            raise!(self, "paid", f, target = to.map_or(-1, |t| t as i32), value = loss, by_card = p.by_card, pay_is_rent = rent, tile = p.tile.map_or(-1, |t| t as i32))?;
        }
        Ok(Paid { paid: true, loss })
    }

    /// `LogMoney`
    fn log_money(&mut self, p: &Pay, loss: i32, gain: i32) {
        let typ = p.typ.unwrap_or(if p.kind == "rent" {
            "rent"
        } else if p.from.is_some() && p.to.is_some() {
            "pay"
        } else if p.from.is_some() {
            "lose"
        } else {
            "gain"
        });
        let amount = if p.from.is_some() { loss } else { gain };
        let mut text = match &p.text {
            Some(t) => t.clone().n("amount", amount),
            None => {
                let src = (!p.source.is_empty()).then(|| Msg::new("log.part.why").msg("why", Msg::new(p.source)));
                let m = match (p.from, p.to) {
                    (Some(f), Some(t)) => Msg::new("log.pay").player_id("who", f).player_id("to", t),
                    (Some(f), None) => Msg::new("log.lose").player_id("who", f),
                    (None, Some(t)) => Msg::new("log.gain").player_id("who", t),
                    (None, None) => return,
                };
                m.n("amount", amount).opt("src", src)
            }
        };
        if let (Some(_), Some(t)) = (p.from, p.to) {
            if gain != loss {
                text = Msg::new("log.with_received").msg("base", text).player_id("who", t).n("gain", gain);
            }
        }
        let player_id = p.from.or(p.to).map_or(-1, |s| s as i32);
        let other = if p.from.is_some() { p.to.map_or(-1, |t| t as i32) } else { -1 };
        let e = self.w.log(typ, player_id, text);
        e.other = other;
        e.value = amount;
        if let Some(t) = p.tile {
            e.to = t as i32;
        }
    }

    /// `RaiseFunds` -- mortgage deeds to cover `amount`, else go bankrupt.
    fn raise_funds(&mut self, i: usize, amount: i32, creditor: Option<usize>) -> Flow<()> {
        let need = amount - self.w.st.players[i].money;
        let deeds = self.mortgageable(i);
        if deeds.iter().map(|&t| self.mortgage_value(t)).sum::<i32>() < need {
            return self.bankrupt(i, creditor, amount);
        }
        let what = Msg::new("log.part.owe")
            .n("amount", amount)
            .opt("to", creditor.map(|c| Msg::new("log.part.owe_to").player_id("who", c)))
            .n("money", self.w.st.players[i].money);
        self.w.log("text", i as i32, Msg::new("log.raise_funds").player_id("who", i).msg("what", what.clone()));
        let ai = self.auto_mortgage(i, need);
        let text = Msg::new("ask.mortgage.text").msg("what", what).n("need", need);
        let reply = self.ask(Ask::mortgage(i, need, text, &deeds, ai))?;
        for id in &reply.a.picked {
            if let Ok(t) = id.parse::<usize>() {
                if t < self.data.tiles.len() && self.w.st.owners[t] == i as i32 && !self.w.st.mortgaged[t] {
                    self.mortgage(i, t, Some(Msg::new("src.raise_funds")))?;
                }
            }
        }
        while self.w.st.players[i].money < amount {
            let Some(&t) = self.mortgage_order(self.mortgageable(i)).first() else { break };
            self.do_mortgage(i, t, Some(Msg::new("src.raise_funds")));
        }
        self.wait(0.6);
        if self.w.st.players[i].money < amount && !self.out(i) {
            return self.bankrupt(i, creditor, amount);
        }
        Ok(())
    }

    /// `Bankrupt` -- everything is cashed in and handed to the creditor.
    fn bankrupt(&mut self, i: usize, creditor: Option<usize>, amount: i32) -> Flow<()> {
        // `bankruptBefore` -- before any asset cash-in; the post half below
        // fires after cash-in but before `remove_from_game`.
        raise!(self, "bankruptBefore", i, value = amount)?;
        if self.out(i) {
            return Ok(());
        }
        let mut cashed = 0;
        for t in self.mortgageable(i) {
            self.w.st.mortgaged[t] = true;
            cashed += self.mortgage_value(t);
        }
        self.w.st.players[i].money += cashed;
        let all = self.w.st.players[i].money.max(0);
        self.w.st.players[i].money = 0;
        let creditor = creditor.filter(|&c| !self.out(c));
        if let Some(c) = creditor {
            self.w.st.players[c].money += all;
        }
        self.w.st.players[i].bankrupt = true;
        self.w.out_count += 1;
        self.w.st.players[i].out_order = self.w.out_count;
        let text = Msg::new("log.bankrupt")
            .player_id("who", i)
            .n("amount", amount)
            .opt("cashed", (cashed > 0).then(|| Msg::new("log.part.cashed").n("n", cashed)))
            .n("all", all)
            .msg("to", creditor.map_or_else(|| Msg::new("log.part.consumed"), |c| Msg::new("log.part.paid_to").player_id("who", c)));
        let e = self.w.log("bankrupt", i as i32, text);
        e.other = creditor.map_or(-1, |c| c as i32);
        e.value = all;
        self.wait(1.6);
        raise!(self, "bankrupt", i)?;
        // `beforeOut` (Fx) -- C# `BeforeOut`, before the player is cleared.
        raise!(self, "beforeOut", i)?;
        let deeds = self.remove_from_game(i);
        if self.check_game_over() {
            return Err(Halt::ended());
        }
        self.auction_leftovers(deeds)
    }

    /// `Forfeit` -- leaving mid-match counts as going out.
    pub(crate) fn forfeit(&mut self, i: usize) -> Flow<()> {
        // `leaveBefore` -- at the very top, before any guard, so it is a real
        // pre-hook for the leave action itself.
        raise!(self, "leaveBefore", i)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        self.w.st.players[i].left = true;
        self.w.out_count += 1;
        self.w.st.players[i].out_order = self.w.out_count;
        self.w.log("left", i as i32, Msg::new("log.forfeit").player_id("who", i));
        // `beforeOut` (Fx) -- C# `BeforeOut`, before the player is cleared.
        raise!(self, "beforeOut", i)?;
        let deeds = self.remove_from_game(i);
        // `leaveAfter` -- after the player is cleared out, but before the
        // game-over check so it still fires when leaving ends the match.
        raise!(self, "leaveAfter", i)?;
        if self.check_game_over() {
            return Err(Halt::ended());
        }
        self.w.leftovers.push_back(deeds);
        if self.w.st.turn == i as i32 {
            self.w.next_turn_pending = true;
        }
        Ok(())
    }

    /// `RemoveFromGame` -- clear the player; its land returns to the bank.
    fn remove_from_game(&mut self, i: usize) -> Vec<usize> {
        let s = &mut self.w.st.players[i];
        s.ai = true;
        s.money = 0;
        for k in [
            key::STAY, key::STUN, key::STUN_START, key::EXILE,
            key::NO_HAND, key::FIRE, key::UNSTOPPABLE,
        ] {
            s.state_set(k, 0);
        }
        s.state_set(key::EXILE_TO, -1);
        self.w.hidden[i].hand.clear();
        self.w.extra_turns.retain(|&x| x != i);
        let deeds: Vec<usize> = (0..self.data.tiles.len()).filter(|&t| self.w.st.owners[t] == i as i32).collect();
        for &t in &deeds {
            self.w.st.owners[t] = -1;
            self.w.st.mortgaged[t] = false;
        }
        if !deeds.is_empty() {
            let houses = deeds.iter().any(|&t| self.w.st.houses[t] > 0);
            let key = if houses { "log.deeds_freed_houses" } else { "log.deeds_freed" };
            self.w.log("text", -1, Msg::new(key).player_id("who", i).i("n", deeds.len() as i64));
        }
        deeds
    }

    /// `AuctionLeftovers` -- auction up to 3 random deeds of a player who went out.
    pub(crate) fn auction_leftovers(&mut self, deeds: Vec<usize>) -> Flow<()> {
        let mut pool: Vec<usize> = deeds.iter().copied().filter(|&t| self.w.st.owners[t] < 0).collect();
        self.w.rng.shuffle(&mut pool);
        pool.truncate(3);
        if pool.is_empty() {
            return Ok(());
        }
        let names = pool.iter().map(|&t| Arg::Tile(t as i32)).collect();
        let more = (deeds.len() > pool.len()).then(|| Msg::new("log.part.auction_sample").i("n", deeds.len() as i64));
        self.w.log("text", -1, Msg::new("log.auction_leftovers").list("tiles", names).opt("more", more));
        self.wait(0.8);
        let from = self.w.st.turn.max(0) as usize;
        for t in pool {
            if !self.playing() {
                break;
            }
            if self.w.st.owners[t] < 0 {
                self.auction_tile(t, from)?;
            }
        }
        Ok(())
    }

    /// `AuctionTile`
    fn auction_tile(&mut self, t: usize, from: usize) -> Flow<()> {
        let players: Vec<usize> = self.present_from(from).into_iter().filter(|&s| self.can_pay(s)).collect();
        if players.is_empty() {
            self.w.log("text", -1, Msg::new("log.auction_nobody").tile("tile", t));
            return Ok(());
        }
        let base = self.buy_price(t);
        let worth: Vec<i32> = players
            .iter()
            .map(|&s| {
                let v = ((base as f64 * (0.6 + self.w.rng.f64() * 0.7) / 100.0) as i32) * 100;
                v.min(self.w.st.players[s].money - 1000)
            })
            .collect();
        let houses = self.w.st.houses[t];
        let text = Msg::new("ask.auction.text")
            .n("price", self.tile(t).price)
            .opt("extra", (houses > 0).then(|| Msg::new("ask.part.auction_houses").i("h", houses)));
        let ask = Ask::auction(t, players, Msg::new("ask.auction.title").tile("tile", t), text, worth);
        let r = self.ask(ask)?;
        if r.a.bidder < 0 {
            self.w.log("text", -1, Msg::new("log.auction_no_bid").tile("tile", t));
            return Ok(());
        }
        let (b, bid) = (r.a.bidder as usize, r.a.bid);
        if self.w.st.owners[t] >= 0 || !self.can_pay(b) || self.w.st.players[b].money < bid {
            self.w.log("text", b as i32, Msg::new("log.auction_void").player_id("who", b));
            return Ok(());
        }
        self.w.st.players[b].money -= bid;
        self.w.st.owners[t] = b as i32;
        self.w.st.mortgaged[t] = false;
        let text = Msg::new("log.auction_won")
            .player_id("who", b)
            .n("bid", bid)
            .tile("tile", t)
            .opt("extra", (houses > 0).then(|| Msg::new("ask.part.incl_houses").i("h", houses)));
        let e = self.w.log("buy", b as i32, text);
        e.value = bid;
        e.to = t as i32;
        // `bought` (Fx) -- C# AuctionTile 23276.
        raise!(self, "bought", b, tile = t as i32)?;
        self.wait(1.0);
        Ok(())
    }

    // =============================================================== cards

    /// `HandLimitOf` / `OverHand`
    pub(crate) fn over_hand(&self, i: usize) -> bool {
        self.w.hidden[i].hand.len() > HAND_LIMIT
    }

    /// `Draw` -- from the top of the pile; reshuffle the discard pile when empty.
    /// Bots discard down to the hand limit at random.
    pub(crate) fn draw(&mut self, i: usize, n: usize, log: bool) -> Flow<()> {
        if self.out(i) {
            return Ok(());
        }
        let mut got = 0;
        let mut drew: Vec<String> = Vec::new();
        let mut reshuffled = false;
        for _ in 0..n {
            if self.w.hidden[i].draw.is_empty() && !self.w.hidden[i].discard.is_empty() {
                let mut pile = std::mem::take(&mut self.w.hidden[i].discard);
                self.w.rng.shuffle(&mut pile);
                self.w.hidden[i].draw = pile;
                self.w.log("text", i as i32, Msg::new("log.reshuffle").player_id("who", i));
                reshuffled = true;
            }
            let Some(card) = self.w.hidden[i].draw.pop() else { break };
            self.w.hidden[i].hand.push(card.clone());
            drew.push(card);
            got += 1;
        }
        if log && got > 0 {
            let over = self.over_hand(i).then(|| Msg::new("log.part.over_hand").i("limit", HAND_LIMIT as i64));
            self.w.log("draw", i as i32, Msg::new("log.draw").player_id("who", i).i("n", got).opt("over", over)).value = got;
        }
        if self.w.st.players[i].ai && self.playing() {
            while self.over_hand(i) {
                let k = self.w.rng.below(self.w.hidden[i].hand.len());
                let card = self.w.hidden[i].hand[k].clone();
                self.discard(i, &card)?;
            }
        }
        if log {
            if reshuffled {
                // `reshuffled` (Fx) -- C# `Each(Reshuffled)` 19655.
                raise!(self, "reshuffled", i)?;
            }
            // `drawn` -- one raise per card still in hand, named on `t.card`, so
            // that card's own hook runs (C# AfterDraw 19660, a fresh instance).
            for id in &drew {
                if self.w.hidden[i].hand.contains(id) {
                    raise!(self, "drawn", i, card = id.clone(), value = got)?;
                }
            }
            // `drew` (Fx) -- C# `Each(Drew)` 19683: the player drew `got` cards.
            raise!(self, "drew", i, value = got, cards = drew.clone())?;
        }
        Ok(())
    }

    /// `DrawR`
    pub(crate) fn draw_r(&mut self, i: usize, n: usize, _why: &str) -> Flow<()> {
        if !self.out(i) && n > 0 {
            self.draw(i, n, true)?;
        }
        Ok(())
    }

    /// `Discard` -- over the hand limit.
    pub(crate) fn discard(&mut self, i: usize, card: &str) -> Flow<()> {
        // `discardBefore` -- before the card leaves the hand.
        raise!(self, "discardBefore", i, card = card.to_string())?;
        let h = &mut self.w.hidden[i];
        if let Some(k) = h.hand.iter().position(|c| c == card) {
            h.hand.remove(k);
        }
        h.discard.push(card.to_string());
        self.w.log("discard", i as i32, Msg::new("log.discard").player_id("who", i).i("limit", HAND_LIMIT as i64).card("card", card)).card = card.to_string();
        // `discarded` (Fx) -- C# `OnDiscarded` on that card, now in the pile.
        raise!(self, "discarded", i, card = card.to_string())?;
        // `discardAfter` -- the card is in the discard pile.
        raise!(self, "discardAfter", i, card = card.to_string())?;
        Ok(())
    }

    /// `CannotPlay`
    fn cannot_play(&self, i: usize) -> Option<&'static str> {
        let s = &self.w.st.players[i];
        if s.exile() > 0 {
            Some("err.play_exiled")
        } else if s.stunned() {
            Some("err.play_stunned")
        } else if s.no_hand() > 0 {
            Some("err.play_no_hand")
        } else {
            None
        }
    }

    /// `WhyNotPlayCard`
    pub(crate) fn cant_play(&self, i: usize, id: &str, asking: bool) -> Option<Msg> {
        if !self.w.hidden[i].hand.iter().any(|c| c == id) {
            return Some(Msg::new("err.no_such_card"));
        }
        if !(self.playing() && self.w.st.turn == i as i32) || self.w.st.step != 1 || asking {
            return Some(Msg::new("err.play_phase"));
        }
        if let Some(why) = self.cannot_play(i) {
            return Some(Msg::new(why));
        }
        if let Some(card) = self.data.card(id) {
            let character = &self.w.st.players[i].character;
            let base = self.data.base_character(character);
            let crychic = self.data.character(character).is_some_and(|c| c.band == "CRYCHIC");
            // CRYCHIC variants may not use the base character's exclusives.
            if card.exclusive() && card.owner != *character && (card.owner != base || crychic) {
                return Some(Msg::new("err.play_exclusive").chara("owner", card.owner.clone()));
            }
        }
        if !self.rules.normal(id) {
            return Some(Msg::new("err.play_timing"));
        }
        self.rules.cant_play(self, i, id)
    }

    /// `PlayFromHand` + `PlayCard`
    pub(crate) fn play_from_hand(&mut self, i: usize, id: &str) -> Flow<()> {
        let Some(k) = self.w.hidden[i].hand.iter().position(|c| c == id) else { return Ok(()) };
        self.w.hidden[i].hand.remove(k);
        self.w.log("play", i as i32, Msg::new("log.play").player_id("who", i).card("card", id)).card = id.to_string();
        if self.w.st.turn == i as i32 {
            self.w.turn.played.push(id.to_string());
        }
        self.wait(0.9);
        let t = raise!(self, "card", i, card = id.to_string(), by_card = Some(i as i32))?;
        let rules = self.rules;
        // `Trigger.Cancelled` -- the play is negated; the card still goes to its
        // Dest below but its effect body does not run.
        let dest = if t.cancelled {
            Dest::Graveyard
        } else {
            rules.play(self, i, id)?
        };
        match dest {
            Dest::Graveyard => {
                if !self.out(i) {
                    self.w.hidden[i].discard.push(id.to_string());
                    // `discarded` -- C# PlayCard 19872: before CardPlayed.
                    raise!(self, "discarded", i, card = id.to_string())?;
                }
            }
            Dest::Hand => self.w.hidden[i].hand.push(id.to_string()),
            Dest::Banished => {
                self.w.log("text", i as i32, Msg::new("log.card_removed").card("card", id));
            }
            Dest::Field => {}
        }
        // `cardAfter` -- the card's own effect is fully resolved and it has
        // landed wherever its `Dest` sent it.
        raise!(self, "cardAfter", i, card = id.to_string(), by_card = Some(i as i32))?;
        // `cardPlayed` (Fx) -- a card's hand effect resolved (its `Dest` is final).
        raise!(self, "cardPlayed", i, card = id.to_string(), by_card = Some(i as i32))?;
        Ok(())
    }

    // =============================================================== events

    /// `SetupEventDeck` -- every non-derived event, shuffled.
    fn setup_event_deck(&mut self) {
        let mut deck: Vec<String> = self.data.events.iter().filter(|e| !e.derived).map(|e| e.id.clone()).collect();
        self.w.rng.shuffle(&mut deck);
        self.w.event_deck = deck;
        self.w.event_discard.clear();
        self.w.event_removed.clear();
    }

    /// `DrawEvent` -- draw the top event and resolve it.
    fn draw_event(&mut self, i: usize) -> Flow<()> {
        if self.w.event_deck.is_empty() {
            if self.w.event_discard.is_empty() {
                self.w.log("text", i as i32, Msg::new("log.no_events"));
                return Ok(());
            }
            let mut deck = std::mem::take(&mut self.w.event_discard);
            self.w.rng.shuffle(&mut deck);
            self.w.event_deck = deck;
            self.w.log("text", -1, Msg::new("log.events_reshuffled"));
        }
        let id = self.w.event_deck.pop().expect("deck refilled above");
        self.w.log("event", i as i32, Msg::new("log.event").player_id("who", i).event("event", id.clone())).card = id.clone();
        self.wait(3.0);
        let t = raise!(self, "event", i, card = id.clone())?;
        let rules = self.rules;
        // `Trigger.Cancelled` -- the event's effect is negated; it is still
        // filed away below but does not resolve.
        let placed = if t.cancelled {
            false
        } else {
            rules.event(self, i, &id)?
        };
        if !placed {
            if self.data.event(&id).is_some_and(|e| e.derived) {
                self.w.event_removed.push(id.clone());
            } else {
                self.w.event_discard.push(id.clone());
            }
        }
        // `eventAfter` -- the event is fully resolved and filed away.
        raise!(self, "eventAfter", i, card = id)?;
        self.wait(0.4);
        Ok(())
    }

    /// Reaction window (C# `React`), delegated to the card rules.
    ///
    /// Every raise site funnels through here (via [`raise!`] / [`Self::raise`]),
    /// so `step` is stamped centrally from the live turn step rather than at
    /// each call site.
    fn react(&mut self, t: &mut Trigger) -> Flow<()> {
        t.step = self.w.st.step;
        let rules = self.rules;
        rules.react(self, t)
    }

    /// Build-and-raise a [`Trigger`], returning it as [`Self::react`] left it so
    /// a reaction's rewrite of a field (e.g. `moveRoll`'s reroll) can be read
    /// back. Prefer the [`raise!`] macro over calling this directly.
    fn raise(&mut self, mut t: Trigger) -> Flow<Trigger> {
        self.react(&mut t)?;
        Ok(t)
    }

    // =============================================================== end

    /// `CheckGameOver` -- one survivor, or every human out.
    pub(crate) fn check_game_over(&mut self) -> bool {
        let alive: Vec<usize> = (0..self.w.player_count()).filter(|&p| !self.out(p)).collect();
        if alive.len() <= 1 {
            self.finish("last", alive.first().copied());
            return true;
        }
        let humans: Vec<_> = self.w.st.players.iter().filter(|s| !s.bot).collect();
        if !humans.is_empty() && humans.iter().all(|s| s.out()) {
            self.finish("out", None);
            return true;
        }
        false
    }

    /// `Finish` -- score, rank, and end the match.
    ///
    /// score = money x w.money + land x w.property + houses x w.houses
    /// (land at its price, half if mortgaged; houses at build cost).
    /// Ranking: the champion first, then survivors by score, then players who
    /// went out, latest first.
    pub(crate) fn finish(&mut self, reason: &str, champion: Option<usize>) {
        if !self.playing() {
            return;
        }
        let st = &mut self.w.st;
        let n = st.players.len();
        for i in 0..n {
            let (mut land, mut houses) = (0, 0);
            for (t, tile) in self.data.tiles.iter().enumerate() {
                if st.owners[t] == i as i32 {
                    land += if st.mortgaged[t] { tile.price / 2 } else { tile.price };
                    houses += st.houses[t] * tile.house;
                }
            }
            let s = &mut st.players[i];
            s.assets = s.money + land + houses;
            s.score = (s.money as f32 * st.score_money + land as f32 * st.score_property + houses as f32 * st.score_houses).round() as i32;
        }
        let mut alive: Vec<usize> = (0..n).filter(|&p| !st.players[p].out()).collect();
        alive.sort_by_key(|&p| (std::cmp::Reverse(Some(p) == champion), std::cmp::Reverse(st.players[p].score), p));
        let mut gone: Vec<usize> = (0..n).filter(|&p| st.players[p].out()).collect();
        gone.sort_by_key(|&p| std::cmp::Reverse(st.players[p].out_order));
        let order: Vec<usize> = alive.into_iter().chain(gone).collect();
        for (r, &p) in order.iter().enumerate() {
            st.players[p].rank = r as i32 + 1;
        }
        st.end_reason = reason.into();
        st.winner = order.first().map_or(-1, |&p| p as i32);
        st.prompt = Default::default();
        st.vote = Default::default();
        st.busy = false;
        st.phase = "ended".into();
        st.turn = -1;
        self.w.next_turn_pending = false;
        self.w.leftovers.clear();
        let winner = self.w.st.winner;
        let why = match reason {
            "out" if self.w.st.mode == crate::MatchMode::Solo as i32 => Msg::new("end.out_solo"),
            "out" => Msg::new("end.out"),
            "vote" => Msg::new("end.vote"),
            "last" => Msg::new("end.last").player_id("who", winner),
            _ => Msg::new("end.score"),
        };
        self.w.log("text", -1, Msg::new("log.match_end").msg("why", why).player_id("winner", winner));
    }
}

#[cfg(test)]
mod tests;
