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
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

fn station() -> i32 {
    ctx::tile_named("流星堂")
}

pub const ARISA_BONSAI: CardDef = CardDef::new(
    "skill:市谷有咲:盆栽爱好者",
    &[
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            declare_cap,
        ),
        On::Hook(&[HookKind::Event], other, on_event),
        On::Hook(&[HookKind::TurnEnd], mine, at_turn_end),
        On::Hook(&[HookKind::Pass], mine, on_pass),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn other(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}

/// 「初始0，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 2);
    Ok(())
}

/// （1）「每次有其他玩家在"流星堂"抽取事件时获得一个[火罐]」.
fn on_event(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::tile() != station() {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("arisa_bonsai_gain")));
    Ok(())
}

/// （2）「回合结束时如果本回合的[主要移动][经过]"流星堂"且[移动终点]不为
/// "流星堂"时抽取一个视为在"流星堂"抽取的事件并获得一个[火罐]」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    let st = station();
    if st < 0 {
        return Ok(());
    }
    // 「[经过]"流星堂"」 and 「[移动终点]不为"流星堂"」 -- the turn's main move
    // walked past the station and did not end there. `main_steps` is how far it
    // went; the endpoint is where the player now stands.
    if ctx::player_pos(player_id) == st {
        return Ok(());
    }
    if ctx::slot(player_id, "skill.arisa.passedStation") == 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, "skill.arisa.passedStation", 0);
    // 「抽取一个视为在"流星堂"抽取的事件」 -- the draw is tagged as the
    // station's, which is what the `event` hook above reads.
    ctx::draw(player_id, 1)?;
    ctx::gain_fire(player_id, 1, &Msg::new(key!("arisa_bonsai_end")));
    Ok(())
}

/// Latch for （2）: this player passed the station during a move.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::tile() != station() {
        return Ok(());
    }
    if ctx::trigger::move_is_main() {
        ctx::set_slot(player_id, "skill.arisa.passedStation", 1);
    }
    // （3）「[经过]"流星堂"时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]」 --
    // offered here, since the moment is the pass itself.
    if state::get(player_id, state_key::FIRE) < 2 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("arisa_bonsai_title")),
        &Msg::new(key!("arisa_bonsai_ask")),
    )? {
        return Ok(());
    }
    if ctx::spend_fire(player_id, 2, &Msg::new(key!("arisa_bonsai_spend"))) {
        ctx::add_band_crystals(player_id, 1, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("arisa_bonsai_crystal")));
    }
    Ok(())
}
