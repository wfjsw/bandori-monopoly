//! Kind constants as they are written inside conditions.
//!
//! The values mirror the host wire enums (`card-sdk::abi::{TriggerKind,
//! ChainKind, HookKind, MoveKind}`) so a host can pass its raw `i32` through
//! without a translation table. This crate stays free of `card-sdk` /
//! `game-core` types on purpose (G1 is an isolated module); when the host
//! wires it up (G2) it fills these `i64`s from the same discriminants.
//!
//! CEL spells them as bare identifiers: `move.kind == Walk`,
//! `effect.has(Pay)`. They are bound as int constants at compile time
//! (unknown-var check) and at eval time (the window scope).
//!
//! # Name collision: `Teleport`
//!
//! `Teleport` is both a [`TriggerKind`] (17) and a [`MoveKind`] (1). In the
//! CEL namespace `Walk` / `Teleport` are the **MoveKind** values (0 / 1) --
//! `move.kind == Teleport` is the common spelling. The trigger kind 17 is
//! exposed as [`TELEPORT_TRIGGER`]; write `effect.has(TeleportTrigger)`.

/// Trigger / chain / hook kind values (shared wire numbering).
#[allow(non_upper_case_globals)]
pub mod trig {
    pub const Roll: i64 = 1;
    pub const MoveRoll: i64 = 2;
    pub const TurnStart: i64 = 3;
    pub const Pass: i64 = 4;
    pub const PassPlayer: i64 = 5;
    pub const SettleBefore: i64 = 6;
    pub const Settle: i64 = 7;
    pub const Mortgage: i64 = 8;
    pub const Pay: i64 = 9;
    pub const Paid: i64 = 10;
    pub const Bankrupt: i64 = 11;
    pub const Card: i64 = 12;
    pub const Event: i64 = 13;
    pub const Abnormal: i64 = 14;
    pub const Target: i64 = 15;
    pub const Stop: i64 = 16;
    pub const Teleport: i64 = 17;
    pub const SkillTeleport: i64 = 18;
    pub const Stun: i64 = 19;
    pub const Stay: i64 = 20;
    pub const Exile: i64 = 21;
    pub const Forced: i64 = 22;
    pub const State: i64 = 23;
    pub const Counteracted: i64 = 24;
    pub const DrawOut: i64 = 25;
    pub const CircleAffected: i64 = 26;
    pub const TwoCards: i64 = 27;
    pub const TurnStartBefore: i64 = 28;
    pub const PassBefore: i64 = 29;
    pub const MortgageBefore: i64 = 30;
    pub const BankruptBefore: i64 = 31;
    pub const CardAfter: i64 = 32;
    pub const EventAfter: i64 = 33;
    pub const SettleAfter: i64 = 34;
    pub const BuyBefore: i64 = 35;
    pub const BuyAfter: i64 = 36;
    pub const BuildBefore: i64 = 37;
    pub const BuildAfter: i64 = 38;
    pub const DiscardBefore: i64 = 39;
    pub const DiscardAfter: i64 = 40;
    pub const EndTurnBefore: i64 = 41;
    pub const EndTurnAfter: i64 = 42;
    pub const LeaveBefore: i64 = 43;
    pub const LeaveAfter: i64 = 44;
    /// ChainKind::SettleBody (the settle body is its own chain link).
    pub const SettleBody: i64 = 64;
    /// ChainKind::Effect -- 「被…效果影响」.
    pub const Effect: i64 = 72;
    pub const FireSpent: i64 = 73;
    pub const SkillUsed: i64 = 74;
    pub const HouseAdded: i64 = 75;
    /// v43: the move head / move-tail pair (`SETTLE-STAGES.md` §7).
    pub const MoveBefore: i64 = 90;
    pub const MoveAfter: i64 = 91;
}

/// MoveKind values. `Walk` / `Teleport` are what `move.kind ==` compares.
#[allow(non_upper_case_globals)]
pub mod mv {
    pub const Walk: i64 = 0;
    pub const Teleport: i64 = 1;
}

/// TriggerKind::Teleport (17), under a name that cannot collide with
/// [`mv::Teleport`].
pub const TELEPORT_TRIGGER: i64 = trig::Teleport;

/// (CEL name, value) for every int constant a condition may reference.
///
/// This is the unknown-var allow-list for bare identifiers that are not
/// context variables, and the table the eval scope binds.
pub fn constants() -> &'static [(&'static str, i64)] {
    &[
        ("Roll", trig::Roll),
        ("MoveRoll", trig::MoveRoll),
        ("TurnStart", trig::TurnStart),
        ("Pass", trig::Pass),
        ("PassPlayer", trig::PassPlayer),
        ("SettleBefore", trig::SettleBefore),
        ("Settle", trig::Settle),
        ("Mortgage", trig::Mortgage),
        ("Pay", trig::Pay),
        ("Paid", trig::Paid),
        ("Bankrupt", trig::Bankrupt),
        ("Card", trig::Card),
        ("Event", trig::Event),
        ("Abnormal", trig::Abnormal),
        ("Target", trig::Target),
        ("Stop", trig::Stop),
        // `Teleport` is MoveKind (1) -- see the module docs. The trigger kind
        // 17 is `TeleportTrigger`.
        ("Teleport", mv::Teleport),
        ("Walk", mv::Walk),
        ("TeleportTrigger", TELEPORT_TRIGGER),
        ("SkillTeleport", trig::SkillTeleport),
        ("Stun", trig::Stun),
        ("Stay", trig::Stay),
        ("Exile", trig::Exile),
        ("Forced", trig::Forced),
        ("State", trig::State),
        ("Counteracted", trig::Counteracted),
        ("DrawOut", trig::DrawOut),
        ("CircleAffected", trig::CircleAffected),
        ("TwoCards", trig::TwoCards),
        ("TurnStartBefore", trig::TurnStartBefore),
        ("PassBefore", trig::PassBefore),
        ("MortgageBefore", trig::MortgageBefore),
        ("BankruptBefore", trig::BankruptBefore),
        ("CardAfter", trig::CardAfter),
        ("EventAfter", trig::EventAfter),
        ("SettleAfter", trig::SettleAfter),
        ("BuyBefore", trig::BuyBefore),
        ("BuyAfter", trig::BuyAfter),
        ("BuildBefore", trig::BuildBefore),
        ("BuildAfter", trig::BuildAfter),
        ("DiscardBefore", trig::DiscardBefore),
        ("DiscardAfter", trig::DiscardAfter),
        ("EndTurnBefore", trig::EndTurnBefore),
        ("EndTurnAfter", trig::EndTurnAfter),
        ("LeaveBefore", trig::LeaveBefore),
        ("LeaveAfter", trig::LeaveAfter),
        ("SettleBody", trig::SettleBody),
        ("Effect", trig::Effect),
        ("FireSpent", trig::FireSpent),
        ("SkillUsed", trig::SkillUsed),
        ("HouseAdded", trig::HouseAdded),
        ("MoveBefore", trig::MoveBefore),
        ("MoveAfter", trig::MoveAfter),
    ]
}