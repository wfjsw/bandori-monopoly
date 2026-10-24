//! `Mor:迷茫之蝶们的三全音` -- C# `CardTritone` (MatchHost.cs:4524-4583): loan the
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:迷茫之蝶们的三全音`）:
//! > 迷茫之蝶们的三全音：
//! > [反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金
//!
//! pay amount back in three turns (crystal decay).
//!
//! Counteraction-only (`Normal => false`): the player is about to lose or pay money.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind, TriggerKind};
use card_sdk::{key, Action, CardDef, Msg, On};

pub const TRITONE: CardDef = CardDef::new(
    "Mor:迷茫之蝶们的三全音",
    &[
        On::Counteract(
            &[ChainKind::Effect],
            "actor == owner && effect.has(Pay) && value > 0",
            None,
            counteract).counter_reason(key!("tritone_when")),
        On::Action(Action::TurnEnd("", None, counteract)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"迷茫之蝶们的三全音"#)
        .raw_name(r#"迷茫之蝶们的三全音"#)
        .tags(&[r#"反击"#])
        .text(r#"[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金"#)
        // sheet cell G2
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书[反击]: 「任意时刻当你将要失去或支付资金时打出此卡」 -- the payment's
        // declaration raises the `effect` chain (kind `Effect`) with a `pay` entry,
        // so the counter answers that link (C# folds this into `t.Kind == "pay"`).
        TriggerKind::Effect => {
            let amount = trigger::value();
            // 规则书[反击]: 「立刻获得此次失去的资金金额」
            // `NEGATION-AUDIT` V4: this gain goes through the money pipeline
            // (the `effect` [反击] window, the modifier stages, `pay` / `payAfter`).
            // It used to be C# `fixedAmount` (bypassing every 支付阶段 window).
            ctx::gain_fixed(
                player_id,
                amount,
                &Msg::new(key!("tritone_why")).n("money", amount as i64),
            )?;
            // 规则书[反击]: 「此卡放置在场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(
                player_id,
                "Mor:迷茫之蝶们的三全音",
                &Msg::new(key!("tritone_note")).n("money", amount as i64),
            );
            ctx::set_crystals(3);
            // C# `Mem["owed"]` -- the amount this card gained, paid back on decay.
            ctx::set_slot(player_id, "tritone_owed", amount);
            ctx::log(
                player_id,
                &Msg::new(key!("tritone_placed"))
                    .player_id("who", player_id)
                    .n("money", amount as i64),
            );
        }
        // 规则书[反击]: 「三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡
        // 获得的资金」 -- C# `DecayCard.TurnEnd` -> `Empty` ->
        // `H.Charge(Seat, -1, Owed, "lose", ...)`.
        TriggerKind::TurnEnd => {
            // C# `DecayCard.TurnEnd` only ticks on its `DecayOn` player's turn end
            // (`turn != DecayOn || !H._placed.Contains(this)` -> return);
            // `DecayOn` defaults to `Player` = where the card is placed.
            if trigger::player_id() != player_id || !ctx::is_placed() {
                return Ok(());
            }
            if ctx::add_crystals(-1, 0)? > 0 {
                return Ok(());
            }
            let owed = ctx::slot(player_id, "tritone_owed");
            // C# `H.Unplace(this, "discard")` -- off the field, onto the discard.
            ctx::set_dest(ctx::Dest::Graveyard);
            if owed > 0 {
                ctx::pay(
                    player_id,
                    owed,
                    &Msg::new(key!("tritone_owed")).n("money", owed as i64),
                )?;
            }
            ctx::log(
                player_id,
                &Msg::new(key!("tritone_decayed"))
                    .player_id("who", player_id)
                    .n("money", owed as i64),
            );
        }
        _ => {}
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
