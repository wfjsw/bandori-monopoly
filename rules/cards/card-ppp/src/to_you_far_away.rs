//!
//! 规则书（docs/rulebook/cards.json, id `PPP:献给远方的你`）:
//! > 献给远方的你：
//! > [手]：
//! > [传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]，然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖（例：[使用者]在#11号格子，一名玩家在#1号格子则绝对距离为10）。
//!
//! a farthest player and settle, then maybe raise a house on your farthest deed.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const TO_YOU_FAR_AWAY: CardDef =
    CardDef::new("PPP:献给远方的你", &[On::Play("", Some(cant_play), play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"献给远方的你"#)
        .raw_name(r#"献给远方的你"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[传送]至任意与[使用者]绝对距离最远的玩家的格子并[结算]，然后可以给任意与[使用者]绝对距离最远的[使用者]拥有且可盖房的格子加盖（例：[使用者]在#11号格子，一名玩家在#1号格子则绝对距离为10）。"#),
            ]),
        ]))
        // sheet cell B11
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「[传送]至任意与[使用者]绝对距离最远的玩家的格子」 -- the destination is another player's tile,
    // so the card needs at least one other player.
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("to_you_far_away_none")).player_id("who", player_id));
    }
    None // playable
}

fn play(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    let others = ctx::others(player_id);
    if others.is_empty() {
        return Ok(());
    }
    let best = others
        .iter()
        .map(|&p| ctx::dist(pos, ctx::player_pos(p)))
        .max()
        .unwrap_or(0);
    let mut far: Vec<i32> = others
        .into_iter()
        .filter(|&p| ctx::dist(pos, ctx::player_pos(p)) == best)
        .collect();
    if far.is_empty() {
        return Ok(());
    }
    let who = if far.len() > 1 {
        far.sort();
        ctx::ask_player(
            player_id,
            &Msg::new(key!("to_you_far_away_title")),
            &Msg::new(key!("to_you_far_away_ask")),
            &far,
        )?
    } else {
        far[0]
    };
    let to = ctx::player_pos(who);
    // `teleport_as` is that routine (it raises `passTile` / `passPlayer` /
    // `moveAfter` at the destination and runs the settle), and `as_main: false`
    // keeps it off the main move -- the sheet never makes it one, so the turn's roll
    // stays available (`run_planned_move` would spend it: `plan.main`, `main_moved`, step
    // -> END).
    ctx::teleport_as(player_id, to, true, false);
    ctx::log(
        player_id,
        &Msg::new(key!("to_you_far_away_moved"))
            .player_id("who", player_id)
            .player_id("target", who)
            .tile("tile", to),
    );
    if ctx::player_out(player_id) {
        return Ok(());
    }
    let at = ctx::player_pos(player_id);
    let money = ctx::money_of(player_id);
    let mine: Vec<i32> = ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::can_build_on(player_id, t) && money >= ctx::build_cost(t))
        .collect();
    let best = mine.iter().map(|&t| ctx::dist(at, t)).max().unwrap_or(0);
    let far_tiles: Vec<i32> = mine
        .into_iter()
        .filter(|&t| ctx::dist(at, t) == best)
        .collect();
    // Skips silently when none can take one.
    ctx::card_offer_build(player_id, &far_tiles);
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
