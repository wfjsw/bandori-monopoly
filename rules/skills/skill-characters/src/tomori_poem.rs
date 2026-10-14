//! `skill:高松灯:诗超绊`（MyGO!!!!!）
//!
//! 规则书（skill sheet, 高松灯）:
//! > （1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）
//! > （2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家
//! > [触发结算]前强制将该玩家[传送]至自己所在的格子，然后使其投掷1d20，若出目
//! > 小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响下
//! > 的支付价格变为原本的一半。
//! > （3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在
//! > [触发结算]前将自己[传送]至那名玩家所在的格子。
//!
//! （2） and （3） are the same shape in opposite directions: someone's landing
//! is within five tiles of the other, and a pot buys a teleport onto the other's
//! square *before* the settle. The 「前后5格内」 window is `dist` <= 5.
//!
//! （2）'s follow-up -- the 1d20, the [停留] on 6 or less, and 「支付价格变为
//! 原本的一半」 -- rides on the forced landing. The half-price is a pay bend on
//! that settle, which is what `PayMul` is.

use card_sdk::abi::{state_key, AbKind, HookKind};
use ruleset_core::ctx::{self, state, trigger};
use card_sdk::{key, CardDef, Msg, On};

/// Settle this player owes at their own next turn end, from （2）.
const DUE: &str = "skill.tomoriPoem.due";

pub const TOMORI_POEM: CardDef = CardDef::new(
    "skill:高松灯:诗超绊",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap),
        On::Hook(&[HookKind::Pass], "actor == owner && is_ring(tile.id)", None, on_pass),
        // （2）/（3） applicability: someone else's landing within five tiles of
        // this player. The pot-cost split and the fire balance stay residual
        // (dynamic). `dist` / `pos` are CEL natives (docs/GUARDS.md §4.2).
        On::Hook(
            &[HookKind::SettleBefore],
            "tile.id >= 0 && actor != owner",
            Some(before_settle_applies),
            before_settle,
        ),
        On::Hook(&[HookKind::PayMul], "", Some(half), on_pay),
        On::Hook(&[HookKind::TurnEnd], "actor == owner && slot('skill.tomoriPoem.due') != 0", None, at_turn_end),
    ],
)
    .silent(&[0]);

/// 「在此技能影响下的支付价格变为原本的一半」 -- only the payments of the
/// player this skill moved.
fn half(player_id: i32) -> bool {
    ctx::is_placed() && state::get(player_id, DUE) > 0
}

/// 「初始4，上限4」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 4, 4);
    Ok(())
}

/// （1）「每次[经过]任意RiNG时获得一个[火罐]」.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    rules_status::fire::gain(player_id, 1, &Msg::new(key!("tomori_poem_gain")))?;
    Ok(())
}

/// Residual guard for [`before_settle`]: the landing is within five tiles of
/// the owner, and the (2)/(3) arm split holds.
///
/// The `dist` scan is live board geometry (docs/GUARDS.md §4.2); it stays out
/// of the CEL `pre` so the host and the IR fill cannot disagree on
/// `dist(tile.id, pos(owner))`.
///
/// The fire balance is **not** a gate: 不再迷茫 (2) 「可移除此卡上的一个
/// [奇迹水晶]以代替此次技能的火罐消耗」 lets a crystal stand in for the pots,
/// so a 0-fire owner with a crystal can still press. [`before_settle`]'s
/// `spend_fire` is what actually collects the cost (fire or the substitution).
fn before_settle_applies(player_id: i32) -> bool {
    let landing = ctx::trigger::tile();
    if landing < 0 {
        return false;
    }
    let mover = ctx::trigger::player_id();
    // （2）「当其他玩家的[移动终点]位于您前后5格内时」 -- someone else's
    // landing within five of this player.
    if mover != player_id {
        return ctx::dist(landing, ctx::player_pos(player_id)) <= 5;
    }
    // （3）「当你的[移动终点]位于其他玩家的前后五格内时」 -- this player's
    // landing within five of some other player.
    (0..ctx::player_count()).any(|p| {
        p != player_id
            && !ctx::player_out(p)
            && ctx::dist(landing, ctx::player_pos(p)) <= 5
    })
}

/// （2） and （3） -- the landing is within five tiles of the other party, and a
/// pot buys a teleport onto their square before the settle.
/// Applicability is the `pre` + [`before_settle_applies`]; this side only
/// spends the pot and teleports.
fn before_settle(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    // （3） is this player's own landing (cost 1); （2） is someone else's
    // landing near this player (cost 4). Same split as the guard.
    let (cost, theirs) = if mover == player_id {
        (1, ctx::player_pos(player_id))
    } else {
        (4, ctx::player_pos(player_id))
    };
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_poem_title")),
        &Msg::new(key!("tomori_poem_ask")).i("n", cost as i64),
    )? {
        return Ok(());
    }
    if !rules_status::fire::spend(player_id, cost, &Msg::new(key!("tomori_poem_spend")))? {
        return Ok(());
    }
    if !ctx::gate(mover, AbKind::Teleport) {
        return Ok(());
    }
    if theirs < 0 {
        return Ok(());
    }
    ctx::teleport_to(mover, theirs);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_poem_moved"))
            .player_id("who", mover)
            .tile("tile", theirs),
    );
    if cost == 4 {
        // （2）「然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的
        // 下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半」.
        let n = ctx::roll(mover, 1, 20);
        if n <= 6 {
            rules_status::stay::add(mover, 1);
            state::set(player_id, DUE, 1);
            ctx::log(
                player_id,
                &Msg::new(key!("tomori_poem_stay")).i("n", n as i64),
            );
        }
    }
    Ok(())
}

/// 「在此技能影响下的支付价格变为原本的一半」.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    trigger::set_pay_amount((amount + 1) / 2);
    Ok(())
}

/// 「并在自己的下回合结束时[触发结算]」 -- the owed settle.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, DUE, 0);
    let at = ctx::player_pos(player_id);
    if at < 0 {
        return Ok(());
    }
    ctx::card_settle_at(player_id, at, true);
    Ok(())
}

/// `characters.json` row for this skill's owner (ABI v63).
pub const CHARACTER: ruleset_core::roster::CharacterDef =
ruleset_core::roster::CharacterDef::new(r#"高松灯"#)
    .display(r#"高松灯"#)
    .band(r#"MyGO!!!!!"#)
    .color(r#"#77BBDD"#)
    .skill(r#"诗超绊"#)
    .text(r#"（1）每次[经过]任意RiNG时获得一个[火罐]（初始4，上限4）（2）当其他玩家的[移动终点]位于您前后5格内时可以选择使用4个火罐，在该玩家[触发结算]前强制将该玩家[传送]至自己所在的格子，然后使其投掷1d20，若出目小于等于6则获得一层[停留]并在自己的下回合结束时[触发结算]；在此技能影响下的支付价格变为原本的一半。（3）当你的[移动终点]位于其他玩家的前后五格内时可以选择使用1个火罐，在[触发结算]前将自己[传送]至那名玩家所在的格子。"#)
    .cn_id(r#"036"#)
    .art("")
    .costume("")
    .exclusive_cards(&[r#"不再迷茫"#])
    .preset(&[])
    .variant_of("")
    .variant_tag("");

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32) -> card_sdk::Asked {
    on_pay(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
