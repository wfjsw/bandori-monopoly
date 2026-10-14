//! `R:（亚子）黑暗大魔姬亚子` -- C# `CardAkoDark` (MatchHost.cs:10781-10814): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:（亚子）黑暗大魔姬亚子`）:
//! > （亚子）黑暗大魔姬亚子： 
//! >  
//! > （1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。
//!
//! just before you pay another player: take a [眩晕] layer instead of paying.

use card_sdk::abi::ChainKind;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const AKO_DARK: CardDef = CardDef::new(
    "R:（亚子）黑暗大魔姬亚子",
    &[On::Counteract(
        &[ChainKind::Effect],
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0",
        None,
        counteract,
    )],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"黑暗大魔姬亚子"#)
        .raw_name(r#"（亚子）黑暗大魔姬亚子"#)
        .tags(&[r#"反击"#])
        .text(r#"（1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。"#)
        .cell(r#"E11"#)
        .fields(&[("band", r#"Roselia"#), ("owner", r#"宇田川亚子"#)]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[反击]: 「使自己获得一层[眩晕]」 -- C# `H.GiveStun(i, 1, i, "黑暗大魔姬亚子")`.
    rules_status::stun::add(player_id, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("ako_dark_stun")).player_id("who", player_id),
    );
    // 规则书（1）[反击]: the pay is waived once the player is stunned (C# CounteractHint:
    // 「这笔就不用付了」) -- C# `c.Trigger.Pay.cancel = true` when `H.State.seats[i].Stunned`.
    if rules_status::stun::get(player_id) > 0 {
        trigger::set_pay_amount(0);
    }
    // 规则书（1）[反击]: 「若此卡从手牌以外的地方打出，你抽一张卡」 -- the
    // play's origin: a card run through `ctx::play_card` did not come from a hand.
    if !ctx::play_from_hand() {
        ctx::draw(player_id, 1)?;
        ctx::log(
            player_id,
            &Msg::new(key!("ako_dark_drawn")).player_id("who", player_id),
        );
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
