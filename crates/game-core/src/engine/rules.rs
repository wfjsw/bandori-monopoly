//! The boundary where card content plugs into the match shell.
//!
//! The shell calls these hooks at the points the rulebook's turn and effect
//! structure needs them; an implementation decides what cards and events
//! actually do. `game-rules` will implement this over the WASM card modules.
//! [`StubRules`] gives every card and event no effect, which leaves a complete,
//! playable game of plain Monopoly.

use super::cx::{Flow, PlayCtx};
use crate::msg::Msg;

/// Where a played card goes afterwards.
///
/// The fates: Graveyard (discard pile, 弃卡区), Hand, Field (remains in play,
/// 场上), Banished (「banish」, out of the game).
/// Planned: the draw-pile fates `DeckTop` / `DeckBottom` / `DeckRandom` (put
/// the card on top of / on the bottom of / shuffle it into the draw pile; the
/// default is **shuffle**).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dest {
    /// The discard pile (弃卡区) -- the default.
    #[default]
    Graveyard,
    /// Back to the hand (手牌).
    Hand,
    /// Remains in play (场上) -- a persistent effect.
    Field,
    /// 「banish」 -- out of the game entirely.
    Banished,
}

// `MoveFlags` is gone: a move is exactly one [`super::move_ctx::MoveKind`] and
// what it resolves is `super::move_ctx::Settle` -- two categories, not one flag
// soup. Card-owned move state (a marker-funded roll, say) rides on the move's tags.

/// Which way a move travels along the board loop (turn order).
///
/// Lives in `game-kinds` -- one definition shared with the guest. Re-exported
/// here so engine call sites keep their path.
pub use game_kinds::Dir;

/// The move that caused a trigger. [`Trigger::mv`] is `None` when no
/// move caused it -- a play, a turn-flow raise, a buy, a card's own activation.
///
/// Grouping these under one optional struct is what makes "no move" a single
/// state instead of a pile of individually-defaulted fields that can disagree
/// with each other (`move_dir: 1, move_from: -1` with `move_kind: None`).
/// The guest ABI and CEL still see the old flat encoding with its sentinels
/// (`dir` = 1, `from` = -1, `kind`/`roll` = null) whenever `mv` is `None`;
/// only the host type is grouped.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerMove {
    /// How the move got there: one walk or one teleport.
    pub kind: super::move_ctx::MoveKind,
    /// Does the move settle where it lands? `false` = a card effect prevented
    /// settle entirely (the move still happens).
    pub settles: bool,
    /// Free-form per-card counters on the move (a marker-funded roll is card-owned
    /// state, tagged by whoever armed it).
    pub tags: Vec<(String, i32)>,
    /// The direction the move travels.
    pub dir: Dir,
    /// The move's move start (its origin tile).
    pub from: i32,
    /// Steps the move has left to walk.
    pub remaining: i32,
    /// The move's path length.
    pub total: i32,
    /// The face the move rolled. `None` before the dice land (the `roll`
    /// window) and whenever the face is not this trigger's subject.
    pub roll: Option<i32>,
    /// The move was shaped or started by a rule whose card id starts with
    /// `skill:` (cond `move.by_skill`).
    pub by_skill: bool,
    /// Was this settle initiated by a card (`settle_tile_at`), not by the
    /// player's own move landing? Mirrors [`super::move_ctx::MoveCtx::settle_redirect`].
    pub settle_redirect: bool,
    /// A die was cast for this walk (a `steps` walk and a teleport are `false`;
    /// a `next_steps` override still casts, so it stays `true`).
    pub rolled: bool,
    /// The length the move will actually take.
    pub effective: i32,
    /// Every governing tile this move has stepped on so far, in order. Pushed
    /// at the same point as [`MoveCtx::path`](super::move_ctx::MoveCtx::path)
    /// (so a `passBefore` of the step not yet taken does not see it; its
    /// `passTile` and settle do). Tiles, not positions.
    pub path: Vec<i32>,
    /// Did an abnormal land during this move? Set on the move's frame as it
    /// lands, readable from every trigger raised later in the walk.
    pub abnormal: bool,
}

/// The roll a `DiceRoll*` trigger belongs to. [`Trigger::roll`] is `None` on
/// every other trigger.
///
/// The guest sees this as `abi::RollPayload`; the two crates cannot share a
/// type, so keep the fields aligned.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerRoll {
    /// The transaction id.
    pub id: i32,
    /// `0` = effect roll, `1` = move roll.
    pub purpose: i32,
    /// The highest-ranked [`RollSource`] seen on the transaction.
    pub source: RollSource,
    /// More than one cast exists.
    pub rerolled: bool,
    /// Index into `casts` of the face that will settle.
    pub chosen_index: i32,
    /// The face the latest pick did not take; `-1` when none.
    pub unused_face: i32,
    /// Every cast so far, `(value, source)`.
    pub casts: Vec<(i32, i32)>,
    /// The settled face at `diceRollSettled`; `None` before it.
    pub final_face: Option<i32>,
}

/// The payment a `pay` / `paid` trigger is about. `None` on every other kind:
/// the amount rides the trigger's `value` and the payee its `target`, but
/// whether the payment is rent is a pay-only fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TriggerPay {
    /// The in-flight kind; `set_pay_kind` rewrites it (last writer wins).
    pub kind: super::play::PayKind,
    /// The in-flight shape; `set_pay_shape` rewrites it before the first share.
    pub shape: super::play::PayShape,
    /// Is this payment rent, as against a buy, a build, or a forced loss?
    /// The declared [`Pay::rent`](super::play::Pay::rent) flag, not
    /// `kind == Rent`: a rent redirected to `Loss` still reads as rent.
    pub is_rent: bool,
    /// Is this payment a command **total**, as against a 「[分摊]」 leg? A
    /// command that has already staged is also `false` (a reverse child).
    pub total_stage: bool,
}

/// Where a purchase's money goes. Lives in `game-kinds` -- one definition
/// shared with the guest. Re-exported here so engine call sites keep their path.
pub use game_kinds::{Payee, Seat, Tile};

/// The purchase a buy trigger is about (`docs/PURCHASE.md`). `None` on every
/// other kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerBuy {
    /// Which kind of purchase this is.
    pub kind: super::play::purchase::BuyKind,
    /// The payee. Force-buy and 收购 name the owner; land / agent / card /
    /// auction buys name the bank.
    pub seller: Payee,
    /// The price the buyer would be charged, post the `BuyAdd` / `BuyMul` /
    /// `BuySet` stages. A hook rewrites it with `set_price`.
    pub price: i32,
    /// Ownership after the deal. `None` = the deal keeps its default (the
    /// buyer). A `BuyAssign` hook rewrites it with `set_deal_owner`.
    pub deal_owner: Option<Payee>,
    /// Houses after the deal. `None` = as standing, unless a hook razes.
    pub deal_houses: Option<i32>,
    /// Mortgage after the deal (default: cleared, except a Force buy keeps
    /// `FORCE_STAYS_MORTGAGED`).
    pub deal_mortgaged: bool,
}

impl TriggerBuy {
    /// A buy payload with no deal rewrite yet (`deal_owner` remains `None`, so
    /// the commit keeps its default owner).
    pub fn new(kind: super::play::purchase::BuyKind, seller: i32, price: i32) -> Self {
        Self {
            kind,
            seller: Payee::from_i32(seller),
            price,
            deal_owner: None,
            deal_houses: None,
            deal_mortgaged: false,
        }
    }

    /// Attach the deal snapshot a `BuyAssign` window may rewrite.
    pub fn deal(mut self, owner: Payee, houses: i32, mortgaged: bool) -> Self {
        self.deal_owner = Some(owner);
        self.deal_houses = Some(houses);
        self.deal_mortgaged = mortgaged;
        self
    }
}

impl TriggerPay {
    pub fn new(
        kind: super::play::PayKind,
        is_rent: bool,
        total_stage: bool,
        shape: super::play::PayShape,
    ) -> Self {
        Self {
            kind,
            shape,
            is_rent,
            total_stage,
        }
    }
}

/// Where a `diceRollBefore` / `diceRollResult` face came from. Mirrors
/// `card_sdk::abi::roll_source`'s wire codes; the guest ABI still sees the `i32`.
/// The engine never branches on a game name -- code `1` is the marker-funded
/// roll that rules-layer clauses match by wire code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RollSource {
    /// Unattributed, or this is not a roll trigger at all (the guest code `0`).
    #[default]
    Unattributed,
    /// Marker-funded roll (the guest code `1`, wire name `charge`).
    Charge,
    /// A hand / field card (the guest code `2`).
    Card,
    /// A skill press (the guest code `3`).
    Skill,
}

impl RollSource {
    /// The guest / CEL encoding.
    pub const fn as_i32(self) -> i32 {
        match self {
            RollSource::Unattributed => 0,
            RollSource::Charge => 1,
            RollSource::Card => 2,
            RollSource::Skill => 3,
        }
    }

    /// Precedence rank (higher wins). The wire codes are not in rank order:
    /// a charge-funded recast outranks a skill, which outranks a card, which
    /// outranks an unattributed roll.
    pub const fn rank(self) -> i32 {
        match self {
            RollSource::Unattributed => 0,
            RollSource::Card => 1,
            RollSource::Skill => 2,
            RollSource::Charge => 3,
        }
    }

    /// Decode the guest / CEL encoding.
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => RollSource::Charge,
            2 => RollSource::Card,
            3 => RollSource::Skill,
            _ => RollSource::Unattributed,
        }
    }
}

/// How a counter invalidated a chain link (Yu-Gi-Oh's two negations, plus the
/// per-recipient one the rulebook needs).
///
/// This replaces the single `Trigger::cancelled` flag, which could only ever say
/// "something cancelled this" and could not distinguish "the effect never
/// happened" from "it happened and settled to nothing".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Negation {
    /// Nothing invalidated the link; it settles for its surviving recipients.
    #[default]
    None,
    /// The link's **activation** is void. It never happened -- nothing settles,
    /// and a listener on the effect does not see it.
    Activation,
    /// The link's **effect** is void. It happened -- a listener on the effect
    /// sees it -- but it settles to nothing.
    Effect,
}

/// One effect a chain link declares. The recipient is **named at declaration**,
/// not at settlement: this is the stable thing a counteraction listens to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    /// What it does -- the raise site's kind name (`"pay"`, `"target"`, and friends).
    pub kind: String,
    /// The seat it is aimed at (-1 for none).
    pub target: i32,
    /// The other party, where the effect has one. A payment touches both its
    /// payer and its payee; without this, 「被…效果影响」 would have to know
    /// that a payment's payer rides on the link and not on the effect -- the
    /// inconsistency that made the clause a three-kind union.
    pub from: i32,
    /// The tile it is aimed at (-1 for none).
    pub tile: i32,
    /// The amount, where the effect has one (a payment's sum, a draw count, ...).
    pub value: i32,
}

/// How the responders of one amount stage combine their rewrites (MONEY-01).
///
/// The rulebook lists 支付增加/减少 (stage 2) and 支付减半/翻倍 (stage 4) as
/// *one window each* and gives no order between two responders. The engine
/// therefore composes within a stage instead of applying one responder after
/// the other:
///
/// * [`Fold::Add`] -- every responder is shown the stage **input** and its
///   `write - input` deltas are summed.
/// * [`Fold::Mul`] -- every responder is shown the stage **input** and its
///   factors (`write / input`) are multiplied (milli precision).
///
/// Both folds are commutative, so declaration order cannot change the figure.
/// A single modifier still lands exactly as before; the one end-of-calc
/// `ceil10` sees the composed figure.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Fold {
    /// Each responder overwrites the running value (stages that are not
    /// amount modifiers, and the writable `*Before` prompts).
    #[default]
    Replace,
    /// Fixed-value stage: deltas sum.
    Add,
    /// Scale stage: factors multiply.
    Mul,
}

impl Fold {
    pub fn is_fold(self) -> bool {
        self != Fold::Replace
    }
}

/// `num / den` rounded half away from zero (0 when `den` is 0).
fn half_rounded(num: i64, den: i64) -> i64 {
    if den == 0 {
        return 0;
    }
    let half = den.abs() / 2;
    if (num >= 0) == (den > 0) {
        num.saturating_add(half) / den
    } else {
        num.saturating_sub(half) / den
    }
}

/// Apply one responder's write `v` to a fold stage (MONEY-01). Shared by the
/// engine trigger and the host's payload mirror.
pub fn fold_apply(fold: Fold, input: i32, value: &mut i32, millis: &mut i64, v: i32) {
    match fold {
        Fold::Replace => *value = v,
        Fold::Add => {
            let delta = v.saturating_sub(input);
            *value = value.saturating_add(delta);
        }
        Fold::Mul => {
            if input == 0 {
                *value = v;
                return;
            }
            // The factor `v / input` is rarely exact in millionths, and the
            // running product is applied back to the input. Truncating both
            // divisions loses up to a whole money unit per responder (a
            // `half()` body over an odd figure loses its ceiling); round each
            // half-away-from-zero instead, and leave the single money round
            // (`ceil10`) to the end of the chain.
            let (input, v) = (i64::from(input), i64::from(v));
            let num = v.saturating_mul(1_000_000);
            let factor = half_rounded(num, input);
            *millis = millis.saturating_mul(factor).saturating_div(1_000_000);
            *value = half_rounded(input.saturating_mul(*millis), 1_000_000)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        }
    }
}

/// One link in a chain (Yu-Gi-Oh's `ChainLink`), and the engine's counteraction trigger.
///
/// The kind name itself encodes condition/post (`"settleBefore"` vs `"settle"`,
/// `"buyBefore"` vs `"buyAfter"`), so there is no separate `when` field.
///
/// A link is either the **effect declaration** (L1, `seq == 1`, `answers == 0`)
/// or a counteraction answering an earlier link (`answers` names it). Resolution is
/// LIFO over the closed chain; see `game-rules`' `hand_counteractions`.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub kind: String,
    pub player_id: i32,
    pub target: i32,
    /// The **governing tile** of this event. Identity: rule instances, extras,
    /// ownership, houses, mortgage, props and identity readers all key on it.
    pub tile: i32,
    /// The **board position** of this event (the slot stepped onto / settled
    /// at), `-1` on kinds with no position. `tile` is resolved from it; the
    /// two are equal when no swap is bound there.
    pub position: i32,
    pub card: Option<String>,
    pub value: i32,
    /// How this stage composes its responders' rewrites ([`Fold`]).
    pub fold: Fold,
    /// The stage input: what every responder reads as `value` while `fold` is
    /// set. Unused on [`Fold::Replace`].
    pub fold_input: i32,
    /// The running product of a [`Fold::Mul`] stage, in millionths (1_000_000
    /// = ×1). Kept off `value` so the multiplication does not round at every
    /// responder; `value` is recomputed from it after each write.
    pub fold_milli: i64,
    /// The turn step (0/1/2/3) active when this trigger raised. Stamped by
    /// `PlayCtx::counteract`, not by the raise site. Only meaningful for kinds that
    /// aren't step-specific (e.g. `mortgage` can raise during both step 1
    /// and step 3).
    pub step: i32,
    /// The player whose card caused this trigger, or `None` when it was not
    /// card-caused (board-driven: rent, buy, build, turn flow). Set at the
    /// raise site from the card's own player. `Some(by) if by != player_id`
    /// means "another player's card did this to me".
    pub by_card: Option<i32>,
    /// Was this raised as part of the turn's main move?
    ///
    /// Lives on the trigger rather than in [`TriggerMove`] because a raise can
    /// be "the main move's settle" without carrying a move payload at all --
    /// the tile `On::Settle` body needs the flag (a main landing only
    /// announces; the offers come at the end step) and has no `mv`.
    pub is_main_move: bool,
    /// The activation this raise belongs to; stamped by `counteract` from
    /// `turn.activation_ordinal`. 0 outside a routine.
    pub activation_ordinal: i32,
    /// Did the abnormal this trigger announces hold the move? Meaningful only on
    /// the `effect` and `abnormal` raises of an abnormal; false elsewhere.
    pub abnormal_blocked: bool,
    /// The move that caused this trigger, when one did. `None` = no move.
    pub mv: Option<TriggerMove>,
    /// The payment a `pay` / `paid` trigger is about. `None` on every other kind.
    pub pay: Option<TriggerPay>,
    /// The purchase a buy trigger is about (`docs/PURCHASE.md`). `None` on
    /// every other kind.
    pub buy: Option<TriggerBuy>,
    /// Where the roll's face came from. [`RollSource::Unattributed`]
    /// on every non-roll trigger.
    pub roll_source: RollSource,
    /// The roll a `DiceRoll*` trigger belongs to. `None` on every other kind.
    pub roll: Option<TriggerRoll>,
    /// How a counter invalidated this link. The effect body is skipped when this
    /// is not [`Negation::None`]; the Before/After hooks still run.
    pub negation: Negation,
    /// Recipients a counter spared from settlement (`spare(link, seat)`). The
    /// effect still settles for everyone else.
    pub spared: Vec<i32>,
    /// Position within the current chain, 1-based. 0 = not on a chain (a bare
    /// hook or gate raise).
    pub seq: u32,
    /// The link this one answers. 0 = this is the effect declaration itself.
    pub answers: u32,
    /// The effects this link declares, with their recipients already named.
    /// Empty on a bare hook/gate raise, and on a link whose card has not
    /// declared its list (see `ctx::declare_effect`).
    pub effects: Vec<Effect>,
    /// The cards the trigger is about when there are several (`drew`).
    pub cards: Vec<String>,
    /// A `BuyGate` refusal's reason key (a `log.*` message key naming why the
    /// gate refused). Written by the responder alongside `set_cancelled`.
    pub reason: Option<String>,
    /// The counter name on a `CounterChanged` hook, or the message name on a
    /// `Message` entry. `None` otherwise.
    pub name: Option<String>,
    /// Unique per raise, stamped once in `Ruleset::counteract` and shared by
    /// every job the raise dispatches. `0` outside a raise and on a trigger a
    /// test built by hand (the allocator only fills a `0`). A listener keys
    /// one-shot claims on it.
    pub uid: i32,
    /// The field instance or row this raise is *about*, or `-1`. Set on
    /// `counterChanged` (the changed instance) and `markChanged` (the mark's
    /// creating instance); never restamped per hook job -- the job's own
    /// instance is `self_uid()`.
    pub instance: i32,
    /// The transition this raise is about. `None` outside the zone family.
    pub zone: Option<game_kinds::ZoneChange>,
    /// The playing card's outcome on `card` / `cardAfter`; `None` elsewhere.
    pub play_outcome: Option<game_kinds::PlayOutcome>,
    /// Why the player left on `beforeOut`; `None` elsewhere.
    pub out_cause: Option<game_kinds::OutCause>,
    /// The tile of the innermost open settle, or `-1` when none is open. Not a
    /// pay-only fact -- `settleBody` / `settleAfter` / `tileResolved` carry it
    /// with no `pay` payload, and `settleAfter` raises after the entry popped.
    pub settle_tile: i32,
    /// The settle's terminal outcome, only on `tileResolved`.
    pub outcome: Option<super::cx::SettleOutcome>,
    /// The `ChargePaying` window came from a skill cost (`raise_from(Skill)`).
    pub charge_skill: bool,
    /// The card whose effect this link *is*: the raised play's card on a root
    /// link, the counter card's own id on a counter link (so an entry answering
    /// it sees the counter as its subject).
    pub subject_card: Option<String>,
    /// The join window opened on this link, when one is open.
    pub join: Option<JoinState>,
    /// Declared, unresolved answers on this link (indices into the chain, in
    /// declaration order). Empty on a new counter link.
    pub answers_declared: Vec<u32>,
    /// The card a copy resolves (mortis). Set when the entry resolved a copied
    /// card, read by `trigger::link_copied`. `None` otherwise.
    pub copied: Option<String>,
    /// The nearest enclosing link with a roll face (`u32::MAX` = none). A
    /// counter body's face write propagates here after it settles.
    pub roll_root: u32,
    /// Declared, unresolved negating answers on this link (a counter body's
    /// `trigger::pending_negation`).
    pub pending_negation: i32,
    /// An extreme is armed on this play or declared on this link.
    pub extreme_armed: bool,
    /// The kind of each link from the root up to this one (a counter body's
    /// `trigger::answered_kind`).
    pub answered_kinds: Vec<String>,
}

/// The join window a `SELF_REDIRECT` effect opened on a link.
///
/// Mirror of [`game_rules`]'s `TriggerJoin`, carried so the bridge and the
/// write-back move it without a second encoding.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JoinState {
    /// The `SELF_REDIRECT` declarer.
    pub opener: i32,
    /// Participating seats, seat order; only these are offered.
    pub seats: Vec<i32>,
    /// Per seat, parallel to `seats`: declared a counter in this window.
    pub declared: Vec<bool>,
    /// Per seat, parallel to `seats`: the counter landed and called `shield`.
    pub used: Vec<bool>,
    /// Per seat, parallel to `seats`: the designation the joiner named
    /// (`ctx::cancel_designation` while this body runs), `-1` when none.
    pub designated: Vec<i32>,
    /// Has `open()` completed its round at least once this pass?
    pub opened: bool,
}

/// The guest-side mirror the bridge/write-back converts through.
impl From<&crate::engine::rules::JoinState> for JoinState {
    fn from(j: &Self) -> Self {
        j.clone()
    }
}

impl Trigger {
    pub fn new(kind: impl Into<String>, player_id: usize) -> Self {
        Self {
            kind: kind.into(),
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            position: -1,
            card: None,
            value: 0,
            fold: Fold::Replace,
            fold_input: 0,
            fold_milli: 1_000_000,
            step: 0,
            by_card: None,
            is_main_move: false,
            activation_ordinal: 0,
            abnormal_blocked: false,
            mv: None,
            pay: None,
            buy: None,
            roll_source: RollSource::Unattributed,
            roll: None,
            negation: Negation::None,
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            cards: Vec::new(),
            reason: None,
            name: None,
            uid: 0,
            instance: -1,
            zone: None,
            play_outcome: None,
            out_cause: None,
            settle_tile: -1,
            outcome: None,
            charge_skill: false,
            subject_card: None,
            join: None,
            answers_declared: Vec::new(),
            copied: None,
            roll_root: u32::MAX,
            pending_negation: 0,
            extreme_armed: false,
            answered_kinds: Vec::new(),
        }
    }

    /// The figure a responder reads as `value`: the stage input on a fold
    /// stage, the running figure otherwise.
    pub fn shown_value(&self) -> i32 {
        if self.fold.is_fold() {
            self.fold_input
        } else {
            self.value
        }
    }

    /// Open this stage as an amount fold: pin the input every responder sees
    /// and reset the multiplier accumulator.
    pub fn begin_fold(&mut self, fold: Fold) {
        self.fold = fold;
        self.fold_input = self.value;
        self.fold_milli = 1_000_000;
    }

    /// A responder wrote `v`. Plain stages overwrite; an [`Fold::Add`] stage
    /// adds the delta to the running sum; a [`Fold::Mul`] stage multiplies the
    /// running product by `v / input` (millionths) and recomputes the figure.
    /// This is what makes a stage commutative (MONEY-01).
    pub fn fold_write(&mut self, v: i32) {
        fold_apply(
            self.fold,
            self.fold_input,
            &mut self.value,
            &mut self.fold_milli,
            v,
        );
    }

    /// Did a counter negate this link at all?
    pub fn is_cancelled(&self) -> bool {
        self.negation != Negation::None
    }

    /// Yu-Gi-Oh's *negate the activation*: the link never happened. Nothing
    /// settles, and a listener on the effect does not see it.
    pub fn negate_activation(&mut self) {
        self.negation = Negation::Activation;
    }

    /// Yu-Gi-Oh's *negate the effect*: the link happened -- a listener on the
    /// effect sees it -- but it settles to nothing. A stronger negation already
    /// in place is not weakened.
    pub fn negate_effect(&mut self) {
        if self.negation == Negation::None {
            self.negation = Negation::Effect;
        }
    }

    /// Take one recipient out of settlement. The effect still settles for the
    /// rest. Complete invalidation is [`Self::negate_activation`], not a
    /// `spare` per seat -- the two are deliberately distinct.
    pub fn spare(&mut self, seat: i32) {
        if !self.spared.contains(&seat) {
            self.spared.push(seat);
        }
    }

    /// Does this link settle for `seat`? False when the whole link was negated,
    /// or when a counter spared this one recipient.
    pub fn settles_for(&self, seat: i32) -> bool {
        self.negation == Negation::None && !self.spared.contains(&seat)
    }

    /// Attach the roll transaction payload.
    pub(crate) fn with_roll(&mut self, r: &super::roll::RollTransaction) -> &mut Self {
        self.roll = Some(TriggerRoll {
            id: r.id,
            purpose: r.purpose,
            source: r.source,
            rerolled: r.casts.len() > 1,
            chosen_index: r.chosen_index as i32,
            unused_face: r.unused_face,
            casts: r.casts.iter().map(|c| (c.raw, c.source.as_i32())).collect(),
            final_face: r.final_face,
        });
        self.roll_source = r.source;
        self
    }

    /// Copy the move context onto this trigger.
    ///
    /// Also stamps [`Trigger::is_main_move`] from the move, so a raise that carries a
    /// move and one that does not agree on "was this the turn's main move".
    pub(crate) fn with_move(&mut self, m: &super::play::Move) -> &mut Self {
        self.mv = Some(TriggerMove {
            kind: m.kind,
            settles: m.settles,
            tags: m.tags.clone(),
            dir: if m.reverse {
                Dir::Backward
            } else {
                Dir::Forward
            },
            from: m.from,
            remaining: m.remaining,
            total: m.total,
            roll: None,
            by_skill: m.by_skill,
            settle_redirect: m.settle_redirect,
            rolled: m.rolled,
            effective: m.effective_steps(),
            path: m.path.clone(),
            abnormal: m.abnormal,
        });
        self.is_main_move = m.is_main_move;
        self
    }
}

/// Raise a counteraction trigger: build a [`Trigger`], apply the optional field
/// overrides, and run it through `PlayCtx::raise` (which stamps `step` centrally and
/// delegates to the card rules). Every raise site should go through this so the
/// shape of a trigger remains uniform.
///
/// `@m <move>` copies the move context onto the trigger from a `Move`
/// ([`Trigger::with_move`]); `@b <buy>` / `@p <pay>` install the purchase /
/// payment payload ([`TriggerBuy`] / [`TriggerPay`]). Each must come before
/// any `field = value` overrides.
///
/// The result is the trigger as `raise` left it, so a counteraction that rewrites a
/// field (e.g. `diceRollResult`'s reroll) can read it back:
///
/// ```ignore
/// raise!(self, "passBefore", i, @m m, tile = next)?;
/// raise!(self, "diceRollBefore", i, value = -1)?;
/// raise!(self, "card", i, card = Some(id.to_string()))?;
/// let t = raise!(self, "diceRollValue", i, @m m, value = m.roll)?; // keep `t` to read back
/// ```
macro_rules! raise {
    ($cx:expr, $kind:expr, $player_id:expr $(, $($rest:tt)*)?) => {{
        let mut __trigger = Trigger::new($kind, $player_id);
        raise!(@set $cx, __trigger $(, $($rest)*)?);
        $cx.raise(__trigger)
    }};
    (@set $cx:expr, $t:ident) => {};
    (@set $cx:expr, $t:ident,) => {};
    (@set $cx:expr, $t:ident, @m $m:expr) => {
        $t.with_move(&$m);
    };
    (@set $cx:expr, $t:ident, @m $m:expr,) => {
        $t.with_move(&$m);
    };
    (@set $cx:expr, $t:ident, @m $m:expr, $($rest:tt)+) => {
        $t.with_move(&$m);
        raise!(@set $cx, $t, $($rest)+);
    };
    (@set $cx:expr, $t:ident, @b $b:expr) => {
        $t.buy = Some($b);
    };
    (@set $cx:expr, $t:ident, @b $b:expr,) => {
        $t.buy = Some($b);
    };
    (@set $cx:expr, $t:ident, @b $b:expr, $($rest:tt)+) => {
        $t.buy = Some($b);
        raise!(@set $cx, $t, $($rest)+);
    };
    (@set $cx:expr, $t:ident, @p $p:expr) => {
        $t.pay = Some($p);
    };
    (@set $cx:expr, $t:ident, @p $p:expr,) => {
        $t.pay = Some($p);
    };
    (@set $cx:expr, $t:ident, @p $p:expr, $($rest:tt)+) => {
        $t.pay = Some($p);
        raise!(@set $cx, $t, $($rest)+);
    };
    // A **position** is converted to its governing tile; a **tile** gets its
    // position derived. Arms assign in written order, so a site that knows
    // both writes `position = p, tile = t` and the explicit tile wins.
    (@set $cx:expr, $t:ident, position = $p:expr) => {
        $t.position = $p;
        $t.tile = $cx.tile_at($p);
    };
    (@set $cx:expr, $t:ident, position = $p:expr,) => {
        $t.position = $p;
        $t.tile = $cx.tile_at($p);
    };
    (@set $cx:expr, $t:ident, position = $p:expr, $($rest:tt)+) => {
        $t.position = $p;
        $t.tile = $cx.tile_at($p);
        raise!(@set $cx, $t, $($rest)+);
    };
    // `slot = p` likewise writes the **position** but leaves an explicit
    // `tile` alone: the settle's terminal raise happens *after* a body may have
    // relocated the card, so `position_of_tile(tile)` is stale there and the
    // settle's own slot must be written, not re-derived from the tile.
    (@set $cx:expr, $t:ident, slot = $p:expr) => {
        $t.position = $p;
    };
    (@set $cx:expr, $t:ident, slot = $p:expr,) => {
        $t.position = $p;
    };
    (@set $cx:expr, $t:ident, slot = $p:expr, $($rest:tt)+) => {
        $t.position = $p;
        raise!(@set $cx, $t, $($rest)+);
    };
    (@set $cx:expr, $t:ident, tile = $s:expr) => {
        $t.tile = $s;
        $t.position = $cx.position_of_tile($s);
    };
    (@set $cx:expr, $t:ident, tile = $s:expr,) => {
        $t.tile = $s;
        $t.position = $cx.position_of_tile($s);
    };
    (@set $cx:expr, $t:ident, tile = $s:expr, $($rest:tt)+) => {
        $t.tile = $s;
        $t.position = $cx.position_of_tile($s);
        raise!(@set $cx, $t, $($rest)+);
    };
    (@set $cx:expr, $t:ident, $field:ident = $value:expr) => {
        $t.$field = $value;
    };
    (@set $cx:expr, $t:ident, $field:ident = $value:expr,) => {
        $t.$field = $value;
    };
    (@set $cx:expr, $t:ident, $field:ident = $value:expr, $($rest:tt)+) => {
        $t.$field = $value;
        raise!(@set $cx, $t, $($rest)+);
    };
    // `@f add` / `@f mul` open an amount fold on the value written so far --
    // write it after the `value = ...` override it should fold.
    (@set $cx:expr, $t:ident, @f add) => {
        $t.begin_fold(Fold::Add);
    };
    (@set $cx:expr, $t:ident, @f add,) => {
        $t.begin_fold(Fold::Add);
    };
    (@set $cx:expr, $t:ident, @f add, $($rest:tt)+) => {
        $t.begin_fold(Fold::Add);
        raise!(@set $cx, $t, $($rest)+);
    };
    (@set $cx:expr, $t:ident, @f mul) => {
        $t.begin_fold(Fold::Mul);
    };
    (@set $cx:expr, $t:ident, @f mul,) => {
        $t.begin_fold(Fold::Mul);
    };
    (@set $cx:expr, $t:ident, @f mul, $($rest:tt)+) => {
        $t.begin_fold(Fold::Mul);
        raise!(@set $cx, $t, $($rest)+);
    };
}

pub(crate) use raise;

/// One declared **tile text** (`CardRules::card_texts`): the deed-popup
/// description an entry speaks for. `game-core`'s own shape -- this crate does
/// not depend on `card-sdk` types (same pattern as [`crate::state::prop`] vs
/// `card_sdk::abi::prop`); the host's `ManifestTileText` carries the same
/// `slot` / `tile` / `key` plus the entry index, which the loader folds into
/// the Vec's order.
///
/// * `slot` -- `"settle"` or `"pass"` (the settle / [pass] half).
/// * `tile` -- `""` = the instance's own tile; otherwise a board tile *name*
///   (`"the start square"`) for a tile-bound text on a tile-less instance.
/// * `key` -- `ui` namespace i18n key (`deed.note.*`); `""` suppresses the slot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileTextDecl {
    pub slot: String,
    pub tile: String,
    pub key: String,
}

pub trait CardRules: Send + Sync {
    /// Hex SHA-256 of the built ruleset, when the implementation has one.
    /// `None` (the default) means "unknown" and is skipped by
    /// [`crate::record::compat`]; `WasmRules` reports the ruleset's own hash.
    /// `StubRules` has no ruleset, so its stamp reads `"stub"`.
    fn ruleset_sha256(&self) -> Option<&str> {
        None
    }

    /// May be played from hand in the operation phase.
    fn normal(&self, _card: &str) -> bool {
        true
    }

    /// A capability gate (`docs/STATUS-HOSTS.md`): `Some(true)` = open. The
    /// ruleset answers from its standing CEL-only `On::Gate` conditions,
    /// natively and purely. `None` (the default) means "no ruleset opinion":
    /// the engine answers open. There is no named engine-side mirror.
    fn gate_open(&self, _w: &super::World, _gate: Gate, _player: usize) -> Option<bool> {
        None
    }

    /// The i18n key for "this capability is closed" -- built by the ruleset
    /// (its gate-declaring rule supplies the key as data). The engine never
    /// names a status.
    fn gate_refusal_msg(&self, _gate: Gate, _player: usize) -> Msg {
        Msg::new("status.move_blocked")
    }

    /// The i18n key for the turn-skip log when this gate is closed at turn
    /// start. Also ruleset-supplied data (`gate_skip_msgs`).
    fn gate_skip_msg(&self, gate: Gate, player: usize) -> Msg {
        self.gate_refusal_msg(gate, player)
    }

    /// May this card be played despite a closed act gate? The ruleset answers
    /// from a card prop whose name it owns (`exemption_prop`).
    fn card_act_exemption(&self, _card: &str) -> bool {
        false
    }

    /// An [`Gate::Exclusive`] question (`docs/GUARDS.md`): `Some(true)` = the
    /// ruleset's CEL-only `ExclusiveUse` `On::Gate` entries all allow this
    /// exclusive play. `None` (the default) means "no ruleset opinion": the
    /// engine's generic own/base verdict stands. A group-skill card (CRYCHIC)
    /// closes the gate to deny a variant its base persona's exclusives.
    fn exclusive_use(
        &self,
        _w: &super::World,
        _player: usize,
        _ask: &ExclusiveAsk<'_>,
    ) -> Option<bool> {
        None
    }

    /// Extra card-specific reason it can't be played right now.
    fn cant_play(&self, _cx: &PlayCtx, _player: usize, _card: &str) -> Option<Msg> {
        None
    }

    /// Collect the activation argument before the play commits
    /// (docs/ACTIVATION.md §2). `Ok(None)` = the player declined and the card
    /// must **not** leave the hand. `Ok(Some(arg))` = proceed; `arg` rides the
    /// play into the body. `Err(Halt)` = a prompt is pending -- the same
    /// multi-pass / answer-log model as a body `Asked`.
    ///
    /// Default: no collector, empty arg. `WasmRules` runs the entry's `OP_DECLARE`.
    fn declare_activation(
        &self,
        _cx: &mut PlayCtx,
        _player: usize,
        _card: &str,
    ) -> Flow<Option<Vec<u8>>> {
        Ok(Some(Vec::new()))
    }

    /// Does this rule declare an activatable `On::Play` (a skill button / card
    /// play)? Passive / hook-only rules answer `false` and never appear in a
    /// viewer's skill list. `WasmRules` answers from the card's Play entry;
    /// the default `false` is [`StubRules`] (no ruleset, nothing to press).
    fn has_play(&self, _card: &str) -> bool {
        false
    }

    /// The card rule's declared static **properties** (`CardDef::props`),
    /// `key -> value`. Keys are [`crate::state::prop`] constants; a key the
    /// card does not declare is simply absent and reads as its default (`0`).
    /// Empty when the card declares none. Never derived from prose.
    fn card_props(&self, _card: &str) -> std::collections::BTreeMap<String, i32> {
        std::collections::BTreeMap::new()
    }

    /// One declared property (see [`Self::card_props`]). The defined default
    /// is `0` for every key the engine reads.
    fn card_prop(&self, card: &str, key: &str) -> i32 {
        self.card_props(card).get(key).copied().unwrap_or(0)
    }

    /// The card rule's declared **tile texts** (`On::texts`): the
    /// deed-popup description each entry speaks for, **in declaration order**
    /// (sorted by entry index, then declaration position -- that order is the
    /// third component of the view's text precedence key, `docs/ENGINE.md`
    /// 「Effect ordering」). Empty when the card declares none.
    ///
    /// [`TileTextDecl`] is `game-core`'s own shape (this crate does not depend
    /// on `card-sdk` types); the host's `ManifestTileText` carries the same
    /// `slot` / `tile` / `key` plus the entry index the loader folds into this
    /// Vec's order. See `docs/TILES.md` 「Tile text」.
    fn card_texts(&self, _card: &str) -> std::sync::Arc<[TileTextDecl]> {
        std::sync::Arc::from(Vec::new())
    }

    /// Does this ruleset declare a rule with this id (a card id or a
    /// `tile:*` tile-rule id)? `bind_tiles` asks before placing an instance.
    /// Default `false` -- `StubRules` has no rules, so nothing binds and the
    /// engine's built-in [`Self::settle_tile`] handles every tile.
    fn has_rule(&self, _id: &str) -> bool {
        false
    }

    /// The rule id that settles a board tile of this **opaque kind**, or `""`
    /// when the ruleset declares none (the engine's generic deed settlement
    /// then applies). Supplied by the ruleset (`CardDef::governs` /
    /// `board_role == "tile"`); the engine keeps no kind→rule table.
    fn tile_rule_id(&self, _kind: &str) -> String {
        String::new()
    }

    /// Every **rule entity** id (`CardInfo::rule`, ABI v119): names that are
    /// rules, not cards. A card area drops them (`World::note_non_card`).
    fn rule_ids(&self) -> Vec<String> {
        Vec::new()
    }

    /// Board-wide **mark-owner** rule ids (`board_role == "mark"`): one
    /// standing instance each on the neutral board owner.
    fn mark_rule_ids(&self) -> Vec<String> {
        Vec::new()
    }

    /// Board-wide **status-host** rule ids (`board_role == "status"`): one
    /// standing instance each on the neutral board owner.
    fn status_rule_ids(&self) -> Vec<String> {
        Vec::new()
    }

    /// Resolve the tile `player` stopped on -- the **settle body**.
    ///
    /// The default is the built-in plain-Monopoly settlement
    /// (`land_at`'s body): generic deed buy / pay-rent / build, plus a plain
    /// log for a non-deed. `WasmRules` overrides this to run the tile's
    /// **rule instances** (`docs/TILES.md`) and falls back to this default for
    /// a tile with none.
    fn settle_tile(
        &self,
        cx: &mut PlayCtx,
        player_id: usize,
        position: usize,
        tile: i32,
        m: &super::move_ctx::MoveCtx,
        is_main_move: bool,
    ) -> Flow<()> {
        let _ = m;
        cx.land_at_built_in(player_id, position, tile, is_main_move)
    }

    /// Would a bot offer it now? Standard / advanced policies consult this;
    /// chaos ignores it by design (`bot-core/src/policy.rs`). The rule set
    /// answers from the card's static `prop::AI_NEVER` -- never legality, so a
    /// human can still play the card.
    fn ai_play(&self, _cx: &PlayCtx, _player: usize, _card: &str) -> bool {
        true
    }

    /// The effect. Returns where the card goes afterwards.
    fn play(&self, cx: &mut PlayCtx, player_id: usize, card: &str) -> Flow<Dest>;

    /// Resolve a drawn event card. Returns `true` if the event remains in play
    /// (otherwise it is discarded).
    fn event(&self, cx: &mut PlayCtx, player_id: usize, id: &str) -> Flow<bool>;

    /// counteraction window on `t`; may raise prompts.
    ///
    /// `t` is mutable: a counteraction may rewrite the move face (`t.value` /
    /// `t.mv.roll`) and the engine then uses the new face.
    fn counteract(&self, _cx: &mut PlayCtx, _t: &mut Trigger) -> Flow<()> {
        Ok(())
    }

    /// Short note shown on a card in hand (`HandNotesOf`).
    fn hand_note(&self, _cx: &PlayCtx, _player: usize, _card: &str) -> Msg {
        Msg::default()
    }

    /// The purchase quote (`docs/PURCHASE.md`): what would `q.player` be
    /// charged for each tile in `q.tiles`, and may they buy it at all?
    ///
    /// Takes the world by reference rather than a [`PlayCtx`] so the view's
    /// A quote can be asked without cloning the world. The default is
    /// the plain rulebook formula -- [`super::play::purchase::quote_native`] --
    /// which is what `StubRules` and the sim run. `WasmRules` overrides it to
    /// run the `BuyGate` / `BuyAdd` / `BuyMul` / `BuySet` hooks in pure guard
    /// mode (like `cant_play`), cloning the world only when a hooking instance
    /// exists. The commit re-quotes, so quote == charge.
    fn buy_quote(
        &self,
        w: &super::World,
        data: &crate::data::GameData,
        q: &super::play::purchase::BuyQuery,
    ) -> Vec<super::play::purchase::Quote> {
        let st = &w.st;
        q.tiles
            .iter()
            .map(|&t| {
                let price = super::play::purchase::base_quote(data, st, t, q.kind);
                super::play::purchase::Quote {
                    price,
                    eligible: price >= 0,
                }
            })
            .collect()
    }

    /// Run the tile rule's mortgage body (`On::Mortgage`) for `tile`.
    /// `reason`: 0 = player act, 1 = raise-funds, 2 = optional funding prompt.
    /// `true` when the deed went onto mortgage. The default is the engine's
    /// standing mortgage (StubRules).
    ///
    /// The mortgage **amount** is not a hook: [`super::play::purchase::mortgage_value`]
    /// is the rulebook figure, and a retuned figure belongs on the tile rule
    /// instance's props (`docs/TILES.md`).
    fn deed_mortgage(
        &self,
        cx: &mut super::PlayCtx,
        player: usize,
        tile: usize,
        _reason: i32,
    ) -> super::Flow<bool> {
        super::play::purchase::standalone_mortgage(cx, player, tile)
    }

    /// Run the tile rule's redeem body (`On::Redeem`) for `tile`.
    /// The default is the engine's standing redeem. The redeem **charge** is
    /// not a hook: [`super::play::purchase::redeem_cost`] is the rulebook
    /// figure, and a retuned figure belongs on the tile rule instance's props
    /// (`docs/TILES.md`).
    fn deed_redeem(
        &self,
        cx: &mut super::PlayCtx,
        player: usize,
        tile: usize,
    ) -> super::Flow<bool> {
        super::play::purchase::standalone_redeem(cx, player, tile)
    }

    /// Run the tile rule's buy body (`On::Buy`) for `tile`.
    /// `kind` is the buy kind; `reason` is the source (0 = settle offer,
    /// 1 = card-driven, 2 = agent, 3 = acquire / auction). `true` when the
    /// deed changed hands. The default is the engine's standing buy
    /// (StubRules).
    fn deed_buy(
        &self,
        cx: &mut super::PlayCtx,
        player: usize,
        tile: usize,
        kind: super::play::purchase::BuyKind,
        _reason: i32,
    ) -> super::Flow<bool> {
        super::play::purchase::standalone_buy(cx, player, tile, kind)
    }

    /// Run the tile rule's build body (`On::Build`) for `tile`.
    /// The default is the engine's standing build (StubRules).
    fn deed_build(&self, cx: &mut super::PlayCtx, player: usize, tile: usize) -> super::Flow<bool> {
        super::play::purchase::standalone_build(cx, player, tile)
    }
}

/// Which capability gate a [`CardRules::gate_open`] question is about.
/// Generic turn/board concepts -- the ruleset decides which opaque state
/// keys close which gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// [主动] skills and 手卡的hand效果.
    Act,
    /// Paying / receiving payment.
    Pay,
    /// This turn's main move.
    Move,
    /// The piece is present on a board square (may be landed on / walked to).
    OnBoard,
    /// The piece may be named as a designation / target.
    Targetable,
    /// May this exclusive card be played? Context rides along as an
    /// [`ExclusiveAsk`] on [`CardRules::exclusive_use`].
    Exclusive,
    /// May a hand card be played right now? Play-only (unlike [`Self::Act`],
    /// which also gates [主动] skills).
    Play,
    /// May an abnormal of this kind (an `AbKind` wire code) from `by` land on the
    /// seat? `exemption = false` asks the refusal; `true` asks "would this kind
    /// hold this mover's move?" (blocked-ness). CEL-only.
    Abnormal { kind: i32, by: i32, exemption: bool },
}

/// The card-side context of a [`Gate::Exclusive`] question.
#[derive(Debug, Clone, Copy)]
pub struct ExclusiveAsk<'a> {
    /// The exclusive card's id.
    pub card: &'a str,
    /// The card's `owner` persona name.
    pub owner: &'a str,
    /// `owner == actor.persona`.
    pub owner_is_self: bool,
    /// `owner == base_persona(actor.persona)`.
    pub owner_is_base: bool,
}

/// Every card and event has no effect yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubRules;

impl CardRules for StubRules {
    fn play(&self, cx: &mut PlayCtx, player_id: usize, card: &str) -> Flow<Dest> {
        // No module body runs and no effect activates -- a stub play remains
        // silent apart from the "not ported" note. (`WasmRules` announces its
        // bodies at `drive_inner_body`; the sim's counts therefore see no new
        // event kinds.)
        cx.log(
            player_id as i32,
            Msg::new("log.card_not_ported").card("card", card),
        );
        Ok(Dest::Graveyard)
    }

    fn event(&self, cx: &mut PlayCtx, player_id: usize, id: &str) -> Flow<bool> {
        cx.log(
            player_id as i32,
            Msg::new("log.event_not_ported").event("event", id),
        );
        Ok(false)
    }
}
