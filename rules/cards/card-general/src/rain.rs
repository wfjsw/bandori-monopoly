//! `通用:雨啊，快点来吧` -- C# `CardRain` (MatchHost.cs:2221-2256): 2d2 players
//!
//! 规则书（docs/rulebook/cards.json, id `通用:雨啊，快点来吧`）:
//! > 雨啊，快点来吧：
//! > [手]：
//! > 投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。
//!
//! gain a [停留] layer, the user among them.

use alloc::vec::Vec;
use card_sdk::abi::{roll_source, RollPurpose};
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const RAIN: CardDef = CardDef::new("通用:雨啊，快点来吧", &[On::Play("", None, rain)])
    .meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"雨啊，快点来吧"#)
            .raw_name(r#"雨啊，快点来吧"#)
            .tags(&[r#"手"#])
            .text(CardText::of(&[
                sec(r#"[手]："#, &[
                    para(r#"投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。"#),
                ]),
            ]))
            // sheet cell A9
            .fields(&[("group", r#"通用"#), ("owner", "")]),
    );

fn rain(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「投掷2d2并记录结果为X」
    // 投掷2d2并记录结果为X -- `ctx::n(1, ...)` so 「后勤人员的努力」 can double
    // this card's first number (C# `x = (c.Doubled == 0) ? x * 2 : x`).
    let r = ctx::roll(player_id, 2, 2, RollPurpose::Effect, roll_source::CARD)?;
    let x = ctx::n(1, r);
    // 规则书[手]: 「[指定]X名玩家（其中必须包括[使用者]）」 -- the user is always
    // one of them (C# `targets = { i }`), the rest come one prompt at a time.
    let mut targets: Vec<i32> = Vec::new();
    // 规则书[手]: 「[指定]X名玩家（其中必须包括[使用者]）」 -- through
    // `ctx::target` like the others, so a cancelled designation of the user counts.
    if let Some(hit) = ctx::target(player_id) {
        targets.push(hit);
    }
    let mut pool: Vec<i32> = ctx::others(player_id);
    while (targets.len() as i32) < x && !pool.is_empty() {
        // C# `H.PickTarget(c, others, ...)`: a player prompt over the other
        // living players (`ctx::others` is `!Out`; an exiled seat is still
        // offered), then the `H.Target` targeting gate on the answer.
        let who = ctx::ask_player(
            player_id,
            &Msg::new(key!("rain_title")),
            &Msg::new(key!("rain_ask"))
                .i("n", targets.len() as i64 + 1)
                .i("x", x as i64),
            &pool,
        )?;
        // 规则书[手]: 「[指定]X名玩家」 -- the gate (out / exile / ImmuneAll /
        // Untargetable / Redirect / the `target` [反击] window). `Some(hit)` is
        // the player actually hit (a `Redirect` hook may have moved it), which is
        // what joins `targets` and leaves the pool.
        // C# `PickTarget` ends the loop on a refused pick; the sheet wants X
        // names, so drop the refused seat and ask again.
        let hit = match ctx::target(who) {
            Some(hit) => hit,
            None => {
                pool.retain(|&s| s != who);
                continue;
            }
        };
        targets.push(hit);
        pool.retain(|&s| s != hit);
    }
    // 规则书[手]: 「被[指定]的玩家获得一层[停留]」
    for t in targets {
        rules_status::stay::add(t, 1);
        ctx::log(t, &Msg::new(key!("rain_stay")).player_id("who", t));
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn rain_kani(player_id: i32) -> card_sdk::Asked {
    rain(player_id)
}
