//! `MyGO:壱雫空` -- C# `CardHitoshizuku` (MatchHost.cs:6364-6415): clear every
//! [停留] / [眩晕] on the table; each player pays the user 1,000 per effect
//! type cleared (the user gains instead of paying for their own).
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:壱雫空`）:
//! > 壱雫空：
//! >  清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。（此卡可在眩晕时打出）
//!

use alloc::vec::Vec;

use card_sdk::abi::state_key;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const HITOSHIZUKU: CardDef = CardDef::new("MyGO:壱雫空", &[On::Play(None, hitoshizuku)]);

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
    // TODO(ABI): the C# also un-skips the turn's move when the user's stay
    // drops to 0 (`H.State.skipMove = false`); needs a skip-move flag.
    for (j, n) in cleared {
        let amount = 1000 * n;
        if j == player_id {
            // 规则书: 「若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金」
            // -- C# `item.Key == i` branch -> `H.GainR(i, 1000 * n, ...)`.
            ctx::gain(player_id, amount, &Msg::new(key!("hitoshizuku_self_gain")));
        } else {
            // 规则书: 「所有玩家因本效果每清除一种效果则支付此卡使用者1000资金」 --
            // C# `H.PayR(j, i, 1000 * n, CardName, i)` (a player-to-player transfer).
            ctx::transfer(j, player_id, amount, &Msg::new(key!("hitoshizuku_pay")))?;
        }
    }
    // TODO(ABI): 「（此卡可在眩晕时打出）」 -- C# `CardHitoshizuku.PlayableStunned`;
    // `CardDef` has no playable-stunned hook, so a stunned player cannot declare
    // the card today.
    Ok(())
}
