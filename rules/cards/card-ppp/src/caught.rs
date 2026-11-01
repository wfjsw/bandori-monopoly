//!
//! 规则书（docs/rulebook/cards.json, id `PPP:抓到了`）:
//! > 抓到了：
//! > 移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。
//!
//! ahead of the user's choice (nearest forward first), treat the walk as the
//! main move, build allowed.

use alloc::vec::Vec;
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const CAUGHT: CardDef = CardDef::new("PPP:抓到了", &[On::Play("", Some(cant_play), play)])
    .meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"抓到了"#)
            .raw_name(r#"抓到了"#)
            .tags(&[])
            .text(CardText::para(
                r#"移动到你前方一名玩家的格子，视为本回合的主要移动且可选择盖房。"#,
            ))
            // sheet cell B9
            .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]),
    );

fn cant_play(player_id: i32) -> Option<Msg> {
    if let Some(reason) = rules_status::move_blocked(player_id) {
        return Some(reason);
    }
    let pos = ctx::player_pos(player_id);
    if ctx::others(player_id)
        .iter()
        .all(|&p| ctx::player_pos(p) == pos)
    {
        return Some(Msg::new(key!("caught_none")).player_id("who", player_id));
    }
    None // playable
}

fn play(player_id: i32) -> card_sdk::Asked {
    let pos = ctx::player_pos(player_id);
    let mut list: Vec<i32> = ctx::others(player_id)
        .into_iter()
        .filter(|&p| ctx::player_pos(p) != pos)
        .collect();
    if list.is_empty() {
        return Ok(());
    }
    list.sort_by_key(|&p| ctx::tile_forward(pos, ctx::player_pos(p)));
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("caught_title")),
        &Msg::new(key!("caught_ask")).player_id("who", player_id),
        &list,
    )?;
    let steps = ctx::tile_forward(pos, ctx::player_pos(who));
    // A stale `resolve = false` on the shared plan must not skip that settle.
    ctx::plan::set_settles(true);
    ctx::plan::set_steps(steps);
    ctx::log(
        player_id,
        &Msg::new(key!("caught_target"))
            .player_id("who", player_id)
            .player_id("target", who)
            .i("n", steps as i64),
    );
    ctx::run_planned_move(player_id);
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
