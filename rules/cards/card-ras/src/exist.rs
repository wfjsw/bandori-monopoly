//! `RAS:EXIST` -- C# `CardExist` (MatchHost.cs:9321-9366).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:EXIST`）:
//! > EXIST：
//! > 将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const EXIST: CardDef = CardDef {
    id: "RAS:EXIST",
    play: Some(exist),
    can_react: None,
    react: None,
    why_not: None,
};

fn exist(seat: i32) {
    // 规则书: 「将此卡放置于自己场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "RAS:EXIST", &Msg::new(key!("exist_note")));
    ctx::log(seat, &Msg::new(key!("exist_placed")).seat("who", seat));
    // TODO(规则书): 「直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡
    // （包括其他玩家指向自身的卡）的目标将改为你」 -- needs the Fx.Redirect /
    // IRedirect hook (C# `CardExist.Redirects`) that retargets single-target cards
    // at this seat while the card is placed.
    // TODO(规则书): 「你的下回合开始时将其翻入弃牌堆」 -- needs the Fx.TurnStart
    // hook (C# `CardExist.TurnStart` -> `End()`), which unplaces the card to the
    // discard pile at the start of this seat's next turn.
    // TODO(规则书): 「若在此期间此卡没有造成影响，抽1张卡」 -- needs the redirect
    // hook to mark `Mem["used"]` (C# `CardExist.Used`); draw 1 only when it never
    // fired. `ctx::draw(seat, 1)` is ready once that flag exists.
}