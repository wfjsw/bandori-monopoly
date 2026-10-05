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
//! (see the TODO); the [持续] stop needs the PassTile hook.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg};

pub const STRONG_FLOWER: CardDef = CardDef {
    id: "PP:可爱又强壮的花朵",
    play: Some(strong_flower),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」 -- nothing to point at without
    // a deed. C# `CardStrongFlower.WhyNot` refuses the play when the seat owns
    // nothing ("你还没有地").
    if ctx::owned_count(seat) == 0 {
        return Some(Msg::new(key!("strong_flower_why_not")));
    }
    None
}

fn strong_flower(seat: i32) {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」
    let tiles: Vec<i32> = ctx::owned_tiles(seat);
    let tile = if tiles.is_empty() {
        -1
    } else {
        ctx::ask_tile(
            seat,
            &Msg::new(key!("strong_flower_title")),
            &Msg::new(key!("strong_flower_ask")),
            &tiles,
        )
    };
    // 规则书[手]: 「将此卡放置在被[指定]格子上」 -- C# `H.PlaceFromPlay(c, i, tile)`
    // binds the card to the chosen tile. `place_card` has no tile parameter, so it
    // sits on the seat's field instead; the choice is logged so the tile is on
    // record when the binding hook lands.
    if tile >= 0 {
        ctx::log(
            seat,
            &Msg::new(key!("strong_flower_at")).tile("tile", tile),
        );
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:可爱又强壮的花朵", &Msg::new(key!("strong_flower_note")));
    // 规则书[手]: 「并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」
    ctx::add_band_crystals(seat, 2, i32::MAX);
    // TODO(规则书): [持续]「[使用者][经过]此卡所在格子时依次进行以下操作：1. [强制停下]，
    // 此次移动变为[结算]；2. 如果[共鸣]则获得1500资金；3. 此卡放入[使用者]弃卡区」 --
    // needs the Fx.PassTile hook (C# `Card.PassTile(MoveCtx, int)`, gated on
    // `t == Tile && m.Seat == User`) to stop on the card's tile
    // (`m.Stopped = true; m.Resolve = true`), the tile binding above so `Tile` is
    // known, H.TryResonance for the 1,500 (`H.GainR(User, 1500, ...)`), and
    // unplace-to-discard (`H.Unplace(this, "discard", "使用者经过了")`).
}