//! `MyGO:壱雫空` -- C# `CardHitoshizuku` (MatchHost.cs:6364-6415): clear every
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:壱雫空`）:
//! > 壱雫空：
//! >  清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。（此卡可在眩晕时打出）
//!
//! [停留] / [眩晕] on the table; each player pays the user 1,000 per effect
//! type cleared (the user gains instead of paying for their own).

use alloc::vec::Vec;

use card_sdk::abi::{prop, state_key};
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const HITOSHIZUKU: CardDef = CardDef::new("MyGO:壱雫空", &[On::Play(None, hitoshizuku, "")])
    // 规则书: 「（此卡可在眩晕时打出）」 -- the `playableStunned` property
    // (C# `Card.PlayableStunned`), skips the stun gate. The exile and no-hand
    // gates have no such exception.
    .props(&[(prop::PLAYABLE_STUNNED, 1)]);

fn hitoshizuku(player_id: i32) -> card_sdk::Asked {
    // (player, number of effect kinds cleared there) -- C# `CardHitoshizuku`'s
    // `dictionary`, built while walking the table.
    let mut cleared: Vec<(i32, i32)> = Vec::new();
    for j in 0..ctx::player_count() {
        if ctx::player_out(j) {
            continue;
        }
        let mut n = 0;
        // 规则书: 「清除场上所有[停留]与[眩晕]效果」 -- C# clamps stay down to
        // `H.V(j, "lockedStay")` (locked layers survive the clear) and zeroes
        // stun; `give_stay`/`give_stun` take negative counts to strip layers.
        let stay = ctx::stay_of(j);
        let locked = ctx::slot(j, "lockedStay").max(0);
        let keep = locked.min(stay);
        if stay > keep {
            ctx::give_stay(j, keep - stay);
            n += 1;
        }
        // 「[眩晕]」 is the `stun` counter **plus** a pending `stunStart` -- a stun
        // applied this turn that only starts counting next turn. Clearing the
        // effect clears both; a lone `stun` would leave the pending one behind.
        let stun = ctx::stun_of(j);
        let pending = ctx::state::get(j, state_key::STUN_START);
        if stun > 0 || pending > 0 {
            ctx::give_stun(j, -stun);
            ctx::state::set(j, state_key::STUN_START, 0);
            n += 1;
        }
        if n > 0 {
            cleared.push((j, n));
            // C# `H.Log("status", j, ...)` names the kinds when both went.
            if n == 2 {
                ctx::log(
                    j,
                    &Msg::new(key!("hitoshizuku_cleared_both")).player_id("who", j),
                );
            } else {
                ctx::log(
                    j,
                    &Msg::new(key!("hitoshizuku_cleared")).player_id("who", j),
                );
            }
        }
    }
    // The stay-clear un-skips the turn's move on its own (「[停留]：处于该状态时
    // [无法移动]」 -- the skip is a consequence of the state, not a latch the
    // card has to write; `game-core`'s `give_stay` clears `skip_move` when the
    // last layer goes and no [除外] holds the move either, the same as the C#'s
    // `H.State.skipMove = false`).
    //
    // Ruling 2026-10-06: 「每清除一种效果」 is per effect type, counted
    // separately for each affected player. 2 types cleared from A and 1 from B
    // counts 3. Every player pays 1000 × count; the user gains 1000 extra per
    // own type (「每种效果使用者额外获得1000资金」).
    //
    // The money moves through the pipeline (`transfer` / `gain` -> payAdd /
    // payMul / payChoose / payAt -> the `pay` [反击] window), so 「[支付]时可以
    // 打出」 windows open on each leg. The status clears above reach the live
    // world for the routine's duration (`Cx::overlay_guest_state`), so a payer
    // we just un-stunned is not blocked by `can_pay`.
    let count: i32 = cleared.iter().map(|&(_, n)| n).sum();
    if count > 0 {
        let each = 1000 * count;
        // 规则书: 「所有玩家因本效果每清除一种效果则支付此卡使用者1000资金」 --
        // every seat pays, the user included (a self-pay nets 0).
        for j in 0..ctx::player_count() {
            if ctx::player_out(j) {
                continue;
            }
            if j == player_id {
                // Self-pay nets nothing; keep the log so the count is visible.
                ctx::log(
                    player_id,
                    &Msg::new(key!("hitoshizuku_pay")).player_id("who", j),
                );
            } else {
                ctx::transfer(j, player_id, each, &Msg::new(key!("hitoshizuku_pay")))?;
            }
        }
    }
    // 规则书: 「若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金」
    for &(j, n) in &cleared {
        if j == player_id && n > 0 {
            ctx::gain(player_id, 1000 * n, &Msg::new(key!("hitoshizuku_self_gain")))?;
        }
    }
    Ok(())
}
