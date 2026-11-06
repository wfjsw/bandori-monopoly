//!
//! 规则书（docs/rulebook/cards.json, id `R:（燐子）Ringing Bloom`）:
//! > （燐子）Ringing Bloom：
//! >
//! > （1）将此卡放置于自身场上
//!
//! > （2）你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半
//!
//! > （3）你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。
//!
//! place this card on your field; it then fakes your house counts up to your
//! best tile and pays out when one of your non-RiNG tiles collects rent.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, ActionKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const RINGING_BLOOM: CardDef = CardDef::new(
    "R:（燐子）Ringing Bloom",
    &[
        On::Play("", None, play),
        On::Action(Action::PayAfter(
            "card.placed && pay_is_rent && target == owner",
            None,
            pay_after)),
        // 规则书（2）: 「视为」 is continuous -- it tracks the max as houses move,
        // so the fall re-runs the same view as the rise.
        On::Action(Action::HouseAdded(
            "card.placed && actor == owner",
            None,
            recompute_view)),
        On::Action(Action::HouseRemoved(
            "card.placed && actor == owner",
            None,
            recompute_view)),
        // 规则书（2）: 「你的所有格子」 -- a tile acquired while the card is placed
        // is lifted by the owner-wide `RENT_HOUSES` view too, so its half-charge
        // is armed like a rise (`bought` carries the buyer as actor).
        On::Action(Action::Bought(
            "card.placed && actor == owner",
            None,
            recompute_view)),
        // Any removal route disarms what the card armed (the raise names the
        // discarded card; `card.placed` is already false here). Silent.
        On::Action(Action::Discarded("trigger_card == card.id", None, on_discarded)).silent(),
    ],
)
.props(&[(card_sdk::abi::prop::SONG, 1)])
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"Ringing Bloom"#)
        .raw_name(r#"（燐子）Ringing Bloom"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"将此卡放置于自身场上"#),
            it(r#"你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半"#),
            it(r#"你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。"#),
        ]))
        // sheet cell E12
        .fields(&[("group", r#"Roselia"#), ("owner", r#"白金燐子"#)]),
);

const ID: &str = "R:（燐子）Ringing Bloom";

/// 规则书（2）: 「你的所有格子上的房屋数视为与你房屋数最多的格子等同」 --
/// the *counted* rent-house-count is the owner's max. 「视为」 is virtual:
/// real `houses` must not change (build caps, raze, sale and the count after
/// this card leaves all still see the standing houses). The override is
/// `prop::RENT_HOUSES` on **this placed instance** (`ctx::set_prop`), so it
/// ends the moment the card leaves the field.
fn counted_max(player_id: i32) -> i32 {
    let mut max_h = 0i32;
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        max_h = max_h.max(ctx::houses_of(t));
    }
    max_h
}

/// Which of the owner's non-RiNG tiles this effect is lifting (「受此效果影响
/// 获得额外房屋数的格子」) -- real count below the counted max.
fn raised_tiles(player_id: i32, max_h: i32) -> alloc::vec::Vec<i32> {
    let mut raised = alloc::vec::Vec::new();
    if max_h <= 0 {
        return raised;
    }
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        if ctx::houses_of(t) < max_h {
            raised.push(t);
        }
    }
    raised
}

fn play(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(player_id, ID, &Msg::new(key!("ringing_bloom_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("ringing_bloom_placed")).player_id("who", player_id),
    );
    // 规则书（2）: the 「视为」 view.
    apply_view(player_id);
    Ok(())
}

/// 规则书（2）: 「你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此
/// 效果影响获得额外房屋数的格子收费减半」 -- recompute the counted max and the
/// half-charge set. Called at play, on every `HouseAdded` and on a `Bought`, so
/// the 「视为」 tracks the max as houses rise and reaches newly acquired tiles.
///
/// TODO(规则书)（2）: `HouseRemoved` now re-runs the view, but a half-charge on a
///   tile that **leaves** its owner keeps standing (`RENT_FACTOR` is a
///   board-tile prop, not a per-owner one), and the removal's owner is the
///   tile's owner at write time -- a razed tile of another seat re-runs the
///   card owner's view, which is harmless but does not clear the old tile.
///   An ownership change that raises no `bought` (`ctx::set_owner`: gift/swap)
///   leaves a gained tile un-halved until the next build, though the
///   owner-wide `RENT_HOUSES` view already lifts it.
fn apply_view(player_id: i32) {
    let max_h = counted_max(player_id);
    // Written unconditionally: a max that drops back to 0 (every house razed)
    // must clear the view too, not leave the last count standing.
    ctx::set_prop(prop::RENT_HOUSES, max_h);
    // Reconcile the whole set, not just the rise: a tile that has caught up with
    // the max is no longer 「受此效果影响获得额外房屋数」 and must charge in full.
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    for &t in &raised_tiles(player_id, max_h) {
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 500);
    }
}

/// 规则书（2）: 「视为」 tracks the max as houses move (either direction) and
/// covers a tile acquired mid-life.
fn recompute_view(player_id: i32) -> card_sdk::Asked {
    apply_view(player_id);
    Ok(())
}

/// The card left play: clear the counted max and every half-charge it armed.
/// Nothing is recomputed -- the card is gone.
fn on_discarded(player_id: i32) -> card_sdk::Asked {
    ctx::set_prop(prop::RENT_HOUSES, 0);
    for &t in &ctx::owned_tiles(player_id) {
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    Ok(())
}

/// Runs through the hook dispatch, so this is a field effect, not a [反击]. Pure
/// guard for [`pay_after`] -- the activation gate. `false` means the card is not
/// activated at all.
fn pay_after(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（你的任意非RiNG格子收费后）: 「收费后」 can also be read as a pay that collected
    // nothing (fully counteracted, or a blocked payee) -- the engine still raises
    // `payAfter` there with `value` 0, and we discard and pay out on it anyway (in a
    // reflected rent only the 0-value parent pay carries `target == owner`, so the
    // card can never fire on the real leg).
    let tile = trigger::tile();
    if tile < 0 || ctx::is_deed(tile) {
        return Ok(());
    }
    // 规则书（3）: 「X为你收费格上的房屋数」 -- the **counted** value (「视为」),
    // i.e. what the rent table was read at, not the standing houses. A read of
    // the standing houses makes X = 0 on the very tiles (2) lifted.
    let x = ctx::rent_houses_of(tile);
    // Disarm the half-charge before leaving: the 「视为」 ends with the card
    // (the `RENT_HOUSES` prop rides this instance), and 「受此效果影响获得额外
    // 房屋数的格子收费减半」 ends with it. TODO(规则书)（2）: a removal path
    // other than this discard leaves the tile props armed; the counted value
    // itself still ends (gone with the card).
    for &t in &ctx::owned_tiles(player_id) {
        if ctx::is_deed(t) {
            continue;
        }
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::log(
        player_id,
        &Msg::new(key!("ringing_bloom_done"))
            .player_id("who", player_id)
            .tile("tile", tile),
    );
    if x > 0 {
        ctx::gain(
            player_id,
            500 * x,
            &Msg::new(key!("ringing_bloom_pay"))
                .player_id("who", player_id)
                .tile("tile", tile)
                .i("n", x as i64),
        )?;
    }
    Ok(())
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
pub fn pay_after_kani(player_id: i32) -> card_sdk::Asked {
    pay_after(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_house_added_kani(player_id: i32) -> card_sdk::Asked {
    recompute_view(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn recompute_view_kani(player_id: i32) -> card_sdk::Asked {
    recompute_view(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_discarded_kani(player_id: i32) -> card_sdk::Asked {
    on_discarded(player_id)
}
