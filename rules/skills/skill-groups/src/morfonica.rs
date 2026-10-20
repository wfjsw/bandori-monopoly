//! `skill:Morfonica:振翅高飞的练习曲`
//!
//! 规则书（band sheet, Morfonica）:
//! > （1）当你一次性失去2000以上资金时，抽一张卡。（2）[CiRCLE奖励]选择[获得]资金时
//! > 根据选择[获得]资金次数设资金量为1000，1500，2000的循环
//!
//! （1） is a `PayAfter` (or `Paid`) size test. （2） is the CiRCLE reward's money
//! option cycling 1000 / 1500 / 2000 by how many times the money option has been
//! taken -- a counter this skill owns, not the board's.

use bandori_std::kind::Actions;
use bandori_std::state_key; use card_sdk::abi::{ActionKind};
use bandori_std::ctx::{self, state};
use card_sdk::{Action, CardDef, key, Msg, On};

/// 「选择[获得]资金次数」 -- how many times the money option has been taken.
const TAKEN: &str = "skill.morfonica.moneyTaken";

pub const MORFONICA: CardDef = CardDef::new(
    "skill:Morfonica:振翅高飞的练习曲",
    &[
        // （1）「一次性失去2000以上资金」 -- the size and direction of the pay
        // are the condition (`value` / `target`, docs/GUARDS.md §4.2).
        On::Action(Action::PayAfter("actor == owner && value >= 2000 && target != owner", "", None, after_pay)),
        // （2）「选择[获得]资金时」 -- `value == REWARD_MONEY` (0) is the money
        // half of the CiRCLE pick.
        On::Action(Actions::PassRewardAffected("actor == owner && value == 0", "", None, on_circle)),
    ],
);

/// （1）「当你一次性失去2000以上资金时，抽一张卡」. Applicability is the `pre`;
/// this side only draws.
fn after_pay(player_id: i32) -> card_sdk::Asked {
    ctx::draw(player_id, 1)?;
    ctx::log(
        player_id,
        &Msg::new(key!("morfonica_draw")).i("n", ctx::trigger::value() as i64),
    );
    Ok(())
}

/// （2）「[CiRCLE奖励]选择[获得]资金时…设资金量为1000，1500，2000的循环」 --
/// `value == 0` is the money option (the `pre`).
fn on_circle(player_id: i32) -> card_sdk::Asked {
    let n = state::get(player_id, TAKEN) % 3;
    state::set(player_id, TAKEN, state::get(player_id, TAKEN) + 1);
    let amount = match n {
        0 => 1000,
        1 => 1500,
        _ => 2000,
    };
    ctx::gain(
        player_id,
        amount,
        &Msg::new(key!("morfonica_circle")).i("n", amount as i64),
    )?;
    // The reward is replaced by the fixed sum.
    ctx::trigger::set_cancelled();
    Ok(())
}

// （2）「[CiRCLE奖励]选择[获得]资金时」 -- `passRewardAffected` now fires between the
// pick and the payout, with `t.value` = 0 (money) / 1 (card). The guard keys on
// the money half only, so the draw option is left alone.

/// Roster band row for this band skill (ABI v63).
pub const BAND: bandori_std::roster::GroupDef =
bandori_std::roster::GroupDef::new(r#"Morfonica"#)
    .short_name(r#"Mor"#)
    .color(r#"#33AAFF"#)
    .skill(r#"振翅高飞的练习曲"#)
    .text(r#"（1）当你一次性失去2000以上资金时，抽一张卡。（2）[CiRCLE奖励]选择[获得]资金时根据选择[获得]资金次数设资金量为1000，1500，2000的循环"#)
    .logo(r#"021"#);

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn after_pay_kani(player_id: i32) -> card_sdk::Asked {
    after_pay(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_circle_kani(player_id: i32) -> card_sdk::Asked {
    on_circle(player_id)
}
