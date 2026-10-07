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
use super::world::{Signal, CIRCLE_MONEY, START_HAND, START_MONEY};
use crate::msg::{Arg, Msg};
use crate::state::{key, stage, Tick};

/// Safety cap on nested money pipelines. The rulebook (「[支付]时可以打出」)
/// has no depth limit, so nested money movements past any reasonable depth
/// should still open their [反击] windows. The real termination argument is
/// that a hook cannot re-trigger on its own movement (`Cx::reentrant_hooks`);
/// this is the runaway guard behind it and **traps loudly** rather than
/// settling silently. See `docs/ENGINE.md`.
pub(crate) const MAX_MONEY_DEPTH: u32 = 32;

/// A payment (C# `PayCtx`, the fields the shell uses).
///
/// One pipeline carries every money movement -- print (game -> player,
/// `from = None`), delete (player -> game, `to = None`) and pay-player (both
/// `Some`) -- whatever caused it (rent, card, skill, CiRCLE reward, buy,
/// build, ...). See [`Cx::money`].
#[derive(Debug, Clone)]
pub struct Pay {
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
pub struct Paid {
    pub paid: bool,
    /// What the payer lost (0 for a print).
    pub loss: i32,
    /// What the payee gained (0 for a delete).
    pub gain: i32,
}

impl Paid {
    /// The amount that actually moved, whichever side it left from.
    pub fn moved(&self) -> i32 {
        self.loss.max(self.gain)
    }
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
        // Tile rule instances on the neutral board owner (`docs/TILES.md`) --
        // one per board tile, like `bind_skills` per player. Idempotent, so a
        // `quick_start` that skipped the pick phase still gets them.
        self.w.bind_tiles(self.data, self.rules);
        let st = &mut self.w.st;
        st.round = 1;
        st.turn = -1;
        st.step = stage::NONE;
        self.w.log(
            "text",
            -1,
            Msg::new("log.match_start")
                .n("money", START_MONEY)
                .i("hand", START_HAND as i64),
        );
    }

    /// `Opening` -- the three match-start points, the opening deal, one mulligan.
    ///
    /// Ordered (the accepted match-start lifecycle):
    ///
    /// 1. **before match start** (`deckBeforeGame`), before the opening hands
    ///    are drawn: start positions and the authoritative **initial hand size**
    ///    (default 2; effects may lower it, minimum 0) are decided here.
    /// 2. the opening deal and mulligan, reading that hand size.
    /// 3. **after match start** (`deckAtGameStart`), after the deal: initial
    ///    tokens/resources (fire pots 「初始N」, P✽P fans) are created here.
    ///
    /// Each point is dispatched to **every effect source** -- field cards
    /// including skills (per player, in field order) and the card ids in that
    /// player's piles/hands -- not just to ids found in piles. (`WasmRules::
    /// game_start_hooks` does the per-source walk; the engine raises once per
    /// player per point.) There is no separate "match started" point: nothing
    /// in the pool wants "after positions are set, before the deal".
    ///
    /// Opening hands do **not** raise the per-draw points (`drewBefore` /
    /// `drawn` / `drew`): 「抽卡」 in the pool means a draw during play, and
    /// 朝同一片天空迈进's 「开局时抽到此卡洗回」 is spelled as a game-start
    /// clause for exactly that reason. A card that wants the opening deal has
    /// `deckAtGameStart`.
    pub(crate) fn opening(&mut self) -> Flow<()> {
        // The authoritative initial hand size: default 2, lowered by before-start
        // effects (「初始手牌减1」), floored at 0.
        for i in 0..self.w.player_count() {
            self.w.st.players[i].state_set(key::START_HAND, START_HAND as i32);
        }
        // 1. before match start -- start positions and the initial hand size.
        for i in 0..self.w.player_count() {
            raise!(self, "deckBeforeGame", i)?;
        }
        // The opening deal, per player's own hand size.
        for i in 0..self.w.player_count() {
            let n = self.start_hand(i);
            self.draw(i, n, false)?;
        }
        self.wait(3.0);
        let humans: Vec<usize> = (0..self.w.player_count())
            .filter(|&p| !self.w.st.players[p].ai)
            .collect();
        if !humans.is_empty() {
            let n = humans
                .iter()
                .map(|&p| self.start_hand(p))
                .max()
                .unwrap_or(START_HAND);
            self.w.log(
                "text",
                -1,
                Msg::new("log.mulligan_offer").i("n", n as i64),
            );
            let ask = Ask::choice(
                humans.clone(),
                Msg::new("ask.mulligan.title"),
                Msg::new("ask.mulligan.text").i("n", n as i64),
                vec![Msg::new("ask.mulligan.keep").i("n", n as i64), Msg::new("ask.mulligan.redo")],
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
        // 3. after match start -- initial tokens and resources.
        for i in 0..self.w.player_count() {
            raise!(self, "deckAtGameStart", i)?;
        }
        self.w.next_turn_pending = true;
        Ok(())
    }

    /// The player's authoritative opening hand size (default 2, minimum 0).
    pub fn start_hand(&self, i: usize) -> usize {
        self.w
            .st
            .players
            .get(i)
            .map(|p| p.state_get(key::START_HAND).clamp(0, 32) as usize)
            .unwrap_or(START_HAND)
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
        let n = self.start_hand(i);
        self.draw(i, n, false)?;
        self.w.log(
            "mulligan",
            i as i32,
            Msg::new("log.mulligan")
                .player_id("who", i)
                .i("n", n as i64),
        );
        Ok(())
    }

    /// `NextTurnRoutine` + `TurnStart`.
    pub(crate) fn next_turn(&mut self) -> Flow<()> {
        self.w.next_turn_pending = false;
        let n = self.w.player_count();
        let turn = self.w.st.turn;
        let mut next = turn;
        let extra =
            turn >= 0 && self.w.extra_turns.contains(&(turn as usize)) && !self.out(turn as usize);
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
        st.step = stage::START;
        st.bought = false;
        st.built = false;
        st.skip_move = false;
        st.landed = -1;
        st.roller = next;
        self.w.turn = super::world::TurnCtx {
            player_id: i,
            extra,
            ..Default::default()
        };
        self.w.signals.push(Signal::TurnBegan);
        let text = Msg::new(if extra { "log.turn_extra" } else { "log.turn" })
            .i("round", self.w.st.round)
            .player_id("who", i);
        self.w.log("turn", next, text);
        self.turn_start(i)
    }

    /// `TurnStartInner` -- status effects tick, then the clock starts.
    fn turn_start(&mut self, i: usize) -> Flow<()> {
        if self.out(i) {
            return Ok(());
        }
        // C# `_targeted[i] = 0` -- the between-turns target counter. Each
        // player's own turn start zeroes its entry (「本回合被其他玩家的卡[指定]
        // 过」 counts since its own turn last started).
        if let Some(n) = self.w.targeted.get_mut(i) {
            *n = 0;
        }
        // C# `_abnormalTurn` -- 「a new turn starts them all at 0」. The turn ctx
        // is rebuilt at `next_turn`, so this is already empty there; zero it
        // again at the turn-start boundary so a world carried across a card's
        // run cannot leave a stale hit behind.
        self.w.turn.abnormal.clear();
        // `turnStartBefore` -- before any exile/stun status ticks resolve, so a
        // counteraction sees the turn exactly as it was left last turn.
        raise!(self, "turnStartBefore", i, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        if self.w.st.players[i].exile() > 0 {
            self.w.st.players[i].state_add(key::EXILE, -1);
            let left = self.w.st.players[i].exile();
            if left > 0 {
                self.w.log(
                    "status",
                    i as i32,
                    Msg::new("log.exiled_skip")
                        .player_id("who", i)
                        .i("left", left),
                );
                self.wait(1.0);
                return self.end_turn(i);
            }
            let to = self.w.st.players[i].exile_to();
            self.w.st.players[i].state_set(key::EXILE_TO, -1);
            self.w.log(
                "status",
                i as i32,
                Msg::new("log.exile_over").player_id("who", i),
            );
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
        // `_turnSnap[i]` -- 「在Livehouse地块开始回合时」 is about the starting
        // square, and 「回到起始地点并取消所有受到的效果」 restores from here.
        self.w.turn.turn_start_pos = self.w.st.players.iter().map(|p| p.pos).collect();
        self.w.turn.turn_snap = self
            .w
            .st
            .players
            .iter()
            .map(|p| super::world::TurnSnap {
                pos: p.pos,
                stay: p.stay(),
                stun: p.stun(),
                exile: p.exile(),
            })
            .collect();
        raise!(self, "turnStart", i, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        self.w.signals.push(Signal::StartTimer(i));
        if self.w.st.players[i].stunned() {
            self.w.log(
                "status",
                i as i32,
                Msg::new("log.stunned_skip").player_id("who", i),
            );
            self.w.st.step = stage::END;
            self.wait(1.2);
            return self.end_turn(i);
        }
        // 「不可阻挡」 -- a player who cannot be stopped walks through a [停留]
        // rather than losing the move to it.
        let unstoppable = self.w.st.players[i].state_get(key::UNSTOPPABLE) > 0;
        if self.w.st.players[i].stay() > 0 && !unstoppable {
            self.w.st.skip_move = true;
            self.w.log(
                "status",
                i as i32,
                Msg::new("log.stay_skip").player_id("who", i),
            );
        }
        // 开始阶段 is over: status has ticked, `turnStart` has fired, and the
        // stun/stay checks have run. The stage *stays* 开始 for a beat first,
        // though -- exactly as 结束 holds after the walk -- so the stage swap
        // has time to read before 运营 opens. `tick_play` lifts the turn into
        // 运营 once this delay expires. (A stunned player left above, so 眩晕
        // skips 运营 and 移动, as the rulebook says.)
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
        self.w.log(
            "text",
            i as i32,
            Msg::new("log.end_turn").player_id("who", i),
        );
        self.end_turn(i)
    }

    /// `EndTurnRoutine` -- status effects wear off, next turn queued.
    pub(crate) fn end_turn(&mut self, i: usize) -> Flow<()> {
        self.w.st.step = stage::END;
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
        // 「并在回合结束时额外进行一次[触发结算]」 is no longer an engine
        // counter: it is a scheduled turn-end rule op (`On::AtEnd`, whose body
        // calls `ctx::settle`) -- `docs/TILES.md`.
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
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.main_move_used").player_id("who", i),
            );
            return Ok(());
        }
        if is_turn {
            self.w.turn.main_moved = true;
        }
        plan.player_id = i;
        plan.roller = i;
        plan.main = is_turn;
        // Only the turn player's move drives the stage machine; a card moving
        // somebody else must not leave `step` parked in MOVE.
        if is_turn {
            self.w.st.step = stage::MOVE;
        }
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
                    None => self.roll_tables(&plan),
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
            self.w.turn.main_steps = if plan.kind == MoveKind::Teleport {
                0
            } else {
                plan.total
            };
        }
        self.w.st.plan = plan.to_plan();
        if plan.kind != MoveKind::Teleport {
            self.w.set_slot(i as i32, "lastWalk", plan.total + 1);
        }
        if is_turn {
            self.w.st.step = stage::END;
        }
        self.wait(1.2);
        Ok(())
    }

    /// Press a skill button: run the rule's `On::Play` entry under its own id.
    ///
    /// A skill is a card rule -- it just lives on the player's field rather than
    /// in a hand -- so the card's play entry answers for it. Nothing moves: there
    /// is no hand card to spend and no destination to resolve, unlike playing a
    /// card. The gate is not re-checked here; `why_not_act` has already asked the
    /// rule's `cant_play` and refused the press if it named a reason, which is
    /// the same shape as every other turn action.
    pub fn use_skill(&mut self, player_id: usize, card: &str) -> Flow<()> {
        // `skillUsed` (Fx.SkillUsed) -- 「使用自己原有的技能（2）时」. Raised
        // before the body so a listener sees the use, not its aftermath -- and so
        // a [反击] answering it can negate the press before anything happens.
        let mut t = super::rules::Trigger::new("skillUsed", player_id);
        t.card = card.to_string();
        // The skill id rides `cards` too (the `drew` pattern), so a listener can
        // read the id it is reacting to (`trigger::cards`) and name a mark after
        // it -- 广町七深（2）「得到一个该角色的标记」.
        t.cards = vec![card.to_string()];
        t.by_card = Some(player_id as i32);
        let t = self.raise(t)?;
        // `Trigger.Cancelled` -- the press is negated (花园多惠（2）「将其抵消」);
        // the body does not run, exactly as `play_from_hand` treats a cancelled
        // `card` link above.
        if t.is_cancelled() {
            return Ok(());
        }
        let rules = self.rules;
        rules.play(self, player_id, card)?;
        Ok(())
    }

    /// `H.DoMoveRoll` -- sum the move's dice tables into one face, without the
    /// `rollAfter` / `moveRoll` raise points. A card that re-rolls an in-flight
    /// move (「放弃第一次的结果重骰一次」) is usually *inside* one of those hooks,
    /// so re-raising them would recurse; the plain table roll is what it wants.
    pub fn do_move_roll(&mut self, plan: &Move) -> i32 {
        self.roll_tables(plan)
    }

    /// `H.SettleAt` -- a full [触发结算] of `tile`, wherever the player is standing.
    /// The player does not move; the tile's own effect resolves.
    pub fn card_settle_at(&mut self, player_id: usize, tile: usize, main: bool) -> Flow<()> {
        if self.out(player_id) || !self.playing() {
            return Ok(());
        }
        let mut m = self.w.turn.plan.clone();
        m.player_id = player_id;
        m.main = main;
        self.settle_at(player_id, tile, &m)?;
        self.wait(0.6);
        Ok(())
    }

    /// `H.BuyRoutine` -- the purchase itself. No tile-kind guard beyond the
    /// engine's own: a card that says 「必须购买」 has already decided the tile is
    /// buyable.
    pub fn card_buy(&mut self, player_id: usize, tile: usize) -> Flow<()> {
        if self.out(player_id) || !self.playing() {
            return Ok(());
        }
        self.buy(player_id, tile)
    }

    /// `H.BuildRoutine` -- pay the tile's build cost and raise one house. Refuses
    /// (with the engine's own reason) when the tile cannot take a house.
    pub fn card_build(&mut self, player_id: usize, tile: usize) -> Flow<()> {
        if self.out(player_id) || !self.playing() {
            return Ok(());
        }
        if let Some(why) = self.why_not_build_on(player_id, tile) {
            self.w.log("text", player_id as i32, why);
            return Ok(());
        }
        self.build(player_id, tile)
    }

    /// `H.OfferBuildAmong` -- prompt to build on one of `tiles`, then build there.
    /// `why` names the effect (shown on the prompt). Skips silently when nothing
    /// in the list can take a house.
    pub fn card_offer_build(&mut self, player_id: usize, tiles: &[usize], why: &str) -> Flow<()> {
        if self.out(player_id) || !self.playing() {
            return Ok(());
        }
        let options: Vec<usize> = tiles
            .iter()
            .copied()
            .filter(|&t| {
                self.why_not_build_on(player_id, t).is_none()
                    && self.w.st.players[player_id].money >= self.build_cost(t)
            })
            .collect();
        if options.is_empty() {
            return Ok(());
        }
        let mut labels = Vec::with_capacity(options.len());
        for &t in &options {
            labels.push(
                Msg::new("ask.build.tile")
                    .tile("tile", t)
                    .i("nth", self.w.st.houses[t] + 1)
                    .n("cost", self.build_cost(t)),
            );
        }
        let ask = Ask::tile(
            player_id,
            Msg::new("ask.build.title"),
            Msg::new("ask.build.among")
                .player_id("who", player_id)
                .text("why", why)
                .i("n", options.len() as i64),
            &options,
            labels,
        );
        let pick = self.ask(ask)?.of(player_id);
        let Some(&t) = usize::try_from(pick).ok().and_then(|p| options.get(p)) else {
            return Ok(());
        };
        self.build(player_id, t)
    }

    /// `H.MortgageRoutine` -- mortgage one of the player's deeds, forced by a
    /// card effect. This is not a user asking while a card is busy (`asking`),
    /// so the "busy" refusal does not apply; the ownership/validity gates still
    /// do.
    pub fn card_mortgage(&mut self, player_id: usize, tile: usize) -> Flow<()> {
        if self.out(player_id) || !self.playing() {
            return Ok(());
        }
        if let Some(why) = self.why_not_mortgage(player_id, tile as i32, false) {
            self.w.log("text", player_id as i32, why);
            return Ok(());
        }
        self.mortgage(player_id, tile, None)
    }

    /// `MainMove` -- the turn's main roll-and-move.
    pub(crate) fn main_move(&mut self, i: usize, roller: usize) -> Flow<()> {
        self.w.st.step = stage::MOVE;
        if self.w.turn.main_moved {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.main_move_used").player_id("who", i),
            );
            self.w.st.step = stage::END;
            return Ok(());
        }
        self.w.turn.main_moved = true;
        // Start from the turn's plan, not a fresh move: a **Play** body may have
        // shaped this move earlier in the turn (change_world's 「你的本次移动掷骰
        // 变为3d20」 writes the dice table at play time). C# `MoveCtx m =
        // _turnCtx.Plan`. Only the identity and the walk's *progress* are
        // forced/ cleared here -- everything a card shaped carries over.
        let mut m = self.w.turn.plan.clone();
        m.player_id = i;
        m.roller = roller;
        m.main = true;
        m.from = 0;
        m.to = 0;
        m.path.clear();
        m.total = 0;
        m.remaining = 0;
        m.passed_players.clear();
        m.stopped = false;
        m.cancelled = false;
        if !self.out(i) {
            // `roll` (pre) -- before the d20 is cast. `value = -1` is the
            // sentinel meaning "no roll yet": the bridge would otherwise
            // derive `move_roll` from `value` and let roll-counteracting cards
            // (which match `Roll | MoveRoll`) fire before any dice exist.
            raise!(self, "roll", i, @m m, value = -1)?;
            if self.out(i) || !self.playing() {
                self.w.st.step = stage::END;
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
                self.w.st.step = stage::END;
                return Ok(());
            }
            // C# `DoMoveRoll` + `TurnCtx.Plan.FixedRoll`: a stored fixed face
            // replaces the roll, otherwise sum the `base` + `dice` tables
            // (default 1d20; `sides == 0` is a flat `count`).
            m.roll = match self.w.turn.fixed_roll {
                Some(n) => n,
                None => self.roll_tables(&m),
            };
            // `rollAfter` (Fx) -- C# `Each(RollAfter)` runs on the fresh roll,
            // *before* the moveRoll [反击] window; a field card may rewrite it
            // (`set_move_roll`), which is how a stored boost lands.
            let t = raise!(self, "rollAfter", i, @m m, value = m.roll)?;
            m.roll = t.value.max(0);
            let t = raise!(self, "moveRoll", i, @m m, value = m.roll)?;
            // A [反击] may have rerolled the dice (C# shares `t.Move` with the
            // counteractions): the face the walk uses is the one left on the trigger.
            m.roll = t.value.max(0);
            // C# `NextStepsFx.MoveBefore` -- a stored step count overrides the
            // roll for this one main move.
            if let Some(steps) = self.w.hidden[i].next_steps.take() {
                m.roll = steps.max(0);
            }
            // C# `RollMove`: `if (!m.Signed) m.Roll = max(m.MinRoll, m.Roll)` --
            // the clamp applies to the final face after the counteractions, so a
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
        self.w.st.step = stage::END;
        self.wait(1.2);
        Ok(())
    }

    /// `DoMoveRoll` -- sum the move's `base` + `dice` tables into one face.
    ///
    /// Each term is `count`d`sides`, or a flat `count` when `sides <= 0` (see
    /// [`crate::engine::move_ctx::Roll`]). This is what makes a Play body's
    /// 「掷骰变为3d20」 (`set_base_dice(3, 20)`) actually roll three dice.
    fn roll_tables(&mut self, m: &Move) -> i32 {
        let mut total = 0;
        for t in m.base.iter().chain(m.dice.iter()) {
            if t.sides <= 0 {
                total += t.count;
            } else {
                for _ in 0..t.count.max(0) {
                    total += self.w.rng.d(t.sides.max(1));
                }
            }
        }
        total
    }

    /// `WalkMove` -- step tile by tile; passing CiRCLE pays the reward.
    fn walk(&mut self, m: &mut Move) -> Flow<()> {
        let i = m.player_id;
        let n = self.data.tiles.len() as i32;
        // `Start` -- 「此次移动以X为起点（不触发起点地块效果）」: the walk begins
        // at `start` rather than where the player stands, and the piece is moved
        // there with no landing effect (C# `m.Start` / `Plan.Start`).
        // `m.from` is the 移动起点 either way -- the [经过] CiRCLE reward's
        // 「[移动起点]不为CiRCLE」 clause reads it.
        if m.start >= 0 {
            self.w.st.players[i].pos = m.start;
            m.from = m.start;
        } else {
            m.from = self.w.st.players[i].pos;
        }
        self.move_start = m.from;
        // C# `WalkMoveSteps`: `steps = (m.Steps >= 0 ? m.Steps : m.Roll)`.
        let steps = if m.steps >= 0 { m.steps } else { m.roll };
        // Snapshot the log-line fields: the loop below writes `m.total` /
        // `m.remaining` / `m.path`, so the closure must not borrow `m`.
        let (main, roller, why, dir, resolve) = (m.main, m.roller, m.why.clone(), m.dir(), m.resolve);
        let head = move |still: bool| {
            let base = if main {
                Msg::new("log.roll").opt(
                    "by",
                    (roller != i).then(|| Msg::new("log.part.rolled_by").player_id("who", roller)),
                )
            } else {
                Msg::new(if dir < 0 {
                    "log.move_back"
                } else {
                    "log.move_forward"
                })
                .opt("why", why.clone().map(|w| Msg::new("log.part.why").msg("why", w)))
            };
            base.player_id("who", i)
                .i("n", steps)
                .opt("nores", (!resolve).then(|| Msg::new("log.part.no_resolve")))
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
            // 行动阶段 13 (`E14`) 「原地[传送]/移动(移动0格)时触发[重叠]」: a
            // 0-step move still raises [重叠] at the tile it stands on.
            // TODO(规则书): `E14` names only [重叠] for the 0-step case; whether
            // [经过] (`passTile`) also fires here is not stated -- `B41`+`E13`
            // give the general rule for a *teleport* and read as both-fire, but
            // `E14` is the only cell that names the 0-move and it names only
            // [重叠]. Implemented as `E14` writes it: [重叠] only.
            self.overlap_at(m, pos as usize)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
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
            // `MoveCtx.Total` / `Remaining` / `Path` -- the walk's length, how
            // much of it is left, and the tiles it visits (`to_plan`'s `reach`).
            // `total` is the planned length (it tracks `ExtraSteps` mid-walk);
            // `remaining` counts the steps after the tile being entered, so a
            // 「经过且未触发结算」 filter (`Remaining > 0`) sees the landing as 0.
            let planned = (steps.max(0) as usize) + m.extra_steps.max(0) as usize;
            let remaining = planned - k - 1;
            m.total = planned as i32;
            m.remaining = remaining as i32;
            let at = next as usize;
            let passes_circle = self.tile(at).kind == "circle";
            let last = remaining == 0;
            // `passBefore` -- the glossary's 「[经过]X」 is any tile on the move
            // path, so this fires for every step (not just CiRCLE / the end).
            raise!(self, "passBefore", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            if self.w.st.players[i].pos != cur {
                break; // moved by a passBefore counteraction
            }
            self.w.st.players[i].pos = next;
            m.path.push(next);
            // `passTile` (Fx) -- the player has stepped onto this tile.
            raise!(self, "passTile", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            if passes_circle || last {
                let text = if first {
                    head(false)
                } else {
                    Msg::new("log.move_on").player_id("who", i)
                };
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
                    // The [经过] CiRCLE reward is `tile:circle`'s **Pass entry**
                    // (`docs/TILES.md`), dispatched through the `passTile` hook
                    // the raise above already fired. With no rule instance bound
                    // on this tile (`StubRules`, or a kind not migrated) the
                    // built-in reward runs here instead -- the same fallback
                    // shape as `CardRules::settle_tile` -> `land_at_built_in`.
                    if self.w.tile_rule_instances(at as i32).is_empty() {
                        self.circle_reward(i, m.resolve && last, m.from)?;
                    }
                }
            }
            // `pass` -- every traversed tile, like `passBefore` / `passTile`
            // (glossary 「[经过]」). `passTile` is the Fx hook kind for the same
            // walk; a card uses one or the other, never both.
            raise!(self, "pass", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            if self.w.st.players[i].pos != next {
                break; // moved by an effect
            }
            // `StopAt` -- 「强制停下」. A hook writes the *plan* (`plan::set_stop_at`),
            // and the walk runs on its own `Move`; pull that write in and stop here.
            if self.w.turn.plan.stop_at >= 0 {
                m.stop_at = self.w.turn.plan.stop_at;
            }
            if self.w.turn.plan.resolve != m.resolve {
                m.resolve = self.w.turn.plan.resolve;
            }
            if m.stop_at >= 0 && next == m.stop_at {
                m.stopped = true;
                break;
            }
            k += 1;
        }
        // 行动阶段 13 (`E13`/`E14`) 「移动终点触发[重叠]」: once the walk is over,
        // [重叠] fires at the tile the mover ended on -- including a walk that
        // stopped early (「强制停下」). Not mid-walk: `B40` is 「移动后」.
        if !self.out(i) && self.playing() {
            let at = self.w.st.players[i].pos as usize;
            self.overlap_at(m, at)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
        }
        self.after_walk(m)
    }

    /// `TeleportMove` / `Teleport`.
    ///
    /// A teleport is 「[路径]只包括[移动终点]的移动动作」 (专有名词 9), so its
    /// pass handling runs **once at the destination** -- not per step.
    /// 回合階段&註釋 `B41` 「传送：空降至目标地格并在该格依次触发[经过],[重叠],
    /// 和[结算]」: a teleport always fires [经过] then [重叠] then [结算] at the
    /// target, **including onto the tile it started from** (「原地[传送]」,
    /// `E14`). The old C# 「原地，不算 [经过]」 carve-out is wrong.
    pub(crate) fn teleport(
        &mut self,
        i: usize,
        to: usize,
        resolve: bool,
        why: Option<Msg>,
    ) -> Flow<()> {
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
        self.move_start = from;
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
        // B41: 「依次触发[经过],[重叠],和[结算]」 at the target -- [经过]
        // (`passTile`) first, then [重叠] (`passPlayer`), then [结算]
        // (`after_walk`). Same order for a teleport onto its own tile.
        raise!(self, "passTile", i, @m m, tile = to as i32)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        self.overlap_at(&mut m, to)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        if self.tile(to).kind == "circle" {
            // Same fallback shape as the walk's: `tile:circle`'s Pass entry
            // rode the `passTile` raise above. With no instance bound the
            // built-in reward runs here instead.
            if self.w.tile_rule_instances(to as i32).is_empty() {
                self.circle_reward(i, true, from)?;
            }
        }
        m.to = self.w.st.players[i].pos as i32;
        self.after_walk(&mut m)?;
        // C# 24390: `Teleported` after the settlement.
        if !self.out(i) && self.playing() {
            raise!(self, "teleported", i, @m m, tile = m.to)?;
        }
        Ok(())
    }

    /// [重叠] (`passPlayer`) -- 「移动终点触发[重叠]」 / 「原地[传送]/移动(移动0格)
    /// 时触发[重叠]」 (行动阶段 13, `E14`): one raise per other player sharing
    /// `at`. Raised once at the end of the move, never mid-walk.
    fn overlap_at(&mut self, m: &mut Move, at: usize) -> Flow<()> {
        let i = m.player_id;
        let players: Vec<usize> = (0..self.w.st.players.len())
            .filter(|&s| s != i && self.w.st.players[s].pos == at as i32 && !self.out(s))
            .collect();
        for o in players {
            m.passed_players.push(o as i32);
            self.w.log(
                "overlap",
                i as i32,
                Msg::new("log.overlap")
                    .player_id("who", i)
                    .player_id("to", o),
            );
            raise!(self, "passPlayer", i, @m m, tile = at as i32, target = o as i32)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
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

    /// `Settle` -> `Land` on the tile the player is standing on.
    fn settle(&mut self, i: usize, m: &Move) -> Flow<()> {
        let at = self.w.st.players[i].pos as usize;
        self.settle_at(i, at, m)
    }

    /// `SettleAt` -- the [触发结算] of an arbitrary tile. A card that resolves a
    /// tile it is sitting on (rather than the player's square) wants this; the
    /// player does not move.
    fn settle_at(&mut self, i: usize, at: usize, m: &Move) -> Flow<()> {
        let owner = self.w.st.owners.get(at).copied().unwrap_or(-1);
        let t = raise!(self, "settle", i, @m m, tile = at as i32, target = owner)?;
        // C# 24448: a cancelled settle ends here -- no Land, no SettleAfter.
        if self.out(i) || t.is_cancelled() {
            return Ok(());
        }
        // `settleBody` (Fx) -- C# `SettleBody`: the first field card that
        // replaces the tile's effect does its own thing and calls
        // `trigger::set_cancelled()`; a later one should check `cancelled()`.
        let si = raise!(self, "settleBody", i, @m m, tile = at as i32, target = owner)?;
        if !si.is_cancelled() {
            // The settle body: the tile's rule instances (`docs/TILES.md`).
            // `CardRules::settle_tile` runs them; the default impl (and
            // `WasmRules` for a tile with no instances) is the built-in
            // `land_at` body below.
            let rules = self.rules;
            rules.settle_tile(self, i, at, m.main)?;
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

    /// `circleAffected`'s `Trigger.value`: which half of the CiRCLE reward was
    /// taken. These are the source of truth -- `card-sdk`'s `abi` mirrors them
    /// (same convention as `Arg` / `TriggerKind`, which `abi` documents as
    /// "must match the engine"). The stunned path forces [`CIRCLE_REWARD_CARD`],
    /// so a money-only clause cannot fire there.
    pub const CIRCLE_REWARD_MONEY: i32 = 0;
    pub const CIRCLE_REWARD_CARD: i32 = 1;

    /// `CircleReward` -- 2,000 money or one card.
    ///
    /// `docs/TILES.md`: the reward step is `tile:circle`'s **Pass entry**
    /// (`ctx::settle_circle_reward`), and suppression is `prop::NO_REWARD` on
    /// the rule instance -- a rule that 「无法获取[CiRCLE奖励]」 / 「[经过]CiRCLE
    /// 时不获得[CiRCLE奖励]」 / 「首次经过CiRCLE不获得经过奖励」 **sets the prop**
    /// and clears it when its own clause ends (「…时」 goes in an event handler on
    /// that state's change). The source owns the arming and the disarming; the
    /// reader is the rule instance, not a per-player latch and not a walk-plan
    /// flag.
    ///
    /// Two placements are read, both named `prop::NO_REWARD`, because the
    /// clauses are of two shapes:
    ///
    /// * on the **tile's** `tile:circle` instance (`ctx::set_tile_prop`) -- a
    ///   walk-scoped veto (「…的移动…不获得[CiRCLE奖励]」). Consumed here, so a
    ///   stale arm cannot veto the next player.
    /// * on the **passing player's own** field instance (`ctx::set_prop`) -- a
    ///   per-player veto (「无法获取[CiRCLE奖励]」 while the source is in play).
    ///   The source arms and disarms it.
    pub fn card_circle_reward(&mut self, i: usize, landing: bool) -> Flow<()> {
        let start = self.move_start;
        self.circle_reward(i, landing, start)
    }

    /// `H.Roll` -- a card- or skill-driven dice roll. Sums `count`d`sides` the
    /// same way `roll_tables` does (a flat `count` when `sides <= 0`); the
    /// caller raises the `Roll` chain link so 「掷骰结算前」 [反击]s see it.
    pub fn card_roll(&mut self, _player_id: usize, count: i32, sides: i32) -> i32 {
        let mut total = 0;
        if sides <= 0 {
            total += count;
        } else {
            for _ in 0..count.max(0) {
                total += self.w.rng.d(sides.max(1));
            }
        }
        total
    }

    /// `H.DoMoveRoll` for a card-driven reroll (「使用火罐进行掷骰」): sum the
    /// move plan's `base` + `dice` tables, honouring `TurnCtx::extreme`. The
    /// caller raises the `Roll` chain link on the face.
    pub fn card_do_move_roll(&mut self, _player_id: usize) -> i32 {
        let plan = self.w.turn.plan.clone();
        self.roll_tables(&plan)
    }

    /// `CircleReward` -- 2,000 money or one card. `start` is the move's 移动起点.
    fn circle_reward(&mut self, i: usize, landing: bool, start: i32) -> Flow<()> {
        if self.w.st.players[i].exile() > 0 {
            return Ok(());
        }
        let at = self.w.st.players[i].pos;
        // 规则书 基础[结算] 1.1: 「[经过]CiRCLE且[移动起点]不为CiRCLE时获得
        // [CiRCLE奖励]」. The pass qualifies only when the move did not begin on
        // CiRCLE -- a same-tile teleport onto CiRCLE or a 0-move that ends there
        // with 移动起点 == CiRCLE pays nothing. (`start < 0` = no move in flight,
        // e.g. a card calling `H.CircleReward` outright: no 移动起点 to compare,
        // so the clause does not bar it.)
        if start >= 0
            && self.tile(at as usize).kind == "circle"
            && self.tile(start as usize).kind == "circle"
        {
            return Ok(());
        }
        for (uid, _) in self.w.tile_rule_instances(at) {
            if self.w.prop_at(uid, crate::state::prop::NO_REWARD) > 0 {
                self.w.set_prop_at(uid, crate::state::prop::NO_REWARD, 0);
                return Ok(());
            }
        }
        if self
            .w
            .field_instances(i as i32)
            .into_iter()
            .any(|(uid, _)| self.w.prop_at(uid, crate::state::prop::NO_REWARD) > 0)
        {
            return Ok(());
        }
        let pick = if self.w.st.players[i].stunned() {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.circle_card_only").player_id("who", i),
            );
            1
        } else {
            let ask = if landing {
                Ask::choice(
                    vec![i],
                    Msg::new("ask.circle.title"),
                    Msg::new("ask.circle.text_landed").player_id("who", i),
                    vec![
                        Msg::new("ask.circle.money_landed").n("money", CIRCLE_MONEY),
                        Msg::new("ask.circle.card_landed"),
                    ],
                    0,
                    10.0,
                )
            } else {
                Ask::choice(
                    vec![i],
                    Msg::new("ask.circle.title"),
                    Msg::new("ask.circle.text").player_id("who", i),
                    vec![
                        Msg::new("ask.circle.money").n("money", CIRCLE_MONEY),
                        Msg::new("ask.circle.card"),
                    ],
                    0,
                    10.0,
                )
            };
            self.ask(ask)?.of(i)
        };
        // `circleAffected` -- the reward has been picked but not paid out. A
        // field card can rewrite it or cancel it outright and pay something
        // else (Morfonica replaces the money half), so this raises between the
        // pick and the payout: after the pick, because a hook has to see which
        // option was taken; before the payout, because it has to be able to
        // change it.
        //
        // `value` is which option the reward resolved to: 0 = money, 1 = card.
        // The stunned path forces the card, so it raises with `value = 1` --
        // a money-only clause ("when the money option is chosen") checks
        // `value == 0` and correctly does not fire there. Raised on both paths
        // rather than only the choice, so a reward rewrite applies however the
        // option was arrived at.
        let ca = raise!(self, "circleAffected", i, value = pick)?;
        if !ca.is_cancelled() {
            if pick == Self::CIRCLE_REWARD_CARD {
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.circle_card").player_id("who", i),
                );
                self.draw_r(i, 1, "src.circle")?;
            } else {
                let mut p = Pay::new(CIRCLE_MONEY, "gain");
                p.to = Some(i);
                p.typ = Some("pass");
                p.source = "src.circle";
                p.text = Some(Msg::new("log.circle_money").player_id("who", i));
                self.money(p)?;
            }
        }
        self.wait(0.4);
        Ok(())
    }

    /// `Land` -- resolve the tile a player stopped on.
    pub fn land(&mut self, i: usize, main: bool) -> Flow<()> {
        let at = self.w.st.players[i].pos as usize;
        self.land_at_built_in(i, at, main)
    }

    /// The landing gates and bookkeeping both settle bodies share: nothing
    /// settles for an exiled / out player or off the board, and a main landing
    /// records where the player stopped (the end-step buy/build gates ask).
    /// Returns false when the settle does not run at all.
    pub fn prep_land(&mut self, i: usize, at: usize, main: bool) -> bool {
        if at >= self.data.tiles.len() || self.w.st.players[i].exile() > 0 || self.out(i) {
            return false;
        }
        if main {
            self.w.st.landed = at as i32;
        }
        true
    }

    /// `Land` at a named tile (the [触发结算] half of [`Self::settle_at`]).
    ///
    /// The **built-in** settle body: what `CardRules::settle_tile` falls back
    /// to when the ruleset has no rule instance for the tile (`StubRules`, or
    /// a kind not yet migrated -- `docs/TILES.md`). Also the whole of `land`
    /// for a card that resolves a tile directly.
    pub fn land_at_built_in(&mut self, i: usize, at: usize, main: bool) -> Flow<()> {
        if !self.prep_land(i, at, main) {
            return Ok(());
        }
        let tile = self.tile(at);
        if tile.is_buyable() {
            let owner = self.w.st.owners[at];
            if owner < 0 {
                if main {
                    let houses = self.w.st.houses[at];
                    let extra =
                        (houses > 0).then(|| Msg::new("log.part.with_houses").i("h", houses));
                    self.w.log(
                        "text",
                        i as i32,
                        Msg::new("log.land_unowned")
                            .player_id("who", i)
                            .tile("tile", at)
                            .n("price", self.buy_price(at))
                            .opt("extra", extra),
                    );
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
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.land_own")
                        .player_id("who", i)
                        .tile("tile", at)
                        .opt("note", note),
                );
            } else {
                self.offer_build(i, at)?;
            }
            return Ok(());
        }
        match tile.kind.as_str() {
            "circle" => {
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.land_circle").player_id("who", i),
                );
                self.draw_r(i, 1, "src.land_circle")?;
                self.wait(0.4);
            }
            "edogawa" => {
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.land_draw")
                        .player_id("who", i)
                        .tile("tile", at),
                );
                self.draw_r(i, 1, "src.land_edogawa")?;
                self.wait(0.4);
            }
            "cafe" | "ryuseido" => {
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.land_event")
                        .player_id("who", i)
                        .tile("tile", at),
                );
                self.draw_r(i, 1, "src.land_event")?;
                self.wait(0.6);
                self.draw_event(i)?;
            }
            "agent" => self.agent_landing(i, at)?,
            _ => {
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.land").player_id("who", i).tile("tile", at),
                );
            }
        }
        Ok(())
    }

    /// `AgentLanding` -- buy or build once in the agent's colour group.
    pub fn agent_landing(&mut self, i: usize, agent: usize) -> Flow<()> {
        let g = self.tile(agent).group;
        let same: Vec<usize> = (0..self.data.tiles.len())
            .filter(|&t| self.tile(t).is_buyable() && self.tile(t).group == g)
            .collect();
        if same.is_empty() {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.agent_none")
                    .player_id("who", i)
                    .tile("agent", agent),
            );
            return Ok(());
        }
        if same
            .iter()
            .all(|&t| self.w.st.owners[t] >= 0 && self.w.st.owners[t] as usize != i)
        {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.agent_all_owned")
                    .player_id("who", i)
                    .tile("agent", agent)
                    .i("n", same.len() as i64),
            );
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
                            .opt(
                                "extra",
                                (houses > 0)
                                    .then(|| Msg::new("ask.part.incl_houses").i("h", houses)),
                            ),
                    );
                }
            } else if self.w.st.owners[t] as usize == i
                && self.why_not_build_on(i, t).is_none()
                && money >= self.build_cost(t)
            {
                options.push(t);
                labels.push(
                    Msg::new("ask.agent.build")
                        .tile("tile", t)
                        .i("nth", self.w.st.houses[t] + 1)
                        .n("cost", self.build_cost(t)),
                );
            }
        }
        if options.is_empty() {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.agent_nothing")
                    .player_id("who", i)
                    .tile("agent", agent),
            );
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
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.agent_skip").player_id("who", i),
            );
            return Ok(());
        };
        if self.w.st.owners[t] < 0 {
            if self.can_pay(i) && self.w.st.players[i].money >= self.buy_price(t) {
                self.buy(i, t)?;
            }
        } else if self.w.st.owners[t] as usize == i
            && self.why_not_build_on(i, t).is_none()
            && self.w.st.players[i].money >= self.build_cost(t)
        {
            self.build(i, t)?;
        }
        self.wait(0.6);
        Ok(())
    }

    /// `PayRent` -- property rent from the rent table; RiNG rent is
    /// rings owned x multiplier x 1d20; agents charge half.
    ///
    /// Also the `ctx::pay_rent` primitive (`docs/TILES.md`): `tile:property` /
    /// `tile:ring` / `tile:agent` bodies call it with the rulebook's 「半价收费」
    /// half-flag. A mortgaged deed charges no rent -- the caller decides
    /// between this and [`Self::offer_force_buy`], per 「如果格子地契已抵押」.
    pub fn card_pay_rent(&mut self, i: usize, t: usize, half: bool) -> Flow<()> {
        self.pay_rent(i, t, half)
    }

    /// The `ctx::offer_force_buy` primitive: 「可选择[支付]…两倍…强行购买」.
    pub fn card_offer_force_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        self.offer_force_buy(i, t)
    }

    /// The `ctx::offer_buy` primitive: 「可选择[消耗]…获得格子地契和拥有权」 on a
    /// non-main landing. (A main landing only *logs* -- the buy is the end
    /// step's offer, rulebook 「[主要移动]和所需[结算]完成后进入结束阶段」.)
    pub fn card_offer_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        self.offer_buy(i, t)
    }

    /// The `ctx::offer_build` primitive: 「可选择[消耗]格子地契所标注的房屋建筑费
    /// 进行升级建造」 on a non-main landing on one's own land.
    pub fn card_offer_build_one(&mut self, i: usize, t: usize) -> Flow<()> {
        self.offer_build(i, t)
    }

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
            (
                rings * mult * d,
                Some(
                    Msg::new("log.part.rent_ring")
                        .i("rings", rings)
                        .i("mult", mult)
                        .i("d", d),
                ),
            )
        } else {
            // The counted house count (`H.RentHouses`): a 「房屋数视为…」
            // override rides here, and real `st.houses` is untouched.
            let h = self.w.rent_houses(t as i32);
            let rent = if tile.rent.is_empty() {
                0
            } else {
                tile.rent[(h as usize).min(tile.rent.len() - 1)]
            };
            (
                rent,
                (h > 0).then(|| Msg::new("log.part.rent_houses").i("h", h)),
            )
        };
        let mut half_part = None;
        if half {
            let full = amount;
            amount = ((full as f64 / 2.0 / 10.0).ceil() as i32) * 10;
            half_part = Some(
                Msg::new("log.part.rent_half")
                    .n("full", full)
                    .n("cut", amount),
            );
        }
        // The 「支付」 / 「地租」 scalars no longer scale the rent table here:
        // they ride the money pipeline's `payMul` stage (`scale_settle_payment`),
        // so a rule that halves a settlement payment covers the payment **as
        // card effects have shaped it** -- a rent-region expansion, a forced
        // stop-and-pay -- and not just this table's number. 规则书
        // (`docs/TILES.md`, 「支付减半」 ruling 2026-10-06).
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
        p.text = Some(
            Msg::new("log.rent")
                .player_id("who", i)
                .tile("tile", t)
                .player_id("owner", owner)
                .opt("note", note),
        );
        let paid = self.money(p)?;
        // 「本回合的[结算]向其他玩家支付了至少1000资金」 -- the turn's running
        // total of what the [触发结算]s cost this player.
        if paid.paid && paid.loss > 0 {
            self.w.turn.paid_in_settle += paid.loss;
        }
        self.wait(0.9);
        Ok(())
    }

    /// The 「支付减半」 / 「地租」 scale, applied at the money pipeline's `payMul`
    /// stage (`docs/TILES.md`; 「支付减半」 ruling 2026-10-06).
    ///
    /// Two homes, both read here rather than in `pay_rent`:
    ///
    /// * `prop::PAY_FACTOR` / `prop::RENT_FACTOR` (milli-units, 500 = ×0.5) on
    ///   the tile's rule instance -- a card that retunes a tile writes these
    ///   (`ctx::set_tile_prop`). `RENT_FACTOR` only scales rent.
    /// * the move plan's `plan::set_pay_factor` / `plan::set_rent_factor` -- a
    ///   card that shapes a *settle* (祥，移动's 「触发结算时进行的支付价格减半」,
    ///   Repaint's 「对方此次结算的支付减半」) arms these, because a hand card
    ///   leaves the field and cannot leave a hook behind.
    ///
    /// Scope: a settlement loss. A purchase (`buy` / `build`), a forced
    /// purchase (rulebook 「此次购买的价格不受任何资金变动效果影响」) and a
    /// print (`gain`) are never scaled. TODO(规则书): whether 「支付」 also
    /// reaches a non-settlement loss a card forces outside a settle.
    fn scale_settle_payment(
        &self,
        amount: i32,
        kind: &str,
        rent: bool,
        tile: Option<usize>,
    ) -> i32 {
        if matches!(kind, "buy" | "build" | "forcebuy" | "gain") {
            return amount;
        }
        let mut f = 1.0f64;
        if rent {
            f *= self.w.turn.plan.rent_factor;
        }
        f *= self.w.turn.plan.pay_factor;
        // A prop the tile does not carry reads as its default `0`, which here
        // means 「no scale」 (×1.0) rather than ×0 -- only a positive milli value
        // is a scale.
        if let Some(t) = tile {
            if rent {
                let r = self.w.tile_prop(t as i32, crate::state::prop::RENT_FACTOR);
                if r > 0 {
                    f *= f64::from(r) / 1000.0;
                }
            }
            let p = self.w.tile_prop(t as i32, crate::state::prop::PAY_FACTOR);
            if p > 0 {
                f *= f64::from(p) / 1000.0;
            }
        }
        if (f - 1.0).abs() <= f64::EPSILON {
            return amount;
        }
        ((amount as f64 * f) as i32).max(0)
    }

    /// `OfferForceBuy` -- mortgaged land charges no rent but may be bought out at
    /// twice its value.
    fn offer_force_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        let owner = self.w.st.owners[t] as usize;
        let price = self.force_buy_price(t);
        if !self.can_pay(i) || self.w.st.players[i].money < price {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.land_mortgaged")
                    .player_id("who", i)
                    .player_id("owner", owner)
                    .tile("tile", t)
                    .n("price", price),
            );
            return Ok(());
        }
        let money = self.w.st.players[i].money;
        let ask = Ask::choice(
            vec![i],
            Msg::new("ask.force_buy.title"),
            Msg::new("ask.force_buy.text")
                .player_id("who", i)
                .player_id("owner", owner)
                .tile("tile", t)
                .n("price", price),
            vec![
                Msg::new("ask.force_buy.yes").n("price", price),
                Msg::new("ask.no_buy"),
            ],
            1,
            15.0,
        )
        .with_ai(|_| if money - price < 4000 { 1 } else { 0 })
        .with_tile(t);
        if self.ask(ask)?.of(i) == 0
            && self.w.st.owners[t] == owner as i32
            && !self.out(owner)
            && !self.out(i)
            && self.w.st.players[i].money >= price
        {
            self.w.st.players[i].money -= price;
            self.w.st.players[owner].money += price;
            self.w.st.owners[t] = i as i32;
            let text = Msg::new("log.force_buy")
                .player_id("who", i)
                .player_id("owner", owner)
                .n("price", price)
                .tile("tile", t);
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
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.land_unowned_poor")
                    .player_id("who", i)
                    .tile("tile", t),
            );
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
                .opt(
                    "extra",
                    (houses > 0).then(|| Msg::new("ask.part.incl_tile_houses").i("h", houses)),
                ),
            vec![Msg::new("ask.buy.yes"), Msg::new("ask.no_buy")],
            1,
            15.0,
        )
        .with_ai(|_| if wants { 0 } else { 1 })
        .with_tile(t);
        if self.ask(ask)?.of(i) == 0
            && self.w.st.owners[t] < 0
            && self.w.st.players[i].money >= self.buy_price(t)
            && !self.out(i)
        {
            self.buy(i, t)?;
        }
        Ok(())
    }

    /// `OfferBuild` (when a non-main move lands on own land).
    fn offer_build(&mut self, i: usize, t: usize) -> Flow<()> {
        if self.why_not_build_on(i, t).is_some() || self.w.st.players[i].money < self.build_cost(t)
        {
            let note = self.w.st.mortgaged[t].then(|| Msg::new("log.part.mortgaged"));
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.land_own")
                    .player_id("who", i)
                    .tile("tile", t)
                    .opt("note", note),
            );
            return Ok(());
        }
        let cost = self.build_cost(t);
        let wants = self.ai_wants_build(i, t);
        let ask = Ask::choice(
            vec![i],
            Msg::new("ask.build.title"),
            Msg::new("ask.build.text")
                .player_id("who", i)
                .tile("tile", t)
                .n("cost", cost),
            vec![Msg::new("ask.build.yes"), Msg::new("ask.build.no")],
            1,
            15.0,
        )
        .with_ai(|_| if wants { 0 } else { 1 })
        .with_tile(t);
        if self.ask(ask)?.of(i) == 0
            && self.why_not_build_on(i, t).is_none()
            && self.w.st.players[i].money >= self.build_cost(t)
        {
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
        (0..self.data.tiles.len())
            .filter(|&t| self.tile(t).kind == "ring" && self.w.st.owners[t] == player_id as i32)
            .count() as i32
    }

    /// Deeds a player could mortgage (not RiNG, not already mortgaged).
    pub(crate) fn mortgageable(&self, player_id: usize) -> Vec<usize> {
        (0..self.data.tiles.len())
            .filter(|&t| {
                self.w.st.owners[t] == player_id as i32
                    && self.tile(t).is_buyable()
                    && self.tile(t).kind != "ring"
                    && !self.w.st.mortgaged[t]
            })
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
        st.step == stage::END
            && st.turn == i as i32
            && !st.bought
            && st.landed == pos
            && self
                .data
                .tiles
                .get(pos as usize)
                .is_some_and(|t| t.is_buyable())
            && st.owners[pos as usize] < 0
            && self.can_pay(i)
    }

    pub(crate) fn can_buy_here(&self, i: usize) -> bool {
        self.buyable_here(i)
            && self.w.st.players[i].money >= self.buy_price(self.w.st.players[i].pos as usize)
    }

    pub(crate) fn can_build_here(&self, i: usize) -> bool {
        let st = &self.w.st;
        let pos = st.players[i].pos as usize;
        st.step == stage::END
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
        if st.turn == i as i32
            && self.playing()
            && st.step == stage::END
            && (st.landed != t as i32 || st.bought)
        {
            return Some(Msg::new("err.build_only_settling"));
        }
        // The move said this landing cannot be built on (`can_build = false`).
        if !st.plan.can_build {
            return Some(Msg::new("err.build_denied"));
        }
        self.why_not_build_on(i, t)
    }

    /// `WhyNotBuildOn` -- the rule lives on [`super::ops`] so a card asking
    /// "where may I build?" gets the same answer the build step does.
    pub(crate) fn why_not_build_on(&self, i: usize, t: usize) -> Option<Msg> {
        self.w.why_not_build_on(self.data, i as i32, t as i32)
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
        // 「本回合购买格子时[消耗]资金时降低N（最低0）」
        let discount = self.w.turn.buy_discount.max(0);
        if discount > 0 {
            price = (price - discount).max(0);
        }
        // 「本回合购买格子不[消耗]资金」 -- the money simply does not move.
        if price > 0 && !self.w.turn.free_buy {
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
        // 「如果购买则拆除那个格子上的所有房屋」
        if self.w.turn.raze_on_buy {
            self.w.st.houses[t] = 0;
        }
        let text = Msg::new("log.buy")
            .player_id("who", i)
            .tile("tile", t)
            .n("price", price)
            .opt(
                "extra",
                (houses > 0).then(|| Msg::new("log.part.incl_houses").i("h", houses)),
            );
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
        // 「加盖房屋时半价」 / 「本回合加盖房屋变为免费」 -- the rate is a value
        // the skill writes; the engine only applies it.
        let pct = self.w.turn.build_cost_pct.clamp(0, 100);
        if pct != 100 {
            cost = cost * pct / 100;
        }
        // 「下次盖房时减免N（可溢出），盖房后减少1层」 -- a layered cut on the
        // cost; the refund below covers the 「可溢出」 half, and one layer pops
        // whether or not the build actually commits.
        let cut = self.w.turn.build_discount.min(cost);
        if cut > 0 {
            cost -= cut;
        }
        if self.w.turn.build_discount_layers > 0 {
            self.w.turn.build_discount_layers -= 1;
            if self.w.turn.build_discount_layers == 0 {
                self.w.turn.build_discount = 0;
            }
        }
        if cost > 0 {
            let mut p = Pay::new(cost, "build");
            p.from = Some(i);
            p.typ = Some("lose");
            p.tile = Some(t);
            p.source = "src.build";
            p.text = Some(
                Msg::new("log.paid_for_house")
                    .player_id("who", i)
                    .tile("tile", t),
            );
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
                let key = if self.w.st.owners[t] != i as i32 {
                    "log.build_refund_not_owned"
                } else {
                    "log.build_refund_full"
                };
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new(key)
                        .player_id("who", i)
                        .tile("tile", t)
                        .n("cost", cost),
                );
            }
            return Ok(());
        }
        self.w.st.houses[t] += 1;
        let h = self.w.st.houses[t];
        let text = Msg::new("log.build")
            .player_id("who", i)
            .tile("tile", t)
            .i("nth", h)
            .n("cost", cost);
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
        let Some(tile) = usize::try_from(t).ok().and_then(|t| self.data.tiles.get(t)) else {
            return Some(Msg::new("err.not_your_deed"));
        };
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
        if st.step == stage::MOVE {
            return Some(Msg::new("err.mortgage_moving"));
        }
        if st.step == stage::END && !self.pending_purchase(i) {
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
        let Some(tile) = usize::try_from(t).ok().and_then(|t| self.data.tiles.get(t)) else {
            return Some(Msg::new("err.not_your_deed"));
        };
        let t = t as usize;
        if !tile.is_buyable() || st.owners[t] != i as i32 {
            return Some(Msg::new("err.not_your_deed"));
        }
        if !st.mortgaged[t] {
            return Some(Msg::new("err.not_mortgaged"));
        }
        if st.turn != i as i32 || st.step != stage::OPS {
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
        // 「本回合进行过抵押操作」 -- a record the card gates ask about. Timed to
        // the turn end so it does not leak.
        let s = &mut self.w.st.players[i];
        s.state_set(key::MORTGAGED, 1);
        s.state_set_expires(key::MORTGAGED, Some(Tick::TurnEnd));
        let v = self.mortgage_value(t);
        self.w.st.mortgaged[t] = true;
        self.w.st.players[i].money += v;
        let text = Msg::new("log.mortgage")
            .player_id("who", i)
            .tile("tile", t)
            .n("n", v)
            .opt("why", why.map(|w| Msg::new("log.part.why").msg("why", w)));
        let e = self.w.log("mortgage", i as i32, text);
        e.value = v;
        e.to = t as i32;
    }

    /// `MortgageRoutine`
    pub(crate) fn mortgage(&mut self, i: usize, t: usize, why: Option<Msg>) -> Flow<()> {
        // `mortgageBefore` -- fires before any guard, so it is a real pre-hook
        // (a counteraction can block the mortgage). The post half below only fires
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
        // 「本回合进行过赎回操作」 -- same record, same timing.
        {
            let s = &mut self.w.st.players[i];
            s.state_set(key::REDEEMED, 1);
            s.state_set_expires(key::REDEEMED, Some(Tick::TurnEnd));
        }
        let cost = self.redeem_cost(t);
        self.w.st.mortgaged[t] = false;
        self.w.st.players[i].money -= cost;
        let text = Msg::new("log.redeem")
            .player_id("who", i)
            .n("cost", cost)
            .tile("tile", t);
        let e = self.w.log("redeem", i as i32, text);
        e.value = cost;
        e.to = t as i32;
    }

    // =============================================================== money

    /// `Money` -- move money between players and/or the bank. A mandatory payment
    /// the payer can't cover triggers `RaiseFunds` (and possibly bankruptcy).
    ///
    /// This is the **one pipeline** every money operation runs through. It
    /// stages the adjustment points -- pre-split `effect` (the [反击] window)
    /// then `payAdd` / `payMul` / `payChoose` / `payAt`, then split into
    /// concrete payer/payee entries, then post-split `pay` / `payAfter` /
    /// `paid` -- and fires before/after money events on every entry.
    ///
    /// Re-entrancy is bounded: a hook cannot re-trigger on its own money
    /// movement (`Cx::reentrant_hooks`), and [`MAX_MONEY_DEPTH`] is a safety
    /// cap that traps loudly. The rulebook has no depth limit, so nested money
    /// opens its [反击] windows at every depth.
    pub fn money(&mut self, p: Pay) -> Flow<Paid> {
        let depth = self.money_depth;
        self.money_depth = depth + 1;
        let r = self.money_inner(p, depth);
        self.money_depth = depth;
        r
    }

    /// [`Self::money`] with the depth already bracketed. `depth` is how many
    /// money pipelines are already open around this one (0 = top-level).
    fn money_inner(&mut self, p: Pay, depth: u32) -> Flow<Paid> {
        if depth >= MAX_MONEY_DEPTH {
            // Safety cap hit -- the real termination argument (a hook cannot
            // re-trigger on its own movement) should make this unreachable.
            // Trap loudly rather than settling silently.
            panic!(
                "MAX_MONEY_DEPTH ({}) exceeded at depth {}: runaway money pipeline (pay={:?})",
                MAX_MONEY_DEPTH, depth, p
            );
        }
        if p.amount <= 0 || (p.from.is_some() && p.from == p.to) {
            return Ok(Paid::default());
        }
        if p.from.is_some_and(|f| self.out(f)) || p.to.is_some_and(|t| self.out(t)) {
            return Ok(Paid::default());
        }
        // C# `TurnCtx.NoMoneyLoss` -- the payer's money cannot drop this turn.
        if let Some(f) = p.from.filter(|&f| self.w.money_locked(f as i32)) {
            self.w.log(
                "text",
                f as i32,
                Msg::new("log.money_locked")
                    .player_id("who", f)
                    .n("amount", p.amount),
            );
            return Ok(Paid::default());
        }
        if p.kind != "forcebuy" {
            // The payer is gated by `can_pay` (out / stunned / exiled). The
            // payee is gated only by [晕眩] (rulebook 49: 「无法收付款」 --
            // stunned cannot *receive* either); exile is not a receiving block
            // -- 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」 still
            // moves it (and a redirect hook can name a different payee). An out
            // payee is refused just above.
            let stunned_payee = p.to.filter(|&t| self.w.st.players[t].stunned());
            if let Some(x) = p.from.filter(|&f| !self.can_pay(f)).or(stunned_payee) {
                let text = Msg::new(if p.from.is_some() {
                    "log.blocked_pay"
                } else {
                    "log.blocked_gain"
                })
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
        // ---- declaration ------------------------------------------------
        // The payment is declared with its **declared** amount, before any
        // modifier has touched it. That is the stable fact a counter listens
        // to: `vocal_too_hard`'s 「5000以上」 reads this, not whatever the
        // settlement has drifted to.
        //
        // `effect` is the [反击] chain. Counters may negate the payment
        // outright, spare a party, or reshape its amount and payee.
        //
        // It opens for **every** money movement -- print (game -> player),
        // delete (player -> game) and pay-player alike -- so any card-caused
        // payment is answerable, not just rent. `player_id` is the payer (-1
        // for a print); `target` is the payee (-1 for a delete).
        //
        // The `effect` [反击] window opens for every money movement at every
        // depth (the rulebook has no depth limit); the re-entrancy guard
        // (`Cx::reentrant_hooks`) is the termination argument.
        if amount > 0 {
            let t = raise!(
                self,
                "effect",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = amount,
                by_card = p.by_card,
                pay_is_rent = rent,
                tile = p.tile.map_or(-1, |t| t as i32),
                effects = vec![super::rules::Effect {
                    kind: "pay",
                    target: to.map_or(-1, |t| t as i32),
                    from: payer,
                    tile: p.tile.map_or(-1, |t| t as i32),
                    value: amount,
                }],
            )?;
            if t.is_cancelled() || !t.settles_for(side as i32) {
                raise!(
                    self,
                    "payAfter",
                    side,
                    player_id = payer,
                    target = to.map_or(-1, |t| t as i32),
                    value = 0,
                    by_card = p.by_card,
                    pay_is_rent = rent,
                    tile = p.tile.map_or(-1, |t| t as i32)
                )?;
                return Ok(Paid::default());
            }
            amount = t.value.max(0);
            to = (t.target >= 0).then_some(t.target as usize);
        }
        // ---- settlement -------------------------------------------------
        // Modifiers run now, against the amount the effect settled on.
        for kind in ["payAdd", "payMul", "payChoose", "payAt"] {
            if amount <= 0 {
                break;
            }
            let t = raise!(
                self,
                kind,
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = amount,
                by_card = p.by_card,
                pay_is_rent = rent,
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
            amount = t.value.max(0);
            // A stage may redirect the payee (`set_pay_target`, e.g. 无路矢's
            // 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」) or the
            // payer; the payload's payer/payee is per-entry and each stage may
            // reshape it.
            if t.target >= 0 {
                to = Some(t.target as usize);
            }
            // The 「支付」 / 「地租」 scalars ride the `payMul` stage
            // (`docs/TILES.md`): applied *after* `payAdd` and the `effect`
            // window, so the scale covers the payment as card effects have
            // shaped it and not only `pay_rent`'s table number.
            if kind == "payMul" {
                amount = self.scale_settle_payment(amount, p.kind, rent, p.tile);
            }
        }
        // `pay` is a settlement hook now: what the payment actually was. It
        // fires after the chain, so it can no longer be used to reconstruct
        // 「被…效果影响」 -- that is `effect`'s job.
        if amount > 0 {
            let t = raise!(
                self,
                "pay",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = amount,
                by_card = p.by_card,
                pay_is_rent = rent,
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
            amount = t.value.max(0);
            to = (t.target >= 0).then_some(t.target as usize);
        }
        // Cancelled: nothing moves and it is not paid (C# 25164 -- PayAfter
        // still runs, with `p.paid == false`).
        if amount <= 0 {
            raise!(
                self,
                "payAfter",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = 0,
                by_card = p.by_card,
                pay_is_rent = rent,
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
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
        raise!(
            self,
            "payAfter",
            side,
            player_id = payer,
            target = to.map_or(-1, |t| t as i32),
            value = moved,
            by_card = p.by_card,
            pay_is_rent = rent,
            tile = p.tile.map_or(-1, |t| t as i32)
        )?;
        if let Some(f) = p.from.filter(|_| loss > 0) {
            raise!(
                self,
                "paid",
                f,
                target = to.map_or(-1, |t| t as i32),
                value = loss,
                by_card = p.by_card,
                pay_is_rent = rent,
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
        }
        Ok(Paid {
            paid: true,
            loss,
            gain,
        })
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
                let src = (!p.source.is_empty())
                    .then(|| Msg::new("log.part.why").msg("why", Msg::new(p.source)));
                let m = match (p.from, p.to) {
                    (Some(f), Some(t)) => {
                        Msg::new("log.pay").player_id("who", f).player_id("to", t)
                    }
                    (Some(f), None) => Msg::new("log.lose").player_id("who", f),
                    (None, Some(t)) => Msg::new("log.gain").player_id("who", t),
                    (None, None) => return,
                };
                m.n("amount", amount).opt("src", src)
            }
        };
        if let (Some(_), Some(t)) = (p.from, p.to) {
            if gain != loss {
                text = Msg::new("log.with_received")
                    .msg("base", text)
                    .player_id("who", t)
                    .n("gain", gain);
            }
        }
        let player_id = p.from.or(p.to).map_or(-1, |s| s as i32);
        let other = if p.from.is_some() {
            p.to.map_or(-1, |t| t as i32)
        } else {
            -1
        };
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
            .opt(
                "to",
                creditor.map(|c| Msg::new("log.part.owe_to").player_id("who", c)),
            )
            .n("money", self.w.st.players[i].money);
        self.w.log(
            "text",
            i as i32,
            Msg::new("log.raise_funds")
                .player_id("who", i)
                .msg("what", what.clone()),
        );
        let ai = self.auto_mortgage(i, need);
        let text = Msg::new("ask.mortgage.text")
            .msg("what", what)
            .n("need", need);
        let reply = self.ask(Ask::mortgage(i, need, text, &deeds, ai))?;
        for id in &reply.a.picked {
            if let Ok(t) = id.parse::<usize>() {
                if t < self.data.tiles.len()
                    && self.w.st.owners[t] == i as i32
                    && !self.w.st.mortgaged[t]
                {
                    self.mortgage(i, t, Some(Msg::new("src.raise_funds")))?;
                }
            }
        }
        while self.w.st.players[i].money < amount {
            let Some(&t) = self.mortgage_order(self.mortgageable(i)).first() else {
                break;
            };
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
            .opt(
                "cashed",
                (cashed > 0).then(|| Msg::new("log.part.cashed").n("n", cashed)),
            )
            .n("all", all)
            .msg(
                "to",
                creditor.map_or_else(
                    || Msg::new("log.part.consumed"),
                    |c| Msg::new("log.part.paid_to").player_id("who", c),
                ),
            );
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
        self.w.log(
            "left",
            i as i32,
            Msg::new("log.forfeit").player_id("who", i),
        );
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
            key::STAY,
            key::STUN,
            key::STUN_START,
            key::EXILE,
            key::NO_HAND,
            key::FIRE,
            key::UNSTOPPABLE,
        ] {
            s.state_set(k, 0);
        }
        s.state_set(key::EXILE_TO, -1);
        self.w.hidden[i].hand.clear();
        self.w.extra_turns.retain(|&x| x != i);
        let deeds: Vec<usize> = (0..self.data.tiles.len())
            .filter(|&t| self.w.st.owners[t] == i as i32)
            .collect();
        for &t in &deeds {
            self.w.st.owners[t] = -1;
            self.w.st.mortgaged[t] = false;
        }
        if !deeds.is_empty() {
            let houses = deeds.iter().any(|&t| self.w.st.houses[t] > 0);
            let key = if houses {
                "log.deeds_freed_houses"
            } else {
                "log.deeds_freed"
            };
            self.w.log(
                "text",
                -1,
                Msg::new(key).player_id("who", i).i("n", deeds.len() as i64),
            );
        }
        deeds
    }

    /// `AuctionLeftovers` -- auction up to 3 random deeds of a player who went out.
    pub(crate) fn auction_leftovers(&mut self, deeds: Vec<usize>) -> Flow<()> {
        let mut pool: Vec<usize> = deeds
            .iter()
            .copied()
            .filter(|&t| self.w.st.owners[t] < 0)
            .collect();
        self.w.rng.shuffle(&mut pool);
        pool.truncate(3);
        if pool.is_empty() {
            return Ok(());
        }
        let names = pool.iter().map(|&t| Arg::Tile(t as i32)).collect();
        let more = (deeds.len() > pool.len())
            .then(|| Msg::new("log.part.auction_sample").i("n", deeds.len() as i64));
        self.w.log(
            "text",
            -1,
            Msg::new("log.auction_leftovers")
                .list("tiles", names)
                .opt("more", more),
        );
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
        let players: Vec<usize> = self
            .present_from(from)
            .into_iter()
            .filter(|&s| self.can_pay(s))
            .collect();
        if players.is_empty() {
            self.w
                .log("text", -1, Msg::new("log.auction_nobody").tile("tile", t));
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
            .opt(
                "extra",
                (houses > 0).then(|| Msg::new("ask.part.auction_houses").i("h", houses)),
            );
        let ask = Ask::auction(
            t,
            players,
            Msg::new("ask.auction.title").tile("tile", t),
            text,
            worth,
        );
        let r = self.ask(ask)?;
        if r.a.bidder < 0 {
            self.w
                .log("text", -1, Msg::new("log.auction_no_bid").tile("tile", t));
            return Ok(());
        }
        let (b, bid) = (r.a.bidder as usize, r.a.bid);
        if self.w.st.owners[t] >= 0 || !self.can_pay(b) || self.w.st.players[b].money < bid {
            self.w.log(
                "text",
                b as i32,
                Msg::new("log.auction_void").player_id("who", b),
            );
            return Ok(());
        }
        self.w.st.players[b].money -= bid;
        self.w.st.owners[t] = b as i32;
        self.w.st.mortgaged[t] = false;
        let text = Msg::new("log.auction_won")
            .player_id("who", b)
            .n("bid", bid)
            .tile("tile", t)
            .opt(
                "extra",
                (houses > 0).then(|| Msg::new("ask.part.incl_houses").i("h", houses)),
            );
        let e = self.w.log("buy", b as i32, text);
        e.value = bid;
        e.to = t as i32;
        // `bought` (Fx) -- C# AuctionTile 23276.
        raise!(self, "bought", b, tile = t as i32)?;
        self.wait(1.0);
        Ok(())
    }

    // =============================================================== cards

    /// `HandLimitOf` / `OverHand` -- the player's own `handLimit` state (cards
    /// can raise or cut it), not the base constant.
    pub(crate) fn over_hand(&self, i: usize) -> bool {
        let limit = self.w.st.players[i].hand_limit();
        self.w.hidden[i].hand.len() as i32 > limit
    }

    /// `Draw` -- from the top of the pile; reshuffle the discard pile when empty.
    /// Bots discard down to the hand limit at random.
    ///
    /// `log` is also "raise the per-draw points": the opening deal passes
    /// `false` (see [`Self::opening`]); every draw during play passes `true`.
    pub(crate) fn draw(&mut self, i: usize, n: usize, log: bool) -> Flow<()> {
        self.draw_cards_with_hooks(i, n, log)?;
        Ok(())
    }

    /// Draw `n` cards one at a time, raising one **before-draw** and one
    /// **after-draw** point per single card. Returns how many entered the hand.
    ///
    /// Per card:
    ///
    /// * `drewBefore` -- before the card moves; `t.card` is the deck's top (or
    ///   empty when the pile is dry). A hook that replaces the draw calls
    ///   `trigger::set_cancelled()` and performs its own look/pick; whatever it
    ///   adds to the hand is the replacement draw (「此次加手视为抽卡动作」),
    ///   and the after points fire for it. A cancelled draw with nothing added
    ///   is skipped outright.
    /// * `drawn` -- after it is in hand, on the **drawn card itself** (C#
    ///   `AfterDraw`).
    /// * `drew` -- after it is in hand, on the **field cards (per-draw effects**:
    ///   梦在前方's crystal per draw, 若宫伊芙's exclusive, ...).
    ///
    /// An N-card draw is N iterations; each payload names one card. Both
    /// card-driven draws (`HostRequest::Draw` -> here) and engine draws (CiRCLE
    /// reward, turn draws) go through this. Opening hands pass `hooks = false`
    /// and raise none of the three.
    pub fn draw_cards_with_hooks(&mut self, i: usize, n: usize, hooks: bool) -> Flow<i32> {
        if self.out(i) {
            return Ok(0);
        }
        let mut got = 0;
        for _ in 0..n {
            if self.w.hidden[i].draw.is_empty() && !self.w.hidden[i].discard.is_empty() {
                let mut pile = std::mem::take(&mut self.w.hidden[i].discard);
                self.w.rng.shuffle(&mut pile);
                self.w.hidden[i].draw = pile;
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.reshuffle").player_id("who", i),
                );
                if hooks {
                    // `reshuffled` (Fx) -- C# `Each(Reshuffled)` 19655.
                    raise!(self, "reshuffled", i)?;
                }
            }
            let top = self.w.hidden[i].draw.last().cloned().unwrap_or_default();
            // `drewBefore` -- the per-card replacement point.
            let mut replacement: Option<String> = None;
            let mut skipped = false;
            if hooks {
                let hand_before = self.w.hidden[i].hand.len();
                let t = raise!(self, "drewBefore", i, card = top, value = 1)?;
                if t.is_cancelled() {
                    skipped = true;
                    // The hook replaced this draw: whatever it added to the
                    // hand is the replacement draw.
                    if self.w.hidden[i].hand.len() > hand_before {
                        replacement = self.w.hidden[i].hand.last().cloned();
                    }
                }
            }
            let card = if let Some(card) = replacement {
                Some(card)
            } else if skipped {
                None
            } else {
                let card = self.w.hidden[i].draw.pop();
                if let Some(c) = &card {
                    self.w.hidden[i].hand.push(c.clone());
                }
                card
            };
            let Some(card) = card else {
                continue;
            };
            got += 1;
            if hooks {
                // `drawn` -- the drawn card's own hook (C# AfterDraw 19660).
                raise!(self, "drawn", i, card = card.clone(), value = 1)?;
                // `drew` -- the field-card per-draw point (C# `Each(Drew)`,
                // now per single card). `cards` names the one card too, so a
                // hook written against the old batch payload still reads it.
                raise!(self, "drew", i, card = card.clone(), value = 1, cards = vec![card])?;
            }
        }
        if hooks && got > 0 {
            let over = self.over_hand(i).then(|| {
                Msg::new("log.part.over_hand")
                    .i("limit", self.w.st.players[i].hand_limit() as i64)
            });
            self.w
                .log(
                    "draw",
                    i as i32,
                    Msg::new("log.draw")
                        .player_id("who", i)
                        .i("n", got)
                        .opt("over", over),
                )
                .value = got;
        }
        if self.w.st.players[i].ai && self.playing() {
            while self.over_hand(i) {
                let k = self.w.rng.below(self.w.hidden[i].hand.len());
                let card = self.w.hidden[i].hand[k].clone();
                self.discard(i, &card)?;
            }
        }
        Ok(got)
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
        self.w
            .log(
                "discard",
                i as i32,
                Msg::new("log.discard")
                    .player_id("who", i)
                    .i("limit", self.w.st.players[i].hand_limit() as i64)
                    .card("card", card),
            )
            .card = card.to_string();
        // `discarded` (Fx) -- C# `OnDiscarded` on that card, now in the pile.
        raise!(self, "discarded", i, card = card.to_string())?;
        // `discardAfter` -- the card is in the discard pile.
        raise!(self, "discardAfter", i, card = card.to_string())?;
        Ok(())
    }

    /// `CannotPlay` -- the status gates. A card that declares
    /// 「可在眩晕时打出」 (the `prop::PLAYABLE_STUNNED` property, C#
    /// `Card.PlayableStunned`) skips the stun gate; the exile and no-hand gates
    /// have no such exception in the pool.
    fn cannot_play(&self, i: usize, id: &str) -> Option<&'static str> {
        let s = &self.w.st.players[i];
        let stun_ok = self.rules.card_prop(id, crate::state::prop::PLAYABLE_STUNNED) != 0;
        if s.exile() > 0 {
            Some("err.play_exiled")
        } else if s.stunned() && !stun_ok {
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
        if !(self.playing() && self.w.st.turn == i as i32) || self.w.st.step != stage::OPS || asking
        {
            return Some(Msg::new("err.play_phase"));
        }
        if let Some(why) = self.cannot_play(i, id) {
            return Some(Msg::new(why));
        }
        if let Some(card) = self.data.card(id) {
            let character = &self.w.st.players[i].character;
            let base = self.data.base_character(character);
            let crychic = self
                .data
                .character(character)
                .is_some_and(|c| c.band == "CRYCHIC");
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
        let Some(k) = self.w.hidden[i].hand.iter().position(|c| c == id) else {
            return Ok(());
        };
        self.w.hidden[i].hand.remove(k);
        // `PlayCtx.FromDeck` -- this one came out of the hand. **Scoped to the
        // play**: raised for the duration of this resolution and restored when
        // it ends, so a later play (or a skill press, which is not a hand press
        // at all) never sees a stale `true`. A nested `ctx::play_card` still
        // clears it for the card it runs and restores the outer value on the
        // way out. A halt mid-play leaves it set -- the play is still resolving
        // -- and the routine's re-run from its snapshot re-arms and restores it.
        let prev_from_hand = self.w.turn.play_from_hand;
        self.w.turn.play_from_hand = true;
        // `PlayCtx.Extreme` -- same scope as `FromDeck`: a [反击] may arm a
        // forced extreme mid-play (「以理论最大值或最小值结算」), and it must
        // not leak into the next play. A nested `ctx::play_card` starts plain
        // and restores the outer value on the way out. A halt mid-play leaves
        // it armed -- the play is still resolving -- and the replay re-derives
        // it from the snapshot.
        let prev_extreme = self.w.turn.extreme;
        self.w.turn.extreme = 0;
        self.w
            .log(
                "play",
                i as i32,
                Msg::new("log.play").player_id("who", i).card("card", id),
            )
            .card = id.to_string();
        if self.w.st.turn == i as i32 {
            self.w.turn.played.push(id.to_string());
        }
        self.wait(0.9);
        let t = raise!(
            self,
            "card",
            i,
            card = id.to_string(),
            by_card = Some(i as i32)
        )?;
        let rules = self.rules;
        // `Trigger.Cancelled` -- the play is negated; the card still goes to its
        // Dest below but its effect body does not run.
        let dest = if t.is_cancelled() {
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
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.card_removed").card("card", id),
                );
            }
            Dest::Field => {}
        }
        // `cardAfter` -- the card's own effect is fully resolved and it has
        // landed wherever its `Dest` sent it.
        raise!(
            self,
            "cardAfter",
            i,
            card = id.to_string(),
            by_card = Some(i as i32)
        )?;
        // `cardPlayed` (Fx) -- a card's hand effect resolved (its `Dest` is final).
        raise!(
            self,
            "cardPlayed",
            i,
            card = id.to_string(),
            by_card = Some(i as i32)
        )?;
        // The play is over: hand back the origin flag (`PlayCtx.FromDeck` is
        // scoped to the play, see the top of this routine) and the forced
        // extreme (`PlayCtx.Extreme`, same scope).
        self.w.turn.play_from_hand = prev_from_hand;
        self.w.turn.extreme = prev_extreme;
        Ok(())
    }

    // =============================================================== events

    /// `SetupEventDeck` -- every non-derived event, shuffled.
    fn setup_event_deck(&mut self) {
        let mut deck: Vec<String> = self
            .data
            .events
            .iter()
            .filter(|e| !e.derived)
            .map(|e| e.id.clone())
            .collect();
        self.w.rng.shuffle(&mut deck);
        self.w.event_deck = deck;
        self.w.event_discard.clear();
        self.w.event_removed.clear();
    }

    /// `DrawEvent` -- draw the top event and resolve it.
    ///
    /// Also the `ctx::draw_event` primitive (`docs/TILES.md`): `tile:event`'s
    /// body is 「抽取一张手卡，然后抽取一个事件卡」 and the event half is this.
    pub fn card_draw_event(&mut self, i: usize) -> Flow<()> {
        self.draw_event(i)
    }

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
        self.w
            .log(
                "event",
                i as i32,
                Msg::new("log.event")
                    .player_id("who", i)
                    .event("event", id.clone()),
            )
            .card = id.clone();
        self.wait(3.0);
        let t = raise!(self, "event", i, card = id.clone())?;
        let rules = self.rules;
        // `Trigger.Cancelled` -- the event's effect is negated; it is still
        // filed away below but does not resolve.
        let placed = if t.is_cancelled() {
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

    /// Counteraction window (C# `Counteract`), delegated to the card rules.
    ///
    /// Every raise site funnels through here (via [`raise!`] / [`Self::raise`]),
    /// so `step` is stamped centrally from the live turn step rather than at
    /// each call site.
    fn counteract(&mut self, t: &mut Trigger) -> Flow<()> {
        t.step = self.w.st.step;
        let rules = self.rules;
        rules.counteract(self, t)
    }

    /// Build-and-raise a [`Trigger`], returning it as [`Self::counteract`] left it so
    /// a counteraction's rewrite of a field (e.g. `moveRoll`'s reroll) can be read
    /// back. Prefer the [`raise!`] macro over calling this directly.
    fn raise(&mut self, mut t: Trigger) -> Flow<Trigger> {
        self.counteract(&mut t)?;
        Ok(t)
    }

    // =============================================================== end

    /// `CheckGameOver` -- one survivor, or every human out.
    pub(crate) fn check_game_over(&mut self) -> bool {
        let alive: Vec<usize> = (0..self.w.player_count())
            .filter(|&p| !self.out(p))
            .collect();
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
                    land += if st.mortgaged[t] {
                        tile.price / 2
                    } else {
                        tile.price
                    };
                    houses += st.houses[t] * tile.house;
                }
            }
            let s = &mut st.players[i];
            s.assets = s.money + land + houses;
            s.score = (s.money as f32 * st.score_money
                + land as f32 * st.score_property
                + houses as f32 * st.score_houses)
                .round() as i32;
        }
        let mut alive: Vec<usize> = (0..n).filter(|&p| !st.players[p].out()).collect();
        alive.sort_by_key(|&p| {
            (
                std::cmp::Reverse(Some(p) == champion),
                std::cmp::Reverse(st.players[p].score),
                p,
            )
        });
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
        self.w.log(
            "text",
            -1,
            Msg::new("log.match_end")
                .msg("why", why)
                .player_id("winner", winner),
        );
    }
}

#[cfg(test)]
mod tests;
