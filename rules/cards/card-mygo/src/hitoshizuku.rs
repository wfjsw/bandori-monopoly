//! `MyGO:壱雫空` -- C# `CardHitoshizuku` (MatchHost.cs:6364-6415): clear every
//! [停留] / [眩晕] on the table; each player pays the user 1,000 per effect
//! type cleared (the user gains instead of paying for their own).
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:壱雫空`）:
//! > 壱雫空：
//! >  清除场上所有[停留]与[眩晕]效果，所有玩家因本效果每清除一种效果则支付此卡使用者1000资金，若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金。（此卡可在眩晕时打出）
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const HITOSHIZUKU: CardDef = CardDef {
    id: "MyGO:壱雫空",
    play: Some(hitoshizuku),
    can_react: None,
    react: None,
    why_not: None,
};

fn hitoshizuku(seat: i32) {
    // (seat, number of effect kinds cleared there) -- C# `CardHitoshizuku`'s
    // `dictionary`, built while walking the table.
    let mut cleared: Vec<(i32, i32)> = Vec::new();
    for j in 0..ctx::seat_count() {
        if ctx::seat_out(j) {
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
        let stun = ctx::stun_of(j);
        if stun > 0 {
            ctx::give_stun(j, -stun);
            n += 1;
        }
        // TODO(规则书): 「清除场上所有[停留]与[眩晕]效果」 -- C# also zeroes
        // `matchSeat.stunStart` alongside `stun`; the vocabulary reads only
        // `stun_of` (the `stun` counter), so a pending `stunStart` survives.
        if n > 0 {
            cleared.push((j, n));
            // C# `H.Log("status", j, ...)` names the kinds when both went.
            if n == 2 {
                ctx::log(j, &Msg::new(key!("hitoshizuku_cleared_both")).seat("who", j));
            } else {
                ctx::log(j, &Msg::new(key!("hitoshizuku_cleared")).seat("who", j));
            }
        }
    }
    // TODO(规则书): the C# also un-skips the turn's move when the user's stay
    // drops to 0 (`H.State.skipMove = false`); needs a skip-move flag.
    for (j, n) in cleared {
        let amount = 1000 * n;
        if j == seat {
            // 规则书: 「若清除了此卡使用者受到的效果则每种效果使用者额外获得1000资金」
            // -- C# `item.Key == i` branch -> `H.GainR(i, 1000 * n, ...)`.
            ctx::gain(seat, amount, &Msg::new(key!("hitoshizuku_self_gain")));
        } else {
            // 规则书: 「所有玩家因本效果每清除一种效果则支付此卡使用者1000资金」 --
            // C# `H.PayR(j, i, 1000 * n, CardName, i)` (a seat-to-seat transfer).
            ctx::transfer(j, seat, amount, &Msg::new(key!("hitoshizuku_pay")));
        }
    }
    // TODO(规则书): 「（此卡可在眩晕时打出）」 -- C# `CardHitoshizuku.PlayableStunned`;
    // `CardDef` has no playable-stunned hook, so a stunned seat cannot declare
    // the card today.
}
