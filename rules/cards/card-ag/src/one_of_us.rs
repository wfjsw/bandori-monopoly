//!
//! 规则书（docs/rulebook/cards.json, id `AG:ONE OF US`）:
//! > ONE OF US：
//! > 将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契，触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契
//!
//! pair two deeds with a partner and split their rent income.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use bandori_std::zones::ZoneChangeCause;
use card_sdk::abi::OutCause;
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

/// Instance props, not per-player slots: a second copy of this card must not
/// overwrite the first copy's pair. `one_of_us_partner` is stored `+1` so seat
/// 0 survives (an unset prop reads back as 0).
const PARTNER: &str = "one_of_us_partner";
/// The two parked deeds are attached instances (uid `+1`); each records its
/// tile in `DEED_TILE` (`tile + 1`). 「放置在该卡上」 is an attachment.
const MINE: &str = "one_of_us_mine";
const THEIRS: &str = "one_of_us_theirs";
const DEED_TILE: &str = "one_of_us_tile";
const RENT: &str = "one_of_us_rent";
/// The bankrupt seat, `+1`, recorded by `before_out` for the leave fallback.
const OUT: &str = "one_of_us_out";
/// The attachment's card identity -- a deed is a property.
const DEED: &str = "tile:property";

pub const ONE_OF_US: CardDef = CardDef::new(
    "AG:ONE OF US",
    &[
        // 将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契
        On::Play("", Some(cant_play), play),
        // 规则书: 「当其中一方破产时」 -- the leaver is one of the pair and the
        // cause is a bankruptcy (`beforeOut` also fires on forfeit / removal).
        On::Action(Action::BeforeBankrupt(
            "card.placed",
            Some(before_out_applies),
            before_out
        )),
        // 规则书: 「将两张被指定地契放置在该卡上并转移到存活方的游戏区」 -- the
        // card leaves with the bankrupt seat's clear; its own leave hook is the
        // fallback when `before_out` could not rehome a deed.
        On::Action(Action::ZoneLeft("card.placed", None, on_leave)),
        // 「先在地契原主人方结算完成，之后被分享方资金直接增加」 -- the settle
        // runs to completion at the original owner (`tileResolved`, the
        // 「结算完成时」 terminal, `SETTLE-STAGES.md` §4 M5) and only then does
        // the partner's share land. `PayAfter` just measures the rent income
        // the settle produced; the split is the terminal's job.
        On::Action(Action::PayAfter(
            "card.placed && pay_is_rent && value > 0",
            Some(measure_rent_guard),
            measure_rent
        )).silent(),
        // The split only speaks when a designated deed produced income.
        // Runs before the silent consume below so it can read the measured rent.
        On::Action(Action::TileResolved(
            "card.placed",
            Some(share_at_resolved_applies),
            share_at_resolved
        )),
        // Consume the measured rent every resolve (bookkeeping) so a
        // skipped split cannot leak into a later settle. `quiet_when_empty`
        // rather than `silent`: it shares a TileResolved drive with the
        // visible share above, and the per-drive silent check would see the
        // share's logs and accuse this bookkeeping of hiding a real effect.
        On::Action(Action::TileResolved(
            "card.placed",
            None,
            clear_measured_rent
        )).quiet_when_empty(),
    ],
).props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"ONE OF US"#)
        .raw_name(r#"ONE OF US"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置在场上，选择场上的另一个Afterglow角色或者拥有商店街格子的角色，指定双方各一块地契，触发结算收益平分，被指定的地块不会有第三方参与分钱，必须优先指定商店街格子（先在地契原主人方结算完成，之后被分享方资金直接增加，不受其他任何效果影响）当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契"#))
        // sheet cell C10
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

/// player_id 拥有商店街格子
fn owns_shop(player_id: i32) -> bool {
    ctx::owned_tiles(player_id).into_iter().any(ctx::is_shop)
}

/// others holding a deed who are Afterglow or own a shop-street tile,
/// and thus a candidate for being the partner.
fn candidates(player_id: i32) -> Vec<i32> {
    // 规则书: 「选择场上的另一个Afterglow角色或者拥有商店街格子的角色」
    ctx::others(player_id)
        .into_iter()
        // 规则书: 「指定双方各一块地契」 -- each side must own a deed to designate.
        .filter(|&p| ctx::owned_count(p) > 0 && (ctx::in_group(p, "Afterglow") || owns_shop(p)))
        .collect()
}

/// refuses without deeds or without a partner.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("one_of_us_no_deeds")));
    }
    if candidates(player_id).is_empty() {
        return Some(Msg::new(key!("one_of_us_no_partner")));
    }
    None
}

/// shop deeds if the player has any, else every deed.
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
    // TODO: This should be placed in declaration
    let partner = ctx::ask_player(
        player_id,
        &Msg::new(key!("one_of_us_title")),
        &Msg::new(key!("one_of_us_partner")),
        &cands,
    )?;
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

    if !ctx::target_tile(a) {
        return Ok(());
    }

    if !ctx::target_tile(b) {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
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
    // the partner rides an instance prop (the card is placed at `player_id`,
    // and the persistent hooks below run as this instance) so a second copy
    // keeps its own pair. Stored as `+1` so player 0 survives (an unset prop
    // reads back as 0). The deed refs and the measured rent ride the instance
    // the same way.
    ctx::set_prop(PARTNER, partner + 1);

    // 规则书: 「指定双方各一块地契」 -- the pair of deeds is the lasting [指定]
    // while this card is in play: one parked attachment per deed, on this
    // instance (「放置在该卡上」), so it dies with the card. Each attachment
    // records its tile; the `one_of_us_mine` / `one_of_us_theirs` props hold the
    // attachment uids.
    let a_uid = park_deed(player_id, a);
    let b_uid = park_deed(player_id, b);
    ctx::set_prop(MINE, a_uid + 1);
    ctx::set_prop(THEIRS, b_uid + 1);
    // 规则书: 「触发结算收益平分…（先在地契原主人方结算完成，之后被分享方资金 直接增加，不受其他任何效果影响）」 -- the split is
    // `measure_rent` + `share_at_resolved` below: the settle completes at the
    // original owner (`tileResolved`), then the other side's half lands as a **raw**
    // money add (`ctx::adjust_money`), so no payment hook can boost it.
    Ok(())
}

/// Park one designated deed on this instance: a `tile:property` instance
/// attached face-up, recording the deed's tile. The name is filed into the
/// hand first (`attach_card` sources from a hand copy). Answers the uid, `-1`
/// when refused.
///
/// 规则书: 「将两张被指定地契放置在该卡上」 -- both designated deeds sit on the
/// card, not as marks on their tiles.
fn park_deed(player_id: i32, tile: i32) -> i32 {
    ctx::add_to_dest(player_id, DEED, ctx::Dest::Hand, false);
    let uid = ctx::attach_card(ctx::self_uid(), DEED, ZoneChangeCause::Effect, false);
    if uid >= 0 {
        ctx::set_prop_at(uid, DEED_TILE, tile + 1);
    }
    uid
}

/// Is `tile` one of the pair's designated deeds? The source of truth is the two
/// parked attachments (`MINE` / `THEIRS`) and the tiles they record.
fn designated(tile: i32) -> bool {
    if tile < 0 {
        return false;
    }
    [MINE, THEIRS].iter().any(|key| {
        let uid = ctx::prop(key) - 1;
        uid >= 0 && ctx::prop_at(uid, DEED_TILE) - 1 == tile
    })
}

/// Recorded on an instance prop; `share_at_resolved` spends it. Residual guard
/// for [`measure_rent`] -- the designated-tile check is a derived lookup
/// (GUARDS.md §6).
fn measure_rent_guard(_owner: i32) -> bool {
    ctx::prop(PARTNER) > 0 && designated(trigger::tile())
}

fn measure_rent(_owner: i32) -> card_sdk::Asked {
    // 规则书: 「触发结算收益平分」 -- the settle's rent is the split's input.
    let paid = trigger::value().max(0);
    ctx::set_prop(RENT, paid);
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
fn share_at_resolved_applies(_owner: i32) -> bool {
    let partner = ctx::prop(PARTNER) - 1;
    let income = ctx::prop(RENT);
    if partner < 0 || income <= 0 || income / 2 <= 0 {
        return false;
    }
    if !designated(trigger::tile()) {
        return false;
    }
    let owner_of_tile = ctx::tile_owner(trigger::tile());
    // 规则书: 「触发结算收益平分…被指定的地块不会有第三方参与分钱」 -- the tile
    // is still one of the pair's designated deeds even after being sold on: the
    // settle paid whoever now owns it, and the split still runs, taking the
    // partner's half out of that owner's income. Only an unowned tile stops it.
    owner_of_tile >= 0
}

/// Bookkeeping: consume the measured rent so a skipped split cannot
/// leak into a later settle.
fn clear_measured_rent(_owner: i32) -> card_sdk::Asked {
    ctx::set_prop(RENT, 0);
    Ok(())
}

fn share_at_resolved(owner: i32) -> card_sdk::Asked {
    let partner = ctx::prop(PARTNER) - 1;
    let income = ctx::prop(RENT);
    // Applicability is [`share_at_resolved_applies`]; the tally is consumed
    // by [`clear_measured_rent`] (a bookkeeping entry that always runs).
    // 「触发结算收益平分」 -- half the settle income moves to the other side of
    // the pair. The settle has already paid the 地契原主人 in full
    // (「先在地契原主人方结算完成」); this is the 被分享方's share.
    let half = income / 2;
    let tile = trigger::tile();
    let owner_of_tile = ctx::tile_owner(tile);
    // The 被分享方 is the other member of the pair (「之后被分享方资金直接增加」).
    let recipient = if owner_of_tile == partner {
        owner
    } else {
        partner
    };
    // 规则书: 「不受其他任何效果影响」 -- raw money moves, no payment pipeline.
    let src = Msg::new(key!("one_of_us_share"));
    ctx::lose_raw(owner_of_tile, half, &src)?;
    ctx::gain_raw(recipient, half, &src)?;
    ctx::log(
        recipient,
        &Msg::new(key!("one_of_us_shared"))
            .player_id("who", recipient)
            .tile("tile", tile)
            .n("money", half as i64),
    );
    Ok(())
}

/// 规则书: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区，该方视为拥有此地契」 --
/// on a bankruptcy, **both** designated deeds move to the survivor unchanged
/// (the row keeps its identity, counters and the card stays attached), and the
/// card itself is deemed the survivor's. 「两张」 is unconditional: a deed sold
/// to a third party moves too, not only the leaver's own.
///
/// 「视为拥有」 is implemented as a real owner move (`ctx::rehome_card`) -- there
/// is no tile-level deemed-owner primitive, and a real move carries every
/// owner-derived view (rent, build, mortgage) with no reader changes. C-Q50.
fn before_out(owner: i32) -> card_sdk::Asked {
    // [`before_out_applies`] is the actor test; the sheet names 破产 only.
    if trigger::out_cause() != Some(OutCause::Bankrupt) {
        return Ok(());
    }
    let partner = ctx::prop(PARTNER) - 1;
    let out = trigger::player_id();
    let survivor = if out == owner { partner } else { owner };
    if survivor < 0 || (ctx::player_out(survivor) && survivor != out) {
        return Ok(());
    }
    // 「转移到存活方的游戏区，该方视为拥有此地契」 -- the card too transfers;
    // `zoneLeft` (below) lands it in the survivor's field.
    ctx::set_prop(OUT, out + 1);
    ctx::set_instance_owner(ctx::self_uid(), survivor);
    // 规则书: 「将两张被指定地契…」 -- both, whoever owns them now. A deed the
    // survivor already holds needs no move.
    for key in [MINE, THEIRS] {
        let uid = ctx::prop(key) - 1;
        if uid < 0 {
            continue;
        }
        let t = ctx::prop_at(uid, DEED_TILE) - 1;
        if t >= 0 && ctx::tile_owner(t) != survivor {
            rehome(t, out, survivor);
        }
    }
    Ok(())
}

/// The card's own leave (cause `Bankruptcy`): the fallback for a deed
/// `before_out` could not rehome, plus 「转移到存活方的游戏区」 -- the card
/// stays in play on the survivor's field instead of leaving the game.
///
/// 规则书: 「当其中一方破产时，将两张被指定地契放置在该卡上并转移到存活方的游戏区」
fn on_leave(owner: i32) -> card_sdk::Asked {
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    let bankrupt = trigger::zone().is_some_and(|z| z.cause == ZoneChangeCause::Bankruptcy);
    if !bankrupt {
        // An ordinary leave; a bankruptcy that already requeued the card must
        // not leave the `Field` destination behind.
        bandori_std::zones::set_leave_destination(ctx::Dest::Graveyard);
        return Ok(());
    }
    // `before_out` already made the survivor the instance's owner (and hence
    // the raise's seat); it also recorded the leaver. Without `before_out` the
    // raise's seat still is the leaver.
    let survivor = ctx::instance_owner(ctx::self_uid());
    let out = match ctx::prop(OUT) {
        0 => owner,
        v => v - 1,
    };
    if survivor < 0 {
        return Ok(());
    }
    bandori_std::zones::set_leave_destination(ctx::Dest::Field);
    // 规则书: 「将两张被指定地契…」 -- the same unconditional hand-over as
    // `before_out`, for a deal it did not get to run.
    for key in [MINE, THEIRS] {
        let uid = ctx::prop(key) - 1;
        if uid < 0 {
            continue;
        }
        let t = ctx::prop_at(uid, DEED_TILE) - 1;
        if t >= 0 && ctx::tile_owner(t) != survivor {
            rehome(t, out, survivor);
        }
    }
    // The pair is done on the way out; the parked attachments die with the card
    // only on a later ordinary leave, so clear them here when it stays.
    ctx::set_prop(PARTNER, 0);
    Ok(())
}

/// Move one deed row to the survivor and log it.
fn rehome(tile: i32, out: i32, survivor: i32) {
    ctx::rehome_card(tile, survivor);
    ctx::log(
        survivor,
        &Msg::new(key!("one_of_us_transferred"))
            .tile("tile", tile)
            .player_id("who", survivor)
            .player_id("out", out),
    );
}

/// Residual guard for [`before_out`]: this copy has a pair and the leaver is
/// one side of it.
fn before_out_applies(owner: i32) -> bool {
    let partner = ctx::prop(PARTNER) - 1;
    let out = trigger::player_id();
    partner >= 0 && (out == owner || out == partner)
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn before_out_kani(player_id: i32) -> card_sdk::Asked {
    before_out(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_leave_kani(player_id: i32) -> card_sdk::Asked {
    on_leave(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn measure_rent_kani(player_id: i32) -> card_sdk::Asked {
    measure_rent(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn share_at_resolved_kani(player_id: i32) -> card_sdk::Asked {
    share_at_resolved(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_measured_rent_kani(player_id: i32) -> card_sdk::Asked {
    clear_measured_rent(player_id)
}
