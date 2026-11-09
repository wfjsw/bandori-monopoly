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
use card_sdk::abi::{ActionKind, ZoneChangeCause};
use card_sdk::{key, Action, CardDef, Msg, On};

/// Turns since the last hand play. A play resets it.
const QUIET: &str = "skill.mutsumiActor.quiet";

/// 「上一位打出过手牌的玩家」 -- latched from **any** player's `CardPlayed` so
/// 状态2 can borrow that seat's character card. Stored 1-based (`seat + 1`);
/// 0 = nobody yet.
const LAST_PLAY: &str = "skill.mutsumiActor.lastPlay";

/// Whose face is currently overlaid (「并获得其初始火罐」), 1-based seat; 0 =
/// none.
const OVERLAID: &str = "skill.mutsumiActor.overlaid";

/// `skill.mutsumiActor.cover` on her persona-skill instance: the borrowed
/// seat + 1. Display state only, and it dies with the instance.
const COVER: &str = "skill.mutsumiActor.cover";

pub const MUTSUMI: CardDef = CardDef::new(
    "skill:若叶睦:天生的演员",
    &[
        On::Action(Action::TurnStartBefore(
            "actor == owner && slot('skillState') != 2",
            None,
            at_turn_start,
        )),
        // Any player's play must latch (「上一位打出过手牌的玩家」); `card.placed`
        // keeps a hand/pile copy of this skill from answering.
        On::Action(Action::CardPlayed("card.placed", None, on_play)).silent(),
        On::Action(Action::TurnEnd(
            card_sdk::condition::MINE,
            None,
            at_turn_end,
        )),
    ],
)
.rule();

fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // `slot('skillState') != 2` is the condition.
    // 「你的每6回合不打出任何手牌，在回合开始前抽1张牌」
    if state::get(player_id, QUIET) >= 6 {
        state::set(player_id, QUIET, 0);
        ctx::draw(player_id, 1)?;
        ctx::log(player_id, &Msg::new(key!("mutsumi_draw")));
    }
    // 「若你的回合开始时手牌数为5，可选择进入状态2」
    if ctx::hand_size(player_id) == 5 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("mutsumi_title")),
            &Msg::new(key!("mutsumi_enter")),
        )? {
            state::set(player_id, state_key::SKILL_STATE, 2);
            enter_state2(player_id);
            ctx::log(player_id, &Msg::new(key!("mutsumi_two")));
        }
    }
    Ok(())
}

/// 「不打出任何手牌」 -- **her own** play resets the silence. Any player's play
/// latches 「上一位打出过手牌的玩家」 for 状态2 (the seat is stored 1-based).
fn on_play(player_id: i32) -> card_sdk::Asked {
    let actor = ctx::trigger::player_id();
    if actor == player_id {
        state::set(player_id, QUIET, 0);
    }
    if actor >= 0 {
        state::set(player_id, LAST_PLAY, actor + 1);
    }
    Ok(())
}

/// 状态2 entry: 「从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖于
/// 自己角色卡上并获得其初始火罐」.
///
/// Ruling: the cover is **identity-only**. The borrowed seat keeps its card (no
/// transfer is printed); this seat's own card is re-derived from her bound
/// skill id on exit, never lost. The borrowed name goes to `set_persona`, so
/// `persona_is` / `in_group` see it while covered, but the skill binding does
/// not move (`persona_skill` still answers her own instance) -- the fire half
/// 「并获得其初始火罐」 rides `fire::overlay`, not a skill swap. The cover prop
/// is display state on that instance and dies with it.
fn enter_state2(player_id: i32) {
    let last = state::get(player_id, LAST_PLAY) - 1;
    if last < 0 {
        return;
    }
    let Some(borrowed) = bandori_std::persona(last) else {
        return;
    };
    // 「并获得其初始火罐」 -- the borrowed face's declared numbers.
    let borrowed_skill = bandori_std::persona_skill(last);
    if let Some(skill) = &borrowed_skill {
        bandori_std::fire::overlay(player_id, skill);
    }
    // The cover: mark the borrowed seat on her persona-skill instance, then
    // swap the displayed identity. (The own name is re-derived on exit: a
    // guest-interned `NameId` does not survive the body.)
    if let Some(own_skill) = bandori_std::persona_skill(player_id) {
        if let Some(uid) = ctx::find_card(player_id, &own_skill) {
            ctx::set_prop_at(uid, COVER, last + 1);
        }
    }
    ctx::set_persona(player_id, &borrowed);
    // The host's `set_persona` also binds the new name's skill rules
    // (`bind_persona_skills`). The ruling keeps the binding hers, so take the
    // freshly placed borrowed row back off the field -- otherwise its own
    // hooks (turn start, pass-tile, abnormal fire) would run for this seat.
    // Her own row never moved: `persona_skill` still answers it and the exit
    // `set_persona` re-binds nothing.
    if let Some(skill) = &borrowed_skill {
        if let Some(uid) = ctx::find_card(player_id, skill) {
            ctx::unplace_at(uid, ctx::Dest::Graveyard, ZoneChangeCause::Effect);
        }
    }
    ctx::log(
        player_id,
        &Msg::new(key!("mutsumi_cover")).card("card", borrowed.as_str()),
    );
    state::set(player_id, OVERLAID, last + 1);
}

/// Her own persona name, from the owner component of her bound skill id
/// (`skill:<persona>:<skill>`).
fn own_persona_name(player_id: i32) -> Option<alloc::string::String> {
    let skill = bandori_std::persona_skill(player_id)?;
    let mut parts = skill.split(':');
    parts.next()?; // the `skill` tag
    Some(alloc::string::String::from(parts.next()?))
}

/// 「直至退出状态2」 -- drop the cover and re-declare her own face (no fire pots
/// of her own). The own name goes back **before** `end_overlay`, which reads
/// `persona_skill` (always her own instance, covered or not).
fn exit_state2(player_id: i32) {
    if state::get(player_id, OVERLAID) <= 0 {
        return;
    }
    if let Some(own_skill) = bandori_std::persona_skill(player_id) {
        if let Some(uid) = ctx::find_card(player_id, &own_skill) {
            ctx::set_prop_at(uid, COVER, 0);
        }
    }
    // Her own name is the owner component of her skill id
    // (`skill:<persona>:<skill>`); the binding never moves, so it is still
    // her own while covered.
    if let Some(own) = own_persona_name(player_id) {
        ctx::set_persona(player_id, &own);
    }
    state::set(player_id, OVERLAID, 0);
    bandori_std::fire::end_overlay(player_id);
}

/// 「若回合结束时你的手牌小于等于1，退出状态2」, and the silence counting up.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        if ctx::hand_size(player_id) <= 1 {
            state::set(player_id, state_key::SKILL_STATE, 1);
            exit_state2(player_id);
            ctx::log(player_id, &Msg::new(key!("mutsumi_left")));
        }
        return Ok(());
    }
    state::set(player_id, QUIET, state::get(player_id, QUIET) + 1);
    Ok(())
}

// 状态2's 「从角色卡堆拿取一张上一位打出过手牌的玩家的角色卡将其覆盖于自己角色卡上
// 并获得其初始火罐」 -- 「角色卡堆」 reads through `bandori_std::persona` (the
// `persona_is` spelling, the seat's character card), the cover is
// `set_persona` + the `COVER` display prop on the persona-skill instance, and
// the fire half stays `fire::overlay` / `end_overlay` keyed on the same latch.

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
