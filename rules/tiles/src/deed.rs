//! Deed economics -- mortgage / redeem amounts, and the clauses that shape them.
//!
//! 规则书（docs/rulebook/rulebook-doc.md）:
//! > 5. 运营阶段及需[支付]或[消耗]资金且资金不足时可以选择**抵押拥有的地契，
//! >    抵押后[获得]地契购买价格50%的资金**。
//! > 6. 运营阶段可**赎回拥有的地契，赎回后[消耗]地契购买价格60%的资金**。
//! > 6. 抵押，赎回，和强制购买的资金变动不受任何效果影响，除非效果写明改动
//! >    抵押，赎回，或强制购买。
//!
//! and 「基础[结算]规则」 item 5.1:
//! > 升级所[消耗]的资金不可通过[抵押]正在升级的格子地契获得。
//!
//! These two figures are the **authority** the engine's mortgage / redeem
//! charge and credit. The engine keeps a mechanical default
//! (`purchase::mortgage_value` / `purchase::redeem_cost`, what `StubRules`
//! runs) and asks [`crate::PROPERTY`]'s deed economics through
//! `CardRules::deed_mortgage_value` / `deed_redeem_cost` when a ruleset is
//! bound -- the same split as `buy_quote` (`docs/PURCHASE.md`).
//!
//! The upgrade-funding exclusion (「升级所[消耗]的资金不可通过[抵押]正在升级的
//! 格子地契获得」) is enforced by the pay pipeline's optional-funding arm
//! (`Cx::offer_mortgage_funds`'s `exclude`), not by a flag here: the deed
//! being built on is never offered in the raise-funds prompt.
//!
//! Force-buy money is likewise unaffected by effects (「此次购买的价格不受任何
//! 资金变动效果影响，收款方无论处于何种状态都可正常收款」) -- `offer_force_buy`
//! moves it directly, outside the money pipeline.

/// 规则书: 「抵押后[获得]地契购买价格50%的资金」.
/// The land price only -- standing houses are not part of the mortgage payout.
#[inline]
pub fn mortgage_value(land_price: i32) -> i32 {
    land_price / 2
}

/// 规则书: 「赎回后[消耗]地契购买价格60%的资金」.
/// Integer form of `round_ties_even(price * 0.6)` (the product of an integer
/// price and 0.6 never lands on `.5`, so this matches `DoRedeem`'s charge).
#[inline]
pub fn redeem_cost(land_price: i32) -> i32 {
    (land_price * 3 + 2) / 5
}