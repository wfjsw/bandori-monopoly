//! `CRYCHIC:是我自己的问题` -- C# `CardMyOwnProblem` (MatchHost.cs:2963-3017):
//! [反击] before settle, step 1 tile away from the nearest player.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:是我自己的问题`）:
//! > 是我自己的问题
//! >  ：[反击]
//! > （1）主要移动结束时，[触发结算]前打出此卡，使自己额外远离绝对距离最近的玩家一格（若距离最近的玩家在身前则向后移动，若与其他玩家重合则可选择任意方向）。
//! > （2）若受到[异常移动效果]影响，此卡不生效
//!

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const MY_OWN_PROBLEM: CardDef = CardDef {
    id: "CRYCHIC:是我自己的问题",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// `H.Nearest(seat)` -- every other seat at the smallest ring distance.
fn nearest(seat: i32) -> Vec<i32> {
    let pos = ctx::seat_pos(seat);
    let others = ctx::others(seat);
    let Some(best) = others.iter().map(|&p| ctx::dist(pos, ctx::seat_pos(p))).min() else {
        return Vec::new();
    };
    others.into_iter().filter(|&p| ctx::dist(pos, ctx::seat_pos(p)) == best).collect()
}

fn can_react(seat: i32) -> bool {
    // 规则书[反击]（1）: 「主要移动结束时，[触发结算]前打出此卡」 -- C#
    // `t.Kind == "settleBefore" && t.Seat == seat && t.Move != null && t.Move.Main`.
    if trigger::kind() != TriggerKind::SettleBefore || trigger::seat() != seat {
        return false;
    }
    // C# `t.Move.Main` -- the trigger payload carries no move context yet
    // (TODO(ABI)), so any settle-before on this seat looks reactable.
    // 规则书[反击]（1）: needs a second player to move away from (C# `H.Others(seat).Count > 0`).
    !ctx::others(seat).is_empty()
}

fn react(seat: i32) {
    // 规则书[反击]（2）: 「若受到[异常移动效果]影响，此卡不生效」 -- C#
    // `H._abnormalTurn[i] > 0` -> `c.Effective = false`.
    // TODO(ABI): （2） 「若受到[异常移动效果]影响，此卡不生效」 -- needs the
    //   abnormal-move counter (C# `H._abnormalTurn[seat]`, bumped whenever an
    //   `Abnormal` move hits the seat) and a `set_effective` hook for
    //   `c.Effective = false`. Until then the step always runs.
    let pos = ctx::seat_pos(seat);
    let near = nearest(seat);
    if near.is_empty() {
        return;
    }
    // 规则书[反击]（1）: 「远离绝对距离最近的玩家一格」 -- pick the tile one step
    // away from `near[0]`. C# writes `H.State.seats[i].pos` directly; the host's
    // `teleport_to` is that same raw position write (no settle, no teleport).
    let theirs = ctx::seat_pos(near[0]);
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let dir: i32 = if theirs == pos {
        // 规则书[反击]（1）: 「若与其他玩家重合则可选择任意方向」 -- C# `H.AskPick`
        // between 向前 / 向后.
        let pick = ctx::ask_pick(
            seat,
            &Msg::new(key!("my_own_problem_title")),
            &Msg::new(key!("my_own_problem_ask")),
            &[
                Msg::new(key!("my_own_problem_forward")),
                Msg::new(key!("my_own_problem_backward")),
            ],
        );
        if pick == 1 {
            -1
        } else {
            1
        }
    } else {
        // 规则书[反击]（1）: 「若距离最近的玩家在身前则向后移动」 -- if the nearest is
        // behind (forward leg longer than half the ring) step forward, else back.
        if ctx::tile_forward(pos, theirs) > n / 2 {
            1
        } else {
            -1
        }
    };
    let to = (pos + dir).rem_euclid(n);
    // 规则书[反击]（1）: 「使自己额外远离绝对距离最近的玩家一格」
    ctx::teleport_to(seat, to);
    ctx::log(
        seat,
        &Msg::new(key!("my_own_problem_moved")).seat("who", seat).tile("tile", to),
    );
}