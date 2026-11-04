//! G12 status / blocked-ness / immunity models
//! (`docs/rulebook/spec/abi/G12-status-and-immunity.md` §3, §5.1, §5.4, §5.6,
//! §5.7, §9.2).
//!
//! Each function mirrors the production decision named beside it; the host
//! arms in `host.rs` call the same functions, so a proof here is a proof of
//! the arm's answer.

/// An [异常] stay layer (`AbKind` 1).
pub const STAY: i32 = 1;
/// An [异常] stun layer (`AbKind` 2).
pub const STUN: i32 = 2;
/// An [异常] exile layer (`AbKind` 3).
pub const EXILE: i32 = 3;
/// A teleport (`AbKind` 4).
pub const TELEPORT: i32 = 4;
/// A forced move (`AbKind` 5).
pub const FORCED: i32 = 5;
/// A stop (`AbKind` 6).
pub const STOP: i32 = 6;
/// A reverse (`AbKind` 7).
pub const REVERSE: i32 = 7;

/// The ruleset's `AbnormalGate` read (`rules/status/src/unstoppable.rs`).
///
/// `exemption = false` is the refusal read of §5.1: an [不可阻挡] mover
/// refuses a received movement kind (4-7, `by != actor`); everything else
/// lands.
/// `exemption = true` is the blocked-ness read of §5.6: the gate answers
/// *open* (not holding the move) for a teleport, and for the three layers
/// only while the mover is [不可阻挡]. Forced / stop / reverse always hold
/// when they land. `by` plays no part in the exemption read.
pub fn abnormal_gate_open(
    kind: i32,
    by: i32,
    actor: i32,
    unstoppable: bool,
    exemption: bool,
) -> bool {
    if exemption {
        // The complement of `value != 4 && (value >= 5 || unstoppable == 0)`.
        kind == TELEPORT || (kind <= TELEPORT && unstoppable)
    } else {
        !(unstoppable && kind >= TELEPORT && by != actor)
    }
}

/// §5.1: an [不可阻挡] mover refuses a landing iff the refusal read is closed.
pub fn abnormal_refused(kind: i32, by: i32, actor: i32, unstoppable: bool) -> bool {
    !abnormal_gate_open(kind, by, actor, unstoppable, false)
}

/// §3 / §5.6: blocked-ness is the mover's own exemption latch and the
/// ruleset's exemption read, and nothing else.
pub fn abnormal_blocked(
    kind: i32,
    by: i32,
    actor: i32,
    unstoppable: bool,
    move_exempt: bool,
) -> bool {
    !move_exempt && !abnormal_gate_open(kind, by, actor, unstoppable, true)
}

/// `cant_move_from` (`crates/game-rules/src/wasm_rules.rs` §5.4): one code,
/// checked in the order turn → skip → spent. A move the named activation
/// itself drove does not read as spent.
pub fn cant_move_code(
    turn: i32,
    player: i32,
    skip_move: bool,
    main_moved: bool,
    move_activation_ordinal: i32,
    activation_ordinal: i32,
) -> i32 {
    if turn != player {
        1
    } else if skip_move {
        3
    } else if main_moved && move_activation_ordinal != activation_ordinal {
        2
    } else {
        0
    }
}

/// `is_immune_from_at` (`crates/game-core/src/engine/ops.rs` §5.7): the
/// any-flavour flag, or the except-owner flavour for a causer other than the
/// instance's owner. The caller checks the instance exists.
pub fn is_immune_from(immune: bool, immune_except_owner: bool, owner: i32, by: i32) -> bool {
    immune || (immune_except_owner && by != owner)
}
