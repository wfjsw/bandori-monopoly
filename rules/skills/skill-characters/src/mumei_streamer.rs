//! `skill:祐天寺若麦:大主播喵梦亲`
//!
//! 规则书（skill sheet, 祐天寺若麦）:
//! > 状态1：（1）每次其他玩家向你支付资金时，你立即获得1火罐（初始0，上限5），并
//! > 使那次支付的金额提高X*100，X为你拥有的火罐数。（2）任何时刻当你的火罐数到达
//! > 上限时，你立即进入状态2。
//! > 状态2：（1）进入状态2时，你立即将场上全部正面朝上的手卡及事件卡翻为背面朝上，
//! > 直到状态2结束为止，场上所有背面朝上的卡无法产生效果，且不受任何效果影响。你
//! > 退出状态2后，被你翻面的卡自动翻回。
//!
//! 状态1（1） is a `PayAdd` bend plus a pot: the pot is granted first, so the
//! 「X为你拥有的火罐数」 figure already counts the one just gained.
//!
//! 状态2's face-down sweep is `set_card_face_down` on every face-up placed
//! card, paired with `set_card_immune` for 「不受任何效果影响」. The other half
//! -- 「背面朝上的卡无法产生效果」 -- is a *global* rule about what a face-down
//! field card does, not something this skill can enforce from inside its own
//! hooks.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// Cards this skill flipped, so it can flip them back on exit.
const FLIPPED: &str = "skill.mumei.flipped";

pub const MUMEI_STREAMER: CardDef = CardDef::new(
    "skill:祐天寺若麦:大主播喵梦亲",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], None, declare_cap, ""),
        On::Hook(&[HookKind::PayAdd], Some(in_one), on_pay_add, ""),
        On::Hook(&[HookKind::TurnEnd], None, at_turn_end, card_sdk::pre::MINE),
        On::Hook(&[HookKind::TurnEnd], None, at_turn_end_exit, card_sdk::pre::MINE),
    ],
)
    .legacy(&[(2, legacy_mine), (3, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn in_one(player_id: i32) -> bool {
    ctx::trigger::target() == player_id && state::get(player_id, state_key::SKILL_STATE) != 2
}

/// 「初始0，上限5」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 5);
    Ok(())
}

/// 状态1（1）「每次其他玩家向你支付资金时，你立即获得1火罐…并使那次支付的金额
/// 提高X*100」.
fn on_pay_add(player_id: i32) -> card_sdk::Asked {
    let from = ctx::trigger::player_id();
    if from == player_id {
        return Ok(());
    }
    // 「你立即获得1火罐」 first, so X counts it.
    ctx::gain_fire(player_id, 1, &Msg::new(key!("mumei_gain")))?;
    // 「使那次支付的金额提高X*100，X为你拥有的火罐数」
    let x = state::get(player_id, state_key::FIRE);
    let amount = ctx::trigger::value();
    ctx::trigger::set_pay_amount(amount + x * 100);
    ctx::log(
        player_id,
        &Msg::new(key!("mumei_bump")).i("n", (x * 100) as i64),
    );
    // （2）「任何时刻当你的火罐数到达上限时，你立即进入状态2」
    if x >= state::max(player_id, state_key::FIRE) {
        enter_two(player_id);
    }
    Ok(())
}

/// The turn-end tick also catches a pot that filled outside a payment.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        return Ok(());
    }
    if state::get(player_id, state_key::FIRE) >= state::max(player_id, state_key::FIRE) {
        enter_two(player_id);
    }
    Ok(())
}

/// 状态2's entry: flip every face-up placed card face-down.
fn enter_two(player_id: i32) {
    state::set(player_id, state_key::SKILL_STATE, 2);
    ctx::log(player_id, &Msg::new(key!("mumei_two")));
    let mut n = 0;
    for (uid, _c) in ctx::field_instances(player_id) {
        if ctx::is_face_down_at(uid) {
            continue;
        }
        ctx::set_face_down_at(uid, true);
        // 「不受任何效果影响」
        ctx::set_immune_at(uid, true);
        n += 1;
    }
    state::set(player_id, FLIPPED, n);
    // 「场上所有背面朝上的卡无法产生效果」 -- the host's placed-card walk now
    // skips a card with `face_down` set (`wasm_rules`), so its `On::Hook` /
    // `On::Gate` entries stop answering on their own. The 「不受任何效果影响」
    // half is the `set_card_immune` claim above.
}

/// 「你退出状态2后，被你翻面的卡自动翻回」 -- whatever clears `skillState`
/// (the band skill's fire-pot drain) leaves 状态2 at a turn end, so the sweep
/// runs here and catches it.
fn at_turn_end_exit(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        return Ok(());
    }
    if state::get(player_id, FLIPPED) <= 0 {
        return Ok(());
    }
    for (uid, _c) in ctx::field_instances(player_id) {
        if ctx::is_immune_at(uid) {
            ctx::set_face_down_at(uid, false);
            ctx::set_immune_at(uid, false);
        }
    }
    state::set(player_id, FLIPPED, 0);
    ctx::log(player_id, &Msg::new(key!("mumei_back")));
    Ok(())
}

// TODO(规则书)[judgement]: 状态2's exit -- the sheet gives no exit condition for
//   this skill (contrast 若叶睦 「回合结束时你的手牌小于等于1，退出状态2」 and
//   三角初华's fire-pot cap). 「你退出状态2后，被你翻面的卡自动翻回」 therefore has
//   no moment of its own to fire at. Taken to mean: whatever else clears
//   `skillState` also un-flips, which is what `at_turn_end_exit` keys on.
