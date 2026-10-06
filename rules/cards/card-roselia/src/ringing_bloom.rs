//! `R:（燐子）Ringing Bloom` -- C# `CardRingingBloom` (MatchHost.cs:10815-10847):
//!
//! 规则书（docs/rulebook/cards.json, id `R:（燐子）Ringing Bloom`）:
//! > （燐子）Ringing Bloom：
//! >
//! > （1）将此卡放置于自身场上
//! >
//! > （2）你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半
//! >
//! > （3）你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。
//!
//! place this card on your field; it then fakes your house counts up to your
//! best tile and pays out when one of your non-RiNG tiles collects rent.

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const RINGING_BLOOM: CardDef = CardDef::new("R:（燐子）Ringing Bloom", &[
    On::Play(None, play),
    On::Hook(&[HookKind::PayAfter], pay_after_guard, pay_after)]);

const ID: &str = "R:（燐子）Ringing Bloom";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡放置于自身场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("ringing_bloom_note")));
    ctx::log(player_id, &Msg::new(key!("ringing_bloom_placed")).player_id("who", player_id));
    // 规则书（2）: 「你的所有格子上的房屋数视为与你房屋数最多的格子等同，但受此效果影响获得额外房屋数的格子收费减半」
    // TODO(规则书)（2）: the house-count override and its half-rent live in the rent
    //   routine (C# `H.RentHouses` + `boosted`): while this card is in play every
    //   your tile's rent reads as if it had your max house count (capped by the
    //   rent table), and a tile whose count this raised charges half. Needs a
    //   rent-house-count / rent-multiplier hook.
    // 规则书（3）: 「你的任意非RiNG格子收费后，此卡置入弃牌堆，然后你获得500*X资金，X为你收费格上的房屋数。」
    //   -- `CardRingingBloom.PayAfter` -> `Done`, see `pay_after`.
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
    // 规则书（3）: 「X为你收费格上的房屋数」 -- C# `H.State.houses[p.tile]`.
    let x = ctx::houses_of(tile);
    // 规则书（3）: 「此卡置入弃牌堆」 -- C# `H.Unplace(this, "discard", "自己的格子收了费")`.
    ctx::unplace_self();
    ctx::to_discard(player_id, ID);
    ctx::log(player_id, &Msg::new(key!("ringing_bloom_done")).player_id("who", player_id).tile("tile", tile));
    // 规则书（3）: 「然后你获得500*X资金」 -- C# `H.GainR(Seat, 500 * x, ...)`.
    if x > 0 {
        ctx::gain(
            player_id,
            500 * x,
            &Msg::new(key!("ringing_bloom_pay")).player_id("who", player_id).tile("tile", tile).i("n", x as i64),
        );
    }
    Ok(())
}
