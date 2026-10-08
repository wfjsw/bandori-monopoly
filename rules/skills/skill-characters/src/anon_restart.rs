//! `skill:千早爱音:重新开始`
//!
//! 规则书（skill sheet, 千早爱音）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始2，上限3）
//! > （2）在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡
//!
//! （1） is `Pass` onto CiRCLE.
//!
//! （2） 「受到[停留][传送]效果影响并结算该效果后」 is the *outcome* of an
//! abnormal effect, which is what `abnormal` is: the settlement hook, fired
//! after the effect landed. The two kinds the clause names are `Stay` and
//! `Teleport`; the trigger carries which one in `abnormal_kind()`.

use card_sdk::abi::{state_key, AbKind, HookKind};
use card_sdk::ctx::{self, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ANON_RESTART: CardDef = CardDef::new(
    "skill:千早爱音:重新开始",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], card_sdk::pre::MINE, None, on_pass),
        On::Hook(&[HookKind::Abnormal], card_sdk::pre::MINE, None, on_abnormal),
    ],
)
    .legacy(&[(1, legacy_mine), (2, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始2，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 2, 3);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("anon_restart_gain")))?;
    Ok(())
}

/// （2）「在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡」.
fn on_abnormal(player_id: i32) -> card_sdk::Asked {
    let kind = match trigger::abnormal_kind() {
        Some(k) => k,
        None => return Ok(()),
    };
    if !matches!(kind, AbKind::Stay | AbKind::Teleport) {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 2 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("anon_restart_title")),
        &Msg::new(key!("anon_restart_ask")),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 2, &Msg::new(key!("anon_restart_spend")))? {
        return Ok(());
    }
    ctx::draw(player_id, 1)?;
    Ok(())
}
