//! `Mor:（Rui）正论恶魔` -- C# `CardRuiDevil`: pay 100 to every other player,
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（Rui）正论恶魔`）:
//! > （Rui）正论恶魔：向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，每当你的技能被触发后，立即将x设置为5
//!
//! Sheet 2026-10-06 新卡组卡 G15: 「每当…立即将x设置为5」 (was 「若你的技能
//! 被触发，将x设置为5」) -- every skill fire during the resolution, immediately.

use card_sdk::CardText;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};

pub const RUI_DEVIL: CardDef = CardDef::new(
    "Mor:（Rui）正论恶魔",
    &[
        On::Play("", None, rui_devil),
        // 规则书: 「在此卡结算过程中，每当你的技能被触发后，立即将x设置为5」.
        // `resolving(…)` keeps the fire inside this card's resolution;
        // `by == owner` says 「你的技能」 -- the fire's cause is a skill of this
        // card's user (`actor` cannot: on a payment it is the payer, and the
        // 300 leg pays *to* the user). `owner` / `self_uid()` cannot scope the
        // frame either -- this card never places itself.
        On::Action(Action::SkillFired(
            "resolving('Mor:（Rui）正论恶魔') && by == owner",
            None,
            on_skill,
        )),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"正论恶魔"#)
        .raw_name(r#"（Rui）正论恶魔"#)
        .tags(&[])
        .text(CardText::para(r#"向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，每当你的技能被触发后，立即将x设置为5"#))
        // sheet cell G15
        .fields(&[("group", r#"Morfonica"#), ("owner", r#"八潮瑠唯"#)]));

/// The Rui character skill's X counter (`skill:八潮瑠唯:正论暴击`).
const X: &str = "skill.yuriCrit.x";

fn rui_devil(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    let why = Msg::new(key!("rui_devil_why"));
    // Outbound 100s are `fixedAmount` in C#, but the host has no such flag:
    // `ctx::transfer` runs the ordinary pay pipeline, so the user's own
    // 「正论暴击」 fires on the leg (`On::Action(Pay)`, ABI v74).
    for &o in &others {
        // 规则书: 「向全场玩家支付100资金（该数值不可被任何效果影响）」
        // TODO(规则书)（该数值不可被任何效果影响）: the 100 is immune to every effect --
        //   we ride the modifiable pipeline, so a crit hit doubles the leg to 200;
        //   needs a no-modifier pay tranche (TODO(ABI)).
        ctx::transfer(player_id, o, 100, &why)?;
    }
    for &o in &others {
        if !ctx::player_out(o) {
            // 规则书: 「之后全场玩家向你支付300资金（可被影响）」
            ctx::transfer(o, player_id, 300, &why)?;
        }
    }
    // 规则书: 「在此卡结算过程中，每当你的技能被触发后，立即将x设置为5」 -- the
    // write rides `on_skill`, once per fire and none without one.
    //
    // The hook raises at the skill's body *entry*, so the skill's own effect
    // still runs after it and may move X again (「正论暴击」 misses add 1). The
    // book's 「立即」 is after the trigger, so the fire's write has the last
    // word: a fire during this resolution re-writes 5 here, once, at the end.
    if ctx::slot(player_id, FIRED) > 0 {
        ctx::set_slot(player_id, FIRED, 0);
        ctx::set_slot(player_id, X, 5);
    }
    Ok(())
}

/// 规则书: 「每当你的技能被触发后，立即将x设置为5」 -- one write per fire, and a
/// flag so the body's end can re-assert it over the skill's own effect.
pub fn on_skill(player_id: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, X, 5);
    ctx::set_slot(player_id, FIRED, 1);
    ctx::log(player_id, &Msg::new(key!("rui_devil_crit")));
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn rui_devil_kani(player_id: i32) -> card_sdk::Asked {
    rui_devil(player_id)
}

/// One fire happened during this resolution (cleared by the body's tail).
const FIRED: &str = "rui_devil.fired";
