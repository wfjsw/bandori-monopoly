//! `Mor:（Rui）正论恶魔` -- C# `CardRuiDevil`: pay 100 to every other player,
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（Rui）正论恶魔`）:
//! > （Rui）正论恶魔：向全场玩家支付100资金（该数值不可被任何效果影响），之后全场玩家向你支付300资金（可被影响）。在此卡结算过程中，若你的技能被触发，将x设置为5
//!
//! then take 300 from each; when the crit fires, X becomes 5.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const RUI_DEVIL: CardDef = CardDef::new("Mor:（Rui）正论恶魔", &[On::Play(None, rui_devil)]);

fn rui_devil(player_id: i32) -> card_sdk::Asked {
    ctx::set_slot(player_id, "ruiHit", 0);
    let others = ctx::others(player_id);
    let why = Msg::new(key!("rui_devil_why"));
    // Outbound 100s are `fixedAmount` in C# (no crit); the host has no such
    // flag yet and SkillRui's PayLast Fx hook is not in the ABI either.
    for &o in &others {
        ctx::transfer(player_id, o, 100, &why)?;
    }
    for &o in &others {
        if !ctx::player_out(o) {
            ctx::transfer(o, player_id, 300, &why)?;
        }
    }
    if ctx::slot(player_id, "ruiHit") > 0 {
        ctx::set_slot(player_id, "ruiX", 5);
        ctx::log(
            player_id,
            &Msg::new(key!("rui_devil_crit")).card("card", "Mor:（Rui）正论恶魔"),
        );
    }
    Ok(())
}
