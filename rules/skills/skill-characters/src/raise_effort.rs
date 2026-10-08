//! `skill:和奏瑞依:一次又一次竭尽全力`
//!
//! 规则书（skill sheet, 和奏瑞依）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行
//!
//! （1） is a field event at `Pass`, and the parenthetical is this skill's to
//! state: it writes `fire`'s bounds the way every pot skill does.
//!
//! （2） 「进行任意掷骰后」 is any roll at all, not just the move dice --
//! `RollAfter` fires for the move's face, and a card's own `ctx::roll` goes
//! through the same host call, so both land here. 「择其一执行」 means the new
//! face replaces or is discarded at the player's choice; the choice is the
//! whole point of spending the pot.

use card_sdk::abi::{roll_source, state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

pub const RAISE_EFFORT: CardDef = CardDef::new(
    "skill:和奏瑞依:一次又一次竭尽全力",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], None, declare_cap, ""),
        On::Hook(&[HookKind::Pass], None, on_pass, card_sdk::pre::MINE),
        On::Hook(&[HookKind::RollAfter], None, on_roll, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("raise_effort_gain")))?;
    Ok(())
}

/// （2）「进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    if ctx::fixed_roll().is_some() {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    let before = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("raise_effort_again_title")),
        &Msg::new(key!("raise_effort_again_text")).i("n", before as i64),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("raise_effort_spend")))? {
        return Ok(());
    }
    // 「使用火罐进行掷骰」 -- the reroll is fire-funded, so `roll_ask` with
    // `roll_source::FIRE` raises the `Roll` chain link and 寄于指尖的执念
    // 「[反击] 当你使用火罐进行掷骰时」 rings on it.
    let again = ctx::do_move_roll_ask(player_id, roll_source::FIRE).max(0);
    // 「择其一执行」 -- keep whichever the player names.
    let keep = ctx::ask_pick(
        player_id,
        &Msg::new(key!("raise_effort_pick_title")),
        &Msg::new(key!("raise_effort_pick_text")),
        &[
            Msg::new(key!("raise_effort_keep_old")).i("n", before as i64),
            Msg::new(key!("raise_effort_keep_new")).i("n", again as i64),
        ],
    )?;
    let face = if keep == 1 { again } else { before };
    ctx::trigger::set_move_roll(face);
    ctx::log(
        player_id,
        &Msg::new(key!("raise_effort_kept")).i("n", face as i64),
    );
    Ok(())
}
