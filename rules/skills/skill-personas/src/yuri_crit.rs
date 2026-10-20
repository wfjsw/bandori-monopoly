//! `skill:八潮瑠唯:正论暴击`
//!
//! 规则书（skill sheet, 八潮瑠唯）:
//! > （1）如果你的（2）技能或角色卡被复制，当复制者使x重置为0时，对方失去该技能（2）
//! > （抵押或赎回以外）
//! > 当你即将获得或失去资金（在所有其他资金量改动效果生效后并结果不等于0）时，
//! > roll1d20，若结果小于等于X，将X重置为0，此次获得资金翻倍或无需失去资金（收款或
//! > 付款方同样受到影响），若结果大于X，X+1。X初始为0
//!
//! The second paragraph is the playable half and is self-contained: a 1d20
//! against a counter X that starts at 0 and climbs by one on every miss. A hit
//! doubles a gain or voids a loss, and resets X. 「在所有其他资金量改动效果
//! 生效后并结果不等于0」 is the *final* figure, which is what `PaySplitChoose`
//! carries once the other bends have run.
//!
//! The first paragraph is the copy-detection half: 「如果你的（2）技能或角色卡
//! 被复制，当复制者使x重置为0时，对方失去该技能（2）」. That needs the engine to
//! say *whose* skill a running body belongs to when the body was copied, which
//! is the same provenance question as the temporary-pot cost. See the TODO.

use bandori_std::state_key; use card_sdk::abi::{ActionKind};
use bandori_std::ctx::{self, state};
use card_sdk::{Action, CardDef, key, Msg, On};

/// The counter X. 「X初始为0」.
const X: &str = "skill.yuriCrit.x";

pub const YURI_CRIT: CardDef = CardDef::new(
    "skill:八潮瑠唯:正论暴击",
    &[
        On::Action(Action::TurnStartBefore("", "", None, declare)),
        // 「当你即将获得或失去资金（在所有其他资金量改动效果生效后并结果不等于0）时」
        // -- the *final* figure after every modifier, which is what the `pay`
        // settlement hook carries (`ActionKind::Pay`, a non-filter kind whose
        // `t.value` the engine folds). ABI v74: the body rolls a die and writes
        // state, so it cannot be a pure Filter; the rewrite lives on the Action
        // as a documented exception. `pay` runs after paySplitAdd/paySplitMul/paySplitChoose/
        // paySplitAt (closer to 「在所有其他资金量改动效果生效后」 than the old
        // PaySplitChoose stage) and still sees a negative running figure for
        // 「无需失去资金」. PayAfter is too late to rewrite.
        On::Action(Action::Pay("", "", Some(mine), on_pay)),
    ],
).silent(&[0]);

fn mine(player_id: i32) -> bool {
    // 「获得或失去资金」 -- the skill fires on either side of a money move:
    // a delete (player -> game) carries `player_id = me`, a print (game ->
    // player) carries `target = me`, a pay-player carries both.
    ctx::trigger::player_id() == player_id || ctx::trigger::target() == player_id
}

/// 「X初始为0」 -- restated so a fresh match starts from zero.
/// This skill prints no fire pots: it never touches `charge` (MARKERS.md §6
/// purge -- yuri_crit stops writing CHARGE entirely; its own X key stays).
fn declare(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, X, 0);
    Ok(())
}

/// 「当你即将获得或失去资金（在所有其他资金量改动效果生效后并结果不等于0）时」.
/// Rides `On::Action(Pay)` (ABI v74 documented exception): the body rolls 1d20
/// and writes the X counter, so it is not a pure Filter. `Pay` is the
/// post-modifier settlement hook whose `t.value` the engine folds -- the
/// 「在所有其他资金量改动效果生效后」 timing the book names.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount == 0 {
        return Ok(());
    }
    let x = state::get(player_id, X);
    let n = ctx::roll(player_id, 1, 20);
    if n <= x {
        // 「若结果小于等于X，将X重置为0，此次获得资金翻倍或无需失去资金
        // （收款或付款方同样受到影响）」
        state::set(player_id, X, 0);
        let after = if amount > 0 { amount * 2 } else { 0 };
        ctx::trigger::set_pay_amount(after);
        ctx::effect(
            player_id,
            &Msg::new(key!("yuri_crit_hit"))
                .i("n", n as i64)
                .i("x", x as i64),
        );
        ctx::log(
            player_id,
            &Msg::new(key!("yuri_crit_hit"))
                .i("n", n as i64)
                .i("x", x as i64),
        );
    } else {
        // 「若结果大于X，X+1」
        state::set(player_id, X, x + 1);
        ctx::effect(
            player_id,
            &Msg::new(key!("yuri_crit_miss"))
                .i("n", n as i64)
                .i("x", (x + 1) as i64),
        );
        ctx::log(
            player_id,
            &Msg::new(key!("yuri_crit_miss"))
                .i("n", n as i64)
                .i("x", (x + 1) as i64),
        );
    }
    Ok(())
}

// TODO(规则书)[judgement]: （1）「如果你的（2）技能或角色卡被复制，当复制者使x
//   重置为0时，对方失去该技能（2）（抵押或赎回以外）」 -- the clause under-specifies
//   -- the engine does not say whose skill a running body belongs to when the
//   body was copied, so 「复制者」 has no reading that is both faithful and
//   checkable. 「（抵押或赎回以外）」 carves out a class of resets the engine
//   does not tag either.

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_kani(player_id: i32) -> card_sdk::Asked {
    declare(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32) -> card_sdk::Asked {
    on_pay(player_id)
}
