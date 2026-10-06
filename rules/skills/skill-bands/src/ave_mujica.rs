//! `skill:Ave Mujica:假面之下的真实`
//!
//! 规则书（band sheet, Ave Mujica）:
//! > 你拥有"状态1"和"状态2"两种状态，初始处于状态1。处于状态2的第三回合开始时为
//! > 此卡添加一个奇迹水晶。你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己
//! > 结算，对应的其他玩家收支不变），处于状态2时可消耗一个奇迹水晶使你的一次[支付]
//! > 金额减半。若你的技能拥有火罐，进入状态2时将火罐补充至上限且每回合结束时失去1
//! > 火罐，回合结束后若火罐为0则退出状态2；退出状态2时，失去所有剩余火罐。
//!
//! This skill owns the 「状态1 / 状态2」 axis every Ave Mujica character skill
//! and the two CRYCHIC ones reference: `skillState` is 1 or 2, and the rules
//! below are what being in 2 *does*. A character skill that says 「进入状态2」
//! writes that value and this skill's hooks pick it up.
//!
//! 「从[收取]与[支付]的资金改为1.5倍（只对自己结算，对应的其他玩家收支不变）」
//! is a bend on *this* player's side of a transfer only -- the other party's
//! figure is untouched. That is a per-party bend, which `PayMul` carries as the
//! amount seen from one side.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// Turns spent in 状态2. 「处于状态2的第三回合开始时」 reads this.
const IN_TWO: &str = "skill.aveMujica.inTwo";
/// `skillState` as of the last check, so 「进入状态2」 can be seen as a transition.
const WAS: &str = "skill.aveMujica.was";

pub const AVE_MUJICA: CardDef = CardDef::new(
    "skill:Ave Mujica:假面之下的真实",
    &[
        On::Play(Some(can_halve), halve),
        On::Hook(&[HookKind::TurnStartBefore], |_| true, at_turn_start),
        On::Hook(&[HookKind::PayMul], in_two, bend),
        On::Hook(&[HookKind::TurnEnd], mine, at_turn_end),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「你在状态2时…」 -- the bends only apply in 状态2.
fn in_two(player_id: i32) -> bool {
    ctx::is_placed() && state::get(player_id, state_key::SKILL_STATE) == 2
}

/// 「处于状态2的第三回合开始时为此卡添加一个奇迹水晶」, and the pot refill on
/// entering 状态2.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // 「初始处于状态1」 -- restated so a fresh match starts in 1.
    if state::get(player_id, state_key::SKILL_STATE) == 0 {
        state::set(player_id, state_key::SKILL_STATE, 1);
        state::set(player_id, IN_TWO, 0);
        state::set(player_id, WAS, 1);
        return Ok(());
    }
    on_state(player_id);
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Ok(());
    }
    let n = state::get(player_id, IN_TWO) + 1;
    state::set(player_id, IN_TWO, n);
    // 「处于状态2的第三回合开始时为此卡添加一个奇迹水晶」
    if n == 3 {
        ctx::add_band_crystals(player_id, 1, i32::MAX);
        ctx::log(player_id, &Msg::new(key!("ave_mujica_crystal")));
    }
    Ok(())
}

/// 「进入状态2时将火罐补充至上限」 -- the character skills drive the entry, so
/// this card watches `skillState` for the 1 → 2 edge and refills on it. Runs at
/// the turn start and the turn end, which brackets every place a character skill
/// writes the value.
fn on_state(player_id: i32) {
    let now = state::get(player_id, state_key::SKILL_STATE);
    let was = state::get(player_id, WAS);
    state::set(player_id, WAS, now);
    if now != 2 || was == 2 {
        return;
    }
    // 「若你的技能拥有火罐，进入状态2时将火罐补充至上限」
    let cap = state::max(player_id, state_key::FIRE);
    if cap <= 0 {
        return;
    }
    state::set(player_id, state_key::FIRE, cap);
    ctx::log(player_id, &Msg::new(key!("ave_mujica_refill")));
}

/// 「你在状态2时从[收取]与[支付]的资金改为1.5倍（只对自己结算）」 -- the figure
/// seen from this player's side.
fn bend(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    // 「只对自己结算，对应的其他玩家收支不变」 -- only the side that is this
    // player moves.
    let from = ctx::trigger::player_id();
    let to = ctx::trigger::target();
    let mine = from == player_id || to == player_id;
    if !mine {
        return Ok(());
    }
    ctx::trigger::set_pay_amount((amount * 3 + 1) / 2);
    Ok(())
}

/// 「处于状态2时可消耗一个奇迹水晶使你的一次[支付]金额减半」 -- the press is the
/// halve, so it rides the pay window rather than standing alone.
fn can_halve(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Some(Msg::new(key!("ave_mujica_not_two")));
    }
    if ctx::band_crystals(player_id) < 1 {
        return Some(Msg::new(key!("ave_mujica_no_crystal")));
    }
    None
}

fn halve(player_id: i32) -> card_sdk::Asked {
    crate::spend_copy_sticker(player_id);
    let mut n = 1;
    // 「每当乐队技能需要移除[奇迹水晶]时，可移除「#L11」上的一个[奇迹水晶]代替」
    if let Some(uid) = ctx::find_card(player_id, "Sumimi:#L11") {
        while n > 0 && ctx::crystals_at(uid) > 0 {
            ctx::add_crystals_at(uid, -1, i32::MAX);
            n -= 1;
        }
    }
    if n > 0 {
        if ctx::band_crystals(player_id) < n {
            return Ok(());
        }
        ctx::add_band_crystals(player_id, -n, i32::MAX);
    }
    ctx::log(player_id, &Msg::new(key!("ave_mujica_halved")));
    Ok(())
}

/// 「每回合结束时失去1火罐，回合结束后若火罐为0则退出状态2；退出状态2时，失去
/// 所有剩余火罐」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    on_state(player_id);
    if state::get(player_id, state_key::SKILL_STATE) != 2 {
        return Ok(());
    }
    // 「每回合结束时失去1火罐」 -- only when the character skill grants pots.
    let cap = state::max(player_id, state_key::FIRE);
    if cap <= 0 {
        return Ok(());
    }
    let have = state::get(player_id, state_key::FIRE);
    if have > 0 {
        ctx::spend_fire(player_id, 1, &Msg::new(key!("ave_mujica_burn")));
    }
    // 「回合结束后若火罐为0则退出状态2；退出状态2时，失去所有剩余火罐」
    if state::get(player_id, state_key::FIRE) == 0 {
        state::set(player_id, state_key::SKILL_STATE, 1);
        state::set(player_id, IN_TWO, 0);
        let left = state::get(player_id, state_key::FIRE);
        if left > 0 {
            ctx::spend_fire(player_id, left, &Msg::new(key!("ave_mujica_exit")));
        }
        ctx::log(player_id, &Msg::new(key!("ave_mujica_left")));
    }
    Ok(())
}
