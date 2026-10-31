//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Change the world`）:
//! > Change the world：
//! >   将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆
//!
//! place on a house-bearing Live House and turn the move dice into 3d20.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, Msg, On, PayAddValue};

pub const CHANGE_WORLD: CardDef = CardDef::new(
    "RAS:Change the world",
    &[
        On::Play("", Some(cant_play), play),
        // 规则书: 「期间每经过一个不属于你的livehouse格子」 -- the owner's main
        // walk this turn, over a buyable Live House the owner does not hold.
        On::Action(Action::PassTile("actor == owner && move.is_main_move && card.placed && card.counter('cw_turn') == turn_key && tile_in(-1, tile.id, \"livehouse\") && tile_in(-1, tile.id, \"buyable\") && tile.owner != owner", None, pass_tile)),
        // 规则书: 「此地块的下一次收费」 -- the rent on *this* card's tile, with a
        // house count `n` to scale by. The tile match is a derived scan
        // (self_tile), residual (docs/GUARDS.md §4.2). Keyed on the tile, not
        // on the payee, so a transferred deed still consumes the one-shot.
        On::Filter(Filter::PaySplitAdd("card.placed && pay_is_rent", Some(on_card_tile), pay_choose)),
        On::Action(Action::PayAfter("card.placed && pay_is_rent", Some(on_card_tile), pay_after)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Change the world"#)
        .raw_name(r#"Change the world"#)
        .tags(&[])
        .text(CardText::para(r#"将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆"#))
        // sheet cell H5
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

const ID: &str = "RAS:Change the world";

fn spots(player_id: i32) -> Vec<i32> {
    ctx::owned_tiles(player_id)
        .into_iter()
        .filter(|&t| ctx::is_live_house_for(player_id, t) && ctx::houses_of(t) > 0)
        .collect()
}

fn cant_play(player_id: i32) -> Option<Msg> {
    if spots(player_id).is_empty() {
        return Some(Msg::new(key!("change_world_no_houses")));
    }
    rules_status::move_blocked(player_id)
}

/// An on-card counter, not a player slot: two copies on the field each carry
/// their own 「期间」 window.
const COUNTER_TURN: &str = "cw_turn";

fn play(player_id: i32) -> card_sdk::Asked {
    // The gate is now `cant_play` above.
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
    ctx::set_fate(ctx::Fate::Field);
    ctx::place_card_on(player_id, tile, ID, &Msg::new(key!("change_world_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_placed"))
            .player_id("who", player_id)
            .tile("tile", tile),
    );
    ctx::set_counter(COUNTER_TURN, ctx::turn_key());
    ctx::plan::set_base_dice(3, 20, "（Change the world）");
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_dice")).player_id("who", player_id),
    );
    Ok(())
}

fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.is_main_move && card.placed &&
    // card.counter('cw_turn') == turn_key && is_venue(tile.id) &&
    // is_buyable(tile.id) && tile.owner != owner` is the condition.
    let t = trigger::tile();
    // 规则书: 「此卡获得一个奇迹水晶」
    ctx::add_crystals(1, 0)?;
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_crystal"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    Ok(())
}

/// Residual guard for [`pay_choose`] / [`pay_after`]: the charge is on the
/// card's own tile.
///
/// 规则书（docs/rulebook/cards.json, id `RAS:Change the world`）:
/// > 此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，
/// > n为此卡放置格上房屋层数
///
/// 「此地块」 is the card's placement tile. The houses can be razed before the
/// charge, leaving `n == 0` (the increment is 0); the charge is still the
/// tile's next one and the card is still consumed (「触发后将该卡放入弃牌堆」).
fn on_card_tile(_player_id: i32) -> bool {
    let tile = ctx::self_tile().unwrap_or(-1);
    tile >= 0 && trigger::tile() == tile
}

/// Applicability is `on_card_tile`.
fn pay_choose(player_id: i32, mut v: PayAddValue) -> PayAddValue {
    let tile = ctx::self_tile().unwrap_or(-1);
    let houses = ctx::rent_houses_of(tile);
    // TODO(规则书)（n为此卡放置格上房屋层数）: 「房屋数」 here has no 「视为」 -- read as the counted value
    // the rent lookup itself uses, not the standing houses (Ringing Bloom's same
    // reading).
    let bonus = 50 * (ctx::crystals() + 1) * houses;
    if bonus <= 0 {
        return PayAddValue {
            value: v.value,
            ..v
        };
    }
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_bonus"))
            .player_id("who", player_id)
            .i("n", bonus as i64),
    );
    PayAddValue {
        value: v.value + bonus,
        ..v
    }
}

/// Applicability is `on_card_tile`.
fn pay_after(player_id: i32) -> card_sdk::Asked {
    // `PaySplitAdd` ran). Applicability is the shared residual `on_card_tile`. A
    // cancelled charge re-raises `payAfter` with `value == 0`: nothing was
    // collected, so the one-shot 「下一次收费」 stays armed.
    if trigger::value() <= 0 {
        return Ok(());
    }
    ctx::set_fate(ctx::Fate::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("change_world_discarded")).player_id("who", player_id),
    );
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
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_choose_kani(player_id: i32, v: card_sdk::PayAddValue) -> card_sdk::PayAddValue {
    pay_choose(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_after_kani(player_id: i32) -> card_sdk::Asked {
    pay_after(player_id)
}
