//! `MyGO:轮符雨` -- C# `CardRinneRain` (MatchHost.cs:6348-6363): gain one
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:轮符雨`）:
//! > 轮符雨： 
//! >  使自己获得一层[停留]并在回合结束时额外进行一次[触发结算]
//!
//! [停留] layer and book an extra [触发结算] at this turn's end.

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::abi::ActionKind;

pub const RINNE_RAIN: CardDef = CardDef::new(
    "MyGO:轮符雨",
    &[
        On::Play("", "", None, rinne_rain),
        // 「并在回合结束时额外进行一次[触发结算]」 is a **scheduling** clause
        // (`docs/TILES.md`), not a tile fact: `On::AtEnd` runs it at the turn
        // end, and its body is a plain `ctx::settle`.
        // The body-top `player_pos < 0` early-out is the condition now: a
        // player not standing on a square has nothing to settle.
        On::Action(&[ActionKind::AtEnd], "owner.pos >= 0", "", None, settle_now),
    ],
)
.props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"轮符雨"#)
        .raw_name(r#"轮符雨"#)
        .tags(&[])
        .text(r#"使自己获得一层[停留]并在回合结束时额外进行一次[触发结算]"#)
        .cell(r#"I4"#)
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", "")]));

fn rinne_rain(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「使自己获得一层[停留]」 -- C# `H.GiveStay(i, 1, i, CardName)`.
    rules_status::stay::add(player_id, 1);
    // 规则书: 「并在回合结束时额外进行一次[触发结算]」 -- C# `H._turnCtx.SettleAtEnd++`.
    // A scheduled turn-end rule op (`docs/TILES.md`): `On::AtEnd` runs at the
    // turn end and its body settles the square the player is on.
    ctx::before_turn_end(player_id);
    ctx::log(player_id, &Msg::new(key!("rinne_rain_log")));
    Ok(())
}

/// 规则书: 「并在回合结束时额外进行一次[触发结算]」 -- `H.SettleAt` on the
/// square the player is standing on. The engine's `ctx::settle` primitive, not
/// a counter the engine ticks (`docs/TILES.md`). Standing on a square is the
/// entry's condition (`owner.pos >= 0`).
fn settle_now(player_id: i32) -> card_sdk::Asked {
    let at = ctx::player_pos(player_id);
    ctx::card_settle_at(player_id, at, false);
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn rinne_rain_kani(player_id: i32) -> card_sdk::Asked {
    rinne_rain(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_now_kani(player_id: i32) -> card_sdk::Asked {
    settle_now(player_id)
}
