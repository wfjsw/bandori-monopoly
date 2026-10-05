//! `Mor:你的光芒将照亮前路` -- C# `CardYourLight` (MatchHost.cs:4644-4665): start this
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:你的光芒将照亮前路`）:
//! > 你的光芒将照亮前路：
//! > 当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡，此次移动以“月之森女子学院”为起点（不触发起点地块效果）。
//!
//! move at 月之森女子学院 (without triggering its tile).

use card_sdk::{ctx, key, CardDef, Msg};

pub const YOUR_LIGHT: CardDef = CardDef {
    id: "Mor:你的光芒将照亮前路",
    play: Some(your_light),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡」
    // C# `CardYourLight.WhyNot`: refuses outside the 20-tile window
    // (`H.Dist(H.State.seats[seat].pos, School) <= 20`), then defers to
    // `H.MoveWhyNot`.
    let school = ctx::tile_named("月之森女子学院");
    if school < 0 {
        return None;
    }
    let pos = ctx::seat_pos(seat);
    if pos >= 0 && ctx::dist(pos, school) > 20 {
        return Some(Msg::new(key!("your_light_why_not")));
    }
    // TODO(规则书): the C# then defers to `H.MoveWhyNot` (only while the turn's
    //   main move is still open) -- that hook is still missing, so this gate is
    //   over-permissive on the move window.
    None
}

fn your_light(seat: i32) {
    // 规则书: 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」
    // C# `H._turnCtx.Plan.Start = School; Plan.StartWhy = CardName`.
    // TODO(规则书): 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」 -- needs
    // the move-plan start override (C# `H._turnCtx.Plan.Start` / `Plan.StartWhy`) so
    // the turn's main move leaves from 月之森女子学院 without passing the start tile.
    ctx::log(seat, &Msg::new(key!("your_light_start")).seat("who", seat)); // 规则书: 「此次移动以…为起点」
}