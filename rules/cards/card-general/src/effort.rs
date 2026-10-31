//!
//! 规则书（docs/rulebook/cards.json, id `通用:尽力后的收获`）:
//! > 尽力后的收获：
//! > [手]：
//! > 立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。
//!
//! main move becomes 1-6 steps and settles.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const EFFORT: CardDef =
    CardDef::new("通用:尽力后的收获", &[On::Play("", Some(cant_play), play)]).meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"尽力后的收获"#)
            .raw_name(r#"尽力后的收获"#)
            .tags(&[r#"手"#])
            .text(CardText::of(&[sec(
                r#"[手]："#,
                &[para(
                    r#"立刻进入移动阶段，本回合的[主要移动]改为移动1到6以内的任意整数并[结算]。"#,
                )],
            )]))
            // sheet cell A11
            .fields(&[("group", r#"通用"#), ("owner", "")]),
    );

fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    // Include the geometric landing in every label; movement counteractions
    // (including Sayo's extension) still happen afterwards.
    let max = ctx::n(1, 6);
    let dir = ctx::plan::dir();
    let options: Vec<Msg> = (1..=max)
        .map(|n| {
            Msg::new(key!("effort_steps"))
                .i("n", n as i64)
                .tile("tile", ctx::tile_steps_ahead(player_id, n * dir))
        })
        .collect();
    let steps = ctx::ask_choice(
        player_id,
        &Msg::new(key!("effort_title")),
        &Msg::new(key!("effort_ask")),
        &options,
    )? as i32
        + 1;
    let steps = steps.max(1);
    ctx::plan::set_steps(steps);
    ctx::plan::set_settles(true);
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
