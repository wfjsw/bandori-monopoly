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
//! `DiceRollAfter` fires for the move's face, and a card's own `ctx::roll` goes
//! through the same host call, so both land here. 「择其一执行」 means the new
//! face replaces or is discarded at the player's choice; the choice is the
//! whole point of spending the pot.

use card_sdk::abi::{roll_source, ActionKind};
use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const RAISE_EFFORT: CardDef = CardDef::new(
    "skill:和奏瑞依:一次又一次竭尽全力",
    &[
        On::Action(&[ActionKind::TurnStartBefore, ActionKind::DeckAtGameStart], "", "", None, declare_cap),
        On::Action(&[ActionKind::Pass], bandori_std::guards::OWN_START, "", None, on_pass),
        // （2）「进行任意掷骰后，可选择使用一个[火罐]再投一次」 -- only with a
        // pot to spend. `fixed_roll` is derived plan state outside the CEL
        // schema (docs/GUARDS.md §4.2c): residual guard.
        On::Action(
            &[ActionKind::DiceRollAfter],
            "actor == owner && charge(owner) >= 1", "",
            Some(roll_unfixed),
            on_roll,
        ),
    ],
)
    .silent(&[0]);

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("raise_effort_gain")))?;
    Ok(())
}

/// Residual guard for （2） -- `fixed_roll` stays here (not yet in the condition
/// vocabulary). `charge(owner) >= 1` is the pre.
fn roll_unfixed(_player_id: i32) -> bool {
    ctx::fixed_roll().is_none()
}

/// （2）「进行任意掷骰后，可选择使用一个[火罐]再投一次骰子并择其一执行」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let before = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("raise_effort_again_title")),
        &Msg::new(key!("raise_effort_again_text")).i("n", before as i64),
    )? {
        return Ok(());
    }
    if !rules_status::fire::spend(player_id, 1, &Msg::new(key!("raise_effort_spend")))? {
        return Ok(());
    }
    // 「使用火罐进行掷骰」 -- the reroll is fire-funded, so `roll_ask` with
    // `roll_source::CHARGE` raises the `Roll` chain link and 寄于指尖的执念
    // 「[反击] 当你使用火罐进行掷骰时」 rings on it.
    let again = ctx::do_move_roll_ask(player_id, roll_source::CHARGE).max(0);
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

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_roll_kani(player_id: i32) -> card_sdk::Asked {
    on_roll(player_id)
}
