//! Unit `general`: clause proofs.
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. mutant (reachable-disagreement companion preferred).
//!
//! Cheap first: recruit / great / perfect money+pile clauses, then fever
//! play + turn-start. Hook clauses whose printed amount is read through
//! `ctx::n(0, …)` are BLOCKED-HOST (see spec/general.rs module docs and the
//! unit table). Targeting / counteraction / move-plan clauses are
//! PROTOCOL-LEVEL and are not faked here.
//!
//! Isolation assumes (stated, never hidden):
//! * `play_doubled` stub returns 0, so `ctx::n(k, v)` doubles only when
//!   `k == 0`. Clauses that tag their printed number with `k == 0` are
//!   BLOCKED-HOST; the ones below tag `k == 1` or use bare literals.
//! * No stacked hooks charge: we invoke a single body in isolation.

use crate::host;
use crate::invoke;
use crate::spec::general as spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume((2..=4).contains(&w.n_players));
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = false;
    }
    let player: i32 = kani::any();
    kani::assume((0..w.n_players).contains(&player));
    w.trig_player = player;
    w
}

fn take_log() -> (usize, [Effect; world::MAX_LOG]) {
    let log_n = world::with_world(|ww| ww.log_n);
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|ww| ww.log);
    (log_n, logbuf)
}

fn intern_count(pile: &[u16; world::MAX_CARDS], n: u8, name: &str) -> i32 {
    let id = crate::intern::intern_or_fail(name).0;
    world::pile_count(pile, n, id)
}

// ===========================================================================
// C-GEN-recruit-01 -- 「[手]：[消耗]1500资金，抽1张卡。」
// ===========================================================================

/// IFF: when the user can cover 1500, the body pays exactly 1500 and draws 1.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn recruit_01_iff() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    // Clause condition: the user has at least 1500 (shortfall is a different clause).
    kani::assume(w.money[player as usize] >= 1500);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("通用:10次招募（1回限定）"),
        player,
    ));
    let post = world::snapshot();
    let (log_n, logbuf) = take_log();
    let effects = &logbuf[..log_n];

    kani::cover!(pre.money[player as usize] == 1500, "exactly 1500");
    kani::cover!(pre.money[player as usize] > 1500, "more than 1500");

    crate::kassert!(
        spec::recruit_01_log_ok(effects, player),
        "C-GEN-recruit-01: log must be exactly PayMoney(1500) then Draw(1)"
    );
    crate::kassert!(
        spec::recruit_01_post_ok(&post, player, pre.money[player as usize]),
        "C-GEN-recruit-01: user's money falls by exactly 1500"
    );
    // Frame: everyone else's money untouched; stay/fire/pos untouched.
    crate::kassert!(
        spec::others_money_unchanged(&pre, &post, pre.n_players, player),
        "C-GEN-recruit-01: frame other money"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, pre.pos),
        "C-GEN-recruit-01: frame pos"
    );
    crate::kassert!(
        world::arr4_eq(post.charge, pre.charge),
        "C-GEN-recruit-01: frame fire"
    );
    crate::kassert!(post.stay == pre.stay, "C-GEN-recruit-01: frame stay");
}

/// MUTANT companion: the off-by-one spend does NOT match the card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn recruit_01_mutant_is_detected() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    kani::assume(w.money[player as usize] >= 1500);
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("通用:10次招募（1回限定）"),
        player,
    ));
    let (log_n, logbuf) = take_log();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::recruit_01_log_ok_mutant(effects, player),
        "mutant 1400-spend must not match the card"
    );
}

// ===========================================================================
// C-GEN-great-01 / 02 / 03
// 「1. [移除]此卡；2. 将一张“PERFECT“加入抽卡区并洗切；3. [获得]2000资金。」
// ===========================================================================

/// IFF: dest Banished, deck holds one more PERFECT, money +2000, log is the gain.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn great_01_02_03_iff() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    // Pre-existing PERFECTs in the deck are allowed (the clause adds one more).
    w.deck_n = kani::any();
    kani::assume(w.deck_n <= 6);
    let pre_deck_perfect = intern_count(&w.deck_pile, w.deck_n, "通用:[衍生]PERFECT");
    kani::assume(pre_deck_perfect <= 2);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("通用:GREAT"), player));
    let post = world::snapshot();
    let (log_n, logbuf) = take_log();
    let effects = &logbuf[..log_n];
    let post_dest = world::with_world(|ww| ww.dest);
    let (post_deck_n, post_deck) = world::with_world(|ww| (ww.deck_n, ww.deck_pile));
    let post_deck_perfect = intern_count(&post_deck, post_deck_n, "通用:[衍生]PERFECT");

    kani::cover!(pre_deck_perfect == 0, "no pre-existing PERFECT");
    kani::cover!(pre_deck_perfect > 0, "one pre-existing PERFECT");

    crate::kassert!(
        spec::great_01_log_ok(effects, player),
        "C-GEN-great-03: log must be exactly GainMoney(2000)"
    );
    crate::kassert!(
        spec::great_post_ok(
            post_dest,
            pre_deck_perfect,
            post_deck_perfect,
            pre.money[player as usize],
            post.money[player as usize]
        ),
        "C-GEN-great-01/02/03: Banished + one more PERFECT in deck + money+2000"
    );
    crate::kassert!(
        spec::others_money_unchanged(&pre, &post, pre.n_players, player),
        "C-GEN-great: frame other money"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "C-GEN-great: frame pos");
    crate::kassert!(post.stay == pre.stay, "C-GEN-great: frame stay");
}

/// MUTANT companion: the +2500 reference does not match the card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn great_01_mutant_is_detected() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("通用:GREAT"), player));
    let (log_n, logbuf) = take_log();
    crate::kassert!(
        !spec::great_01_log_ok_mutant(&logbuf[..log_n], player),
        "mutant +2500 must not match the card"
    );
}

// ===========================================================================
// C-GEN-perfect-01 / 02 / 03
// 「1. [移除]此卡；2. 将一张“FEVER!“加入弃卡区；3. [获得]3000资金。」
// ===========================================================================

/// IFF: dest Banished, discard holds one more FEVER!, money +3000.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn perfect_01_02_03_iff() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    w.discard_n = kani::any();
    kani::assume(w.discard_n <= 6);
    let pre_discard_fever = intern_count(&w.discard_pile, w.discard_n, "通用:[衍生]FEVER!");
    kani::assume(pre_discard_fever <= 2);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("通用:[衍生]PERFECT"),
        player,
    ));
    let post = world::snapshot();
    let (log_n, logbuf) = take_log();
    let effects = &logbuf[..log_n];
    let post_dest = world::with_world(|ww| ww.dest);
    let (post_discard_n, post_discard) = world::with_world(|ww| (ww.discard_n, ww.discard_pile));
    let post_discard_fever = intern_count(&post_discard, post_discard_n, "通用:[衍生]FEVER!");

    kani::cover!(pre_discard_fever == 0, "no pre-existing FEVER!");
    kani::cover!(pre_discard_fever > 0, "one pre-existing FEVER!");

    crate::kassert!(
        spec::perfect_01_log_ok(effects, player),
        "C-GEN-perfect-03: log must be exactly GainMoney(3000)"
    );
    crate::kassert!(
        spec::perfect_post_ok(
            post_dest,
            pre_discard_fever,
            post_discard_fever,
            pre.money[player as usize],
            post.money[player as usize]
        ),
        "C-GEN-perfect-01/02/03: Banished + one more FEVER! in discard + money+3000"
    );
    crate::kassert!(
        spec::others_money_unchanged(&pre, &post, pre.n_players, player),
        "C-GEN-perfect: frame other money"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, pre.pos),
        "C-GEN-perfect: frame pos"
    );
    crate::kassert!(post.stay == pre.stay, "C-GEN-perfect: frame stay");
}

/// MUTANT companion: the +2500 reference does not match the card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn perfect_01_mutant_is_detected() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("通用:[衍生]PERFECT"),
        player,
    ));
    let (log_n, logbuf) = take_log();
    crate::kassert!(
        !spec::perfect_01_log_ok_mutant(&logbuf[..log_n], player),
        "mutant +2500 must not match the card"
    );
}

// ===========================================================================
// C-GEN-fever-01 -- 「[手]：将此卡放置在[使用者]的[场地]。」
// ===========================================================================

/// IFF: PlaceCard on the user's field, nothing else.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fever_01_iff() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    // Isolation: no pre-placed FEVER, so the play body is the only writer.
    w.field_n = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("通用:[衍生]FEVER!"),
        player,
    ));
    let post = world::snapshot();
    let (log_n, logbuf) = take_log();
    let effects = &logbuf[..log_n];
    let this_card = world::card_slot_of("通用:[衍生]FEVER!");

    crate::kassert!(
        spec::fever_01_log_ok(effects, player, this_card),
        "C-GEN-fever-01: log must be exactly PlaceCard(Field)"
    );
    crate::kassert!(
        spec::others_money_unchanged(&pre, &post, pre.n_players, player),
        "C-GEN-fever-01: frame money"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, pre.pos),
        "C-GEN-fever-01: frame pos"
    );
    crate::kassert!(post.stay == pre.stay, "C-GEN-fever-01: frame stay");
    // Post: one field instance owned by the player.
    let (field_n, owner0) = world::with_world(|ww| {
        let n = ww.field_n;
        let o = if n > 0 { ww.field[0].player } else { -99 };
        (n, o)
    });
    crate::kassert!(field_n == 1, "C-GEN-fever-01: exactly one field instance");
    crate::kassert!(
        owner0 == player,
        "C-GEN-fever-01: instance owned by the user"
    );
}

/// MUTANT companion: dest Graveyard must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fever_01_mutant_is_detected() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    w.field_n = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(
        crate::card_id!("通用:[衍生]FEVER!"),
        player,
    ));
    let (log_n, logbuf) = take_log();
    let this_card = world::card_slot_of("通用:[衍生]FEVER!");
    crate::kassert!(
        !spec::fever_01_log_ok_mutant(&logbuf[..log_n], player, this_card),
        "mutant dest=Graveyard must not match the card"
    );
}

// ===========================================================================
// C-GEN-fever-04 -- 「[持续]（2）[拥有者]回合开始时将此卡放入[使用者]弃卡区。」
// Guard on the hook: `card.placed` (single-source IR).
// ===========================================================================

fn fever_turn_start_world(placed: bool, same_turn: bool) -> World {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    if placed {
        w.field_n = 1;
        w.field[0].player = player;
        w.field[0].card = crate::intern::intern_or_fail("通用:[衍生]FEVER!").0;
    } else {
        w.field_n = 0;
    }
    use card_sdk::abi::TriggerKind;
    w.trig_kind = "turnStart";
    if same_turn {
        w.trig_player = player;
    } else {
        let other: i32 = kani::any();
        kani::assume((0..w.n_players).contains(&other) && other != player);
        w.trig_player = other;
    }
    w
}

fn fever_guard_admits(w: &World, player: i32, placed: bool) -> bool {
    let gid = crate::guard_id!("card.placed");
    let ov = crate::ir_fill::TriggerOverlay::default();
    let card = crate::intern::intern_or_fail("通用:[衍生]FEVER!").0 as i64;
    // `card.placed` reads one scalar; `fill_for_guard` skips the 60-tile /
    // geometry / VARS bulk that made this harness fail the unwinding
    // assertion at `fill_from_world`.
    let ctx = crate::ir_fill::fill_for_guard(w, gid, player, card, placed, -1, &ov);
    crate::guard!(gid, &ctx)
}

/// IFF: when the card is placed and the turn-start is the owner's, dest becomes
/// Graveyard and the effect log is empty (set_dest is unlogged).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fever_04_turn_start_iff() {
    let w = fever_turn_start_world(true, true);
    let player = w.trig_player;
    let admitted = fever_guard_admits(&w, player, true);
    kani::cover!(admitted, "card.placed admits");
    crate::kassert!(
        admitted,
        "C-GEN-fever-04: placed card must pass card.placed"
    );
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only_idx(
        crate::card_id!("通用:[衍生]FEVER!"),
        "TurnStart",
        player,
    ));
    let post = world::snapshot();
    let (log_n, logbuf) = take_log();
    let effects = &logbuf[..log_n];
    let post_dest = world::with_world(|ww| ww.dest);

    kani::cover!(log_n == 0, "no effects (set_dest is unlogged)");

    crate::kassert!(
        spec::fever_04_log_ok(effects),
        "C-GEN-fever-04: no Effect log entries"
    );
    crate::kassert!(
        post_dest == spec::DEST_GRAVEYARD,
        "C-GEN-fever-04: dest must be Graveyard (弃卡区)"
    );
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "C-GEN-fever-04: frame money"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, pre.pos),
        "C-GEN-fever-04: frame pos"
    );
    crate::kassert!(post.stay == pre.stay, "C-GEN-fever-04: frame stay");
}

/// NO-OP: (a) the guard rejects when the card is not placed (host would not
/// call the body); (b) a turn start that is not the owner's is a body no-op.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fever_04_wrong_turn_noop() {
    let w = fever_turn_start_world(true, false);
    let player = w.trig_player;
    let admitted = fever_guard_admits(&w, player, true);
    let pre_dest = w.dest;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    // The host admits (`card.placed`); the body itself must no-op because
    // `trigger::player_id() != player_id`.
    crate::kassert!(admitted, "C-GEN-fever-04: placed card passes the guard");
    invoke::unwrap_done(invoke::hook_only_idx(
        crate::card_id!("通用:[衍生]FEVER!"),
        "TurnStart",
        player,
    ));
    let post = world::snapshot();
    let (log_n, _) = take_log();
    let post_dest = world::with_world(|ww| ww.dest);
    crate::kassert!(log_n == 0, "C-GEN-fever-04: wrong turn start is a no-op");
    crate::kassert!(post_dest == pre_dest, "C-GEN-fever-04: dest untouched");
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "C-GEN-fever-04: frame money"
    );
    crate::kassert!(post.stay == pre.stay, "C-GEN-fever-04: frame stay");
}

/// Guard-reject path: not placed ⇒ host does not call the body.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fever_04_not_placed_guard_rejects() {
    let w = fever_turn_start_world(false, true);
    let player = w.trig_player;
    let admitted = fever_guard_admits(&w, player, false);
    crate::kassert!(
        !admitted,
        "C-GEN-fever-04: unplaced card must fail card.placed"
    );
}

/// MUTANT companion: dest Banished must not match.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn fever_04_mutant_is_detected() {
    let w = fever_turn_start_world(true, true);
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only_idx(
        crate::card_id!("通用:[衍生]FEVER!"),
        "TurnStart",
        player,
    ));
    let post_dest = world::with_world(|ww| ww.dest);
    crate::kassert!(
        !spec::fever_04_dest_ok_mutant(post_dest),
        "mutant dest=Banished must not match the card"
    );
}

// ===========================================================================
// C-GEN-budokan-01 -- X formula (pure). The transfer loop is PROTOCOL-LEVEL
// (Op::target is a stub returning 0), so the body is not harnessed end-to-end.
// The pure X contract is still machine-checked here so the printed
// 「向上取整10」 reading is pinned.
// ===========================================================================

/// Pinned reading: `X = ceil_to_10(2000 / n)`, with the 3-player case 670.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn budokan_x_formula() {
    let n: i32 = kani::any();
    kani::assume((1..=3).contains(&n));
    let x = spec::budokan_x(n);
    kani::cover!(n == 1, "1 other living");
    kani::cover!(n == 2, "2 others");
    kani::cover!(n == 3, "3 others (2000/3 -> 670)");
    // Ceil-to-10: x is a multiple of 10, 2000/n <= x < 2000/n + 10.
    crate::kassert!(x % 10 == 0, "X is a multiple of 10");
    crate::kassert!(x >= (2000 + n - 1) / n, "X is at least ceil(2000/n)");
    crate::kassert!(x < (2000 + n - 1) / n + 10, "X is below ceil(2000/n)+10");
    if n == 3 {
        crate::kassert!(x == 670, "2000/3 ceil-10 is 670");
    }
    if n == 1 {
        crate::kassert!(x == 2000, "2000/1 is 2000");
    }
    if n == 2 {
        crate::kassert!(x == 1000, "2000/2 is 1000");
    }
}

/// MUTANT companion: floor-to-10 disagrees on n=3 (660 vs 670).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn budokan_x_mutant_is_detected() {
    let n: i32 = 3;
    crate::kassert!(
        spec::budokan_x(n) != spec::budokan_x_mutant(n),
        "ceil-10 vs floor-10 must disagree on 2000/3"
    );
}

// ===========================================================================
// C-GEN-harvest-01 -- step bound (pure). The move-plan rewrite + settle is
// PROTOCOL-LEVEL. The printed 6 is a 1-based doubling slot (`ctx::n(1, 6)`),
// so the legal set is 1..=6, or 1..=12 when 「后勤人员的努力」 doubles it.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn harvest_steps_bound() {
    let s: i32 = kani::any();
    let doubled: bool = kani::any();
    kani::assume((-2..=15).contains(&s));
    let cap = if doubled { 12 } else { 6 };
    let ok = spec::harvest_steps_ok(s, doubled);
    kani::cover!(s == 1, "min step");
    kani::cover!(s == cap, "cap step");
    kani::cover!(s == 0, "below");
    kani::cover!(s == cap + 1, "above the cap");
    assert_eq!(ok, (1..=cap).contains(&s), "1..=cap is the legal set");
}

/// MUTANT companion: the off-by-one cap is distinguishable.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn harvest_steps_mutant_is_detected() {
    crate::kassert!(
        spec::harvest_steps_ok(7, false) != spec::harvest_steps_ok_mutant(7, false),
        "off-by-one mutant must disagree at 7"
    );
}

// ===========================================================================
// C-GEN-tsugu-01 -- branch predicate (pure). The draw half is modeled in the
// body but the full 3d10 [手] also rewrites the main move / offers a buy
// (PROTOCOL-LEVEL), so only the printed thresholds are pinned here.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn tsugu_thresholds() {
    let s: i32 = kani::any();
    kani::assume((3..=30).contains(&s));
    kani::cover!(s == 21, "just below 22");
    kani::cover!(s == 22, "exactly 22");
    kani::cover!(s == 26, "exactly 26");
    kani::cover!(s == 28, "exactly 28");
    kani::cover!(s == 25, "lt26");
    assert_eq!(spec::tsugu_ge22(s), s >= 22);
    assert_eq!(spec::tsugu_ge26(s), s >= 26);
    assert_eq!(spec::tsugu_ge28(s), s >= 28);
    assert_eq!(spec::tsugu_lt26(s), s < 26);
    // Stacking: ≥28 implies ≥26 and ≥22.
    if s >= 28 {
        crate::kassert!(spec::tsugu_ge26(s) && spec::tsugu_ge22(s), "stacking");
    }
    // Branch 4 does not stack with branch 2.
    if s >= 26 {
        crate::kassert!(!spec::tsugu_lt26(s), "04 excludes 02");
    }
}

/// MUTANT companion: exclusive 22 disagrees at 22.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn tsugu_ge22_mutant_is_detected() {
    crate::kassert!(
        spec::tsugu_ge22(22) != spec::tsugu_ge22_mutant(22),
        "exclusive-22 mutant must disagree at 22"
    );
}

// ===========================================================================
// C-GEN-party-02 -- payout predicate (pure). The multi-player prompt loop is
// PROTOCOL-LEVEL / expensive; X and the inclusive-35 gate are pinned here.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn party_x_and_payout() {
    let others: i32 = kani::any();
    kani::assume((0..=3).contains(&others));
    let x = spec::party_x(others);
    crate::kassert!(x == others + 1, "X = other spenders + 1");
    kani::cover!(x == 1, "X=1 extra-turn branch");
    kani::cover!(x == 2, "X=2 roll branch");

    let r: i32 = kani::any();
    kani::assume((x..=20 * x).contains(&r)); // Xd20 range
    let hit = spec::party_pays_out(r);
    kani::cover!(r == 35, "exactly 35 (inclusive)");
    kani::cover!(r == 34, "just below");
    assert_eq!(hit, r >= 35, "「至少为35」 is inclusive");
}

/// MUTANT companion: exclusive 35 disagrees at 35.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn party_payout_mutant_is_detected() {
    crate::kassert!(
        spec::party_pays_out(35) != spec::party_pays_out_mutant(35),
        "exclusive-35 mutant must disagree at 35"
    );
}

// ===========================================================================
// C-GEN-rain-01 -- X range (pure). The [指定] pipeline is a protocol stub
// (Op::target returns 0), so the designation loop is PROTOCOL-LEVEL.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn rain_x_range() {
    // 2d2 sum: the host's roll_dice is symbolic; pin the printed range.
    let x: i32 = kani::any();
    kani::assume((0..=6).contains(&x));
    let in_range = (spec::RAIN_X_LO..=spec::RAIN_X_HI).contains(&x);
    kani::cover!(x == 2, "min");
    kani::cover!(x == 4, "max");
    kani::cover!(x == 1, "below (impossible on 2d2)");
    assert_eq!(in_range, x >= 2 && x <= 4, "2d2 sum is in 2..=4");
}

// ===========================================================================
// C-GEN-encore-01 -- leaf body: negate and change nothing else.
// The [反击] window is PROTOCOL-LEVEL.
//
// BLOCKED-HOST: `Op::trig_set_negate_effect` is a **unit** call (no argument),
// but the host models it as `trig_negate = (a0 != 0)` and the ABI always
// passes `a0 = 0`, so the flag never sets. The body correctly calls
// `trigger::negate_effect()`; the host cannot observe it. The harness below
// proves only the quiet-frame half (no Effect, money/pos/stay untouched) and
// the unit table records BLOCKED-HOST for the negation itself.
// ===========================================================================

/// Quiet-frame half of C-GEN-encore-01. Does NOT assert `trig_negate`
/// (BLOCKED-HOST, see module notes).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn encore_01_leaf_quiet_frame() {
    let mut w = symbolic_play_world();
    let player = w.trig_player;
    w.trig_negate = false;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::counteract_idx(crate::card_id!("通用:安可"), player));
    let post = world::snapshot();
    let (log_n, logbuf) = take_log();
    crate::kassert!(
        spec::encore_01_log_ok(&logbuf[..log_n]),
        "C-GEN-encore-01: no Effect entries (negation is trig_negate)"
    );
    crate::kassert!(
        world::arr4_eq(post.money, pre.money),
        "C-GEN-encore-01: frame money"
    );
    crate::kassert!(
        world::arr4_eq(post.pos, pre.pos),
        "C-GEN-encore-01: frame pos"
    );
    crate::kassert!(post.stay == pre.stay, "C-GEN-encore-01: frame stay");
    // Documented host gap (not a body finding):
    let negate = world::with_world(|ww| ww.trig_negate);
    let _ = negate; // BLOCKED-HOST: unit-call models as a0 != 0 with a0 = 0
}

// ===========================================================================
// C-GEN-clearcp-02 -- settle payout amount (pure + body-visible gain).
// The multi-turn spread (03) and the mark plumbing are PROTOCOL-LEVEL /
// OPEN; the printed 800 is pinned here.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn clearcp_settle_amount() {
    let settler: i32 = kani::any();
    kani::assume((0..4).contains(&settler));
    // The printed settle payout is 800; the on-card count starts at 6 and
    // drops by 1 per settle.
    let n: i32 = kani::any();
    kani::assume((0..=8).contains(&n));
    if n == spec::CLEARCP_START {
        crate::kassert!(!spec::clearcp_after_settle(n), "6 is pre-settle");
    }
    if n == spec::CLEARCP_START - 1 {
        crate::kassert!(spec::clearcp_after_settle(n), "5 is post one settle");
    }
    if n == 0 {
        crate::kassert!(
            spec::clearcp_graveyards_at_zero(n),
            "0 triggers the graveyard"
        );
    }
    // Pinned amount.
    crate::kassert!(
        spec::clearcp_02_effects_ok(
            &[Effect::GainMoney {
                player: settler,
                n: 800
            }],
            settler
        ),
        "settle pays the settler 800"
    );
    crate::kassert!(
        !spec::clearcp_02_effects_ok_mutant(
            &[Effect::GainMoney {
                player: settler,
                n: 800
            }],
            settler
        ),
        "mutant 1000 must not match 800"
    );
}

// ===========================================================================
// C-GEN-parking-01 -- house-delta bound (pure).
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn parking_house_delta_bound() {
    let d: i32 = kani::any();
    kani::assume((-1..=3).contains(&d));
    let ok = spec::parking_house_delta_ok(d);
    kani::cover!(d == 1, "at most +1");
    kani::cover!(d == 2, "over the cap");
    assert_eq!(ok, d <= 1, "「每个格子因此效果最多获得1层」");
}

/// MUTANT companion: the +2 set is distinguishable.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn parking_house_delta_mutant_is_detected() {
    crate::kassert!(
        spec::parking_house_delta_ok(2) != spec::parking_house_delta_ok_mutant(2),
        "+2 mutant must disagree"
    );
}
