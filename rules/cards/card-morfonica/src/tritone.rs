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
        // 规则书[反击]: 「将要失去**或**支付资金时」 -- the clause is a
        // disjunction, and one guard cannot express it (`effect.has(Pay)` is
        // meaningless on a `spendBefore` link, where `pay_kind` is -1), so the
        // body hangs on two links: `Effect` for a payment, `SpendBefore` for a
        // raw loss.
        On::Counteract(
            &[ChainKind::Effect],
            // A 「[分摊]」 command total also raises `effect`, but 「此次失去的
            // 资金金额」 is the share this player actually loses (G02 §2.21):
            // exclude the command-window with `!pay_split_total`, so the answer
            // lands on the leg.
            "actor == owner && !pay_split_total && effect.has(Pay) && value > 0",
            None,
            counteract).counter_reason(key!("tritone_when")),
        On::Counteract(
            &[ChainKind::SpendBefore],
            "actor == owner && value > 0",
            None,
            counteract).counter_reason(key!("tritone_when")),
        On::Action(Action::TurnEnd("", None, counteract)),
    ],
)
// 规则书「奇迹水晶3」: the `⌛3` marker the client shows.
.props(&[(bandori_std::marker::TURNS_LEFT, 3)])
.meta(&card_sdk::meta::CardMeta::new()
        .name(r#"迷茫之蝶们的三全音"#)
        .raw_name(r#"迷茫之蝶们的三全音"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 任意时刻当你将要失去或支付资金时打出此卡，立刻获得此次失去的资金金额，此卡放置在场上，三回合后（奇迹水晶3，每回合结束时移除1）弃置此卡并支付由此卡获得的资金"#))
        // sheet cell G2
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

/// An on-card counter (this instance) -- a player slot would be shared by a
/// second copy.
const OWED: &str = "owed";

fn counteract(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书[反击]: 「将要失去…资金时」 -- a raw loss raises the `spendBefore`
        // window (no payment, so no `pay` entry); the figure is the loss.
        TriggerKind::SpendBefore => {
            let amount = trigger::value();
            let gained = ctx::gain_fixed(
                player_id,
                amount,
                &Msg::new(key!("tritone_reason")).n("money", amount as i64),
            )?;
            ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
            ctx::place_card(
                player_id,
                "Mor:迷茫之蝶们的三全音",
                &Msg::new(key!("tritone_note")).n("money", gained as i64),
            );
            ctx::set_crystals(3);
            ctx::set_counter(OWED, gained);
            ctx::log(
                player_id,
                &Msg::new(key!("tritone_placed"))
                    .player_id("who", player_id)
                    .n("money", gained as i64),
            );
        }
        // 规则书[反击]: 「任意时刻当你将要…支付资金时打出此卡」 -- the payment's
        // declaration raises the `effect` chain (kind `Effect`) with a `pay` entry,
        // so the counter answers that link.
        TriggerKind::Effect => {
            let amount = trigger::value();
            // 规则书[反击]: 「立刻获得此次失去的资金金额」 `NEGATION-AUDIT` V4: this gain goes through the money
            // pipeline (the `effect` [反击] window, the modifier stages, `pay` / `payAfter`).
            // The pipeline answers with what actually moved (a `paySplitAdd` modifier or a
            // negation reshapes it); 「由此卡获得的资金」 is that settled figure, not the declared
            // one.
            let gained = ctx::gain_fixed(
                player_id,
                amount,
                &Msg::new(key!("tritone_reason")).n("money", amount as i64),
            )?;
            ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Effect);
            ctx::place_card(
                player_id,
                "Mor:迷茫之蝶们的三全音",
                &Msg::new(key!("tritone_note")).n("money", gained as i64),
            );
            // 规则书: 「奇迹水晶3」
            ctx::set_crystals(3);
            // 规则书: 「并支付由此卡获得的资金」
            ctx::set_counter(OWED, gained);
            ctx::log(
                player_id,
                &Msg::new(key!("tritone_placed"))
                    .player_id("who", player_id)
                    .n("money", gained as i64),
            );
        }
        TriggerKind::TurnEnd => {
            // A card played on its owner's own turn spends its first crystal at that same
            // turn's end, then one per turn end after -- three turn ends of life in all.
            if !ctx::is_placed() {
                return Ok(());
            }
            // 「每回合结束时移除1」 -- the tick.
            ctx::dec_turns_left();
            if ctx::add_crystals(-1, 0)? > 0 {
                return Ok(());
            }
            let owed = ctx::counter(OWED);
            ctx::set_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
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
