//! `PP:可爱又强壮的花朵` -- C# `CardStrongFlower` (MatchHost.cs:7629-7698):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:可爱又强壮的花朵`）:
//! > 可爱又强壮的花朵：
//! > [手]：
//! > [指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]。
//! > [持续]：
//! > [使用者][经过]此卡所在格子时依次进行以下操作：
//! > 1. [强制停下]，此次移动变为[结算]；
//! > 2. 如果[共鸣]则获得1500资金；
//! > 3. 此卡放入[使用者]弃卡区。
//!
//! place on one of your deeds; the user stops there and settles when passing.
//! The tile is chosen here but `place_card` cannot bind a field card to a tile
//! (see the TODO); the [持续] stop is expressible via `ctx::plan` once the
//! tile binding lands.

use alloc::vec::Vec;
use card_sdk::abi::{HookKind, TriggerKind};
use card_sdk::ctx::{plan, trigger};
use card_sdk::{ctx, key, CardDef, Msg, On};

const ID: &str = "PP:可爱又强壮的花朵";

pub const STRONG_FLOWER: CardDef = CardDef::new(
    "PP:可爱又强壮的花朵",
    &[
        On::Play(Some(cant_play), strong_flower),
        On::Hook(&[HookKind::PassTile], |_| true, pass_tile),
    ],
);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」 -- nothing to point at without
    // a deed. C# `CardStrongFlower.WhyNot` refuses the play when the player owns
    // nothing ("你还没有地").
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("strong_flower_why_not")));
    }
    None
}

fn strong_flower(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」
    let tiles: Vec<i32> = ctx::owned_tiles(player_id);
    let tile = if tiles.is_empty() {
        -1
    } else {
        ctx::ask_tile(
            player_id,
            &Msg::new(key!("strong_flower_title")),
            &Msg::new(key!("strong_flower_ask")),
            &tiles,
        )?
    };
    // 规则书[手]: 「将此卡放置在被[指定]格子上」 -- bound to the chosen tile, so
    // the `PassTile` hook below can ask 「此卡所在格子」.
    if tile >= 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("strong_flower_at")).tile("tile", tile),
        );
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(
        player_id,
        tile,
        "PP:可爱又强壮的花朵",
        &Msg::new(key!("strong_flower_note")),
    );
    // 规则书[手]: 「并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 2, i32::MAX);
    Ok(())
}

/// 规则书[持续]: 「[使用者][经过]此卡所在格子时依次进行以下操作：1. [强制停下]，
/// 此次移动变为[结算]；2. 如果[共鸣]则获得1500资金；3. 此卡放入[使用者]弃卡区」
/// -- the user's own pass, onto the tile this card is bound to, and the three
/// steps in order.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PassTile || trigger::player_id() != player_id {
        return Ok(());
    }
    let Some(tile) = ctx::self_tile() else {
        return Ok(());
    };
    if trigger::tile() != tile {
        return Ok(());
    }
    // 1. 「[强制停下]，此次移动变为[结算]」
    plan::set_stop_at(tile);
    plan::set_resolve(true);
    // 2. 「如果[共鸣]则获得1500资金」
    if crate::resonance::try_resonance(player_id)? {
        ctx::gain(player_id, 1500, &Msg::new(key!("strong_flower_gain")));
    }
    // 3. 「此卡放入[使用者]弃卡区」
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}
