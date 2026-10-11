//! Unit `crychic`: clause proofs.
//!
//! Per clause (KANI-FOUNDATION.md §3): iff + no-op / cover + mutant.
//! Specs are `crate::spec::crychic` — book only. Bodies are invoked unmodified
//! via `crate::invoke`.
//!
//! Isolation assumes are listed next to each `kani::assume`.

use rules_cond_ir::ctx::{IrCtx, SetId};
use rules_cond_ir::vars::VarId;

use crate::host;
use crate::intern;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn effects() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn band_crystals_of(p: i32) -> i32 {
    world::with_world(|w| w.band_crystals[p as usize])
}

fn field_n() -> usize {
    world::with_world(|w| w.field_n as usize)
}

fn key_get(p: i32, key: intern::NameId) -> i32 {
    world::with_world(|w| w.key_get(p, key))
}

/// Light symbolic world: 2..=4 players, bounded money, empty piles.
fn symbolic_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = false;
    }
    w.symbolic_answers = true;
    w
}

fn any_seat(n: i32) -> i32 {
    let p: i32 = kani::any();
    kani::assume(p >= 0 && p < n);
    p
}

// ===========================================================================
// C-CRYCHIC-soyoreturn-02 — 「（2）[手] 获得500资金」
// ===========================================================================

/// IFF: play body gains exactly 500 for the player; frame otherwise empty.
#[kani::proof]
#[kani::unwind(16)]
pub fn soyoreturn_02_hand_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play("CRYCHIC:（soyo）回到曾经", player));

    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    assert!(
        spec::crychic::soyoreturn_02_log_ok(effects, player),
        "C-CRYCHIC-soyoreturn-02: exactly GainMoney{player, 500}"
    );
    assert!(
        post.money[player as usize] == pre.money[player as usize] + 500,
        "C-CRYCHIC-soyoreturn-02: money +500"
    );
    // frame
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.stay == pre.stay, "frame: stay");
    assert!(post.fire == pre.fire, "frame: fire");
    assert!(post.hand == pre.hand, "frame: hand bitset");
    assert!(post.pay_amount == pre.pay_amount, "frame: pay_amount");
}

/// MUTANT companion: the 501-spec is distinguishable from the body.
#[kani::proof]
#[kani::unwind(16)]
pub fn soyoreturn_02_mutant_is_detected() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("CRYCHIC:（soyo）回到曾经", player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    assert!(
        spec::crychic::soyoreturn_02_log_ok(effects, player),
        "card matches the book"
    );
    let wrong = spec::crychic::soyoreturn_02_log_ok_mutant(effects, player);
    kani::cover!(!wrong, "off-by-one mutant disagrees");
    assert!(!wrong, "mutant 501 must NOT match the body");
}

// ===========================================================================
// C-CRYCHIC-vocalist-01 — leaf body of the [反击] waive
// ===========================================================================

/// Light fill for `actor == owner && effect.has(Pay) && target >= 0 && value >= 5000`.
fn fill_vocalist(w: &World, owner: i32) -> IrCtx {
    let mut c = ir_fill::fill_pay_add(w, owner);
    // `effect.has(Pay)` — TriggerKind::Pay = 9.
    c.set_insert(SetId::ChainKinds, 9);
    c.set_insert(SetId::ChainHits, owner as i64);
    c.set_scalar(VarId::EffectCount, 1);
    c
}

/// IFF: when the single-source guard admits, the counteract body rewrites the
/// pending pay amount to 0 and logs exactly that. When it rejects, no body.
#[kani::proof]
#[kani::unwind(16)]
pub fn vocalist_01_waive_iff() {
    let mut w = symbolic_world();
    let owner = any_seat(w.n_players);
    w.trig_player = owner; // actor
    w.trig_target = kani::any();
    kani::assume(w.trig_target >= 0 && w.trig_target < w.n_players);
    w.trig_value = kani::any();
    kani::assume(w.trig_value >= 0 && w.trig_value <= 20_000);
    w.pay_amount = w.trig_value;

    let gid = crate::guard_id!("actor == owner && effect.has(Pay) && target >= 0 && value >= 5000");
    let ctx = fill_vocalist(&w, owner);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (pay >= 5000)");
    kani::cover!(!admitted, "guard rejects");

    if !admitted {
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract(
        "CRYCHIC:主唱太拼命了",
        owner,
    ));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    assert!(
        spec::crychic::vocalist_01_log_ok(effects),
        "C-CRYCHIC-vocalist-01: exactly SetPayAmount{to: 0}"
    );
    assert!(
        post.pay_amount == 0,
        "C-CRYCHIC-vocalist-01: payment waived"
    );
    assert!(post.money == pre.money, "frame: money (waive is at rewrite time)");
    assert!(post.pos == pre.pos, "frame: pos");
}

/// MUTANT companion: SetPayAmount{1} is not what the body does.
#[kani::proof]
#[kani::unwind(16)]
pub fn vocalist_01_mutant_is_detected() {
    let mut w = symbolic_world();
    let owner = any_seat(w.n_players);
    w.trig_player = owner;
    w.trig_target = (owner + 1) % w.n_players;
    w.trig_value = 5000;
    w.pay_amount = 5000;
    world::reset_world(w);
    invoke::unwrap_done(invoke::counteract("CRYCHIC:主唱太拼命了", owner));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    assert!(spec::crychic::vocalist_01_log_ok(effects), "card matches book");
    let wrong = spec::crychic::vocalist_01_log_ok_mutant(effects);
    kani::cover!(!wrong, "mutant SetPayAmount{1} disagrees");
    assert!(!wrong, "mutant must NOT match");
}

// ===========================================================================
// C-CRYCHIC-grab-01 — 「（1）获得一层[停留]。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn grab_01_stay_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    let passer_key = intern::intern("want_to_grab_passer").0;

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("CRYCHIC:想要抓住...", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    // GiveStay is host-faithful and book-exact. The PASSER latch is
    // `state::set(PASSER, -1)` — the body is book-correct, but `Op::state_set`
    // stores the field id and drops the value, so the log carries
    // `SetState{n: 0}` rather than `n: -1`. Assert the stay (PROVEN) and the
    // presence of the latch write; the value is BLOCKED-HOST.
    assert!(
        effects.len() == 2
            && matches!(effects[0], Effect::GiveStay { player: p, n: 1 } if p == player)
            && matches!(effects[1], Effect::SetState { player: p, key, .. } if p == player && key == passer_key),
        "C-CRYCHIC-grab-01: GiveStay{{1}} + SetState(PASSER) present"
    );
    assert!(
        post.stay[player as usize] == pre.stay[player as usize] + 1,
        "C-CRYCHIC-grab-01: stay +1"
    );
    // Book wants PASSER = -1. Host drops the value (BLOCKED-HOST).
    let passer_val = key_get(player, intern::NameId(passer_key));
    kani::cover!(passer_val == -1, "book value -1 visible (fixed host)");
    kani::cover!(passer_val == 0, "host stores field id 0");
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

#[kani::proof]
#[kani::unwind(16)]
pub fn grab_01_mutant_is_detected() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    let passer_key = intern::intern("want_to_grab_passer").0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("CRYCHIC:想要抓住...", player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    assert!(
        matches!(effects[0], Effect::GiveStay { player: p, n: 1 } if p == player),
        "card gives exactly one stay"
    );
    let wrong = matches!(effects[0], Effect::GiveStay { player: p, n: 2 } if p == player);
    kani::cover!(!wrong, "two-stay mutant disagrees");
    assert!(!wrong, "mutant GiveStay{2} must NOT match");
}

// ===========================================================================
// C-CRYCHIC-debut-02 — turn-end stun + band crystal
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn debut_02_turn_end_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.band_crystals[player as usize] = kani::any();
    kani::assume(w.band_crystals[player as usize] >= 0 && w.band_crystals[player as usize] <= 9);

    let pre_crystals = w.band_crystals[player as usize];
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::entry("CRYCHIC:初演大成功", 1, player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let post_crystals = band_crystals_of(player);

    assert!(
        spec::crychic::debut_02_log_ok(effects, player),
        "C-CRYCHIC-debut-02: exactly GiveStun{player, 1}"
    );
    assert!(
        post_crystals == pre_crystals + spec::crychic::debut_02_crystal_delta(),
        "C-CRYCHIC-debut-02: band crystal +1"
    );
    // give_stun is modeled as stay+1 in the Kani host (stun_of reads stay).
    assert!(
        post.stay[player as usize] == pre.stay[player as usize] + 1,
        "C-CRYCHIC-debut-02: stun layer +1 (host models stun on stay)"
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

#[kani::proof]
#[kani::unwind(16)]
pub fn debut_02_mutant_is_detected() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.band_crystals[player as usize] = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry("CRYCHIC:初演大成功", 1, player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    assert!(spec::crychic::debut_02_log_ok(effects, player), "card matches book");
    let wrong = spec::crychic::debut_02_log_ok_mutant(effects, player);
    kani::cover!(!wrong, "GiveStay-mutant disagrees");
    assert!(!wrong, "mutant GiveStay must NOT match GiveStun");
}

/// C-CRYCHIC-debut-01 leaf: the play body itself logs nothing (scheduling ops
/// are protocol stubs). The standing no-drop claim is PROTOCOL-LEVEL.
#[kani::proof]
#[kani::unwind(12)]
pub fn debut_01_play_logs_nothing() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("CRYCHIC:初演大成功", player));
    let (logbuf, log_n) = effects();
    assert!(
        spec::crychic::debut_01_log_ok(&logbuf[..log_n]),
        "C-CRYCHIC-debut-01 leaf: play body produces no effects (arms stubs only)"
    );
}

// ===========================================================================
// C-CRYCHIC-ifonly-01 — cash out band crystals
// ===========================================================================

#[kani::proof]
#[kani::unwind(20)]
pub fn ifonly_01_cashout_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.band_crystals[player as usize] = kani::any();
    kani::assume(w.band_crystals[player as usize] >= 0 && w.band_crystals[player as usize] <= 9);
    let x = w.band_crystals[player as usize];

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let field0 = field_n();
    invoke::unwrap_done(invoke::play("CRYCHIC:如果能一直持续下去...", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    let post_crystals = band_crystals_of(player);

    assert!(
        spec::crychic::ifonly_01_log_ok(effects, player, x),
        "C-CRYCHIC-ifonly-01: GainMoney{2000+500X} + PlaceCard on field"
    );
    assert!(
        post.money[player as usize] == pre.money[player as usize] + spec::crychic::ifonly_01_amount(x),
        "C-CRYCHIC-ifonly-01: money +2000+500X"
    );
    assert!(
        post_crystals == 0,
        "C-CRYCHIC-ifonly-01: all band crystals removed"
    );
    assert!(
        field_n() == field0 + 1,
        "C-CRYCHIC-ifonly-01: card placed as [持续]"
    );
    kani::cover!(x == 0, "X = 0");
    kani::cover!(x >= 1, "X >= 1");
    assert!(post.pos == pre.pos, "frame: pos");
}

#[kani::proof]
#[kani::unwind(20)]
pub fn ifonly_01_mutant_is_detected() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    w.band_crystals[player as usize] = 2;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("CRYCHIC:如果能一直持续下去...", player));
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    assert!(
        spec::crychic::ifonly_01_log_ok(effects, player, 2),
        "card matches book at X=2"
    );
    let wrong = spec::crychic::ifonly_01_log_ok_mutant(effects, player, 2);
    kani::cover!(!wrong, "X+1 mutant disagrees");
    assert!(!wrong, "mutant 2000+500*(X+1) must NOT match");
}

// ===========================================================================
// C-CRYCHIC-ifonly-02 — [持续] hand >= 7 → discard
// ===========================================================================

/// IFF: when the single-source guard `card.placed && owner.hand >= 7` admits,
/// the card must be sent to the discard (UnplaceCard / field drop). The body
/// currently only `set_dest(Graveyard)` + narration — if that is all, this
/// proof FAILS (finding LESS).
#[kani::proof]
#[kani::unwind(20)]
pub fn ifonly_02_discard_at_seven_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;

    // `owner.hand` is the hand-size view: hand_n + resonance + non-empty bits.
    // Pin a placed card and hand >= 7 via the interned pile.
    w.hand_n = kani::any();
    kani::assume(w.hand_n >= 0 && w.hand_n <= 8);
    // Force the *view* to >= 7 by filling hand bits + hand_n.
    w.hand_n = 7;
    for i in 0..7 {
        w.hand_pile[i] = 100 + i as u16;
        w.hand[i] = true;
    }
    // Place a stand-in field card so `card.placed` is true.
    w.field_n = 1;
    w.field[0] = world::FieldCard {
        uid: 7,
        card: 0,
        player,
        tile: 0,
        face_down: false,
        immune: false,
        crystals: 0,
        props: world::NameMap::new(),
    };
    w.next_uid = 8;

    let gid = crate::guard_id!("card.placed && owner.hand >= 7");
    let mut ctx = ir_fill::fill_pay_add(&w, player);
    ctx.set_scalar(VarId::CardPlaced, 1);
    // `owner.hand` is a seat field; fill_pay_add does not set Hand. Set the
    // owner's hand seat field directly (the IR reads SeatField::Hand).
    use rules_cond_ir::ctx::SeatField;
    ctx.seat[player as usize][SeatField::Hand as usize] = 7;
    ctx.set_scalar(VarId::OwnerHand, 7);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (placed && hand >= 7)");
    kani::cover!(!admitted, "guard rejects");

    if !admitted {
        return;
    }

    let field0 = field_n();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only(
        "CRYCHIC:如果能一直持续下去...",
        "",
        player,
    ));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    // Book: 「此卡立即置入弃牌堆」 — the card leaves the field into discard.
    // Observed as UnplaceCard (host's unplace) or a field-count drop.
    let unplaced = spec::crychic::ifonly_02_log_ok(effects, 7)
        || field_n() < field0
        || effects
            .iter()
            .any(|e| matches!(e, Effect::UnplaceCard { .. }));
    assert!(
        unplaced,
        "C-CRYCHIC-ifonly-02: card must go to discard (UnplaceCard / field drop); \
         log={:?} field {} -> {}",
        effects,
        field0,
        field_n()
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

// ===========================================================================
// C-CRYCHIC-soyoreturn-01 branch 2 leaf — 「获得1000资金并将此卡加入手牌」
// ===========================================================================

#[kani::proof]
#[kani::unwind(20)]
pub fn soyoreturn_01_branch2_iff() {
    let mut w = symbolic_world();
    let player = any_seat(w.n_players);
    w.trig_player = player;
    // Branch 2 is pick index 1 of the two-option prompt.
    w.symbolic_answers = true;

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only(
        "CRYCHIC:（soyo）回到曾经",
        "Drawn",
        player,
    ));
    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    // Symbolic pick: branch 0 is the discard/cashout (needs a multi-step
    // arrange); branch 1 is the +1000 keep. Cover both; assert branch 2's
    // leaf when the pick is 1.
    let picked_keep = effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { n: 1000, .. }));
    let picked_discard = effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { n, .. } if *n != 1000 && *n > 0));
    kani::cover!(picked_keep, "branch 2 (keep, +1000)");
    kani::cover!(picked_discard, "branch 1 (discard cashout)");
    kani::cover!(effects.is_empty(), "no money branch visible");

    if picked_keep {
        assert!(
            spec::crychic::soyoreturn_01_branch2_log_ok(effects, player),
            "C-CRYCHIC-soyoreturn-01 branch 2: exactly GainMoney{1000}"
        );
    }
    assert!(post.pos == pre.pos, "frame: pos");
}