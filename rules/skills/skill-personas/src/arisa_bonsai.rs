//! `skill:市谷有咲:盆栽爱好者`
//!
//! 规则书（skill sheet, 市谷有咲）:
//! > （1）每次有其他玩家在"流星堂"抽取事件时获得一个[火罐]（初始0，上限2）
//! > （2）回合结束时如果本回合的[主要移动][经过]"流星堂"且[移动终点]不为
//! > "流星堂"时抽取一个视为在"流星堂"抽取的事件并获得一个[火罐]。
//! > （3）[经过]"流星堂"时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。
//!
//! 「流星堂」 is the tile the three clauses all name. (1) is someone *else's*
//! event draw there; (2) is this player's own pass that did not end there; (3)
//! is a press on passing it.

use card_sdk::abi::{state_key, HookKind};
use ruleset_core::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

fn station() -> i32 {
    ctx::tile_named("流星堂")
}

pub const ARISA_BONSAI: CardDef = CardDef::new(
    "skill:市谷有咲:盆栽爱好者",
    &[
        On::Action(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Action(&[HookKind::Event], "actor != owner && tile.id == tile_named('流星堂')", None, on_event),
        // （2）: the turn's main move walked past 流星堂 and did not end there,
        // and the pass latch is armed. The named tile must exist (residual).
        On::Action(
            &[HookKind::TurnEnd],
            "actor == owner && pos(owner) != tile_named('流星堂') && slot('skill.arisa.passedStation') != 0",
            Some(station_exists),
            at_turn_end,
        ),
        On::Action(&[HookKind::Pass], "actor == owner && tile.id == tile_named('流星堂')", None, on_pass),
    ],
)
    .silent(&[0]);

/// 「初始0，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 2);
    Ok(())
}

/// （1）「每次有其他玩家在"流星堂"抽取事件时获得一个[火罐]」.
fn on_event(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("arisa_bonsai_gain")))?;
    Ok(())
}

/// Residual guard for [`at_turn_end`]: 流星堂 is on the board.
fn station_exists(_player_id: i32) -> bool {
    station() >= 0
}

/// （2）「回合结束时如果本回合的[主要移动][经过]"流星堂"且[移动终点]不为
/// "流星堂"时抽取一个视为在"流星堂"抽取的事件并获得一个[火罐]」.
/// `pos(owner) != tile_named('流星堂') && slot('skill.arisa.passedStation') != 0`
/// is the pre; [`station_exists`] the residual.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, "skill.arisa.passedStation", 0);
    // 「抽取一个视为在"流星堂"抽取的事件」 -- the draw is tagged as the
    // station's, which is what the `event` hook above reads.
    ctx::draw(player_id, 1)?;
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("arisa_bonsai_end")))?;
    Ok(())
}

/// Latch for （2）: this player passed the station during a move.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::move_is_main() {
        ctx::set_slot(player_id, "skill.arisa.passedStation", 1);
    }
    // （3）「[经过]"流星堂"时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]」 --
    // offered here, since the moment is the pass itself.
    if state::get(player_id, state_key::CHARGE) < 2 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("arisa_bonsai_title")),
        &Msg::new(key!("arisa_bonsai_ask")),
    )? {
        return Ok(());
    }
    if rules_status::fire::spend(player_id, 2, &Msg::new(key!("arisa_bonsai_spend")))? {
        ctx::add_group_crystals(player_id, 1, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("arisa_bonsai_crystal")));
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_event_kani(player_id: i32) -> card_sdk::Asked {
    on_event(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

/// cfg(kani) direct-call wrapper (ppcost body-id pattern).
#[cfg(kani)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
