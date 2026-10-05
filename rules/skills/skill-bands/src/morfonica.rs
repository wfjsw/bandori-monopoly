//! `skill:Morfonica:振翅高飞的练习曲`
//!
//! 规则书（band sheet, Morfonica）:
//! > （1）当你一次性失去2000以上资金时，抽一张卡。（2）[CiRCLE奖励]选择[获得]资金时
//! > 根据选择[获得]资金次数设资金量为1000，1500，2000的循环
//!
//! （1） is a `PayAfter` (or `Paid`) size test. （2） is the CiRCLE reward's money
//! option cycling 1000 / 1500 / 2000 by how many times the money option has been
//! taken -- a counter this skill owns, not the board's.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「选择[获得]资金次数」 -- how many times the money option has been taken.
const TAKEN: &str = "skill.morfonica.moneyTaken";

pub const MORFONICA: CardDef = CardDef::new("skill:Morfonica:振翅高飞的练习曲", &[
    On::Hook(&[HookKind::PayAfter], mine, after_pay),
    On::Hook(&[HookKind::CircleAffected], mine, on_circle)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「当你一次性失去2000以上资金时，抽一张卡」.
fn after_pay(player_id: i32) {
    if ctx::trigger::value() < 2000 {
        return;
    }
    // Only a *loss* counts -- `payAfter` on a payment out of this player.
    if ctx::trigger::target() == player_id {
        return;
    }
    ctx::draw(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("morfonica_draw")).i("n", ctx::trigger::value() as i64));
}

/// （2）「[CiRCLE奖励]选择[获得]资金时…设资金量为1000，1500，2000的循环」.
fn on_circle(player_id: i32) {
    let n = state::get(player_id, TAKEN) % 3;
    state::set(player_id, TAKEN, state::get(player_id, TAKEN) + 1);
    let amount = match n {
        0 => 1000,
        1 => 1500,
        _ => 2000,
    };
    ctx::gain(player_id, amount, &Msg::new(key!("morfonica_circle")).i("n", amount as i64));
    // The reward is replaced by the fixed sum.
    ctx::trigger::set_cancelled();
}

// TODO(规则书): （2）「[CiRCLE奖励]选择[获得]资金时」 -- the clause names the
//   *money* option of the CiRCLE reward specifically. `circleAffected` fires for
//   every reward (the draw option too), and the trigger carries no "which option"
//   field, so this body replaces the whole reward rather than only its money half.