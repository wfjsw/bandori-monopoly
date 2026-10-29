//! `Mor:离心力，不为所动` -- C# `CardCentrifugal` (MatchHost.cs:5029-5078): second
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:离心力，不为所动`）:
//! > 离心力，不为所动：
//! > [反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果
//!
//! another player's card targeting between your turns cancels it and grants
//! immunity.
//!
//! Counteraction-only (`Normal => false`).

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, ChainKind, FilterKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Filter, GateValue, Msg, On};

const ID: &str = "Mor:离心力，不为所动";

pub const CENTRIFUGAL: CardDef = CardDef::new(
    "Mor:离心力，不为所动",
    &[
        On::Counteract(
            &[ChainKind::Effect],
            // 规则书: 「当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时」
            // `chain_has(Target) && target == owner && by >= 0 && by != owner` is
            // the `target` effect entry, aimed at the owner, from another
            // player's card. 「第二次」 -- C# `H._targeted[seat] >= 2` --
            // `targeted_count(owner) >= 2` (a derived-list counter).
            // TODO(规则书)（在两个你的回合之间（回合1结束，回合2开始））: the book counts
            //   designations between your turns -- `targeted_count` counts since your
            //   own turn start, so a targeting during your own turn counts too.
            // TODO(规则书)（第二次成为其他角色技能或卡牌的目标时）: the book says the second
            //   time -- `>= 2` keeps offering the card on every later targeting.
            // TODO(规则书)（第二次成为其他角色技能或卡牌的目标）: the book counts the
            //   second *targeting* -- a tile targeting opens the window but never
            //   advances `targeted_count`, so it is offered off an unrelated count;
            //   `tile_id < 0` would require a player target.
            "chain_has(Target) && target == owner && by >= 0 && by != owner && targeted_count(owner) >= 2",
            None,
            counteract).counter_reason(key!("centrifugal_when")),
        // The body-top `trigger::player_id() != player_id` early-out is
        // `actor == owner` (`pre::MINE`): the gate only claims immunity for
        // the seat it protects.
        // TODO(规则书)（无效化你受到的所有效果）: the book says all effects you
        //   receive -- `ImmuneAll` is only consulted for other players' effects, so
        //   your own effects still land through the shield.
        On::Filter(Filter::ImmuneAll(card_sdk::pre::MINE, None, immune_all)),
        // 规则书: 「直到下个你的回合开始时」
        On::Action(Action::TurnStart("actor == owner && card.placed", None, turn_start)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"离心力，不为所动"#)
        .raw_name(r#"离心力，不为所动"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果"#))
        // sheet cell G10
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果」
    // C# `c.Trigger.Cancelled = true` also voids the targeting that opened the window.
    trigger::set_cancelled(); // void the targeting that opened this window
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("centrifugal_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("centrifugal_log")).player_id("who", player_id),
    );
    // 规则书[反击]: 「无效化你受到的所有效果」 -- the ImmuneAll hook below covers
    //   targeting (`H.Target`), money transfers (the pay pipeline), and abnormal
    //   effects (the gate checks ImmuneAll first).
    Ok(())
}

/// `Fx.ImmuneAll` (C# `CardCentrifugal.ImmuneAll`): while placed, the owner is
/// untouchable by other players' effects. The engine asks ImmuneAll at
/// resolution -- after the designation was named and its chain formed, before
/// the effect lands -- for targets, abnormals, and card-driven payments, so this
/// one hook covers all three (C# `AnyFx(seat, f => f.ImmuneAll(seat))`). The
/// "only our own seat" check is the entry's condition (`pre::MINE`).
fn immune_all(_player_id: i32, mut g: GateValue) -> GateValue {
    // 规则书: 「无效化你受到的所有效果」
    g.allowed = Some(false); // deny
    g
}

/// `Fx.TurnStart` (C# `CardCentrifugal.TurnStart` -> `H.Unplace`): the immunity
/// ends at the owner's next turn start.
fn turn_start(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && card.placed` is the pre.
    // 规则书[反击]: 「直到下个你的回合开始时」 -- C# `H.Unplace(this, "discard",
    //   "效果结束了")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("centrifugal_end")).player_id("who", player_id),
    );
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
pub fn immune_all_kani(player_id: i32, v: card_sdk::GateValue) -> card_sdk::GateValue {
    immune_all(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}
