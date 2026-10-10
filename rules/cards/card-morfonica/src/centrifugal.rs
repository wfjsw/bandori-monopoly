//! `Mor:离心力，不为所动` -- C# `CardCentrifugal` (MatchHost.cs:5029-5078): second
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:离心力，不为所动`）:
//! > 离心力，不为所动：
//! > [反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果
//!
//! hostile targeting between your turns cancels it and grants immunity.
//!
//! Counteraction-only (`Normal => false`).

use card_sdk::abi::{ChainKind, GateKind, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mor:离心力，不为所动";

pub const CENTRIFUGAL: CardDef = CardDef::new(
    "Mor:离心力，不为所动",
    &[
        On::Counteract(
            &[ChainKind::Effect],
            // `chain_has(Target) && target == owner && by >= 0 && by != owner` is
            // the `target` effect entry, aimed at the owner, from another
            // player's card. 「第二次」 -- C# `H._targeted[seat] >= 2` --
            // `targeted_count(owner) >= 2` (a derived-list counter).
            "chain_has(Target) && target == owner && by >= 0 && by != owner && targeted_count(owner) >= 2",
            None,
            counteract,
        ),
        // The body-top `trigger::player_id() != player_id` early-out is
        // `actor == owner` (`pre::MINE`): the gate only claims immunity for
        // the seat it protects.
        On::Gate(&[GateKind::ImmuneAll], card_sdk::pre::MINE, None, immune_all),
        On::Hook(&[HookKind::TurnStart], "actor == owner && card.placed", None, turn_start),
    ],
);

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
/// untouchable by other players' effects. The engine checks ImmuneAll before
/// targeting, before abnormals, and before card-driven payments, so this one
/// hook covers all three (C# `AnyFx(seat, f => f.ImmuneAll(seat))`). The
/// "only our own seat" check is the entry's condition (`pre::MINE`).
fn immune_all(player_id: i32) -> card_sdk::Asked {
    trigger::set_cancelled();
    ctx::log(
        player_id,
        &Msg::new(key!("centrifugal_blocked")).player_id("who", player_id),
    );
    Ok(())
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
