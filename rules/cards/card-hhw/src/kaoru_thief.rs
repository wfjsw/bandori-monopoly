//! `HHW:（薰）怪盗hello happy` -- C# `CardKaoruThief` (MatchHost.cs:3780-3871).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（薰）怪盗hello happy`）:
//! > （薰）怪盗hello happy：
//! >  将此卡放置于场上（充能3，衰减1）并向一名玩家场上放置一个怪盗标记，你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算。此卡在场上时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算。
//!
//! Place this card in play and put a thief mark on one other player. The charge
//! (3, decaying 1) and the two in-play movement hooks are ABI TODOs below.

use card_sdk::{ctx, key, CardDef, Msg};

pub const KAORU_THIEF: CardDef = CardDef {
    id: "HHW:（薰）怪盗hello happy",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardKaoruThief.WhyNot`: refuses the play with no other player alive.
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("x_no_rival")));
    }
    None
}

fn play(seat: i32) {
    let others = ctx::others(seat);
    // 规则书: 「向一名玩家场上放置一个怪盗标记」 -- C# `H.PickTarget` over `H.Others`.
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("kaoru_thief_ask_title")),
        &Msg::new(key!("kaoru_thief_ask_text")),
        &others,
    );
    // 规则书: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "HHW:（薰）怪盗hello happy", &Msg::new(key!("kaoru_thief_note")));
    // 规则书: 「向一名玩家场上放置一个怪盗标记」
    ctx::add_tok(who, key!("kaoru_thief_tok"), 1, i32::MAX);
    ctx::log(seat, &Msg::new(key!("kaoru_thief_marked")).seat("who", who));
    // TODO(规则书): 「（充能3，衰减1）」 -- the placement's crystal charge and its
    // turn-end decay need card_crystals on a placed card (`H.PlaceFromPlay(c, -1,
    // -1, 3)` + DecayCard).
    // TODO(规则书): 「你本回合的移动阶段可以选择在经过该玩家时使自己强制停下并触发结算」
    // -- needs the Fx.PassSeat hook (C# `CardKaoruThief.PassSeat`) over the move.
    // TODO(规则书): 「此卡在场时所有在薰所在格子的人如果可以移动，则主要移动改为投掷1d2（前后）和1d10（距离）进行结算」
    // -- needs the Fx.RollPlan hook (C# `CardKaoruThief.RollPlan`).
}