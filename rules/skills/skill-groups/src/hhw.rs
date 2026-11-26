//! `skill:Hello, Happy World!:传播笑容`
//!
//! 规则书（band sheet, Hello, Happy World!）:
//! > （1）向他人的格子付款时可选择支付双倍价格，若如此做，记录该玩家并使此卡获得
//! > 一个奇迹水晶。（2）被记录的玩家的格子产生收款时，移除此卡的一个奇迹水晶，记录
//! > 该次收款的价格（可累加）。（3）你进行盖房时，可从盖房花费中减去所记录的数字，
//! > 然后将记录数字归零。
//!
//! 「记录该玩家」 is one named player; 「记录该次收款的价格（可累加）」 is a running
//! total this card holds. （3） spends that total against a build.

use bandori_std::ctx::{self, state};
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "skill:Hello, Happy World!:传播笑容";
/// One held mark per doubled payer: `[X玩家标记（HHW）]`. The player's seat
/// number is in the key, so a second doubled payment keeps the first.
const MARK: &str = "skill.hhw.mark.";
/// 「记录该次收款的价格（可累加）」.
const SAVED: &str = "skill.hhw.saved";

/// The held-mark name one payer's mark lives under.
fn mark(who: i32) -> alloc::string::String {
    alloc::format!("{MARK}{who}")
}

/// Does this card still hold `who`'s mark?
fn has_mark(player_id: i32, who: i32) -> bool {
    ctx::held_count(player_id, &mark(who)) > 0
}

pub const HHW: CardDef = CardDef::new(
    "skill:Hello, Happy World!:传播笑容",
    &[
        // （1）「向他人的格子付款时」 -- another player, a rent. The
        // recorded-player / crystals scans stay residual (dynamic keys).
        //
        // Rides `PaySplitChooseBefore` (v78), restoring the rulebook PaySplitChoose position:
        // the interactive `ask_yes` + `set_pay_amount` + state/crystal writes sit
        // immediately before the pure `PaySplitChoose` filter stage.
        On::Action(Action::PaySplitChooseBefore(
            "actor == owner && target >= 0 && target != owner && pay_is_rent",
            None,
            on_pay_choose,
        )),
        // （2） a recorded player's rent collection, with a crystal to spend.
        // The collect is the trigger's `target`, not `actor` (the payer), so no
        // `condition::MINE` -- the payer may be anyone, the body filters on `target`.
        On::Action(Action::PayAfter(
            "card.placed",
            Some(after_pay_applies),
            after_pay,
        )),
        // （3） a recorded amount to spend down.
        On::Action(Action::BuildBefore(
            card_sdk::condition::MINE,
            Some(has_saved),
            before_build,
        )),
    ],
)
.rule();

/// Residual guard for [`after_pay`]: a recorded player collected a rent and
/// this card has a crystal to spend. Dynamic keys (`skill.hhw.*`), not CEL.
fn after_pay_applies(player_id: i32) -> bool {
    // The recorded player's tile **collects**: that is the pay trigger's
    // `target` (the payer rides `player_id`).
    let to = ctx::trigger::target();
    if !has_mark(player_id, to) {
        return false;
    }
    // 规则书（`Sumimi:#L11`）: 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的
    // 一个[奇迹水晶]代替」 -- the cover may pay the removal too.
    ctx::trigger::pay_is_rent() && crate::cover_crystals(player_id) >= 1
}

/// Residual guard for [`before_build`]: something recorded to spend.
fn has_saved(player_id: i32) -> bool {
    state::get(player_id, SAVED) > 0
}

/// （1）「向他人的格子付款时可选择支付双倍价格」.
/// Applicability is the entry's condition; the ask is the 「可」 opt-in.
/// Rides `PaySplitChooseBefore` (v78): the prompt and the `set_pay_amount` rewrite sit
/// at the rulebook PaySplitChoose position.
fn on_pay_choose(player_id: i32) -> card_sdk::Asked {
    let to = ctx::trigger::target();
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("hhw_title")),
        &Msg::new(key!("hhw_double")).player_id("who", to),
    )? {
        return Ok(());
    }
    let amount = ctx::trigger::value();
    ctx::trigger::set_pay_amount(amount * 2);
    // 「若如此做，获得一个那名玩家的[X玩家标记（HHW）]」 + the crystal of
    // （1）'s old wording. The mark is keyed on the payer, so a later double
    // against a different player adds a second mark instead of overwriting.
    ctx::add_held_by_name(player_id, &mark(to), 1, i32::MAX)?;
    ctx::add_crystals(1, i32::MAX)?;
    ctx::log(
        player_id,
        &Msg::new(key!("hhw_recorded")).player_id("who", to),
    );
    Ok(())
}

/// （2）「自己拥有对应[X玩家标记（HHW）]的玩家的格子[收取]资金时：移除此卡的1个
/// 对应的[X玩家标记（HHW）]，记录此次[收取]的金额（可累加）」.
fn after_pay(player_id: i32) -> card_sdk::Asked {
    // The mark of the collecting player (the guard admitted one).
    let to = ctx::trigger::target();
    ctx::add_held_by_name(player_id, &mark(to), -1, i32::MAX)?;
    // 规则书: 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替」
    // -- 「此卡」 is the placed `Sumimi:#L11`, drained first.
    crate::spend_crystals(player_id, 1)?;
    let v = ctx::trigger::value();
    state::set(player_id, SAVED, state::get(player_id, SAVED) + v);
    ctx::log(player_id, &Msg::new(key!("hhw_saved")).i("n", v as i64));
    Ok(())
}

/// （3）「你进行盖房时，可从盖房花费中减去所记录的数字，然后将记录数字归零」.
fn before_build(player_id: i32) -> card_sdk::Asked {
    let saved = state::get(player_id, SAVED);
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("hhw_title")),
        &Msg::new(key!("hhw_spend")).i("n", saved as i64),
    )? {
        return Ok(());
    }
    ctx::set_build_discount(saved, 1);
    state::set(player_id, SAVED, 0);
    ctx::log(player_id, &Msg::new(key!("hhw_built")).i("n", saved as i64));
    Ok(())
}

/// Roster band row for this band skill (ABI v63).
pub const BAND: bandori_std::roster::GroupDef =
bandori_std::roster::GroupDef::new(r#"Hello, Happy World!"#)
    .short_name(r#"HHW"#)
    .color(r#"#FFDD00"#)
    .skill(r#"传播笑容"#)
    .text(r#"[自动]：
（1）自己拥有对应[X玩家标记（HHW）]的玩家的格子[收取]资金时：移除此卡的1个对应的[X玩家标记（HHW）]，记录此次[收取]的金额（可累加）。
[主动]：
（1）自己向其他玩家的格子[支付]资金时：可选择[支付]双倍金额。若如此做，获得一个那名玩家的[X玩家标记（HHW）]。
（2）自己盖房时：可依次进行以下操作：
1. 从盖房花费中减去此卡记录的金额（最低为0）。
2. 将此卡记录的金额归零。"#)
    .logo(r#"003"#);

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_pay_choose_kani(player_id: i32) -> card_sdk::Asked {
    on_pay_choose(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_pay_kani(player_id: i32) -> card_sdk::Asked {
    after_pay(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn before_build_kani(player_id: i32) -> card_sdk::Asked {
    before_build(player_id)
}
