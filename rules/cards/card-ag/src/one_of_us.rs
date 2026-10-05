//! `AG:ONE OF US` -- C# `CardOneOfUs` (MatchHost.cs:1378-1546):
//! pair two deeds with a partner and split their rent income.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:ONE OF US`）:
//! > ONE OF US：
//! > 将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契，触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契
//!

use alloc::vec::Vec;

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const ONE_OF_US: CardDef = CardDef::new("AG:ONE OF US", &[
    On::Play(Some(cant_play), play),
    On::Hook(&[HookKind::BeforeOut], |_| true, before_out)]);

fn owns_shop(player_id: i32) -> bool {
    // C# `H.OwnedBy(p).Any(H.IsShop)`.
    ctx::owned_tiles(player_id).into_iter().any(ctx::is_shop)
}

/// C# `CardOneOfUs.Candidates`: others holding a deed who are Afterglow or own
/// a shop-street tile.
fn candidates(player_id: i32) -> Vec<i32> {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」 -- C#
    // `(H.BandOf(p) == "Afterglow" || H.OwnedBy(p).Any(H.IsShop)) && H.OwnedBy(p).Count > 0`.
    ctx::others(player_id)
        .into_iter()
        .filter(|&p| ctx::owned_count(p) > 0 && (ctx::in_band(p, "Afterglow") || owns_shop(p)))
        .collect()
}

/// C# `CardOneOfUs.WhyNot`: refuses without deeds or without a partner.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("one_of_us_no_deeds")));
    }
    if candidates(player_id).is_empty() {
        return Some(Msg::new(key!("one_of_us_no_partner")));
    }
    None
}

/// C# `CardOneOfUs.Prefer`: shop deeds if the player has any, else every deed.
fn prefer(player_id: i32) -> Vec<i32> {
    let owned = ctx::owned_tiles(player_id);
    let shops: Vec<i32> = owned.iter().copied().filter(|&t| ctx::is_shop(t)).collect();
    if shops.is_empty() {
        owned
    } else {
        shops
    }
}

fn play(player_id: i32) {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」
    let cands = candidates(player_id);
    if cands.is_empty() {
        return;
    }
    let partner = ctx::ask_player(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_partner")),
        &cands,
    );
    // 规则书: 「指定双方各一块地契」; 「必须优先指定商店街格子」 -- C# `Prefer`
    // restricts each side's pick to its shop deeds when it has any.
    let mine = prefer(player_id);
    if mine.is_empty() {
        return;
    }
    let a = ctx::ask_tile(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_mine")),
        &mine,
    );
    let theirs = prefer(partner);
    if theirs.is_empty() {
        return;
    }
    let b = ctx::ask_tile(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_theirs")).player_id("who", partner),
        &theirs,
    );
    // 规则书: 「指定双方各一块地契」 -- C# `H.TargetTile(c, b, tt)` designates
    // the partner's deed (and so the partner, through ImmuneAll + the
    // targeted/target window); the own deed `a` is not gated in the C#.
    if !ctx::target_tile(b) {
        return;
    }
    // 规则书: 「将此卡放置在场上」 -- C# `H.PlaceFromPlay(c)` after `H.TargetTile`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "AG:ONE OF US",
        &Msg::new(key!("one_of_us_note")).player_id("who", partner).tile("a", a).tile("b", b),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("one_of_us_paired"))
            .player_id("who", player_id)
            .player_id("partner", partner)
            .tile("a", a)
            .tile("b", b),
    );
    // C# `card.Mem["partner"]` / `["mine"]` / `["theirs"]` -- the pair rides
    // per-player slots (the card is placed at `player_id`, so the slot namespace is
    // the card's) where the persistent hooks below can see it.
    // Stored as `+1` so player 0 / tile 0 survive the zero-terminated slot table
    // (an unset slot reads back as -1).
    ctx::set_slot(player_id, "one_of_us_partner", partner + 1);
    ctx::set_slot(player_id, "one_of_us_mine", a + 1);
    ctx::set_slot(player_id, "one_of_us_theirs", b + 1);
    // TODO(规则书)[judgement]: 「触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）」
    //   the clause under-specifies -- see the note above it
    // -- the Fx.PayAfter hook (`TriggerKind::PayAfter`) exists and `pay_is_rent()` /
    // `value()` carry `p.IsRent` / `p.finalGain`, but the designated-tile match
    // (`p.tile == MyTile` / `p.tile == TheirTile`) needs `t.Pay.tile` on the pay
    // trigger (currently -1), and 「被分享方资金直接增加，不受其他任何效果影响」
    // needs the raw `H.State.seats[].money` add (not `H.GainR`). Until both land
    // the split cannot key on the designated tiles without over-firing.
    // 规则书: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契」
    // -- the deed hand-over is `On::Hook(&[HookKind::BeforeOut], ...)` below
    // (C# `CardOneOfUs.BeforeOut`).
}

/// 规则书: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契」
/// -- C# `CardOneOfUs.BeforeOut` (MatchHost.cs:1523): when either side of the
/// pair leaves, the designated deeds it still owns go to the survivor and the
/// pair is cleared. (The C# transfers ownership directly; it does not park the
/// deeds on the card.)
fn before_out(owner: i32) {
    // `owner` is the card's player; `trigger::player_id()` is the player leaving (C#
    // `BeforeOut(int player_id)`).
    if !ctx::is_placed(owner) {
        return;
    }
    let partner = ctx::slot(owner, "one_of_us_partner") - 1;
    if partner < 0 {
        return;
    }
    let out = trigger::player_id();
    // C# `player_id != Player && player_id != Partner` -> return.
    if out != owner && out != partner {
        return;
    }
    let survivor = if out == owner { partner } else { owner };
    // C# `if (H.Out(num) && num != player_id) return;`.
    if ctx::player_out(survivor) && survivor != out {
        return;
    }
    let mine = ctx::slot(owner, "one_of_us_mine") - 1;
    let theirs = ctx::slot(owner, "one_of_us_theirs") - 1;
    for t in [mine, theirs] {
        // C# `owners[num2] == player_id` -> `owners[num2] = num`.
        if t >= 0 && ctx::tile_owner(t) == out {
            ctx::set_owner(t, survivor);
            ctx::log(
                survivor,
                &Msg::new(key!("one_of_us_transferred"))
                    .tile("tile", t)
                    .player_id("who", survivor)
                    .player_id("out", out),
            );
        }
    }
    // C# `Mem["partner"] = -1` -- the pair is done.
    ctx::set_slot(owner, "one_of_us_partner", 0);
}