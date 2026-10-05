//! `MyGO:（soyo）混合的颜色` -- C# `CardSoyoColors` (MatchHost.cs:6705-6748):
//! place this card on one of your deeds; that tile gains every colour, and
//! rent charged there under a foreign agent is halved again.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（soyo）混合的颜色`）:
//! > （soyo）混合的颜色：
//! >  将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SOYO_COLORS: CardDef = CardDef::new("MyGO:（soyo）混合的颜色", &[
    On::Play(Some(cant_play), soyo_colors)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- C# `CardSoyoColors.WhyNot`
    // refuses the play with no deed ("你还没有地").
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("soyo_colors_why_not_deed")));
    }
    None
}

fn soyo_colors(player_id: i32) {
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() {
        return;
    }
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- C# `H.AskTileOf` over
    // `H.OwnedBy(i)`, then `H.PlaceFromPlay(c, i, tile)`.
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("soyo_colors_title")),
        &Msg::new(key!("soyo_colors_ask")),
        &mine,
    );
    ctx::set_dest(ctx::Dest::Field);
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- bound to the chosen tile.
    ctx::place_card_on(player_id, tile, "MyGO:（soyo）混合的颜色", &Msg::new(key!("soyo_colors_note")).tile("tile", tile));
    ctx::log(player_id, &Msg::new(key!("soyo_colors_placed")).tile("tile", tile).player_id("who", player_id));
    // 规则书: 「该格获得所有颜色」 -- `ALL_COLORS` is exactly that: `is_color`
    // answers true for every group. (The parenthetical -- the tile itself may
    // only be built through a matching-colour agent -- is `why_not_build_on`,
    // which reads the tile's *own* group and so already refuses.)
    ctx::set_tile_color(tile, ctx::ALL_COLORS);
    // TODO(规则书): 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的
    // 减半收费基础上额外减半」 -- the `Fx.PayMul` hook kind is in
    // (`On::Hook(&[HookKind::PayMul], …)`), but the C# guard keys on
    // `H._agentGroup` (the colour group of the 地产商 tile being settled, set
    // only inside `AgentLanding`'s half-price pass) and that has no ctx read,
    // so the body cannot tell an agent-landing charge from ordinary rent.
    // Once it is readable the body is `p.IsRent && p.tile == Tile && p.to ==
    // Player && agent_group >= 0 && agent_group != tile_group(Tile)` ->
    // `set_pay_amount(ceil_to(value / 2, 10))`.
}