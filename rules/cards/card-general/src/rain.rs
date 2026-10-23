//! `通用:雨啊，快点来吧` -- C# `CardRain` (MatchHost.cs:2221-2256): 2d2 players
//!
//! 规则书（docs/rulebook/cards.json, id `通用:雨啊，快点来吧`）:
//! > 雨啊，快点来吧：
//! > [手]：
//! > 投掷2d2并记录结果为X。[指定]X名玩家（其中必须包括[使用者]），被[指定]的玩家获得一层[停留]。
//!
//! gain a [停留] layer, the user among them.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{CardText, para, sec};

pub const RAIN: CardDef = CardDef::new("通用:雨啊，快点来吧", &[On::Play("", None, None, rain)])
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
    let x = ctx::n(1, ctx::roll(player_id, 2, 2));
    // 规则书[手]: 「[指定]X名玩家（其中必须包括[使用者]）」 -- the user is always
    // one of them (C# `targets = { i }`), the rest come one prompt at a time.
    let mut targets: Vec<i32> = Vec::new();
    targets.push(player_id);
    let mut pool: Vec<i32> = ctx::others(player_id);
    while (targets.len() as i32) < x && !pool.is_empty() {
        // C# `H.PickTarget(c, others, ...)`: a player prompt over the candidates
        // (`!Out && !exile`), then the `H.Target` targeting gate on the answer.
        let who = ctx::ask_player(
            player_id,
            &Msg::new(key!("rain_title")),
            &Msg::new(key!("rain_ask"))
                .i("n", targets.len() as i64 + 1)
                .i("x", x as i64),
            &pool,
        )?;
        // 规则书[手]: 「[指定]X名玩家」 -- the gate (out / exile / ImmuneAll /
        // Untargetable / Redirect / the `target` [反击] window). C# `PickTarget`
        // answers `t.yes ? t.index : -1` and a failed pick ends the loop.
        // `Some(hit)` is the player actually hit (a `Redirect` hook may have moved
        // it), which is what joins `targets` and leaves the pool.
        let hit = match ctx::target(who) {
            Some(hit) => hit,
            None => break,
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
