//!
//! 规则书（docs/rulebook/cards.json, id `HHW:微笑巡逻队`）:
//! > 微笑巡逻队：
//! >  付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）
//!
//! on one of your tiles, then roll 3d20 and maybe build free on the tile the
//! roll points at (a different tile, which need not be yours).

use bandori_std::ctx;
use card_sdk::abi::{roll_source, BuildCharge, RollPurpose};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const SMILE_PATROL: CardDef =
    CardDef::new("HHW:微笑巡逻队", &[On::Play("", Some(cant_play), play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"微笑巡逻队"#)
        .raw_name(r#"微笑巡逻队"#)
        .tags(&[])
        .text(CardText::para(r#"付款并在任意自己的格子加盖一层房屋，投掷3d20并在投掷结果数字对应的格子额外免费加盖一层房屋（若为可建造格子）"#))
        // sheet cell F6
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // TODO(规则书)（付款并在任意自己的格子加盖一层房屋）: 规则书未给打出条件 -- 我们要求至少有一格自己的可加盖且付得起的格子才可打出，否则拒绝；免费
    // 加盖那一半其实单独打也有效。
    let money = ctx::money_of(player_id);
    for t in ctx::owned_tiles(player_id) {
        // The amount is the *effective* price (percent / discount applied).
        if ctx::can_build_on(player_id, t) && money >= ctx::build_price(t) {
            return None;
        }
    }
    Some(Msg::new(key!("x_no_buildable")))
}

fn play(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（付款）: Then the free build below is the second, separate half of the
    // clause. 规则书未给付款额 -- 我们按该格房屋表的正常造价收费 (OPEN-HHW-PATROL-PRICE).
    ctx::card_offer_build(player_id, &ctx::owned_tiles(player_id));
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // TODO(规则书)（投掷3d20）: 三个面可读作各对应一格 -- 我们只取三面之和，
    // 用它的取模定位一格并只加盖一层 (OPEN-HHW-PATROL-3D20).
    let r = ctx::roll(player_id, 3, 20, RollPurpose::Effect, roll_source::CARD)?;
    // The face is a **position**; every build read is tile-keyed.
    let position = (r - 1).rem_euclid(n);
    let tile = ctx::tile_at(position);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_patrol_rolled"))
            .player_id("who", player_id)
            .tile("tile", tile)
            .i("roll", r as i64),
    );
    // 「若为可建造格子」 -- the verb carries the whole guard: `Free` costs
    // nothing and `owner_required = false` lets an unowned / foreign tile take
    // the house, while `guard_buildable` still applies the tile-level vetoes
    // (`prop::NO_BUILD`, `prop::NO_BUILD_ABOVE`, the cap, mortgage, ...).
    let before = ctx::houses_of(tile);
    if ctx::card_build_as(player_id, tile, BuildCharge::Free, 0, 0, 0, false) {
        ctx::log(
            player_id,
            &Msg::new(key!("smile_patrol_built"))
                .tile("tile", tile)
                .i("n", ctx::houses_of(tile) as i64),
        );
    } else {
        ctx::log(
            player_id,
            &Msg::new(key!("smile_patrol_cannot_build")).tile("tile", tile),
        );
        let _ = before;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
