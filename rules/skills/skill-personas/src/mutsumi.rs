//! `skill:若叶睦:天生的演员`（Ave Mujica）
//!
//! 规则书（skill sheet, 若叶睦）:
//! > [资源]：
//! > （1）[火罐]：初始0，上限6。
//! > [自动]：
//! > （1）自己未处于[人偶模式]，回合开始前：获得1个[火罐]。
//! > （2）自己从手牌使用卡牌时：失去所有[火罐]。
//! > （3）自己进入[人偶模式]时：依次进行以下操作：
//! > 1. 将此角色卡换为上一位打出过手牌的玩家的角色卡。
//! > 2. 获得新角色的初始[火罐]。
//! > 3. 获得1层[睦状态]。
//! > [主动]：
//! > （1）自己未处于[人偶模式]，回合开始前：可使用6个[火罐]，自己[抽卡]1张。
//! > （2）自己未处于[人偶模式]，回合开始时且手牌数为5：可选择进入[人偶模式]。
//! > [睦状态]：回合结束时，如果自己的手牌数小于等于1，则退出[人偶模式]并减少1层。
//!
//! 「状态1 / 状态2」 is the axis the Ave Mujica band skill owns.
//!
//! The fire pool is a **per-turn accumulator**: +1 before each own turn start,
//! cap 6, wiped by any own hand play. The draw costs the full six. 状态2's
//! 「并获得其初始火罐」 is an overlay of the borrowed face's `CardDef` numbers
//! (`fire::overlay` / `end_overlay`), keyed on the last-player-to-play latch.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::{ActionKind, ZoneChangeCause};
use card_sdk::{key, Action, CardDef, Msg, On};

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

/// 「可使用6个[火罐]，自己[抽卡]1张」.
const DRAW_COST: i32 = 6;

pub const MUTSUMI: CardDef = CardDef::new(
    "skill:若叶睦:天生的演员",
    &[
        On::Action(Action::DeckAtGameStart("card.placed", None, declare_cap)).silent(),
        On::Action(Action::TurnStartBefore(
            "actor == owner && slot('skillState') != 2",
            None,
            at_turn_start,
        )),
        // Any player's play must latch (「上一位打出过手牌的玩家」); `card.placed`
        // keeps a hand/pile copy of this skill from answering. The fire wipe is
        // the owner's own play only (body).
        On::Action(Action::CardPlayed("card.placed", None, on_play)).quiet_when_empty(),
        On::Action(Action::TurnEnd(
            card_sdk::condition::MINE,
            None,
            at_turn_end,
        )),
    ],
)
.rule()
// 「（初始0，上限6）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 0),
    (bandori_std::fire::FIRE_CAP, 6),
]);

/// 「初始0，上限6」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // `slot('skillState') != 2` is the condition.
    // [自动]（1）「自己未处于[人偶模式]，回合开始前：获得1个[火罐]」.
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("mutsumi_gain")))?;
    // [主动]（1）「自己未处于[人偶模式]，回合开始前：可使用6个[火罐]，自己
    // [抽卡]1张」 -- the gain above is [自动]（1） and lands first.
    if bandori_std::fire::get(player_id) >= DRAW_COST
        && bandori_std::fire::spend_skill(
            player_id,
            DRAW_COST,
            &Msg::new(key!("mutsumi_draw_cost")),
        )?
    {
        ctx::draw(player_id, 1)?;
        ctx::log(player_id, &Msg::new(key!("mutsumi_draw")));
    }
    // [主动]（2）「自己未处于[人偶模式]，回合开始时且手牌数为5：可选择进入
    // [人偶模式]」
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

/// [自动]（2）「自己从手牌使用卡牌时：失去所有[火罐]」 -- **her own** play
/// burns the pool. Any player's play latches 「上一位打出过手牌的玩家」 for 状态2
/// (the seat is stored 1-based).
fn on_play(player_id: i32) -> card_sdk::Asked {
    let actor = ctx::trigger::player_id();
    if actor == player_id {
        bandori_std::fire::regime::burn_all(player_id, &Msg::new(key!("mutsumi_burn")))?;
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

/// 「[睦状态]：回合结束时，如果自己的手牌数小于等于1，则退出[人偶模式]并减少1层」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2
        && ctx::hand_size(player_id) <= 1
    {
        state::set(player_id, state_key::SKILL_STATE, 1);
        exit_state2(player_id);
        ctx::log(player_id, &Msg::new(key!("mutsumi_left")));
    }
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

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}
