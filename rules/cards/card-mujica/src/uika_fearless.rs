//! `Mujica:（初华）我，无畏悲伤` -- C# `CardUikaFearless` (MatchHost.cs:5711-5743):
//! if you settled on a memory tile since CiRCLE, draw 1 or fix this turn's roll.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（初华）我，无畏悲伤`）:
//! > （初华）我，无畏悲伤：
//! >  打出此卡时若自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]，你可选择抽1张卡或使你本回合的投掷结果可定义为1-6以内的任何数字
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const UIKA_FEARLESS: CardDef = CardDef {
    id: "Mujica:（初华）我，无畏悲伤",
    play: Some(uika_fearless),
    can_react: None,
    react: None,
    why_not: None,
};

fn uika_fearless(seat: i32) {
    // 规则书: 「若自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]」 -- C#
    // `H.V(i, "uikaMemory")` is bumped on every memory-tile settle and cleared
    // when CiRCLE is passed. The slot is the stand-in; the engine hooks that
    // write it are missing (TODO below).
    let memory = ctx::slot(seat, "uikaMemory");
    if memory <= 0 {
        // C# `c.Effective = false` + a log line; no set_effective hook yet.
        ctx::log(seat, &Msg::new(key!("uika_fearless_no_memory")).seat("who", seat));
        return;
    }
    // 规则书: 「你可选择抽1张卡或使你本回合的投掷结果可定义为1-6以内的任何数字」
    let mut options = alloc::vec::Vec::new();
    options.push(Msg::new(key!("uika_fearless_draw")));
    // C# only offers the fix-roll option when `H.MoveWhyNot(i) == null &&
    // H._turnCtx.Plan.FixedRoll < 0`. `H.MoveWhyNot` is `State.turn == seat` plus
    // the main-move / skipMove checks; the turn half is expressible, the rest is
    // TODO'd below (and `Plan.FixedRoll` is still missing).
    let can_move = ctx::turn_seat() == seat;
    if can_move {
        options.push(Msg::new(key!("uika_fearless_fix")));
    }
    let pick = ctx::ask_pick(
        seat,
        &Msg::new(key!("uika_fearless_title")),
        &Msg::new(key!("uika_fearless_ask")),
        &options,
    );
    if pick == 1 && can_move {
        // 规则书: 「使你本回合的投掷结果可定义为1-6以内的任何数字」
        // C# `H.AskNumber(..., 1, 6, ...)` then `H._turnCtx.Plan.FixedRoll = n`.
        let n = ctx::ask_number(
            seat,
            &Msg::new(key!("uika_fearless_fix_title")),
            &Msg::new(key!("uika_fearless_fix_ask")),
            1,
            6,
        );
        // TODO(ABI): `H._turnCtx.Plan.FixedRoll = n` -- the fixed move-roll plan
        // has no ctx counterpart. Until then the chosen number is only logged.
        // TODO(规则书): the C# `H.MoveWhyNot` half of the gate -- refuses after
        // this turn's main move (`_turnCtx.MainMoved`) and when the turn's move
        // is skipped (`State.skipMove`); needs `H.MoveWhyNot` in the ABI.
        ctx::log(
            seat,
            &Msg::new(key!("uika_fearless_fixed")).seat("who", seat).i("n", n as i64),
        );
    } else {
        // 规则书: 「抽1张卡」
        ctx::draw(seat, 1);
        ctx::log(seat, &Msg::new(key!("uika_fearless_drew")).seat("who", seat));
    }
    // TODO(规则书): 「自从上一次[经过]CiRCLE后有在任何[回忆地块][触发结算]」 -- the
    // `uikaMemory` counter needs the settle-on-memory-tile and pass-CiRCLE
    // engine hooks (C# `H.SetV(i, "uikaMemory", ...)`) so the slot is non-zero
    // exactly when the condition holds.
}