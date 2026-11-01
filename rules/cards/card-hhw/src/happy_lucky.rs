//!
//! 规则书（docs/rulebook/cards.json, id `HHW:Happy, Lucky, Smile, Yeah！`）:
//! > Happy, Lucky, Smile, Yeah！：
//! >  投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。
//!
//! roll 1d4 mod 4 and teleport along that axis to the farthest tile of the
//! board as drawn.

use card_sdk::abi::{roll_source, MoveKind, RollPurpose};
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const HAPPY_LUCKY: CardDef = CardDef::new(
    "HHW:Happy, Lucky, Smile, Yeah！",
    &[On::Play("", Some(cant_play), play)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Happy, Lucky, Smile, Yeah！"#)
        .raw_name(r#"Happy, Lucky, Smile, Yeah！"#)
        .tags(&[])
        .text(CardText::para(r#"投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。"#))
        // sheet cell F10
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    let r = ctx::roll(player_id, 1, 4, RollPurpose::Effect, roll_source::CARD)?;
    let num = r.rem_euclid(4);
    // 规则书: 「对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子
    // （例：在弦卷豪宅骰到4则传送到商店街）」 -- the drawn board's ray in that
    // direction. The roll is 1/2/3/0 for 上/下/左/右; the axis code is
    // `(num + 3) % 4`. A ray with no tile falls back to standing still.
    let p = ctx::player_pos(player_id);
    let far = ctx::farthest_tile_on_axis(p, (num + 3) % 4);
    let to = if far >= 0 { far } else { p };
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_settles(true);
    ctx::run_planned_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("happy_lucky_moved"))
            .player_id("who", player_id)
            .i("roll", r as i64)
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
