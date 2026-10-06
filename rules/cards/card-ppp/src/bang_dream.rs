//! `PPP:Bang Dream!` -- C# `CardBangDream`: +1 miracle crystal on the band card,
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:Bang Dream!`）:
//! > Bang Dream!：
//! > [手]：
//! > 为[使用者]的团卡添加一个[奇迹水晶]，然后[传送]至任意[使用者]拥有的格子且可选择盖房。
//!
//! then teleport to one of your own tiles without settling (C#
//! `H.ForceTeleport(..., resolve: false)`), then optionally build there.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const BANG_DREAM: CardDef = CardDef::new("PPP:Bang Dream!", &[
    On::Play(Some(cant_play), bang_dream)]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardBangDream.WhyNot`: refuses with 「你还没有地」 when the player owns no tile.
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("bang_dream_no_land")));
    }
    None // playable
}

fn bang_dream(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「为[使用者]的团卡添加一个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 1, i32::MAX);
    ctx::log(player_id, &Msg::new(key!("bang_dream_crystal")).player_id("who", player_id));
    // C# `Play` yields break when `H.OwnedBy` is empty (WhyNot refuses earlier).
    let mine = ctx::owned_tiles(player_id);
    if mine.is_empty() {
        return Ok(());
    }
    // 规则书: 「[传送]至任意[使用者]拥有的格子」
    let to = ctx::ask_tile(
        player_id,
        &Msg::new(key!("bang_dream_title")),
        &Msg::new(key!("bang_dream_ask")),
        &mine,
    )?;
    // C# `H.ForceTeleport(i, to, resolve: false, ...)` -- no settle on arrival.
    ctx::teleport_to(player_id, to);
    ctx::log(player_id, &Msg::new(key!("bang_dream_moved")).player_id("who", player_id).tile("tile", to));
    // 规则书: 「且可选择盖房」 -- C# `H.OfferBuildAmong(i, [to], ...)`: prompt to
    // pay `build_cost` and raise one house on `to`. Skips silently when `to`
    // cannot take one.
    ctx::card_offer_build(player_id, &[to]);
    Ok(())
}