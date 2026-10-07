//! `R:（燐子）Ringing Bloom` -- C# `CardRingingBloom` (MatchHost.cs:10815-10847):
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

use card_sdk::abi::{prop, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const RINGING_BLOOM: CardDef = CardDef::new(
    "R:（燐子）Ringing Bloom",
    &[
        On::Play(None, play),
        On::Hook(&[HookKind::PayAfter], pay_after_guard, pay_after),
    ],
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
        if ctx::is_ring(t) {
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
        if ctx::is_ring(t) {
            continue;
        }
        if ctx::houses_of(t) < max_h {
            raised.push(t);
        }
    }
    raised
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡放置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("ringing_bloom_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("ringing_bloom_placed")).player_id("who", player_id),
    );
    // 规则书（2）: 「你的所有格子上的房屋数视为与你房屋数最多的格子等同」 --
    // `prop::RENT_HOUSES` on this instance is the counted value the rent
    // lookup reads (`ctx::rent_houses_of` / `pay_rent`). Gone with the card.
    let max_h = counted_max(player_id);
    if max_h > 0 {
        ctx::set_prop(prop::RENT_HOUSES, max_h);
    }
    // 规则书（2）: 「但受此效果影响获得额外房屋数的格子收费减半」 -- only the
    // raised ones, via `prop::RENT_FACTOR` (500 = x0.5) on the tile's rule
    // instance. Cleared when this card leaves (see `pay_after`).
    for &t in &raised_tiles(player_id, max_h) {
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 500);
    }
    // 规则书（3）: 「你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。」
    //   -- `CardRingingBloom.PayAfter` -> `Done`, see `pay_after`.
    // TODO(规则书)（2）: 「视为」 is continuous (it tracks the max as houses
    //   move); this snapshots the max at play. A later build past the snapshot
    //   or a raze of the max tile is not tracked. A `houseAdded` refresh would
    //   cover the rise; no hook covers the fall.
    Ok(())
}

/// C# `CardRingingBloom.PayAfter` -> `Done`: after a non-RiNG rent lands on this
/// player, unplace to discard and gain 500 per standing house on the charged tile.
/// Runs through the Fx hook dispatch, so this is a field effect, not a [反击].
/// Pure guard for [`pay_after`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pay_after_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn pay_after(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PayAfter || !trigger::pay_is_rent() {
        return Ok(());
    }
    // C# `p.to != Player` -- the rent must be landing on this card's owner.
    if trigger::target() != player_id {
        return Ok(());
    }
    // C# `p.tile < 0 || H._tiles[p.tile].kind == "ring"` -- skip RiNG (and tile-less) pays.
    let tile = trigger::tile();
    if tile < 0 || ctx::is_ring(tile) {
        return Ok(());
    }
    // 规则书（3）: 「X为你收费格上的房屋数」 -- the **counted** value (「视为」),
    // i.e. what the rent table was read at, not the standing houses.
    // TODO(规则书)（3）: the sheet says 「房屋数」 without 「视为」; read as the
    //   counted value because (2) has already redefined what 「房屋数」 means
    //   for this player's tiles. A reading of the standing houses would make
    //   X = 0 on the very tiles (2) lifted.
    let x = ctx::rent_houses_of(tile);
    // Disarm the half-charge before leaving: the 「视为」 ends with the card
    // (the `RENT_HOUSES` prop rides this instance), and 「受此效果影响获得额外
    // 房屋数的格子收费减半」 ends with it. TODO(规则书)（2）: a removal path
    // other than this discard leaves the tile props armed; the counted value
    // itself still ends (gone with the card).
    let max_h = counted_max(player_id);
    for &t in &raised_tiles(player_id, max_h) {
        ctx::set_tile_prop(t, prop::RENT_FACTOR, 0);
    }
    // 规则书（3）: 「此卡置入弃牌堆」 -- C# `H.Unplace(this, "discard", "自己的格子收了费")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("ringing_bloom_done"))
            .player_id("who", player_id)
            .tile("tile", tile),
    );
    // 规则书（3）: 「然后你获得500*X资金」 -- C# `H.GainR(Seat, 500 * x, ...)`.
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