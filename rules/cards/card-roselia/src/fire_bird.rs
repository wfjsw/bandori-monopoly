//! `R:Fire bird` -- C# `CardFireBird` (MatchHost.cs:10446-10516): pay 1,600 and
//!
//! 规则书（docs/rulebook/cards.json, id `R:Fire bird`）:
//! > Fire bird：
//! >  支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶，自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶，自己的所有格子收费变成1.5倍，奇迹水晶耗尽时将此卡放入弃牌堆
//!
//! place this card with X self-chosen miracle crystals. While it is in play:
//! pay 400 and burn one crystal at every turn end, all your tiles charge 1.5x
//! rent, and the card hits the discard when the crystals run out.

use card_sdk::{ctx, key, CardDef, Msg};

pub const FIRE_BIRD: CardDef = CardDef {
    id: "R:Fire bird",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardFireBird.WhyNot`: 「资金不够 1,600」.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「支付1600资金」 -- the pay is the card's cost (C#
    // `H.State.seats[seat].money >= 1600`).
    if ctx::money(seat) < 1600 {
        return Some(Msg::new(key!("fire_bird_no_money")));
    }
    None
}

fn play(seat: i32) {
    // 规则书: 「支付1600资金」 -- C# `PayCtx { amount = 1600, kind = "pay", must = false }`.
    let paid = ctx::pay(seat, 1600, &Msg::new(key!("fire_bird_why")));
    if paid < 1600 {
        // TODO(规则书): the C# sets `c.Effective = false` when the payment does not
        // go through (the play is wasted); the ABI has no set_effective hook.
        return;
    }
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- C# `H.AskNumber(i, ..., 1, 10, ...)`.
    let x = ctx::ask_number(
        seat,
        &Msg::new(key!("fire_bird_ask_title")),
        &Msg::new(key!("fire_bird_ask_text")),
        1,
        10,
    );
    // 规则书: 「将此卡放置在自己场地上」 -- C# `H.PlaceFromPlay(c, -1, -1, Math.Max(1, r.value))`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "R:Fire bird", &Msg::new(key!("fire_bird_note")));
    // 规则书: 「并为其添加X（自选）个奇迹水晶」 -- the crystal charge is kept as a seat
    // token; `place_card` takes no crystal count (see the TODO below).
    let n = x.max(1);
    ctx::add_tok(seat, key!("fire_bird_crystals"), n, i32::MAX);
    ctx::log(
        seat,
        &Msg::new(key!("fire_bird_placed"))
            .seat("who", seat)
            .i("n", n as i64),
    );
    // TODO(规则书): 「并为其添加X（自选）个奇迹水晶」 -- the crystals belong on the placed
    // card (`H.PlaceFromPlay(c, -1, -1, crystals)` + `DecayCard`), not a seat token;
    // needs card_crystals on a placed card.
    // TODO(规则书): 「每回合结束时失去400资金并移除1个奇迹水晶」 -- needs the Fx.TurnEnd
    // hook (C# `DecayCard.TurnEnd` -> `CardFireBird.Burn`): `H.LoseR(Seat, 400, ...)` then
    // `AddCrystals(-1, "回合结束")`.
    // TODO(规则书): 「自己的所有格子收费变成1.5倍」 -- needs the Fx.PayMul hook (C#
    // `CardFireBird.PayMul`): rent paid to this seat `CeilTo(* 1.5, 10)`.
    // TODO(规则书): 「奇迹水晶耗尽时将此卡放入弃牌堆」 -- the same decay reaching 0
    // (`DecayCard.Decay` -> `H.Unplace(this, "discard", ...)`).
}