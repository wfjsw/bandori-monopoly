//! `Mujica:#J11` -- C# `CardJ11` (MatchHost.cs:5876-5928): a crystal-decay card
//! you may discard to stun yourself and your 1-tile neighbours.
//!
//! Not in `docs/rulebook/cards.json`; translated from the C# class.
//! C# `CardJ11 : DecayCard` (MatchHost.cs:5876-5928): placed with 2 miracle
//! crystals; on pay-choose / targeted, may discard to 1 [stun] self and the
//! players within 1 tile (and cancel the payment when stunned).
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const J11: CardDef = CardDef {
    id: "Mujica:#J11",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // C# `H.PlaceFromPlay(c, -1, -1, 2)` -- place with 2 miracle crystals.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mujica:#J11", &Msg::new(key!("j11_note")));
    ctx::log(seat, &Msg::new(key!("j11_placed")).seat("who", seat));
    // TODO(ABI): the 2 crystal count (C# `PlaceFromPlay(..., crystals: 2)`) --
    // `place_card` has no crystal argument. `DecayCard.TurnEnd` burns one
    // crystal per turn end and discards the card at 0; needs the Fx.TurnEnd
    // hook plus a per-card crystal field.
    // TODO(ABI): `PayChoose` (C# `CardJ11.PayChoose`) -- offer the discard when
    // the owner is about to pay money; needs the Fx.PayChoose hook.
    // TODO(ABI): `Targeted` (C# `CardJ11.Targeted`) -- offer the discard when a
    // card targets the owner; needs the Fx.Targeted hook.
    // The discard body itself is expressible once those hooks exist:
    //   ctx::unplace_card(seat);
    //   ctx::give_stun(seat, 1);
    //   for p in neighbours_within(seat, 1) { ctx::give_stun(p, 1); }
    // and the pay-cancel (C# `p.cancel = true` when `Me.Stunned`) is a PayCtx
    // field the ABI does not carry.
}