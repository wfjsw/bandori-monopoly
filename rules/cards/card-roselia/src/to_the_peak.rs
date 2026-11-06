//!
//! 规则书（docs/rulebook/cards.json, id `R:向着顶点`）:
//! > 向着顶点：
//! >  移动到下一个可被购买的livehouse格子，视为你的主要移动。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。
//!
//! free Livehouse, or build one of yours at 1.5x cost.

use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TO_THE_PEAK: CardDef = CardDef::new("R:向着顶点", &[On::Play("", Some(cant_play), play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"向着顶点"#)
        .raw_name(r#"向着顶点"#)
        .tags(&[])
        .text(CardText::para(r#"移动到下一个可被购买的livehouse格子，视为你的主要移动。若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋。"#))
        // sheet cell E4
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

fn livehouses(player_id: i32) -> Vec<i32> {
    (0..ctx::tile_count())
        .filter(|&t| ctx::is_buyable(t) && ctx::is_color(player_id, t, 6))
        .collect()
}

fn cant_build_on(player_id: i32, t: i32) -> Option<&'static str> {
    // TODO(规则书)（为属于你的一个livehouse格子加盖一层房屋）:
    if ctx::can_build_on(player_id, t) {
        None
    } else {
        Some("blocked")
    }
}

fn cant_play(player_id: i32) -> Option<Msg> {
    let free: Vec<i32> = livehouses(player_id)
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        if rules_status::stun::get(player_id) > 0 {
            return Some(Msg::new(key!("to_the_peak_stunned")));
        }
        if rules_status::stay::get(player_id) > 0
            && bandori_std::ctx::state::get(player_id, bandori_std::state_key::UNSTOPPABLE) <= 0
        {
            return Some(Msg::new(key!("to_the_peak_stuck")));
        }
        return rules_status::move_blocked(player_id);
    }
    if livehouses(player_id)
        .into_iter()
        .any(|t| ctx::tile_owner(t) == player_id && cant_build_on(player_id, t).is_none())
    {
        return None;
    }
    Some(Msg::new(key!("to_the_peak_nothing")))
}

fn play(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    // 规则书: 「移动到下一个可被购买的livehouse格子」 -- unowned Livehouses, nearest ahead.
    let free: Vec<i32> = livehouses(player_id)
        .into_iter()
        .filter(|&t| ctx::tile_owner(t) < 0)
        .collect();
    if !free.is_empty() {
        let to = free
            .into_iter()
            .min_by_key(|&t| {
                // `tile_forward` is a slot difference, `t` a tile identity.
                let d = ctx::tile_forward(pos, ctx::position_of_tile(t));
                if d == 0 {
                    ctx::tile_count()
                } else {
                    d
                }
            })
            .unwrap_or(-1);
        // `Forward` 0 (the only free tile is underfoot) is taken as a full lap, as in
        // the `min_by_key` above.
        let num = ctx::tile_forward(pos, ctx::position_of_tile(to));
        let num = if num == 0 { ctx::tile_count() } else { num };
        ctx::plan::set_steps(num);
        ctx::plan::set_settles(true);
        // 规则书: 「视为你的主要移动」 -- `run_planned_move` runs the walk as the main move.
        ctx::run_planned_move(player_id);
        ctx::log(
            player_id,
            &Msg::new(key!("to_the_peak_walk"))
                .player_id("who", player_id)
                .tile("tile", to),
        );
        return Ok(());
    }
    // 规则书: 「若所有livehouse格子已被购买，可花费1.5倍价格为属于你的一个livehouse格子加盖一层房屋」
    let mut mine: Vec<i32> = Vec::new();
    for t in livehouses(player_id) {
        if cant_build_on(player_id, t).is_some() {
            continue;
        }
        // The same figure the charge below uses -- `build_price * 3 / 2` is
        // truncated, the charge is ceil-to-10, so they differ off a multiple
        // of 20 and a truncated filter would let the play abort after both
        // asks.
        if ctx::money_of(player_id) >= ceil_to(ctx::build_price(t) as i64 * 3 / 2, 10) {
            mine.push(t);
        }
    }
    if mine.is_empty() {
        ctx::log(player_id, &Msg::new(key!("to_the_peak_no_money")));
        return Ok(());
    }
    let title = Msg::new(key!("to_the_peak_ask_title"));
    let text = Msg::new(key!("to_the_peak_ask_text"));
    if !ctx::ask_yes(player_id, &title, &text)? {
        return Ok(());
    }
    let tile = ctx::ask_tile(player_id, &title, &text, &mine)?;
    // 1.5× the effective price, one ceil-to-10; `Fixed` tells the pipeline the
    // amount is the whole charge, so no percent / discount applies a second
    // time. The verb pays through `PayKind::Build` and runs
    // `buildBefore` / `houseAdded` / `buildAfter` (设计 6.5).
    let amount = ceil_to(ctx::build_price(tile) as i64 * 3 / 2, 10);
    if ctx::money_of(player_id) < amount {
        return Ok(());
    }
    if ctx::card_build_as(
        player_id,
        tile,
        card_sdk::abi::BuildCharge::Fixed,
        amount,
        0,
        0,
        true,
    ) {
        ctx::log(
            player_id,
            &Msg::new(key!("to_the_peak_build"))
                .player_id("who", player_id)
                .tile("tile", tile),
        );
    }
    Ok(())
}

fn ceil_to(x: i64, unit: i64) -> i32 {
    if unit <= 0 {
        return x as i32;
    }
    (((x + unit - 1) / unit) * unit) as i32
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
