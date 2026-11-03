//! Match routines: turns, movement, landing, money, property, cards, events, scoring.
//!
//! The engine's generic match flow (「回合」/「移动」/「结算」/「支付」), shaped by
//! the 规则书 and `docs/ENGINE.md`. Card/skill/group hooks, active events
//! and per-player card variables are card content and enter through
//! [`super::rules::CardRules`]; card- or event-specific branches belong there,
//! not here.

use super::cx::{Ask, Flow, Halt, PlayCtx};
pub(crate) use super::move_ctx::MoveCtx as Move;
use super::move_ctx::MoveKind;
use super::rules::{raise, Dest, Fold, RollSource, Trigger, TriggerBuy, TriggerPay};
use super::world::{Extreme, Signal, WalkSeg, START_HAND, START_MONEY};
use crate::msg::{Arg, Msg};
use crate::state::{key, stage, Tick};
use game_kinds::move_plan::{self, PlanKind};
use game_kinds::{Payee, Seat, Tile};

/// The purchase surface (`docs/PURCHASE.md`): quote / deal / assign.
pub mod purchase;

/// Safety cap on nested money pipelines. The rulebook (「pay时可以打出」)
/// has no depth limit, so nested money movements past any reasonable depth
/// should still open their counteraction windows. The real termination argument is
/// that a hook cannot re-trigger on its own movement (`PlayCtx::reentrant_hooks`);
/// this is the runaway guard behind it and **traps loudly** rather than
/// settling silently. See `docs/ENGINE.md`.
pub(crate) const MAX_MONEY_DEPTH: u32 = 32;

/// Q3 (PIPELINE-AUDIT, user ruling 2026-10-07): how many times a payment may
/// flip direction. A negative final amount settles the original at 0 and
/// starts a **new** payment with the participants swapped; if that reverse
/// payment is itself driven negative it reverses again. Cap the flip-flop
/// here and settle the leftover as nothing (logged).
pub(crate) const MAX_PAY_REVERSALS: u8 = 8;

/// Q7 (`SETTLE-STAGES.md` §7, user ruling 2026-10-07): how many times a
/// settlement may be re-targeted by a relocation during `moveAfter` /
/// `settleBefore`. Each redirect re-raises `settleBefore` at the new tile;
/// after this many the settle runs where the mover is standing (and logs).
/// Same spirit as the engine's other loop caps.
pub(crate) const MAX_SETTLE_REDIRECTS: u32 = 8;
/// Rulebook-level runaway guard (STACK-01): two cards can settle each other
/// forever (笑容大游行's `SettleBody` self-nest is one real instance). The
/// work-stack keeps the **Rust stack** flat; this cap is only the rules-level
/// termination rule -- log and stop after N nested settles.
/// TODO(规则书): the book states no settle-depth bound.
pub const MAX_SETTLE_DEPTH: u32 = 64;

/// Fold the plan's **shape** writes into the running move -- but only the ones
/// the `moveBefore` window (or any earlier writer) actually changed.
///
/// `turn.plan` is what cards write (`plan::set_*`) and what later stages read
/// (`scale_settle_payment` and kin); `m` is the in-flight move the engine has
/// already stamped with its run fields (roll / path / `player_id` / …). The
/// plan is never reset between moves, so a blanket copy would leak one move's
/// `no_buy` / `can_build` / `pay_factor` into every later one. Compare against
/// the plan as it stood before the window and apply only the delta -- and
/// **never** copy `m` over the plan (that wipes the earlier writes).
/// Money rounding (user ruling 2026-10-15, OPEN-取整 family): round **up** to a
/// multiple of 10. Applied once, after every add/mul modifier of a payment has
/// been processed (`money_inner`), and to a pre-split total that some
/// `payCombined*` hook rewrote (`pay_total`). Non-positive figures are returned
/// unchanged -- a negative is reversed into a child payment and must not be
/// CeilTo'd toward +inf first.
///
/// Distinct from [`purchase::half_ceil10`] (half-rent 「向上取整10」) and from
/// the kani `round10` twins: those take a half already taken; this one is the
/// end-of-calc money round and is positive-only.
pub(crate) fn ceil10_money(x: i32) -> i32 {
    if x <= 0 {
        return x;
    }
    ((x as i64 + 9) / 10 * 10).clamp(0, i32::MAX as i64) as i32
}

/// Multiply `amount` by a milli factor (1_000_000 = ×1) -- the engine's own
/// contribution to a mul phase, folded exactly like a responder's answer so it
/// composes with the card stages whatever their order (MONEY-01).
fn apply_milli_factor(amount: i32, milli: i64, modified: &mut bool) -> i32 {
    if milli == 1_000_000 {
        return amount;
    }
    *modified = true;
    // Q3: no floor here. A scaled negative remains negative and is reversed
    // after the chain resolves.
    (i64::from(amount).saturating_mul(milli) / 1_000_000)
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// The `log.dice` line a move-plan roll posts ([`PlayCtx::card_do_move_roll`]).
///
/// The first `base`/`dice` term becomes the `count`d`sides` pair -- a flat
/// term is a `0`-sided `count`, the same convention [`PlayCtx::card_roll`] uses --
/// and any further terms ride `extra` (`+1d4`, `+2`). Empty tables are `0d0`,
/// the roll of nothing. This is what keeps 松原花音's second 「移动掷骰」
/// reading 「掷 1d20 = 5」 instead of the old hardcoded 「掷 0d0 = 5」.
pub(crate) fn plan_dice_msg(player_id: i32, plan: &Move, total: i32) -> Msg {
    let mut terms = plan.base.iter().chain(plan.dice.iter());
    let (count, sides) = match terms.next() {
        Some(t) => (t.dice.count, t.dice.sides.max(0)),
        None => (0, 0),
    };
    let mut extra = String::new();
    for t in terms {
        extra.push('+');
        if t.dice.sides > 0 {
            extra.push_str(&format!("{}d{}", t.dice.count, t.dice.sides));
        } else {
            extra.push_str(&t.dice.count.to_string());
        }
    }
    let mut msg = Msg::new("log.dice")
        .player_id("who", player_id)
        .i("count", count)
        .i("sides", sides);
    if !extra.is_empty() {
        msg = msg.text("extra", extra);
    }
    msg.i("sum", total).opt("detail", None)
}

fn fold_plan_delta(m: &mut Move, before: &Move, plan: &Move) {
    // Copy on change in either direction: a window may *clear* a field
    // (`teleport_to`, `steps`, `parity`) as well as set one.
    if plan.kind != before.kind {
        m.kind = plan.kind;
    }
    // Another seat rolls this one (`幻觉来了` 「上一名玩家代替进行此次投掷」).
    if plan.roller != before.roller {
        m.roller = plan.roller;
    }
    if plan.teleport_to != before.teleport_to {
        m.teleport_to = plan.teleport_to;
    }
    if plan.settles != before.settles {
        m.settles = plan.settles;
    }
    if plan.base != before.base {
        m.base = plan.base.clone();
    }
    if plan.dice != before.dice {
        m.dice = plan.dice.clone();
    }
    if plan.min_roll != before.min_roll {
        m.min_roll = plan.min_roll;
    }
    if plan.signed != before.signed {
        m.signed = plan.signed;
    }
    if plan.steps != before.steps {
        m.steps = plan.steps;
    }
    if plan.extra_steps != before.extra_steps {
        m.extra_steps = plan.extra_steps;
    }
    if plan.start != before.start {
        m.start = plan.start;
        m.start_reason = plan.start_reason.clone();
    }
    if plan.reverse != before.reverse {
        m.reverse = plan.reverse;
    }
    if plan.stop_at != before.stop_at {
        m.stop_at = plan.stop_at;
    }
    if plan.parity != before.parity {
        m.parity = plan.parity;
    }
    if plan.settle_tile != before.settle_tile {
        m.settle_tile = plan.settle_tile;
    }
    if plan.pay_factor != before.pay_factor {
        m.pay_factor = plan.pay_factor;
    }
    if plan.rent_factor != before.rent_factor {
        m.rent_factor = plan.rent_factor;
    }
    if plan.settle_as_agent != before.settle_as_agent {
        m.settle_as_agent = plan.settle_as_agent;
    }
    if plan.can_build != before.can_build {
        m.can_build = plan.can_build;
    }
    if plan.more_steps != before.more_steps {
        m.more_steps = plan.more_steps;
    }
    if plan.no_buy != before.no_buy {
        m.no_buy = plan.no_buy;
    }
    if plan.cancelled != before.cancelled {
        m.cancelled = plan.cancelled;
    }
    if plan.tags != before.tags {
        m.tags = plan.tags.clone();
    }
    if plan.by_skill != before.by_skill {
        m.by_skill = plan.by_skill;
    }
    if plan.main_override != before.main_override {
        m.main_override = plan.main_override;
    }
    if plan.void_walk != before.void_walk {
        m.void_walk = plan.void_walk;
    }
}

/// Why a payment is happening -- what caused the money to move. Distinct from
/// [`PayEvent`] (what the log line is called); a rent payment logs as `rent`,
/// a forced loss as `lose`, and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PayKind {
    /// 「pay」 rent on a landed deed.
    Rent,
    /// A purchase (land / agent / card / auction).
    Buy,
    /// A build (「加盖房屋」).
    Build,
    /// A print (「gain」) -- money in.
    Gain,
    /// 「强行购买」 -- the force-buy pipeline pay.
    ForceBuy,
    /// A card-driven `pay` / `gain` from a guest body (the old kind string
    /// `"card"`). Scaled by 「支付」 settle factors (it is *not* in the
    /// never-scale set), and never rent.
    #[default]
    Card,
    /// 「受到损失」 -- a raw loss off the payer (Mashiro's redirect lands here).
    /// Logs as a loss and keeps the declared `rent` flag.
    Loss,
}

impl PayKind {
    /// The stable spelling used in code and in the scaled-settle filter.
    pub const fn as_str(self) -> &'static str {
        match self {
            PayKind::Rent => "rent",
            PayKind::Buy => "buy",
            PayKind::Build => "build",
            PayKind::Gain => "gain",
            PayKind::ForceBuy => "forcebuy",
            PayKind::Card => "card",
            PayKind::Loss => "loss",
        }
    }

    /// The guest wire code (the `pay_kind` pre reads it).
    pub const fn as_i32(self) -> i32 {
        match self {
            PayKind::Card => 0,
            PayKind::Rent => 1,
            PayKind::Buy => 2,
            PayKind::Build => 3,
            PayKind::Loss => 4,
            PayKind::ForceBuy => 5,
            PayKind::Gain => 6,
        }
    }

    /// Decode the guest wire code; an unknown code is `Card`.
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => PayKind::Rent,
            2 => PayKind::Buy,
            3 => PayKind::Build,
            4 => PayKind::Loss,
            5 => PayKind::ForceBuy,
            6 => PayKind::Gain,
            _ => PayKind::Card,
        }
    }
}

/// How a payment is shaped. `Normal` is the full pipeline; `Fixed` is the
/// immutable leg (a responder cannot touch amount, target, cancel or spare, and
/// no factor or rounding applies); `PayeeHalf` charges the declared payee half
/// of what the payer lost, as a fresh `Fixed` child ([`PlayCtx::money`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PayShape {
    #[default]
    Normal,
    Fixed,
    PayeeHalf,
}

impl PayShape {
    /// Decode the guest wire code; an unknown code is `Normal`.
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => PayShape::Fixed,
            2 => PayShape::PayeeHalf,
            _ => PayShape::Normal,
        }
    }
}

/// Which scope a pay factor belongs to (guest code order). `Auto` is not a
/// wire scope: `set_pay_factor` resolves it to the innermost live scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayFactorScope {
    /// The innermost live scope, for the unscoped `set_pay_factor` (a
    /// `thisSettle` in a settle, else `thisMove` in a move, else `nextMove`).
    Auto,
    NextMove,
    ThisMove,
    NextSettle,
    ThisSettle,
}

impl PayFactorScope {
    /// Decode the guest wire code; an unknown code is `NextMove`.
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => PayFactorScope::ThisMove,
            2 => PayFactorScope::NextSettle,
            3 => PayFactorScope::ThisSettle,
            4 => PayFactorScope::Auto,
            _ => PayFactorScope::NextMove,
        }
    }
}

/// The log-line flavour of one money movement. These are exactly the
/// [`crate::state::MatchEvent::kind`] values a money event carries on the
/// wire -- serde / the event log remain stringly at the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayEvent {
    /// A rent payment.
    Rent,
    /// A payment from one player to another.
    Pay,
    /// Money left a player (「spend」 / a buy / a build).
    Lose,
    /// Money reached a player (「gain」).
    Gain,
    /// The the start square pass reward.
    Pass,
}

impl PayEvent {
    /// The wire / event-log spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            PayEvent::Rent => "rent",
            PayEvent::Pay => "pay",
            PayEvent::Lose => "lose",
            PayEvent::Gain => "gain",
            PayEvent::Pass => "pass",
        }
    }
}

/// One money movement: print (`from = None`), delete (player -> game,
/// `to = None`) and pay-player (both `Some`) -- whatever caused it (rent,
/// card, skill, the start square reward, buy, build, ...). See [`PlayCtx::money`].
#[derive(Debug, Clone)]
pub struct Pay {
    pub from: Option<usize>,
    pub to: Option<usize>,
    pub amount: i32,
    /// What caused this payment.
    pub kind: PayKind,
    /// How the payment is shaped ([`PayShape`]).
    pub shape: PayShape,
    /// Is this payment rent? Declared, not derived: Mashiro keeps 「同等的资金」
    /// a rent while `kind` becomes `Loss`, so the rent factor still applies.
    pub rent: bool,
    /// The declared payee of a [`PayShape::PayeeHalf`] leg, captured before any
    /// response. Runtime-only (derived from the declared `to` on replay).
    pub partner: Option<usize>,
    /// Event type; derived from `kind`/direction when `None`.
    pub typ: Option<PayEvent>,
    pub tile: Option<usize>,
    /// Mandatory: raise funds (mortgage, then bankruptcy) if short.
    pub must: bool,
    /// Offer a cancellable mortgage for an optional purchase/build.
    pub allow_mortgage: bool,
    /// Log line; gets the amount as the `amount` argument. A **full** line
    /// (`log.paid_for` and kin), never a bare reason -- see [`Pay::reason`].
    pub text: Option<Msg>,
    /// Key naming where the money came from (`src.*`), shown in parentheses.
    pub source: &'static str,
    /// The cause of the move, shown in parentheses on the standard line the
    /// same way [`Pay::source`] is -- but as a full [`Msg`], for a card's own
    /// reason key (`cards:<crate>.<key>`, e.g. 「登上武道馆」) which is not a
    /// static `src.*` key. Prefer this over [`Pay::text`] for card-driven
    /// moves: `text` replaces the whole line and so must carry the action
    /// (who / to / amount) itself, which a bare card name does not.
    pub reason: Option<Msg>,
    /// The player whose card caused this payment, or `None` when the payment
    /// is board-driven (rent, buy, build). Stamped onto the `pay` / `paid`
    /// triggers so 「被…效果影响」 can tell the two apart.
    pub by_card: Option<i32>,
    /// Run the command-wide **pre-split** stage (`payCombinedAdd` → `payCombinedMul`
    /// → `payCombinedCancel`) on this payment's amount. `true` for every ordinary
    /// payment: a single-pair payment's command total *is* its amount. A
    /// 「[分摊]」 leg sets it `false` -- the card body has already shaped the
    /// command total through [`PlayCtx::pay_total`] before dividing it into shares
    /// (`PIPELINE-AUDIT` Q2).
    pub runs_total_stage: bool,
    /// Q3 (PIPELINE-AUDIT, user ruling 2026-10-07): how many times this
    /// payment has already reversed direction. A reverse child starts at
    /// `parent + 1` and is refused past [`MAX_PAY_REVERSALS`].
    pub reverse_depth: u8,
    /// An extra milli-scale (1000 = ×1.0) the **caller** of the payment baked
    /// into the figure's formula -- `ctx::charge_rent`'s `milli_scale`
    /// (「(4-X)/4倍价格」), not a standing tile/plan factor. Multiplied in
    /// [`Play::scale_settle_payment`] alongside `RENT_FACTOR` / `pay_factor`,
    /// so every add/mul of one payment composes at the `paySplitMul` stage and
    /// the one end-of-calc `ceil10` sees the fully shaped figure
    /// (`spec/ROUNDING.md`). A reverse child does **not** copy this: its
    /// amount is already the post-scale figure.
    pub milli_scale: i32,
}

impl Pay {
    pub fn new(amount: i32, kind: PayKind) -> Self {
        Self {
            from: None,
            to: None,
            amount,
            kind,
            shape: PayShape::Normal,
            rent: kind == PayKind::Rent,
            partner: None,
            typ: None,
            tile: None,
            must: false,
            allow_mortgage: false,
            text: None,
            source: "",
            reason: None,
            by_card: None,
            runs_total_stage: true,
            reverse_depth: 0,
            milli_scale: 1000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Paid {
    /// Did the payment commit? `true` for any commit, a `money_locked` zero
    /// included -- the receipt packs this as `SETTLED`.
    pub paid: bool,
    /// What the payer lost (0 for a print).
    pub loss: i32,
    /// What the payee gained (0 for a delete).
    pub gain: i32,
    /// A mandatory payment the payer could not complete -- they went out
    /// through `raise_funds` ([`super::Paid::shortfall`] on the receipt).
    pub shortfall: bool,
}

impl Paid {
    /// The amount that actually moved, whichever side it left from.
    pub fn moved(&self) -> i32 {
        self.loss.max(self.gain)
    }
}

/// Where a move's dice came from. A plan tagged `fireRoll` was funded by the
/// fire pot (the pre-G03 signal; G03's roll transaction replaces it), so its
/// roll reads as `Charge` on every ladder window -- `misaki_card`'s
/// 「使用火罐进行移动掷骰后」 pre is `roll_source == 1`.
fn move_roll_source(m: &Move) -> RollSource {
    if m.tags.iter().any(|(k, v)| k == "fireRoll" && *v != 0) {
        RollSource::Charge
    } else {
        RollSource::Unattributed
    }
}

impl PlayCtx<'_> {
    // =============================================================== turn flow

    /// Match start without prompts (the host then runs `opening`).
    pub(crate) fn begin_play(&mut self) {
        let n = self.data.tiles.len();
        let st = &mut self.w.st;
        st.phase = "play".into();
        st.owners = vec![-1; n];
        st.houses = vec![0; n];
        st.mortgaged = vec![false; n];
        st.embers = vec![0; n];
        let players = st.players.len();
        for i in 0..players {
            self.w.st.players[i].money = START_MONEY;
            self.w.st.players[i].pos = 0;
            let mut draw = std::mem::take(&mut self.w.hidden[i].deck);
            self.w.rng.shuffle(&mut draw);
            self.w.hidden[i].deck = draw;
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

    /// The three match-start points, the opening deal, one mulligan.
    ///
    /// Ordered (the accepted match-start lifecycle):
    ///
    /// 1. **before match start** (`deckBeforeGame`), before the opening hands
    ///    are drawn: start positions and the authoritative **initial hand size**
    ///    (default 2; effects may lower it, minimum 0) are decided here.
    /// 2. the opening deal and mulligan, reading that hand size.
    /// 3. **after match start** (`deckAtGameStart`), after the deal: initial
    ///    tokens/resources (spendable pots 「初始N」, P✽P fans) are created here.
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
        // 2. 初始重抽 -- **every** seat decides, not only the human ones. The
        // window is a hard barrier (rulebook 时点流程: step 6 completes before
        // step 7's 游戏开始时 effects and step 8's first turn): no seat may
        // activate an effect or advance the game until every seat has an
        // answer on record. Bots decide through their policy, a seat the
        // machine has taken over answers as the machine, and a seat that
        // never answers is forced into "keep" only when the window runs out
        // -- the choice is not cut short by the 恢复时间 pool (`use_pool` off).
        //
        // A table whose every seat is machine-decided has nothing to wait
        // for, so it skips the ask and applies the same policy answers
        // directly (`ai_answer_for_entry` is the exact `fill_ai` body, same
        // RNG draws). Sims and bot-only matches must not halt on a prompt no
        // one reads, and their chaos seats must still re-draw exactly as they
        // would in a mixed table.
        let all: Vec<usize> = (0..self.w.player_count()).collect();
        if !all.is_empty() {
            let n = all
                .iter()
                .map(|&p| self.start_hand(p))
                .max()
                .unwrap_or(START_HAND);
            self.w
                .log("text", -1, Msg::new("log.mulligan_offer").i("n", n as i64));
            let mut ask = Ask::choice(
                all.clone(),
                Msg::new("ask.mulligan.title"),
                Msg::new("ask.mulligan.text").i("n", n as i64),
                vec![
                    Msg::new("ask.mulligan.keep").i("n", n as i64),
                    Msg::new("ask.mulligan.redo"),
                ],
                0,
                20.0,
            )
            .with_kind("mulligan");
            // Not an ordinary prompt: keep the seat's 恢复时间 pool out of it.
            ask.use_pool = false;
            let answers: Vec<i32> = if all.iter().all(|&p| self.w.st.players[p].auto_setup()) {
                let mut a = Vec::with_capacity(all.len());
                for &p in &all {
                    let v = self.ai_answer_for_entry(&ask, p);
                    a.push(v);
                }
                a
            } else {
                let r = self.ask(ask)?;
                all.iter().map(|&p| r.of(p)).collect()
            };
            let redo: Vec<usize> = all
                .iter()
                .copied()
                .zip(&answers)
                .filter(|&(_, &v)| v == 1)
                .map(|(p, _)| p)
                .collect();
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

    /// Return the hand to the deck and draw a fresh opening hand.
    fn mulligan(&mut self, i: usize) -> Flow<()> {
        let h = &mut self.w.hidden[i];
        let hand = std::mem::take(&mut h.hand);
        h.deck.extend(hand);
        let mut draw = std::mem::take(&mut self.w.hidden[i].deck);
        self.w.rng.shuffle(&mut draw);
        self.w.hidden[i].deck = draw;
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

    /// Queue the next turn and start it.
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
        st.skip_move = false;
        st.landed = -1;
        st.roller = next;
        self.w.turn = super::world::TurnCtx {
            player_id: i,
            extra,
            ..Default::default()
        };
        // A preview cast (`ctx::roll` with a move purpose outside a move) is
        // kept for the whole turn so its id stays addressable; the turn's
        // rolls are over, so drop them with it.
        self.w.open_rolls.clear();
        self.w.signals.push(Signal::TurnBegan);
        let text = Msg::new(if extra { "log.turn_extra" } else { "log.turn" })
            .i("round", self.w.st.round)
            .player_id("who", i);
        self.w.log("turn", next, text);
        self.turn_start(i)
    }

    /// Generic auto-skip latch (`World::end_turn_request`): true when a guest
    /// asked to end `i`'s turn. Consumed here so one request closes at most
    /// one turn; a request aimed at another seat waits for their turn. The
    /// host logs its own skip line; the engine only paces (`wait(1.0)`) and
    /// calls [`Self::end_turn`] -- same shape as the old status auto-skips.
    fn take_end_turn_request(&mut self, i: usize) -> bool {
        if self.w.end_turn_request == Some(i as i32) {
            self.w.end_turn_request = None;
            true
        } else {
            false
        }
    }

    /// Turn start: status effects tick, then the clock starts.
    fn turn_start(&mut self, i: usize) -> Flow<()> {
        if self.out(i) {
            return Ok(());
        }
        // The between-turns target counter. Each player's own turn start
        // zeroes its entry (「本回合被其他玩家的卡designate过」 counts since its own
        // turn last started).
        if let Some(n) = self.w.targeted.get_mut(i) {
            *n = 0;
        }
        // 「当前回合内你每获得过一次资金」 is per **turn**, not per own-turn:
        // a counteraction read at a turn's end (HHW:（育美）(2), 「你回合外收到资金的回合
        // 结束时」) sees just that turn's credits. Everyone's counter starts
        // fresh with the turn.
        self.w.reset_gains();
        // 「本回合内」 counterers (无法将视线移开) start fresh too.
        self.w.reset_countered();
        // Pending pay factors are turn-scoped: a `nextMove` / `nextSettle`
        // write lands on exactly one move / settle and the turn starts them
        // clean. The per-seat settle counter is stamped through this turn.
        self.w.turn.next_move_pay_factor = 1000;
        self.w.turn.next_settle_pay_factor = 1000;
        self.w.turn.next_settle_pay_add = 0;
        self.w.turn.factor_applied.clear();
        self.w.turn.paid_in_settle_by = vec![0; self.w.st.players.len()];
        // 「a new turn starts them all at 0」. The turn ctx is rebuilt at
        // `next_turn`, so this is already empty there; zero it again at the
        // turn-start boundary so a world carried across a card's run cannot
        // leave a stale hit behind.
        self.w.turn.abnormal.clear();
        // `linger` instances expire at turn start (`docs/PURCHASE.md`).
        // TODO(规则书) ruling 10: 「本回合」 linger effects in an extra turn or
        // another player's turn -- today they last exactly one turn.
        let round = self.w.st.round;
        self.w.turn.lingering.retain(|l| l.expires > round);
        // `turnStartBefore` -- before any status ticks settles, so a
        // counteraction sees the turn exactly as it was left last turn. Status
        // hosts (e.g. the off-board host's layer tick) run as field hooks collected
        // last inside this raise (`docs/STATUS-HOSTS.md` §4).
        raise!(self, "turnStartBefore", i, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        // Generic auto-skip latch (`end_turn_now`): the host logs its own line
        // and asks the engine to close the turn. Same pacing as every status
        // skip -- one beat, then `end_turn` (which raises only `endTurnAfter`).
        if self.take_end_turn_request(i) {
            self.wait(1.0);
            return self.end_turn(i);
        }
        let s = &mut self.w.st.players[i];
        // Timed counters due now -- the item says so, not the engine naming
        // the pending-layer counter. The hand-gate 2->1 fold is that host's `TurnStart`
        // hook (in `rules/status`), not an inline dance here.
        s.tick_state(Tick::TurnStart);
        // Snapshot the turn's starting square and keyed state (`turn_snapshot` /
        // `turn_start_pos`): 「在场地地块开始回合时」 is about the starting
        // square, and 「回到起始地点并取消所有受到的效果」 restores from here.
        self.w.turn.turn_start_pos = self.w.st.players.iter().map(|p| p.pos).collect();
        self.w.turn.turn_snapshot = self
            .w
            .st
            .players
            .iter()
            .map(|p| super::world::TurnSnapshot {
                pos: p.pos,
                state: p.state.iter().map(|(k, v)| (k.clone(), v.value)).collect(),
                money: p.money,
                owners: self.w.st.owners.clone(),
                houses: self.w.st.houses.clone(),
                mortgaged: self.w.st.mortgaged.clone(),
            })
            .collect();
        raise!(self, "turnStart", i, tile = self.w.st.players[i].pos)?;
        if self.out(i) {
            return Ok(());
        }
        // Same generic latch as above -- a `turnStart` hook may also close the
        // turn (`end_turn_now`).
        if self.take_end_turn_request(i) {
            self.wait(1.0);
            return self.end_turn(i);
        }
        // The turn clock is a modifier point: hooks rewrite the 免费时间
        // refill and the 恢复时间 (seconds) before the host applies them.
        // `turnBaseTime`'s default is the match's per-turn base refill
        // (`ClockSettings::base_refill`), not a fixed constant -- the host
        // then refills/caps it at `StartTimer` (see `engine::Signal`).
        let refill = raise!(self, "turnBonusTime", i, value = super::BONUS_REFILL as i32)?.value;
        let base = raise!(
            self,
            "turnBaseTime",
            i,
            value = self.w.clock.base_refill as i32
        )?
        .value;
        self.w.signals.push(Signal::StartTimer(
            i,
            refill.max(0) as f32,
            base.max(0) as f32,
        ));
        // The ruleset's act / move gates (`docs/STATUS-HOSTS.md` §4).
        // A closed act gate skips the whole turn; a closed move gate skips
        // only the main move. Skip messages come from the ruleset.
        let (act, mv) = (
            self.w.gate_open(self.rules, super::Gate::Act, i),
            self.w.gate_open(self.rules, super::Gate::Move, i),
        );
        // 「（或无法移动的回合结束时）」: a turn that cannot take its main move
        // withholds the landing settle (the move goes nowhere) and moves it to
        // the turn end. Stamp **before** the act-gate return -- [晕眩] closes
        // both gates, and the act skip never reaches the move branch below.
        // The stamp is the default only: an answer the card gave when the
        // blocking effect landed keeps it (G03 §4.7).
        use super::world::SettleAnswer;
        if (!act || !mv) && self.w.blocked_settle_answer(i) == SettleAnswer::Unanswered {
            self.w.set_blocked_settle_answer(i, SettleAnswer::Settle);
        }
        if !act {
            self.w.log(
                "status",
                i as i32,
                self.rules.gate_skip_msg(super::Gate::Act, i),
            );
            self.w.st.step = stage::END;
            self.wait(1.2);
            return self.end_turn(i);
        }
        if !mv {
            self.w.st.skip_move = true;
            self.w.log(
                "status",
                i as i32,
                self.rules.gate_skip_msg(super::Gate::Move, i),
            );
        }
        // 开始阶段 is over: status has ticked, `turnStart` has run, and the
        // the gate checks have run. The stage *remains* 开始 for a beat first,
        // though -- exactly as 结束 holds after the walk -- so the stage swap
        // has time to read before 运营 opens. `tick_play` lifts the turn into
        // 运营 once this delay expires. (A player with a closed act gate left above, so 眩晕
        // skips 运营 and 移动, as the rulebook says.)
        self.w.st.roller = i as i32;
        self.wait(1.2);
        Ok(())
    }

    /// The player's "end turn" command.
    pub(crate) fn end_turn_cmd(&mut self, i: usize) -> Flow<()> {
        // The turn end is a phase, not an edge: once it is up (结束 after
        // the walk, or queued by an auto-skip) a second `end` must be a
        // no-op -- otherwise it re-runs the whole end phase, wears another
        // status layer off, re-raises `turnEnd` / `turnEndAfter` and logs
        // 「结束了回合」 again. Clients do send it twice: an accepting seat's
        // own duplicate (a view that lags the state) and each other seat's
        // client racing the same frame. The record keeps the extra commands,
        // so a replay must fold them here too. `do_act` already folds at the
        // gate; this catches a routine that was in flight.
        if self.w.end_already_queued() {
            return Ok(());
        }
        // `endTurnBefore` -- the player chose to end the turn. (Auto-skips --
        // a closed act gate -- go straight to [`Self::end_turn`] and so raise only
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

    /// End of turn: status effects wear off, next turn queued.
    pub(crate) fn end_turn(&mut self, i: usize) -> Flow<()> {
        self.w.st.step = stage::END;
        // The offer claim is one move's; a claim staging is still holding came
        // from outside a move (see `main_move`), so it dies with this turn.
        // Writes later in the end hooks are for the next turn's move and stay.
        self.w.turn.plan.claimed = false;
        // A blocked main move's withheld settle (G03 §4.7). The blocked branch
        // stamped `Settle` by default and a card could only decline with
        // `Skip`; run the ordinary landing settle at the seat's own tile (the
        // walk went nowhere) before the turn-end raises -- the full stage
        // path (`settleBefore` -> `settle` -> `settleBody` -> the tile rule ->
        // `settleAfter` -> `tileResolved`), not the built-in fallback.
        if !self.out(i)
            && self.playing()
            && self.w.blocked_settle_answer(i) == super::world::SettleAnswer::Settle
        {
            let at = self.w.st.players[i].pos;
            let mut m = Move::default();
            m.player_id = i;
            m.from = at;
            m.to = at;
            m.settles = true;
            self.after_walk(&mut m)?;
        }
        self.w
            .set_blocked_settle_answer(i, super::world::SettleAnswer::Unanswered);
        // End-phase sequence: `turnEndBefore` (also the `before_turn_end`
        // callbacks) -> status wear-off -> `turnEnd` -> `turnEndAfter` (also
        // the `at_turn_end` callbacks) -> `endTurnAfter`.
        raise!(self, "turnEndBefore", i)?;
        if self.out(i) || !self.playing() {
            // An out player / ended match skips the rest.
            self.w.next_turn_pending = true;
            return Ok(());
        }
        {
            let s = &mut self.w.st.players[i];
            // Timed counters due now (timed layers wear off here). The
            // item carries its own expiry, so this names no keys. The hand-gate
            // 1->0 clear is that host's `TurnEnd` hook, not inline.
            s.tick_state(Tick::TurnEnd);
        }
        // 「并在回合结束时额外进行一次[触发结算]」 is no longer an engine
        // counter: it is a scheduled turn-end rule op (`On::Schedule`, whose body
        // calls `ctx::settle`) -- `docs/TILES.md`.
        self.w.next_turn_pending = true;
        // `turnEnd` (hook point) -- every placed card decays/acts here.
        raise!(self, "turnEnd", i)?;
        // `turnEndAfter` (hook) -- also runs the `at_turn_end` callbacks, so a
        // the layer they grant survives this turn's wear-off.
        raise!(self, "turnEndAfter", i)?;
        // `endTurnAfter` -- the turn is over and the next one is queued. Raised
        // for every turn end, including auto-skips.
        raise!(self, "endTurnAfter", i)?;
        Ok(())
    }

    // =============================================================== movement

    /// Card-driven bare teleport (`ctx::teleport_to` / `HostRequest::Teleport`):
    /// a position write with no settle. Runs through [`Self::teleport_as`] so the
    /// move tail (`moveBefore` / `moveAfter` / `moveResolved`) raises like every
    /// other movement case (B41 raises [pass],overlap at the endpoint;
    /// `settles = false` gates only the settle stages).
    pub fn card_teleport(&mut self, player_id: usize, tile: usize) -> Flow<()> {
        if self.move_depth > 0 {
            let from = self.w.st.players[player_id].pos;
            let by_skill = self.w.plan().by_skill;
            // Nested jump (a `passTile` / move hook calling `ctx::teleport_to`):
            // a bare position write, no second move tail -- the outer move's
            // raise is already in flight and a nested `teleport_as` would
            // re-enter the same hook. A skill jump still gets the tail (lisa's
            // 「使用技能进行传送后」 window) under a side frame.
            self.w.teleport_to(player_id as i32, tile as i32);
            if by_skill {
                self.teleport_tail_only(player_id, from, tile as i32)?;
            }
            return Ok(());
        }
        self.teleport_as(player_id, tile, false, None, false)?;
        Ok(())
    }

    /// Raise the movement tail for a teleport nested inside another move's
    /// stage (a skill jump in a settle). Position is written by the caller;
    /// this raises `moveAfter` then `moveResolved` on a synthetic teleport Move
    /// under its own side frame, so a tail counter's writes die with it.
    fn teleport_tail_only(&mut self, i: usize, from: i32, to: i32) -> Flow<()> {
        if self.in_teleport_tail {
            return Ok(());
        }
        self.in_teleport_tail = true;
        let target = self.w.plan().clone();
        self.w.begin_plan(i, PlanKind::SideMove, target);
        let mut m = Move::new(i);
        m.kind = MoveKind::Teleport;
        m.by_skill = true;
        m.settles = false;
        m.from = from;
        m.to = to;
        m.teleport_to = Some(Tile(to));
        m.path = vec![to];
        m.total = 1;
        let r = (|| -> Flow<()> {
            self.move_after(&mut m)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            self.move_resolved(&mut m)
        })();
        self.w.end_plan(true);
        self.in_teleport_tail = false;
        r
    }

    /// Run a card-shaped movement **now**. A move of the turn player is the
    /// main move unless it is forced (`plan.forced`, 「移动X」 / 「移动1d20」 --
    /// an event move goes through even when the slot is spent, `docs/EVENTS.md`)
    /// or `plan.main_override` says the forced move is 「视为主要移动」.
    ///
    /// Every move runs on its own plan frame (`World::begin_plan`): the frame is
    /// seeded from the activation's command when there is one, so the move's
    /// command can be dropped at its end without the write leaking to the next.
    /// A call from a move's own roll window that carries `main_override` +
    /// `void_walk` + a destination replaces the pending walk instead
    /// (`spec/abi/G04-move-plan.md` §4.4).
    ///
    /// The answer is **"the move ran"**: `false` on a spent main slot, a
    /// start-time cancellation, or a replacement the window cancelled.
    pub fn run_planned_move(&mut self, player_id: usize, plan: Move) -> Flow<bool> {
        let i = player_id;
        // The plan the body shaped is the frame/staging target. The host passes
        // the guest's captured copy: a paused body's writes live in the run's
        // world copy, not in the live staging, so the live plan can be stale.
        let target = plan;
        let is_turn = self.w.st.turn == i as i32;
        let is_main_move = move_plan::is_main_move(is_turn, target.forced, target.main_override);
        let owner = self.w.turn.plan_stack.last().map_or(i, |f| f.owner);
        let replace = move_plan::replacement_taken(
            self.w.move_roll_windows.last().copied(),
            owner,
            target.main_override,
            target.void_walk,
            target.teleport_to.is_some(),
        );
        if replace {
            return self.replace_main_move(i, &target);
        }
        if is_main_move && self.w.turn.main_moved {
            self.w.log(
                "text",
                i as i32,
                Msg::new("log.main_move_used").player_id("who", i),
            );
            self.w.turn.plan_scope.moves += 1;
            return Ok(false);
        }
        self.w.turn.plan_scope.moves += 1;
        if is_main_move {
            self.w.turn.main_moved = true;
        }
        let kind = if is_main_move {
            PlanKind::MainMove
        } else {
            PlanKind::SideMove
        };
        self.w.begin_plan(owner, kind, target);
        let r = self.run_move_body(i, None, is_main_move);
        self.w.end_plan(r.as_ref().is_ok_and(|&ran| ran));
        r
    }

    /// Stamp the engine-owned identity and progress of a move onto it. The plan
    /// frame is the move's *shape*; these fields are not `PlanField`s, so a
    /// body that re-reads the frame after a window re-stamps them here.
    fn stamp_move_identity(
        &self,
        m: &mut Move,
        i: usize,
        roller: Option<usize>,
        is_main_move: bool,
    ) {
        m.player_id = i;
        if let Some(r) = roller {
            m.roller = Some(Seat(r as i32));
        } else if m.roller.is_none() {
            m.roller = Some(Seat(i as i32));
        }
        m.is_main_move = is_main_move;
        m.from = m.start.map_or_else(|| self.w.st.players[i].pos, |t| t.0);
        m.to = m.from;
        m.path.clear();
        m.total = 0;
        m.remaining = 0;
        m.passed_players.clear();
        m.stopped = false;
        m.cancelled = false;
    }

    /// Run the movement of the already-open top frame.
    ///
    /// `diceRollBefore` is the turn move's alone; the rest (a `rollPlan` cast
    /// when there is no step count, the ladder, the walk, the tail) is shared
    /// with card moves (`spec/abi/G04-move-plan.md` §4.2).
    fn run_move_body(&mut self, i: usize, roller: Option<usize>, is_main_move: bool) -> Flow<bool> {
        let mut m = self.w.plan().clone();
        self.stamp_move_identity(&mut m, i, roller, is_main_move);
        // Only the turn player's *main* move drives the stage machine.
        if is_main_move {
            self.w.st.step = stage::MOVE;
            // A stored step count (`next_steps`) is consumed here, before any
            // window, so the windows see the length the walk will take
            // (`spec/abi/G04-move-plan.md` §3, R11).
            if let Some(steps) = self.w.hidden[i].next_steps.take() {
                let steps = steps.max(0);
                self.w.plan_mut().next_steps = Some(steps);
                m.next_steps = Some(steps);
            }
        }
        let mut ran = true;
        if !self.out(i) {
            // The roll transaction opens before the plan window so its bodies
            // see the payload; a teleport rewrite below abandons it without
            // casting (G03 §3). A non-main move with a fixed step count walks
            // without a cast at all.
            let rolled_now = is_main_move || m.steps.is_none();
            let draft = if rolled_now {
                self.open_move_roll(i, move_roll_source(&m))
            } else {
                0
            };
            // The roll window is open from the plan window through the last
            // raise.
            let before = self.w.plan().clone();
            self.w.move_roll_windows.push(i);
            self.w.turn.plan_scope.window_before.push(before);
            if rolled_now {
                self.raise_roll("diceRollPlan", draft, Some(&m), -1)?;
                // Re-read the window's shape from the frame, but keep the
                // engine-stamped identity / progress (the frame carries plan
                // fields only; `player_id` is not a `PlanField`).
                m = self.w.plan().clone();
                self.stamp_move_identity(&mut m, i, roller, is_main_move);
                // The window may have raised the move's own source
                // (「使用火罐进行移动掷骰」 tags the plan here).
                self.set_roll_source(draft, move_roll_source(&m));
            }
            if self.out(i) || !self.playing() {
                if rolled_now {
                    self.abandon_roll(draft);
                }
                self.close_roll_window(&mut m);
                return self
                    .finish_move_body(i, m, is_main_move, false)
                    .map(|()| false);
            }
            if m.teleport_to.is_some() {
                // Planted before the roll (have already been rewritten): no die
                // is cast. The main move's `roll` condition still runs, on the
                // transaction's own view and with the "no face" sentinel.
                if is_main_move {
                    let view = self.roll_move_view(draft, i);
                    self.raise_roll("diceRollBefore", draft, Some(&view), -1)?;
                }
                if rolled_now {
                    self.abandon_roll(draft);
                }
                ran = self.run_planned_teleport(i, &mut m, is_main_move)?;
            } else if rolled_now {
                ran = self.run_planned_roll(i, draft, &mut m)?;
            }
            // A roll-window body may have written a destination instead of
            // a face (`plan::set_teleport_to`, saaya's empty face
            // 「移动到下一个可购买格子」). It is one-shot: take it from the
            // frame and jump there; the walk below is then skipped.
            //
            // A replacement (`void_walk`, §4.4) already ran its teleport
            // from inside the window and wrote the landing back onto this
            // frame for the record; taking it again would teleport (and
            // settle) twice. Leave it on the frame -- `close_roll_window`
            // folds it into the move's progress -- and jump only for a
            // bare window write.
            let after_teleport = if self.w.plan().void_walk {
                None
            } else {
                self.take_plan_teleport()
            };
            if let Some(to) = after_teleport {
                let (settles, reason) = (m.settles, m.reason.clone());
                ran = self.teleport_as(i, to, settles, reason, is_main_move)?;
            }
            if ran {
                self.close_roll_window(&mut m);
            }
            // A `void_walk` teleport has already run (and raised its tail)
            // from inside the window; a bare window write teleported above;
            // only an ordinary move walks. `ran` is not required here: a
            // replacement whose abnormal gate refused it never reaches the
            // engine (`HostRequest::Move` returns 0 after clearing
            // `teleport_to`/`void_walk` and resetting `kind = Walk`), and
            // the outer walk must resume
            // (`spec/abi/G04-move-plan.md` §4.4, R16).
            if !self.out(i)
                && self.playing()
                && move_plan::walk_runs(
                    m.teleport_to.is_some() || after_teleport.is_some(),
                    m.cancelled,
                    m.void_walk,
                )
            {
                self.walk(&mut m)?;
            }
        }
        self.finish_move_body(i, m, is_main_move, ran).map(|()| ran)
    }

    /// Adopt `plan` as the shared `turn.plan` for the move that is about to run.
    ///
    /// The plan a card shaped (or the one `main_move` stamped) is the
    /// authoritative shape: `plan::set_*` writes land on it. Seed `turn.plan`
    /// from it before anything the move raises -- the settle-side readers
    /// (`scale_settle_payment`) and the walk's per-step sync
    /// (`walk_inner_depth`) read `turn.plan`, and a stale `settles = true`
    /// there revives settlement on a 「不触发结算」 move.
    ///
    /// Note this carries **more** than `PlayCtx::adopt_turn_policy`, which only
    /// ferries the purchase/settle policy knobs (`pay_factor`, `rent_factor`,
    /// `can_build`, `no_buy`) across a host-routine pause; the rest of the
    /// shape (`settles`, `steps`, `kind`, `teleport_to`, …) is not carried and
    /// must be seeded here.
    pub(crate) fn seed_turn_plan(&mut self, plan: &Move) {
        // `settle_redirect` is engine-set on `card_settle_move` only; never let
        // a plan seed carry a sticky `true`.
        let mut p = plan.clone();
        p.settle_redirect = false;
        self.w.turn.plan = p;
    }

    /// The roll ladder inside an open window: cast, the three reshape / window
    /// raises, the `next_steps` override and the immobile clamp. Returns whether
    /// the move still runs.
    fn run_planned_roll(&mut self, i: usize, draft: i32, m: &mut Move) -> Flow<bool> {
        // The ladder (cast, the three reshape windows, the Result
        // counteraction window, the settle) is G03's. `rolled` is the
        // 「掷骰移动」 fact -- the length came from a die, not from a fixed
        // count -- so it survives an override (`next_steps`) but a fixed step
        // count is not a dice move.
        m.rolled = m.steps.is_none();
        // A stored fixed face replaces the throw. A `next_steps` count is the
        // length the walk takes, already folded in before the windows (R11), so
        // it outranks the pin and is the face every window sees.
        let pinned = self.w.turn.fixed_roll.take();
        let fixed = match m.next_steps {
            Some(steps) => Some(super::world::FixedFace {
                value: steps.max(0),
                source: pinned.map_or(RollSource::Unattributed, |f| f.source),
            }),
            None => pinned,
        };
        m.roll = self.run_move_roll(draft, &*m, fixed, false)?;
        // The unsigned-roll clamp applies to the final face.
        if !m.signed {
            if let Some(mr) = m.min_roll {
                m.roll = m.roll.max(mr);
            }
        }
        // 「如果为0则此次移动不[结算]」 written by a window body.
        if !self.w.plan().settles {
            m.settles = false;
        }
        if self.out(i) || !self.playing() {
            return Ok(false);
        }
        // G-STATUS-02: an immobile main move goes nowhere and settles nothing.
        let exempt = self.w.st.players[i].state_get(key::MOVE_EXEMPT) > 0;
        let immobile = !exempt && !self.w.gate_open(self.rules, super::Gate::Move, i);
        if immobile && m.is_main_move {
            m.roll = 0;
            m.steps = Some(0);
            m.settles = false;
            // 「（或无法移动的回合结束时）」 -- the withheld settle is the
            // default only; a card that answered `Skip` keeps it.
            use super::world::SettleAnswer;
            if self.w.blocked_settle_answer(i) == SettleAnswer::Unanswered {
                self.w.set_blocked_settle_answer(i, SettleAnswer::Settle);
            }
        }
        Ok(true)
    }

    /// Run a planned teleport (the entry teleport, or one a window wrote) and
    /// stamp its progress. Returns whether it ran.
    fn run_planned_teleport(&mut self, i: usize, m: &mut Move, is_main_move: bool) -> Flow<bool> {
        let Some(to) = m.teleport_to.map(|t| t.0 as usize) else {
            return Ok(false);
        };
        let settles = m.settles;
        let reason = m.reason.clone();
        let ran = self.teleport_as(i, to, settles, reason, is_main_move)?;
        m.kind = MoveKind::Teleport;
        m.rolled = false;
        m.total = 1;
        m.remaining = 0;
        m.path = vec![to as i32];
        m.cancelled = !ran;
        m.to = self.w.st.players[i].pos;
        Ok(ran)
    }

    /// Fold the window's writes into the move, apply the `void_walk` /
    /// replacement progress and pop the window.
    fn close_roll_window(&mut self, m: &mut Move) {
        let before = self
            .w
            .turn
            .plan_scope
            .window_before
            .pop()
            .unwrap_or_default();
        let after = self.w.plan().clone();
        if let Some(replace_done) = self.w.move_roll_windows.pop() {
            let _ = replace_done;
        }
        fold_plan_delta(m, &before, &after);
        if m.void_walk && m.teleport_to.is_some() {
            // The replacement ran from inside the window and wrote its progress
            // back (`replace_main_move`); copy it and skip the walk.
            m.kind = after.kind;
            m.path = after.path.clone();
            m.to = after.to;
            m.total = after.total;
            m.remaining = after.remaining;
            m.cancelled = after.cancelled;
            m.rolled = false;
        } else if m.void_walk {
            // A staged void with no target: no walk, no `lastWalk`.
            m.kind = MoveKind::Teleport;
            m.total = 0;
            m.remaining = 0;
            m.rolled = false;
        }
    }

    /// Shared completion: followers for a main move, the summary, `lastWalk`
    /// and the end-stage bookkeeping.
    fn finish_move_body(&mut self, i: usize, m: Move, is_main_move: bool, _ran: bool) -> Flow<()> {
        if is_main_move && !m.cancelled && self.playing() {
            self.replay_followers(i, &m)?;
        }
        self.w.st.plan = m.to_plan();
        if is_main_move {
            self.w.turn.main_steps = if m.kind == MoveKind::Teleport {
                0
            } else {
                m.total
            };
            if m.kind != MoveKind::Teleport {
                self.w
                    .set_slot(i as i32, "lastWalk", (m.total - m.remaining) + 1);
            }
            if let Some(p) = self.w.st.players.get_mut(i) {
                p.state_set(key::ABNORMAL_HIT, 0);
            }
            self.w.st.step = stage::END;
        }
        self.wait(1.2);
        Ok(())
    }

    /// A move's rolling window issued a replacement: run the teleport on its
    /// own side frame and write it back onto the pending walk. The outer move
    /// skips its walk and does the completion (`spec/abi/G04-move-plan.md`
    /// §4.4).
    fn replace_main_move(&mut self, i: usize, target: &Move) -> Flow<bool> {
        let Some(to) = target.teleport_to.map(|t| t.0 as usize) else {
            return Ok(false);
        };
        let owner = self.w.turn.plan_stack.last().map_or(i, |f| f.owner);
        self.w.begin_plan(owner, PlanKind::SideMove, target.clone());
        if let Some(p) = self.w.turn.plan_stack.last_mut() {
            p.plan.by_skill = target.by_skill;
        }
        let settles = target.settles;
        let reason = target.reason.clone();
        let ran = self.teleport_as(i, to, settles, reason, true)?;
        let landing = self.w.st.players[i].pos;
        self.w.end_plan(ran);
        if let Some(f) = self.w.turn.plan_stack.last_mut() {
            let p = &mut f.plan;
            p.kind = MoveKind::Teleport;
            p.teleport_to = Some(Tile(landing));
            p.path = vec![landing];
            p.to = landing;
            p.from = target.from;
            p.total = 1;
            p.remaining = 0;
            p.cancelled = !ran;
        }
        Ok(ran)
    }

    /// Replay a main move's result for its recorded followers, each under its
    /// own side frame so its counter writes die with it.
    fn replay_followers(&mut self, i: usize, m: &Move) -> Flow<()> {
        let followers = m.followers.clone();
        for f in followers {
            let fi = f as usize;
            if m.cancelled || self.out(fi) || !self.playing() || fi == i {
                continue;
            }
            let mut fm = m.clone();
            fm.player_id = fi;
            fm.roller = Some(Seat(fi as i32));
            fm.is_main_move = false;
            fm.followers.clear();
            fm.path.clear();
            fm.total = 0;
            fm.remaining = 0;
            fm.passed_players.clear();
            fm.stopped = false;
            fm.cancelled = false;
            fm.from = 0;
            fm.to = 0;
            self.w.begin_plan(fi, PlanKind::SideMove, fm.clone());
            if let Some(f) = self.w.turn.plan_stack.last_mut() {
                f.plan = fm.clone();
            }
            let r = (|| -> Flow<()> {
                let mut fm = self.w.plan().clone();
                if fm.teleport_to.is_some() {
                    let to = fm.teleport_to.unwrap().0 as usize;
                    let settles = fm.settles;
                    let reason = fm.reason.clone();
                    self.teleport(fi, to, settles, reason)?;
                } else {
                    self.walk(&mut fm)?;
                }
                Ok(())
            })();
            self.w.end_plan(true);
            r?;
        }
        Ok(())
    }

    /// Note the move's runtime progress on the top frame so a card body that
    /// reads between raises sees it (`before` is unused by callers today).
    fn adopt_plan(&mut self, m: &Move) {
        let p = self.w.plan_mut();
        p.settle_redirect = false;
        p.from = m.from;
        p.to = m.to;
        p.path = m.path.clone();
        p.total = m.total;
        p.remaining = m.remaining;
    }

    /// Press a skill button: run the rule's `On::Play` entry under its own id.
    ///
    /// A skill is a card rule -- it just lives on the player's field rather than
    /// in a hand -- so the card's play entry answers for it. Nothing moves: there
    /// is no hand card to spend and no destination to settles, unlike playing a
    /// card. The gate is not re-checked here; `cant_act` has already asked the
    /// rule's `cant_play` and refused the press if it named a reason, which is
    /// the same shape as every other turn action.
    pub fn use_skill(&mut self, player_id: usize, card: &str) -> Flow<()> {
        // `skillUsed` -- 「使用自己原有的技能（2）时」. Raised
        // before the body so a listener sees the use, not its aftermath -- and so
        // a counteraction answering it can negate the press before anything happens.
        let mut t = super::rules::Trigger::new("cardUsed", player_id);
        t.card = Some(card.to_string());
        // The skill id rides `cards` too (the `drew` pattern), so a listener can
        // read the id it is reacting to (`trigger::cards`) and name a mark after
        // it -- a persona skill that 「gains a mark for that persona」.
        t.cards = vec![card.to_string()];
        t.by_card = Some(player_id as i32);
        let t = self.raise(t)?;
        // A cancelled trigger (`is_cancelled`) -- the press is negated (花园多惠（2）
        // 「将其抵消」); the body does not run, exactly as `play_from_hand` treats
        // a cancelled
        // `card` link above. The card still flashes, marked negated. A press
        // that goes through announces itself at its body (`drive_inner_body`).
        if t.is_cancelled() {
            self.card_activated(
                crate::state::card_trigger::SKILL,
                player_id as i32,
                card,
                t.target,
                t.tile,
                true,
            );
            return Ok(());
        }
        // Record the press like a hand play: `turn.played` is the once-per-turn
        // de-dup the policy's skill pick reads (`ai_skill_choice`). Without it
        // a client-driven seat presses a free skill on every tick and never
        // gets to the rest of its turn. The turn-owner guard mirrors
        // `play_card` (an off-turn press ends nothing and must not count).
        if self.w.st.turn == player_id as i32 {
            self.w.turn.played.push(card.to_string());
        }
        let rules = self.rules;
        rules.play(self, player_id, card)?;
        Ok(())
    }

    /// Sum the move's dice tables into one face, without the
    /// `rollAfter` / `diceRollResult` raise points. A card that re-rolls an in-flight
    /// move (「放弃第一次的结果重骰一次」) is usually *inside* one of those hooks,
    /// so re-raising them would recurse; the plain table roll is what it wants.
    pub fn do_move_roll(&mut self, plan: &Move) -> i32 {
        self.roll_tables(plan)
    }

    /// A full [触发结算] of `tile`, wherever the player is standing.
    /// The player does not move; the tile's own effect resolves.
    ///
    /// STACK-01: enqueues a settle job and lets [`Self::drain_work`] pump it.
    /// Nested calls (a card settling from inside another settle) must go
    /// through the drive's `NeedHost` arm, which suspends instead of recursing;
    /// this entry is the outermost / non-drive path.
    pub fn settle_tile_at(
        &mut self,
        player_id: usize,
        tile: usize,
        is_main_move: bool,
    ) -> Flow<()> {
        if self.out(player_id) || !self.playing() {
            return Ok(());
        }
        // Rulebook-level depth guard (STACK-01): not the stack-safety
        // mechanism -- the work stack is -- but the rules themselves can loop
        // forever (two cards settling each other). Log and stop after
        // [`MAX_SETTLE_DEPTH`] nested settles.
        // Depth cap lives in `step_settle` (both this path and a work-stack
        // `SettleWork` go through it).
        let m = self.card_settle_move(player_id, tile, is_main_move);
        // A nested settle's counter writes must not shape the outer move.
        self.w.begin_plan(player_id, PlanKind::SideMove, m.clone());
        let mut frame = crate::engine::work::SettleFrame::new(player_id, tile, m);
        frame.scoped = true;
        // Always go through the pump: nested jobs suspend and resume on the
        // work stack (the pump catches `Halt::Suspended` and keeps going).
        // A call from inside an active pump must not reach here -- the drive's
        // `NeedHost` arm suspends instead -- but if it does, enqueue only.
        if self.is_draining_work() {
            self.push_work(crate::engine::work::SettleWork { frame });
            return Ok(());
        }
        self.push_work(crate::engine::work::SettleWork { frame });
        self.drain_work()?;
        self.wait(0.6);
        Ok(())
    }

    /// The [`Move`] a `settle_at` threads through the settle stages:
    /// the plan's payment shaping (`pay_factor`, tags) but not its walk. A
    /// settle at an arbitrary tile is not the turn's move, and inheriting
    /// `from`/`to`/`steps` left the piece one tile past a `teleport_to` +
    /// `settle_at` (游击演出 ends on chosen+1).
    ///
    /// Sets [`Move::settle_redirect`] so a hook on the nested settle can tell
    /// a card-initiated redirect from a real player settle. The plan is
    /// cloned, never written back: the plan's own flag remains `false` (there
    /// is no `plan::` setter for it) so a later move cannot inherit a sticky
    /// `true`.
    pub fn card_settle_move(&self, player_id: usize, tile: usize, is_main_move: bool) -> Move {
        let mut m = self.w.plan().clone();
        m.settle_redirect = true;
        m.player_id = player_id;
        m.is_main_move = is_main_move;
        m.kind = MoveKind::Teleport;
        m.teleport_to = Some(Tile(tile as i32));
        m.from = tile as i32;
        m.to = tile as i32;
        m.steps = Some(0);
        m.extra_steps = 0;
        m.start = Some(Tile(tile as i32));
        m
    }

    /// The purchase itself. No tile-kind guard beyond the
    /// engine's own: a card that says 「必须购买」 has already decided the tile is
    /// buyable. `kind` is a [`purchase::BuyKind`] as `i32` (the legacy
    /// `card_buy` passes `BuyKind::Card`).
    /// `true` when the deed actually changed hands. HOST-06: the guest's
    /// `ctx::card_buy`/`buy` `-> bool` is this outcome, not a constant.
    pub fn card_buy(&mut self, player_id: usize, tile: usize, kind: i32) -> Flow<bool> {
        if self.out(player_id) || !self.playing() {
            return Ok(false);
        }
        let kind = purchase::BuyKind::from_i32(kind).unwrap_or(purchase::BuyKind::Card);
        self.deed_buy(player_id, tile, kind, 1)
    }

    /// A card handed a deed over outside the buy routine (tomoe_savior's
    /// 「从该玩家处收购该地契」) and announces the acquisition.
    ///
    /// Three things land here, in this order:
    ///
    /// 1. **The transfer is committed first.** The card's own `set_owner` is a
    ///    write on its world *copy*; this runs against the live world the hook
    ///    chain sees, and 「购买」 hooks build on the tile (Afterglow
    ///    「自动免费在上面加盖一栋房子」 calls `card_build`, which refuses with
    ///    `err.build_not_own` while the old owner is still live). Committing
    ///    the hand-over against the live world first makes it visible to every
    ///    hook in the chain.
    /// 2. **The deed comes off mortgage**, the same post-state `buy()` leaves.
    ///    规则书 基础settle 6: 「如果格子地契已抵押则无效果」 -- no house can be
    ///    built on a mortgaged deed -- and 巴's 「收购」 pays the 「常规收购价」
    ///    (「地契购买价格」) with no 「仍为抵押状态」 clause, unlike 强制购买
    ///    which states one explicitly. RULING in TEST-FINDINGS §6.
    /// 3. **Both acquisition hooks run**, the same pair `buy()` raises after an
    ///    ordinary purchase: `bought` (「a player became the owner of the tile」)
    ///    then `buyAfter` (「after the deed has changed hands」). Every
    ///    「购买地契」 listener therefore hears a hand-over the rulebook counts
    ///    as a purchase, whichever of the two it sits on.
    ///
    /// `by` is the cause (stamped as `by_card`, 「购买」 hooks key 「来自你以外」
    /// on it).
    /// `true` when the hand-over landed (the player is in play and the tile
    /// exists). HOST-06: the guest's `ctx::raise_bought` `-> bool` is this.
    pub fn card_raise_bought(
        &mut self,
        player_id: usize,
        tile: usize,
        by_player: usize,
    ) -> Flow<bool> {
        if self.out(player_id) || !self.playing() {
            return Ok(false);
        }
        if tile >= self.w.st.owners.len() {
            return Ok(false);
        }
        self.w.st.owners[tile] = player_id as i32;
        if tile < self.w.st.mortgaged.len() {
            self.w.st.mortgaged[tile] = false;
        }
        let mut t = Trigger::new("bought", player_id);
        t.tile = tile as i32;
        t.by_card = Some(by_player as i32);
        self.raise(t)?;
        let mut t = Trigger::new("buyAfter", player_id);
        t.tile = tile as i32;
        t.by_card = Some(by_player as i32);
        self.raise(t)?;
        Ok(true)
    }

    /// `ctx::linger` (`docs/PURCHASE.md` P5) -- bind the running card's own def
    /// as a turn-scoped instance in [`super::world::TurnCtx`]'s `lingering`.
    /// `card` is the running
    /// card's id (the `CardDef` the instance binds) and `props` are the
    /// instance's props (e.g. `prop::NO_BUILD`), which is the hand-card home
    /// for a per-turn veto.
    pub fn card_linger(
        &mut self,
        player_id: usize,
        expires: i32,
        card: &str,
        props: std::collections::BTreeMap<String, i32>,
    ) {
        let round = self.w.st.round;
        let entry = super::world::Lingering {
            card: card.to_string(),
            owner: player_id as i32,
            expires: if expires > 0 { round + expires } else { round },
            props,
        };
        if !self.w.turn.lingering.iter().any(|l| l == &entry) {
            self.w.turn.lingering.push(entry);
        }
    }

    /// 「收购」 (`docs/PURCHASE.md` P3) -- take `tile` from `from` at `price`:
    /// pipeline pay, then assign → `bought` → `buyAfter`. Replaces the manual
    /// `set_owner` + `raise_bought` pair (tomoe_savior's port target).
    /// TODO(规则书) ruling 8: 巴's 「常规收购价一半」 base is undecided; the
    /// caller passes the price.
    /// `true` when the deed actually changed hands. HOST-06.
    pub fn card_acquire(
        &mut self,
        player_id: usize,
        from: usize,
        tile: usize,
        price: i32,
    ) -> Flow<bool> {
        if self.out(player_id) || !self.playing() {
            return Ok(false);
        }
        if self.w.st.owners.get(tile).copied().unwrap_or(-1) != from as i32 {
            return Ok(false);
        }
        // `BuyGate` -- the same gate every other buy kind runs.
        let gate = raise!(
            self,
            "buyGate",
            player_id,
            tile = tile as i32,
            @b TriggerBuy::new(purchase::BuyKind::Acquire, from as i32, price)
        )?;
        if gate.is_cancelled() {
            return Ok(false);
        }
        let price = price.max(0);
        if price > 0 {
            let mut p = Pay::new(price, PayKind::Buy);
            p.from = Some(player_id);
            p.to = Some(from);
            p.typ = Some(PayEvent::Lose);
            p.tile = Some(tile);
            p.source = "src.buy";
            p.text = Some(
                Msg::new("log.paid_for")
                    .player_id("who", player_id)
                    .tile("tile", tile),
            );
            let paid = self.money(p)?;
            if !paid.paid {
                return Ok(false);
            }
        }
        // The deal: `BuyAssign` rewrites owner / houses / mortgage before the
        // one ownership write (`docs/PURCHASE.md`).
        let mut deal = purchase::default_deal(
            self.data,
            &self.w.st,
            tile,
            player_id,
            purchase::BuyKind::Acquire,
            price,
            true,
        );
        let at = raise!(
            self,
            "buyAssign",
            player_id,
            tile = tile as i32,
            @b TriggerBuy::new(purchase::BuyKind::Acquire, from as i32, price).deal(
                Payee::from_i32(deal.owner_after),
                deal.houses_after,
                deal.mortgaged_after,
            )
        )?;
        if let Some(b) = at.buy {
            if let Some(o) = b.deal_owner {
                deal.owner_after = o.as_i32();
            }
            if let Some(h) = b.deal_houses {
                deal.houses_after = h;
            }
            deal.mortgaged_after = b.deal_mortgaged;
        }
        purchase::assign_deed(
            &mut self.w,
            tile,
            deal.owner_after,
            deal.houses_after,
            deal.mortgaged_after,
        );
        let mut t = Trigger::new("bought", player_id);
        t.tile = tile as i32;
        t.buy = Some(TriggerBuy::new(
            purchase::BuyKind::Acquire,
            from as i32,
            price,
        ));
        self.raise(t)?;
        let mut t = Trigger::new("buyAfter", player_id);
        t.tile = tile as i32;
        t.buy = Some(TriggerBuy::new(
            purchase::BuyKind::Acquire,
            from as i32,
            price,
        ));
        self.raise(t)?;
        Ok(true)
    }

    /// Pay the tile's build cost and raise one house. Refuses
    /// (with the engine's own reason) when the tile cannot take a house.
    /// `true` when a house was added. HOST-06.
    pub fn card_build(&mut self, player_id: usize, tile: usize) -> Flow<bool> {
        if self.out(player_id) || !self.playing() {
            return Ok(false);
        }
        if let Some(reason) = self.cant_build_on(player_id, tile) {
            self.w.log("text", player_id as i32, reason);
            return Ok(false);
        }
        self.deed_build(player_id, tile)
    }

    /// Prompt to build on one of `tiles`, then build there.
    /// `reason` names the effect (shown on the prompt). Skips silently when nothing
    /// in the list can take a house.
    /// Prompt to build on one of `tiles`, then build there.
    /// `reason` names the effect (shown on the prompt). Skips silently when nothing
    /// in the list can take a house. `true` when a house was added. HOST-06.
    pub fn card_offer_build(
        &mut self,
        player_id: usize,
        tiles: &[usize],
        reason: &str,
    ) -> Flow<bool> {
        if self.out(player_id) || !self.playing() {
            return Ok(false);
        }
        let options: Vec<usize> = tiles
            .iter()
            .copied()
            .filter(|&t| {
                self.cant_build_on(player_id, t).is_none()
                    && self.w.st.players[player_id].money >= self.build_cost(t)
            })
            .collect();
        if options.is_empty() {
            return Ok(false);
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
                .text("reason", reason)
                .i("n", options.len() as i64),
            &options,
            labels,
        );
        let pick = self.ask(ask)?.of(player_id);
        let Some(&t) = usize::try_from(pick).ok().and_then(|p| options.get(p)) else {
            return Ok(false);
        };
        self.deed_build(player_id, t)
    }

    /// Mortgage one of the player's deeds, forced by a
    /// card effect. This is not a user asking while a card is busy (`asking`),
    /// so the "busy" refusal does not apply; the ownership/validity gates still
    /// do. `true` when the deed went onto mortgage. HOST-06.
    pub fn card_mortgage(&mut self, player_id: usize, tile: usize) -> Flow<bool> {
        if self.out(player_id) || !self.playing() {
            return Ok(false);
        }
        if let Some(reason) = self.guard_mortgage(player_id, tile as i32, false) {
            self.w.log("text", player_id as i32, reason);
            return Ok(false);
        }
        self.deed_mortgage(player_id, tile, 0)
    }

    /// The turn's main roll-and-move. Opens a Main frame on the staging plan
    /// and runs the shared body; a spent slot leaves staging untouched.
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
        // Taking the main move lifts the 「无法移动」 premise: a withheld
        // turn-end settle (stamped at a blocked turn start, or by a card) is no
        // longer owed -- the move settles where it lands instead. A move that
        // is *still* immobile re-stamps it at the clamp in `run_move_body`.
        self.w
            .set_blocked_settle_answer(i, super::world::SettleAnswer::Unanswered);
        // A claim written outside a frame (a turn-start hook, a pressed skill)
        // landed on staging. It is one-shot and belongs to exactly the move
        // that consumes it, so clear it from staging here rather than let it
        // stand the offers of every later move down too.
        let seed = {
            let mut seed = self.w.turn.plan.clone();
            self.w.turn.plan.claimed = false;
            seed
        };
        self.w.begin_plan(i, PlanKind::MainMove, seed);
        let m = self.w.plan().clone();
        self.adopt_plan(&m);
        let r = self.run_move_body(i, Some(roller), true);
        self.w.end_plan(r.as_ref().is_ok_and(|&ran| ran));
        r.map(|_| ())
    }

    /// Sum the move's `base` + `dice` tables into one face.
    ///
    /// Each term is `count`d`sides`, or a flat `count` when `sides <= 0` (see
    /// [`crate::engine::move_ctx::Roll`]). This is what makes a Play body's
    /// 「掷骰变为3d20」 (`set_base_dice(3, 20)`) actually roll three dice.
    pub(crate) fn roll_tables(&mut self, m: &Move) -> i32 {
        let extreme = self.w.turn.extreme;
        let mut total = 0;
        for t in m.base.iter().chain(m.dice.iter()) {
            if t.dice.sides <= 0 {
                total += t.dice.count;
            } else if extreme != Extreme::Plain {
                // 规则书: 「使任意结果只有数字区间的效果以理论最大值或最小值结算」
                // -- a move roll's `NdM` is such an interval: settle every die at
                // the armed extreme instead of casting (as `PlayCtx::roll` does).
                let face = if extreme == Extreme::Max {
                    t.dice.sides.max(1)
                } else {
                    1
                };
                total += face * t.dice.count.max(0);
            } else {
                for _ in 0..t.dice.count.max(0) {
                    total += self.w.rng.d(t.dice.sides.max(1));
                }
            }
        }
        total
    }

    /// Step tile by tile; passing the start square pays the reward.
    ///
    /// Wraps [`Self::walk_inner`] with the lazy-flush lifecycle: any exit
    /// (interrupt, out, prompt halt) publishes whatever segment is still
    /// pending so the client token is never left short of the engine's, and
    /// the innermost [`WalkSeg`] is popped (a nested walk keeps its own).
    fn walk(&mut self, m: &mut Move) -> Flow<()> {
        let depth = self.w.walk_depth();
        let r = self.walk_inner(m);
        self.w.flush_walk();
        while self.w.walk_depth() > depth {
            self.w.pop_walk_seg();
        }
        r
    }

    fn walk_inner(&mut self, m: &mut Move) -> Flow<()> {
        self.move_depth += 1;
        let r = self.walk_inner_depth(m);
        self.move_depth -= 1;
        r
    }

    fn walk_inner_depth(&mut self, m: &mut Move) -> Flow<()> {
        let i = m.player_id;
        let n = self.data.tiles.len() as i32;
        // 「此次移动以X为起点（不触发起点地块效果）」: the walk begins at
        // `start` rather than where the player stands, and the piece is moved
        // there with no landing effect.
        // `m.from` is the move start either way -- the [pass] the start square reward's
        // 「[move start]不为the start square」 clause reads it.
        if let Some(start) = m.start {
            self.w.st.players[i].pos = start.0;
            m.from = start.0;
        } else {
            m.from = self.w.st.players[i].pos;
        }
        // Skill-source tag from the shared plan (stamped by the host when the
        // calling rule's id starts with `skill:`).
        if self.w.plan().by_skill {
            m.by_skill = true;
        }
        self.move_start = m.from;
        // `moveBefore` (`SETTLE-STAGES.md` §7 「移动前」, ABI v43) -- the move's
        // plan is fixed and nothing has been walked yet. Every move raises it:
        // walk or teleport, main or card-driven, settling or not. A counteraction
        // that cancels the move abandons it here (no tail at all); one that
        // alters it writes the plan (`plan::set_stop_at` / `set_steps` / …) and
        // the walk pulls the write in below.
        //
        // The plan (`turn.plan`) is the authoritative move *shape* -- cards
        // write it both before the walk (a `diceRollResult` counteraction's
        // `set_pay_factor`) and inside this window. Fold the window's writes
        // into the running move; do **not** copy `m` over the plan, which would
        // wipe the earlier ones (`scale_settle_payment` reads the plan's
        // factors), and do **not** blanket-copy the plan back (it is never
        // reset between moves, so stale fields would leak in).
        let plan_before = self.w.plan().clone();
        // Expose the resolved distance before the first step: `value` is the
        // base distance and Total/Remaining include existing extra steps.
        // A post-roll skill can preview its landing and extend this move
        // without rolling again or dropping an earlier movement modifier.
        let base_steps = m.steps.unwrap_or(m.roll);
        m.total = base_steps.max(0) + m.extra_steps.max(0);
        m.remaining = m.total;
        let t = raise!(self, "moveBefore", i, @m m, tile = m.from, value = base_steps)?;
        if t.is_cancelled() || self.out(i) || !self.playing() {
            // The move was cancelled before its first step: it never happened,
            // so there is no move tail (`moveAfter` / `moveResolved`) either.
            // Mark it so a shared main move does not replay for its followers.
            m.cancelled = t.is_cancelled();
            return Ok(());
        }
        fold_plan_delta(m, &plan_before, &self.w.plan().clone());
        // Base distance: a stored step count overrides the roll
        // (`steps = if m.steps >= 0 { m.steps } else { m.roll }`).
        let steps = m.steps.unwrap_or(m.roll);
        // Snapshot the log-line fields: the loop below writes `m.total` /
        // `m.remaining` / `m.path`, so the closure must not borrow `m`.
        let (is_main_move, roller, reason, dir, settles) = (
            m.is_main_move,
            m.roller,
            m.reason.clone(),
            m.dir(),
            m.settles,
        );
        let head = move |still: bool| {
            let base = if is_main_move {
                Msg::new("log.roll").opt(
                    "by",
                    (roller != Some(Seat(i as i32))).then(|| {
                        Msg::new("log.part.rolled_by")
                            .player_id("who", roller.map(|r| r.0).unwrap_or(i as i32))
                    }),
                )
            } else {
                Msg::new(if dir < 0 {
                    "log.move_back"
                } else {
                    "log.move_forward"
                })
                .opt(
                    "reason",
                    reason
                        .clone()
                        .map(|w| Msg::new("log.part.reason").msg("reason", w)),
                )
            };
            base.player_id("who", i)
                .i("n", steps)
                .opt("nores", (!settles).then(|| Msg::new("log.part.no_resolve")))
                .opt("still", still.then(|| Msg::new("log.part.no_move")))
        };
        let kind = if m.is_main_move {
            "diceRollBefore"
        } else {
            "move"
        };
        let pos = self.w.st.players[i].pos;
        if steps <= 0 {
            let e = self.w.log(kind, i as i32, head(true));
            e.from = pos;
            e.to = pos;
            e.dice = if m.is_main_move { steps.max(0) } else { 0 };
            self.wait(if m.is_main_move { 1.2 } else { 0.3 });
            // 行动阶段 13 (`E14`) 「原地teleport/移动(移动0格)时触发overlap」: a
            // 0-step move still raises overlap at the tile it stands on.
            // TODO(规则书): `E14` names only overlap for the 0-step case; whether
            // [pass] (`passTile`) also raises here is not stated -- `B41`+`E13`
            // give the general rule for a *teleport* and read as both-raise, but
            // `E14` is the only cell that names the 0-move and it names only
            // overlap. Implemented as `E14` writes it: overlap only.
            self.overlap_at(m, pos as usize)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            self.move_after(m)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            self.after_walk(m)?;
            return self.move_resolved(m);
        }
        // Lazy-flush segment: steps accumulate here and are published as one
        // `roll` / `move` event when the walk reaches the start square / its last step,
        // or when anything else is about to be logged (see `World::flush_walk`).
        // Armed with the head text now; `pending_first` marks that it has not
        // been published. At every step boundary the segment covers exactly
        // `from..=to` (`to` lags `next` until the step completes, so a
        // `passBefore` hook flushes the approach up to `cur`).
        self.w.push_walk_seg(WalkSeg {
            player: i as i32,
            from: pos,
            to: pos,
            value: 0,
            ahead: steps.max(0) + m.extra_steps.max(0),
            pending_first: true,
            is_main_move: m.is_main_move,
            roll: m.roll,
            head: head(false),
        });
        // The walk is bounded by `steps + extra_steps`, re-read each step: a
        // card that adds steps mid-walk lengthens it. (`more_steps` is stored
        // on the move but has no second walk phase yet.)
        let mut k = 0usize;
        while k < (steps.max(0) as usize) + m.extra_steps.max(0) as usize {
            // The frame is the authority for a mid-walk extension: a card's
            // `add_extra_steps` must lengthen a walk that is already running.
            m.extra_steps = self.w.plan().extra_steps;
            let cur = self.w.st.players[i].pos;
            let next = move_plan::next_parity_tile(cur, m.dir(), m.parity, n);
            // [`MoveCtx::total`] / `remaining` / `path` -- the walk's length, how
            // much of it is left, and the tiles it visits (`to_plan`'s `reach`).
            // `total` is the planned length (it tracks `extra_steps` mid-walk);
            // `remaining` counts the steps after the tile being entered, so a
            // 「pass且未触发结算」 filter (`remaining > 0`) sees the landing as 0.
            let planned = (steps.max(0) as usize) + m.extra_steps.max(0) as usize;
            let remaining = planned - k - 1;
            m.total = planned as i32;
            m.remaining = remaining as i32;
            let at = next as usize;
            let last = remaining == 0;
            // `passBefore` -- the glossary's 「[pass]X」 is any tile on the move
            // path, so this runs for every step (not just the end).
            // A hook that logs here flushes the segment covering steps up to
            // `cur` (the step onto `next` has not happened yet).
            raise!(self, "passBefore", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            self.land_plan_teleport(i);
            if self.w.st.players[i].pos != cur {
                break; // moved by a passBefore counteraction
            }
            self.w.st.players[i].pos = next;
            m.path.push(next);
            // The step is taken: extend the pending segment to `next`.
            if let Some(seg) = self.w.walk_seg_mut() {
                seg.to = next;
                seg.value += m.dir();
                seg.ahead = remaining as i32;
            }
            if last || self.w.tile_prop(at as i32, crate::state::prop::START) != 0 {
                // Publish the approach before `passTile` on the start square
                // (the pass-reward prompt) or the walk's last step. The client
                // must walk to the tile before showing that choice, then
                // continue from it after the reward resolves, without snapping
                // back to `from`. Other mid-walk events use the lazy flush.
                self.w.flush_walk();
            }
            // `passTile` (hook) -- the player has stepped onto this tile. A hook
            // that logs here flushes the segment covering steps up to `next`.
            raise!(self, "passTile", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            self.land_plan_teleport(i);
            // A pass-reward (if any) is the tile rule's Pass entry on the
            // `passTile` hook above. The engine keeps no built-in fallback.
            // `pass` -- every traversed tile, like `passBefore` / `passTile`
            // (glossary 「[pass]」). `passTile` is the hook kind for the same
            // walk; a card uses one or the other, never both.
            raise!(self, "pass", i, @m m, tile = next)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            self.land_plan_teleport(i);
            if self.w.st.players[i].pos != next {
                break; // moved by an effect
            }
            // 「强制停下」. A hook writes the *plan* (`plan::set_stop_at`),
            // and the walk runs on its own `Move`; pull that write in and stop here.
            if self.w.plan().stop_at.is_some() {
                m.stop_at = self.w.plan().stop_at;
            }
            if self.w.plan().settles != m.settles {
                m.settles = self.w.plan().settles;
            }
            if m.stop_at == Some(Tile(next)) {
                m.stopped = true;
                break;
            }
            k += 1;
        }
        // A `passTile` counteraction may have answered 「是否触发结算」 for this
        // walk (R:选择自己的舞台).
        self.take_resolve_override(i, m);
        // 行动阶段 13 (`E13`/`E14`) 「move end触发overlap」: once the walk is over,
        // overlap runs at the tile the mover ended on -- including a walk that
        // stopped early (「强制停下」). Not mid-walk: `B40` is 「移动后」.
        if !self.out(i) && self.playing() {
            let at = self.w.st.players[i].pos as usize;
            self.overlap_at(m, at)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
        }
        self.move_after(m)?;
        if self.out(i) || !self.playing() {
            return Ok(());
        }
        self.after_walk(m)?;
        self.move_resolved(m)
    }

    /// Teleport a player to a destination tile.
    ///
    /// A teleport is 「[路径]只包括[move end]的移动动作」 (专有名词 9), so its
    /// pass handling runs **once at the destination** -- not per step.
    /// 回合階段&註釋 `B41` 「传送：空降至目标地格并在该格依次触发[pass],overlap,
    /// 和settle」: a teleport always raises [pass] then overlap then settle at the
    /// target, **including onto the tile it started from** (「原地teleport」,
    /// `E14`) -- there is no 「原地，不算 [pass]」 carve-out.
    pub(crate) fn teleport(
        &mut self,
        i: usize,
        to: usize,
        settles: bool,
        reason: Option<Msg>,
    ) -> Flow<bool> {
        self.teleport_as(i, to, settles, reason, false)
    }

    /// [`Self::teleport`] with the move's `main` flag named (「视为当回合的主要
    /// 移动」 -- the return teleport of a status's own clause). Answers whether
    /// the teleport actually ran (a `moveBefore` counteraction may cancel it).
    pub fn teleport_as(
        &mut self,
        i: usize,
        to: usize,
        settles: bool,
        reason: Option<Msg>,
        is_main_move: bool,
    ) -> Flow<bool> {
        // With no frame live this teleport is its own move and scopes itself
        // (`settle_tile_at`'s sibling); a live frame is owned by the nearest
        // caller (a main/side move, a replacement, a follower replay).
        let scoped = self.w.turn.plan_stack.is_empty();
        if scoped {
            let kind = if is_main_move {
                PlanKind::MainMove
            } else {
                PlanKind::SideMove
            };
            let target = self.w.plan().clone();
            self.w.begin_plan(i, kind, target);
        }
        self.move_depth += 1;
        let r = self.teleport_as_depth(i, to, settles, reason, is_main_move);
        self.move_depth -= 1;
        if scoped {
            self.w.end_plan(r.as_ref().is_ok_and(|&ran| ran));
        }
        r
    }

    /// 规则书: 「选择自己的本次移动…是否触发结算」 (`R:选择自己的舞台`) -- the
    /// counteraction's answer rides a one-shot player slot: `1` arms the
    /// settle, `2` pins it off. It overrides the caller's 「不[结算]」 and the
    /// plan alike, and is consumed here so it cannot leak into the next move.
    fn take_resolve_override(&mut self, i: usize, m: &mut Move) {
        match self.w.st.players[i].state_get("resolveOverride") {
            1 => m.settles = true,
            2 => m.settles = false,
            _ => return,
        }
        self.w.st.players[i].state_set("resolveOverride", 0);
    }

    /// 「选择自己的本次移动…是否触发结算」 -- write the one-shot
    /// `resolveOverride` slot [`Self::take_resolve_override`] consumes. An
    /// `Unanswered` write leaves the slot alone.
    pub fn set_move_settle_answer(&mut self, i: usize, a: super::world::SettleAnswer) {
        if let Some(slot) = a.slot() {
            self.w.st.players[i].state_set("resolveOverride", slot);
        }
    }

    /// The armed move-settle answer (`Unanswered` when the slot is empty).
    pub fn move_settle_answer(&self, i: usize) -> super::world::SettleAnswer {
        super::world::SettleAnswer::from_i32(self.w.st.players[i].state_get("resolveOverride"))
    }

    /// 「（或无法移动的回合结束时）」 -- the turn-end half for `i`. `i` is the
    /// blocked seat, not necessarily the turn player. Read and cleared by
    /// [`Self::end_turn`].
    pub fn set_turn_end_settle_answer(&mut self, i: usize, a: super::world::SettleAnswer) {
        self.w.set_blocked_settle_answer(i, a);
    }

    /// `i`'s turn-end settle answer, `Unanswered` by default.
    pub fn turn_end_settle_answer(&self, i: usize) -> super::world::SettleAnswer {
        self.w.blocked_settle_answer(i)
    }

    /// Consume a `teleport_to` a hook wrote on the *plan*.
    /// (`plan::set_teleport_to`) instead of calling `ctx::teleport_to`.
    ///
    /// The two write shapes are the same relocation -- `ctx::teleport_to` is a
    /// bare position write at `move_depth > 0` -- so a plan write inside a move
    /// must move the piece too, or the destination the hook logged (and the
    /// `log.outcome.plan_teleport` line) is a promise the board never keeps and
    /// the settle runs at the original tile (sakiko_crychic's settleBefore nudge,
    /// saaya_sky's dice floor).
    ///
    /// One-shot: the write is cleared whether or not the tile is usable.
    /// Returns the tile when it is on the board.
    fn take_plan_teleport(&mut self) -> Option<usize> {
        let t = self.w.plan_mut().teleport_to.take()?.0;
        (t >= 0 && (t as usize) < self.data.tiles.len()).then_some(t as usize)
    }

    /// Move the player to a plan-written destination, if a hook just wrote one.
    /// A bare write: the stage's own flow (`break` on a step, the `settleBefore`
    /// re-target loop) carries on from the new tile.
    fn land_plan_teleport(&mut self, i: usize) {
        if let Some(to) = self.take_plan_teleport() {
            self.w.st.players[i].pos = to as i32;
        }
    }

    fn teleport_as_depth(
        &mut self,
        i: usize,
        to: usize,
        settles: bool,
        reason: Option<Msg>,
        is_main_move: bool,
    ) -> Flow<bool> {
        if to >= self.data.tiles.len() || self.out(i) {
            return Ok(false);
        }
        let from = self.w.st.players[i].pos;
        let mut m = Move::new(i);
        m.is_main_move = is_main_move;
        m.settles = settles;
        m.kind = MoveKind::Teleport;
        m.teleport_to = Some(Tile(to as i32));
        m.from = from;
        m.path = vec![to as i32];
        m.total = 1;
        // Skill-source tag from the shared plan (stamped by the host when the
        // calling rule's id starts with `skill:`).
        m.by_skill = self.w.plan().by_skill;
        self.move_start = from;
        // `moveBefore` (`SETTLE-STAGES.md` §7 「移动前」, ABI v43) -- the
        // teleport's destination is fixed and nothing has moved yet. A
        // counteraction that cancels the move abandons it here; one that alters
        // it rewrites `teleport_to` on the plan and the teleport follows.
        let plan_before = self.w.plan().clone();
        let t = raise!(self, "moveBefore", i, @m m, tile = from)?;
        if t.is_cancelled() || self.out(i) || !self.playing() {
            m.cancelled = t.is_cancelled();
            return Ok(false);
        }
        fold_plan_delta(&mut m, &plan_before, &self.w.plan().clone());
        let to = if m.teleport_to.is_some() {
            m.teleport_to.unwrap().0 as usize
        } else {
            to
        };
        if to >= self.data.tiles.len() {
            return Ok(false);
        }
        let settles = m.settles;
        self.w.st.players[i].pos = to as i32;
        let text = Msg::new("log.teleport")
            .player_id("who", i)
            .tile("to", to)
            .opt(
                "reason",
                reason.map(|w| Msg::new("log.part.reason").msg("reason", w)),
            )
            .opt("nores", (!settles).then(|| Msg::new("log.part.no_resolve")));
        let e = self.w.log("teleport", i as i32, text);
        e.from = from;
        e.to = to as i32;
        self.wait(0.5);
        // R2 / M6a (`SETTLE-STAGES.md` §6): 专名词 9 gives a teleport a [路径]
        // of just the endpoint, and B41 raises [pass],overlap,settle at that
        // endpoint in that order. 其他规则注意事项 1.2 then removes only the
        // settle -- so a 「不[触发结算]」 teleport still raises `passTile`
        // ([pass]) and `passPlayer` (overlap) at its destination, never mid-route
        // squares. The old "a teleport with settles clear resolves nothing"
        // reading over-read 「是否settle」.
        raise!(self, "passTile", i, @m m, tile = to as i32)?;
        if self.out(i) || !self.playing() {
            return Ok(true);
        }
        // 规则书: 「强制停下并触发结算」 -- a `passTile` counteraction (live前的
        // 准备) may arm the settle this teleport skipped. The walk syncs that
        // plan write at each step; the teleport's `settles` capture is from
        // before the window, so re-read it here.
        if self.w.plan().settles != m.settles {
            m.settles = self.w.plan().settles;
        }
        self.take_resolve_override(i, &mut m);
        let settles = m.settles;
        self.overlap_at(&mut m, to)?;
        if self.out(i) || !self.playing() {
            return Ok(true);
        }
        // A pass-reward (if any) is the tile rule's Pass entry on the
        // `passTile` raise above. The engine keeps no built-in fallback.
        m.to = self.w.st.players[i].pos as i32;
        // `moveAfter` (行动阶段 13 「移动后」/「主要移动结束时」) -- after
        // overlap, before the settle stages, for every completed move including
        // a 「不[触发结算]」 one (`SETTLE-STAGES.md` §7, ruling R1).
        self.move_after(&mut m)?;
        if self.out(i) || !self.playing() {
            return Ok(true);
        }
        // The settle stages (14–15) run only when the move settles (其他规则
        // 注意事项 1.2: 「是否settle」 gates only the settle).
        if settles {
            self.after_walk(&mut m)?;
        }
        // `Teleported` after the settlement (or at once when the move
        // does not settle). Retained for wire/replay compatibility only --
        // the listener-facing move-end point is `moveAfter` (supersedes this
        // for card listeners; `Teleported` is hook-only).
        if !self.out(i) && self.playing() {
            raise!(self, "teleported", i, @m m, tile = m.to)?;
        }
        // `moveResolved` (Q6) -- the move tail's terminal, strictly last
        // (`SETTLE-STAGES.md` §7: 「final, after any settle」). Runs for a
        // 「不[触发结算]」 move too: the movement completed, it just settled to
        // nothing. Not raised when the mover went out mid-walk (the move is
        // abandoned, not resolved).
        if !self.out(i) && self.playing() {
            raise!(self, "moveResolved", i, @m m, tile = self.w.st.players[i].pos)?;
        }
        Ok(true)
    }

    /// overlap (`passPlayer`) -- 「move end触发overlap」 / 「原地teleport/移动(移动0格)
    /// 时触发overlap」 (行动阶段 13, `E14`): one raise per other player sharing
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

    /// `moveAfter` (行动阶段 13 「移动后」/「主要移动结束时」, ABI v43) -- the
    /// move-end point, after `passPlayer` (overlap) and before `settleBefore`.
    /// Runs for **every** completed move: walk or teleport, main or
    /// card-driven, settling or not (ruling R1 / 其他规则注意事项 1.2 -- 「是否
    /// settle」 gates only the settle stages 14–15). 「移动后」 / 「主要移动结束
    /// 时」 / 「[move end]」 clauses land here. A relocation during this window
    /// redirects the settle (Q7): `after_walk` reads the position fresh.
    fn move_after(&mut self, m: &mut Move) -> Flow<()> {
        let i = m.player_id;
        if self.out(i) || !self.playing() {
            self.w.turn.move_open = false;
            return Ok(());
        }
        m.to = self.w.st.players[i].pos as i32;
        raise!(self, "moveAfter", i, @m m, tile = m.to)?;
        Ok(())
    }

    /// `moveResolved` (Q6) -- the move tail's terminal, strictly after any
    /// settle (`SETTLE-STAGES.md` §7). Runs for a 「不[触发结算]」 move too:
    /// the movement completed, it just settled to nothing. Not raised when the
    /// mover went out mid-walk (the move is abandoned, not resolved).
    fn move_resolved(&mut self, m: &Move) -> Flow<()> {
        let i = m.player_id;
        if self.out(i) || !self.playing() {
            self.w.turn.move_open = false;
            return Ok(());
        }
        raise!(self, "moveResolved", i, @m m, tile = self.w.st.players[i].pos)?;
        // A `thisMove` factor dies with its move (the settle above already saw
        // it); a later landing pays full again.
        self.w.plan_mut().pay_factor = 1.0;
        self.w.turn.move_open = false;
        Ok(())
    }

    /// The settle stages (行动阶段 14–15) for a move that settles.
    /// The move tail (`moveAfter` before this, `moveResolved` after) is raised
    /// by the walk / teleport themselves, so a 「不[触发结算]」 move runs the
    /// tail and skips only this.
    fn after_walk(&mut self, m: &mut Move) -> Flow<()> {
        let i = m.player_id;
        if self.out(i) || !self.playing() || !m.settles {
            return Ok(());
        }
        // Q7 (`SETTLE-STAGES.md` §7, user ruling 2026-10-07): a relocation
        // during `moveAfter` or `settleBefore` means the settle did not happen
        // at the old tile and proceeds at the new tile, and the new tile gets
        // its own `settleBefore` window. Re-raise `settleBefore` only if the
        // position actually changed during the previous window; at most
        // [`MAX_SETTLE_REDIRECTS`] redirects per settlement (log and settle
        // where standing after that). A relocation during `settle` /
        // `settleBody` / `settleAfter` does **not** re-target -- the landed
        // tile finishes (`settle_at` threads one `at`).
        let mut redirects = 0;
        loop {
            let at = self.w.st.players[i].pos as usize;
            raise!(self, "settleBefore", i, @m m, tile = at as i32)?;
            if self.out(i) || !self.playing() {
                return Ok(());
            }
            // A hook that changes the destination writes the plan rather than
            // the position; land it so the `now != at` re-target below runs the
            // settle (and its lines) at the new tile.
            self.land_plan_teleport(i);
            let now = self.w.st.players[i].pos as usize;
            if now == at {
                break;
            }
            redirects += 1;
            if redirects > MAX_SETTLE_REDIRECTS {
                // Log and settle where standing: the loop cap has spoken.
                self.w.log(
                    "text",
                    i as i32,
                    Msg::new("log.settle_redirect_cap")
                        .player_id("who", i)
                        .i("n", MAX_SETTLE_REDIRECTS as i64),
                );
                break;
            }
            // else: the new tile gets its own `settleBefore` window
        }
        if !self.out(i) && self.playing() {
            self.settle(i, m)?;
        }
        Ok(())
    }

    /// [触发结算] on the tile the player is standing on.
    ///
    /// Q7 (settled, `SETTLE-STAGES.md` §7, user ruling 2026-10-07): a
    /// relocation during `moveAfter` / `settleBefore` redirects the settle and
    /// re-runs `settleBefore` at the new tile (loop-capped at
    /// [`MAX_SETTLE_REDIRECTS`]) -- that loop lives in [`Self::after_walk`].
    /// A relocation inside `settle` / `settleBody` / `settleAfter` does **not**
    /// re-target: `pos` is read **once**, here, and the landed tile finishes
    /// (行动阶段 15 「触发地块/落点效果」).
    fn settle(&mut self, i: usize, m: &Move) -> Flow<()> {
        let at = self.w.st.players[i].pos as usize;
        self.settle_at(i, at, m)
    }

    /// The [触发结算] of an arbitrary tile. A card that resolves a
    /// tile it is sitting on (rather than the player's square) wants this; the
    /// player does not move.
    ///
    /// STACK-01: implemented as a stage machine ([`Self::step_settle`]) so a
    /// nested settle suspends and resumes on the work stack instead of
    /// recursing. See `docs/ENGINE.md` "settle work stack".
    fn settle_at(&mut self, i: usize, at: usize, m: &Move) -> Flow<()> {
        let mut frame = crate::engine::work::SettleFrame::new(i, at, m.clone());
        self.step_settle(&mut frame)
    }

    /// Run `frame` to completion, or suspend and park a
    /// [`ContinueSettleWork`](crate::engine::work::ContinueSettleWork) on the
    /// work stack when a raise needs a nested settle first.
    ///
    /// Order is the old synchronous nest: the nested settle (and the drive
    /// that asked for it) finish before this settle's next stage, exactly
    /// where `settle_at` returned to the hook body.
    pub fn step_settle(&mut self, frame: &mut crate::engine::work::SettleFrame) -> Flow<()> {
        use crate::engine::work::ContinueSettleWork;
        // Rulebook runaway guard (not stack safety): two cards can settle each
        // other forever. Count only while a settle is actually in flight.
        // Rulebook runaway guard: cap settles per pump chain.
        if !self.note_settle_op() {
            self.w.log(
                "text",
                frame.player as i32,
                Msg::new("log.settle_depth_cap")
                    .player_id("who", frame.player as i32)
                    .i("n", MAX_SETTLE_DEPTH as i64),
            );
            if frame.scoped {
                self.w.end_plan(false);
            }
            return Ok(());
        }
        // One entry per frame, pushed on its **first** step only: a parked
        // frame keeps `stack_idx`, so a resume never pushes again. The
        // `nextSettle` factor is consumed here (and reset), so it lands on
        // exactly this settle; the `nextSettle` addend too.
        if frame.stack_idx == usize::MAX {
            let f = std::mem::take(&mut self.w.turn.next_settle_pay_factor) as f64 / 1000.0;
            self.w.turn.next_settle_pay_factor = 1000;
            let a = std::mem::take(&mut self.w.turn.next_settle_pay_add);
            let base = self.w.turn.settle_pay_factor;
            self.settle_stack.push(super::cx::SettleEntry {
                player: frame.player,
                tile: frame.at,
                base,
                base_rent: self.w.turn.settle_rent_factor,
                base_add: self.w.turn.settle_pay_add,
                base_rent_add: self.w.turn.settle_rent_add,
                outcome: super::cx::SettleOutcome::Settled,
            });
            frame.stack_idx = self.settle_stack.len() - 1;
            // The guest factor API reads the product and the innermost base
            // off the world (`settle_pay_factor` already includes every outer
            // settle; the new one composes on top of it).
            self.w.turn.settle_open = self.settle_stack.len() as u32;
            self.w.turn.settle_base = base;
            self.w.turn.settle_pay_factor = base * f;
            self.w.turn.settle_pay_add = self.w.turn.settle_pay_add.saturating_add(a);
        }
        let r = self.step_settle_run(frame);
        let suspended = r.as_ref().err().map(|e| e.is_suspended()).unwrap_or(false);
        if suspended {
            // Stage not advanced: the re-entered raise adopts
            // `counteract_resume`. Park this settle to resume after the
            // nested work (its stack entry stays).
            self.insert_work(crate::engine::work::ContinueSettleWork {
                frame: frame.clone(),
            });
        } else {
            // Completions nest LIFO, so the finished entry is the top: its
            // `base` is the product of every outer settle, which restores.
            let finished = self.settle_stack[frame.stack_idx];
            self.settle_stack.truncate(frame.stack_idx);
            self.w.turn.settle_open = self.settle_stack.len() as u32;
            self.w.turn.settle_pay_factor = finished.base;
            self.w.turn.settle_base = self.settle_stack.last().map_or(1.0, |s| s.base);
            self.w.turn.settle_rent_factor = finished.base_rent;
            self.w.turn.settle_rent_base = self.settle_stack.last().map_or(1.0, |s| s.base_rent);
            self.w.turn.settle_pay_add = finished.base_add;
            self.w.turn.settle_rent_add = finished.base_rent_add;
        }
        r
    }

    fn step_settle_run(&mut self, frame: &mut crate::engine::work::SettleFrame) -> Flow<()> {
        use crate::engine::work::ContinueSettleWork;
        use crate::engine::work::SettleStage;
        loop {
            let more = match frame.stage {
                SettleStage::RaiseSettle => {
                    if frame.owner < 0 {
                        frame.owner = self.w.st.owners.get(frame.at).copied().unwrap_or(-1);
                    }
                    let owner = frame.owner;
                    let (i, at) = (frame.player, frame.at);
                    let m = frame.m.clone();
                    let t = raise!(
                        self,
                        "settle",
                        i,
                        @m m,
                        tile = at as i32,
                        target = owner,
                        settle_tile = at as i32
                    )?;
                    if self.out(i) || t.is_cancelled() {
                        self.settle_stack[frame.stack_idx].outcome = if self.out(i) {
                            super::cx::SettleOutcome::PayerOut
                        } else {
                            super::cx::SettleOutcome::Cancelled
                        };
                        frame.stage = SettleStage::RaiseTileResolved;
                    } else {
                        frame.stage = SettleStage::RaiseSettlePay;
                    }
                    true
                }
                // The settle's 支付价格 shaping (MONEY-01): `settleAdd` sums,
                // `settleMul` multiplies, both raised **once** for the whole
                // settle and kept on the world for every leg to fold
                // (`TurnCtx::settle_pay_add` / `settle_pay_factor`). 「本次结算
                // 的支付减半」 is one contribution to one settle, not a rewrite
                // per payment, so two of them compose to a quarter instead of
                // the last one winning.
                SettleStage::RaiseSettlePay => {
                    let owner = frame.owner;
                    let (i, at) = (frame.player, frame.at);
                    let m = frame.m.clone();
                    let t = raise!(
                        self, "settleAdd", i, @m m, tile = at as i32, target = owner,
                        value = 0, @f add
                    )?;
                    self.w.turn.settle_pay_add = self.w.turn.settle_pay_add.saturating_add(t.value);
                    let t = raise!(
                        self,
                        "settleRentAdd",
                        i,
                        @m m,
                        tile = at as i32,
                        target = owner,
                        value = 0,
                        @f add
                    )?;
                    self.w.turn.settle_rent_add =
                        self.w.turn.settle_rent_add.saturating_add(t.value);
                    let t = raise!(
                        self, "settleMul", i, @m m, tile = at as i32, target = owner,
                        value = 1_000_000, @f mul
                    )?;
                    let f = t.fold_milli as f64 / 1_000_000.0;
                    self.w.turn.settle_pay_factor *= f;
                    let t = raise!(
                        self, "settleRentMul", i, @m m, tile = at as i32, target = owner,
                        value = 1_000_000, @f mul
                    )?;
                    self.w.turn.settle_rent_factor *= t.fold_milli as f64 / 1_000_000.0;
                    frame.stage = SettleStage::RaiseSettleBody;
                    true
                }
                SettleStage::AfterSettle => {
                    frame.stage = SettleStage::RaiseSettleBody;
                    true
                }
                SettleStage::RaiseSettleBody => {
                    let owner = frame.owner;
                    let (i, at) = (frame.player, frame.at);
                    let m = frame.m.clone();
                    let si = raise!(
                        self,
                        "settleBody",
                        i,
                        @m m,
                        tile = at as i32,
                        target = owner,
                        settle_tile = at as i32
                    )?;
                    if si.is_cancelled() {
                        self.settle_stack[frame.stack_idx].outcome =
                            super::cx::SettleOutcome::Cancelled;
                        frame.stage = SettleStage::RaiseSettleAfter;
                    } else {
                        frame.stage = SettleStage::SettleTile;
                    }
                    true
                }
                SettleStage::SettleTile => {
                    let rules = self.rules;
                    rules.settle_tile(self, frame.player, frame.at, frame.m.is_main_move)?;
                    frame.stage = SettleStage::RaiseSettleAfter;
                    true
                }
                SettleStage::RaiseSettleAfter => {
                    if self.out(frame.player) || !self.playing() {
                        self.settle_stack[frame.stack_idx].outcome = if self.out(frame.player) {
                            super::cx::SettleOutcome::PayerOut
                        } else {
                            super::cx::SettleOutcome::Aborted
                        };
                        frame.stage = SettleStage::RaiseTileResolved;
                    } else {
                        let owner = frame.owner;
                        let (i, at) = (frame.player, frame.at);
                        let m = frame.m.clone();
                        raise!(
                            self,
                            "settleAfter",
                            i,
                            @m m,
                            tile = at as i32,
                            target = owner,
                            settle_tile = at as i32
                        )?;
                        frame.stage = SettleStage::RaiseTileResolved;
                    }
                    true
                }
                SettleStage::RaiseTileResolved => {
                    let owner = frame.owner;
                    let (i, at) = (frame.player, frame.at);
                    let m = frame.m.clone();
                    // The terminal raise: the settle played out unless an
                    // earlier terminal branch already recorded otherwise.
                    let outcome = self.settle_stack[frame.stack_idx].outcome;
                    raise!(
                        self,
                        "tileResolved",
                        i,
                        @m m,
                        tile = at as i32,
                        target = owner,
                        settle_tile = at as i32,
                        outcome = Some(outcome)
                    )?;
                    frame.stage = SettleStage::Done;
                    false
                }
                SettleStage::Done => {
                    if frame.scoped {
                        self.w.end_plan(true);
                    }
                    false
                }
            };
            // A raise that suspended: park the current stage (not advanced)
            // and let the pump run the nested settle first.
            // (The `?` above already returned; this match completes stages.)
            if !more {
                return Ok(());
            }
        }
    }

    // =============================================================== tiles

    /// A card- or skill-driven dice roll. Sums `count`d`sides` the
    /// same way `roll_tables` does (a flat `count` when `sides <= 0`); the
    /// caller raises the `Roll` chain link so 「掷骰结算前」 counteractions see it.
    /// Always logs one `"dice"` event (all rolls animate) -- including the flat
    /// `sides <= 0` shape.
    pub fn card_roll(&mut self, player_id: usize, count: i32, sides: i32) -> i32 {
        if sides <= 0 {
            let total = count;
            let text = Msg::new("log.dice")
                .player_id("who", player_id as i32)
                .i("count", count)
                .i("sides", 0)
                .i("sum", total)
                .opt("detail", None);
            self.w.log("dice", player_id as i32, text).value = total;
            return total;
        }
        // Each roll gets its own `dice` line (faces included), so a card that
        // rolls several times (热气球演出's four 3d20) shows every roll.
        self.roll(player_id as i32, count.max(0), sides.max(1), None)
    }

    /// A card-driven reroll (resource-powered dice): sum the
    /// move plan's `base` + `dice` tables. The caller raises the `Roll` chain
    /// link on the face. Logs one `"dice"` event (all rolls animate), naming
    /// the plan's dice -- 松原花音's second 「移动掷骰」 reads 「掷 1d20 = 5」,
    /// never a bare `0d0`.
    ///
    /// Not on the guest path since v111: `ctx::do_move_roll` is `roll_direct` →
    /// `run_direct_roll`, whose cast logs the same line and folds the
    /// `diceRollCast` rewrite. Kept as the host-side helper for a direct plan
    /// roll (it honours `extreme`, like the turn's own move).
    /// 规则书: 「使任意结果只有数字区间的效果以理论最大值或最小值结算」 -- the
    /// move's `NdM` tables are such an interval, so `roll_tables` settles them at
    /// the armed extreme; the turn's main move rolls through the same helper.
    pub fn card_do_move_roll(&mut self, player_id: usize) -> i32 {
        let plan = self.w.plan().clone();
        let total = self.roll_tables(&plan);
        let text = plan_dice_msg(player_id as i32, &plan, total);
        self.w.log("dice", player_id as i32, text).value = total;
        total
    }

    /// Resolve the tile a player stopped on.
    pub fn land(&mut self, i: usize, is_main_move: bool) -> Flow<()> {
        let at = self.w.st.players[i].pos as usize;
        self.land_at_built_in(i, at, is_main_move)
    }

    /// The landing gates and bookkeeping both settle bodies share: nothing
    /// settles for an off-board / out player or off the board, and a main landing
    /// records where the player stopped (the end-step buy/build gates ask).
    /// Returns false when the settle does not run at all.
    pub fn prep_land(&mut self, i: usize, at: usize, is_main_move: bool) -> bool {
        if at >= self.data.tiles.len()
            || !self.w.gate_open(self.rules, super::Gate::OnBoard, i)
            || self.out(i)
        {
            return false;
        }
        if is_main_move {
            self.w.st.landed = at as i32;
        }
        true
    }

    /// Land on a named tile (the [触发结算] half of [`Self::settle_at`]).
    ///
    /// The **built-in** settle body: what `CardRules::settle_tile` falls back
    /// to when the ruleset has no rule instance for the tile (`StubRules`, or
    /// a kind not yet migrated -- `docs/TILES.md`). Also the whole of `land`
    /// for a card that resolves a tile directly.
    pub fn land_at_built_in(&mut self, i: usize, at: usize, is_main_move: bool) -> Flow<()> {
        if !self.prep_land(i, at, is_main_move) {
            return Ok(());
        }
        let tile = self.tile(at);
        if tile.is_buyable() {
            let owner = self.w.st.owners[at];
            if owner < 0 {
                // The settle of an unowned buyable IS the optional buy
                // (规则书 「可选择[消耗]…获得格子地契和拥有权」).
                self.offer_buy(i, at)?;
            } else if owner as usize != i {
                self.pay_rent(i, at, false)?;
            } else {
                self.offer_build(i, at)?;
            }
            return Ok(());
        }
        // Non-deed: the settle body is the ruleset's (`CardRules::settle_tile`
        // rule instances). With no bound rule (StubRules) there is only the
        // generic landing log -- no game-named tile branches.
        self.w.log(
            "text",
            i as i32,
            Msg::new("log.land").player_id("who", i).tile("tile", at),
        );
        Ok(())
    }

    /// Property rent from the rent table; the flat deed rent is
    /// rings owned x multiplier x 1d20; agents charge half.
    ///
    /// Also the `ctx::pay_rent` primitive (`docs/TILES.md`): the deed tile rule /
    /// the flat-deed tile rule / the agent tile rule bodies call it with the rulebook's 「half-charge」
    /// half-flag. A mortgaged deed charges no rent -- the caller decides
    /// between this and [`Self::offer_force_buy`], per 「如果格子地契已抵押」.
    pub fn card_pay_rent(&mut self, i: usize, t: usize, half: bool) -> Flow<()> {
        self.pay_rent(i, t, half)
    }

    /// `ctx::charge_rent` -- a rent-shaped 「收费」 at `milli_scale` (1000 = ×1.0)
    /// of the tile's ordinary-settle rent, **outside** a full [触发结算].
    ///
    /// 规则书 (`RAS:练习室里的风暴` (2), sheet 2026-10-06): 「对那个玩家进行一次
    /// 相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费」. The amount is the
    /// same figure [`Self::pay_rent`] charges ([`Self::rent_figure`] -- table /
    /// houses / the dice-rent roll) and the scale rides
    /// [`Self::scale_settle_payment`] with the rent factors
    /// (`Pay::milli_scale`, `spec/ROUNDING.md`). The payment goes through the
    /// money pipeline as [`PayKind::Rent`] (so `PaySplitAdd` / rent factors see
    /// it and `pay_is_rent` is true) -- but this is a 「收费」, not a 「结算」:
    /// no settle hooks, no counteraction-as-settle, no buy/build offer.
    ///
    /// Refuses (returns 0) when the tile is unowned, mortgaged, the payer is
    /// the owner, or either side is out.
    ///
    /// TODO(规则书)[ruling]: on a **mortgaged** tile 「普通[结算]」 is a forced-buy
    /// offer, not a rent, so there is no obvious 收费 recipient. Class C.
    /// Refused here rather than guessing.
    ///
    /// `by_card` is the player whose card caused the charge (stamped onto the
    /// `pay` / `paid` triggers so 「被…效果影响」 can tell card from board).
    /// Returns the amount actually paid. Counts toward the turn's
    /// `paid_in_settle` when the charge runs inside a settle.
    pub fn card_charge_rent(
        &mut self,
        i: usize,
        t: usize,
        milli_scale: i32,
        by_card: Option<i32>,
    ) -> Flow<i32> {
        let owner = self.w.st.owners[t];
        if owner < 0 || owner as usize == i || self.out(i) || self.out(owner as usize) {
            return Ok(0);
        }
        let owner = owner as usize;
        // TODO(规则书)[ruling]: mortgaged 「普通[结算]」 is a forced-buy offer, not
        // a rent -- no obvious recipient. Class C; refuse rather than guess.
        if self.w.st.mortgaged[t] {
            return Ok(0);
        }
        let (amount, detail) = self.rent_figure(owner, t);
        if amount <= 0 {
            return Ok(0);
        }
        let mut p = Pay::new(amount, PayKind::Rent);
        p.from = Some(i);
        p.to = Some(owner);
        p.typ = Some(PayEvent::Rent);
        p.tile = Some(t);
        p.must = true;
        p.milli_scale = milli_scale;
        p.by_card = by_card;
        p.source = "src.rent";
        p.text = Some(
            Msg::new("log.rent")
                .player_id("who", i)
                .tile("tile", t)
                .player_id("owner", owner)
                .opt("note", detail),
        );
        let paid = self.money(p)?;
        Ok(if paid.paid { paid.loss } else { 0 })
    }

    /// The figure `card_charge_rent(payer, tile)` would charge as a **fresh**
    /// hypothetical charge: the quote of the tile's rent scaled by the tile's
    /// own factors and the standing plan / settle factors. The in-flight
    /// payment's own rent factor and `milli_scale` are not in it, so a
    /// half-charge on top of a rent is not scaled twice. `-1` = no rent charge
    /// (unowned, own tile, mortgaged, or a zero table). Rolls a fresh d20 for a
    /// dice-rent tile and therefore advances the RNG.
    pub fn rent_quote(&mut self, payer: usize, t: usize) -> i32 {
        self.w.rent_quote(self.data, payer, t)
    }

    /// The `ctx::offer_force_buy` primitive: 「可选择pay…两倍…强行购买」.
    pub fn card_offer_force_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        self.offer_force_buy(i, t)
    }

    /// The `ctx::offer_buy` primitive: 「可选择spend…获得格子地契和拥有权」.
    /// Called from the tile settle body (`tile:property` / `tile:ring`) -- the
    /// ONE place that offers a normal land buy.
    pub fn card_offer_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        self.offer_buy(i, t)
    }

    /// The `ctx::offer_build` primitive: 「可选择spend格子地契所标注的房屋建筑费
    /// 进行升级建造」 on one's own land. Called from the tile settle body.
    pub fn card_offer_build_one(&mut self, i: usize, t: usize) -> Flow<()> {
        self.offer_build(i, t)
    }

    /// The ordinary-settle rent figure of `t` as `pay_rent` charges it, plus
    /// its detail note (`log.part.rent_houses` / `log.part.rent_dice`).
    /// **Single source** for [`Self::pay_rent`] and [`Self::card_charge_rent`]:
    /// the table number at the counted house count ([`World::rent_houses`]) or
    /// the dice-rent roll (`unit × 1d20`).
    fn rent_figure(&mut self, owner: usize, t: usize) -> (i32, Option<Msg>) {
        self.w.rent_figure(self.data, owner, t)
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
        let (mut amount, detail) = self.rent_figure(owner, t);
        let mut half_part = None;
        let mut half_only = None;
        if half {
            let full = amount;
            amount = purchase::half_ceil10(full);
            half_part = Some(
                Msg::new("log.part.rent_half")
                    .n("full", full)
                    .n("cut", amount),
            );
            // Bare in its own parens: no leading comma (`rent_half` is the
            // tail of a house / rent-unit note).
            half_only = Some(
                Msg::new("log.part.rent_half_only")
                    .n("full", full)
                    .n("cut", amount),
            );
        }
        // The 「支付」 / 「地租」 scalars no longer scale the rent table here:
        // they ride the money pipeline's `paySplitMul` stage (`scale_settle_payment`),
        // so a rule that halves a settlement payment covers the payment **as
        // card effects have shaped it** -- a rent-region expansion, a forced
        // stop-and-pay -- and not just this table's number. 规则书
        // (`docs/TILES.md`, 「支付减半」 ruling 2026-10-06).
        let note = match (detail, half_part) {
            (Some(d), h) => Some(d.opt("half", h)),
            (None, _) => half_only,
        };
        let mut p = Pay::new(amount, PayKind::Rent);
        p.from = Some(i);
        p.to = Some(owner);
        p.typ = Some(PayEvent::Rent);
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
        self.money(p)?;
        // The turn tally is the money pipeline's: it counts while a settle is
        // open, whatever the payment kind (「本回合的[结算]向其他玩家支付了
        // 至少1000资金」).
        self.wait(0.9);
        Ok(())
    }

    /// Raise one amount stage and fold the answer into `amount` (MONEY-01).
    ///
    /// A [`Fold::Add`] / [`Fold::Mul`] stage hands every responder the stage
    /// input and composes their writes (sum / product); a [`Fold::Replace`]
    /// stage overwrites. `modified` records that some stage shaped the figure
    /// (the single end-of-calc round reads it).
    #[allow(clippy::too_many_arguments)]
    fn apply_amount_stage(
        &mut self,
        kind: &str,
        fold: Fold,
        side: usize,
        payer: i32,
        to: &mut Option<usize>,
        amount: &mut i32,
        modified: &mut bool,
        p: &mut Pay,
        rent: &mut bool,
        fixed: &mut bool,
    ) -> Flow<()> {
        let before = *amount;
        let target = to.map_or(-1, |t| t as i32);
        let (by_card, total_stage) = (p.by_card, p.runs_total_stage);
        let tile = p.tile.map_or(-1, |t| t as i32);
        let settle_tile = self.settle_tile();
        let trig = TriggerPay::new(p.kind, *rent, total_stage, p.shape);
        let t = match fold {
            Fold::Replace => raise!(
                self, kind, side, player_id = payer, target = target, value = before,
                by_card = by_card, @p trig, settle_tile = settle_tile, tile = tile
            ),
            Fold::Add => raise!(
                self, kind, side, player_id = payer, target = target, value = before,
                by_card = by_card, @p trig, settle_tile = settle_tile, tile = tile, @f add
            ),
            Fold::Mul => raise!(
                self, kind, side, player_id = payer, target = target, value = before,
                by_card = by_card, @p trig, settle_tile = settle_tile, tile = tile, @f mul
            ),
        }?;
        // The stage may rewrite the payload (`set_pay_kind` / `set_pay_shape`).
        Self::fold_pay_payload(p, &t, rent, fixed);
        if crate::engine::stage_dbg() {
            eprintln!(
                "STAGE {kind} rent={rent} before={before} after={} tile={tile} st={settle_tile}",
                t.value
            );
        }
        if t.value != before {
            *modified = true;
        }
        *amount = t.value;
        if t.target >= 0 {
            *to = Some(t.target as usize);
        }
        Ok(())
    }

    /// The 「支付」 scale home, applied at the mul phase's tail.
    ///
    /// * the move plan's `pay_factor` (`plan::set_pay_factor`) -- a card that
    ///   shapes a settle (祥，移动's 「触发结算时进行的支付价格减半」, Repaint's
    ///   「对方此次结算的支付减半」) arms it, because a hand card leaves the
    ///   field and cannot leave a hook behind;
    /// * the tile's `prop::PAY_FACTOR` -- a card that retunes a tile writes it
    ///   (`ctx::set_tile_prop`). It applies to *any* payment on the tile, not
    ///   only rent.
    ///
    /// The rent-side factors (`plan.rent_factor`, `prop::RENT_FACTOR`) and the
    /// caller's [`Pay::milli_scale`] are applied by the `rentMul` phase; all of
    /// them are multiplicative contributions, so the one end-of-calc `ceil10`
    /// sees the fully shaped figure (`spec/ROUNDING.md`).
    ///
    /// Scope: a settlement loss. A purchase (`buy` / `build`), a forced
    /// purchase (rulebook 「此次购买的价格不受任何资金变动效果影响」) and a
    /// print (`gain`) are never scaled. TODO(规则书): whether 「支付」 also
    /// reaches a non-settlement loss a card forces outside a settle.
    fn scale_settle_payment(
        &self,
        amount: i32,
        kind: PayKind,
        rent: bool,
        tile: Option<usize>,
        milli_scale: i32,
        modified: &mut bool,
    ) -> i32 {
        if matches!(
            kind,
            PayKind::Buy | PayKind::Build | PayKind::ForceBuy | PayKind::Gain
        ) {
            return amount;
        }
        // The engine's own contributions as one product: the move's pay /
        // rent plan factors, the open settles' (a `thisSettle` write composes
        // there, and the move tail clears the plan half outside a move), the
        // caller's formula scale (permille; `0` = free, a negative clamps to
        // 0 -- always multiplied, it is not a 「0 means unset」 prop) and the
        // tile's standing factor ([`purchase::tile_money_milli`], the single
        // source for that half; the standing quote reads the same figure).
        let mut milli = (self.w.plan().pay_factor * 1_000_000.0).round() as i64;
        if rent {
            let rf = (self.w.plan().rent_factor * 1_000_000.0).round() as i64;
            milli = milli.saturating_mul(rf) / 1_000_000;
        }
        milli = milli.saturating_mul(i64::from(milli_scale.max(0)) * 1000) / 1_000_000;
        if let Some(t) = tile {
            milli = milli.saturating_mul(purchase::tile_money_milli(&self.w, t, rent)) / 1_000_000;
        }
        if crate::engine::stage_dbg() {
            eprintln!(
                "SCALE kind={kind:?} rent={rent} tile={tile:?} milli={milli} amount={amount}"
            );
        }
        apply_milli_factor(amount, milli, modified)
    }

    /// Mortgaged land charges no rent but may be bought out at
    /// twice its value (`docs/PURCHASE.md` P3). The `BuyGate` hook runs first
    /// (Poppin's hill lock covers Force too); the price is `FORCE_MULT` × the
    /// deed value (default ×2); `FORCE_STAYS_MORTGAGED` keeps the mortgage.
    /// TODO(规则书) ruling 6: no `bought` / `buyAfter` hooks until it is decided
    /// whether 强制购买 is a 「购买」.
    fn offer_force_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        let owner = self.w.st.owners[t] as usize;
        // `BuyGate` -- every kind, Force included (Poppin's hill lock covers
        // 「不可被抵押双倍支付购买」). Re-quoted below, so quote == charge.
        let gate = raise!(
            self,
            "buyGate",
            i,
            tile = t as i32,
            @b TriggerBuy::new(purchase::BuyKind::Force, owner as i32, 0)
        )?;
        if gate.is_cancelled() {
            return Ok(());
        }
        // The quoted Force price: 2×(land + houses) as the base, then whatever
        // `BuyAdd` / `BuyMul` / `BuySet` hooks make of it (`docs/PURCHASE.md`).
        let quote = self.buy_quote_for(i, t, purchase::BuyKind::Force);
        if !quote.eligible || quote.price < 0 {
            return Ok(());
        }
        let price = quote.price.max(0);
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
        // Standard reads `StrategyParams::force_buy_reserve` (default ==
        // `FORCE_BUY_RESERVE`); chaos keeps its own `CHAOS_RESERVE`.
        let wants_force = self.ai_wants_force_buy(i, price);
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
        .with_ai(|_| if wants_force { 0 } else { 1 })
        .with_tile(t);
        let mut ask = ask;
        ask.view.price = price;
        if self.ask(ask)?.of(i) == 0
            && self.w.st.owners[t] == owner as i32
            && !self.out(owner)
            && !self.out(i)
            && self.w.st.players[i].money >= price
        {
            // Direct transfer: 「此次购买的价格不受任何资金变动效果影响，收款方
            // 无论处于何种状态都可正常收款」. The pay pipeline does not see it.
            self.w.st.players[i].money -= price;
            self.w.credit(owner as i32, price);
            // The deal: `BuyAssign` rewrites owner / houses / mortgage before
            // the one ownership write (`docs/PURCHASE.md`). The rulebook default
            // is 「获得的地契仍为抵押状态」; TODO(规则书): `FORCE_STAYS_MORTGAGED`
            // (`= 0` clears) is not distinguishable from "absent" through
            // `tile_prop`, so the native default holds unconditionally.
            let mut deal = purchase::default_deal(
                self.data,
                &self.w.st,
                t,
                i,
                purchase::BuyKind::Force,
                price,
                true,
            );
            let at = raise!(
                self,
                "buyAssign",
                i,
                tile = t as i32,
                @b TriggerBuy::new(purchase::BuyKind::Force, owner as i32, price).deal(
                    Payee::from_i32(deal.owner_after),
                    deal.houses_after,
                    deal.mortgaged_after,
                )
            )?;
            if let Some(b) = at.buy {
                if let Some(o) = b.deal_owner {
                    deal.owner_after = o.as_i32();
                }
                if let Some(h) = b.deal_houses {
                    deal.houses_after = h;
                }
                deal.mortgaged_after = b.deal_mortgaged;
            }
            purchase::assign_deed(
                &mut self.w,
                t,
                deal.owner_after,
                deal.houses_after,
                deal.mortgaged_after,
            );
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

    /// Offer a buy of unowned land. The tile settle body calls this -- it is
    /// the ONE offer site for a normal land buy (规则书 「无主的[可购买格子]的
    /// [结算]是：可选择[消耗]…获得格子地契和拥有权」).
    fn offer_buy(&mut self, i: usize, t: usize) -> Flow<()> {
        // Never prompt for an offer `buy` would refuse: the plan bars a buy,
        // or a `BuyGate` hook vetoes it.
        if self.w.plan().no_buy {
            return Ok(());
        }
        let quote = self.buy_quote_for(i, t, purchase::BuyKind::Land);
        if !quote.eligible || quote.price < 0 {
            return Ok(());
        }
        let price = quote.price.max(0);
        if !self.can_pay(i) || self.w.purchase_funds(self.data, i, None) < price {
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
        let mut ask = Ask::choice(
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
        ask.view.price = price;
        if self.ask(ask)?.of(i) == 0 && self.w.st.owners[t] < 0 && !self.out(i) {
            self.deed_buy(i, t, purchase::BuyKind::Land, 0)?;
        }
        Ok(())
    }

    /// Offer a build on own land. The tile settle body calls this -- it is
    /// the ONE offer site for a normal build (规则书 「玩家拥有的[可购买格子]的
    /// [结算]是：…可选择[消耗]…进行升级建造」).
    fn offer_build(&mut self, i: usize, t: usize) -> Flow<()> {
        if self.cant_build_on(i, t).is_some()
            || self.w.purchase_funds(self.data, i, Some(t)) < self.build_cost(t)
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
        if self.ask(ask)?.of(i) == 0 && self.cant_build_on(i, t).is_none() {
            self.deed_build(i, t)?;
        }
        Ok(())
    }

    // =============================================================== property

    pub fn build_cost(&self, t: usize) -> i32 {
        purchase::build_cost(self.data, t)
    }

    /// The mortgage payout ([`purchase::mortgage_value`], the engine-side copy
    /// of the rulebook figure). A retuned figure lives on the tile rule
    /// instance's props (`docs/TILES.md`), not on a `CardRules` hook.
    pub(crate) fn mortgage_value(&self, t: usize) -> i32 {
        purchase::mortgage_value(self.data, t)
    }

    /// The redeem charge ([`purchase::redeem_cost`], the engine-side copy of
    /// the rulebook figure). A retuned figure lives on the tile rule
    /// instance's props (`docs/TILES.md`), not on a `CardRules` hook.
    pub fn redeem_cost(&self, t: usize) -> i32 {
        purchase::redeem_cost(self.data, t)
    }

    /// Deeds a player could mortgage (not the flat deed, not already mortgaged).
    pub(crate) fn mortgageable(&self, player_id: usize) -> Vec<usize> {
        (0..self.data.tiles.len())
            .filter(|&t| {
                self.w.st.owners[t] == player_id as i32
                    && self.tile(t).is_buyable()
                    && self.w.tile_prop(t as i32, crate::state::prop::NO_MORTGAGE) == 0
                    && !self.w.st.mortgaged[t]
            })
            .collect()
    }

    /// Bare land first, cheapest first at the default
    /// [`crate::strategy::StrategyParams`] keys (the sort key is
    /// `(has_houses · house_key, price · price_key, tile)`, which with both
    /// keys at `1000` is exactly the old `(houses > 0, price, t)` order).
    fn mortgage_order(
        &self,
        mut deeds: Vec<usize>,
        p: &crate::strategy::StrategyParams,
    ) -> Vec<usize> {
        deeds.sort_by_key(|&t| p.mortgage_key(self.w.st.houses[t], self.tile(t).price, t));
        deeds
    }

    /// The AI's (and the time-out) mortgage selection.
    ///
    /// Standard: bare land first, cheapest first, until `need` is covered (the
    /// seat's [`crate::strategy::StrategyParams`] key).
    /// Chaos: a random subset that covers `need` -- at least one deed, never a
    /// tidy little list.
    pub fn auto_mortgage(&mut self, player_id: usize, need: i32) -> Vec<String> {
        self.auto_mortgage_from(player_id, need, self.mortgageable(player_id))
    }

    pub fn auto_mortgage_from(
        &mut self,
        player_id: usize,
        need: i32,
        deeds: Vec<usize>,
    ) -> Vec<String> {
        if self.is_chaos(player_id) {
            let mut rest = deeds;
            let mut got = 0;
            let mut out = vec![];
            while !rest.is_empty() && (got < need || out.is_empty()) {
                let k = self.w.rng.below(rest.len());
                let t = rest.remove(k);
                out.push(t.to_string());
                got += self.mortgage_value(t);
            }
            return out;
        }
        let p = self.strategy_of(player_id);
        let mut got = 0;
        let mut out = vec![];
        for t in self.mortgage_order(deeds, &p) {
            if got >= need {
                break;
            }
            out.push(t.to_string());
            got += self.mortgage_value(t);
        }
        out
    }

    /// The rule lives on [`super::ops`] so a card asking
    /// "where may I build?" gets the same answer the build step does.
    pub(crate) fn cant_build_on(&self, i: usize, t: usize) -> Option<Msg> {
        self.w
            .cant_build_on(self.data, i as i32, t as i32)
            // The payer's gate (the pay-gate layers 「无法收付款」) -- `can_pay` also
            // refuses an out seat.
            .or_else(|| (!self.can_pay(i)).then(|| Msg::new("err.cannot_spend")))
    }

    /// The quoted price for one tile, through the rules crate's `buy_quote`
    /// (`docs/PURCHASE.md`). For `StubRules` this is `quote_native` -- the
    /// plain rulebook formula -- so the sim's behaviour is unchanged.
    pub fn buy_quote_for(&self, i: usize, t: usize, kind: purchase::BuyKind) -> purchase::Quote {
        // Force-buy and 收购 name the owner as the payee; every other kind buys
        // from the bank (`docs/PURCHASE.md`).
        let seller = match kind {
            purchase::BuyKind::Force | purchase::BuyKind::Acquire => {
                self.w.st.owners.get(t).copied().unwrap_or(-1)
            }
            _ => -1,
        };
        purchase::quote_for(self.rules(), &self.w, self.data, i, kind, &[(t, seller)])
            .into_iter()
            .next()
            .unwrap_or(purchase::Quote {
                price: -1,
                eligible: false,
            })
    }

    /// Run a purchase of tile `t` for player `i` at this [`purchase::BuyKind`].
    /// `true` when the deed actually changed hands (HOST-06: every refusal
    /// path -- `BuyGate` / `buyBefore` cancel, already owned, unpaid quote,
    /// unpaid money -- is `false`).
    pub fn buy(&mut self, i: usize, t: usize, kind: purchase::BuyKind) -> Flow<bool> {
        // `BuyGate` -- may this player buy this tile at all? Every kind, Force
        // included (Poppin's hill lock covers 「不可被抵押双倍支付购买」).
        let gate = raise!(
            self,
            "buyGate",
            i,
            tile = t as i32,
            @b TriggerBuy::new(kind, -1, 0)
        )?;
        if gate.is_cancelled() {
            return Ok(false);
        }
        // rana's 「该次传送不可进行地契购买」 (`docs/PURCHASE.md`) -- the
        // plan's no-buy flag is a **native** gate on the ordinary Land offer.
        // TODO(规则书) ruling 6: whether it
        // also bars a card-driven (`BuyKind::Card`) buy is undecided; today it
        // does not, matching the flag being about the turn's own landing.
        if kind == purchase::BuyKind::Land && self.w.plan().no_buy {
            return Ok(false);
        }
        // `buyBefore` -- before any guard, so it is a real pre-hook (runs even
        // on the no-op/already-owned path). A hook that claims it with
        // `set_cancelled` vetoes the buy outright (Poppin's hill lock).
        let bt = raise!(self, "buyBefore", i, tile = t as i32)?;
        if bt.is_cancelled() {
            return Ok(false);
        }
        if self.out(i) || !self.playing() {
            return Ok(false);
        }
        if self.w.st.owners[t] >= 0 {
            return Ok(false);
        }
        // The quoted price (the rules crate's `buy_quote`, re-quoted here at
        // commit so quote == charge). For `StubRules` this is the plain
        // rulebook formula; the `BuyAdd` / `BuyMul` / `BuySet` stages ride on
        // top of it in the rules crate.
        let quote = self.buy_quote_for(i, t, kind);
        if quote.price < 0 {
            return Ok(false);
        }
        let mut price = quote.price.max(0);
        let houses = self.w.st.houses[t];
        if price > 0 {
            let mut p = Pay::new(price, PayKind::Buy);
            p.from = Some(i);
            p.typ = Some(PayEvent::Lose);
            p.tile = Some(t);
            p.source = "src.buy";
            p.allow_mortgage = matches!(kind, purchase::BuyKind::Land | purchase::BuyKind::Agent);
            p.text = Some(Msg::new("log.paid_for").player_id("who", i).tile("tile", t));
            let paid = self.money(p)?;
            if !paid.paid {
                return Ok(false);
            }
            price = paid.loss;
        }
        // The deal: `BuyAssign` rewrites owner / houses / mortgage before the
        // one ownership write (`docs/PURCHASE.md`). A land buy clears the
        // mortgage; 迷宫般的仓库's 「拆除那个格子上的所有房屋」 is a `BuyAssign`
        // rewrite of `deal_houses`, not an engine flag.
        let mut deal = purchase::default_deal(self.data, &self.w.st, t, i, kind, price, true);
        let at = raise!(
            self,
            "buyAssign",
            i,
            tile = t as i32,
            @b TriggerBuy::new(kind, deal.seller, price).deal(
                Payee::from_i32(deal.owner_after),
                deal.houses_after,
                deal.mortgaged_after,
            )
        )?;
        if let Some(b) = at.buy {
            if let Some(o) = b.deal_owner {
                deal.owner_after = o.as_i32();
            }
            if let Some(h) = b.deal_houses {
                deal.houses_after = h;
            }
            deal.mortgaged_after = b.deal_mortgaged;
        }
        purchase::assign_deed(
            &mut self.w,
            t,
            deal.owner_after,
            deal.houses_after,
            deal.mortgaged_after,
        );
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
        // `bought` (hook) -- the owner is set.
        raise!(self, "bought", i, tile = t as i32)?;
        // `buyAfter` -- the deed has changed hands.
        raise!(self, "buyAfter", i, tile = t as i32)?;
        Ok(true)
    }

    pub fn build(&mut self, i: usize, t: usize) -> Flow<bool> {
        // `buildBefore` -- before any guard or payment, so it is a real pre-hook.
        // A hook that claims it with `set_cancelled` vetoes the build outright
        // (卡池BUG 「不可在造价1500及以上的格子上加盖房屋」).
        let bt = raise!(self, "buildBefore", i, tile = t as i32)?;
        if bt.is_cancelled() {
            return Ok(false);
        }
        if self.out(i) || !self.playing() {
            return Ok(false);
        }
        let mut cost = self.build_cost(t);
        // 「加盖房屋时半价」 / 「本回合加盖房屋变为免费」 -- the rate is a value
        // the skill writes; the engine only applies it.
        let pct = self.w.turn.build_cost_pct.clamp(0, 100);
        if pct != 100 {
            cost = cost * pct / 100;
        }
        // 「下次盖房时减免N（可溢出），盖房后减少1层」 -- a layered cut on the
        // cost. 「可溢出」: the cut is not clamped to the cost; the part past 0
        // is refunded as money. One layer pops whether or not the build
        // actually commits.
        let cut = self.w.turn.build_discount;
        if cut > 0 {
            cost -= cut;
        }
        if cost < 0 {
            let refund = -cost;
            self.w.credit(i as i32, refund);
            cost = 0;
        }
        if self.w.turn.build_discount_layers > 0 {
            self.w.turn.build_discount_layers -= 1;
            if self.w.turn.build_discount_layers == 0 {
                self.w.turn.build_discount = 0;
            }
        }
        if cost > 0 {
            let mut p = Pay::new(cost, PayKind::Build);
            p.from = Some(i);
            p.typ = Some(PayEvent::Lose);
            p.tile = Some(t);
            p.source = "src.build";
            p.allow_mortgage = true;
            p.text = Some(
                Msg::new("log.paid_for_house")
                    .player_id("who", i)
                    .tile("tile", t),
            );
            let paid = self.money(p)?;
            if !paid.paid {
                return Ok(false);
            }
            cost = paid.loss;
        }
        let full = self.tile(t).rent.len().saturating_sub(1) as i32;
        if self.w.st.owners[t] != i as i32 || self.w.st.houses[t] >= full {
            if cost > 0 && !self.out(i) {
                self.w.credit(i as i32, cost);
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
            return Ok(false);
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
        // `houseAdded` -- 「你拥有格子上的房屋总数增加时」 (鳰原令王那 (2)) is
        // the post-commit point for *any* house increase, the same raise the
        // guest's `add_house` logs at its own commit (`wasm_rules.rs`
        // `house_log`). The engine's build writes `houses[t]` directly and used
        // to raise nothing here, so a real 盖房 never offered the skill.
        // Raised before `buildAfter` (the hook point before the action hook,
        // matching `buy()`'s `bought` → `buyAfter`).
        raise!(self, "houseAdded", i, tile = t as i32, value = h)?;
        // `buildAfter` -- only after the house actually commits (not on the
        // refund path above).
        raise!(self, "buildAfter", i, tile = t as i32)?;
        Ok(true)
    }

    /// Why this deed cannot be mortgaged, or `None` when it can.
    pub fn guard_mortgage(&self, i: usize, t: i32, asking: bool) -> Option<Msg> {
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
        if self.w.tile_prop(t as i32, crate::state::prop::NO_MORTGAGE) > 0 {
            return Some(Msg::new("err.no_mortgage"));
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
        if st.step == stage::END {
            // The settle (which owns the optional buy / build) has already
            // run; there is no END-step purchase left to fund.
            return Some(Msg::new("err.mortgage_after_roll"));
        }
        None
    }

    /// Why this deed cannot be redeemed, or `None` when it can.
    pub fn guard_redeem(&self, i: usize, t: i32, asking: bool) -> Option<Msg> {
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

    /// Mortgage a deed and credit its value to the owner.
    /// Mechanism only -- the tile rule's `On::Mortgage` body owns eligibility
    /// and the amount (`ctx::commit_mortgage`). Raises the mortgage hooks so
    /// 「当你抵押…时」 listeners (and their counteractions) still fire.
    pub fn commit_mortgage(&mut self, i: usize, t: usize, amount: i32) -> Flow<i32> {
        if t >= self.w.st.mortgaged.len() || self.w.st.mortgaged[t] {
            return Ok(0);
        }
        if self.w.st.owners.get(t).copied().unwrap_or(-1) != i as i32 {
            return Ok(0);
        }
        let v = amount.max(0);
        // `mortgageBefore` -- a real pre-hook (a counteraction can block).
        let before = raise!(self, "mortgageBefore", i, tile = t as i32)?;
        if before.is_cancelled() {
            return Ok(0);
        }
        self.do_mortgage(i, t, v, None);
        raise!(self, "mortgage", i, tile = t as i32)?;
        Ok(v)
    }

    /// Lift a mortgage and charge `amount` directly. Mechanism only.
    ///
    /// The `spendBefore` window runs first, at the full cost, before the
    /// direct debit: a responder may reduce (or zero) it, and a cancel leaves
    /// the figure alone. It never moves money itself.
    pub fn commit_redeem(
        &mut self,
        i: usize,
        t: usize,
        amount: i32,
        by_card: Option<i32>,
    ) -> Flow<i32> {
        if t >= self.w.st.mortgaged.len() || !self.w.st.mortgaged[t] {
            return Ok(0);
        }
        if self.w.st.owners.get(t).copied().unwrap_or(-1) != i as i32 {
            return Ok(0);
        }
        let cost = self.spend_before(i, t as i32, amount, by_card)?;
        self.w.st.mortgaged[t] = false;
        self.w.st.players[i].money -= cost;
        // 「本回合进行过赎回操作」 -- same record, same timing.
        {
            let s = &mut self.w.st.players[i];
            s.state_set(key::REDEEMED, 1);
            s.state_set_expires(key::REDEEMED, Some(Tick::TurnEnd));
        }
        let text = Msg::new("log.redeem")
            .player_id("who", i)
            .n("cost", cost)
            .tile("tile", t);
        let e = self.w.log("redeem", i as i32, text);
        e.value = cost;
        e.to = t as i32;
        Ok(cost)
    }

    /// Mortgage a deed and credit `v` to the owner (raw mechanism).
    fn do_mortgage(&mut self, i: usize, t: usize, v: i32, reason: Option<Msg>) {
        // 「本回合进行过抵押操作」 -- a record the card gates ask about. Timed to
        // the turn end so it does not leak.
        let s = &mut self.w.st.players[i];
        s.state_set(key::MORTGAGED, 1);
        s.state_set_expires(key::MORTGAGED, Some(Tick::TurnEnd));
        self.w.st.mortgaged[t] = true;
        self.w.credit(i as i32, v);
        let text = Msg::new("log.mortgage")
            .player_id("who", i)
            .tile("tile", t)
            .n("n", v)
            .opt(
                "reason",
                reason.map(|w| Msg::new("log.part.reason").msg("reason", w)),
            );
        let e = self.w.log("mortgage", i as i32, text);
        e.value = v;
        e.to = t as i32;
    }

    /// Route a mortgage through the tile rule's `On::Mortgage` body
    /// (`CardRules::deed_mortgage`). `reason`: 0 = player act, 1 = raise-funds,
    /// 2 = optional funding prompt.
    pub fn deed_mortgage(&mut self, i: usize, t: usize, reason: i32) -> Flow<bool> {
        self.rules().deed_mortgage(self, i, t, reason)
    }

    /// Route a redeem through the tile rule's `On::Redeem` body.
    pub fn deed_redeem(&mut self, i: usize, t: usize) -> Flow<bool> {
        self.rules().deed_redeem(self, i, t)
    }

    /// Route a buy through the tile rule's `On::Buy` body
    /// (`CardRules::deed_buy`). `reason`: 0 = settle offer, 1 = card-driven,
    /// 2 = agent, 3 = acquire / auction.
    pub fn deed_buy(
        &mut self,
        i: usize,
        t: usize,
        kind: purchase::BuyKind,
        reason: i32,
    ) -> Flow<bool> {
        self.rules().deed_buy(self, i, t, kind, reason)
    }

    /// Route a build through the tile rule's `On::Build` body
    /// (`CardRules::deed_build`).
    pub fn deed_build(&mut self, i: usize, t: usize) -> Flow<bool> {
        self.rules().deed_build(self, i, t)
    }

    /// Mortgage one of the player's deeds.
    /// `true` when the deed actually went onto mortgage (HOST-06).
    pub fn mortgage(&mut self, i: usize, t: usize, reason: Option<Msg>) -> Flow<bool> {
        // `mortgageBefore` -- runs before any guard, so it is a real pre-hook
        // (a counteraction can block the mortgage). The post half below only runs
        // when the mortgage actually applied.
        raise!(self, "mortgageBefore", i, tile = t as i32)?;
        if self.out(i) || !self.playing() {
            return Ok(false);
        }
        if self.w.st.owners[t] == i as i32 && !self.w.st.mortgaged[t] {
            let v = self.mortgage_value(t);
            self.do_mortgage(i, t, v, reason);
            raise!(self, "mortgage", i, tile = t as i32)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// The `spendBefore` window of a raw spend (redeem, auction, `lose_raw`).
    ///
    /// Opens as a counteraction link *and* a hook dispatch (`SpendBefore` has
    /// a `ChainKind`, like the `*Before` pay stages): a hand card answers with
    /// `On::Counteract`, a placed card hooks it as an action. Responders see
    /// the **full cost** and reduce it with `trigger::set_pay_amount`; a cancel
    /// is not a failure and leaves the figure as it is. Never moves money.
    pub fn spend_before(
        &mut self,
        i: usize,
        tile: i32,
        value: i32,
        by_card: Option<i32>,
    ) -> Flow<i32> {
        let t = raise!(
            self,
            "spendBefore",
            i,
            tile = tile,
            value = value.max(0),
            by_card = by_card
        )?;
        Ok(t.value.max(0))
    }

    /// Pay the redeem cost and lift the mortgage. `true` when the deed was
    /// lifted.
    pub fn redeem(&mut self, i: usize, t: usize) -> Flow<bool> {
        let cost = self.redeem_cost(t);
        Ok(self.commit_redeem(i, t, cost, None)? > 0)
    }

    // =============================================================== money

    /// Move money between players and/or the bank. A mandatory payment
    /// the payer can't cover triggers `RaiseFunds` (and possibly bankruptcy).
    ///
    /// This is the **one pipeline** every money operation runs through. It
    /// stages the adjustment points -- pre-split `effect` (the counteraction window)
    /// then `paySplitAdd` / `paySplitMul` / `paySplitChoose` / `paySplitAt`, then split into
    /// concrete payer/payee entries, then post-split `pay` / `payAfter` /
    /// `paid` -- and runs before/after money events on every entry.
    ///
    /// Re-entrancy is bounded: a hook cannot re-trigger on its own money
    /// movement (`PlayCtx::reentrant_hooks`), and [`MAX_MONEY_DEPTH`] is a safety
    /// cap that traps loudly. The rulebook has no depth limit, so nested money
    /// opens its counteraction windows at every depth.
    /// Fold a pay window's payload writes into the in-flight payment: `kind`
    /// from `set_pay_kind`, `shape` from `set_pay_shape`, with the rent flag
    /// they carry. Last writer wins (the trigger payload holds the pending
    /// write; a hostfn enforces shape only before the first split leg).
    fn fold_pay_trigger(p: &mut Pay, t: &Trigger) {
        let Some(tp) = t.pay else {
            return;
        };
        p.kind = tp.kind;
        p.rent = tp.is_rent;
        p.shape = tp.shape;
    }

    fn fold_pay_payload(p: &mut Pay, t: &Trigger, rent: &mut bool, fixed: &mut bool) {
        Self::fold_pay_trigger(p, t);
        *rent = p.rent;
        *fixed = p.shape == PayShape::Fixed;
    }

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
        let mut p = p;
        if depth >= MAX_MONEY_DEPTH {
            // Safety cap hit -- the real termination argument (a hook cannot
            // re-trigger on its own movement) should make this unreachable.
            // Trap loudly rather than settling silently.
            panic!(
                "MAX_MONEY_DEPTH ({}) exceeded at depth {}: runaway money pipeline (pay={:?})",
                MAX_MONEY_DEPTH, depth, p
            );
        }
        // Q4 (PIPELINE-AUDIT P16/K17): a self-payment 「ApayA」 is **not** a
        // no-op. The counteraction windows run as normal (a counteraction may
        // redirect the payee) and the affordability / raise-funds / bankruptcy
        // path below applies exactly as to any other payment; only the net
        // balance effect is zero when the pair settles.
        if p.amount <= 0 {
            return Ok(Paid::default());
        }
        if p.from.is_some_and(|f| self.out(f)) || p.to.is_some_and(|t| self.out(t)) {
            return Ok(Paid::default());
        }
        // The payer's money cannot drop this turn (`money_locked`): the payment
        // still **completes** at loss 0, so a buy / build / acquisition goes
        // through for free instead of being treated as unpaid.
        // TODO(规则书)（打出此卡后，本回合内你的资金不会下降）: the book stops only
        // 你 from losing money -- whether the payee still receives it (and the
        // gain counters tick) is an open ruling (CROSS-TESTS S4); we waive the
        // whole transfer, so the payee gets nothing.
        if let Some(f) = p.from.filter(|&f| self.w.money_locked(f as i32)) {
            self.w.log(
                "text",
                f as i32,
                Msg::new("log.money_locked")
                    .player_id("who", f)
                    .n("amount", p.amount),
            );
            return Ok(Paid {
                paid: true,
                ..Paid::default()
            });
        }
        if p.kind != PayKind::ForceBuy {
            // The payer is gated by `can_pay` (the ruleset's pay gate).
            // A payee who is on the board but pay-gated cannot *receive*
            // (rulebook 49). An off-board payee is **not** blocked here:
            // 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」
            // (MyGO:无路矢) reroutes the payee from a `paySplitAdd` hook, which
            // runs below. The unredirected off-board payee is refused at
            // transfer time instead (see the final-payee gate).
            let gated_payee = p.to.filter(|&t| {
                self.w.gate_open(self.rules, super::Gate::OnBoard, t)
                    && !self.w.gate_open(self.rules, super::Gate::Pay, t)
            });
            if let Some(x) = p.from.filter(|&f| !self.can_pay(f)).or(gated_payee) {
                let text = Msg::new(if p.from.is_some() {
                    "log.blocked_pay"
                } else {
                    "log.blocked_gain"
                })
                .player_id("who", x)
                .msg("status", self.blocked(x))
                .n("amount", p.amount);
                self.w.log("text", x as i32, text);
                return Ok(Paid::default());
            }
        }
        // The amount runs PaySplitAdd -> PaySplitMul -> PaySplitChoose -> PaySplitAt -> the `pay`
        // counteraction window *before* any money moves or funds are raised.
        // `player_id` is the payer and `target` the payee (-1 = the bank); any
        // of them may rewrite the amount (`set_pay_amount`).
        let payer = p.from.map_or(-1, |f| f as i32);
        let side = p.from.or(p.to).expect("a payment has a side");
        // Declared rent, not the (possibly rewritten) kind: Mashiro keeps
        // 「同等的资金」 a rent while `kind` becomes `Loss`.
        let mut rent = p.rent;
        let total_stage = p.runs_total_stage;
        let mut amount = p.amount;
        let mut to = p.to;
        // `PayeeHalf` captures the declared payee before any response and lets
        // a responder rewrite `to`; it pays the captured partner half of what
        // the payer lost after this leg commits ([`Pay::partner`]).
        let mut partner = None;
        if p.shape == PayShape::PayeeHalf {
            partner = p.to;
        }
        let mut fixed = p.shape == PayShape::Fixed;
        // User ruling 2026-10-15 (OPEN-取整 family): money rounds up to a
        // multiple of 10 **once**, after every add/mul modifier has rewritten
        // the running figure. Modifiers transform exactly and must not CeilTo
        // inside their own body; this flag is what schedules the single
        // end-of-calc round. An amount no modifier touched remains exactly as the
        // base formula produced it (rent table, split share, printed X, ...).
        let mut modified = false;
        // ---- declaration ------------------------------------------------
        // The payment is declared with its **declared** amount, before any
        // modifier has touched it. That is the stable fact a counter listens
        // to: `vocal_too_hard`'s 「5000以上」 reads this, not whatever the
        // settlement has drifted to.
        //
        // `effect` is the counteraction chain. Counters may negate the payment
        // outright, spare a party, or reshape its amount and payee.
        //
        // It opens for **every** money movement -- print (game -> player),
        // delete (player -> game) and pay-player alike -- so any card-caused
        // payment is answerable, not just rent. `player_id` is the payer (-1
        // for a print); `target` is the payee (-1 for a delete).
        //
        // The `effect` counteraction window opens for every money movement at every
        // depth (the rulebook has no depth limit); the re-entrancy guard
        // (`PlayCtx::reentrant_hooks`) is the termination argument.
        if amount > 0 {
            let t = raise!(
                self,
                "effect",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = amount,
                by_card = p.by_card,
                @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                tile = p.tile.map_or(-1, |t| t as i32),
                effects = vec![super::rules::Effect {
                    kind: "pay".into(),
                    target: to.map_or(-1, |t| t as i32),
                    from: payer,
                    tile: p.tile.map_or(-1, |t| t as i32),
                    value: amount,
                }],
            )?;
            // Kind / shape writes land before the shape is read.
            Self::fold_pay_payload(&mut p, &t, &mut rent, &mut fixed);
            // The shape may have arrived in this window; `PayeeHalf` still
            // needs the declared payee as its half victim (the write may not
            // have moved `to`).
            if partner.is_none() && p.shape == PayShape::PayeeHalf {
                partner = to;
            }
            if !fixed && (t.is_cancelled() || !t.settles_for(side as i32)) {
                raise!(
                    self,
                    "payAfter",
                    side,
                    player_id = payer,
                    target = to.map_or(-1, |t| t as i32),
                    value = 0,
                    by_card = p.by_card,
                    @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                    settle_tile = self.settle_tile(),
                    tile = p.tile.map_or(-1, |t| t as i32)
                )?;
                return Ok(Paid::default());
            }
            // A `Fixed` leg still raises the window but ignores what a
            // responder wrote into the declared figure / payee, and never
            // cancels: the amount is the card's own.
            if !fixed {
                // Q3: adjustments compose freely -- the stages do **not** clamp and do
                // **not** stop at zero. A figure driven negative is still in flight;
                // later stages may bring it back, and only the post-chain sign decides
                // between settle / nothing / reverse.
                // The `effect` answer's rewrite (a 「支付减半」 sets the figure
                // here, 规则书 支付阶段 1) lands before the modifier stages.
                if t.value != amount {
                    modified = true;
                }
                amount = t.value;
                to = (t.target >= 0).then_some(t.target as usize);
                let in_settle = self.settle_open() > 0
                    && !matches!(
                        p.kind,
                        PayKind::Buy | PayKind::Build | PayKind::ForceBuy | PayKind::Gain
                    );
                // ---- add phase ----
                if rent {
                    self.apply_amount_stage(
                        "rentAdd",
                        Fold::Add,
                        side,
                        payer,
                        &mut to,
                        &mut amount,
                        &mut modified,
                        &mut p,
                        &mut rent,
                        &mut fixed,
                    )?;
                }
                if in_settle {
                    // The settle's `settleAdd` was composed **once** at the settle
                    // raise (`SettleStage::RaiseSettlePay`) and stored; every leg just
                    // takes the stored addend. Raising it per leg as well would count
                    // one responder twice.
                    let add = self.w.turn.settle_pay_add;
                    if add != 0 {
                        amount = amount.saturating_add(add);
                        modified = true;
                    }
                    // 规则书 twin: `settleRentAdd` is 「本次结算的[地租] +N」 -- a
                    // rent leg only, so a card leg of the same settle keeps its
                    // own figure (`docs/CARDS.md`).
                    if rent {
                        let radd = self.w.turn.settle_rent_add;
                        if radd != 0 {
                            amount = amount.saturating_add(radd);
                            modified = true;
                        }
                    }
                }
                if p.runs_total_stage && !fixed {
                    let mut to_slot = to;
                    let before_total = amount;
                    // An add-only run cannot cancel, so it always yields a figure.
                    let shaped = self
                        .pay_total_stages(
                            side,
                            payer,
                            &mut to_slot,
                            &mut p,
                            amount,
                            &["payCombinedAdd"],
                        )?
                        .unwrap_or(amount);
                    // The pre-split windows may have rewritten kind / rent / shape
                    // (`set_pay_kind` / `set_pay_shape`).
                    rent = p.rent;
                    fixed = p.shape == PayShape::Fixed;
                    if shaped != before_total {
                        modified = true;
                    }
                    amount = shaped;
                    to = to_slot;
                }
                if !fixed {
                    self.apply_amount_stage(
                        "paySplitAdd",
                        Fold::Add,
                        side,
                        payer,
                        &mut to,
                        &mut amount,
                        &mut modified,
                        &mut p,
                        &mut rent,
                        &mut fixed,
                    )?;
                }
                // ---- mul phase ----
                // A shape that turned `Fixed` at the add phase skips the rest of
                // the per-share chain (and the factors that ride it).
                if rent && !fixed {
                    self.apply_amount_stage(
                        "rentMul",
                        Fold::Mul,
                        side,
                        payer,
                        &mut to,
                        &mut amount,
                        &mut modified,
                        &mut p,
                        &mut rent,
                        &mut fixed,
                    )?;
                }
                let in_settle = self.settle_open() > 0
                    && !matches!(
                        p.kind,
                        PayKind::Buy | PayKind::Build | PayKind::ForceBuy | PayKind::Gain
                    );
                if in_settle && !fixed {
                    // Same for the settle's `settleMul`: composed once at the settle
                    // raise, applied to every leg as one factor of the leg's product.
                    let m = (self.w.turn.settle_pay_factor * 1_000_000.0).round() as i64;
                    amount = apply_milli_factor(amount, m, &mut modified);
                    // 规则书 twin: `settleRentMul` is 「本次结算的[地租] ×N」 -- a
                    // rent leg only, like the `rentMul` stage above.
                    if rent {
                        let r = (self.w.turn.settle_rent_factor * 1_000_000.0).round() as i64;
                        amount = apply_milli_factor(amount, r, &mut modified);
                    }
                }
                if p.runs_total_stage && !fixed {
                    let mut to_slot = to;
                    let before_total = amount;
                    match self.pay_total_stages(
                        side,
                        payer,
                        &mut to_slot,
                        &mut p,
                        amount,
                        &["payCombinedMul", "payCombinedCancel"],
                    )? {
                        // `payCombinedCancel` dropped the whole command.
                        None => {
                            raise!(
                                self,
                                "payAfter",
                                side,
                                player_id = payer,
                                target = to.map_or(-1, |t| t as i32),
                                value = 0,
                                by_card = p.by_card,
                                @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                                settle_tile = self.settle_tile(),
                                tile = p.tile.map_or(-1, |t| t as i32)
                            )?;
                            return Ok(Paid::default());
                        }
                        Some(shaped) => {
                            rent = p.rent;
                            fixed = p.shape == PayShape::Fixed;
                            if shaped != before_total {
                                modified = true;
                            }
                            amount = shaped;
                            to = to_slot;
                        }
                    }
                }
            }
            // A `Fixed` leg skips the per-share stages (and so the factors and
            // `milli_scale` that ride `paySplitMul`).
            let split_stages: &[&str] = if fixed {
                &[]
            } else {
                &["paySplitMul", "paySplitChoose", "paySplitAt"]
            };
            for &kind in split_stages {
                // v96: the writable prompt between the `paySplitAdd` fold and the
                // `paySplitMul` fold (规则书 支付阶段 3 「支付计算后」) -- it reads
                // the composed add figure and its write becomes the mul input.
                if kind == "paySplitMul" {
                    let ask = raise!(
                        self,
                        "paySplitMulBefore",
                        side,
                        player_id = payer,
                        target = to.map_or(-1, |t| t as i32),
                        value = amount,
                        by_card = p.by_card,
                        @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                        settle_tile = self.settle_tile(),
                        tile = p.tile.map_or(-1, |t| t as i32)
                    )?;
                    Self::fold_pay_payload(&mut p, &ask, &mut rent, &mut fixed);
                    if ask.value != amount {
                        modified = true;
                    }
                    amount = ask.value;
                    if ask.target >= 0 {
                        to = Some(ask.target as usize);
                    }
                }
                if kind == "paySplitChoose" {
                    let ask = raise!(
                        self,
                        "paySplitChooseBefore",
                        side,
                        player_id = payer,
                        target = to.map_or(-1, |t| t as i32),
                        value = amount,
                        by_card = p.by_card,
                        @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                        settle_tile = self.settle_tile(),
                        tile = p.tile.map_or(-1, |t| t as i32)
                    )?;
                    Self::fold_pay_payload(&mut p, &ask, &mut rent, &mut fixed);
                    if ask.value != amount {
                        modified = true;
                    }
                    amount = ask.value;
                    if ask.target >= 0 {
                        to = Some(ask.target as usize);
                    }
                }
                let before = amount;
                let t = raise!(
                    self,
                    kind,
                    side,
                    player_id = payer,
                    target = to.map_or(-1, |t| t as i32),
                    value = amount,
                    by_card = p.by_card,
                    @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                    settle_tile = self.settle_tile(),
                    tile = p.tile.map_or(-1, |t| t as i32)
                )?;
                Self::fold_pay_payload(&mut p, &t, &mut rent, &mut fixed);
                // V5 (`NEGATION-AUDIT`) at the settlement stages: `paySplitChoose`
                // is the Flag-shaped one -- a `false` Filter answer is folded to
                // `set_cancelled()` by `drive_flag`, and that **stops the payment**
                // (abi.rs `FilterKind::PaySplitChoose`: 「Body answers cancel: false
                // stops the payment」). 规则书 支付阶段 5 「取消支付 / 无需失去资金」
                // (G-MONEY-14, rulebook-doc.md:233) is exactly this row. The same
                // flag is honoured by the `effect` declaration and the `pay`
                // settlement window below.
                if kind == "paySplitChoose" && t.is_cancelled() {
                    raise!(
                        self,
                        "payAfter",
                        side,
                        player_id = payer,
                        target = to.map_or(-1, |t| t as i32),
                        value = 0,
                        by_card = p.by_card,
                        @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                        settle_tile = self.settle_tile(),
                        tile = p.tile.map_or(-1, |t| t as i32)
                    )?;
                    return Ok(Paid::default());
                }
                if t.value != before {
                    modified = true;
                }
                amount = t.value;
                if t.target >= 0 {
                    to = Some(t.target as usize);
                }
                // The 「支付」 scalars ride the mul phase *after* the card's own
                // stages (`docs/TILES.md`): the scale covers the payment as card
                // effects have shaped it, and never a purchase (rulebook 「此次购买
                // 的价格不受任何资金变动效果影响」), a forced purchase or a print.
                // It sits after `paySplitMul` and before the stage-5 windows.
                if kind == "paySplitMul" {
                    amount = self.scale_settle_payment(
                        amount,
                        p.kind,
                        rent,
                        p.tile,
                        p.milli_scale,
                        &mut modified,
                    );
                }
            }
        }
        // `pay` is a settlement hook now: what the payment actually was. It
        // runs after the chain, so it can no longer be used to reconstruct
        // 「被…效果影响」 -- that is `effect`'s job.
        //
        // V5 (`NEGATION-AUDIT`): the settlement window honours
        // `set_cancelled()` -- a counter body that cancels the `pay` link stops
        // the payment outright (it used to be ignored unless the body also
        // zeroed the figure).
        if amount != 0 {
            let before = amount;
            let t = raise!(
                self,
                "pay",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = amount,
                by_card = p.by_card,
                @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                settle_tile = self.settle_tile(),
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
            Self::fold_pay_payload(&mut p, &t, &mut rent, &mut fixed);
            if fixed {
                // Read-only: the leg keeps its figure and payee and cannot be
                // cancelled here.
            } else if t.is_cancelled() {
                amount = 0;
            } else {
                if t.value != before {
                    modified = true;
                }
                amount = t.value;
                to = (t.target >= 0).then_some(t.target as usize);
            }
        }
        // A `PayeeHalf` leg pays the partner directly; its own `to` is forced
        // off before affordability (the partner charge is the half leg below).
        if partner.is_some() {
            to = None;
        }
        // User ruling 2026-10-15: the one money round. After every add/mul
        // modifier has rewritten the running figure (and the settlement hook
        // has had its say), a **positive** figure some modifier touched is
        // rounded up to a multiple of 10 -- once, here. Untouched bases (rent
        // table, `ceil10(ceil(total/n))` split shares, printed X formulas) are
        // left as-is. A negative figure is not rounded: the reversal below
        // would otherwise see a CeilTo toward +inf (e.g. -7 → 0) and drop the
        // child payment.
        if !fixed && modified && amount > 0 {
            amount = ceil10_money(amount);
        }
        // Q3 (PIPELINE-AUDIT, user ruling 2026-10-07): adjustments compose
        // freely and there is no floor between stages. When the counteraction
        // chain has resolved and the **final** amount is negative, the original
        // payment is adjusted to 0 and a **new** payment starts with the
        // participants swapped and the absolute value. That child runs the
        // full pipeline (its own modifiers, counteraction windows, Q1
        // shortfall → mortgage → bankruptcy). Applies to one-sided movements
        // too: a negative 「gain」 (bank→A) becomes A losing |x| 「spend」, and
        // a negative 「spend」 becomes a gain. A child that is itself driven
        // negative reverses again, capped at [`MAX_PAY_REVERSALS`].
        if amount < 0 {
            // The original settles as 0.
            raise!(
                self,
                "payAfter",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = 0,
                by_card = p.by_card,
                @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                settle_tile = self.settle_tile(),
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
            if p.reverse_depth >= MAX_PAY_REVERSALS {
                self.w.log(
                    "text",
                    side as i32,
                    Msg::new("log.pay_reversal_capped")
                        .player_id("who", side)
                        .n("amount", amount),
                );
                return Ok(Paid::default());
            }
            let rev_amount = -amount;
            self.w.log(
                "text",
                side as i32,
                Msg::new("log.pay_reversed")
                    .player_id("who", side)
                    .n("amount", rev_amount),
            );
            let mut rev = Pay::new(rev_amount, p.kind);
            // Participants swapped. For a one-sided movement this flips the
            // bank leg: a negative 「gain」 becomes a 「spend」 and vice versa.
            rev.from = to;
            rev.to = p.from;
            rev.tile = p.tile;
            rev.must = true;
            rev.reason = p.reason.clone();
            rev.by_card = p.by_card;
            rev.runs_total_stage = true;
            rev.reverse_depth = p.reverse_depth + 1;
            let _ = self.money(rev)?;
            return Ok(Paid::default());
        }
        // Cancelled / zero: nothing moves and it is not paid (`payAfter` still
        // runs, with `p.paid == false`).
        //
        // Q4 (`PIPELINE-AUDIT` P16/K17): a self-payment 「ApayA」 is not a
        // no-op and is not short-circuited here. The counteraction windows above
        // have run in full (a counteraction may have redirected the payee), and
        // the affordability / raise-funds / bankruptcy path below applies
        // exactly as to any other payment. Only the **net balance effect** is
        // zero when the pair settles: the debit and the credit cancel.
        if amount == 0 {
            raise!(
                self,
                "payAfter",
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = 0,
                by_card = p.by_card,
                @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                settle_tile = self.settle_tile(),
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
            return Ok(Paid::default());
        }
        // [除外] 「无法收付款」 (rulebook 50): a final payee still on the off-board
        // gate cannot receive. A `set_pay_target` redirect (MyGO:无路矢's
        // 「[除外]期间本应获得的格子收入由此前指定的那名玩家获得」) has already
        // moved `to` off the off-board seat by now -- only an unredirected leg
        // lands here. Refuse it the same way the early payee gate does.
        // `ForceBuy` skips the status pay gate (same as the early gate).
        if p.kind != PayKind::ForceBuy {
            if let Some(t) = to
                .filter(|&t| !self.out(t) && !self.w.gate_open(self.rules, super::Gate::OnBoard, t))
            {
                let text = Msg::new(if p.from.is_some() {
                    "log.blocked_pay"
                } else {
                    "log.blocked_gain"
                })
                .player_id("who", t)
                .msg("status", self.blocked(t))
                .n("amount", amount);
                self.w.log("text", t as i32, text);
                raise!(
                    self,
                    "payAfter",
                    side,
                    player_id = payer,
                    target = t as i32,
                    value = 0,
                    by_card = p.by_card,
                    @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                    settle_tile = self.settle_tile(),
                    tile = p.tile.map_or(-1, |t| t as i32)
                )?;
                return Ok(Paid::default());
            }
        }
        let mut loss = if p.from.is_some() { amount } else { 0 };
        if let Some(f) = p.from {
            if self.w.st.players[f].money < loss {
                if p.must {
                    self.raise_funds(f, loss, to)?;
                    if self.out(f) {
                        // The payer could not complete a mandatory payment and
                        // went out -- the receipt's `SHORTFALL`.
                        return Ok(Paid {
                            shortfall: true,
                            ..Paid::default()
                        });
                    }
                } else if p.allow_mortgage {
                    // Fund the final amount after the payment hooks, without
                    // forcing an optional purchase or risking bankruptcy.
                    let exclude = if p.kind == PayKind::Build {
                        p.tile
                    } else {
                        None
                    };
                    if !self.offer_mortgage_funds(f, loss, exclude)? {
                        self.w.log(
                            "text",
                            f as i32,
                            Msg::new("log.purchase_unfunded")
                                .player_id("who", f)
                                .n("amount", loss),
                        );
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
                // 「当前回合内你每获得过一次资金」 -- one count per settled credit,
                // whatever the cause (rulebook 支付阶段 7 「资金变动」); the one
                // credit path is [`World::credit`].
                self.w.credit(t as i32, gain);
            }
        }
        self.log_money(&p, to, loss, gain);
        // 「本回合的[结算]向其他玩家支付了至少1000资金」 (RAS:游击演出) -- every
        // payment that leaves the payer for another player while a [触发结算]
        // is open counts, whatever its kind (a card's 「收费」, a forced buy,
        // the settle's own rent). A payment outside any settle does not.
        // A `PayeeHalf` main leg committed `to = None` but still paid a settle.
        if self.settle_open() > 0
            && loss > 0
            && p.from.is_some()
            && (p.to.is_some() || p.partner.is_some())
            && p.to != p.from
        {
            self.w.turn.paid_in_settle += gain;
            let f = p.from.unwrap_or(0);
            if let Some(n) = self.w.turn.paid_in_settle_by.get_mut(f) {
                *n += gain;
            }
        }
        // `payAfter` (hook) -- every settled payment, then the `paid` counteraction
        // window only when the payer lost money.
        let moved = if p.from.is_some() { loss } else { gain };
        raise!(
            self,
            "payAfter",
            side,
            player_id = payer,
            target = to.map_or(-1, |t| t as i32),
            value = moved,
            by_card = p.by_card,
            @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
            settle_tile = self.settle_tile(),
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
                @p TriggerPay::new(p.kind, rent, total_stage, p.shape),
                settle_tile = self.settle_tile(),
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
        }
        let paid = Paid {
            paid: true,
            loss,
            gain,
            shortfall: false,
        };
        // The `PayeeHalf` leg: the declared partner now *loses* half of what
        // the payer just lost (规则书 「令此次支付的对象失去此次金额一半的资金」
        // -- a delete, not a payment back), as its own `Fixed` pipeline one
        // depth down (so it logs, fires `paid` and counts in
        // `paid_in_settle_by`). No half when the main leg was cancelled, short,
        // or the payer is out.
        if let (Some(partner), Some(payer_ix)) = (partner, p.from) {
            if loss > 0 && !self.out(partner) && !self.out(payer_ix) {
                let mut half = Pay::new(loss / 2, PayKind::Card);
                half.from = Some(partner);
                half.to = None;
                half.shape = PayShape::Fixed;
                half.tile = p.tile;
                half.must = true;
                half.by_card = p.by_card;
                half.source = p.source;
                half.reason = p.reason.clone();
                half.reverse_depth = p.reverse_depth;
                let _ = self.money(half)?;
            }
        }
        Ok(paid)
    }

    /// The command-wide **pre-split** stages (`PIPELINE-AUDIT` Q2) on `total`,
    /// run in `kinds` order: `payCombinedAdd` ([`Fold::Add`]) /
    /// `payCombinedMul` ([`Fold::Mul`]) / `payCombinedCancel`, the last
    /// returning `None` when the whole command is dropped.
    ///
    /// The caller splits the run in two: the add stage belongs to the money
    /// pipeline's **add phase**, the mul and cancel stages to its **mul
    /// phase**, with the rent / settle / per-share stages between them
    /// (规则书 支付阶段 2 then 4).
    ///
    /// The figure is the 「分摊前」 total: a 「[分摊]pay2000」 divides the shaped
    /// result, not the raw 2000, into shares (规则书 支付阶段 2/4 applied to the
    /// command rather than to one pair). A pre-split hook may also redirect the
    /// payee; `to` is updated in place the same way the per-share stages do it.
    ///
    /// Q3: the stages do not clamp -- a total driven negative is returned as
    /// such so the caller can reverse after the chain resolves.
    #[allow(clippy::too_many_arguments)]
    fn pay_total_stages(
        &mut self,
        side: usize,
        payer: i32,
        to: &mut Option<usize>,
        p: &mut Pay,
        total: i32,
        kinds: &[&str],
    ) -> Flow<Option<i32>> {
        let total_stage = p.runs_total_stage;
        let mut amount = total;
        for &kind in kinds {
            // v78: interactive pre-stage (see the paySplitChooseBefore note
            // above). The ask's `set_pay_amount` / `set_pay_target` fold here
            // just like any other stage's rewrite.
            if kind == "payCombinedAdd" {
                let ask = raise!(
                    self,
                    "payCombinedAddBefore",
                    side,
                    player_id = payer,
                    target = to.map_or(-1, |t| t as i32),
                    value = amount,
                    by_card = p.by_card,
                    @p TriggerPay::new(p.kind, p.rent, total_stage, p.shape),
                    settle_tile = self.settle_tile(),
                    tile = p.tile.map_or(-1, |t| t as i32)
                )?;
                Self::fold_pay_trigger(p, &ask);
                amount = ask.value;
                if ask.target >= 0 {
                    *to = Some(ask.target as usize);
                }
            }
            let t = raise!(
                self,
                kind,
                side,
                player_id = payer,
                target = to.map_or(-1, |t| t as i32),
                value = amount,
                by_card = p.by_card,
                @p TriggerPay::new(p.kind, p.rent, total_stage, p.shape),
                settle_tile = self.settle_tile(),
                tile = p.tile.map_or(-1, |t| t as i32)
            )?;
            Self::fold_pay_trigger(p, &t);
            if kind == "payCombinedCancel" && t.is_cancelled() {
                return Ok(None);
            }
            amount = t.value;
            if t.target >= 0 {
                *to = Some(t.target as usize);
            }
        }
        Ok(Some(amount))
    }

    /// `PlayCtx::pay_total` -- the public face of [`Self::pay_total_stages`]. A card
    /// body that is about to 「[分摊]」 a figure calls this first, so the
    /// command-wide stage shapes the **total**, then divides the result and
    /// runs each share through [`Self::money`] with `Pay::total_stage = false`
    /// (`ctx::split_pay` / `ctx::transfer_leg`).
    ///
    /// Returns the shaped total, or `None` when the command was cancelled.
    pub fn pay_total(
        &mut self,
        from: Option<usize>,
        to: Option<usize>,
        total: i32,
        by_card: Option<i32>,
    ) -> Flow<Option<i32>> {
        if total <= 0 {
            return Ok(None);
        }
        if from.is_some_and(|f| self.out(f)) || to.is_some_and(|t| self.out(t)) {
            return Ok(None);
        }
        let mut to = to;
        let side = from.or(to).expect("a payment has a side");
        let payer = from.map_or(-1, |f| f as i32);
        let mut probe = Pay::new(total, PayKind::Card);
        probe.by_card = by_card;
        let shaped = self.pay_total_stages(
            side,
            payer,
            &mut to,
            &mut probe,
            total,
            &["payCombinedAdd", "payCombinedMul", "payCombinedCancel"],
        )?;
        // A cancelled command is `None`. A non-positive shaped total is nothing
        // to divide -- the per-leg `money()` calls (and their Q3 reversal) live
        // on the shares, not here.
        //
        // User ruling 2026-10-15: if a pre-split add/mul hook rewrote the total,
        // round it up to a multiple of 10 **here**, once, before 「[分摊]」
        // divides. An untouched total is the base figure and remains as-is (the
        // per-share formula still CeilTo-10s each share -- that share rounding
        // is the book's printed 「向上取整10」, not this rule).
        Ok(shaped
            .map(|s| if s != total { ceil10_money(s) } else { s })
            .filter(|&s| s > 0))
    }

    /// Emit the money log line for one settled payment.
    fn log_money(&mut self, p: &Pay, to: Option<usize>, loss: i32, gain: i32) {
        let typ = p.typ.unwrap_or(if p.kind == PayKind::Rent {
            PayEvent::Rent
        } else if p.kind == PayKind::Loss {
            PayEvent::Lose
        } else if p.from.is_some() && to.is_some() {
            PayEvent::Pay
        } else if p.from.is_some() {
            PayEvent::Lose
        } else {
            PayEvent::Gain
        });
        let amount = if p.from.is_some() { loss } else { gain };
        // The cause, in parentheses (`log.part.reason`). A card's own reason key
        // rides `reason` as a `Msg`; a board-driven one is the static `src.*`
        // key in `source`. Never the whole line: `text` is for that, and a bare
        // card name (「登上武道馆」) says nothing about who paid what.
        let reason = match &p.reason {
            Some(r) => Some(r.clone()),
            None if !p.source.is_empty() => Some(Msg::new(p.source)),
            _ => None,
        };
        let src = reason.map(|w| Msg::new("log.part.reason").msg("reason", w));
        let mut text = match &p.text {
            Some(t) => t.clone().n("amount", amount),
            None => {
                let m = match (p.from, to) {
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
        if let (Some(_), Some(t)) = (p.from, to) {
            if gain != loss {
                text = Msg::new("log.with_received")
                    .msg("base", text)
                    .player_id("who", t)
                    .n("gain", gain);
            }
        }
        let player_id = p.from.or(to).map_or(-1, |s| s as i32);
        let other = if p.from.is_some() {
            to.map_or(-1, |t| t as i32)
        } else {
            -1
        };
        let e = self.w.log(typ.as_str(), player_id, text);
        e.other = other;
        e.value = amount;
        if let Some(t) = p.tile {
            e.to = t as i32;
        }
    }

    /// Optional funding: selecting no mortgage cancels the purchase/build.
    /// Rulebook: the deed being upgraded cannot fund its own construction.
    fn offer_mortgage_funds(
        &mut self,
        i: usize,
        amount: i32,
        exclude: Option<usize>,
    ) -> Flow<bool> {
        let need = amount - self.w.st.players[i].money;
        let deeds: Vec<usize> = self
            .mortgageable(i)
            .into_iter()
            .filter(|&t| Some(t) != exclude)
            .collect();
        if deeds.iter().map(|&t| self.mortgage_value(t)).sum::<i32>() < need {
            return Ok(false);
        }
        let ai = self.auto_mortgage_from(i, need, deeds.clone());
        let text = Msg::new("ask.mortgage.optional")
            .n("amount", amount)
            .n("need", need);
        let mut ask = Ask::mortgage(i, need, text, &deeds, ai);
        // A mortgage prompt with a cancel option accepts value=1 to skip.
        ask.view.options = vec![Msg::new("ask.mortgage.skip")];
        ask.view.fallback = 1;
        let reply = self.ask(ask)?;
        if reply.of(i) == 1 {
            return Ok(false);
        }
        for id in &reply.answered.picked {
            if let Ok(t) = id.parse::<usize>() {
                if deeds.contains(&t) && self.w.st.owners[t] == i as i32 && !self.w.st.mortgaged[t]
                {
                    self.deed_mortgage(i, t, 2)?;
                }
            }
        }
        self.wait(0.6);
        Ok(!self.out(i) && self.w.st.players[i].money >= amount)
    }

    /// Mortgage deeds to cover `amount`, else go bankrupt.
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
        for id in &reply.answered.picked {
            if let Ok(t) = id.parse::<usize>() {
                if t < self.data.tiles.len()
                    && self.w.st.owners[t] == i as i32
                    && !self.w.st.mortgaged[t]
                {
                    self.deed_mortgage(i, t, 1)?;
                }
            }
        }
        while self.w.st.players[i].money < amount {
            // Chaos keeps the default order (its policy is not parameterised).
            let p = if self.is_chaos(i) {
                crate::strategy::StrategyParams::default()
            } else {
                self.strategy_of(i)
            };
            let Some(&t) = self.mortgage_order(self.mortgageable(i), &p).first() else {
                break;
            };
            let v = self.mortgage_value(t);
            self.deed_mortgage(i, t, 1)?;
            let _ = v;
        }
        self.wait(0.6);
        if self.w.st.players[i].money < amount && !self.out(i) {
            return self.bankrupt(i, creditor, amount);
        }
        Ok(())
    }

    /// Everything is cashed in and handed to the creditor.
    fn bankrupt(&mut self, i: usize, creditor: Option<usize>, amount: i32) -> Flow<()> {
        if self.out(i) {
            return Ok(());
        }
        // B3 (`PIPELINE-AUDIT` K2): mark the seat dead **before** the
        // `bankruptBefore` window, so the dying player's own effect sources
        // cannot join it. 规则书 L16 defines 破产 as the state entered when the
        // payment fails, L81 「所有其正在生效的卡，技能效果停止生效」 is what
        // entering that state does, and L17 excludes 破产 from [存活]. Other
        // players' `bankruptBefore` hooks still run and may inspect the
        // remaining state (K5) -- the cash-in below has not run yet.
        self.w.st.players[i].bankrupt = true;
        self.w.out_count += 1;
        self.w.st.players[i].out_order = self.w.out_count;
        // `bankruptBefore` -- before any asset cash-in; the post half below
        // runs after cash-in but before `remove_from_game`.
        raise!(self, "bankruptBefore", i, value = amount)?;
        if !self.playing() {
            return Ok(());
        }
        let mut cashed = 0;
        for t in self.mortgageable(i) {
            self.w.st.mortgaged[t] = true;
            cashed += self.mortgage_value(t);
        }
        self.w.credit(i as i32, cashed);
        let all = self.w.st.players[i].money.max(0);
        self.w.st.players[i].money = 0;
        let creditor = creditor.filter(|&c| !self.out(c));
        if let Some(c) = creditor {
            self.w.credit(c as i32, all);
        }
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
        // `beforeOut` (hook) -- before the player is cleared.
        raise!(self, "beforeOut", i)?;
        let deeds = self.remove_from_game(i)?;
        // `bankruptResolved` (Q6 / K11) -- the terminal: cash-in done, the seat
        // cleared, the leftover auctions finished. Nothing else observes
        // "cleanup + auctions finished", so this is the point a 「结算完成时」
        // card would hook.
        let ended = self.check_game_over();
        if !ended {
            self.auction_leftovers(deeds)?;
        }
        raise!(self, "bankruptResolved", i)?;
        if ended {
            return Err(Halt::ended());
        }
        Ok(())
    }

    /// Leaving mid-match counts as going out.
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
        // `beforeOut` (hook) -- before the player is cleared.
        raise!(self, "beforeOut", i)?;
        let deeds = self.remove_from_game(i)?;
        // `leaveAfter` -- after the player is cleared out, but before the
        // game-over check so it still runs when leaving ends the match.
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

    /// Clear the player; its land returns to the bank.
    fn remove_from_game(&mut self, i: usize) -> Flow<Vec<usize>> {
        // Collect the rule ids this seat's field carries **before** the clear
        // below: markers those rules own leave the game with them (user ruling
        // 2026-10-07), wherever the copies sit.
        let owned_rules: Vec<String> = self.w.st.players[i]
            .field
            .iter()
            .map(|f| f.card.clone())
            .collect();
        // Every bound counter unit dies with its owning card instance (user
        // ruling 2026-10-10). Collect the uids before the field is cleared.
        let owned_uids: Vec<i32> = self.w.st.players[i].field.iter().map(|f| f.uid).collect();
        let s = &mut self.w.st.players[i];
        s.ai = true;
        s.money = 0;
        // Bankruptcy clears **everything the player holds** (规则书 L81) --
        // including ruleset-invented state keys (PP fans live here). One
        // generic sweep; the engine names no keys.
        for v in s.state.values_mut() {
            v.value = 0;
        }
        // B2 (`PIPELINE-AUDIT` K6/K14) -- 规则书 L81: 「将其控制的所有棋子，persona卡，
        // group卡，和手卡移出游戏。所有其正在生效的卡，技能效果停止生效。」 The
        // field holds the persona cards, group cards and every 「[持续]」 card
        // (including the bound `skill:*` instances). Both leave the game with
        // the seat, so no hook of theirs can run again. (The hook dispatch also
        // skips `out()` seats -- belt and
        // braces for any instance a later path re-places.)
        //
        // Markers (user ruling 2026-10-07, reversing the previous batch's
        // keep-tokens deviation): bankruptcy clears **everything the player
        // holds** -- spendable resources / named counters / tokens / statuses -- plus every
        // marker **owned by one of their rules** wherever that copy sits
        // (another player's counter, a tile). Neutral board marks ([tile marks],
        // `owner == -1`) are not player-held and remain. `pp::
        // shanyao_counter_on_short_payment` pins the fan being cleared.
        s.tokens.clear();
        s.field.clear();
        self.w.hidden[i].hand.clear();
        self.w.extra_turns.retain(|&x| x != i);
        // One cleanup path keyed by instance (user ruling 2026-10-10): every
        // bound counter unit dies with its owning card instance. The standing
        // the tile-mark owner pseudo card is never on a player field and is never
        // destroyed, so its tile [tile marks] remain. Legacy rows (`instance == -1`)
        // still go through the name-keyed purge below.
        for uid in owned_uids {
            self.w.destroy_instance_units(uid);
        }
        self.purge_markers_owned_by(&owned_rules);
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
        // Raise the purge's mark removals even though no further run commits
        // for this player: the `markChanged` drain needs a committing frame.
        self.commit_marks_delta()?;
        Ok(deeds)
    }

    /// Drain the pending mark log into `markChanged` raises. Called by
    /// `commit_after` and by paths that clear marks with no later run
    /// (`remove_from_game`).
    fn commit_marks_delta(&mut self) -> Flow<()> {
        let rows = std::mem::take(&mut self.w.mark_log);
        for row in rows {
            let mut t = Trigger::new("markChanged", row.owner as usize);
            t.tile = row.tile;
            t.name = Some(row.kind);
            t.value = row.delta;
            t.card = row.card;
            t.instance = row.instance;
            self.raise(t)?;
        }
        Ok(())
    }

    /// User ruling 2026-10-07 (marker ownership): drop every marker whose
    /// owning rule is in `rules`, wherever its copies sit -- every player's
    /// counter, and every tile mark of that kind. Neutral board marks
    /// ([tile marks], `owner == -1`) are never player-rule-owned and remain.
    fn purge_markers_owned_by(&mut self, rules: &[String]) {
        if rules.is_empty() {
            return;
        }
        let doomed: Vec<String> = self
            .w
            .marker_owner
            .iter()
            .filter(|(_, owner)| rules.iter().any(|r| r == *owner))
            .map(|(name, _)| name.clone())
            .collect();
        for name in doomed {
            for p in self.w.st.players.iter_mut() {
                p.tokens.retain(|t| t.name != name);
            }
            let gone: Vec<crate::state::TileMark> = self
                .w
                .st
                .marks
                .iter()
                .filter(|m| m.owner != crate::state::BOARD_OWNER && m.kind == name)
                .cloned()
                .collect();
            for m in &gone {
                self.w.log_mark_row(m, -m.count);
            }
            self.w
                .st
                .marks
                .retain(|m| m.owner == crate::state::BOARD_OWNER || m.kind != name);
        }
    }

    /// Auction up to 3 random deeds of a player who went out.
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

    /// Auction one deed among the players still in.
    fn auction_tile(&mut self, t: usize, from: usize) -> Flow<()> {
        // `BuyGate` filters bidders (`docs/PURCHASE.md` P4): a player the gate
        // refuses cannot bid.
        let mut players: Vec<usize> = Vec::new();
        for s in self.present_from(from) {
            if !self.can_pay(s) {
                continue;
            }
            let gate = raise!(
                self,
                "buyGate",
                s,
                tile = t as i32,
                @b TriggerBuy::new(purchase::BuyKind::Auction, -1, 0)
            )?;
            if !gate.is_cancelled() {
                players.push(s);
            }
        }
        if players.is_empty() {
            self.w
                .log("text", -1, Msg::new("log.auction_nobody").tile("tile", t));
            return Ok(());
        }
        // The quote base sets AI worth (`docs/PURCHASE.md` P4).
        let base = purchase::quote_native(self.data, &self.w.st, t).max(0);
        // Per-seat ceiling, from the seat's own policy (standard randomises
        // around the price; chaos spends down to `CHAOS_RESERVE`).
        let mut worth: Vec<i32> = Vec::with_capacity(players.len());
        for &s in &players {
            worth.push(self.ai_auction_worth(s, base));
        }
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
            // Bankruptcy leftover auction (`auction_leftovers`) is the only
            // caller today and runs on its own dedicated timer -- never a
            // seat pool. A future ordinary auction would pass `use_pool: true`
            // and a pool-backed window instead.
            self.w.clock.bankrupt_auction_time,
            false,
        );
        let r = self.ask(ask)?;
        if r.answered.bidder < 0 {
            self.w
                .log("text", -1, Msg::new("log.auction_no_bid").tile("tile", t));
            return Ok(());
        }
        let (b, bid) = (r.answered.bidder as usize, r.answered.bid);
        if self.w.st.owners[t] >= 0 || !self.can_pay(b) {
            self.w.log(
                "text",
                b as i32,
                Msg::new("log.auction_void").player_id("who", b),
            );
            return Ok(());
        }
        // `spendBefore` -- the raw-spend window runs before affordability, so a
        // reduction can prevent a shortfall. Only the affordability test and
        // the debit use the shaped figure; the bid itself (and the deal)
        // stays the bid.
        let cost = self.spend_before(b, t as i32, bid, None)?;
        // B4 (`PIPELINE-AUDIT` K10b) -- 规则书 L76: 「pay或spend资金且资金不足
        // 时可以选择抵押拥有的地契」. A short bidder is offered the mortgage path
        // to fund the bid; only a bidder who cannot raise it (and therefore goes
        // out) voids the auction. L20 「spend同等资金」 keeps the bid itself a
        // direct delete (ruling 7, TODO(规则书) ruling 7).
        if self.w.st.players[b].money < cost {
            self.raise_funds(b, cost, None)?;
            if self.out(b) || self.w.st.players[b].money < cost {
                self.w.log(
                    "text",
                    b as i32,
                    Msg::new("log.auction_void").player_id("who", b),
                );
                return Ok(());
            }
        }
        // Money remains direct until ruling 7 (TODO(规则书) ruling 7).
        self.w.st.players[b].money -= cost;
        // The deal: `BuyAssign` rewrites owner / houses / mortgage before the
        // one ownership write (`docs/PURCHASE.md`).
        let mut deal = purchase::default_deal(
            self.data,
            &self.w.st,
            t,
            b,
            purchase::BuyKind::Auction,
            bid,
            true,
        );
        let at = raise!(
            self,
            "buyAssign",
            b,
            tile = t as i32,
            @b TriggerBuy::new(purchase::BuyKind::Auction, -1, bid).deal(
                Payee::from_i32(deal.owner_after),
                deal.houses_after,
                deal.mortgaged_after,
            )
        )?;
        if let Some(b) = at.buy {
            if let Some(o) = b.deal_owner {
                deal.owner_after = o.as_i32();
            }
            if let Some(h) = b.deal_houses {
                deal.houses_after = h;
            }
            deal.mortgaged_after = b.deal_mortgaged;
        }
        purchase::assign_deed(
            &mut self.w,
            t,
            deal.owner_after,
            deal.houses_after,
            deal.mortgaged_after,
        );
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
        // `bought` (hook) -- the auction winner is set as the owner.
        raise!(
            self,
            "bought",
            b,
            tile = t as i32,
            @b TriggerBuy::new(purchase::BuyKind::Auction, -1, bid)
        )?;
        self.wait(1.0);
        Ok(())
    }

    // =============================================================== cards

    /// The player's own `handLimit` state (cards
    /// can raise or cut it), not the base constant.
    pub fn over_hand(&self, i: usize) -> bool {
        self.w.st.players[i].over_hand(self.w.hidden[i].hand.len())
    }

    /// From the top of the pile; reshuffle the discard pile when empty.
    /// Bots discard down to the hand limit at random.
    ///
    /// `log` is also "raise the per-draw points": the opening deal passes
    /// `false` (see [`Self::opening`]); every draw during play passes `true`.
    pub(crate) fn draw(&mut self, i: usize, n: usize, log: bool) -> Flow<()> {
        self.draw_cards_with_hooks(i, n, log)?;
        Ok(())
    }

    /// Restore the draw pile and run its hook before any later after-effect.
    pub fn refill_draw_pile(&mut self, i: usize, hooks: bool) -> Flow<bool> {
        let refilled = self.w.refill_draw_pile(i);
        if refilled && hooks {
            raise!(self, "reshuffled", i)?;
        }
        Ok(refilled)
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
    ///   and the after points run for it. A cancelled draw with nothing added
    ///   is skipped outright.
    /// * `drawn` -- after it is in hand, on the **drawn card itself**.
    /// * `drew` -- after it is in hand, on the **field cards (per-draw effects**:
    ///   梦在前方's named counter per draw, 若宫伊芙's exclusive, ...).
    ///
    /// An N-card draw is N iterations; each payload names one card. Both
    /// card-driven draws (`HostRequest::Draw` -> here) and engine draws (the start square
    /// reward, turn draws) go through this. Opening hands pass `hooks = false`
    /// and raise none of the three.
    pub fn draw_cards_with_hooks(&mut self, i: usize, n: usize, hooks: bool) -> Flow<i32> {
        if self.out(i) {
            return Ok(0);
        }
        let mut got = 0;
        for _ in 0..n {
            self.refill_draw_pile(i, hooks)?;
            let top = self.w.hidden[i].deck.last().cloned().unwrap_or_default();
            // `drewBefore` -- the per-card replacement point.
            let mut replacement: Option<String> = None;
            let mut skipped = false;
            if hooks {
                let hand_before = self.w.hidden[i].hand.len();
                let t = raise!(self, "drewBefore", i, card = Some(top), value = 1)?;
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
                let card = self.w.hidden[i].deck.pop();
                if let Some(c) = &card {
                    self.w.hidden[i].hand.push(c.clone());
                }
                card
            };
            let Some(card) = card else {
                self.refill_draw_pile(i, hooks)?;
                continue;
            };
            got += 1;
            // Empty-deck maintenance has priority over the card's own Drawn,
            // field Drew hooks, and any prompts those effects may open.
            self.refill_draw_pile(i, hooks)?;
            if hooks {
                // `drawn` -- the drawn card's own hook.
                raise!(self, "drawn", i, card = Some(card.clone()), value = 1)?;
                // `drew` -- the field-card per-draw point (one raise per single
                // card). `cards` names the one card too, so a hook written
                // against an older batch payload still reads it.
                raise!(
                    self,
                    "drew",
                    i,
                    card = Some(card.clone()),
                    value = 1,
                    cards = vec![card]
                )?;
            }
        }
        if hooks && got > 0 {
            let over = self.over_hand(i).then(|| {
                Msg::new("log.part.over_hand").i(
                    "limit",
                    i64::from(self.w.st.players[i].hand_limit().unwrap_or_default()),
                )
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
        // 「任何时刻若持有手卡超过上限则需要立刻将手卡弃至上限张」 -- discard
        // down immediately, bot and human alike (the human `discard` command
        // remains for voluntary / other overflow paths).
        if self.playing() {
            while self.over_hand(i) {
                let hand_len = self.w.hidden[i].hand.len();
                let k = self.w.rng.below(hand_len);
                let card = self.w.hidden[i].hand[k].clone();
                self.discard(i, &card)?;
            }
        }
        Ok(got)
    }

    /// Discard down to the hand limit.
    pub(crate) fn discard(&mut self, i: usize, card: &str) -> Flow<()> {
        // `discardBefore` -- before the card leaves the hand.
        raise!(self, "discardBefore", i, card = Some(card.to_string()))?;
        let h = &mut self.w.hidden[i];
        if let Some(k) = h.hand.iter().position(|c| c == card) {
            h.hand.remove(k);
        }
        h.graveyard.push(card.to_string());
        let limit = i64::from(self.w.st.players[i].hand_limit().unwrap_or_default());
        self.w
            .log(
                "discard",
                i as i32,
                Msg::new("log.discard")
                    .player_id("who", i)
                    .i("limit", limit)
                    .card("card", card),
            )
            .card = card.to_string();
        self.refill_draw_pile(i, true)?;
        // `discarded` (hook) -- on the filed card, which may already have been
        // shuffled into its owner's newly refilled deck.
        raise!(self, "discarded", i, card = Some(card.to_string()))?;
        // `discardAfter` -- after filing and immediate deck maintenance.
        raise!(self, "discardAfter", i, card = Some(card.to_string()))?;
        Ok(())
    }

    /// Capability gates the ruleset owns. Each closed gate yields the
    /// ruleset's own refusal `Msg` (its `gate_msgs` key as data). A card the
    /// ruleset exempts from the act gate (its declared `exemption_prop`)
    /// skips that one check.
    fn cannot_play(&self, i: usize, id: &str) -> Option<Msg> {
        if !self.w.gate_open(self.rules, super::Gate::OnBoard, i) {
            return Some(self.rules.gate_refusal_msg(super::Gate::OnBoard, i));
        }
        if !self.w.gate_open(self.rules, super::Gate::Act, i) && !self.rules.card_act_exemption(id)
        {
            return Some(self.rules.gate_refusal_msg(super::Gate::Act, i));
        }
        if !self.w.gate_open(self.rules, super::Gate::Play, i) {
            // A standing play lock. Play-only: the turn still runs.
            return Some(self.rules.gate_refusal_msg(super::Gate::Play, i));
        }
        None
    }

    /// The exclusive-use check (`docs/GUARDS.md` §ExclusiveUse). The generic
    /// rule is the engine's own: an exclusive is playable iff `card.owner` is
    /// the actor's persona or its base (`variant_of`). A group-skill card may
    /// then close the CEL-only `ExclusiveUse` gate to deny the base case
    /// (CRYCHIC). Memoized per `(player, card)` under the world stamp, like
    /// [`Self::rules_cant_play`] -- the gate is a ruleset query, not a field
    /// read.
    fn exclusive_play(&self, i: usize, id: &str, card: &crate::data::CardData) -> Option<Msg> {
        let persona = &self.w.st.players[i].persona;
        let base = self.data.base_persona(persona);
        let owner_is_self = card.owner == *persona;
        let owner_is_base = card.owner == base;
        if !owner_is_self && !owner_is_base {
            return Some(Msg::new("err.play_exclusive").persona("owner", card.owner.clone()));
        }
        let ask = super::ExclusiveAsk {
            card: id,
            owner: &card.owner,
            owner_is_self,
            owner_is_base,
        };
        // Group-skill denial (`ExclusiveUse` gates), memoized under the world
        // stamp like `rules_cant_play`.
        if self.exclusive_gate_denied(i, id, &ask) {
            return Some(Msg::new("err.play_exclusive").persona("owner", card.owner.clone()));
        }
        None
    }

    /// Why this card cannot be played from hand, or `None` when it can.
    pub fn cant_play(&self, i: usize, id: &str, asking: bool) -> Option<Msg> {
        if !self.w.hidden[i].hand.iter().any(|c| c == id) {
            return Some(Msg::new("err.no_such_card"));
        }
        if !(self.playing() && self.w.st.turn == i as i32) || self.w.st.step != stage::OPS || asking
        {
            return Some(Msg::new("err.play_phase"));
        }
        if let Some(reason) = self.cannot_play(i, id) {
            return Some(reason);
        }
        if let Some(card) = self.data.card(id) {
            if card.exclusive() {
                if let Some(reason) = self.exclusive_play(i, id, card) {
                    return Some(reason);
                }
            }
        }
        if !self.rules.normal(id) {
            return Some(Msg::new("err.play_timing"));
        }
        self.rules_cant_play(i, id)
    }

    /// Play a card from the hand.
    pub(crate) fn play_from_hand(&mut self, i: usize, id: &str) -> Flow<()> {
        let Some(k) = self.w.hidden[i].hand.iter().position(|c| c == id) else {
            return Ok(());
        };
        // Activation argument (docs/ACTIVATION.md §2) -- collected **before**
        // `hand.remove`, the point of no return. A decline leaves the card in
        // hand and the play never announces (LOY-SEEYOU-01's "do not commit
        // the play until …").
        let arg = match self.rules.declare_activation(self, i, id)? {
            Some(a) => a,
            None => return Ok(()),
        };
        self.w.turn.play_arg = arg;
        self.w.hidden[i].hand.remove(k);
        // `play_from_hand` -- this one came out of the hand. **Scoped to the
        // play**: raised for the duration of this resolution and restored when
        // it ends, so a later play (or a skill press, which is not a hand press
        // at all) never sees a stale `true`. A nested `ctx::play_card` still
        // clears it for the card it runs and restores the outer value on the
        // way out. A halt mid-play leaves it set -- the play is still resolving
        // -- and the routine's re-run from its snapshot re-arms and restores it.
        let prev_from_hand = self.w.turn.play_from_hand;
        self.w.turn.play_from_hand = true;
        // `extreme` -- same scope as `play_from_hand`: a counteraction may arm a
        // forced extreme mid-play (「以理论最大值或最小值结算」), and it must
        // not leak into the next play. A nested `ctx::play_card` starts plain
        // and restores the outer value on the way out. A halt mid-play leaves
        // it armed -- the play is still resolving -- and the replay re-derives
        // it from the snapshot.
        let prev_extreme = self.w.turn.extreme;
        self.w.turn.extreme = Extreme::Plain;
        // The card whose own effect the placement flag is scoped to; same
        // lifetime as `play_from_hand` (design G06 §2.15).
        let prev_play_card = self.w.turn.play_card.take();
        self.w.turn.play_card = Some(id.to_string());
        // Per-pair designations cancelled on the previous play must not leak
        // into this one. Scoped to the play (not the command): the cancels are
        // written by a counteraction in the `card` window below, which is its
        // own command and has already left by the time the body runs.
        self.w.turn.cancelled_designations.clear();
        // `play_doubled` is scoped to one play.
        self.w.turn.play_doubled = None;
        // Did this play place anything? (the copy-source record, G06 §5.6)
        self.w.turn.placed_during_play = false;
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
        // 「当有人同一回合内打出两张卡时」 (TriggerKind::TwoCards) -- raised on
        // the play that makes it two hand-card plays this turn (any seat; the
        // counter lives on the turn state). Skills do not count (they do not
        // go through `play_from_hand`).
        self.w.turn.cards_played = self.w.turn.cards_played.saturating_add(1);
        let two_cards = self.w.turn.cards_played == 2;
        self.wait(0.9);
        let t = raise!(
            self,
            "card",
            i,
            card = Some(id.to_string()),
            cards = vec![id.to_string()],
            by_card = Some(i as i32)
        )?;
        if two_cards {
            // A separate counteraction timing from the play's own `card` window: Here
            // the world answers 「当有人同一回合内打出两张卡时」, not the play.
            raise!(
                self,
                "twoCards",
                i,
                card = Some(id.to_string()),
                cards = vec![id.to_string()],
                by_card = Some(i as i32)
            )?;
        }
        let rules = self.rules;
        // A cancelled trigger (`is_cancelled`) -- the play is negated; the card still goes to its
        // Dest below but its effect body does not run. The card still flashes,
        // marked negated. A play that goes through announces itself at its body
        // (`drive_inner_body`), which is the point the effect actually runs.
        let dest = if t.is_cancelled() {
            self.card_activated(
                crate::state::card_trigger::PLAY,
                i as i32,
                id,
                t.target,
                t.tile,
                true,
            );
            Dest::Graveyard
        } else {
            rules.play(self, i, id)?
        };
        match dest {
            Dest::Graveyard => {
                if !self.out(i) {
                    self.w.hidden[i].graveyard.push(id.to_string());
                    self.refill_draw_pile(i, true)?;
                    // `discarded` -- before the `cardPlayed` announcement.
                    raise!(self, "discarded", i, card = Some(id.to_string()))?;
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
        // landed wherever its `Dest` sent it. The played id rides `cards` too
        // (the `card` / `skillUsed` pattern), so a residual can read
        // `trigger::cards()` and match the id's band prefix.
        raise!(
            self,
            "cardAfter",
            i,
            card = Some(id.to_string()),
            cards = vec![id.to_string()],
            by_card = Some(i as i32)
        )?;
        // `cardPlayed` (hook) -- a card's hand effect resolved (its `Dest` is final).
        raise!(
            self,
            "cardPlayed",
            i,
            card = Some(id.to_string()),
            cards = vec![id.to_string()],
            by_card = Some(i as i32)
        )?;
        // The completed play joins the copy-source log 「上一张卡」 (G06
        // §2.15). Not cleared at a turn start; capped by `MAX_PLAY_LOG`.
        let placed = self.w.turn.placed_during_play;
        self.w.record_play(id, i as i32, placed);
        self.w.turn.placed_during_play = false;
        // The play is over: hand back the origin flag (`play_from_hand` is
        // scoped to the play, see the top of this routine) and the forced
        // extreme (`extreme`, same scope).
        self.w.turn.play_from_hand = prev_from_hand;
        self.w.turn.extreme = prev_extreme;
        self.w.turn.play_card = prev_play_card;
        Ok(())
    }

    // =============================================================== events

    /// Every non-derived event, shuffled.
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

    /// Draw the top event and settles it.
    ///
    /// Also the `ctx::draw_event` primitive (`docs/TILES.md`): `tile:event`'s
    /// body is 「抽取一张手卡，然后抽取一个事件卡」 and the event half is this.
    ///
    /// The engine keeps only the generic mechanics (`docs/EVENTS.md`): the
    /// deck, the draw, the public reveal, the **active list** and the filing
    /// away. What the event *does* is its rule instance on the neutral board
    /// owner -- bound here before the body runs, unbound when it expires.
    pub fn card_draw_event(&mut self, i: usize) -> Flow<()> {
        self.draw_event(i)
    }

    fn draw_event(&mut self, i: usize) -> Flow<()> {
        // Fallback for a deck left empty outside a draw (an expiry or a raw
        // state write since the last one).
        self.w.refill_event_deck();
        let Some(id) = self.w.event_deck.pop() else {
            self.w.log("text", i as i32, Msg::new("log.no_events"));
            return Ok(());
        };
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
        // Face-down slot on the public `eventTop` view: consumed by this draw.
        self.w.st.event_top.retain(|e| e != &id);
        let t = raise!(self, "event", i, card = Some(id.clone()))?;
        let rules = self.rules;
        // A cancelled trigger (`is_cancelled`) -- the event's effect is negated; it is still
        // filed away below but does not settles.
        //
        // Bind the rule instance first so the body's `On::Play` and any hook it
        // declares run against a live instance. `bind_event` is a no-op for
        // `StubRules` (no `event:*` rule), which is the built-in fallback: the
        // draw is logged and the card filed away, the same as before.
        // Bind the rule instance first so the body's `On::Play` and any hook it
        // declares run against a live instance. `bind_event` is a no-op for
        // `StubRules` (no `event:*` rule), which is the built-in fallback: the
        // draw is logged and the card filed away, the same as before. A
        // negated draw binds nothing -- its effect never resolves. The event
        // still flashes, marked negated. A draw that goes through announces
        // itself at its body (`drive_inner_body`).
        let placed = if t.is_cancelled() {
            self.card_activated(
                crate::state::card_trigger::EVENT,
                i as i32,
                &id,
                t.target,
                t.tile,
                true,
            );
            false
        } else {
            self.w.bind_event(self.data, rules, i as i32, &id);
            rules.event(self, i, &id)?
        };
        // A body that expired itself mid-Play (「放入事件弃牌」 / 「永久移除」
        // through `ctx::event_expire`) has already filed the card away. A
        // one-shot, a negated draw and the `StubRules` fallback are filed here.
        // Derived events are 「永久移除」 rather than recycled.
        let filed = self.w.event_discard.iter().any(|e| e == &id)
            || self.w.event_removed.iter().any(|e| e == &id);
        if !placed && !filed {
            self.w
                .expire_event(&id, self.data.event(&id).is_some_and(|e| e.derived));
        }
        // `eventAfter` -- the event is fully resolved and filed away.
        raise!(self, "eventAfter", i, card = Some(id))?;
        // Everything has resolved and the card is filed: only now does an
        // emptied deck take the shuffled discard (this card included).
        self.w.refill_event_deck();
        self.wait(0.4);
        Ok(())
    }

    /// Counteraction window, delegated to the card rules.
    ///
    /// Every raise site funnels through here (via [`raise!`] / [`Self::raise`]),
    /// so `step` is stamped centrally from the live turn step rather than at
    /// each call site.
    fn counteract(&mut self, t: &mut Trigger) -> Flow<()> {
        t.step = self.w.st.step;
        let rules = self.rules;
        let mut r = rules.counteract(self, t);
        // STACK-01: a counter body that parked work (a join window, a nested
        // settle) suspends the walk. The routine's Rust stack is still live
        // here, so pump the parked work and re-enter the walk instead of
        // unwinding past the body this raise belongs to -- the same contract
        // `raise_core` honours for guest raises.
        let mut pumps = 0u32;
        while r.as_ref().err().is_some_and(|e| e.is_suspended())
            && !self.is_draining_work()
            && pumps < 64
        {
            self.drain_work()?;
            pumps += 1;
            r = rules.counteract(self, t);
        }
        r
    }

    /// Build-and-raise a [`Trigger`], returning it as [`Self::counteract`] left it so
    /// a counteraction's rewrite of a field (e.g. `diceRollResult`'s reroll) can be read
    /// back. Prefer the [`raise!`] macro over calling this directly.
    pub(crate) fn raise(&mut self, mut t: Trigger) -> Flow<Trigger> {
        self.counteract(&mut t)?;
        Ok(t)
    }

    // =============================================================== end

    /// One survivor, or every human out.
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

    /// Score, rank, and end the match.
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
        let reason = match reason {
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
                .msg("reason", reason)
                .player_id("winner", winner),
        );
    }
}

#[cfg(test)]
mod tests;
