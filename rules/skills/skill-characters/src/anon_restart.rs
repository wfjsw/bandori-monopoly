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

pub const ANON_RESTART: CardDef = CardDef::new("skill:千早爱音:重新开始", &[
    On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
    On::Hook(&[HookKind::Pass], mine, on_pass),
    On::Hook(&[HookKind::Abnormal], mine, on_abnormal)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始2，上限3」.
fn declare_cap(player_id: i32) {
    state::set_bounds(player_id, state_key::FIRE, 0, 3);
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return;
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("anon_restart_gain")));
}

/// （2）「在受到[停留][传送]效果影响并结算该效果后，可以消耗2个火罐，抽一张卡」.
fn on_abnormal(player_id: i32) {
    let kind = match trigger::abnormal_kind() {
        Some(k) => k,
        None => return,
    };
    if !matches!(kind, AbKind::Stay | AbKind::Teleport) {
        return;
    }
    if state::get(player_id, state_key::FIRE) < 2 {
        return;
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("anon_restart_title")),
        &Msg::new(key!("anon_restart_ask")),
    ) {
        return;
    }
    if !ctx::spend_fire(player_id, 2, &Msg::new(key!("anon_restart_spend"))) {
        return;
    }
    ctx::draw(player_id, 1);
}