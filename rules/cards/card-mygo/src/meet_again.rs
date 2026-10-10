//! `MyGO:若能再次交汇` -- C# `CardMeetAgain` (MatchHost.cs:6915): after your
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:若能再次交汇`）:
//! > 若能再次交汇：
//! > [反击]移动掷骰后，且场上有可被经过的玩家时可打出，持续进行移动掷骰直至[经过]下一名玩家（最多掷骰至移动超过原本移动终点的20格以后）。
//!
//! move roll, keep re-rolling movement dice until the total passes the next
//! player in the direction of travel.

use card_sdk::abi::{ChainKind, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MEET_AGAIN: CardDef = CardDef::new(
    "MyGO:若能再次交汇",
    &[On::Counteract(
        &[ChainKind::MoveRoll],
        "actor == owner && move.kind != Teleport && move.roll != null && next_dist(owner, move.dir) > 0",
        None,
        counteract,
    )],
)
.legacy(&[(0, legacy_can_counteract)]);

/// Distance in the direction of travel to the nearest other player who can be
/// passed (C# `CardMeetAgain.Next` over `H.Forward`, picked by `MoveCtx.Dir`).
fn next_dist(player_id: i32, dir: i32) -> i32 {
    let n = ctx::tile_count();
    let pos = ctx::player_pos(player_id);
    if n <= 0 || pos < 0 {
        return -1;
    }
    let mut best = i32::MAX;
    for o in ctx::others(player_id) {
        let p = ctx::player_pos(o);
        if p < 0 {
            continue;
        }
        // C# `Next`: `H.Forward(pos, p)` when moving forward (`Dir == 1`),
        // `H.Forward(p, pos)` when moving backward.
        let d = if dir >= 0 {
            ctx::tile_forward(pos, p)
        } else {
            ctx::tile_forward(p, pos)
        };
        if d > 0 && d < best {
            best = d;
        }
    }
    if best == i32::MAX {
        -1
    } else {
        best
    }
}

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    // 规则书: 「[反击]移动掷骰后」 -- C# `t.Kind == "moveRoll" && t.Seat == seat`.
    if trigger::player_id() != player_id {
        return false;
    }
    // 规则书: 「且场上有可被经过的玩家时可打出」 -- C# `!t.Move.Teleport && Next(t.Move) > 0`
    // (`t.Move.Teleport` is `trigger::move_kind() == Some(MoveKind::Teleport)`).
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return false;
    }
    trigger::move_roll().is_some() && next_dist(player_id, trigger::move_dir()) > 0
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.kind != Teleport && move.roll != null &&
    // next_dist(owner, move.dir) > 0` is the pre.
    // 规则书: 「持续进行移动掷骰直至[经过]下一名玩家」
    let Some(mut roll) = trigger::move_roll() else {
        return Ok(());
    };
    let dist = next_dist(player_id, trigger::move_dir());
    // 规则书: 「（最多掷骰至移动超过原本移动终点的20格以后）」 -- the original
    // endpoint is the roll this card answered; re-rolls may push at most 20 past it.
    let origin_end = roll;
    let cap = origin_end + 20;
    for _ in 0..30 {
        if roll >= dist || roll >= cap {
            break;
        }
        // 规则书: `H.DoMoveRoll(move)` re-rolls the move's whole dice table
        // (base + extra dice + flat bonuses).
        let n = ctx::do_move_roll(player_id).max(0);
        roll += n;
        trigger::set_move_roll(roll);
        // 规则书: 「持续进行移动掷骰直至[经过]下一名玩家」 -- each re-roll is logged
        // with the running total and the distance still to go.
        ctx::log(
            player_id,
            &Msg::new(key!("meet_again_reroll"))
                .i("add", n as i64)
                .i("total", roll as i64)
                .i("dist", dist as i64),
        );
    }
    Ok(())
}
