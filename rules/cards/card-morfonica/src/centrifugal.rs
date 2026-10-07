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
        On::Counteract(&[ChainKind::Effect], can_counteract, counteract),
        On::Gate(&[GateKind::ImmuneAll], immune_all),
        On::Hook(&[HookKind::TurnStart], |_| true, turn_start),
    ],
);

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「第二次成为其他角色技能或卡牌的目标时」
    // C# `t.Kind == "target" && t.Target == seat && t.ByCard >= 0 && t.ByCard != seat`
    //   and `H._targeted[player_id] >= 2` (the counter is bumped on every target raise
    //   between the player's turns, `MatchHost.cs:19138` / reset at `:25775`).
    if trigger::kind() != ChainKind::Effect {
        return false;
    }
    // 规则书[反击]: 「成为其他角色技能或卡牌的目标」 -- the `target` effect entry
    // (the designation), not just any effect aimed at the player. The money
    // pipeline declares a `pay` effect on every [支付]/[获得] and must not open
    // this window.
    if !ctx::effect::has(TriggerKind::Target) {
        return false;
    }
    // `t.Target` is the one being aimed at (C# `t.Target == seat`).
    if trigger::target() != player_id {
        return false;
    }
    // 「其他角色」 -- the targeting must come from another player's card.
    if !trigger::by_card().is_some_and(|by| by != player_id) {
        return false;
    }
    // 规则书[反击]: 「第二次」 -- C# `H._targeted[seat] >= 2`; the engine bumps
    //   the counter on every `H.Target` of this player (before the [反击] window).
    ctx::targeted_count(player_id) >= 2
}

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
/// hook covers all three (C# `AnyFx(seat, f => f.ImmuneAll(seat))`).
fn immune_all(player_id: i32) -> card_sdk::Asked {
    // The hook runs on every placed card across all players; only claim
    // immunity for our own seat (`t.player` = the protected seat).
    if trigger::player_id() != player_id {
        return Ok(());
    }
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
    if trigger::player_id() != player_id || !ctx::is_placed() {
        return Ok(());
    }
    // 规则书[反击]: 「直到下个你的回合开始时」 -- C# `H.Unplace(this, "discard",
    //   "效果结束了")`.
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("centrifugal_end")).player_id("who", player_id),
    );
    Ok(())
}
