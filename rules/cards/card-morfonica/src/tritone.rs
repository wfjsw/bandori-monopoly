//! `Mor:迷茫之蝶们的三全音` -- C# `CardTritone` (MatchHost.cs:4524-4583): loan the
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:迷茫之蝶们的三全音`）:
//! > 迷茫之蝶们的三全音：
//! > [反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金
//!
//! pay amount back in three turns -- one crystal off at **every** turn end
//! (any seat's), discard and payback after the third.
//!
//! Counteraction-only (`Normal => false`): the player is about to lose or pay money.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const TRITONE: CardDef = CardDef::new(
    "Mor:迷茫之蝶们的三全音",
    &[
        On::Counteract(
            &[ChainKind::Effect],
            // TODO(规则书)（任意时刻当你将要失去或支付资金时打出此卡）: the book opens
            // the window on 「失去」 too -- a raw loss (`ctx::lose_raw` /
            // `adjust_money`) raises no `effect` chain, so only 「支付」 answers.
            "actor == owner && effect.has(Pay) && value > 0",
            None,
            counteract).counter_reason(key!("tritone_when")),
        On::Action(Action::TurnEnd("", None, counteract)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"迷茫之蝶们的三全音"#)
        .raw_name(r#"迷茫之蝶们的三全音"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金"#))
        // sheet cell G2
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

/// C# `Mem["owed"]`: the amount this card gained, paid back on decay. An
/// on-card counter (this instance) -- a player slot would be shared by a
/// second copy.
const OWED: &str = "owed";

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
            // The pipeline answers with what actually moved (a `paySplitAdd`
            // modifier or a negation reshapes it); 「由此卡获得的资金」 is that
            // settled figure, not the declared one.
            let gained = ctx::gain_fixed(
                player_id,
                amount,
                &Msg::new(key!("tritone_why")).n("money", amount as i64),
            )?;
            // 规则书[反击]: 「此卡放置在场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 3)`.
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(
                player_id,
                "Mor:迷茫之蝶们的三全音",
                &Msg::new(key!("tritone_note")).n("money", gained as i64),
            );
            // 规则书: 「奇迹水晶3」
            ctx::set_crystals(3);
            // C# `Mem["owed"]`: what this card actually gained, paid back on
            // decay -- per instance, so a second copy keeps its own figure.
            // 规则书: 「并支付由此卡获得的资金」
            ctx::set_counter(OWED, gained);
            ctx::log(
                player_id,
                &Msg::new(key!("tritone_placed"))
                    .player_id("who", player_id)
                    .n("money", gained as i64),
            );
        }
        // 规则书[反击]: 「三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡
        // 获得的资金」 -- C# `DecayCard.TurnEnd` -> `Empty` ->
        // `H.Charge(Seat, -1, Owed, "lose", ...)`.
        TriggerKind::TurnEnd => {
            // 规则书: 「每回合结束时移除1」 -- 「每回合」 is every turn end, not only
            // the owner's (C# `DecayCard.DecayOn` keys it to the owner; the
            // book does not). A card played on its owner's own turn spends its
            // first crystal at that same turn's end, then one per turn end
            // after -- three turn ends of life in all.
            if !ctx::is_placed() {
                return Ok(());
            }
            if ctx::add_crystals(-1, 0)? > 0 {
                return Ok(());
            }
            let owed = ctx::counter(OWED);
            // C# `H.Unplace(this, "discard")` -- off the field, onto the discard.
            ctx::set_dest(ctx::Dest::Graveyard);
            if owed > 0 {
                // TODO(规则书)（弃置此卡并支付由此卡获得的资金）: the book names no
                // payee -- we pay the bank (OPEN-三全音收款人).
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
