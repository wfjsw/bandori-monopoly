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
//! 状态1（1） is a `PaySplitAdd` bend plus a pot: the pot is granted first, so the
//! 「X为你拥有的火罐数」 figure already counts the one just gained.
//!
//! 状态2's face-down sweep is `set_card_face_down` on every face-up placed
//! card, paired with `set_card_immune` for 「不受任何效果影响」. The other half
//! -- 「背面朝上的卡无法产生效果」 -- is a *global* rule about what a face-down
//! field card does, not something this skill can enforce from inside its own
//! hooks.

use bandori_std::state_key; use card_sdk::abi::{ActionKind, FilterKind};
use bandori_std::ctx::{self, state};
use card_sdk::{Action, CardDef, Filter, key, Msg, On, PayAddValue};

/// Cards this skill flipped, so it can flip them back on exit.
const FLIPPED: &str = "skill.mumei.flipped";
/// Post-gain pot count latched at the payment declaration, so the pure
/// `PaySplitAdd` filter sees the post-gain X even when the gain just filled the pot
/// (and so entered 状态2 before the filter's own condition is read).
const BUMP_X: &str = "skill.mumei.bumpX";

pub const MUMEI_STREAMER: CardDef = CardDef::new(
    "skill:祐天寺若麦:大主播喵梦亲",
    &[
        On::Action(Action::TurnStartBefore("", None, None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, None, declare_cap)),
        // 状态1（1）「你立即获得1火罐」 + （2） enter 状态2 at the cap -- both are
        // mutating, so they ride `ActionKind::Effect` (the payment declaration,
        // before `PaySplitAdd`). The post-gain X is latched for the pure filter.
        On::Action(Action::Effect("target == owner && actor != owner && slot('skillState') != 2", None, None, on_pay_declare)),
        // 状态1（1）「并使那次支付的金额提高X*100」 -- pure amount reshape.
        On::Filter(Filter::PaySplitAdd("target == owner && actor != owner && slot('skill.mumei.bumpX') >= 0", None, on_pay_add)),
        // Drop the one-shot latch once the payment is done (split legs all see
        // the same X).
        On::Action(Action::PayAfter("actor == owner && slot('skill.mumei.bumpX') >= 0", None, None, clear_bump)),
        On::Action(Action::TurnEnd("actor == owner && slot('skillState') != 2", None, Some(fire_at_cap), at_turn_end)),
        // 「你退出状态2后，被你翻面的卡自动翻回」 -- only while out of 状态2
        // with a non-empty flip list. Both are player-state reads (`skillState`
        // and the `skill.mumei.flipped` count), so the entry does not activate
        // (and so does not flash) when there is nothing to sweep.
        On::Action(Action::TurnEnd("actor == owner && slot('skillState') != 2 && slot('skill.mumei.flipped') > 0", None, None, at_turn_end_exit)),
    ],
)
    .silent(&[0, 1, 4])
    // 「（初始0，上限5）」
    .props(&[(bandori_std::fire::FIRE_INITIAL, 0), (bandori_std::fire::FIRE_CAP, 5)]);

/// 「初始0，上限5」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// 状态1（1）「每次其他玩家向你支付资金时，你立即获得1火罐」 and （2）'s
/// cap transition. Mutating halves of the clause, riding the payment
/// declaration (`ActionKind::Effect`) so the `PaySplitAdd` filter below sees the
/// post-gain X.
fn on_pay_declare(player_id: i32) -> card_sdk::Asked {
    // 「你立即获得1火罐」 first, so X counts it.
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("mumei_gain")))?;
    let x = state::get(player_id, state_key::CHARGE);
    // Latch the post-gain X. The latch is what the pure `PaySplitAdd` filter reads;
    // it also lets that filter fire for this very payment when the gain just
    // filled the pot and （2） entered 状态2 before `PaySplitAdd` is dispatched.
    state::set(player_id, BUMP_X, x);
    // （2）「任何时刻当你的火罐数到达上限时，你立即进入状态2」
    if x >= state::max(player_id, state_key::CHARGE) {
        enter_two(player_id);
    }
    Ok(())
}

/// 状态1（1）「并使那次支付的金额提高X*100，X为你拥有的火罐数」 -- pure amount
/// reshape off the latched post-gain X. The log is allowed (not a mutation).
fn on_pay_add(player_id: i32, mut v: PayAddValue) -> PayAddValue {
    let x = state::get(player_id, BUMP_X).max(0);
    let out = v.value + x * 100;
    if x > 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("mumei_bump")).i("n", (x * 100) as i64),
        );
    }
    PayAddValue { value: out, ..v }
}

/// Drop the one-shot [`BUMP_X`] latch once the payment has settled.
fn clear_bump(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, BUMP_X, -1);
    Ok(())
}

/// Residual guard for [`at_turn_end`]: the pot is at its cap. `state::max` is
/// not in the CEL schema (docs/GUARDS.md §4.2), so the compare stays here.
fn fire_at_cap(player_id: i32) -> bool {
    state::get(player_id, state_key::CHARGE) >= state::max(player_id, state_key::CHARGE)
}

/// The turn-end tick also catches a pot that filled outside a payment.
/// `slot('skillState') != 2` is the pre and [`fire_at_cap`] the residual -- the
/// body is the transition itself.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    enter_two(player_id);
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
    // skips a card with `face_down` set (`wasm_rules`), so its `On::Action` /
    // `On::Gate` entries stop answering on their own. The 「不受任何效果影响」
    // half is the `set_card_immune` claim above.
}

/// 「你退出状态2后，被你翻面的卡自动翻回」 -- whatever clears `skillState`
/// (the band skill's fire-pot drain) leaves 状态2 at a turn end, so the sweep
/// runs here and catches it. `slot('skillState') != 2 && slot('skill.mumei.flipped') > 0`
/// is the pre.
fn at_turn_end_exit(player_id: i32) -> card_sdk::Asked {
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

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_declare_kani(player_id: i32) -> card_sdk::Asked {
    on_pay_declare(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_add_kani(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue {
    on_pay_add(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_bump_kani(player_id: i32) -> card_sdk::Asked {
    clear_bump(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_exit_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end_exit(player_id)
}
