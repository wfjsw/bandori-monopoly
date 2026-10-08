//! `skill:长崎素世（CRYCHIC）:雨中祈晴`
//!
//! 规则书（skill sheet, 长崎素世（CRYCHIC））:
//! > 当其他玩家在属于你的格子上触发结算时，你可取消那次支付，使你下回合的主要移动
//! > 变为传送至触发此技能的那格。
//!
//! One clause, two halves: the pay is voided, and the next main move is a
//! teleport back to that tile. 「取消那次支付」 is the pay chain's own cancel
//! (`set_pay_amount(0)`); 「下回合的主要移动变为传送至…」 is an armed one-shot
//! the next `RollPlan` consumes -- the same shape as detour's 「下一次的移动掷骰
//! 变更为1d6」.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// The tile this skill wants to teleport back to, or -1.
const OWED: &str = "skill.soyoCrychic.owed";

pub const SOYO_CRYCHIC: CardDef = CardDef::new(
    "skill:长崎素世（CRYCHIC）:雨中祈晴",
    &[
        On::Hook(&[HookKind::PayChoose], card_sdk::pre::MINE, None, on_pay),
        On::Hook(&[HookKind::RollPlan], card_sdk::pre::MINE, None, on_plan),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付」.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    let payer = ctx::trigger::player_id();
    if payer < 0 || payer == player_id {
        return Ok(());
    }
    let t = ctx::trigger::tile();
    if t < 0 || ctx::tile_owner(t) != player_id {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("soyo_crychic_title")),
        &Msg::new(key!("soyo_crychic_ask")).tile("tile", t),
    )? {
        return Ok(());
    }
    // 「取消那次支付」
    ctx::trigger::set_pay_amount(0);
    // 「使你下回合的主要移动变为传送至触发此技能的那格」
    state::set(player_id, OWED, t);
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_crychic_cancelled")).tile("tile", t),
    );
    Ok(())
}

/// 「下回合的主要移动变为传送至…那格」 -- consumed by the next plan.
fn on_plan(player_id: i32) -> card_sdk::Asked {
    let t = state::get(player_id, OWED);
    if t < 0 {
        return Ok(());
    }
    state::set(player_id, OWED, -1);
    plan::set_kind(card_sdk::abi::MoveKind::Teleport);
    plan::set_teleport_to(t);
    plan::set_resolve(true);
    ctx::log(
        player_id,
        &Msg::new(key!("soyo_crychic_moved")).tile("tile", t),
    );
    Ok(())
}
