//! `MyGO:羽丘的不可思议女孩` -- C# `CardHaneoka` (MatchHost.cs:6495): roll 1d20
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:羽丘的不可思议女孩`）:
//! > 羽丘的不可思议女孩：
//! >  位于属于自己的格子上时，投掷1d20，若出目大于10则在当前格子免费加盖一层房屋，若出目大于15，则额外抽一张卡，大于20，则将此卡放置在自己场上，在后续任何时刻可将其置入弃牌堆并抵消一次任意付款。若严格小于10，此卡放入弃牌堆且视为此卡未生效
//!
//! on your own tile: 11+ builds a free house, 16+ draws, 20 keeps the card in
//! play so it can cancel a payment later.

use card_sdk::{ctx, key, CardDef, Msg};

pub const HANEOKA: CardDef = CardDef {
    id: "MyGO:羽丘的不可思议女孩",
    play: Some(haneoka),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// 规则书: 「位于属于自己的格子上时」 -- C# `CardHaneoka.WhyNot` refuses the
/// card off your own tile ("只有站在自己的格子上才能打出").
fn why_not(seat: i32) -> Option<Msg> {
    let pos = ctx::seat_pos(seat);
    if ctx::tile_owner(pos) != seat {
        return Some(Msg::new(key!("haneoka_why_not_own")));
    }
    None
}

fn haneoka(seat: i32) {
    let pos = ctx::seat_pos(seat);
    // 规则书: 「位于属于自己的格子上时」
    if ctx::tile_owner(pos) != seat {
        return;
    }
    // 规则书: 「投掷1d20」
    // TODO: the C# rolls with H.CardRoll (PlayCtx.Extreme can force max/min).
    let r = ctx::roll(seat, 1, 20);
    if r < 10 {
        // 规则书: 「若严格小于10，此卡放入弃牌堆且视为此卡未生效」
        ctx::log(
            seat,
            &Msg::new(key!("haneoka_no_effect"))
                .card("card", "MyGO:羽丘的不可思议女孩")
                .i("roll", r as i64),
        );
        // TODO(规则书): 「且视为此卡未生效」 -- needs PlayCtx.Effective (C#
        // `c.Effective = false`, so the play does not count as a card use).
        return;
    }
    if r > 10 {
        // 规则书: 「若出目大于10则在当前格子免费加盖一层房屋」 -- C# `H.AddHouse(seat,
        // pos, CardName)`.
        // The C# gate is `H.WhyNotBuildOn(seat, pos) == null || (own && rent &&
        // under the rent-table cap && !mortgaged)`; the own + not-mortgaged arm
        // is what we can check (`WhyNotBuildOn` and the `rent.Length - 1` cap
        // have no query).
        if ctx::tile_owner(pos) == seat && !ctx::mortgaged_of(pos) {
            let h = ctx::add_house(pos, 1);
            ctx::log(
                seat,
                &Msg::new(key!("haneoka_house"))
                    .seat("who", seat)
                    .tile("tile", pos)
                    .i("n", h as i64)
                    .card("card", "MyGO:羽丘的不可思议女孩"),
            );
        } else {
            ctx::log(
                seat,
                &Msg::new(key!("haneoka_no_house"))
                    .tile("tile", pos)
                    .card("card", "MyGO:羽丘的不可思议女孩"),
            );
        }
        // TODO(规则书): 「免费加盖一层房屋」 -- the C# `AddHouse` itself clamps at
        // the tile's rent-table max (`rent.Length - 1`) and `H.WhyNotBuildOn`
        // covers the agent-build gates; neither has a query, so the host
        // `add_house` (which only floors at 0) may overshoot the cap.
    }
    if r > 15 {
        // 规则书: 「若出目大于15，则额外抽一张卡」
        ctx::draw(seat, 1);
    }
    if r >= 20 {
        // 规则书: 「大于20，则将此卡放置在自己场上」
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(
            seat,
            "MyGO:羽丘的不可思议女孩",
            &Msg::new(key!("haneoka_placed"))
                .card("card", "MyGO:羽丘的不可思议女孩")
                .seat("who", seat),
        );
        // TODO(规则书): 「在后续任何时刻可将其置入弃牌堆并抵消一次任意付款」 -- needs
        // Fx.PayChoose (C# `CardHaneoka.PayChoose` cancels one payment of yours
        // and unplaces this card to the discard).
    }
}
