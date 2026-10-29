//! `MyGO:（soyo）混合的颜色` -- C# `CardSoyoColors` (MatchHost.cs:6705-6748):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（soyo）混合的颜色`）:
//! > （soyo）混合的颜色：
//! >  将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。
//!
//! place this card on one of your deeds; that tile gains every colour, and an
//! agent settle that charges rent there is halved again.

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Filter, Msg, On};

pub const SOYO_COLORS: CardDef = CardDef::new(
    "MyGO:（soyo）混合的颜色",
    &[
        On::Play("", Some(cant_play), soyo_colors),
        // 规则书: 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半」
        On::Filter(Filter::PaySplitMul("", Some(on_rent), half_again)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"混合的颜色"#)
        .raw_name(r#"（soyo）混合的颜色"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于你拥有地契的一个格子，该格获得所有颜色（该格本身不可因自有以外的颜色的地产商盖房），因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的减半收费基础上额外减半。"#))
        // sheet cell I9
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", r#"长崎素世"#)]));

/// 规则书: 「因该效果从在其他颜色的地产商格子触发结算的玩家处收费时，收费在地产商的
/// 减半收费基础上额外减半」 -- a rent-shaped charge on this card's own tile while
/// the charge comes from an agent settle. The payer is `trigger::player_id()` and
/// still stands on the agent tile the settle was triggered at; a plain landing on
/// this tile is no agent charge.
///
/// TODO(规则书)（「其他颜色」）: no colour comparison is made -- an agent of the
/// tile's own colour is halved too (BACKLOG C-Q14, still open).
fn on_rent(_player_id: i32) -> bool {
    if !ctx::is_placed() {
        return false;
    }
    if !ctx::trigger::pay_is_rent() {
        return false;
    }
    if !ctx::tile_is(ctx::player_pos(ctx::trigger::player_id()), "agent") {
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

/// 规则书: 「收费在地产商的减半收费基础上额外减半」 -- half the already
/// agent-halved charge (ceil, the pipeline's `ceil10` rounds after).
fn half_again(player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    let out = (value + 1) / 2;
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_colors_half")).i("n", value as i64),
    );
    out
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
    // `is_color` answers true for every group.
    // TODO(规则书)（该格获得所有颜色）: 颜色授予随卡结束 -- 实例离开场上时没有把
    //   ANY_COLOR 写回 0 的钩子（`destroy_instance_units` 只清标记），全色留在格子上。
    ctx::set_tile_prop(tile, card_sdk::abi::prop::ANY_COLOR, 1);
    // 规则书: 「该格本身不可因自有以外的颜色的地产商盖房」 -- the parenthetical:
    // the tile may only be built through a matching-colour agent, which is
    // `agent_colour_set`'s `buildable = tg == g` reading the tile's *own* group,
    // so the build branch already refuses.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn soyo_colors_kani(player_id: i32) -> card_sdk::Asked {
    soyo_colors(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn half_again_kani(player_id: i32, value: i32) -> i32 {
    half_again(player_id, value)
}
