//! `CRYCHIC:优雅的呐喊` -- C# `CardElegantShout` (MatchHost.cs:2912-2936): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:优雅的呐喊`）:
//! > 优雅的呐喊
//! >  ：[反击] 当你在回合外受到抽卡效果时打出，你的下回合结束时抽一张卡。
//!
//! on an out-of-turn draw, draw 1 at the end of your next turn.
//!
//! The engine raises `drawOut` at the `ctx::draw` chokepoint when the draw is
//! effect-caused and lands outside the drawer's own turn (wasm_rules
//! `HostRequest::Draw`). `pre::MINE` is the drawer (「你」).

use bandori_std::ctx;
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const ELEGANT_SHOUT: CardDef = CardDef::new(
    "CRYCHIC:优雅的呐喊",
    &[
        On::Counteract(
            &[ChainKind::DrawOut],
            card_sdk::pre::MINE,
            None,
            counteract).counter_reason(key!("elegant_shout_when")),
        On::Action(Action::AtEnd("", None, at_end)),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"优雅的呐喊"#)
        .raw_name(r#"优雅的呐喊"#)
        .tags(&[r#"反击"#])
        .text(r#"[反击] 当你在回合外受到抽卡效果时打出，你的下回合结束时抽一张卡。"#)
        // sheet cell M6
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]),
);

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「你的下回合结束时抽一张卡」
    ctx::log(
        player_id,
        &Msg::new(key!("elegant_shout_note")).player_id("who", player_id),
    );
    // C# `H.ExtraOf<ElegantFx>(c.Seat).Count++` -- each counteraction stacks one
    // draw; scheduling one `AtEnd` per counteraction stacks the same way.
    ctx::at_next_turn_end(player_id);
    Ok(())
}

/// C# `ElegantFx.TurnEnd` -- one draw at the end of the player's next turn.
fn at_end(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「你的下回合结束时抽一张卡」
    ctx::draw(player_id, 1)?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_end_kani(player_id: i32) -> card_sdk::Asked {
    at_end(player_id)
}
