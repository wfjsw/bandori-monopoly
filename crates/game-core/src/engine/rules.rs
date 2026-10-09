//! The boundary where card content plugs into the match shell.
//!
//! The shell calls these hooks at the same points the C# does; an implementation
//! decides what cards and events actually do. `game-rules` will implement this over
//! the WASM card modules. [`StubRules`] gives every card and event no effect, which
//! leaves a complete, playable game of plain BanG Dream Monopoly.

use super::cx::{Cx, Flow};
use crate::msg::Msg;

/// Where a played card goes afterwards (C# `PlayCtx.Dest`).
///
/// The port's names for the fates: Graveyard (discard pile, 弃卡区), Hand,
/// Field (stays in play, 场上), Banished (「[移除]」, out of the game).
/// Planned: the draw-pile fates `DeckTop` / `DeckBottom` / `DeckRandom` (C#
/// `c.Dest = "deck"` -> `H.AddToDeck(seat, card, where)` with `where` =
/// `"top"` / `"bottom"` / `"shuffle"`; the C# default is **shuffle**).
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

/// How a counter invalidated a chain link (Yu-Gi-Oh's two negations, plus the
/// per-recipient one the rulebook needs).
///
/// This replaces the single `Trigger.Cancelled` flag, which could only ever say
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
    pub card: String,
    pub value: i32,
    /// The turn step (0/1/2/3) active when this trigger fired. Stamped by
    /// `Cx::counteract`, not by the raise site. Only meaningful for kinds that
    /// aren't step-specific (e.g. `mortgage` can fire during both step 1
    /// and step 3).
    pub step: i32,
    /// The player whose card caused this trigger (C# `t.ByCard`), or `None` when
    /// it was not card-caused (board-driven: rent, buy, build, turn flow). Set
    /// at the raise site from the card's own player. `Some(by) if by != player_id` is
    /// C# `H.HitByOtherCard(t, seat)`.
    pub by_card: Option<i32>,
    /// `t.Pay.IsRent` on `pay`/`paid` triggers -- is this payment rent (C# `t.Pay.kind == "rent"`).
    pub pay_is_rent: bool,
    /// `t.Move` -- how the move that caused this trigger got there (C#
    /// `m.Teleport`); `None` when the move did not cause it.
    pub move_kind: Option<super::move_ctx::MoveKind>,
    /// `t.Move.Resolve` -- does that move settle where it lands?
    pub move_resolve: bool,
    /// `t.Move.Tags` -- free-form per-card counters on that move (a [火罐] roll
    /// is card-owned state, tagged by whoever armed it).
    pub move_tags: Vec<(String, i32)>,
    /// `t.Move.Main` -- was this the turn's main move (C# `MoveCtx.main`)?
    pub move_main: bool,
    /// `t.Move.Dir` -- 1 forward, -1 backward. Only meaningful when `move_flags.is_move()`.
    pub move_dir: i32,
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
    /// `t.Move.Remaining` -- steps the move has left to walk.
    pub move_remaining: i32,
    /// `t.Move.Total` -- the move's path length (C# `m.Path.Count`).
    pub move_total: i32,
    /// The cards the trigger is about when there are several (`drew`).
    pub cards: Vec<String>,
    /// `t.Roll.Source` -- where a `roll` / `moveRoll` face came from, as a
    /// `card_sdk::abi::roll_source` code (`0` = unattributed, `1` = a fire pot,
    /// `2` = a hand/field card, `3` = a skill press). 「当你使用火罐进行掷骰时」
    /// reads this. `0` on every non-roll trigger.
    pub roll_source: i32,
    // ---- v40 purchase payload (`docs/PURCHASE.md`) ------------------------
    /// `t.Buy.Kind` -- a [`super::play::purchase::BuyKind`] as `i32`
    /// (`0` = land). `0` on every non-buy trigger.
    pub buy_kind: i32,
    /// `t.Buy.Seller` -- the payee (`-1` = the bank). Force-buy and 收购 name
    /// the owner; land / agent / card / auction buys name the bank.
    pub seller: i32,
    /// `t.Buy.Price` -- the price the buyer would be charged, post the
    /// `BuyAdd` / `BuyMul` / `BuySet` stages. A hook rewrites it with
    /// `set_price`.
    pub price: i32,
    /// `t.Buy.DealOwner` -- ownership after the deal (default: the buyer).
    /// A `BuyAssign` hook rewrites it with `set_deal_owner`.
    pub deal_owner: i32,
    /// `t.Buy.DealHouses` -- houses after the deal (default: as standing,
    /// unless a hook razes). `set_deal_houses`.
    pub deal_houses: i32,
    /// `t.Buy.DealMortgaged` -- mortgage after the deal (default: cleared,
    /// except a Force buy keeps `FORCE_STAYS_MORTGAGED`). `set_deal_mortgaged`.
    pub deal_mortgaged: bool,
    /// A `BuyGate` refusal's reason key (a `log.*` message key naming why the
    /// gate refused). Written by the responder alongside `set_cancelled`.
    pub reason: String,
}

impl Trigger {
    pub fn new(kind: &'static str, player_id: usize) -> Self {
        Self {
            kind,
            player_id: player_id as i32,
            target: player_id as i32,
            tile: -1,
            card: String::new(),
            value: 0,
            step: 0,
            by_card: None,
            pay_is_rent: false,
            move_kind: None,
            move_resolve: false,
            move_tags: Vec::new(),
            move_main: false,
            move_dir: 1,
            negation: Negation::None,
            spared: Vec::new(),
            seq: 0,
            answers: 0,
            effects: Vec::new(),
            move_remaining: 0,
            move_total: 0,
            cards: Vec::new(),
            roll_source: 0,
            buy_kind: 0,
            seller: -1,
            price: 0,
            deal_owner: -1,
            deal_houses: 0,
            deal_mortgaged: false,
            reason: String::new(),
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

    /// Copy the move context (C# `t.Move`) onto this trigger.
    pub(crate) fn with_move(&mut self, m: &super::play::Move) -> &mut Self {
        self.move_kind = Some(m.kind);
        self.move_resolve = m.resolve;
        self.move_tags = m.tags.clone();
        self.move_main = m.main;
        self.move_dir = m.dir();
        self.move_remaining = m.remaining;
        self.move_total = m.total;
        self
    }
}

/// Raise a counteraction trigger: build a [`Trigger`], apply the optional field
/// overrides, and run it through `Cx::raise` (which stamps `step` centrally and
/// delegates to the card rules). Every raise site should go through this so the
/// shape of a trigger stays uniform.
///
/// `mv = <move>` copies the move context (C# `t.Move`) on from a `Move`, and
/// must come before any other field overrides.
///
/// The result is the trigger as `raise` left it, so a counteraction that rewrites a
/// field (e.g. `moveRoll`'s reroll) can read it back:
///
/// ```ignore
/// raise!(self, "passBefore", i, @m m, tile = next)?;
/// raise!(self, "roll", i, value = -1)?;
/// raise!(self, "card", i, card = id.to_string())?;
/// let t = raise!(self, "moveRoll", i, @m m, value = m.roll)?; // keep `t` to read back
/// ```
macro_rules! raise {
    ($cx:expr, $kind:expr, $player_id:expr $(, @m $m:expr)? $(, $field:ident = $value:expr)* $(,)?) => {{
        let mut __trigger = Trigger::new($kind, $player_id);
        $( __trigger.with_move(&$m); )?
        $( __trigger.$field = $value; )*
        $cx.raise(__trigger)
    }};
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

    /// `Card.Normal` -- may be played from hand in the operation phase.
    fn normal(&self, _card: &str) -> bool {
        true
    }

    /// `Card.WhyNot` -- extra card-specific reason it can't be played right now.
    fn cant_play(&self, _cx: &Cx, _player: usize, _card: &str) -> Option<Msg> {
        None
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

    /// `Card.AiPlay` -- would a bot play it now?
    fn ai_play(&self, _cx: &Cx, _player: usize, _card: &str) -> bool {
        true
    }

    /// `Card.Play` -- the effect. Returns where the card goes afterwards.
    fn play(&self, cx: &mut Cx, player_id: usize, card: &str) -> Flow<Dest>;

    /// Resolve a drawn event card. Returns `true` if the event stays in play
    /// (otherwise it is discarded).
    fn event(&self, cx: &mut Cx, player_id: usize, id: &str) -> Flow<bool>;

    /// Counteraction window at `t` (C# `Counteract(trigger)` -- the counteract window); may raise prompts.
    ///
    /// `t` is mutable: a counteraction may rewrite the move roll (`t.value` /
    /// `t.Move.Roll`) and the engine then uses the new face (C# shares the
    /// `MoveCtx` with the counteractions).
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
