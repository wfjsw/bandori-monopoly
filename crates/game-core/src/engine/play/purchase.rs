//! The purchase surface: what can pay, how much, how ownership is assigned.
//!
//! `docs/PURCHASE.md`. The rules crate decides eligibility, price and the
//! deal; this file keeps the generic mechanics -- the quote formula's native
//! path, one ownership primitive, and the commit shape the hooks rewrite.
//!
//! * **Price stages** mirror 支付阶段 2 / 4 / 5: `BuyAdd` (fixed ±) →
//!   `BuyMul` (×) → `BuySet` (free / fixed), each floored at 0.
//! * **Seller** is the bank (`-1`) for land / agent / card / auction buys, and
//!   the owner for force-buy and 收购.
//! * **`direct`** (bypass the money pipeline) is set by the tile prop
//!   `FORCE_FIXED` (「不受任何资金变动效果影响…」).
//! * **Assignment** defaults come from tile props: a land buy clears the
//!   mortgage; `FORCE_STAYS_MORTGAGED` (「获得的地契仍为抵押状态」) keeps it.

use super::super::world::World;
use crate::data::GameData;
use crate::state::MatchState;

/// Which kind of purchase this is (C# `Buy.Kind`). Mirrors
/// `card_sdk::abi::BuyKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BuyKind {
    /// An ordinary land buy (the end step's offer, or `ctx::buy`).
    #[default]
    Land = 0,
    /// An agent offer's buy branch.
    Agent = 1,
    /// A card-driven buy (`ctx::buy` from a card body).
    Card = 2,
    /// 「强行购买」 -- a mortgaged deed bought out at 2×.
    Force = 3,
    /// 「收购」 -- a deed taken from its owner at the acquisition price.
    Acquire = 4,
    /// An auction win.
    Auction = 5,
}

impl BuyKind {
    pub const fn as_i32(self) -> i32 {
        self as i32
    }

    pub fn from_i32(v: i32) -> Option<Self> {
        Some(match v {
            0 => Self::Land,
            1 => Self::Agent,
            2 => Self::Card,
            3 => Self::Force,
            4 => Self::Acquire,
            5 => Self::Auction,
            _ => return None,
        })
    }

    /// The wire name (the `Trigger.buy_kind` string).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Land => "land",
            Self::Agent => "agent",
            Self::Card => "card",
            Self::Force => "force",
            Self::Acquire => "acquire",
            Self::Auction => "auction",
        }
    }
}

/// One quote request: `buy_quotes(player, kind, &[tile])`.
#[derive(Debug, Clone)]
pub struct BuyQuery {
    pub player: usize,
    pub kind: BuyKind,
    pub tiles: Vec<usize>,
    /// The payee (`-1` = the bank). Force-buy and 收购 name the owner.
    pub seller: i32,
}

/// What a quote answers for one tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quote {
    /// The price the buyer would actually be charged (post `BuyAdd` / `BuyMul`
    /// / `BuySet`), floored at 0. `-1` when the tile is not buyable at all.
    pub price: i32,
    /// May this player buy this tile right now? (`BUYABLE` + `BuyGate`.)
    pub eligible: bool,
}

/// The deed deal being committed. `BuyAssign` hooks rewrite the three
/// `*_after` fields before the ownership write; the defaults come from the
/// tile props and the buy kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deal {
    pub tile: usize,
    /// The buyer.
    pub player: usize,
    pub kind: BuyKind,
    /// The payee (`-1` = the bank).
    pub seller: i32,
    /// The price the pipeline (or the direct transfer) moves.
    pub price: i32,
    /// Bypass the money pipeline (the `FORCE_FIXED` prop).
    pub direct: bool,
    /// Ownership after the deal (default: the buyer).
    pub owner_after: i32,
    /// Houses after the deal (default: as standing, unless raze).
    pub houses_after: i32,
    /// Mortgage after the deal (default: cleared on a land buy, kept on a
    /// force-buy whose tile carries `FORCE_STAYS_MORTGAGED`).
    pub mortgaged_after: bool,
}

/// The plain rulebook formula: land price plus the houses already standing on
/// the tile. This is what `CardRules::buy_quote`'s default returns, and what
/// the sim and `StubRules` run -- the price stages (`BuyAdd` → `BuyMul` →
/// `BuySet`) sit on top of it in the rules crate.
pub fn quote_native(data: &GameData, st: &MatchState, t: usize) -> i32 {
    let Some(tile) = data.tiles.get(t) else {
        return -1;
    };
    tile.price + st.houses.get(t).copied().unwrap_or(0) * tile.house
}

/// The base a quote starts from, before the `BuyAdd` / `BuyMul` / `BuySet`
/// stages. `Force` is 「购买格子地契和建造已有房子的资金总价的两倍」; every other
/// kind is the plain land price plus standing houses.
pub fn base_quote(data: &GameData, st: &MatchState, t: usize, kind: BuyKind) -> i32 {
    match kind {
        BuyKind::Force => force_price_native(data, st, t),
        _ => quote_native(data, st, t),
    }
}

/// Ask the rules crate's `buy_quote` for `tiles` -- each `(tile, seller)`, the
/// seller `-1` for the bank -- on behalf of `player` at `kind`. One batched
/// call, so a hooking ruleset fires its hooks once for the whole list and the
/// callers all see the same figure (quote == charge).
pub fn quote_for(
    rules: &dyn crate::engine::rules::CardRules,
    w: &World,
    data: &GameData,
    player: usize,
    kind: BuyKind,
    tiles: &[(usize, i32)],
) -> Vec<Quote> {
    let q = BuyQuery {
        player,
        kind,
        tiles: tiles.iter().map(|&(t, _)| t).collect(),
        seller: tiles.first().map_or(-1, |&(_, s)| s),
    };
    rules.buy_quote(w, data, &q)
}

/// The price stages (`docs/PURCHASE.md`): `BuyAdd` (fixed ±) → `BuyMul` (×) →
/// `BuySet` (free / fixed), each floored at 0. Applied to `base` after the
/// native quote. The stages are the rules crate's hooks; for `StubRules` there
/// are none and `base` passes through unchanged.
pub fn apply_stages(mut base: i32, add: i32, mul_milli: i32, set: Option<i32>) -> i32 {
    // BuyAdd: fixed ±, floored at 0.
    base = (base + add).max(0);
    // BuyMul: ×, floored at 0. Milli-units (500 = ×0.5, 1000 = ×1).
    if mul_milli != 1000 && mul_milli > 0 {
        base = ((base as i64 * mul_milli as i64) / 1000) as i32;
        base = base.max(0);
    }
    // BuySet: free / fixed, floored at 0.
    if let Some(s) = set {
        base = s.max(0);
    }
    base
}

/// The force-buy base: 2×(land price + houses). 「购买格子地契和建造已有房子的
/// 资金总价的两倍」. Not scaled by any money-modifying effect
/// (「此次购买的价格不受任何资金变动效果影响」).
pub fn force_price_native(data: &GameData, st: &MatchState, t: usize) -> i32 {
    2 * quote_native(data, st, t).max(0)
}

/// One ownership primitive: write the deed's owner / houses / mortgage in one
/// place, so every buy path (land, agent, card, force, acquire, auction) ends
/// at the same write.
pub fn assign_deed(w: &mut World, t: usize, owner: i32, houses: i32, mortgaged: bool) {
    if t >= w.st.owners.len() {
        return;
    }
    w.st.owners[t] = owner;
    if t < w.st.houses.len() {
        w.st.houses[t] = houses.max(0);
    }
    if t < w.st.mortgaged.len() {
        w.st.mortgaged[t] = mortgaged;
    }
}

/// The default deal for a buy of `t` by `player` at `kind` / `price`.
///
/// * seller: the bank, except force-buy / 收购 which name the current owner;
/// * owner_after: the buyer;
/// * houses_after: as standing (a raze is a `BuyAssign` rewrite);
/// * mortgaged_after: cleared, except a force-buy keeps whatever the tile's
///   `FORCE_STAYS_MORTGAGED` prop says (default true for Force -- 「获得的地契
///   仍为抵押状态」).
pub fn default_deal(
    _data: &GameData,
    st: &MatchState,
    t: usize,
    player: usize,
    kind: BuyKind,
    price: i32,
    force_stays_mortgaged: bool,
) -> Deal {
    let seller = match kind {
        BuyKind::Force | BuyKind::Acquire => st.owners.get(t).copied().unwrap_or(-1),
        _ => -1,
    };
    let houses = st.houses.get(t).copied().unwrap_or(0);
    let mortgaged_after = match kind {
        BuyKind::Force => force_stays_mortgaged,
        _ => false,
    };
    Deal {
        tile: t,
        player,
        kind,
        seller,
        price: price.max(0),
        direct: false,
        owner_after: player as i32,
        houses_after: houses,
        mortgaged_after,
    }
}