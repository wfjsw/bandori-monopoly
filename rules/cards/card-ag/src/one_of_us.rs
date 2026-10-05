//! `AG:ONE OF US` -- C# `CardOneOfUs` (MatchHost.cs:1378-1546):
//! pair two deeds with a partner and split their rent income.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:ONE OF US`）:
//! > ONE OF US：
//! > 将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契，触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const ONE_OF_US: CardDef = CardDef {
    id: "AG:ONE OF US",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn owns_shop(seat: i32) -> bool {
    // C# `H.OwnedBy(p).Any(H.IsShop)`.
    ctx::owned_tiles(seat).into_iter().any(ctx::is_shop)
}

/// C# `CardOneOfUs.Candidates`: others holding a deed who are Afterglow or own
/// a shop-street tile.
fn candidates(seat: i32) -> Vec<i32> {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」 -- C#
    // `(H.BandOf(p) == "Afterglow" || H.OwnedBy(p).Any(H.IsShop)) && H.OwnedBy(p).Count > 0`.
    ctx::others(seat)
        .into_iter()
        .filter(|&p| ctx::owned_count(p) > 0 && (ctx::in_band(p, "Afterglow") || owns_shop(p)))
        .collect()
}

/// C# `CardOneOfUs.WhyNot`: refuses without deeds or without a partner.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::owned_count(seat) == 0 {
        return Some(Msg::new(key!("one_of_us_no_deeds")));
    }
    if candidates(seat).is_empty() {
        return Some(Msg::new(key!("one_of_us_no_partner")));
    }
    None
}

/// C# `CardOneOfUs.Prefer`: shop deeds if the seat has any, else every deed.
fn prefer(seat: i32) -> Vec<i32> {
    let owned = ctx::owned_tiles(seat);
    let shops: Vec<i32> = owned.iter().copied().filter(|&t| ctx::is_shop(t)).collect();
    if shops.is_empty() {
        owned
    } else {
        shops
    }
}

fn play(seat: i32) {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」
    let cands = candidates(seat);
    if cands.is_empty() {
        return;
    }
    let partner = ctx::ask_seat(
        seat,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_partner")),
        &cands,
    );
    // 规则书: 「指定双方各一块地契」; 「必须优先指定商店街格子」 -- C# `Prefer`
    // restricts each side's pick to its shop deeds when it has any.
    let mine = prefer(seat);
    if mine.is_empty() {
        return;
    }
    let a = ctx::ask_tile(
        seat,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_mine")),
        &mine,
    );
    let theirs = prefer(partner);
    if theirs.is_empty() {
        return;
    }
    let b = ctx::ask_tile(
        seat,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_theirs")).seat("who", partner),
        &theirs,
    );
    // 规则书: 「将此卡放置在场上」 -- C# `H.PlaceFromPlay(c)` after `H.TargetTile`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        seat,
        "AG:ONE OF US",
        &Msg::new(key!("one_of_us_note")).seat("who", partner).tile("a", a).tile("b", b),
    );
    ctx::log(
        seat,
        &Msg::new(key!("one_of_us_paired"))
            .seat("who", seat)
            .seat("partner", partner)
            .tile("a", a)
            .tile("b", b),
    );
    // TODO(ABI): the pair (`Mem["partner"]` / `Mem["mine"]` / `Mem["theirs"]`) needs
    // per-card Mem state on placed cards so the persistent hooks below can see it.
    // TODO(规则书): 「触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）」
    // -- needs the Fx.PayAfter hook (C# `CardOneOfUs.PayAfter`): on a rent payment
    // to either designated tile, move half of `finalGain` to the partner with a
    // raw money add (`H.State.seats[].money`, not `H.GainR`).
    // TODO(规则书): 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契」
    // -- needs the Fx.BeforeOut hook (C# `CardOneOfUs.BeforeOut`); the deed
    // hand-over itself is `ctx::set_owner` (now available) once the hook fires.
}