//! `MyGO:（soyo）混合的颜色` -- C# `CardSoyoColors` (MatchHost.cs:6705-6748):
//! place this card on one of your deeds; that tile gains every colour, and
//! rent charged there under a foreign agent is halved again.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（soyo）混合的颜色`）:
//! > （soyo）混合的颜色：
//! >  将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SOYO_COLORS: CardDef = CardDef {
    id: "MyGO:（soyo）混合的颜色",
    play: Some(soyo_colors),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- C# `CardSoyoColors.WhyNot`
    // refuses the play with no deed ("你还没有地").
    if ctx::owned_count(seat) == 0 {
        return Some(Msg::new(key!("soyo_colors_why_not_deed")));
    }
    None
}

fn soyo_colors(seat: i32) {
    let mine = ctx::owned_tiles(seat);
    if mine.is_empty() {
        return;
    }
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- C# `H.AskTileOf` over
    // `H.OwnedBy(i)`, then `H.PlaceFromPlay(c, i, tile)`.
    let tile = ctx::ask_tile(
        seat,
        &Msg::new(key!("soyo_colors_title")),
        &Msg::new(key!("soyo_colors_ask")),
        &mine,
    );
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:（soyo）混合的颜色", &Msg::new(key!("soyo_colors_note")).tile("tile", tile));
    ctx::log(seat, &Msg::new(key!("soyo_colors_placed")).tile("tile", tile).seat("who", seat));
    // TODO(规则书): 「将此卡放置于你拥有地契的一个格子」 -- the placement is bound
    // to the chosen tile (`Card.Tile`); `place_card` only attaches to the
    // seat's field, so needs field-card tile placement.
    // TODO(规则书): 「该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房）」 -- needs
    // the Fx.ExtraColor hook (C# `CardSoyoColors.ExtraColor`) so the tile
    // counts as every colour for agent builds, while the tile itself may only
    // be built through a matching-colour agent.
    // TODO(规则书): 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的
    // 减半收费基础上额外减半」 -- needs the Fx.PayMul hook (C#
    // `CardSoyoColors.PayMul` halves the agent rent again when `H._agentGroup`
    // differs from the tile's own group).
}