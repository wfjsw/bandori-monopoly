//! `Mor:你的光芒将照亮前路` -- C# `CardYourLight` (MatchHost.cs:4644-4665): start this
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:你的光芒将照亮前路`）:
//! > 你的光芒将照亮前路：
//! > 当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡，此次移动以“月之森女子学院”为起点（不触发起点地块效果）。
//!
//! move at 月之森女子学院 (without triggering its tile).

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const YOUR_LIGHT: CardDef = CardDef::new("Mor:你的光芒将照亮前路", &[
    On::Play(your_light),
    On::CantPlay(cant_play),
]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡」
    // C# `CardYourLight.WhyNot`: refuses outside the 20-tile window
    // (`H.Dist(H.State.seats[seat].pos, School) <= 20`), then defers to
    // `H.MoveWhyNot`.
    let school = ctx::tile_named("月之森女子学院");
    if school < 0 {
        return None;
    }
    let pos = ctx::player_pos(player_id);
    if pos >= 0 && ctx::dist(pos, school) > 20 {
        return Some(Msg::new(key!("your_light_why_not")));
    }
    // 规则书: 「可以从手牌中打出此卡」 -- the C# then defers to `H.MoveWhyNot`
    // (only while the turn's main move is still open).
    ctx::cant_move(player_id)
}

fn your_light(player_id: i32) {
    // 规则书: 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」
    // C# `H._turnCtx.Plan.Start = School; Plan.StartWhy = CardName`.
    // TODO(规则书): 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」 -- needs
    // the move-plan start override (C# `H._turnCtx.Plan.Start` / `Plan.StartWhy`) so
    // the turn's main move leaves from 月之森女子学院 without passing the start tile.
    ctx::log(player_id, &Msg::new(key!("your_light_start")).player_id("who", player_id)); // 规则书: 「此次移动以…为起点」
}