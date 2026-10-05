//! `PPP:（里美）我的心就像巧克力螺` -- C# `CardRimiChoco` (MatchHost.cs:9241-9285):
//! jump 1-4 tiles (either way) as the main move, unstoppable.
//!
//! 规则书（docs/rulebook/cards.json, id `PPP:（里美）我的心就像巧克力螺`）:
//! > （里美）我的心就像巧克力螺：
//! > [手]：
//! > 立刻进入移动阶段，本回合的[主要移动]改为移动到当前格子绝对距离1到4格或以内的任何格子并[结算]，期间[不可阻挡]。
//!

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg};

// TODO(规则书): C# `CardRimiChoco.WhyNot` is just `H.MoveWhyNot(seat)` (main move
//   already used / movement blocked) -- needs `H.MoveWhyNot`; `why_not` stays None.
pub const RIMI_CHOCO: CardDef = CardDef {
    id: "PPP:（里美）我的心就像巧克力螺",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    let n = ctx::tile_count();
    let pos = ctx::seat_pos(seat);
    if n <= 0 || pos < 0 {
        return;
    }
    // 规则书: 「移动到当前格子绝对距离1到4格或以内的任何格子」 -- C#
    // `CardRimiChoco.Play` offers the eight tiles at offsets ±1..±4.
    let mut tiles: Vec<i32> = Vec::new();
    for i in -4i32..=4 {
        if i != 0 {
            tiles.push((pos + i).rem_euclid(n));
        }
    }
    if tiles.is_empty() {
        return;
    }
    let to = ctx::ask_tile(
        seat,
        &Msg::new(key!("rimi_choco_title")),
        &Msg::new(key!("rimi_choco_ask")),
        &tiles,
    );
    // 规则书: 「期间[不可阻挡]」 -- C# `H._turnCtx.Unstoppable = true` for the move.
    // 规则书: 「并[结算]」 / 「本回合的[主要移动]改为…」 -- C# `H.CardMove` with
    // `Steps` (forward, or `Reverse` with `n - steps` when the target is behind).
    let steps = ctx::tile_forward(pos, to);
    let reverse = steps > 4;
    let _len = if reverse { n - steps } else { steps };
    ctx::log(
        seat,
        &Msg::new(key!("rimi_choco_moved"))
            .seat("who", seat)
            .tile("tile", to)
            .i("n", _len as i64),
    );
    // TODO(ABI): 「本回合的[主要移动]改为移动到…并[结算]，期间[不可阻挡]」 -- needs
    //   `H.CardMove` (so the walk settles on `to` and consumes the main move) and
    //   the `Unstoppable` turn flag (C# `H._turnCtx.Unstoppable`). Until then the
    //   seat is not moved; only the destination choice runs.
}