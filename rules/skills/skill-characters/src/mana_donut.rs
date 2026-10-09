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
//! that buys the half-price window. 「直到你的下个回合结束，你的资金消耗减半」
//! is a pay bend with a `TurnEnd` expiry, which is what the keyed state carries.
//!
//! TODO(规则书): the half-price window currently ends at the **arming turn's**
//! end (`state::set_expires(..., TURN_END)` ticks one layer off at the owner's
//! next `TurnEnd` wear-off, before the `TurnEnd` hook), not at 「你的下个回合
//! 结束」. The book's window is one turn longer. `docs/rulebook/COVERAGE.md`
//! marks the clause **unc**; changing the window length is a behaviour change
//! and needs that ruling first.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// 「你的资金消耗减半」 -- armed until the owner's next turn-end wear-off.
const HALF: &str = "skill.manaDonut.half";
/// （2）「经过"购物中心"，…时」 -- the five named food tiles.
const SHOP_PASS_PRE: &str = "actor == owner && fire(owner) >= 1 && \
     (tile.id == tile_named('购物中心') || tile.id == tile_named('便利店') || \
       tile.id == tile_named('快餐店') || tile.id == tile_named('羽泽咖啡厅') || \
       tile.id == tile_named('山吹面包房'))";

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
        // （1）「每次[经过]CiRCLE时获得1个[火罐]」 -- only the CiRCLE step.
        On::Hook(
            &[HookKind::Pass],
            "actor == owner && tile.id == tile_named('CiRCLE')",
            None,
            on_pass_circle,
        ),
        // （2） the shop offer -- only a named food tile, and only when there
        // is a 火罐 to spend. The choice / spend / pay stay in the body.
        On::Hook(&[HookKind::Pass], SHOP_PASS_PRE, None, on_pass_shop),
        // 「你的资金消耗减半」 -- only while the window is armed. TODO(规则书):
        // `actor == owner` is what 「你的资金消耗减半」 says; the old residual
        // `half` omitted it, so an armed window also halved *other* players'
        // pays. Left out of this condition so behaviour does not move under the
        // ckpt gate -- land the fix with the 「下个回合结束」 ruling above.
        On::Hook(
            &[HookKind::PayChoose],
            "card.placed && slot('skill.manaDonut.half') > 0",
            None,
            on_pay,
        ),
        // No `TurnEnd` clear: `state_set_expires(..., TURN_END)` already ticks
        // the window off at the owner's wear-off, before the `TurnEnd` hook
        // fires -- a clear there was a no-op. See the TODO above for the
        // window length the book actually specifies.
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

/// （1）「每次[经过]CiRCLE时获得1个[火罐]」.
fn on_pass_circle(player_id: i32) -> card_sdk::Asked {
    ctx::gain_fire(player_id, 1, &Msg::new(key!("mana_donut_gain")))?;
    Ok(())
}

/// （2）「经过"购物中心"，…时，在触发结算前可选择消耗一个火罐并向对应格子
/// 进行一次支付」. The fire check is the condition (`fire(owner) >= 1`);
/// everything after the offer is resolution-time content.
fn on_pass_shop(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
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
    // 「之后，直到你的下个回合结束，你的资金消耗减半」
    state::set(player_id, HALF, 1);
    state::set_expires(player_id, HALF, card_sdk::ctx::state::TURN_END);
    ctx::log(player_id, &Msg::new(key!("mana_donut_half")));
    Ok(())
}

/// 「你的资金消耗减半」.
fn on_pay(player_id: i32) -> card_sdk::Asked {
    let amount = ctx::trigger::value();
    if amount <= 0 {
        return Ok(());
    }
    ctx::trigger::set_pay_amount((amount + 1) / 2);
    Ok(())
}