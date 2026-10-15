//! `RAS:Change the world` -- C# `CardChangeWorld` (MatchHost.cs:9441-9531):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Change the world`）:
//! > Change the world： 
//! >   将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆
//!
//! place on a house-bearing Live House and turn the move dice into 3d20.

use alloc::vec::Vec;

use card_sdk::abi::HookKind;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const CHANGE_WORLD: CardDef = CardDef::new(
    "RAS:Change the world",
    &[
        On::Play("", Some(cant_play), play),
        // 规则书: 「期间每经过一个不属于你的livehouse格子」 -- the owner's main
        // walk this turn, over a buyable Live House the owner does not hold.
        On::Hook(
            &[HookKind::PassTile],
            "actor == owner && move.main && card.placed && slot('change_world_turn') == turn_key && tile_in(-1, tile.id, \"livehouse\") && tile_in(-1, tile.id, \"buyable\") && tile.owner != owner",
            None,
            pass_tile,
        ),
        // 「此地块的下一次收费」 -- the rent on *this* card's tile, with a
        // house count `n` to scale by. Tile match + houses are derived scans
        // (self_tile / houses_of), residual (docs/GUARDS.md §4.2).
        On::Hook(
            &[HookKind::PayAdd],
            "card.placed && pay_is_rent && target == owner",
            Some(on_card_tile_with_houses),
            pay_choose,
        ),
        On::Hook(
            &[HookKind::PayAfter],
            "card.placed && pay_is_rent && target == owner",
            Some(on_card_tile_with_houses),
            pay_after,
        ),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Change the world"#)
        .raw_name(r#"Change the world"#)
        .tags(&[])
        .text(r#"将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆"#)
        .cell(r#"H5"#)
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

const ID: &str = "RAS:Change the world";

/// C# `H.IsLiveHouse` (color group 6) tiles that can hold houses (board.json
/// `rent` non-empty). `ExtraColor` tiles (e.g. 「游击演出」's deed) are missing.
const LIVEHOUSE_PROPS: [&str; 4] = [
    "DUB MUSIC EXPERIMENT",
    "武道馆",
    "Space",
    "Live House Galaxy",
];

/// C# `Spots(player_id)` = owned Live Houses with `houses[t] > 0`.
fn spots(player_id: i32) -> Vec<i32> {
    LIVEHOUSE_PROPS
        .iter()
        .filter_map(|&n| {
            let t = ctx::tile_named(n);
            (t >= 0 && ctx::tile_owner(t) == player_id && ctx::houses_of(t) > 0).then_some(t)
        })
        .collect()
}

/// C# `CardChangeWorld.WhyNot`: 「你没有盖了房屋的 Live House 格子」 or
/// `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    if spots(player_id).is_empty() {
        return Some(Msg::new(key!("change_world_no_houses")));
    }
    rules_status::move_blocked(player_id)
}

/// Where the card's tile is written down (C# `Mem` on a tile-bound field card).
/// The turn the card was played (C# `Mem["turn"] = H.TurnKey`).
const SLOT_TURN: &str = "change_world_turn";

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡放置于你的一个有房屋的livehouse格子上」
    // -- C# `Spots(player_id)` = owned Live Houses with `houses[t] > 0`, then
    // `H.AskTileOf`. The gate is now `cant_play` above.
    let spots = spots(player_id);
    if spots.is_empty() {
        return Ok(());
    }
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("change_world_title")),
        &Msg::new(key!("change_world_ask")),
        &spots,
    )?;
    // 规则书: 「将此卡放置于你的一个有房屋的livehouse格子上」
    // -- C# `H.PlaceFromPlay(c, i, tile)` binds the card to that tile.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(player_id, tile, ID, &Msg::new(key!("change_world_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_placed"))
            .player_id("who", player_id)
            .tile("tile", tile),
    );
    // C# `Mem["turn"] = H.TurnKey` and the tile the card sits on; the player slot
    // stands in for the per-field-card `Mem` map.
    ctx::set_slot(player_id, SLOT_TURN, ctx::turn_key());
    // 规则书: 「你的本次移动掷骰变为3d20」 -- C# `H._turnCtx.Plan.Base.Clear()` +
    // `Add((3, 20, "（Change the world）"))`.
    ctx::plan::set_base_dice(3, 20, "（Change the world）");
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_dice")).player_id("who", player_id),
    );
    Ok(())
}

/// C# `CardChangeWorld.PassTile` -- while the card is placed, its owner's main
/// move passing a non-owned buyable Live House adds a crystal.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.main && card.placed &&
    // slot('change_world_turn') == turn_key && is_venue(tile.id) &&
    // is_buyable(tile.id) && tile.owner != owner` is the pre.
    let t = trigger::tile();
    ctx::add_crystals(1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_crystal"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    Ok(())
}

/// Residual guard for [`pay_choose`] / [`pay_after`]: the rent is on the card's
/// own tile and that tile still has houses (`n` in `50*(y+1)*n`).
///
/// 规则书（docs/rulebook/cards.json, id `RAS:Change the world`）:
/// > 此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，
/// > n为此卡放置格上房屋层数
///
/// 「此地块」 is the card's placement tile; with `n == 0` the clause is
/// vacuous, so there is no rent bonus (and no discard trigger) to apply.
fn on_card_tile_with_houses(_player_id: i32) -> bool {
    let tile = ctx::self_tile().unwrap_or(-1);
    tile >= 0 && trigger::tile() == tile && ctx::houses_of(tile) > 0
}

/// C# `CardChangeWorld.PayAdd` -- when rent on the card's tile is paid to the
/// card's owner, the rent grows by `50 * (Crystals + 1) * houses[Tile]`.
/// Applicability is `on_card_tile_with_houses`.
fn pay_choose(player_id: i32) -> card_sdk::Asked {
    let tile = ctx::self_tile().unwrap_or(-1);
    let houses = ctx::houses_of(tile);
    // 规则书: 「此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数」
    // -- C# `Bonus() = 50 * (Crystals + 1) * houses[Tile]`, added to `p.amount`.
    let bonus = 50 * (ctx::crystals() + 1) * houses;
    if bonus <= 0 {
        return Ok(());
    }
    trigger::set_pay_amount(trigger::value() + bonus);
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_bonus"))
            .player_id("who", player_id)
            .i("n", bonus as i64),
    );
    Ok(())
}

/// C# `CardChangeWorld.PayAfter` -- the pay that carried the bonus discards the
/// card. Applicability is `on_card_tile_with_houses`.
fn pay_after(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「触发后将该卡放入弃牌堆」 -- C# `PayAfter` fires when the pay
    // carried the `changeWorld` tag (i.e. `PayAdd` ran). Applicability is the
    // shared residual `on_card_tile_with_houses`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_discarded")).player_id("who", player_id),
    );
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
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_choose_kani(player_id: i32) -> card_sdk::Asked {
    pay_choose(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_after_kani(player_id: i32) -> card_sdk::Asked {
    pay_after(player_id)
}
