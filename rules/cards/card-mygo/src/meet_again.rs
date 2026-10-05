//! `MyGO:若能再次交汇` -- C# `CardMeetAgain` (MatchHost.cs:6915): after your
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:若能再次交汇`）:
//! > 若能再次交汇：
//! > [反击]移动掷骰后，且场上有可被经过的玩家时可打出，持续进行移动掷骰直至[经过]下一名玩家
//!
//! move roll, keep re-rolling movement dice until the total passes the next
//! player in the direction of travel.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const MEET_AGAIN: CardDef = CardDef {
    id: "MyGO:若能再次交汇",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// Distance ahead of `seat` to the nearest other player who can be passed
/// (C# `CardMeetAgain.Next` over `H.Forward`). Forward-only: see the TODO on
/// `MoveCtx.Dir` below.
fn next_dist(seat: i32) -> i32 {
    let n = ctx::tile_count();
    let pos = ctx::seat_pos(seat);
    if n <= 0 || pos < 0 {
        return -1;
    }
    let mut best = i32::MAX;
    for o in ctx::others(seat) {
        let p = ctx::seat_pos(o);
        if p < 0 {
            continue;
        }
        // C# `H.Forward(pos, p)` -- ring steps ahead of `pos` to `p`.
        let d = ctx::tile_forward(pos, p);
        if d > 0 && d < best {
            best = d;
        }
    }
    if best == i32::MAX { -1 } else { best }
}

fn can_react(seat: i32) -> bool {
    // 规则书: 「[反击]移动掷骰后」 -- C# `t.Kind == "moveRoll" && t.Seat == seat`.
    if trigger::kind() != TriggerKind::MoveRoll || trigger::seat() != seat {
        return false;
    }
    // 规则书: 「且场上有可被经过的玩家时可打出」 -- C# `!t.Move.Teleport && Next(t.Move) > 0`.
    trigger::move_roll().is_some() && next_dist(seat) > 0
}

fn react(seat: i32) {
    // 规则书: 「持续进行移动掷骰直至[经过]下一名玩家」
    let Some(mut roll) = trigger::move_roll() else { return };
    let dist = next_dist(seat);
    if dist <= 0 {
        return;
    }
    for _ in 0..30 {
        if roll >= dist {
            break;
        }
        // C# `H.DoMoveRoll(move)` re-rolls the move's dice; the move's base is
        // 1d20 (C# `MoveCtx.Base`).
        let n = ctx::roll(seat, 1, 20).max(0);
        roll += n;
        trigger::set_move_roll(roll);
        // 规则书: 「持续进行移动掷骰直至[经过]下一名玩家」 -- each re-roll is logged
        // with the running total and the distance still to go.
        ctx::log(
            seat,
            &Msg::new(key!("meet_again_reroll"))
                .i("add", n as i64)
                .i("total", roll as i64)
                .i("dist", dist as i64),
        );
    }
    // TODO(规则书): 「[经过]下一名玩家」 in the direction of travel -- the C#
    // `Next` picks by `MoveCtx.Dir` (H.Forward(pos, p) vs H.Forward(p, pos)); the
    // trigger carries no movement direction, so this port always looks forward
    // (matching `game-core`'s forward-only main move today). Needs MoveCtx.Dir
    // on the move trigger.
    // TODO(规则书): the C# re-roll is `H.DoMoveRoll(move)`, which also sums the
    // move's extra dice (`MoveCtx.Dice`) and bonus; this port re-rolls the bare
    // base 1d20. Needs an H.DoMoveRoll movement-dice routine.
    // TODO(规则书): the C# `CanReact` also refuses teleports (`!t.Move.Teleport`);
    // the trigger exposes only the roll, so a teleport with a non-zero roll would
    // pass the guard. Needs a teleport flag on the move trigger.
}
