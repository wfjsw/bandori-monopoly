//! `skill:纯田真奈:甜甜圈爱好者`（Sumimi）
//!
//! 规则书（skill sheet, 纯田真奈）:
//! > （1）每次[经过]CiRCLE时获得1个[火罐]（初始1，上限2）
//! > （2）经过"购物中心"，"便利店"，"快餐店"，"羽泽咖啡厅"，"山吹面包房"时，在
//! > 触发结算前可选择消耗一个火罐并向对应格子进行一次支付（可向抵押格子正常支付其
//! > 原本收费，若为无主格子则为消耗对应收费资金），之后，直到你的下个回合结束，你的
//! > 资金消耗减半。
//!
//! （2） 「向对应格子进行一次支付」 is a voluntary pay to the tile's owner (or
//! the bank, when it is unowned) *before* the settle -- a pre-emptive charge
//! that buys the half-price window.
//!
//! 「直到你的下个回合结束，你的资金消耗减半」 -- the window runs through the
//! end of the owner's **next** turn, not the arming turn's. The keyed state
//! carries two `TurnEnd` layers (one lost at each of the owner's own turn-end
//! wear-offs), so an arm on turn T dies at the end of turn T+1. 「你的资金
//! 消耗减半」 also scopes to the owner's own spending: the pay bend is
//! `actor == owner`.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「你的资金消耗减半」 -- two `TurnEnd` layers, so the window runs through
/// the end of the owner's **next** turn (the arming turn's end takes the first).
const HALF: &str = "skill.manaDonut.half";
/// One `Pass` entry serves both clauses -- `CardInfo::entry` answers only the
/// **first** entry per (kind, trigger), so the two bodies share one dispatch.
/// The condition is "the body will do something": either the CiRCLE gain, or
/// the shop offer (which needs a 火罐 to spend).
const PASS_PRE: &str = "actor == owner && (tile.id == tile_named('CiRCLE') || \
     (fire(owner) >= 1 && (tile.id == tile_named('购物中心') || \
       tile.id == tile_named('便利店') || tile.id == tile_named('快餐店') || \
       tile.id == tile_named('羽泽咖啡厅') || tile.id == tile_named('山吹面包房'))))";

pub const MANA_DONUT: CardDef = CardDef::new(
    "skill:纯田真奈:甜甜圈爱好者",
    &[
        // 「初始1」 is created at game start; the cap is declared here and
        // re-declared (only when unset) at a turn start so a mid-game
        // placement still gets one. Both write only what they must.
        On::Hook(&[HookKind::DeckAtGameStart], "card.placed", None, declare_cap),
        On::Hook(
            &[HookKind::TurnStartBefore],
            "card.placed",
            Some(cap_unset),
            declare_cap,
        ),
        // （1）+（2） one `Pass` entry (see [`PASS_PRE`]); the body picks the
        // branch. The choice / spend / pay stay in the body.
        On::Hook(&[HookKind::Pass], PASS_PRE, None, on_pass),
        // 「你的资金消耗减半」 -- only the owner's own pays, and only while
        // the window is armed. `value > 0` is the amount floor.
        On::Hook(
            &[HookKind::PayChoose],
            "actor == owner && card.placed && slot('skill.manaDonut.half') > 0 && value > 0",
            None,
            on_pay,
        ),
        // No `TurnEnd` clear: the two `TurnEnd` layers tick off at the owner's
        // own wear-offs (before the `TurnEnd` hook fires).
    ],
);

/// The TurnStartBefore re-declare only does something when the cap is still
/// unset (a mid-game placement of the skill). `state::max` is not in the CEL
/// schema, so this is the residual.
fn cap_unset(player_id: i32) -> bool {
    state::max(player_id, state_key::FIRE) == 0
}

/// 「初始1，上限2」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 1, 2);
    Ok(())
}

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」 / （2） the shop offer. The
/// condition already picked "something happens"; this picks which.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    // （1）「每次[经过]CiRCLE时获得1个[火罐]」
    if t == ctx::tile_named("CiRCLE") {
        ctx::gain_fire(player_id, 1, &Msg::new(key!("mana_donut_gain")))?;
        return Ok(());
    }
    // （2）「经过"购物中心"，…时，在触发结算前可选择消耗一个火罐并向对应格子
    // 进行一次支付」. The fire check is the condition (`fire(owner) >= 1`);
    // everything after the offer is resolution-time content.
    let price = ctx::rent_of(t).max(ctx::buy_price(t));
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("mana_donut_title")),
        &Msg::new(key!("mana_donut_ask"))
            .tile("tile", t)
            .i("n", price as i64),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("mana_donut_spend")))? {
        return Ok(());
    }
    // 「可向抵押格子正常支付其原本收费，若为无主格子则为消耗对应收费资金」 --
    // the pay goes to the owner when there is one, and simply leaves otherwise.
    let owner = ctx::tile_owner(t);
    if owner >= 0 && owner != player_id {
        ctx::transfer(player_id, owner, price, &Msg::new(key!("mana_donut_pay")))?;
    } else {
        ctx::pay(player_id, price, &Msg::new(key!("mana_donut_pay")))?;
    }
    // 「之后，直到你的下个回合结束，你的资金消耗减半」 -- two `TurnEnd`
    // layers: one is lost at this turn's wear-off, the second at the owner's
    // next turn end. The window therefore covers the rest of this turn, the
    // other players' turns, and the owner's next turn through its end.
    state::set(player_id, HALF, 2);
    state::set_expires(player_id, HALF, card_sdk::ctx::state::TURN_END);
    ctx::log(player_id, &Msg::new(key!("mana_donut_half")));
    Ok(())
}

/// 「你的资金消耗减半」. `value > 0` is the pre.
fn on_pay(_player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    ctx::trigger::set_pay_amount((amount + 1) / 2);
    Ok(())
}