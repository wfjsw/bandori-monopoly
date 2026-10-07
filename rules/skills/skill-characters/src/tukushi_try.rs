//! `skill:都筑诗船:尽力了吗`（CiRCLE）
//!
//! 规则书（skill sheet, 都筑诗船）:
//! > （1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限2）
//! > （2）你的主要阶段中可消耗1火罐使你与一名其他玩家接下来的两次移动掷骰先后
//! > -1d6与+1d6，若由于此效果导致移动数为0，则视为原地进行一次结算。在此技能效果
//! > 影响下触发结算时，除购买地契外支付或消耗的资金减半，若其他玩家在此技能效果
//! > 影响下触发结算时加盖了房屋，你获得相当于那层房屋造价原价的资金。
//! > （3）初始获得"Space"。
//!
//! （2） pairs two players with two opposite dice -- the first roll gets `-1d6`,
//! the second `+1d6` -- and the whole spell carries a pay bend and a build
//! kickback. The pairing is a value (「你与一名其他玩家」) and the two rolls are
//! a countdown, both of which the keyed state holds.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// The paired player, or -1.
const PARTNER: &str = "skill.tukushiTry.partner";
/// Rolls left on the pair (starts at 2).
const LEFT: &str = "skill.tukushiTry.left";
/// 「在此技能效果影响下触发结算时…」 -- the pay bend / build kickback latch.
const SPELL: &str = "skill.tukushiTry.spell";

pub const TUKUSHI_TRY: CardDef = CardDef::new(
    "skill:都筑诗船:尽力了吗",
    &[
        On::Play(Some(can_use), use_skill),
        On::Hook(
            &[HookKind::TurnStartBefore, HookKind::DeckAtGameStart],
            |_| true,
            at_start,
        ),
        On::Hook(&[HookKind::Pass], mine, on_pass),
        On::Hook(&[HookKind::RollAfter], paired, on_roll),
        On::Hook(&[HookKind::PayChoose], paired, on_pay),
        On::Hook(&[HookKind::HouseAdded], partner, on_built),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// Either half of the pair is under the spell.
fn paired(player_id: i32) -> bool {
    let p = state::get(player_id, PARTNER);
    state::get(player_id, SPELL) > 0
        && (ctx::trigger::player_id() == player_id || ctx::trigger::player_id() == p)
}

/// 「若其他玩家在此技能效果影响下触发结算时加盖了房屋」 -- the *partner* built.
fn partner(player_id: i32) -> bool {
    state::get(player_id, SPELL) > 0 && ctx::trigger::player_id() == state::get(player_id, PARTNER)
}

/// 「初始1，上限2」 + （3）「初始获得"Space"」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 2);
    let t = ctx::tile_named("Space");
    if t >= 0 && ctx::tile_owner(t) < 0 {
        ctx::set_owner(t, player_id);
        ctx::log(
            player_id,
            &Msg::new(key!("tukushi_try_space")).tile("tile", t),
        );
    }
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_circle(ctx::trigger::tile()) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("tukushi_try_gain")));
    Ok(())
}

/// （2） 「你的主要阶段中可消耗1火罐」.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("tukushi_try_no_fire")));
    }
    None
}

/// （2）「使你与一名其他玩家接下来的两次移动掷骰先后-1d6与+1d6」.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    let others: alloc::vec::Vec<i32> = (0..ctx::player_count())
        .filter(|&p| p != player_id && !ctx::player_out(p))
        .collect();
    if others.is_empty() {
        return Ok(());
    }
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("tukushi_try_title")),
        &Msg::new(key!("tukushi_try_ask")),
        &others
            .iter()
            .map(|&p| Msg::new(key!("tukushi_try_option")).player_id("who", p))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&who) = others.get(pick) else {
        return Ok(());
    };
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("tukushi_try_spend"))) {
        return Ok(());
    }
    state::set(player_id, PARTNER, who);
    state::set(player_id, LEFT, 2);
    state::set(player_id, SPELL, 1);
    state::set_expires(player_id, SPELL, card_sdk::ctx::state::TURN_END);
    ctx::log(
        player_id,
        &Msg::new(key!("tukushi_try_paired")).player_id("who", who),
    );
    Ok(())
}

/// （2）「接下来的两次移动掷骰先后-1d6与+1d6，若由于此效果导致移动数为0，则视为
/// 原地进行一次结算」.
fn on_roll(player_id: i32) -> card_sdk::Asked {
    let left = state::get(player_id, LEFT);
    if left <= 0 {
        return Ok(());
    }
    state::set(player_id, LEFT, left - 1);
    // 「先后-1d6与+1d6」 -- the first is the cut, the second the boost.
    let d = ctx::roll(player_id, 1, 6).max(0);
    let before = ctx::trigger::move_roll().unwrap_or(ctx::trigger::value());
    let after = if left == 2 { before - d } else { before + d };
    if after <= 0 {
        // 「若由于此效果导致移动数为0，则视为原地进行一次结算」
        ctx::trigger::set_move_roll(0);
        let at = ctx::player_pos(ctx::trigger::player_id());
        if at >= 0 {
            ctx::card_settle_at(ctx::trigger::player_id(), at, true);
        }
        ctx::log(player_id, &Msg::new(key!("tukushi_try_zero")));
        return Ok(());
    }
    ctx::trigger::set_move_roll(after);
    Ok(())
}

/// （2）「在此技能效果影响下触发结算时，除购买地契外支付或消耗的资金减半」.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    // 「除购买地契外」 -- a buy is not bent.
    if ctx::trigger::value() > 0 && ctx::slot(player_id, "skill.tukushiTry.isBuy") != 0 {
        return Ok(());
    }
    ctx::trigger::set_pay_amount((amount + 1) / 2);
    Ok(())
}

/// （2）「若其他玩家在此技能效果影响下触发结算时加盖了房屋，你获得相当于那层房屋
/// 造价原价的资金」.
fn on_built(player_id: i32) -> card_sdk::Asked {
    let tile = ctx::trigger::tile();
    let cost = ctx::build_cost(tile);
    if cost > 0 {
        ctx::gain(
            player_id,
            cost,
            &Msg::new(key!("tukushi_try_kickback")).i("n", cost as i64),
        )?;
    }
    Ok(())
}
