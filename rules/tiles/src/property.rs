//! `tile:property` -- [可购买格子] 的 [结算].
//!
//! 规则书（data/rules.txt, 「基础[结算]规则」, lines 100–106）:
//! > · 无主的[可购买格子]的[结算]是：可选择[消耗]购买格子地契和建造已有房子的资金总价，获得格子地契和拥有权。
//! > · 玩家拥有的[可购买格子]的[结算]是：
//! > 　· 如果格子地契未抵押则可选择[消耗]格子地契所标注的房屋建筑费进行升级建造，每块地有标注的等级上限（例：所有RiNG不可升级，购物中心最大等级为三栋房屋）。升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得。
//! > 　· 如果格子地契已抵押则无效果。
//! > · 其他玩家拥有的[可购买格子]的[结算]是：
//! > 　· 如果格子地契未抵押则[支付]拥有格子的玩家格子地契所标记的现等级地租。
//! > 　· 如果格子地契已抵押则可选择[支付]拥有格子的玩家购买格子地契和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的地契仍为抵押状态。此次购买的价格不受任何资金变动效果影响，收款方无论处于何种状态都可正常收款。
//!
//! and 「专有名词」:
//! > · [可购买格子]：非CiRCLE，CiRCLE 咖啡厅，江户川乐器店，流星堂，或[地产商]的格子。
//!
//! The four-way branch (unowned / own / other, each split on mortgaged) is the
//! body. Each step is an engine primitive -- `ctx::offer_buy` / `offer_build` /
//! `pay_rent` / `offer_force_buy` -- so the body is citations and calls.
//!
//! TODO(规则书): 「每块地有标注的等级上限」 is `buildMax` (`TileData.rent.len() - 1`,
//! 0 for RiNG).
//!
//! Mortgage / redeem amounts and the deed-economics clauses live in
//! [`crate::deed`] (规则书 基础规则 5–6). 「升级所[消耗]的资金不可通过[抵押]正在
//! 升级的格子地契获得」 is enforced by the pay pipeline's optional-funding arm
//! (`Cx::offer_mortgage_funds`'s `exclude`), not by a flag here.

use ruleset_core::ctx;
use card_sdk::abi::prop;
use card_sdk::{CardDef, Msg, On};

pub const PROPERTY: CardDef = CardDef::new(
    "tile:property",
    &[
        On::Settle("", None, settle),
        On::Mortgage("", None, mortgage),
        On::Redeem("", None, redeem),
    ],
)
.board_role("tile");

/// 规则书: the whole of lines 100–106 -- who owns the deed, and whether it is
/// mortgaged, decide which of the five clauses fires.
/// `tile:ring` reuses this body (see `ring.rs`).
pub(crate) fn settle(player_id: i32) -> card_sdk::Asked {
    let at = ctx::self_tile().unwrap_or(-1);
    if at < 0 {
        return Ok(());
    }
    // The settle IS the optional buy / build (规则书 「无主的[可购买格子]的[结算]
    // 是：可选择[消耗]…获得格子地契和拥有权」) -- offered as part of the settle,
    // not deferred to the end phase.
    let owner = ctx::tile_owner(at);
    if owner < 0 {
        // 规则书: 「可选择[消耗]…获得格子地契和拥有权」 -- `H.OfferBuy`.
        ctx::offer_buy(player_id, at)?;
    } else if owner != player_id {
        if ctx::mortgaged_of(at) {
            // 规则书: 「如果格子地契已抵押则可选择[支付]拥有格子的玩家购买格子地契
            // 和建造已有房子的资金总价的两倍，从该玩家处强行购买该格地契，获得的
            // 地契仍为抵押状态。」 -- `H.OfferForceBuy`. 「此次购买的价格不受任何
            // 资金变动效果影响，收款方无论处于何种状态都可正常收款」 is that
            // routine's (it moves money directly, outside the pay pipeline).
            ctx::offer_force_buy(player_id, at)?;
        } else {
            // 规则书: 「如果格子地契未抵押则[支付]拥有格子的玩家格子地契所标记的
            // 现等级地租。」 -- `H.PayRent`, the rent table at the current level.
            ctx::pay_rent(player_id, at, false)?;
        }
    } else {
        // 规则书: 「如果格子地契未抵押则可选择[消耗]格子地契所标注的房屋建筑费进行
        // 升级建造」 -- `H.OfferBuild`. 「如果格子地契已抵押则无效果」 is the
        // build gate's. 「每块地有标注的等级上限」 is `buildMax`
        // (`prop::BUILD_MAX`, rent.len() - 1).
        let _ = ctx::prop(prop::BUILD_MAX);
        ctx::offer_build(player_id, at)?;
    }
    Ok(())
}

/// 规则书 基础规则 5: 「运营阶段及需[支付]或[消耗]资金且资金不足时可以选择
/// 抵押拥有的地契，抵押后[获得]地契购买价格50%的资金」.
///
/// The body owns eligibility and the payout; `ctx::apply_mortgage` is the
/// mechanism (flag write + direct money move -- 「不受任何效果影响」).
/// `trigger.value` is the reason: 0 = player act (needs 运营阶段), 1 =
/// raise-funds (mid-payment, short of funds), 2 = optional funding prompt
/// (mid-payment, the payer asked).
pub(crate) fn mortgage(player_id: i32) -> card_sdk::Asked {
    let at = ctx::self_tile().unwrap_or(-1);
    if at < 0 {
        return Ok(());
    }
    // Ownership / standing.
    if ctx::tile_owner(at) != player_id || ctx::mortgaged_of(at) {
        return Ok(());
    }
    // 「不可被抵押」 tile prop (e.g. RiNG) -- tile-wide, so `tile_prop`.
    if ctx::tile_prop(at, prop::NO_MORTGAGE) > 0 {
        return Ok(());
    }
    if !ctx::can_pay(player_id) {
        return Ok(());
    }
    // The phase/turn gate. Reason 0 (player act) needs 运营阶段 (step 2);
    // raise-funds / optional funding (reason 1 / 2) run mid-payment and skip
    // the phase half -- 「需[支付]或[消耗]资金且资金不足时」 is that window.
    let reason = card_sdk::ctx::trigger::value();
    if reason == 0 && card_sdk::ctx::trigger::step() != 2 {
        return Ok(());
    }
    let amount = crate::deed::mortgage_value(ctx::tile_price(at));
    if amount <= 0 {
        return Ok(());
    }
    ctx::apply_mortgage(player_id, at, amount);
    Ok(())
}

/// 规则书 基础规则 6: 「运营阶段可赎回拥有的地契，赎回后[消耗]地契购买价格
/// 60%的资金」. Money is 「不受任何效果影响」 -- `ctx::apply_redeem` moves it
/// directly.
pub(crate) fn redeem(player_id: i32) -> card_sdk::Asked {
    let at = ctx::self_tile().unwrap_or(-1);
    if at < 0 {
        return Ok(());
    }
    if ctx::tile_owner(at) != player_id || !ctx::mortgaged_of(at) {
        return Ok(());
    }
    // 「运营阶段」 only.
    if card_sdk::ctx::trigger::step() != 2 {
        return Ok(());
    }
    let cost = crate::deed::redeem_cost(ctx::tile_price(at));
    if ctx::money_of(player_id) < cost {
        return Ok(());
    }
    ctx::apply_redeem(player_id, at, cost);
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn settle_kani(player_id: i32) -> card_sdk::Asked {
    settle(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn mortgage_kani(player_id: i32) -> card_sdk::Asked {
    mortgage(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn redeem_kani(player_id: i32) -> card_sdk::Asked {
    redeem(player_id)
}
