//! `Mor:你的光芒将照亮前路` -- C# `CardYourLight` (MatchHost.cs:4644-4665): start this
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:你的光芒将照亮前路`）:
//! > 你的光芒将照亮前路：
//! > 当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡，此次移动以“月之森女子学院”为起点（不触发起点地块效果）。
//!
//! move at 月之森女子学院 (without triggering its tile).

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const YOUR_LIGHT: CardDef = CardDef::new(
    "Mor:你的光芒将照亮前路",
    &[On::Play(
        // The dist clause stays in the residual guard (not `pre`): a play gate
        // must not trade its specific refusal (`your_light_why_not`) for the
        // generic `err.play_pre` a pre-reject surfaces (GUARDS.md §4.4).
        "",
        Some(cant_play),
        your_light)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"你的光芒将照亮前路"#)
        .raw_name(r#"你的光芒将照亮前路"#)
        .tags(&[])
        .text(CardText::para(r#"当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡，此次移动以“月之森女子学院”为起点（不触发起点地块效果）。"#))
        // sheet cell G4
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「当你在“月之森女子学院”格子前后20格之内，可以从手牌中打出此卡」
    // C# `CardYourLight.WhyNot`: refuses outside the 20-tile window
    // (`H.Dist(H.State.seats[seat].pos, School) <= 20`) and only when
    // `pos >= 0` (a seat not on the board admits). School unknown admits
    // (the C# `WhyNot` early-returns `None` then; on such a board the body is
    // then a no-op -- no start and no start log). The refusal is the specific
    // `your_light_why_not`, so it lives here, not in `pre`.
    let school = ctx::tile_named("月之森女子学院");
    if school < 0 {
        return None;
    }
    let pos = ctx::player_pos(player_id);
    if pos >= 0 && ctx::dist(pos, school) > 20 {
        return Some(Msg::new(key!("your_light_why_not")));
    }
    // 规则书: 「可以从手牌中打出此卡」 -- the 20-tile window is the only printed
    // play condition. Whether 「此次移动」 needs the turn's main move is
    // OPEN-移动起点: the start arm below keeps the `H.MoveWhyNot` gate, the play
    // itself no longer refuses (the 3-valued `status.move_*` message stays).
    // TODO(规则书)（可以从手牌中打出此卡）: only the *start* is gated on the main
    // move -- the book never says 「此次移动」 is the main move (OPEN-移动起点).
    None
}

fn your_light(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」
    // C# `H._turnCtx.Plan.Start = School; Plan.StartWhy = CardName`.
    // 规则书: 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」
    // C# `H._turnCtx.Plan.Start = School; Plan.StartWhy = CardName` =
    //   `plan::set_start(school, why)`.
    let school = ctx::tile_named("月之森女子学院");
    // TODO(规则书)（可以从手牌中打出此卡）: 「此次移动」 as the turn's main move is
    // unresolved (OPEN-移动起点) -- the start arm stays shut while the main move
    // is spent or blocked.
    if school >= 0 && rules_status::move_blocked(player_id).is_none() {
        ctx::plan::set_start(school, "你的光芒将照亮前路");
        // The log names the school as the start, so it belongs to the branch
        // that actually sets it (a board without the tile is a silent no-op).
        ctx::log(
            player_id,
            &Msg::new(key!("your_light_start")).player_id("who", player_id),
        ); // 规则书: 「此次移动以“月之森女子学院”为起点（不触发起点地块效果）」
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn your_light_kani(player_id: i32) -> card_sdk::Asked {
    your_light(player_id)
}
