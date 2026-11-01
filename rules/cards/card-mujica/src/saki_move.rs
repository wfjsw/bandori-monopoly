//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:祥，移动`）:
//! > 祥，移动：
//! >  强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半
//!
//! 3 tiles in a chosen direction, settling at half price.

use card_sdk::abi::PayFactorScope;
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const SAKI_MOVE: CardDef = CardDef::new("Mujica:祥，移动", &[On::Play("", None, saki_move)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"祥，移动"#)
        .raw_name(r#"祥，移动"#)
        .tags(&[])
        .text(CardText::para(r#"强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半"#))
        // sheet cell J4
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

fn saki_move(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「强制一名玩家向你选择的方向移动3格」 -- pick the target.
    let mut candidates = ctx::others(player_id);
    // 规则书: 「（可在掷骰前选择自己以代替主要移动）」
    if rules_status::move_blocked(player_id).is_none() {
        candidates.insert(0, player_id);
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("saki_move_title")),
        &Msg::new(key!("saki_move_ask")),
        &candidates,
    )?;
    // None = designation failed, the play folds.
    let Some(hit) = ctx::target(who) else {
        return Ok(());
    };
    // 规则书: 「向你选择的方向」
    let forward = ctx::ask_choice(
        player_id,
        &Msg::new(key!("saki_move_dir_title")),
        &Msg::new(key!("saki_move_dir_ask")),
        &[
            Msg::new(key!("saki_move_forward")),
            Msg::new(key!("saki_move_backward")),
        ],
    )? == 0;
    // 规则书: 「强制一名玩家…」 -- a forced move; 「（可在掷骰前选择自己以代替主要
    // 移动）」 makes only the self-target the main move.
    ctx::plan::set_forced(true);
    ctx::plan::set_main_override(hit == player_id);
    ctx::plan::set_steps(3);
    ctx::plan::set_reverse(!forward);
    ctx::plan::set_settles(true);
    ctx::plan::multiply_pay_factor(500, PayFactorScope::ThisMove);
    ctx::log(
        player_id,
        &Msg::new(key!("saki_move_ordered"))
            .player_id("who", hit)
            .i("n", 3),
    );
    // 规则书: 「强制一名玩家向你选择的方向移动3格并[触发结算]」 -- the frame carries
    // this shape only; a turn main move that follows seeds from the clean staging.
    ctx::run_planned_move(hit);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn saki_move_kani(player_id: i32) -> card_sdk::Asked {
    saki_move(player_id)
}
