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
//! tracked as a counter a play resets. 状态2's 「并获得其初始火罐」 is an
//! overlay of the borrowed face's `CardDef` numbers (`fire::overlay` /
//! `end_overlay`), keyed on the last-player-to-play latch.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// Turns since the last hand play. A play resets it.
const QUIET: &str = "skill.mutsumiActor.quiet";

/// 「上一位打出过手牌的玩家」 -- latched from `CardPlayed` so 状态2's overlay
/// can borrow that face's 「初始火罐」. -1 = nobody yet.
const LAST_PLAY: &str = "skill.mutsumiActor.lastPlay";

/// Whose face is currently overlaid (「并获得其初始火罐」), or -1.
const OVERLAID: &str = "skill.mutsumiActor.overlaid";

pub const MUTSUMI_ACTOR: CardDef = CardDef::new(
    "skill:若叶睦:天生的演员",
    &[
        On::Action(Action::TurnStartBefore(
            "actor == owner && slot('skillState') != 2",
            None,
            None,
            at_turn_start,
        )),
        On::Action(Action::CardPlayed(card_sdk::pre::MINE, None, None, on_play)),
        On::Action(Action::TurnEnd(
            card_sdk::pre::MINE,
            None,
            None,
            at_turn_end,
        )),
    ],
)
.silent(&[1]);

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
            enter_state2(player_id);
            ctx::log(player_id, &Msg::new(key!("mutsumi_actor_two")));
        }
    }
    Ok(())
}

/// 「不打出任何手牌」 -- a play resets the silence. Also latches
/// 「上一位打出过手牌的玩家」 for 状态2's fire overlay.
fn on_play(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, QUIET, 0);
    let actor = ctx::trigger::player_id();
    if actor >= 0 {
        state::set(player_id, LAST_PLAY, actor);
    }
    Ok(())
}

/// 状态2 entry: overlay the borrowed face's 「初始火罐」 (MARKERS.md §7 --
/// `fire::overlay` force-writes the borrowed `CardDef` numbers). The
/// character-cover pile stays open (see the TODO below); the fire half of
/// 「并获得其初始火罐」 is the latched player's own skill `CardDef`.
fn enter_state2(player_id: i32) {
    let last = state::get(player_id, LAST_PLAY);
    if last < 0 {
        return;
    }
    if let Some(skill) = ctx::persona_skill(last) {
        bandori_std::fire::overlay(player_id, &skill);
        state::set(player_id, OVERLAID, last);
    }
}

/// 「直至退出状态2」 -- re-declare her own face (no fire pots of her own).
fn exit_state2(player_id: i32) {
    if state::get(player_id, OVERLAID) >= 0 {
        bandori_std::fire::end_overlay(player_id);
        state::set(player_id, OVERLAID, -1);
    }
}

/// 「若回合结束时你的手牌小于等于1，退出状态2」, and the silence counting up.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        if ctx::hand_size(player_id) <= 1 {
            state::set(player_id, state_key::SKILL_STATE, 1);
            exit_state2(player_id);
            ctx::log(player_id, &Msg::new(key!("mutsumi_actor_left")));
        }
        return Ok(());
    }
    state::set(player_id, QUIET, state::get(player_id, QUIET) + 1);
    Ok(())
}

// 状态2's 「从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖于自己角色卡上
// 并获得其初始火罐」 -- the fire half (「并获得其初始火罐」) is implemented above
// as a `CardDef` overlay (`fire::overlay` / `end_overlay`), keyed on the
// last-player-to-play latch. The character-cover pile (「从角色卡堆拿取」) is
// still open.
// TODO(规则书)[judgement]: 状态2's character cover -- 「角色卡堆」 is a pile the
//   vocabulary has no read for. Two_in_one's fixed-pair swap (`set_persona` +
//   unplace/place) is the shape; here the pair is 「上一位打出过手牌的玩家」.

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_play_kani(player_id: i32) -> card_sdk::Asked {
    on_play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
