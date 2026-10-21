//! Kind constants as they are written inside conditions.
//!
//! **Ownership rule.** This crate authors **neither** kind table. Engine
//! kinds live in `crates/game-kinds` (owned by `crates/`); ruleset kinds live
//! in `bandori-std::kind`. Neither side inspects the other's ids. The
//! constants here **mirror** those owning tables as the canonical wire
//! **spellings** (`TriggerKind::spelling`) -- they are strings, never ids.
//!
//! CEL `kind` is `VarTy::Str`: a bare ident binds the spelling, and a string
//! literal is the same spelling. Constants below are `&str` spellings
//! (`"turnStart"`, `"r:bandori:21"`). There is no int encoding of kinds at
//! all.
//!
//! The IR bakes every spelling to `id_of(spelling)` (see `VarTy::Str`), so the
//! CEL string and the IR hash are the same identity. `move.kind` stays an
//! `int` -- [`mv`] is a closed engine enum with no namespace problem.
//!
//! CEL spells kinds as bare identifiers: `kind == Exile`,
//! `effect.has(Pay)`. String literals also work: `kind == "r:bandori:21"` /
//! `kind == "turnStart"`. They are bound as string constants at compile time
//! (unknown-var check) and at eval time (the window scope).
//!
//! `Teleport` is a [`mv`] value only (the old TriggerKind 17 was retired in
//! ABI v84). Write `move.kind == Teleport`.

/// Trigger / chain / hook kind spellings (the wire identity).
#[allow(non_upper_case_globals)]
pub mod trig {
    pub const DiceRollBefore: &str = "diceRollBefore";
    pub const DiceRollValue: &str = "diceRollValue";
    pub const DiceRollResult: &str = "diceRollResult";
    pub const TurnStart: &str = "turnStart";
    pub const Pass: &str = "pass";
    pub const PassPlayer: &str = "passPlayer";
    pub const SettleBefore: &str = "settleBefore";
    pub const Settle: &str = "settle";
    pub const Mortgage: &str = "mortgage";
    pub const Pay: &str = "pay";
    pub const Paid: &str = "paid";
    pub const Bankrupt: &str = "bankrupt";
    pub const Card: &str = "card";
    pub const Event: &str = "event";
    pub const Abnormal: &str = "abnormal";
    pub const Target: &str = "target";
    // 16 / 17 / 19 / 20 / 22 retired in ABI v84 (Stop / Teleport / Stun /
    // Stay / Forced); 18 (SkillTeleport) retired in ABI v85 -- never reuse.
    /// Ruleset namespace (bandori-std::kind::Exile).
    pub const Exile: &str = "r:bandori:21";
    pub const State: &str = "state";
    pub const Counteracted: &str = "counteracted";
    pub const DrawOut: &str = "drawOut";
    /// Ruleset namespace (bandori-std::kind::PassRewardAffected).
    pub const PassRewardAffected: &str = "r:bandori:26";
    pub const TwoCards: &str = "twoCards";
    pub const TurnStartBefore: &str = "turnStartBefore";
    pub const PassBefore: &str = "passBefore";
    pub const MortgageBefore: &str = "mortgageBefore";
    pub const BankruptBefore: &str = "bankruptBefore";
    pub const CardAfter: &str = "cardAfter";
    pub const EventAfter: &str = "eventAfter";
    pub const SettleAfter: &str = "settleAfter";
    pub const BuyBefore: &str = "buyBefore";
    pub const BuyAfter: &str = "buyAfter";
    pub const BuildBefore: &str = "buildBefore";
    pub const BuildAfter: &str = "buildAfter";
    pub const DiscardBefore: &str = "discardBefore";
    pub const DiscardAfter: &str = "discardAfter";
    pub const EndTurnBefore: &str = "endTurnBefore";
    pub const EndTurnAfter: &str = "endTurnAfter";
    pub const LeaveBefore: &str = "leaveBefore";
    pub const LeaveAfter: &str = "leaveAfter";
    /// ChainKind::SettleBody (the settle body is its own chain link).
    pub const SettleBody: &str = "settleBody";
    /// ChainKind::Effect -- 「被…效果影响」.
    pub const Effect: &str = "effect";
    /// Engine namespace (`ActionKind::MarkerSpent`). Any named-counter spend
    /// commits.
    pub const MarkerSpent: &str = "markerSpent";
    /// Engine namespace (`ActionKind::CardUsed`). A field card's `On::Play`
    /// runs.
    pub const CardUsed: &str = "cardUsed";
    /// Engine namespace (`ActionKind::HouseAdded`). A deed's house count is
    /// written.
    pub const HouseAdded: &str = "houseAdded";
    /// v43: the move head / move-tail pair (`SETTLE-STAGES.md` §7).
    pub const MoveBefore: &str = "moveBefore";
    pub const MoveAfter: &str = "moveAfter";
    /// 「将X个反面[P㏄P粉丝]变正」 (bandori-std::kind::FansFlip).
    /// Ruleset namespace.
    pub const FansFlip: &str = "r:bandori:107";
}

/// MoveKind values. `Walk` / `Teleport` are what `move.kind ==` compares.
/// These stay `int` constants -- `move.kind` is a closed engine enum.
#[allow(non_upper_case_globals)]
pub mod mv {
    pub const Walk: i64 = 0;
    pub const Teleport: i64 = 1;
}

// TELEPORT_TRIGGER (TriggerKind 17) retired in ABI v84 -- never reuse.

/// (CEL name, spelling) for every **string** constant a condition may
/// reference -- the trigger / chain / hook kinds.
///
/// This is half the unknown-var allow-list for bare identifiers that are not
/// context variables, and half the table the eval scope binds (as CEL
/// strings). See [`int_constants`] for the move-kind ints.
pub fn str_constants() -> &'static [(&'static str, &'static str)] {
    &[
        ("DiceRollBefore", trig::DiceRollBefore),
        ("DiceRollResult", trig::DiceRollResult),
        ("DiceRollValue", trig::DiceRollValue),
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
        ("Exile", trig::Exile),
        ("State", trig::State),
        ("Counteracted", trig::Counteracted),
        ("DrawOut", trig::DrawOut),
        ("PassRewardAffected", trig::PassRewardAffected),
        // ABI v69 alias for already-shipped condition strings.
        ("CircleAffected", trig::PassRewardAffected),
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
        ("MarkerSpent", trig::MarkerSpent),
        ("CardUsed", trig::CardUsed),
        ("HouseAdded", trig::HouseAdded),
        ("MoveBefore", trig::MoveBefore),
        ("MoveAfter", trig::MoveAfter),
        ("FansFlip", trig::FansFlip),
    ]
}

/// (CEL name, value) for every **int** constant a condition may reference.
/// The move-kind pair; `move.kind == Walk` / `move.kind == Teleport`.
pub fn int_constants() -> &'static [(&'static str, i64)] {
    &[
        // `Teleport` is MoveKind (1) only -- TriggerKind 17 retired in v84.
        ("Teleport", mv::Teleport),
        ("Walk", mv::Walk),
    ]
}

/// Every bare identifier a condition may reference that is not a context
/// variable (kind spellings plus the move-kind ints).
pub fn ident_known(name: &str) -> bool {
    str_constants().iter().any(|(n, _)| *n == name)
        || int_constants().iter().any(|(n, _)| *n == name)
}