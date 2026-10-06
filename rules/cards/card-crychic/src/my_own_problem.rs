//! `CRYCHIC:是我自己的问题` -- C# `CardMyOwnProblem` (MatchHost.cs:2963-3017):
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:是我自己的问题`）:
//! > 是我自己的问题 
//! >  ：[反击]
//! > （1）主要移动结束时，[触发结算]前打出此卡，使自己额外远离绝对距离最近的玩家一格（若距离最近的玩家在身前则向后移动，若与其他玩家重合则可选择任意方向）。
//! > （2）若受到[异常移动效果]影响，此卡不生效
//!
//! [反击] before settle, step 1 tile away from the nearest player.

use alloc::vec::Vec;

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const MY_OWN_PROBLEM: CardDef = CardDef::new(
    "CRYCHIC:是我自己的问题",
    &[On::Counteract(&[ChainKind::SettleBefore], can_counteract, counteract)],
);

/// `H.Nearest(seat)` -- every other player at the smallest ring distance.
fn nearest(player_id: i32) -> Vec<i32> {
    let pos = ctx::player_pos(player_id);
    let others = ctx::others(player_id);
    let Some(best) = others
        .iter()
        .map(|&p| ctx::dist(pos, ctx::player_pos(p)))
        .min()
    else {
        return Vec::new();
    };
    others
        .into_iter()
        .filter(|&p| ctx::dist(pos, ctx::player_pos(p)) == best)
        .collect()
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]（1）: 「主要移动结束时，[触发结算]前打出此卡」 -- C#
    // `t.Kind == "settleBefore" && t.Seat == seat && t.Move != null && t.Move.Main`.
    if trigger::kind() != TriggerKind::SettleBefore || trigger::player_id() != player_id {
        return false;
    }
    if !trigger::move_is_main() {
        return false;
    }
    // 规则书[反击]（1）: needs a second player to move away from (C# `H.Others(seat).Count > 0`).
    !ctx::others(player_id).is_empty()
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]（2）: 「若受到[异常移动效果]影响，此卡不生效」 -- C#
    // `H._abnormalTurn[i] > 0` -> `c.Effective = false` (the card is still
    // consumed; only its effect body is skipped).
    if ctx::abnormal_count(player_id) > 0 {
        ctx::log(player_id, &Msg::new(key!("my_own_problem_ineffective")));
        return Ok(());
    }
    let pos = ctx::player_pos(player_id);
    let near = nearest(player_id);
    if near.is_empty() {
        return Ok(());
    }
    // 规则书[反击]（1）: 「远离绝对距离最近的玩家一格」 -- pick the tile one step
    // away from `near[0]`. C# writes `H.State.seats[i].pos` directly; the host's
    // `teleport_to` is that same raw position write (no settle, no teleport).
    let theirs = ctx::player_pos(near[0]);
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let dir: i32 = if theirs == pos {
        // 规则书[反击]（1）: 「若与其他玩家重合则可选择任意方向」 -- C# `H.AskPick`
        // between 向前 / 向后.
        let pick = ctx::ask_pick(
            player_id,
            &Msg::new(key!("my_own_problem_title")),
            &Msg::new(key!("my_own_problem_ask")),
            &[
                Msg::new(key!("my_own_problem_forward")),
                Msg::new(key!("my_own_problem_backward")),
            ],
        )?;
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
    ctx::teleport_to(player_id, to);
    ctx::log(
        player_id,
        &Msg::new(key!("my_own_problem_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    Ok(())
}
