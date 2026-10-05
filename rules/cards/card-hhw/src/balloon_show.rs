//! `HHW:热气球演出` -- C# `CardBalloonShow` (MatchHost.cs:4134-4162).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:热气球演出`）:
//! > 热气球演出：
//! >  投掷4次3d20并记录其结果，选择其中之一，传送至结果对应序号的格子，视为你的主要移动
//!

use card_sdk::{ctx, key, CardDef, Msg};
use alloc::vec::Vec;

pub const BALLOON_SHOW: CardDef = CardDef {
    id: "HHW:热气球演出",
    play: Some(play),
    can_react: None,
    react: None,
    // TODO(ABI): C# `CardBalloonShow.WhyNot` is `H.MoveWhyNot(seat)` (refuses
    // after the main move); that hook is still missing.
    why_not: None,
};

fn play(seat: i32) {
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    // 规则书: 「投掷4次3d20并记录其结果」 -- C# `H.Roll(seat, 3, 20, ...)` four times.
    let mut tiles: Vec<i32> = Vec::new();
    for _ in 0..4 {
        let r = ctx::roll(seat, 3, 20);
        let t = (r - 1).rem_euclid(n);
        if !tiles.contains(&t) {
            tiles.push(t);
        }
    }
    if tiles.is_empty() {
        return;
    }
    // 规则书: 「选择其中之一」 -- C# `H.AskTileOf` over the distinct rolled tiles.
    let title = Msg::new(key!("balloon_show_ask_title"));
    let text = Msg::new(key!("balloon_show_ask_text"));
    let to = ctx::ask_tile(seat, &title, &text, &tiles);
    // 规则书: 「传送至结果对应序号的格子」 -- C# `H.CardMove(..., TeleportTo)`.
    ctx::teleport_to(seat, to);
    ctx::log(seat, &Msg::new(key!("balloon_show_moved")).seat("who", seat).tile("tile", to));
    // TODO(规则书): 「视为你的主要移动」 -- needs the H.CardMove / main-move routine
    // so this teleport consumes the turn's main move (C# `H.CardMove(c, new MoveCtx
    // { TeleportTo = ... })`). Until then the seat still gets its normal main move
    // after the teleport.
}