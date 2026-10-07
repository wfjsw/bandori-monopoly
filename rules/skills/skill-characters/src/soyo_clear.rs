//! `skill:长崎素世:通透的颜色`
//!
//! 规则书（skill sheet, 长崎素世）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）
//! > （2）移动掷骰后可消耗一个火罐，使自己本回合的[移动终点]对你视为对应颜色
//! > 的地产商格子，持续至你的下回合开始；若以此法单次免除了至少1500资金的
//! > [支付]，则你下次经过CiRCLE时不获得火罐。
//!
//! （2） is a re-colour of the landing tile *for this player*, which is exactly
//! `Fx.ExtraColor`. 「对应颜色」 is the colour of the 地产商 tile being settled,
//! which is what `is_live_house_for` already reads -- the clause names the
//! colour group of the agent, so the landing counts as that group.
//!
//! The penalty half -- 「若以此法单次免除了至少1500资金的[支付]，则你下次经过
//! CiRCLE时不获得火罐」 -- is a latch on a saving the re-colour actually caused.
//! What counts as 「以此法免除」 is not fully determined by the text: it says
//! the payment was avoided by *this* effect, and the engine does not currently
//! attribute a payment's reduction to one skill. See the TODO.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// The re-colour is armed for this turn's landing.
const ARMED: &str = "skill.soyoClear.armed";
/// 「下次经过CiRCLE时不获得火罐」 -- the penalty latch.
const PENALTY: &str = "skill.soyoClear.penalty";

pub const SOYO_CLEAR: CardDef = CardDef::new(
    "skill:长崎素世:通透的颜色",
    &[
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            declare_cap,
        ),
        On::Hook(&[HookKind::Pass], mine, on_pass),
        On::Hook(&[HookKind::RollAfter], mine, offer),
        On::Hook(&[HookKind::Settle], mine, on_settle),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「初始1，上限1」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 1);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」, minus the penalty latch.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    if state::get(player_id, PENALTY) != 0 {
        state::set(player_id, PENALTY, 0);
        ctx::log(player_id, &Msg::new(key!("soyo_clear_denied")));
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("soyo_clear_gain")));
    Ok(())
}

/// （2） 「移动掷骰后可消耗一个火罐」 -- the moment the face exists.
fn offer(player_id: i32) -> card_sdk::Asked {
    if ctx::fixed_roll().is_some() {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("soyo_clear_title")),
        &Msg::new(key!("soyo_clear_ask")),
    )? {
        return Ok(());
    }
    if ctx::spend_fire(player_id, 1, &Msg::new(key!("soyo_clear_spend"))) {
        state::set(player_id, ARMED, 1);
    }
    Ok(())
}

/// （2）「使自己本回合的[移动终点]对你视为对应颜色的地产商格子」 -- applied at
/// the settle, when the landing tile is known, and it wears off at the next
/// turn start (`expires: TurnStart`).
fn on_settle(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, ARMED) == 0 {
        return Ok(());
    }
    state::set(player_id, ARMED, 0);
    let t = ctx::trigger::tile();
    if t < 0 {
        return Ok(());
    }
    // 「对应颜色」 -- the colour of the 地产商 tile the settle is about. When
    // the landing *is* an agent tile that is its own group; otherwise the
    // clause has no colour to name and the re-colour is a no-op.
    let g = ctx::tile_group(t);
    if g < 0 {
        return Ok(());
    }
    ctx::set_extra_color(player_id, t, g);
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_clear_coloured")).tile("tile", t),
    );
    // TODO(规则书)[judgement]: 「若以此法单次免除了至少1500资金的[支付]，则你下次经过CiRCLE时
    //   不获得火罐」 -- the clause under-specifies -- the engine does not attribute
    //   a payment's reduction to one skill, so 「以此法免除」 has no reading that
    //   is both faithful and checkable. The penalty latch is wired in `on_pass`.
    Ok(())
}
