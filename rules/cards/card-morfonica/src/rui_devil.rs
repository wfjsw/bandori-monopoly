//! `Mor:（Rui）正论恶魔` -- C# `CardRuiDevil`: pay 100 to every other player,
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（Rui）正论恶魔`）:
//! > （Rui）正论恶魔：向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，每当你的技能被触发后，立即将x设置为5
//!
//! Sheet 2026-10-06 新卡组卡 G15: 「每当…立即将x设置为5」 (was 「若你的技能
//! 被触发，将x设置为5」) -- every skill fire during the resolution, immediately.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const RUI_DEVIL: CardDef = CardDef::new("Mor:（Rui）正论恶魔", &[On::Play(None, rui_devil, "")]);

/// The Rui character skill's X counter (`skill:八潮瑠唯:正论恶魔`).
const X: &str = "skill.yuriCrit.x";

fn rui_devil(player_id: i32) -> card_sdk::Asked {
    let others = ctx::others(player_id);
    let why = Msg::new(key!("rui_devil_why"));
    // Outbound 100s are `fixedAmount` in C# (no crit); the host has no such
    // flag yet and SkillRui's PayLast Fx hook is not in the ABI either.
    for &o in &others {
        ctx::transfer(player_id, o, 100, &why)?;
        // Sheet G15: 「每当你的技能被触发后，立即将x设置为5」 -- the skill
        // fires on each payment leg during the resolution; override X now.
        ctx::set_slot(player_id, X, 5);
    }
    for &o in &others {
        if !ctx::player_out(o) {
            ctx::transfer(o, player_id, 300, &why)?;
            ctx::set_slot(player_id, X, 5);
        }
    }
    ctx::set_slot(player_id, X, 5);
    ctx::log(
        player_id,
        &Msg::new(key!("rui_devil_crit")).card("card", "Mor:（Rui）正论恶魔"),
    );
    Ok(())
}
