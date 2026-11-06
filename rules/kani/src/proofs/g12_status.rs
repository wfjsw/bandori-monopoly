//! G12 status, blocked-ness and immunity obligations
//! (`docs/rulebook/spec/abi/G12-status-and-immunity.md` §3, §5.1, §5.4, §5.6,
//! §5.7, §9.2).
//!
//! The exact predicates live in `spec::g12_status` and are the same functions
//! the host arms call. The bounds are the model's usual ones: two seats, one
//! field instance. `abnormal_gate_iff` is written from the two CEL condition
//! families in §5.1 and `cant_move_from_iff` from §5.4's order.

use card_sdk::kani_host::{Args, Op};

use crate::host;
use crate::kassert;
use crate::spec::g12_status::{
    abnormal_blocked, abnormal_gate_open, abnormal_refused, cant_move_code, is_immune_from, EXILE,
    FORCED, REVERSE, STAY, STOP, STUN, TELEPORT,
};
use crate::world;

/// The model bound: at most two seats.
const SEATS: i32 = 2;

fn any_kind() -> i32 {
    let kind: i32 = kani::any();
    kani::assume((STAY..=REVERSE).contains(&kind));
    kind
}

fn any_seat() -> i32 {
    let p: i32 = kani::any();
    kani::assume((0..SEATS).contains(&p));
    p
}

// ---------------------------------------------------------------------------
// §5.1 / §5.6 — refusal and blocked-ness
// ---------------------------------------------------------------------------

/// A landing is refused exactly where the refusal read is closed, and
/// blocked-ness is exactly the mover exemption and the exemption read.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn abnormal_gate_iff() {
    let kind = any_kind();
    let by = any_seat();
    let actor = any_seat();
    let unstoppable: bool = kani::any();
    let move_exempt: bool = kani::any();

    // Independent statement of §5.1's to fragments: an [不可阻挡] mover
    // refuses a *received* movement kind (teleport / forced / stop /
    // reverse); layers, a self-caused move and an unprotected mover pass.
    let want_refused = unstoppable && kind >= TELEPORT && by != actor;
    kassert!(
        abnormal_refused(kind, by, actor, unstoppable) == want_refused,
        "refused exactly a received movement kind under [不可阻挡]"
    );
    kassert!(
        abnormal_refused(kind, by, actor, unstoppable)
            == !abnormal_gate_open(kind, by, actor, unstoppable, false),
        "refused iff the refusal read is closed"
    );

    // §3 / §5.6: blocked iff the mover has no MOVE_EXEMPT and the ruleset
    // does not exempt the kind. Teleport is always replaced, never blocked;
    // the layers are exempt only under [不可阻挡]; forced / stop / reverse
    // always hold when they land.
    let want_blocked = !move_exempt && !(kind == TELEPORT || (kind <= EXILE && unstoppable));
    kassert!(
        abnormal_blocked(kind, by, actor, unstoppable, move_exempt) == want_blocked,
        "blocked is exactly the two exemption terms"
    );
    kassert!(
        abnormal_blocked(kind, by, actor, unstoppable, move_exempt)
            == (!move_exempt && !abnormal_gate_open(kind, by, actor, unstoppable, true)),
        "blocked iff MOVE_EXEMPT is absent and the exemption read is closed"
    );
    // `by` never enters the blocked-ness read.
    kassert!(
        abnormal_blocked(kind, by, actor, unstoppable, move_exempt)
            == abnormal_blocked(kind, by ^ 1, actor, unstoppable, move_exempt),
        "the exemption read is causer-blind"
    );

    kani::cover!(
        abnormal_refused(kind, by, actor, unstoppable),
        "a received movement kind"
    );
    kani::cover!(!abnormal_refused(kind, by, actor, unstoppable), "a landing");
    kani::cover!(
        kind == TELEPORT && !abnormal_blocked(kind, by, actor, unstoppable, move_exempt),
        "teleport is never blocked"
    );
    kani::cover!(
        abnormal_blocked(kind, by, actor, unstoppable, move_exempt),
        "blocked"
    );
    kani::cover!(
        move_exempt && !abnormal_blocked(kind, by, actor, unstoppable, move_exempt),
        "MOVE_EXEMPT frees the walk"
    );
    kani::cover!(
        abnormal_refused(kind, by, actor, unstoppable),
        "a refused landing still has no blocked-ness"
    );
}

/// MUTANT: the refusal ignores the causer — a self-caused teleport would be
/// refused too.
#[kani::proof]
#[kani::should_panic]
pub fn abnormal_gate_ignores_by_mutant() {
    let kind = any_kind();
    let by = any_seat();
    let actor = any_seat();
    let unstoppable: bool = kani::any();
    let move_exempt: bool = kani::any();
    kassert!(
        abnormal_refused(kind, by, actor, unstoppable) == (unstoppable && kind >= TELEPORT),
        "by dropped"
    );
    let _ = move_exempt;
}

/// MUTANT: blocked-ness drops `MOVE_EXEMPT`.
#[kani::proof]
#[kani::should_panic]
pub fn abnormal_blocked_drops_move_exempt_mutant() {
    let kind = any_kind();
    let by = any_seat();
    let actor = any_seat();
    let unstoppable: bool = kani::any();
    let move_exempt: bool = kani::any();
    kassert!(
        abnormal_blocked(kind, by, actor, unstoppable, move_exempt)
            == !(kind == TELEPORT || (kind <= EXILE && unstoppable)),
        "MOVE_EXEMPT dropped"
    );
}

// ---------------------------------------------------------------------------
// §5.4 — the cant-move code space
// ---------------------------------------------------------------------------

/// The code is `0` exactly where the move is free, and each other code
/// exactly where it says, in the turn → skip → spent order.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn cant_move_from_iff() {
    let turn = any_seat();
    let player = any_seat();
    let skip: bool = kani::any();
    let main_moved: bool = kani::any();
    let moved_by: i32 = kani::any();
    kani::assume((0..=2).contains(&moved_by));
    let asking: i32 = kani::any();
    kani::assume((0..=2).contains(&asking));

    let code = cant_move_code(turn, player, skip, main_moved, moved_by, asking);

    let free = turn == player && !skip && !(main_moved && moved_by != asking);
    kassert!((code == 0) == free, "0 exactly where the move is free");
    kassert!((code == 1) == (turn != player), "1 exactly off-turn");
    kassert!(
        (code == 2) == (turn == player && !skip && main_moved && moved_by != asking),
        "2 exactly spent by another activation"
    );
    kassert!(
        (code == 3) == (turn == player && skip),
        "3 exactly the skip latch"
    );
    kassert!((0..=3).contains(&code), "the code space is exactly 0..=3");
    // The order: a [停留] turn answers 3 even after the activation's own
    // teleport; an off-turn read answers 1 however the latches stand.
    kassert!(code != 3 || turn == player, "off-turn beats skip");
    kassert!(code != 2 || !skip, "skip beats spent");

    kani::cover!(code == 0, "free");
    kani::cover!(code == 1, "off-turn");
    kani::cover!(code == 2, "spent");
    kani::cover!(code == 3, "skip");
    kani::cover!(main_moved && moved_by == asking, "own move not spent");
}

/// MUTANT: "already moved" is never suppressed — an activation that drove the
/// move itself still reads spent.
#[kani::proof]
#[kani::should_panic]
pub fn cant_move_spent_not_suppressed_mutant() {
    fn mutant(turn: i32, player: i32, skip: bool, main_moved: bool) -> i32 {
        if turn != player {
            1
        } else if skip {
            3
        } else if main_moved {
            2
        } else {
            0
        }
    }
    let turn = any_seat();
    let player = any_seat();
    let skip: bool = kani::any();
    let main_moved: bool = kani::any();
    let ord: i32 = kani::any();
    kani::assume((0..=2).contains(&ord));
    kassert!(
        cant_move_code(turn, player, skip, main_moved, ord, ord)
            == mutant(turn, player, skip, main_moved),
        "an own move reads spent"
    );
}

/// MUTANT: the skip check comes before the turn check, so an off-turn
/// [停留] turn answers 3 instead of 1.
#[kani::proof]
#[kani::should_panic]
pub fn cant_move_skip_before_turn_mutant() {
    fn mutant(turn: i32, player: i32, skip: bool, main_moved: bool) -> i32 {
        if skip {
            3
        } else if turn != player {
            1
        } else if main_moved {
            2
        } else {
            0
        }
    }
    let turn = any_seat();
    let player = any_seat();
    let main_moved: bool = kani::any();
    kassert!(
        cant_move_code(turn, player, true, main_moved, 0, 0)
            == mutant(turn, player, true, main_moved),
        "turn check dropped"
    );
}

// ---------------------------------------------------------------------------
// §5.7 — the causer-aware immunity read
// ---------------------------------------------------------------------------

/// The truth table over the two flags and owner equality: the any-flavour
/// always protects, the except-owner flavour protects only a foreign causer.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
pub fn immune_from_at_iff() {
    let immune: bool = kani::any();
    let except_owner: bool = kani::any();
    let owner = any_seat();
    let by = any_seat();

    let got = is_immune_from(immune, except_owner, owner, by);
    let want = immune || (except_owner && by != owner);
    kassert!(got == want, "exactly the three-term predicate");

    kassert!(
        is_immune_from(true, except_owner, owner, by),
        "the any-flavour always protects"
    );
    kassert!(
        owner == by || !except_owner || got,
        "the owner's own effect still lands under the except-owner flavour"
    );
    kassert!(
        by != owner || !except_owner || immune || !got,
        "the except-owner flavour does not protect the owner"
    );

    kani::cover!(got, "immune");
    kani::cover!(immune, "any-flavour");
    kani::cover!(except_owner && by != owner, "foreign causer");
    kani::cover!(except_owner && by == owner && !immune, "owner's own effect");
    kani::cover!(!immune && !except_owner, "no immunity");
}

/// MUTANT: the except-owner flavour protects the owner too (the pre-G12
/// cause-blind collapse).
#[kani::proof]
#[kani::should_panic]
pub fn immune_from_at_owner_blind_mutant() {
    let except_owner: bool = kani::any();
    let owner = any_seat();
    let by = any_seat();
    kassert!(
        is_immune_from(false, except_owner, owner, by) == except_owner,
        "owner's own effect skipped"
    );
}

// ---------------------------------------------------------------------------
// the host arms are wired to the model
// ---------------------------------------------------------------------------

// Each wiring harness installs only the arms it calls (`host_subset!`), so
// CBMC never builds the ~300-op `dispatch` match.

crate::host_subset!(g12_dispatch {
    trigger_activation_ordinal => arm_trigger_activation_ordinal,
    trigger_abnormal_blocked => arm_trigger_abnormal_blocked,
    trigger_move_abnormal => arm_trigger_move_abnormal,
    cant_move_code_for_activation => arm_cant_move_code_for_activation,
    cant_move => arm_cant_move,
    skip_main_move => arm_skip_main_move,
    clear_move_skip => arm_clear_move_skip,
    run_planned_move => arm_card_move,
    is_immune_from_at => arm_is_immune_from_at,
    set_self_immune_except_owner => arm_set_self_immune_except_owner,
    dice_immune => arm_dice_immune,
    set_dice_immune => arm_set_dice_immune,
});

fn call(op: Op, a: Args) -> i64 {
    g12_dispatch(op, a)
}

/// The three trigger readers answer the fields, the frame's bit OR the turn
/// latch, and nothing else.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn host_arms_trigger_readers() {
    host::install_backend(g12_dispatch);
    let ord: i32 = kani::any();
    kani::assume((0..=2).contains(&ord));
    let blocked: bool = kani::any();

    world::with_world_mut(|w| {
        w.n_players = SEATS;
        w.turn_activation_ordinal = ord;
        w.trig_abnormal_blocked = blocked;
    });

    kassert!(
        call(Op::trigger_activation_ordinal, [0; 6]) == ord as i64,
        "activation ordinal is the stamp"
    );
    kassert!(
        call(Op::trigger_abnormal_blocked, [0; 6]) == blocked as i64,
        "blocked flag is the trigger's"
    );
    kassert!(
        call(Op::trigger_move_abnormal, [0; 6]) == 0,
        "no frame and no latch is false"
    );
    world::with_world_mut(|w| {
        w.move_snap = crate::world::MoveSnap {
            abnormal: true,
            ..crate::world::MoveSnap::NONE
        }
    });
    kassert!(
        call(Op::trigger_move_abnormal, [0; 6]) == 1,
        "the frame's span bit reads"
    );
    world::with_world_mut(|w| {
        w.move_snap = crate::world::MoveSnap::NONE;
        w.turn_move_abnormal = true;
    });
    kassert!(
        call(Op::trigger_move_abnormal, [0; 6]) == 1,
        "the turn latch reads"
    );

    kani::cover!(blocked, "blocked stamp");
    kani::cover!(ord > 0, "a running activation");
}

/// The ordinal form suppresses only the asking activation, the skip answers
/// `3` over the spent read, and the plain read composes on the running stamp.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn host_arms_cant_move() {
    host::install_backend(g12_dispatch);
    let ord: i32 = kani::any();
    kani::assume((0..=2).contains(&ord));
    world::with_world_mut(|w| {
        w.n_players = SEATS;
        w.turn_player = 0;
        w.main_moved = true;
        w.move_activation_ordinal = ord + 1;
        w.turn_activation_ordinal = ord;
    });

    kassert!(
        call(Op::cant_move_code_for_activation, [0, ord, 0, 0, 0, 0]) == 2,
        "another activation is spent"
    );
    kassert!(
        call(Op::cant_move, [0; 6]) == 2,
        "the plain read composes on the running stamp"
    );
    world::with_world_mut(|w| w.move_activation_ordinal = ord);
    kassert!(
        call(Op::cant_move_code_for_activation, [0, ord, 0, 0, 0, 0]) == 0,
        "the asking activation is not spent"
    );
    kassert!(call(Op::cant_move, [0; 6]) == 0, "plain read agrees");
    call(Op::skip_main_move, [0, 1, 0, 0, 0, 0]);
    kassert!(
        call(Op::cant_move_code_for_activation, [0, ord, 0, 0, 0, 0]) == 3,
        "the skip latch answers 3 even after its own move"
    );
    call(Op::clear_move_skip, [0, 0, 0, 0, 0, 0]);
    kassert!(call(Op::cant_move, [1; 6]) == 1, "off-turn answers 1");
}

/// Taking the main move latches it to the running activation (`play.rs`), so
/// its own routine re-reads the move as not spent while a routine that starts
/// afterwards is refused.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn run_planned_move_latches_the_activation() {
    host::install_backend(g12_dispatch);
    world::with_world_mut(|w| {
        w.n_players = SEATS;
        w.turn_player = 0;
    });
    world::begin_activation();
    let ord = world::with_world(|w| w.turn_activation_ordinal);
    kassert!(
        call(Op::run_planned_move, [0; 6]) == 1,
        "a fresh main move runs"
    );
    kassert!(
        world::with_world(|w| w.move_activation_ordinal) == ord,
        "the move is latched to the activation that spent it"
    );
    kassert!(
        call(Op::cant_move, [0; 6]) == 0,
        "the asking activation is not spent"
    );
    world::begin_activation();
    kassert!(call(Op::cant_move, [0; 6]) == 2, "a later activation is");
    kassert!(
        call(Op::run_planned_move, [0; 6]) == 0,
        "and its move is refused"
    );
}

/// The immunity arm reads the three-term predicate, and the setter writes the
/// running instance and answers whether it exists.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(10)]
pub fn host_arms_immunity() {
    host::install_backend(g12_dispatch);
    let uid: i32 = 7;
    let immune: bool = kani::any();
    let except_owner: bool = kani::any();
    let owner = any_seat();
    let by = any_seat();
    let on: bool = kani::any();
    world::with_world_mut(|w| {
        w.n_players = SEATS;
        w.field[0] = world::FieldCard {
            uid,
            card: 0,
            player: owner,
            owner,
            tile: 0,
            face_down: false,
            immune,
            immune_except_owner: except_owner,
            crystals: 0,
            props: world::NameMap::new(),
            effective_card: u16::MAX,
            on: -1,
            created_by: -1,
            effective: true,
            mine: false,
        };
        w.field_n = 1;
    });

    kassert!(
        call(Op::is_immune_from_at, [uid, by, 0, 0, 0, 0])
            == is_immune_from(immune, except_owner, owner, by) as i64,
        "the immunity read is the three-term predicate"
    );
    kassert!(
        call(Op::is_immune_from_at, [uid + 100, by, 0, 0, 0, 0]) == 0,
        "no instance is false"
    );
    kassert!(
        call(Op::set_self_immune_except_owner, [on as i32, 0, 0, 0, 0, 0]) == 1,
        "the setter answers the instance exists"
    );
    kassert!(
        world::with_world(|w| w.field[0].immune_except_owner) == on,
        "the running instance is written"
    );
    world::with_world_mut(|w| w.field_n = 0);
    kassert!(
        call(Op::set_self_immune_except_owner, [1, 0, 0, 0, 0, 0]) == 0,
        "no running instance is false"
    );

    kani::cover!(immune || (except_owner && by != owner), "immune");
    kani::cover!(except_owner && by == owner && !immune, "owner's own effect");
}

/// The dice arm reads the turn flag and answers the previous value.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn host_arms_dice_immunity() {
    host::install_backend(g12_dispatch);
    world::with_world_mut(|w| w.n_players = SEATS);

    kassert!(call(Op::dice_immune, [0; 6]) == 0, "starts down");
    kassert!(
        call(Op::set_dice_immune, [1, 0, 0, 0, 0, 0]) == 0,
        "the setter answers the previous value"
    );
    kassert!(call(Op::dice_immune, [0; 6]) == 1, "the flag is up");
    kassert!(
        call(Op::set_dice_immune, [0, 0, 0, 0, 0, 0]) == 1,
        "the previous value is answered"
    );
    kassert!(call(Op::dice_immune, [0; 6]) == 0, "the flag is down");
}
