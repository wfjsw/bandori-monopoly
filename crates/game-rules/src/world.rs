//! The world a card effect can see and change.
//!
//! `game-core`'s match state implements this in the `WasmRules` bridge. The
//! `Clone` bound is what makes replay work: every run starts from a clone of the
//! snapshot, and only a run that finishes replaces the real state. The RNG
//! **must** live inside the world so that a replayed run draws the same numbers.
//!
//! The methods mirror the C# `H.*` vocabulary the card classes use. Anything
//! that needs a player decision is deliberately absent: the host surfaces it as
//! a [`crate::Prompt`] and the run is replayed with the answer.

use game_core::msg::Msg;

/// The trigger a reaction is checked against (C# `Trigger`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trigger {
    pub kind: crate::TriggerKind,
    pub seat: i32,
    /// `t.Target` (`t.Pay.to` on pay triggers).
    pub target: i32,
    /// `t.Tile` (the tile settled on, passed, mortgaged, ...).
    pub tile: i32,
    /// `t.Value` (`t.Pay.amount` on pay triggers).
    pub value: i32,
    /// `t.Move.Roll`; `None` when there is no move or it was cancelled.
    pub move_roll: Option<i32>,
    /// `t.Card` -- the card id on card/event/reacted triggers (`""` otherwise).
    pub card: String,
}

pub trait CardWorld: Clone + 'static {
    // ---------------------------------------------------------- dice & log
    /// Sum of `count` d`sides` from the match RNG; also records a dice event.
    fn roll(&mut self, seat: i32, count: i32, sides: i32) -> i32;
    /// Log line (a localizable message from the card).
    fn log(&mut self, seat: i32, msg: Msg);

    // ------------------------------------------------------------- board
    fn tile_count(&self) -> i32;
    /// Tile index of a name (a data key), or -1.
    fn tile_named(&self, name: &str) -> i32;
    fn tile_owner(&self, tile: i32) -> i32;
    fn seat_pos(&self, seat: i32) -> i32;
    /// Tile `steps` around the ring from a seat (-1 when the seat is out).
    fn tile_steps_ahead(&self, seat: i32, steps: i32) -> i32;
    fn rent_of(&self, tile: i32) -> i32;
    fn buy_price(&self, tile: i32) -> i32;
    fn build_cost(&self, tile: i32) -> i32;
    fn mortgage_value(&self, tile: i32) -> i32;
    fn owned_count(&self, seat: i32) -> i32;
    fn owned_at(&self, seat: i32, index: i32) -> i32;
    /// `TileData.IsBuyable` (a deed tile).
    fn is_buyable(&self, tile: i32) -> i32;
    /// C# `H.IsShop` -- a 商店街 deed (buyable, colour group 10).
    fn is_shop(&self, tile: i32) -> i32;
    /// `TileData.group` -- colour group (-1 for no tile).
    fn tile_group(&self, tile: i32) -> i32;
    /// `H._tiles[t].price` -- the land price alone (cf. `buy_price`).
    fn tile_price(&self, tile: i32) -> i32;
    /// `H.State.houses[t]`.
    fn houses_of(&self, tile: i32) -> i32;
    /// Set the tile's house count (house transfer effects).
    fn set_houses(&mut self, tile: i32, n: i32);
    /// `H.AddHouse` -- returns the new count.
    fn add_house(&mut self, tile: i32, n: i32) -> i32;
    /// `H.State.mortgaged[t]` (1 = mortgaged).
    fn mortgaged_of(&self, tile: i32) -> i32;
    /// Mortgage/redeem outright (transfer effects; no money moves).
    fn set_mortgaged(&mut self, tile: i32, v: i32);
    /// `H.State.owners[t] = seat` -- hand a deed over outright.
    fn set_owner(&mut self, tile: i32, seat: i32);
    /// C# `H.Dist` -- undirected ring distance.
    fn dist(&self, a: i32, b: i32) -> i32;
    /// C# `H.Forward` -- steps forward from `a` to `b`.
    fn tile_forward(&self, a: i32, b: i32) -> i32;
    /// C# `H.Neighbor` -- next present seat in direction `dir` (±1), or -1.
    fn neighbor(&self, seat: i32, dir: i32) -> i32;
    /// C# `H.SeatsOn` -- present seats on a tile, minus `except` (-1 = none).
    fn seats_on_count(&self, tile: i32, except: i32) -> i32;
    fn seats_on_at(&self, tile: i32, except: i32, index: i32) -> i32;

    // -------------------------------------------------------------- seats
    fn seat_count(&self) -> i32;
    fn seat_out(&self, seat: i32) -> i32;
    fn others_count(&self, seat: i32) -> i32;
    fn others_at(&self, seat: i32, index: i32) -> i32;
    fn money(&self, seat: i32) -> i32;
    /// `H.GainR` -- money in, logged with its reason. Returns what was gained.
    fn gain(&mut self, seat: i32, amount: i32, src: Msg) -> i32;
    /// `H.PayR` -- money out (what the seat could pay), logged.
    fn pay(&mut self, seat: i32, amount: i32, src: Msg) -> i32;

    // ------------------------------------------------------------ hand/deck
    /// `H.DrawR` -- draw `n` cards (reshuffles when needed). Returns drawn count.
    fn draw(&mut self, seat: i32, n: i32) -> i32;
    fn add_to_hand(&mut self, seat: i32, card: &str);
    fn add_to_deck(&mut self, seat: i32, card: &str, shuffle: bool);
    /// C# `H.AddToDeck(seat, card, where)` -- `pos`: 0 = top, 1 = bottom,
    /// 2 = shuffle in (the C# default).
    fn add_to_deck_at(&mut self, seat: i32, card: &str, pos: i32);
    fn to_discard(&mut self, seat: i32, card: &str);
    /// Copies of `card` in hand (C# `_hidden[s].hand` count).
    fn hand_count(&self, seat: i32, card: &str) -> i32;
    /// Copies of `card` in the discard pile.
    fn discard_count(&self, seat: i32, card: &str) -> i32;
    /// Draw-pile size.
    fn deck_count(&self, seat: i32) -> i32;
    /// Discard-pile size (all ids).
    fn discard_size(&self, seat: i32) -> i32;
    /// C# `H.DiscardFromHand` -- one copy hand -> discard; 1 when it was held.
    fn discard_from_hand(&mut self, seat: i32, card: &str) -> i32;
    /// C# `H.ShuffleAllIntoDeck` -- hand + discard into the draw pile, shuffled.
    fn sweep_to_deck(&mut self, seat: i32) -> i32;

    // -------------------------------------------------------- field cards
    /// `H.PlaceCard` -- a card (usually this one) stays in play at the seat.
    fn place_card(&mut self, seat: i32, card: &str, note: Msg);
    /// `PlayCtx.Dest` -- where this card goes when its effect finishes.
    fn set_dest(&mut self, dest: i32);
    /// `H.Unplace` -- take the card out of play (`true` when it was there).
    fn unplace_card(&mut self, seat: i32) -> bool;
    fn is_placed(&self, seat: i32) -> i32;

    // ------------------------------------------------------ marks & tokens
    /// `H.AddMark` -- a marker on a tile (`kind` names it, `note` explains it).
    fn add_mark(&mut self, tile: i32, seat: i32, kind: &str, note: Msg);
    fn count_marks(&self, tile: i32, kind: &str, owner: i32) -> i32;
    fn remove_marks(&mut self, tile: i32, kind: &str, owner: i32) -> i32;
    fn tok(&self, seat: i32, name: &str) -> i32;
    fn set_tok(&mut self, seat: i32, name: &str, value: i32);
    /// Returns how much the counter actually moved by.
    fn add_tok(&mut self, seat: i32, name: &str, n: i32, max: i32) -> i32;

    // -------------------------------------------------- per-seat slots (V)
    fn slot(&self, seat: i32, key: &str) -> i32;
    fn set_slot(&mut self, seat: i32, key: &str, value: i32);
    fn inc_slot(&mut self, seat: i32, key: &str, by: i32) -> i32;

    // ------------------------------------------------------- status pots
    fn band_crystals(&self, seat: i32) -> i32;
    fn add_band_crystals(&mut self, seat: i32, n: i32, max: i32) -> i32;
    fn fire(&self, seat: i32) -> i32;
    fn fire_max(&self, seat: i32) -> i32;
    fn gain_fire(&mut self, seat: i32, n: i32, why: Msg) -> i32;
    fn give_stay(&mut self, seat: i32, n: i32);
    fn give_stun(&mut self, seat: i32, n: i32);
    fn give_exile(&mut self, seat: i32, n: i32, to: i32);
    fn give_extra_turn(&mut self, seat: i32);
    /// `H.CanPay` -- not out, not stunned, not exiled.
    fn can_pay(&self, seat: i32) -> i32;
    /// C# `H.SpendFire` -- spend `n` [火罐]; 1 when the seat had enough (logged).
    fn spend_fire(&mut self, seat: i32, n: i32, why: Msg) -> i32;
    /// `[停留]` layers (C# `H.State.seats[s].stay`).
    fn stay_of(&self, seat: i32) -> i32;
    /// `[晕眩]` layers (C# `H.State.seats[s].stun`).
    fn stun_of(&self, seat: i32) -> i32;
    /// `H.State.turn` -- whose turn (-1 when none).
    fn turn_seat(&self) -> i32;
    /// `H.State.round`.
    fn round_no(&self) -> i32;
    /// C# `H.TurnKey` -- `round * 100 + turn + 1` (once-per-turn latches).
    fn turn_key(&self) -> i32;
    /// Is the seat's character exactly `name`?
    fn character_is(&self, seat: i32, name: &str) -> i32;
    /// C# `H.BandOf(seat) == name`.
    fn in_band(&self, seat: i32, name: &str) -> i32;

    // --------------------------------------------------- ring & movement
    /// `H.RingMultiplier` -- the RiNG rent multiplier right now.
    fn ring_multiplier(&self) -> i32;
    /// `H._ringBonus` -- nudge the RiNG multiplier; returns the new value.
    fn add_ring_bonus(&mut self, n: i32) -> i32;
    /// `H.ForceTeleport(..., resolve: false)` -- move a seat without settling.
    fn teleport_to(&mut self, seat: i32, tile: i32);

    // ---------------------------------------------------------- trigger
    fn trigger(&self) -> Trigger;
    fn set_trigger_move_roll(&mut self, roll: i32);
    /// `t.Card == id` -- is this trigger about that card?
    fn trig_card_is(&self, id: &str) -> i32;
}