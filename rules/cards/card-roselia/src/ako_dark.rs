//! `R:（亚子）黑暗大魔姬亚子` -- C# `CardAkoDark` (MatchHost.cs:10781-10814): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:（亚子）黑暗大魔姬亚子`）:
//! > （亚子）黑暗大魔姬亚子：
//! >  
//! > （1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。
//!
//! just before you pay another player: take a [眩晕] layer instead of paying.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::{it, CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const AKO_DARK: CardDef = CardDef::new(
    "R:（亚子）黑暗大魔姬亚子",
    &[On::Counteract(
        &[ChainKind::Effect],
        // 规则书（1）[反击]: 「当你即将向其他玩家支付资金时可打出此卡」 -- a pay
        // owed to another living player (`target >= 0 && target != owner`), not a
        // gain, with a positive amount.
        "actor == owner && effect.has(Pay) && target >= 0 && target != owner && value > 0",
        None,
        counteract).counter_reason(key!("ako_dark_when"))],
)
// 规则书（1）: 「若此卡从手牌以外的地方打出」 -- the off-hand half presupposes a
// non-hand playable zone; the field copy is offerable in the counteraction
// window only when the card says so (the hand copy is offered anyway).
.props(&[(card_sdk::abi::prop::COUNTERACT_FROM_FIELD, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"黑暗大魔姬亚子"#)
        .raw_name(r#"（亚子）黑暗大魔姬亚子"#)
        .tags(&[r#"反击"#])
        .text(CardText::items(&[
            it(r#"[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。"#),
        ]))
        // sheet cell E11
        .fields(&[("group", r#"Roselia"#), ("owner", r#"宇田川亚子"#)]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[反击]: 「使自己获得一层[眩晕]」 -- C# `H.GiveStun(i, 1, i, "黑暗大魔姬亚子")`.
    // The grant is refused behind [不可阻挡] / a cancelled abnormal window, so
    // the log follows the layer count instead of the call.
    let before = rules_status::stun::get(player_id);
    rules_status::stun::add(player_id, 1);
    if rules_status::stun::get(player_id) > before {
        ctx::log(
            player_id,
            &Msg::new(key!("ako_dark_stun")).player_id("who", player_id),
        );
    }
    // 规则书（1）[反击]: the pay is waived once the player is stunned (C# CounteractHint:
    // 「这笔就不用付了」) -- C# `c.Trigger.Pay.cancel = true` when `H.State.seats[i].Stunned`.
    if rules_status::stun::get(player_id) > 0 {
        trigger::set_pay_amount(0);
    }
    // 规则书（1）[反击]: 「若此卡从手牌以外的地方打出，你抽一张卡」 -- the
    // declaration's own origin: `is_placed()` is the field copy this
    // counteraction was declared from (`current_uid >= 0`); a hand declaration
    // runs with no instance at all. `ctx::play_from_hand()` is the *enclosing*
    // activation's origin (only a hand play sets it), so it says nothing here.
    // TODO(规则书)（若此卡从手牌以外的地方打出）: 书中假设此卡能留在手牌以外并打出 --
    //   现在没有效果会把它放到场上（反击结算后默认进弃牌堆），场地副本这条路仍走不到；
    //   `COUNTERACT_FROM_FIELD` 只是让场地副本在窗口里可选.
    if ctx::is_placed() {
        ctx::draw(player_id, 1)?;
        ctx::log(
            player_id,
            &Msg::new(key!("ako_dark_drawn")).player_id("who", player_id),
        );
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
