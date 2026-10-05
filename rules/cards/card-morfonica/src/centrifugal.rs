//! `Mor:离心力，不为所动` -- C# `CardCentrifugal` (MatchHost.cs:5029-5078): second
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:离心力，不为所动`）:
//! > 离心力，不为所动：
//! > [反击]当你在两个你的回合之间（回合1结束，回合2开始）第二次成为其他角色技能或卡牌的目标时，你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果
//!
//! hostile targeting between your turns cancels it and grants immunity.
//!
//! Reaction-only (`Normal => false`).

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const CENTRIFUGAL: CardDef = CardDef {
    id: "Mor:离心力，不为所动",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「第二次成为其他角色技能或卡牌的目标时」
    // C# `t.Kind == "target" && t.Target == seat && t.ByCard >= 0 && t.ByCard != seat`
    //   and `H._targeted[seat] >= 2` (the counter is bumped on every target raise
    //   between the seat's turns, `MatchHost.cs:19138` / reset at `:25775`).
    if trigger::kind() != TriggerKind::Target {
        return false;
    }
    // `t.Target` is the one being aimed at (C# `t.Target == seat`).
    if trigger::target() != seat {
        return false;
    }
    // TODO(ABI): `t.ByCard` (the targeting came from another player's card / skill)
    //   is not on the trigger payload, and the between-turns target counter
    //   (`H._targeted`) has no raise/reset hook -- so the 「第二次」 / 「其他角色」
    //   gates cannot be checked. This window opens on every targeting of the seat
    //   (over-permissive).
    true
}

fn react(seat: i32) {
    // 规则书[反击]: 「你可以打出此卡，直到下个你的回合开始时，无效化你受到的所有效果」
    // C# `c.Trigger.Cancelled = true` also voids the targeting that opened the window.
    // TODO(ABI): `Trigger.Cancelled` is not writable through the ABI.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mor:离心力，不为所动", &Msg::new(key!("centrifugal_note")));
    ctx::log(seat, &Msg::new(key!("centrifugal_log")).seat("who", seat));
    // TODO(规则书)[反击]: 「无效化你受到的所有效果」 until the owner's next turn
    // start -- needs the Fx.ImmuneAll hook (C# `CardCentrifugal.ImmuneAll`) and
    // Fx.TurnStart to unplace (C# `CardCentrifugal.TurnStart` -> `H.Unplace`).
}