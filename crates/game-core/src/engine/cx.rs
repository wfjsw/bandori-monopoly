//! Routine context, flow control, and prompts.
//!
//! A routine is ordinary straight-line code over a [`Cx`]. When it needs a player
//! decision it calls [`Cx::ask`]. If the answer is already in this routine's answer
//! log, `ask` returns it; otherwise it halts the routine with [`Halt::Ask`]. The host
//! then shows the prompt, collects the answer, and re-runs the routine from the same
//! snapshot with the longer log -- deterministic replay instead of coroutines.

use serde::{Deserialize, Serialize};

use crate::data::{GameData, TileData};
use crate::msg::Msg;
use crate::state::MatchPrompt;

use super::rules::CardRules;
use super::world::World;

/// Why a routine stopped before finishing. Opaque to card rules: they only pass it
/// on with `?`.
#[derive(Debug)]
pub struct Halt(pub HaltKind);

#[derive(Debug)]
pub enum HaltKind {
    /// Needs a player decision. The partial world is shown; nothing is committed.
    Ask(Box<Ask>),
    /// The match ended mid-routine. Commit what happened so far.
    Ended,
}

impl Halt {
    pub(crate) fn ended() -> Self {
        Halt(HaltKind::Ended)
    }
}

pub type Flow<T> = Result<T, Halt>;

/// A prompt plus everything the host needs to answer it without the routine:
/// per-player AI answers (computed when the prompt was raised) and auction valuations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ask {
    pub view: MatchPrompt,
    /// AI answer for each entry in `view.players`.
    pub ai: Vec<i32>,
    /// AI card/deed selection for `pick` / `mortgage` prompts, per player.
    pub ai_picked: Vec<Vec<String>>,
    /// Auction: most each player's AI is willing to bid.
    pub worth: Vec<i32>,
}

impl Ask {
    fn new(kind: &str, players: Vec<usize>, title: Msg, text: Msg, time: f32) -> Self {
        let n = players.len();
        Self {
            view: MatchPrompt {
                kind: kind.into(),
                title,
                text,
                players: players.iter().map(|&s| s as i32).collect(),
                answers: vec![-1; n],
                time_left: time,
                ..MatchPrompt::default()
            },
            ai: vec![0; n],
            ai_picked: vec![vec![]; n],
            worth: vec![0; n],
        }
    }

    /// `Choice`: pick one of `options`.
    pub fn choice(
        players: Vec<usize>,
        title: Msg,
        text: Msg,
        options: Vec<Msg>,
        fallback: i32,
        time: f32,
    ) -> Self {
        let mut a = Self::new("choice", players, title, text, time);
        a.view.options = options;
        a.view.fallback = fallback;
        a.ai = vec![fallback; a.view.players.len()];
        a
    }

    /// `TileAsk`: pick one of `tiles` (answer == len means "none").
    pub fn tile(
        player_id: usize,
        title: Msg,
        text: Msg,
        tiles: &[usize],
        labels: Vec<Msg>,
    ) -> Self {
        let mut a = Self::new("tile", vec![player_id], title, text, 20.0);
        a.view.items = tiles.iter().map(|t| t.to_string()).collect();
        a.view.options = labels;
        a.view.fallback = tiles.len() as i32;
        a.ai = vec![a.view.fallback];
        a
    }

    /// `MortgageAsk`: choose deeds worth at least `need`.
    pub fn mortgage(
        player_id: usize,
        need: i32,
        text: Msg,
        deeds: &[usize],
        ai_pick: Vec<String>,
    ) -> Self {
        let mut a = Self::new(
            "mortgage",
            vec![player_id],
            Msg::new("ask.mortgage.title"),
            text,
            25.0,
        );
        a.view.items = deeds.iter().map(|t| t.to_string()).collect();
        a.view.bid = need;
        a.ai_picked = vec![ai_pick];
        a
    }

    /// `Auction`: open bidding on a tile.
    pub fn auction(
        tile: usize,
        players: Vec<usize>,
        title: Msg,
        text: Msg,
        worth: Vec<i32>,
    ) -> Self {
        let mut a = Self::new("auction", players, title, text, 10.0);
        a.view.tile = tile as i32;
        a.view.bid = 0;
        a.view.bidder = -1;
        a.worth = worth;
        a
    }

    pub fn with_ai(mut self, ai: impl Fn(usize) -> i32) -> Self {
        self.ai = self.view.players.iter().map(|&s| ai(s as usize)).collect();
        self
    }

    pub fn with_tile(mut self, t: usize) -> Self {
        self.view.tile = t as i32;
        self
    }

    pub fn with_kind(mut self, kind: &str) -> Self {
        self.view.kind = kind.into();
        self
    }
}

/// A completed prompt, as stored in the answer log.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Answered {
    /// Parallel to the prompt's players; never negative once complete.
    pub answers: Vec<i32>,
    pub picked: Vec<String>,
    pub bid: i32,
    pub bidder: i32,
}

/// What a routine gets back from [`Cx::ask`].
#[derive(Debug, Clone)]
pub struct Reply {
    pub players: Vec<i32>,
    pub fallback: i32,
    pub a: Answered,
}

impl Reply {
    /// `Ask.Answer(seat)`.
    pub fn of(&self, player_id: usize) -> i32 {
        match self.players.iter().position(|&s| s == player_id as i32) {
            Some(i) if self.a.answers.get(i).is_some_and(|&v| v >= 0) => self.a.answers[i],
            _ => self.fallback,
        }
    }
}

/// Routine context: the world being mutated, read-only game data, card rules, and
/// the replay cursor.
pub struct Cx<'a> {
    pub(crate) w: World,
    pub(crate) data: &'a GameData,
    pub(crate) rules: &'a dyn CardRules,
    answers: &'a [Answered],
    cursor: usize,
    /// Seconds of presentation time requested since the last answered prompt; the
    /// host waits this long before the next automatic step.
    pub(crate) delay: f32,
    /// How deep the `money()` pipeline is nested right now. A card hook that
    /// forces another money movement from inside a before/after money event
    /// re-enters `money()`; this bounds that, the way `MAX_COUNTERACT_DEPTH`
    /// bounds the chain. Transient (not serialized).
    pub(crate) money_depth: u32,
    /// Card uids whose hooks are currently running and must not re-trigger on
    /// their own movement (the termination argument for nested money). Transient.
    pub reentrant_hooks: Vec<i32>,
    /// The 移动起点 of the move currently being walked / teleported, or `-1`
    /// when no move is in flight. 「[经过]CiRCLE且[移动起点]不为CiRCLE」 reads it
    /// (`circle_reward`). Transient (not serialized) -- the walk sets it as it
    /// starts and the reward step is the only reader.
    pub(crate) move_start: i32,
}

impl<'a> Cx<'a> {
    /// Press a skill button: run the rule's `On::Play` entry under its own id.
    ///
    /// A skill is a card rule -- it just lives on the player's field rather than
    /// in a hand -- so the card's play entry answers for it. Nothing moves: there
    /// is no hand card to spend and no destination to resolve, unlike playing a
    /// card. The gate is not re-checked here; `why_not_act` has already asked the
    /// rule's `cant_play` and refused the press if it named a reason, which is
    /// the same shape as every other turn action.
    /// A copy of the replayable world (a rules host runs against one).
    pub fn world_copy(&self) -> World {
        self.w.clone()
    }

    /// Swap in the world a rules host produced; returns the previous one.
    pub fn swap_world(&mut self, w: World) -> World {
        std::mem::replace(&mut self.w, w)
    }

    /// Adopt the turn-ctx **policy** another world set up (build/buy discounts,
    /// free buy, fixed roll, ...) without taking its progress counters. A host
    /// routine runs against the live world and is not replayed, so a card's
    /// `set_build_discount` has to cross before `card_build` reads it; the
    /// replay re-derives the progress counters from the drive's snapshot.
    pub fn adopt_turn_policy(&mut self, from: &World) {
        let f = &from.turn;
        let t = &mut self.w.turn;
        t.build_discount = f.build_discount;
        t.build_discount_layers = f.build_discount_layers;
        t.build_cost_pct = f.build_cost_pct;
        t.buy_discount = f.buy_discount;
        t.free_buy = f.free_buy;
        t.raze_on_buy = f.raze_on_buy;
        t.fixed_roll = f.fixed_roll;
        t.extreme = f.extreme;
        t.play_from_hand = f.play_from_hand;
        t.no_money_loss = f.no_money_loss.clone();
        // The movement plan a card shaped just before pausing for a host
        // routine (`plan::set_pay_factor` / `set_rent_factor` / `set_can_build`
        // ) lives on the card's world copy; the routine
        // runs against the live world, so the knobs have to cross too --
        // otherwise 练习室里的风暴's (4-X)/4 and Repaint's 「支付减半」 are
        // dropped at the `SettleAt` / `Move` boundary.
        t.plan.pay_factor = f.plan.pay_factor;
        t.plan.rent_factor = f.plan.rent_factor;
        t.plan.can_build = f.plan.can_build;
        t.plan.no_buy = f.plan.no_buy;
        // A card body may latch a decision across the pauses its host routines
        // cause -- e.g. 黑色生日 freezing 「资金在1000以下」 at resolution so the
        // replay's re-run takes the same branch across its two [支付] entries.
        // That write lives on the card's world copy and is dropped when the run
        // pauses. Carry it over -- but *only* for keys a body explicitly
        // registers under the `latch.` prefix.
        //
        // TODO(规则书): a card's *other* state writes (a status clear, e.g. 壱雫空
        // zeroing [晕眩] and then paying) still do not reach the live world before
        // the host routine runs, so `can_pay` blocks the leg. A general carry is
        // **not** feasible in this replay model: the body re-runs from the top on
        // every pause, so an additive write (`give_stun(2)`) would land here and
        // again on the replay (4), and even a "clears only" carry breaks
        // `rb_general::parking_replaces_settle_with_stay`. The card-side
        // workaround is `gain_fixed` (see `rules/cards/card-mygo/src/hitoshizuku.rs`
        // `TODO(ABI)`); a real fix wants the host routine to see a merged view
        // that is not persisted (swap in the guest state for the routine's
        // duration, restore after).
        for (to, from_p) in self.w.st.players.iter_mut().zip(from.st.players.iter()) {
            for (k, v) in &from_p.state {
                if !k.starts_with("latch:") {
                    continue;
                }
                if !to.state.contains_key(k) {
                    to.state.insert(k.clone(), v.clone());
                }
            }
        }
    }

    /// The shared game data (tile/card lookups).
    pub fn game_data(&self) -> &'a GameData {
        self.data
    }

    /// The rules in force (a nested run may call back into them).
    pub fn rules(&self) -> &'a dyn CardRules {
        self.rules
    }

    pub fn new(
        w: World,
        data: &'a GameData,
        rules: &'a dyn CardRules,
        answers: &'a [Answered],
    ) -> Self {
        Self {
            w,
            data,
            rules,
            answers,
            cursor: 0,
            delay: 0.0,
            money_depth: 0,
            reentrant_hooks: Vec::new(),
            move_start: -1,
        }
    }

    /// Raise a prompt. Returns the logged answer, or halts the routine.
    pub fn ask(&mut self, mut ask: Ask) -> Flow<Reply> {
        self.w.ask_seq += 1;
        ask.view.id = self.w.ask_seq;
        if let Some(a) = self.answers.get(self.cursor) {
            self.cursor += 1;
            self.delay = 0.0;
            return Ok(Reply {
                players: ask.view.players,
                fallback: ask.view.fallback,
                a: a.clone(),
            });
        }
        self.w.st.prompt = ask.view.clone();
        Err(Halt(HaltKind::Ask(Box::new(ask))))
    }

    /// Presentation pause (C# `yield return <float>`).
    pub(crate) fn wait(&mut self, secs: f32) {
        self.delay += secs;
    }

    // ---- public surface for card rules ------------------------------------------

    pub fn data(&self) -> &GameData {
        self.data
    }

    pub fn state(&self) -> &crate::state::MatchState {
        &self.w.st
    }

    /// Append a log line.
    pub fn log(&mut self, player_id: i32, msg: Msg) {
        self.w.log("text", player_id, msg);
    }

    /// `H.Roll` -- `count` d`sides`, logged as a dice event.
    pub fn roll(&mut self, player_id: i32, count: i32, sides: i32, what: Option<Msg>) -> i32 {
        let faces: Vec<i32> = (0..count).map(|_| self.w.rng.d(sides)).collect();
        let sum = faces.iter().sum();
        let detail = (count > 1).then(|| {
            let joined = faces
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<_>>()
                .join("+");
            Msg::new("log.part.dice_faces").text("faces", joined)
        });
        let text = Msg::new("log.dice")
            .player_id("who", player_id)
            .i("count", count)
            .i("sides", sides)
            .opt("what", what.map(|w| Msg::new("log.part.why").msg("why", w)))
            .i("sum", sum)
            .opt("detail", detail);
        self.w.log("dice", player_id, text).value = sum;
        sum
    }

    pub fn tile_count(&self) -> usize {
        self.data.tiles.len()
    }

    // ---- shared queries ---------------------------------------------------------

    pub fn tile(&self, t: usize) -> &'a TileData {
        &self.data.tiles[t]
    }

    pub(crate) fn out(&self, i: usize) -> bool {
        self.w.out(i)
    }

    /// The match is in the play phase (C# `State.phase == "play"`).
    pub fn playing(&self) -> bool {
        self.w.st.phase == "play"
    }

    /// `CanPay`: not out, not stunned, not exiled.
    pub(crate) fn can_pay(&self, i: usize) -> bool {
        let s = &self.w.st.players[i];
        !s.out() && !s.stunned() && s.exile() == 0
    }

    /// `Blocked(i)` -- why a player can't move money.
    pub(crate) fn blocked(&self, i: usize) -> &'static str {
        let s = &self.w.st.players[i];
        if s.exile() > 0 {
            "status.exiled"
        } else {
            "status.stunned"
        }
    }

    /// Players starting at `from`, wrapping, that are in the game and not exiled
    /// (C# `From(from)` / `H.PresentFrom`).
    pub fn present_from(&self, from: usize) -> Vec<usize> {
        let n = self.w.player_count();
        (0..n)
            .map(|k| (from + k) % n)
            .filter(|&i| !self.out(i) && self.w.st.players[i].exile() == 0)
            .collect()
    }
}
