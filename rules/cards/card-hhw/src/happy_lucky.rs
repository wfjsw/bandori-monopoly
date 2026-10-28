//! `HHW:Happy, Lucky, Smile, Yeah！` -- C# `CardHappyLucky` (MatchHost.cs:4093-4133):
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:Happy, Lucky, Smile, Yeah！`）:
//! > Happy, Lucky, Smile, Yeah！：
//! >  投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。
//!
//! roll 1d4 mod 4 and teleport along that axis to the farthest tile of the
//! board as drawn.

use card_sdk::abi::{roll_source, MoveKind, RollPurpose};
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const HAPPY_LUCKY: CardDef = CardDef::new(
    "HHW:Happy, Lucky, Smile, Yeah！",
    &[On::Play("", Some(cant_play), play)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Happy, Lucky, Smile, Yeah！"#)
        .raw_name(r#"Happy, Lucky, Smile, Yeah！"#)
        .tags(&[])
        .text(CardText::para(r#"投掷1d4mod4，对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子（例：在弦卷豪宅骰到4则传送到商店街），视为你的主要移动。"#))
        // sheet cell F10
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

/// C# `CardHappyLucky.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「视为你的主要移动」 -- the teleport is the turn's main move, so the
    // C# `H.MoveWhyNot` gate applies (off-turn / already-moved / skip-move refuse).
    rules_status::move_blocked(player_id)
}

/// The board's drawn cells -- the TTS `pathXY` fold the client renders
/// (`webui/src/scenes/board/Ring.tsx` `FOLD`): `[col, row]` on the 12 x 10
/// grid, tile 0 = CiRCLE at the bottom right, counter-clockwise. The C# `Grid`
/// ring (a 16 x 16 border) is *not* this layout: the drawn board folds each
/// side inward, so a ring column/row is not the board's farthest tile.
const CELLS: [(i32, i32); 60] = [
    (11, 9),
    (10, 9),
    (9, 9),
    (8, 9),
    (7, 9),
    (7, 8),
    (7, 7),
    (6, 7),
    (5, 7),
    (4, 7),
    (4, 8),
    (4, 9),
    (3, 9),
    (2, 9),
    (1, 9),
    (0, 9),
    (0, 8),
    (0, 7),
    (0, 6),
    (1, 6),
    (2, 6),
    (3, 6),
    (3, 5),
    (3, 4),
    (3, 3),
    (2, 3),
    (1, 3),
    (0, 3),
    (0, 2),
    (0, 1),
    (0, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (4, 1),
    (4, 2),
    (5, 2),
    (6, 2),
    (7, 2),
    (7, 1),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 0),
    (11, 1),
    (11, 2),
    (11, 3),
    (10, 3),
    (9, 3),
    (8, 3),
    (8, 4),
    (8, 5),
    (8, 6),
    (9, 6),
    (10, 6),
    (11, 6),
    (11, 7),
    (11, 8),
];

/// Drawn grid cell of tile `i`.
fn cell(i: i32) -> (i32, i32) {
    CELLS[i.rem_euclid(60) as usize]
}

/// The farthest tile from `(x, y)` in direction `num` (1 上, 2 下, 3 左, else
/// 右) along the drawn row/column -- `-1` when no tile lies that way.
fn farthest(x: i32, y: i32, num: i32) -> i32 {
    let mut best = -1;
    let mut best_key = i32::MAX;
    for (i, &(cx, cy)) in CELLS.iter().enumerate() {
        let (along, key) = match num {
            1 => (cx == x && cy < y, cy),
            2 => (cx == x && cy > y, -cy),
            3 => (cy == y && cx < x, cx),
            _ => (cy == y && cx > x, -cx),
        };
        if along && key < best_key {
            best_key = key;
            best = i as i32;
        }
    }
    best
}

fn play(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（对应投掷结果1-4沿上，下，左，右其中之一的方向）: the effect is
    // unconditional -- the drawn-board grid below is fixed at 60 tiles, so any
    // other board silently no-ops.
    if ctx::tile_count() != 60 {
        return Ok(());
    }
    // 规则书: 「投掷1d4mod4」 -- C# `H.CardRoll(c, 1, 4, CardName) % 4`.
    let r = ctx::roll(player_id, 1, 4, RollPurpose::Effect, roll_source::CARD)?;
    let num = r.rem_euclid(4);
    // 规则书: 「对应投掷结果1-4沿上，下，左，右其中之一的方向传送至直线距离最远的格子
    // （例：在弦卷豪宅骰到4则传送到商店街）」 -- the drawn board's row/column ray
    // in that direction (1=上, 2=下, 3=左, 4=右 i.e. `num == 0`), the edge tile
    // itself when already on that edge (as C# `FromGrid` resolved a miss).
    // Example: 弦卷豪宅 (index 29) is [0, 1]; a 4 goes right to index 46 = 商店街.
    // TODO(规则书)（对应投掷结果1-4沿上，下，左，右其中之一的方向）: the
    // result -> direction map and face 4 -> 右 are C# `Grid`; OPEN-HHW-DIR, unruled.
    let p = ctx::player_pos(player_id);
    let (x, y) = cell(p);
    let far = farthest(x, y, num);
    let to = if far >= 0 { far } else { p };
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
