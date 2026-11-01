//!
//! 规则书（docs/rulebook/cards.json, id `HHW:热气球演出`）:
//! > 热气球演出：
//! >  投掷4次3d20mod60并记录其结果，选择其中之一，传送至结果对应序号的格子，视为你的主要移动
//!

use alloc::vec::Vec;
use card_sdk::abi::{roll_source, MoveKind, RollPurpose};
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BALLOON_SHOW: CardDef =
    CardDef::new("HHW:热气球演出", &[On::Play("", Some(cant_play), play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"热气球演出"#)
        .raw_name(r#"热气球演出"#)
        .tags(&[])
        .text(CardText::para(r#"投掷4次3d20mod60并记录其结果，选择其中之一，传送至结果对应序号的格子，视为你的主要移动"#))
        // sheet cell F11
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    // 规则书: 「记录其结果」 -- every face is logged, and all four stay options below even
    // when two add up to the same tile.
    let mut tiles: Vec<i32> = Vec::new();
    for _ in 0..4 {
        let r = ctx::roll(player_id, 3, 20, RollPurpose::Effect, roll_source::CARD)?;
        ctx::log(
            player_id,
            &Msg::new(key!("balloon_show_face"))
                .player_id("who", player_id)
                .i("n", r as i64),
        );
        // 规则书: 「投掷4次3d20mod60并记录其结果」 -- sum mod 60 fixes the 序号
        // (a 60th face wraps to 序号 60); `n` is the fixed 60-tile board.
        let t = (r - 1).rem_euclid(n);
        tiles.push(t);
    }
    if tiles.is_empty() {
        return Ok(());
    }
    let title = Msg::new(key!("balloon_show_ask_title"));
    let text = Msg::new(key!("balloon_show_ask_text"));
    let to = ctx::ask_tile(player_id, &title, &text, &tiles)?;
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_settles(true);
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("balloon_show_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
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
