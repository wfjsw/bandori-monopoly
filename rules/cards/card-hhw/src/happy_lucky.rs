//! `HHW:Happy, Lucky, Smile, Yeah！` -- C# `CardHappyLucky` (MatchHost.cs:4093-4133):
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:Happy, Lucky, Smile, Yeah！`）:
//! > Happy, Lucky, Smile, Yeah！：
//! >  投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。
//!
//! roll 1d4 mod 4 and teleport along that axis to the farthest tile.

use card_sdk::abi::MoveKind;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const HAPPY_LUCKY: CardDef = CardDef::new(
    "HHW:Happy, Lucky, Smile, Yeah！",
    &[On::Play("", Some(cant_play), play)],
);

/// C# `CardHappyLucky.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「视为你的主要移动」 -- the teleport is the turn's main move, so the
    // C# `H.MoveWhyNot` gate applies (off-turn / already-moved / skip-move refuse).
    ctx::cant_move(player_id)
}

/// C# `MatchHost.Grid` -- the 60-tile board laid out on a 16x16 ring border:
/// bottom row right-to-left (0..=15), left column bottom-to-top (15..=30),
/// top row left-to-right (30..=45), right column top-to-bottom (45..=59).
fn grid(i: i32) -> (i32, i32) {
    if i <= 15 {
        (15 - i, 0)
    } else if i <= 30 {
        (0, i - 15)
    } else if i <= 45 {
        (i - 30, 15)
    } else {
        (15, 60 - i)
    }
}

/// C# `MatchHost.FromGrid` -- the inverse of [`grid`].
fn from_grid(x: i32, y: i32) -> i32 {
    if y == 0 {
        15 - x
    } else if x == 0 {
        15 + y
    } else if y == 15 {
        30 + x
    } else {
        (60 - y).rem_euclid(60)
    }
}

fn play(player_id: i32) -> card_sdk::Asked {
    // The C# grid below is the fixed 60-tile ring; bail out on any other board.
    if ctx::tile_count() != 60 {
        return Ok(());
    }
    // 规则书: 「投掷1d4mod4」 -- C# `H.CardRoll(c, 1, 4, CardName) % 4`.
    let r = ctx::roll(player_id, 1, 4);
    let num = r.rem_euclid(4);
    // 规则书: 「对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子
    // （例：在弦卷豪宅骰到4则传送到商店街）」 -- C# `Grid`/`FromGrid`: the face
    // 1=上 (y=15), 2=下 (y=0), 3=左 (x=0), 4=右 (x=15, i.e. `num == 0`); the
    // destination is that axis's far border tile. Example: 弦卷豪宅 (#30,
    // index 29) is (0, 14); a 4 goes right to (15, 14) = index 46 = 商店街.
    let (x, y) = grid(ctx::player_pos(player_id));
    let to = match num {
        1 => from_grid(x, 15), // 上
        2 => from_grid(x, 0),  // 下
        3 => from_grid(0, y),  // 左
        _ => from_grid(15, y), // 右 (face 4 -> num 0)
    };
    // 规则书: 「传送至直线距离最远的格子」 -- C# `H.CardMove(c, new MoveCtx
    // { TeleportTo = to })` (`Resolve` defaults to true, so it settles).
    ctx::plan::set_kind(MoveKind::Teleport);
    ctx::plan::set_teleport_to(to);
    ctx::plan::set_resolve(true);
    // 规则书: 「视为你的主要移动」 -- C# `H.CardMove` (`MainMoveAs`) consumes the
    // turn's main move and runs the teleport immediately.
    ctx::card_move(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("happy_lucky_moved"))
            .player_id("who", player_id)
            .i("roll", r as i64)
            .tile("tile", to),
    );
    Ok(())
}
