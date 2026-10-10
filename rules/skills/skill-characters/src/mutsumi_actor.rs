//! `skill:若叶睦:天生的演员`（Ave Mujica）
//!
//! 规则书（skill sheet, 若叶睦）:
//! > 状态1：你的每6回合不打出任何手牌，在回合开始前抽1张牌。若你的回合开始时手牌数
//! > 为5，可选择进入状态2。
//! > 状态2：进入该状态后，你从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖
//! > 于自己角色卡上并获得其初始火罐，直至退出状态2；若回合结束时你的手牌小于等于1，
//! > 退出状态2。
//!
//! 「状态1 / 状态2」 is the axis the Ave Mujica band skill owns.
//!
//! 状态1's 「每6回合不打出任何手牌」 is a six-turn silence on hand plays,
//! tracked as a counter a play resets. 状态2's 「覆盖于自己角色卡上」 is the
//! character swap two_in_one already does -- unplace the old skill, place the
//! new -- plus the new character's initial fire.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// Turns since the last hand play. A play resets it.
const QUIET: &str = "skill.mutsumiActor.quiet";

pub const MUTSUMI_ACTOR: CardDef = CardDef::new(
    "skill:若叶睦:天生的演员",
    &[
        On::Hook(
            &[HookKind::TurnStartBefore],
            "actor == owner && slot('skillState') != 2",
            None,
            at_turn_start,
        ),
        On::Hook(&[HookKind::CardPlayed], card_sdk::pre::MINE, None, on_play),
        On::Hook(&[HookKind::TurnEnd], card_sdk::pre::MINE, None, at_turn_end),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // `slot('skillState') != 2` is the pre.
    // 「你的每6回合不打出任何手牌，在回合开始前抽1张牌」
    if state::get(player_id, QUIET) >= 6 {
        state::set(player_id, QUIET, 0);
        ctx::draw(player_id, 1)?;
        ctx::log(player_id, &Msg::new(key!("mutsumi_actor_draw")));
    }
    // 「若你的回合开始时手牌数为5，可选择进入状态2」
    if ctx::hand_size(player_id) == 5 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("mutsumi_actor_title")),
            &Msg::new(key!("mutsumi_actor_enter")),
        )? {
            state::set(player_id, state_key::SKILL_STATE, 2);
            ctx::log(player_id, &Msg::new(key!("mutsumi_actor_two")));
        }
    }
    Ok(())
}

/// 「不打出任何手牌」 -- a play resets the silence.
fn on_play(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, QUIET, 0);
    Ok(())
}

/// 「若回合结束时你的手牌小于等于1，退出状态2」, and the silence counting up.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        if ctx::hand_size(player_id) <= 1 {
            state::set(player_id, state_key::SKILL_STATE, 1);
            ctx::log(player_id, &Msg::new(key!("mutsumi_actor_left")));
        }
        return Ok(());
    }
    state::set(player_id, QUIET, state::get(player_id, QUIET) + 1);
    Ok(())
}

// 状态2's 「从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖于自己角色卡上
// 并获得其初始火罐」 is the character swap: unplace the old skill rule, place the
// new one, write the new character's initial fire. Two_in_one already does the
// same shape for a fixed pair; here the pair is 「上一位打出过手牌的玩家」, which
// needs a last-player-to-play latch the engine does not carry.
// TODO(规则书)[judgement]: 状态2's character swap -- the clause under-specifies --
//   「上一位打出过手牌的玩家」 needs a last-player-to-play latch the engine does
//   not carry, and 「角色卡堆」 is a pile the vocabulary has no read for.
