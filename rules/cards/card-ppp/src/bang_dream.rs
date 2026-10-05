//! `PPP:Bang Dream!` -- C# `CardBangDream`: +1 miracle crystal on the band card,
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Bang Dream!`）:
//! > Bang Dream!：
//! > [手]：
//! > 为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房。
//!
//! then teleport to one of your own tiles without settling (C#
//! `H.ForceTeleport(..., resolve: false)`). The optional build afterwards needs a
//! build-house routine the ABI does not carry yet (TODO in source).

use card_sdk::{ctx, key, CardDef, Msg};

pub const BANG_DREAM: CardDef = CardDef {
    id: "PPP:Bang Dream!",
    play: Some(bang_dream),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // C# `CardBangDream.WhyNot`: refuses with 「你还没有地」 when the seat owns no tile.
    if ctx::owned_count(seat) == 0 {
        return Some(Msg::new(key!("bang_dream_no_land")));
    }
    None // playable
}

fn bang_dream(seat: i32) {
    // 规则书: 「为[使用者]的团卡添加一个[奇迹水晶]」
    ctx::add_band_crystals(seat, 1, i32::MAX);
    ctx::log(seat, &Msg::new(key!("bang_dream_crystal")).seat("who", seat));
    // C# `Play` yields break when `H.OwnedBy` is empty (WhyNot refuses earlier).
    let mine = ctx::owned_tiles(seat);
    if mine.is_empty() {
        return;
    }
    // 规则书: 「[传送]至任意[使用者]拥有的格子」
    let to = ctx::ask_tile(
        seat,
        &Msg::new(key!("bang_dream_title")),
        &Msg::new(key!("bang_dream_ask")),
        &mine,
    );
    // C# `H.ForceTeleport(i, to, resolve: false, ...)` -- no settle on arrival.
    ctx::teleport_to(seat, to);
    ctx::log(seat, &Msg::new(key!("bang_dream_moved")).seat("who", seat).tile("tile", to));
    // TODO(规则书): 「且可选择盖房」 -- needs a build-house routine (C#
    // `H.OfferBuildAmong(i, [to], ...)`): prompt to pay `build_cost` and raise one
    // house on `to`. The ctx vocabulary has `build_cost` (a query) but no build op.
}