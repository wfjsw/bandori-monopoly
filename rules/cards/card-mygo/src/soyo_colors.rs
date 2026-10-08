//! `MyGO:（soyo）混合的颜色` -- C# `CardSoyoColors` (MatchHost.cs:6705-6748):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（soyo）混合的颜色`）:
//! > （soyo）混合的颜色：
//! >  将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。
//!
//! place this card on one of your deeds; that tile gains every colour, and
//! rent charged there under a foreign agent is halved again.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const SOYO_COLORS: CardDef = CardDef::new(
    "MyGO:（soyo）混合的颜色",
    &[
        On::Play("", Some(cant_play), soyo_colors),
        On::Hook(&[card_sdk::abi::HookKind::PayMul], "", Some(on_rent), half_again),
    ],
);

/// 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时」 -- rent charged
/// on this card's own tile. The card's tile 「获得所有颜色」, so any agent
/// settling it sees a "different colour" tile and the charge is halved again on
/// top of the agent's half.
fn on_rent(player_id: i32) -> bool {
    if !ctx::is_placed() {
        return false;
    }
    if !ctx::trigger::pay_is_rent() {
        return false;
    }
    if ctx::trigger::target() != player_id {
        return false;
    }
    let t = ctx::trigger::tile();
    if t < 0 {
        return false;
    }
    let Some(mine) = ctx::self_tile() else {
        return false;
    };
    mine >= 0 && t == mine
}

/// 「收费在地产商的减半收费基础上额外减半」.
fn half_again(player_id: i32) -> card_sdk::Asked {
    let v = ctx::trigger::value();
    if v <= 0 {
        return Ok(());
    }
    ctx::trigger::set_pay_amount((v + 1) / 2);
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_colors_half")).i("n", v as i64),
    );
    Ok(())
}

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- C# `CardSoyoColors.WhyNot`
    // refuses the play with no deed ("你还没有地").
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("soyo_colors_why_not_deed")));
    }
    None
}

fn soyo_colors(player_id: i32) -> card_sdk::Asked {
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() {
        return Ok(());
    }
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- C# `H.AskTileOf` over
    // `H.OwnedBy(i)`, then `H.PlaceFromPlay(c, i, tile)`.
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("soyo_colors_title")),
        &Msg::new(key!("soyo_colors_ask")),
        &mine,
    )?;
    ctx::set_dest(ctx::Dest::Field);
    // 规则书: 「将此卡放置于你拥有地契的一个格子」 -- bound to the chosen tile.
    ctx::place_card_on(
        player_id,
        tile,
        "MyGO:（soyo）混合的颜色",
        &Msg::new(key!("soyo_colors_note")).tile("tile", tile),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_colors_placed"))
            .tile("tile", tile)
            .player_id("who", player_id),
    );
    // 规则书: 「该格获得所有颜色」 -- the `anyColor` tile prop is exactly that:
    // `is_color` answers true for every group. (The parenthetical -- 「该格本身
    // 不可因自有以外的颜色的地产商盖房」, the tile itself may only be built through
    // a matching-colour agent -- is `agent_colour_set`'s `buildable = tg == g`,
    // which reads the tile's *own* group and so already refuses.)
    ctx::set_tile_prop(tile, card_sdk::abi::prop::ANY_COLOR, 1);
    // 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的
    // 减半收费基础上额外减半」 -- the agent group is the group of the tile being
    // settled; 「其他颜色」 is that group differing from this tile's.
    Ok(())
}
