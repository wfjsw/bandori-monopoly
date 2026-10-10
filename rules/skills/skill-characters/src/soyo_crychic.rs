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
        // 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付」 -- the
        // body-top applicability checks are the condition now: another player
        // (`actor != owner`, not `MINE` -- the old `mine` guard was inverted
        // against the body and the rulebook), a tile we own, and a positive
        // amount. The opt-in ask stays in the body (a player choice).
        On::Hook(
            &[HookKind::PayChoose],
            "actor != owner && tile.owner == owner && value > 0",
            None,
            on_pay,
        ),
        // 「下回合的主要移动变为传送至…那格」 -- the owner's own plan consumes
        // the armed one-shot (`MINE` = `turn_player == owner` on `RollPlan`).
        // `skill.soyoCrychic.owed` is not in SLOT_NAMES, so the latch is the
        // residual guard.
        On::Hook(&[HookKind::RollPlan], card_sdk::pre::MINE, Some(on_plan_owed), on_plan),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「当其他玩家在属于你的格子上触发结算时，你可取消那次支付」.
/// Applicability is the entry's condition (`actor != owner && tile.owner ==
/// owner && value > 0`); the ask is the 「可」 opt-in.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
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

/// Residual guard for 「传送至…那格」 -- `skill.soyoCrychic.owed` is not in the
/// condition vocabulary's SLOT_NAMES, so the latch stays here.
fn on_plan_owed(player_id: i32) -> bool {
    state::get(player_id, OWED) >= 0
}

/// 「下回合的主要移动变为传送至…那格」 -- consumed by the next plan. The latch
/// is the residual guard (`on_plan_owed`).
fn on_plan(player_id: i32) -> card_sdk::Asked {
    let t = state::get(player_id, OWED);
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
