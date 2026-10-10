//! The boundary where card content plugs into the match shell.
//!
//! The shell calls these hooks at the points the rulebook's turn and effect
//! structure needs them; an implementation decides what cards and events
//! actually do. `game-rules` will implement this over the WASM card modules.
//! [`StubRules`] gives every card and event no effect, which leaves a complete,
//! playable game of plain BanG Dream Monopoly.

use super::cx::{Cx, Flow};
use crate::msg::Msg;

/// Where a played card goes afterwards.
///
/// The fates: Graveyard (discard pile, 弃卡区), Hand, Field (stays in play,
/// 场上), Banished (「[移除]」, out of the game).
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
    /// Stays in play (场上) -- a persistent effect.
    Field,
    /// 「[移除]」 -- out of the game entirely.
    Banished,
}

// `MoveFlags` is gone: a move is exactly one [`super::move_ctx::MoveKind`] and
// what it resolves is `super::move_ctx::Settle` -- two categories, not one flag
// soup. Card-owned move state (a [火罐] roll, say) rides on the move's tags.

/// Which way a move travels along the ring.
///
/// Replaces the ±1 `move_dir` integer: forward and backward are the only two
/// directions a walk has, so they are two variants, not a sign bit. The guest
/// ABI and CEL still see `1` / `-1` (see [`Dir::as_i32`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Dir {
    /// 「前进」 -- towards higher tile indices (the guest / CEL `1`).
    #[default]
    Forward,
    /// 「后退」 -- towards lower tile indices (the guest / CEL `-1`).
    Backward,
}

impl Dir {
    /// The guest / CEL encoding: `1` forward, `-1` backward.
    pub const fn as_i32(self) -> i32 {
        match self {
            Dir::Forward => 1,
            Dir::Backward => -1,
        }
    }

    /// Decode the guest / CEL encoding (`1` / `-1`; anything else reads forward).
    pub fn from_i32(v: i32) -> Self {
        if v < 0 {
            Dir::Backward
        } else {
            Dir::Forward
        }
    }
}

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
    pub resolve: bool,
    /// Free-form per-card counters on the move (a [火罐] roll is card-owned
    /// state, tagged by whoever armed it).
    pub tags: Vec<(String, i32)>,
    /// The direction the move travels.
    pub dir: Dir,
    /// The move's 移动起点 (its origin tile).
    pub from: i32,
    /// Steps the move has left to walk.
    pub remaining: i32,
    /// The move's path length.
    pub total: i32,
    /// The face the move rolled. `None` before the dice land (the `roll`
    /// window) and whenever the face is not this trigger's subject.
    pub roll: Option<i32>,
}

/// The payment a `pay` / `paid` trigger is about. `None` on every other kind:
/// the amount rides the trigger's `value` and the payee its `target`, but
/// whether the payment is rent is a pay-only fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TriggerPay {
    /// Is this payment rent, as against a buy, a build, or a forced loss?
    /// Always `false` on a card-driven payment.
    pub is_rent: bool,
}

/// Where a purchase's money goes. `-1` on the wire means the bank; that is a
/// real payee, not "absent", so it gets a variant of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payee {
    /// The bank (guest / wire `-1`).
    Bank,
    /// A seat.
    Seat(i32),
}

impl Payee {
    /// The guest / wire encoding: `-1` for the bank, else the seat.
    pub fn as_i32(self) -> i32 {
        match self {
            Payee::Bank => -1,
            Payee::Seat(n) => n,
        }
    }

    /// Decode the guest / wire encoding (`-1` = bank).
    pub fn from_i32(v: i32) -> Self {
        if v < 0 {
            Payee::Bank
        } else {
            Payee::Seat(v)
        }
    }
}

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
    pub deal_owner: Option<i32>,
    /// Houses after the deal (default: as standing, unless a hook razes).
    pub deal_houses: i32,
    /// Mortgage after the deal (default: cleared, except a Force buy keeps
    /// `FORCE_STAYS_MORTGAGED`).
    pub deal_mortgaged: bool,
}

impl TriggerBuy {
    /// A buy payload with no deal rewrite yet (`deal_owner` stays `None`, so
    /// the commit keeps its default owner).
    pub fn new(
        kind: super::play::purchase::BuyKind,
        seller: i32,
        price: i32,
    ) -> Self {
        Self {
            kind,
            seller: Payee::from_i32(seller),
            price,
            deal_owner: None,
            deal_houses: 0,
            deal_mortgaged: false,
        }
    }

    /// Attach the deal snapshot a `BuyAssign` window may rewrite.
    pub fn deal(mut self, owner: i32, houses: i32, mortgaged: bool) -> Self {
        self.deal_owner = Some(owner);
        self.deal_houses = houses;
        self.deal_mortgaged = mortgaged;
        self
    }
}

impl TriggerPay {
    pub fn new(is_rent: bool) -> Self {
        Self { is_rent }
    }
}

/// Where a `roll` / `moveRoll` face came from (「当你使用火罐进行掷骰时」).
/// Mirrors `card_sdk::abi::roll_source`'s codes; the guest ABI still sees the
/// `i32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RollSource {
    /// Unattributed, or this is not a roll trigger at all (the guest code `0`).
    #[default]
    Unattributed,
    /// A fire pot (the guest code `1`).
    Fire,
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
            RollSource::Fire => 1,
            RollSource::Card => 2,
            RollSource::Skill => 3,
        }
    }

    /// Decode the guest / CEL encoding.
    pub fn from_i32(v: i32) -> Self {
        match v {
            1 => RollSource::Fire,
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
/// not at settlement: this is the stable thing a [反击] listens to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    /// What it does -- the raise site's kind name (`"pay"`, `"target"`, `"stay"`, ...).
    pub kind: &'static str,
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

/// One link in a chain (Yu-Gi-Oh's `ChainLink`), and the engine's counteraction trigger.
///
/// The kind name itself encodes pre/post (`"settleBefore"` vs `"settle"`,
/// `"buyBefore"` vs `"buyAfter"`), so there is no separate `when` field.
///
/// A link is either the **effect declaration** (L1, `seq == 1`, `answers == 0`)
/// or a [反击] answering an earlier link (`answers` names it). Resolution is
/// LIFO over the closed chain; see `game-rules`' `hand_counteractions`.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub kind: &'static str,
    pub player_id: i32,
    pub target: i32,
    pub tile: i32,
    /// The card this trigger is about (`t.Card`), or `None`.
    pub card: Option<String>,
    pub value: i32,
    /// The turn step (0/1/2/3) active when this trigger fired. Stamped by
    /// `Cx::counteract`, not by the raise site. Only meaningful for kinds that
    /// aren't step-specific (e.g. `mortgage` can fire during both step 1
    /// and step 3).
    pub step: i32,
    /// The player whose card caused this trigger, or `None` when it was not
    /// card-caused (board-driven: rent, buy, build, turn flow). Set at the
    /// raise site from the card's own player. `Some(by) if by != player_id`
    /// means "another player's card did this to me".
    pub by_card: Option<i32>,
    /// Was this fired as part of the turn's [主要移动]?
    ///
    /// Lives on the trigger rather than in [`TriggerMove`] because a raise can
    /// be "the main move's settle" without carrying a move payload at all --
    /// the tile `On::Settle` body needs the flag (a main landing only
    /// announces; the offers come at the end step) and has no `mv`.
    pub main: bool,
    /// The move that caused this trigger, when one did. `None` = no move.
    pub mv: Option<TriggerMove>,
    /// The payment a `pay` / `paid` trigger is about. `None` on every other kind.
    pub pay: Option<TriggerPay>,
    /// The purchase a buy trigger is about (`docs/PURCHASE.md`). `None` on
    /// every other kind.
    pub buy: Option<TriggerBuy>,
    /// Where a `roll` / `moveRoll` face came from. [`RollSource::Unattributed`]
    /// on every non-roll trigger.
    pub roll_source: RollSource,
    /// How a counter invalidated this link. The effect body is skipped when this
    /// is not [`Negation::None`]; the Before/After hooks still fire.
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
}

impl Trigger {
    pub fn new(kind: &'static str, player_id: usize) -> Self {
        Self {
            kind,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            card: None,
            value: 0,
            step: 0,
            by_card: None,
            main: false,
            mv: None,
            pay: None,
            buy: None,
            roll_source: RollSource::Unattributed,
            negation: Negation::None,
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            cards: Vec::new(),
            reason: None,
            name: None,
        }
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

    /// Copy the move context onto this trigger.
    ///
    /// Also stamps [`Trigger::main`] from the move, so a raise that carries a
    /// move and one that does not agree on "was this the turn's main move".
    pub(crate) fn with_move(&mut self, m: &super::play::Move) -> &mut Self {
        self.mv = Some(TriggerMove {
            kind: m.kind,
            resolve: m.resolve,
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
        });
        self.main = m.main;
        self
    }
}

/// Raise a counteraction trigger: build a [`Trigger`], apply the optional field
/// overrides, and run it through `Cx::raise` (which stamps `step` centrally and
/// delegates to the card rules). Every raise site should go through this so the
/// shape of a trigger stays uniform.
///
/// `@m <move>` copies the move context onto the trigger from a `Move`
/// ([`Trigger::with_move`]); `@b <buy>` / `@p <pay>` install the purchase /
/// payment payload ([`TriggerBuy`] / [`TriggerPay`]). Each must come before
/// any `field = value` overrides.
///
/// The result is the trigger as `raise` left it, so a counteraction that rewrites a
/// field (e.g. `moveRoll`'s reroll) can read it back:
///
/// ```ignore
/// raise!(self, "passBefore", i, @m m, tile = next)?;
/// raise!(self, "roll", i, value = -1)?;
/// raise!(self, "card", i, card = Some(id.to_string()))?;
/// let t = raise!(self, "moveRoll", i, @m m, value = m.roll)?; // keep `t` to read back
/// ```
macro_rules! raise {
    ($cx:expr, $kind:expr, $player_id:expr $(, $($rest:tt)*)?) => {{
        let mut __trigger = Trigger::new($kind, $player_id);
        raise!(@set __trigger $(, $($rest)*)?);
        $cx.raise(__trigger)
    }};
    (@set $t:ident) => {};
    (@set $t:ident,) => {};
    (@set $t:ident, @m $m:expr) => {
        $t.with_move(&$m);
    };
    (@set $t:ident, @m $m:expr,) => {
        $t.with_move(&$m);
    };
    (@set $t:ident, @m $m:expr, $($rest:tt)+) => {
        $t.with_move(&$m);
        raise!(@set $t, $($rest)+);
    };
    (@set $t:ident, @b $b:expr) => {
        $t.buy = Some($b);
    };
    (@set $t:ident, @b $b:expr,) => {
        $t.buy = Some($b);
    };
    (@set $t:ident, @b $b:expr, $($rest:tt)+) => {
        $t.buy = Some($b);
        raise!(@set $t, $($rest)+);
    };
    (@set $t:ident, @p $p:expr) => {
        $t.pay = Some($p);
    };
    (@set $t:ident, @p $p:expr,) => {
        $t.pay = Some($p);
    };
    (@set $t:ident, @p $p:expr, $($rest:tt)+) => {
        $t.pay = Some($p);
        raise!(@set $t, $($rest)+);
    };
    (@set $t:ident, $field:ident = $value:expr) => {
        $t.$field = $value;
    };
    (@set $t:ident, $field:ident = $value:expr,) => {
        $t.$field = $value;
    };
    (@set $t:ident, $field:ident = $value:expr, $($rest:tt)+) => {
        $t.$field = $value;
        raise!(@set $t, $($rest)+);
    };
}
pub(crate) use raise;

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

    /// Extra card-specific reason it can't be played right now.
    fn cant_play(&self, _cx: &Cx, _player: usize, _card: &str) -> Option<Msg> {
        None
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

    /// Does this ruleset declare a rule with this id (a card id or a
    /// `tile:*` tile-rule id)? `bind_tiles` asks before placing an instance.
    /// Default `false` -- `StubRules` has no rules, so nothing binds and the
    /// engine's built-in [`Self::settle_tile`] handles every tile.
    fn has_rule(&self, _id: &str) -> bool {
        false
    }

    /// Resolve the tile `player` stopped on -- the **settle body**.
    ///
    /// The default is the built-in plain-BanG Dream Monopoly settlement
    /// (`land_at`'s body): buyable / circle / edogawa / cafe / ryuseido /
    /// agent. `WasmRules` overrides this to run the tile's **rule instances**
    /// (`docs/TILES.md`) and falls back to this default for a tile with none.
    fn settle_tile(
        &self,
        cx: &mut Cx,
        player_id: usize,
        tile: usize,
        main: bool,
    ) -> Flow<()> {
        cx.land_at_built_in(player_id, tile, main)
    }

    /// Would a bot play it now?
    fn ai_play(&self, _cx: &Cx, _player: usize, _card: &str) -> bool {
        true
    }

    /// The effect. Returns where the card goes afterwards.
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest>;

    /// Resolve a drawn event card. Returns `true` if the event stays in play
    /// (otherwise it is discarded).
    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool>;

    /// [反击] window on `t`; may raise prompts.
    ///
    /// `t` is mutable: a counteraction may rewrite the move face (`t.value` /
    /// `t.mv.roll`) and the engine then uses the new face.
    fn counteract(&self, _cx: &mut Cx, _t: &mut Trigger) -> Flow<()> {
        Ok(())
    }

    /// Short note shown on a card in hand (`HandNotesOf`).
    fn hand_note(&self, _cx: &Cx, _player: usize, _card: &str) -> Msg {
        Msg::default()
    }

    /// The purchase quote (`docs/PURCHASE.md`): what would `q.player` be
    /// charged for each tile in `q.tiles`, and may they buy it at all?
    ///
    /// Takes the world by reference rather than a [`Cx`] so the view's
    /// `st.buy_price` preview can ask without cloning the world. The default is
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
}

/// Every card and event has no effect yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubRules;

impl CardRules for StubRules {
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest> {
        // No module body runs and no effect activates -- a stub play stays
        // silent apart from the "not ported" note. (`WasmRules` announces its
        // bodies at `drive_inner_body`; the sim's counts therefore see no new
        // event kinds.)
        cx.log(
            player_id as i32,
            Msg::new("log.card_not_ported").card("card", card),
        );
        Ok(Dest::Graveyard)
    }

    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool> {
        cx.log(
            player_id as i32,
            Msg::new("log.event_not_ported").event("event", id),
        );
        Ok(false)
    }
}
