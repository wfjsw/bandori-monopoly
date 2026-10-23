//! `R:live前的准备` -- C# `CardBeforeLive` (MatchHost.cs:10673-10709): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:live前的准备`）:
//! > live前的准备：
//! > [反击] 经过江户川乐器店时可打出此卡，使自己在江户川乐器店强制停下并触发结算
//!
//! on passing 江户川乐器店: force yourself to stop there and settle.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::{CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const BEFORE_LIVE: CardDef = CardDef::new(
    "R:live前的准备",
    &[On::Counteract(
        &[ChainKind::Pass],
        "actor == owner && move.remaining > 0 && tile.id == tile_named('江户川乐器店')",
        Some(key!("before_live_when")),
        None,
        counteract,
    )],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"live前的准备"#)
        .raw_name(r#"live前的准备"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 经过江户川乐器店时可打出此卡，使自己在江户川乐器店强制停下并触发结算"#))
        // sheet cell E8
        .fields(&[("group", r#"Roselia"#), ("owner", "")]),
);

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.remaining > 0 && tile.id ==
    // tile_named('江户川乐器店')` is the pre; no residual guard.
    // 规则书[反击]: 「使自己在江户川乐器店强制停下并触发结算」
    ctx::log(
        player_id,
        &Msg::new(key!("before_live_stop"))
            .player_id("who", player_id)
            .tile("tile", ctx::tile_named("江户川乐器店")),
    );
    // 规则书[反击]: 「强制停下并触发结算」 -- C# `CardBeforeLive.Counteract`:
    // `m.Stopped = true; m.Resolve = true` behind `H.AbnormalGate`.
    if !ctx::gate(trigger::player_id(), bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    let shop = ctx::tile_named("江户川乐器店");
    if shop >= 0 {
        ctx::plan::set_stop_at(shop);
        ctx::plan::set_resolve(true);
    }
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
