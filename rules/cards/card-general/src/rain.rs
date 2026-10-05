//! `通用:雨啊，快点来吧` -- C# `CardRain` (MatchHost.cs:2221-2256): 2d2 players
//! gain a [停留] layer, the user among them.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:雨啊，快点来吧`）:
//! > 雨啊，快点来吧：
//! > [手]：
//! > 投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。
//!

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg};

pub const RAIN: CardDef = CardDef {
    id: "通用:雨啊，快点来吧",
    play: Some(rain),
    can_react: None,
    react: None,
    why_not: None,
};

fn rain(seat: i32) {
    // 规则书[手]: 「投掷2d2并记录结果为X」
    let x = ctx::roll(seat, 2, 2);
    // TODO(规则书): the C# `x = (c.Doubled == 0) ? x * 2 : x` doubles X when the
    // band skill 「后勤人员的努力」 chose to double the card's number
    // (`PlayCtx.Doubled`); the ABI has no PlayCtx numbers yet.
    // 规则书[手]: 「[指定]X名玩家（其中必须包括[使用者]）」 -- the user is always
    // one of them (C# `targets = { i }`), the rest come one prompt at a time.
    let mut targets: Vec<i32> = Vec::new();
    targets.push(seat);
    let mut pool: Vec<i32> = ctx::others(seat);
    while (targets.len() as i32) < x && !pool.is_empty() {
        // C# `H.PickTarget` also runs the `H.Target` targeting gate
        // (Untargetable check) after the seat prompt -- no such gate in the
        // vocabulary yet.
        let who = ctx::ask_seat(
            seat,
            &Msg::new(key!("rain_title")),
            &Msg::new(key!("rain_ask")).i("n", targets.len() as i64 + 1).i("x", x as i64),
            &pool,
        );
        targets.push(who);
        pool.retain(|&s| s != who);
    }
    // 规则书[手]: 「被[指定]的玩家获得一层[停留]」
    for t in targets {
        ctx::give_stay(t, 1);
        ctx::log(t, &Msg::new(key!("rain_stay")).seat("who", t));
    }
}