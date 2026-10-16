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

pub const PROPERTY: CardDef = CardDef::new("tile:property", &[On::Settle("", None, settle)])
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

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn settle_kani(player_id: i32) -> card_sdk::Asked {
    settle(player_id)
}
