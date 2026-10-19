//! `AG:ONE OF US` -- C# `CardOneOfUs` (MatchHost.cs:1378-1546):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:ONE OF US`）:
//! > ONE OF US：
//! > 将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契，触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契
//!
//! pair two deeds with a partner and split their rent income.

use alloc::vec::Vec;

use card_sdk::abi::ActionKind;
use bandori_std::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const ONE_OF_US: CardDef = CardDef::new(
    "AG:ONE OF US",
    &[
        On::Play("", "", Some(cant_play), play),
        // 规则书: 「当其中一方破产时」 -- the leaver is one of the pair.
        On::Action(
            &[ActionKind::BeforeOut],
            "card.placed && slot('one_of_us_partner') > 0 && (actor == owner || actor == slot('one_of_us_partner') - 1)", "",
            None,
            before_out,
        ),
        // 「先在地契原主人方结算完成，之后被分享方资金直接增加」 -- the settle
        // runs to completion at the original owner (`tileResolved`, the
        // 「结算完成时」 terminal, `SETTLE-STAGES.md` §4 M5) and only then does
        // the partner's share land. `PayAfter` just measures the rent income
        // the settle produced; the split is the terminal's job.
        On::Action(&[ActionKind::PayAfter], "card.placed && slot('one_of_us_partner') > 0 && pay_is_rent && value > 0", "", Some(measure_rent_guard), measure_rent),
        // The split only speaks when a designated deed produced income.
        // Runs before the silent consume below so it can read the measured rent.
        On::Action(
            &[ActionKind::TileResolved],
            "card.placed && slot('one_of_us_partner') > 0", "",
            Some(share_at_resolved_applies),
            share_at_resolved,
        ),
        // Consume the measured rent every resolve (bookkeeping) so a
        // skipped split cannot leak into a later settle. `quiet_when_empty`
        // rather than `silent`: it shares a TileResolved drive with the
        // visible share above, and the per-drive silent check would see the
        // share's logs and accuse this bookkeeping of hiding a real effect.
        On::Action(&[ActionKind::TileResolved], "card.placed && slot('one_of_us_partner') > 0", "", None, clear_measured_rent),
    ],
).props(&[(card_sdk::abi::prop::SONG, 1)]).silent(&[2]).quiet_when_empty(&[4]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"ONE OF US"#)
        .raw_name(r#"ONE OF US"#)
        .tags(&[])
        .text(r#"将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契，触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契"#)
        .cell(r#"C10"#)
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

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
        .filter(|&p| ctx::owned_count(p) > 0 && (ctx::in_group(p, "Afterglow") || owns_shop(p)))
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

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」
    let cands = candidates(player_id);
    if cands.is_empty() {
        return Ok(());
    }
    let partner = ctx::ask_player(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_partner")),
        &cands,
    )?;
    // 规则书: 「指定双方各一块地契」; 「必须优先指定商店街格子」 -- C# `Prefer`
    // restricts each side's pick to its shop deeds when it has any.
    let mine = prefer(player_id);
    if mine.is_empty() {
        return Ok(());
    }
    let a = ctx::ask_tile(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_mine")),
        &mine,
    )?;
    let theirs = prefer(partner);
    if theirs.is_empty() {
        return Ok(());
    }
    let b = ctx::ask_tile(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_theirs")).player_id("who", partner),
        &theirs,
    )?;
    // 规则书: 「指定双方各一块地契」 -- C# `H.TargetTile(c, b, tt)` designates
    // the partner's deed (and so the partner, through ImmuneAll + the
    // targeted/target window); the own deed `a` is not gated in the C#.
    if !ctx::target_tile(b) {
        return Ok(());
    }
    // 规则书: 「将此卡放置在场上」 -- C# `H.PlaceFromPlay(c)` after `H.TargetTile`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "AG:ONE OF US",
        &Msg::new(key!("one_of_us_note"))
            .player_id("who", partner)
            .tile("a", a)
            .tile("b", b),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("one_of_us_paired"))
            .player_id("who", player_id)
            .player_id("partner", partner)
            .tile("a", a)
            .tile("b", b),
    );
    // C# `card.Mem["partner"]` -- the partner rides a per-player slot (the card
    // is placed at `player_id`, so the slot namespace is the card's) where the
    // persistent hooks below can see it. Stored as `+1` so player 0 survives the
    // zero-terminated slot table (an unset slot reads back as -1).
    ctx::set_slot(player_id, "one_of_us_partner", partner + 1);
    // 规则书: 「指定双方各一块地契」 -- the pair of deeds is the lasting [指定]
    // while this card is in play: one designation mark per deed, owned by this
    // field instance (dies with the card via `destroy_instance_units`). This
    // replaces the old `one_of_us_mine` / `one_of_us_theirs` tile slots.
    ctx::designate_tile(
        a,
        player_id,
        &Msg::new(key!("one_of_us_note"))
            .player_id("who", partner)
            .tile("a", a)
            .tile("b", b),
    );
    ctx::designate_tile(
        b,
        player_id,
        &Msg::new(key!("one_of_us_note"))
            .player_id("who", partner)
            .tile("a", a)
            .tile("b", b),
    );
    // 规则书: 「触发结算收益平分…（先在地契原主人方结算完成，之后被分享方资金
    // 直接增加，不受其他任何效果影响）」 -- the split is `measure_rent` +
    // `share_at_resolved` below: the settle completes at the original owner
    // (`tileResolved`), then the other side's half lands as a **raw** money
    // add (`ctx::adjust_money`), so no payment hook can boost it.
    // 规则书: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契」
    // -- the deed hand-over is `On::Action(&[ActionKind::BeforeOut], "", Some(...))` below
    // (C# `CardOneOfUs.BeforeOut`).
    Ok(())
}

/// Is `tile` one of the pair's designated deeds?
/// The source of truth is this instance's [指定] marks
/// (`ctx::designates`, one per deed placed in `play`).
fn designated(tile: i32) -> bool {
    tile >= 0 && ctx::designates(tile)
}

/// Measure the rent income a designated deed just produced (`Fx.PayAfter`).
/// `t.Pay.tile` rides the pay trigger (ABI v42), so the designated-tile match
/// is exact. Recorded on a slot; `share_at_resolved` spends it.
/// Residual guard for [`measure_rent`] -- the designated-tile check is a
/// derived lookup (GUARDS.md §6).
fn measure_rent_guard(_owner: i32) -> bool {
    designated(trigger::tile())
}

fn measure_rent(owner: i32) -> card_sdk::Asked {
    let paid = trigger::value().max(0);
    ctx::set_slot(owner, "one_of_us_rent", paid);
    Ok(())
}

/// 「先在地契原主人方结算完成，之后被分享方资金直接增加」 -- the settle's
/// terminal (`tileResolved`, 「结算完成时」; `SETTLE-STAGES.md` §4 M5). The
/// 地契原主人's side has already settled; now the 被分享方's half lands. Fires
/// even when the settle was cancelled (complete-as-nothing) -- with no recorded
/// income there is nothing to split.
/// Residual guard for [`share_at_resolved`]: a designated deed produced
/// income worth splitting. Derived lookups (`designated`, the live tile
/// owner) -- docs/GUARDS.md §4.2.
fn share_at_resolved_applies(owner: i32) -> bool {
    let partner = ctx::slot(owner, "one_of_us_partner") - 1;
    let income = ctx::slot(owner, "one_of_us_rent");
    if income <= 0 || income / 2 <= 0 {
        return false;
    }
    if !designated(trigger::tile()) {
        return false;
    }
    let owner_of_tile = ctx::tile_owner(trigger::tile());
    // 规则书: 「指定双方各一块地契」 -- the tile is one of the pair's deeds;
    // its owner is one side of the pair (the 「地契原主人」), not a third party
    // (「被指定的地块不会有第三方参与分钱」). Either side's deed splits.
    owner_of_tile >= 0 && (owner_of_tile == owner || owner_of_tile == partner)
}

/// Bookkeeping: consume the measured rent so a skipped split cannot
/// leak into a later settle.
fn clear_measured_rent(owner: i32) -> card_sdk::Asked {
    ctx::set_slot(owner, "one_of_us_rent", 0);
    Ok(())
}

fn share_at_resolved(owner: i32) -> card_sdk::Asked {
    let partner = ctx::slot(owner, "one_of_us_partner") - 1;
    let income = ctx::slot(owner, "one_of_us_rent");
    // Applicability is [`share_at_resolved_applies`]; the tally is consumed
    // by [`clear_measured_rent`] (a bookkeeping entry that always runs).
    // 「触发结算收益平分」 -- half the settle income moves to the other side of
    // the pair. The settle has already paid the 地契原主人 in full
    // (「先在地契原主人方结算完成」); this is the 被分享方's share.
    let half = income / 2;
    let tile = trigger::tile();
    let owner_of_tile = ctx::tile_owner(tile);
    // The 被分享方 is the other member of the pair (「之后被分享方资金直接增加」).
    let recipient = if owner_of_tile == partner { owner } else { partner };
    // 规则书: 「不受其他任何效果影响」 -- raw money moves, no payment pipeline.
    let src = Msg::new(key!("one_of_us_share"));
    ctx::lose_raw(owner_of_tile, half, &src);
    ctx::gain_raw(recipient, half, &src);
    ctx::log(
        recipient,
        &Msg::new(key!("one_of_us_shared"))
            .player_id("who", recipient)
            .tile("tile", tile)
            .n("money", half as i64),
    );
    Ok(())
}

/// 规则书: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有次地契」
/// -- C# `CardOneOfUs.BeforeOut` (MatchHost.cs:1523): when either side of the
/// pair leaves, the designated deeds it still owns go to the survivor and the
/// pair is cleared. (The C# transfers ownership directly; it does not park the
/// deeds on the card.)
fn before_out(owner: i32) -> card_sdk::Asked {
    // `owner` is the card's player; `trigger::player_id()` is the player leaving (C#
    // `BeforeOut(int player_id)`). `actor == owner || actor == partner` is the pre.
    let partner = ctx::slot(owner, "one_of_us_partner") - 1;
    let out = trigger::player_id();
    let survivor = if out == owner { partner } else { owner };
    // C# `if (H.Out(num) && num != player_id) return Ok(());`.
    if ctx::player_out(survivor) && survivor != out {
        return Ok(());
    }
    // The pair's designated deeds -- this instance's [指定] marks (one per
    // deed). Same iteration the old `one_of_us_mine` / `one_of_us_theirs` slots
    // did; the marks stay as the record of 「两张被指定地契」 (the pair is done
    // once `one_of_us_partner` is cleared below, so nothing re-reads them).
    for t in ctx::designated_tiles() {
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
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn measure_rent_kani(player_id: i32) -> card_sdk::Asked {
    measure_rent(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_measured_rent_kani(player_id: i32) -> card_sdk::Asked {
    clear_measured_rent(player_id)
}

// Kani-only direct-call exports (ppcost). Production builds omit these.

#[cfg(kani)]
#[inline(always)]
pub fn before_out_kani(player_id: i32) -> card_sdk::Asked {
    before_out(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn share_at_resolved_kani(player_id: i32) -> card_sdk::Asked {
    share_at_resolved(player_id)
}
