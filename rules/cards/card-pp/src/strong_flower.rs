//! `PP:可爱又强壮的花朵` -- C# `CardStrongFlower` (MatchHost.cs:7629-7698):
//! place on one of your deeds; the user stops there and settles when passing.
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
//! The tile is chosen here but `place_card` cannot bind a field card to a tile
//! (see the TODO); the [持续] stop is expressible via `ctx::plan` once the
//! tile binding lands.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, On, Msg};

pub const STRONG_FLOWER: CardDef = CardDef::new("PP:可爱又强壮的花朵", &[
    On::Play(strong_flower),
    On::CantPlay(cant_play),
]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」 -- nothing to point at without
    // a deed. C# `CardStrongFlower.WhyNot` refuses the play when the player owns
    // nothing ("你还没有地").
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("strong_flower_why_not")));
    }
    None
}

fn strong_flower(player_id: i32) {
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
        )
    };
    // 规则书[手]: 「将此卡放置在被[指定]格子上」 -- C# `H.PlaceFromPlay(c, i, tile)`
    // binds the card to the chosen tile. `place_card` has no tile parameter, so it
    // sits on the player's field instead; the choice is logged so the tile is on
    // record when the binding hook lands.
    if tile >= 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("strong_flower_at")).tile("tile", tile),
        );
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:可爱又强壮的花朵", &Msg::new(key!("strong_flower_note")));
    // 规则书[手]: 「并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 2, i32::MAX);
    // TODO(规则书): [持续]「[使用者][经过]此卡所在格子时依次进行以下操作：1. [强制停下]，
    // 此次移动变为[结算]；2. 如果[共鸣]则获得1500资金；3. 此卡放入[使用者]弃卡区」 --
    // the stop+settle half now maps to `ctx::plan::set_stop_at(tile)` +
    // `ctx::plan::set_resolve(true)` (C# `m.Stopped = true; m.Resolve = true`),
    // but the trigger is still unmapped: the Fx.PassTile hook is landed
    // (`HookKind::PassTile`) yet the card cannot be tile-bound
    // (`H.PlaceFromPlay(c, owner, tile)`) so `Tile` is unknown and the gate
    // `t == Tile` cannot fire. Remaining unmapped halves: tile binding,
    // H.TryResonance for the 1,500, and unplace-to-discard.
}