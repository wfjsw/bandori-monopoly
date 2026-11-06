//! HOST-OPS smoke proofs: one tiny harness per newly modeled op family.
//!
//! These pin the *host model formulas* (not a card clause) under Kani:
//!
//! | harness | family | obligation |
//! |---|---|---|
//! | `rent_of_property_table` | `rent_of` | table lookup at symbolic houses |
//! | `rent_of_ring_is_price` | `rent_of` | RiNG quotes the tile's `price` |
//! | `tile_at_set_tile_at_roundtrip` | `tile_at` / `set_tile_at` | move + readback |
//! | `trig_move_sentinels` | `trig_move_*` | no-move sentinels |
//! | `trig_move_payload_reads` | `trig_move_*` | payload getters |
//! | `trig_buy_payload_reads` | `trig_price` / seller / kind / deal | buy snap |
//! | `trig_roll_source_reads` | `trig_roll_source` | guest codes 0..=3 |
//!
//! Conformance against the real engine is `rules/kani/tests/conf_model.rs`;
//! these proofs only show the model arms are non-vacuous and total.

use card_sdk::kani_host::{Args, Op};

use crate::board;
use crate::host;
use crate::world::{self, BuySnap, MoveSnap, World, ROLL_CARD, ROLL_CHARGE, ROLL_SKILL};

fn install_empty() {
    host::install();
    world::reset_world(World::new());
}

fn call(op: Op, a: Args) -> i64 {
    host::dispatch(op, a)
}

// ---------------------------------------------------------------------------
// rent_of
// ---------------------------------------------------------------------------

/// Property rent = `board::table_rent` at the counted houses (clamped).
/// Fixed tile (购物中心 = engine index 1) so CBMC explores only the house
/// axis; `conf_model` pins the multi-tile grid.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(12)]
pub fn rent_of_property_table() {
    install_empty();
    let tile: i32 = 1; // 购物中心, rent [300, 1360, 3220, 6680]
    let houses: i32 = kani::any();
    kani::assume((0..=5).contains(&houses));
    world::with_world_mut(|w| {
        w.tiles[tile as usize].houses = houses;
    });
    let got = call(Op::rent_of, [tile, 0, 0, 0, 0, 0]);
    let t = board::data_at(tile).unwrap();
    let expect = board::table_rent(t, houses) as i64;
    kani::cover!(houses == 0, "0 houses");
    kani::cover!(houses == 2, "2 houses");
    kani::cover!(houses >= 4, "clamped at 3+");
    crate::kassert!(
        got == expect,
        "rent_of must equal table_rent at counted houses"
    );
}

/// RiNG rent quotes the tile's base `price` (the rolled figure is a pay-time
/// fact; `WorldOps::rent_of` returns the table value).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn rent_of_ring_is_price() {
    install_empty();
    let ring: i32 = kani::any();
    kani::assume(board::data_at(ring).map_or(false, board::is_deed));
    let got = call(Op::rent_of, [ring, 0, 0, 0, 0, 0]);
    let price = board::data_at(ring).unwrap().price as i64;
    crate::kassert!(got == price, "RiNG rent_of is the tile's base price");
}

// ---------------------------------------------------------------------------
// tile_at / set_tile_at
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(12)]
pub fn tile_at_set_tile_at_roundtrip() {
    install_empty();
    let uid: i32 = kani::any();
    kani::assume(uid > 0 && uid < 1000);
    let from: i32 = kani::any();
    kani::assume((-1..=59).contains(&from));
    let to: i32 = kani::any();
    kani::assume((-1..=59).contains(&to));
    world::with_world_mut(|w| {
        w.field[0] = world::FieldCard {
            uid,
            card: 0,
            player: 0,
            tile: from,
            face_down: false,
            immune: false,
            crystals: 0,
            props: world::NameMap::new(),
            effective_card: u16::MAX,
        };
        w.field_n = 1;
    });
    crate::kassert!(call(Op::tile_at, [uid, 0, 0, 0, 0, 0]) == from as i64);
    crate::kassert!(call(Op::set_tile_at, [uid, to, 0, 0, 0, 0]) == 1);
    crate::kassert!(call(Op::tile_at, [uid, 0, 0, 0, 0, 0]) == to as i64);
    // Missing uid: set_tile_at is a false no-op.
    crate::kassert!(call(Op::set_tile_at, [uid + 500, 0, 0, 0, 0, 0]) == 0);
    crate::kassert!(call(Op::tile_at, [uid + 500, 0, 0, 0, 0, 0]) == -1);
}

// ---------------------------------------------------------------------------
// trig_move_*
// ---------------------------------------------------------------------------

/// No-move sentinels (hostfns `map_or` defaults).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn trig_move_sentinels() {
    install_empty();
    world::with_world_mut(|w| {
        w.move_snap = MoveSnap::NONE;
        w.trig_main = true;
    });
    crate::kassert!(call(Op::trig_move_kind, [0; 6]) == -1);
    crate::kassert!(call(Op::trig_move_dir, [0; 6]) == 1);
    crate::kassert!(call(Op::trig_move_from, [0; 6]) == -1);
    crate::kassert!(call(Op::trig_move_roll, [0; 6]) == -1);
    crate::kassert!(call(Op::trig_move_remaining, [0; 6]) == 0);
    crate::kassert!(call(Op::trig_move_total, [0; 6]) == 0);
    crate::kassert!(call(Op::trig_move_settles, [0; 6]) == 0);
    // `trig_move_main` is `Trigger::main`, independent of the move payload.
    crate::kassert!(call(Op::trig_move_main, [0; 6]) == 1);
}

/// Payload getters return the stored `MoveSnap`.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn trig_move_payload_reads() {
    install_empty();
    let kind: i32 = kani::any();
    kani::assume(kind == 0 || kind == 1);
    let dir: i32 = kani::any();
    kani::assume(dir == 1 || dir == -1);
    let from: i32 = kani::any();
    kani::assume((0..=59).contains(&from));
    let remaining: i32 = kani::any();
    kani::assume((0..=20).contains(&remaining));
    let total: i32 = kani::any();
    kani::assume((0..=20).contains(&total));
    let roll: i32 = kani::any();
    kani::assume((2..=12).contains(&roll));
    let resolve: bool = kani::any();
    world::with_world_mut(|w| {
        let mut m = MoveSnap::NONE;
        m.kind = kind;
        m.dir = dir;
        m.from = from;
        m.remaining = remaining;
        m.total = total;
        m.roll = roll;
        m.settles = resolve;
        w.move_snap = m;
    });
    crate::kassert!(call(Op::trig_move_kind, [0; 6]) == kind as i64);
    crate::kassert!(call(Op::trig_move_dir, [0; 6]) == dir as i64);
    crate::kassert!(call(Op::trig_move_from, [0; 6]) == from as i64);
    crate::kassert!(call(Op::trig_move_remaining, [0; 6]) == remaining as i64);
    crate::kassert!(call(Op::trig_move_total, [0; 6]) == total as i64);
    crate::kassert!(call(Op::trig_move_roll, [0; 6]) == roll as i64);
    crate::kassert!(call(Op::trig_move_settles, [0; 6]) == resolve as i64);
}

// ---------------------------------------------------------------------------
// trig_price / seller / buy_kind / deal
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn trig_buy_payload_reads() {
    install_empty();
    let kind: i32 = kani::any();
    kani::assume((0..=5).contains(&kind));
    let seller: i32 = kani::any();
    kani::assume(seller == -1 || (0..=3).contains(&seller));
    let price: i32 = kani::any();
    kani::assume((0..=10_000).contains(&price));
    let deal_owner: i32 = kani::any();
    kani::assume(deal_owner == -1 || (0..=3).contains(&deal_owner));
    let deal_houses: i32 = kani::any();
    kani::assume((0..=5).contains(&deal_houses));
    let deal_mortgaged: bool = kani::any();
    world::with_world_mut(|w| {
        w.buy_snap = BuySnap {
            kind,
            seller,
            price,
            deal_owner,
            deal_houses,
            deal_mortgaged,
        };
    });
    crate::kassert!(call(Op::trig_buy_kind, [0; 6]) == kind as i64);
    crate::kassert!(call(Op::trig_seller, [0; 6]) == seller as i64);
    crate::kassert!(call(Op::trig_price, [0; 6]) == price as i64);
    crate::kassert!(call(Op::trig_deal_owner, [0; 6]) == deal_owner as i64);
    crate::kassert!(call(Op::trig_deal_houses, [0; 6]) == deal_houses as i64);
    crate::kassert!(call(Op::trig_deal_mortgaged, [0; 6]) == deal_mortgaged as i64);
    // Setters rewrite the snap.
    call(Op::trig_set_price, [price + 1, 0, 0, 0, 0, 0]);
    crate::kassert!(call(Op::trig_price, [0; 6]) == (price + 1) as i64);
}

// ---------------------------------------------------------------------------
// trig_roll_source
// ---------------------------------------------------------------------------

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn trig_roll_source_reads() {
    install_empty();
    let src: i32 = kani::any();
    kani::assume((0..=3).contains(&src));
    world::with_world_mut(|w| w.trig_roll_source = src);
    let got = call(Op::trig_roll_source, [0; 6]);
    crate::kassert!(got == src as i64);
    kani::cover!(src == ROLL_CHARGE, "fire pot");
    kani::cover!(src == ROLL_CARD, "card");
    kani::cover!(src == ROLL_SKILL, "skill");
}

// ---------------------------------------------------------------------------
// v105 identity / frame / counter cap (stubbed until modeled)
// ---------------------------------------------------------------------------

/// The six v105 imports answer their documented defaults: no raise identity
/// (`0` / `-1`), the body runs (`1`), and no stored cap (`0`). Replacing a
/// stub with a model must keep these totals.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn identity_and_cap_stubs() {
    install_empty();
    let uid: i32 = kani::any();
    crate::kassert!(call(Op::trig_uid, [0; 6]) == 0);
    crate::kassert!(call(Op::trig_instance, [0; 6]) == -1);
    crate::kassert!(call(Op::body_enter, [0; 6]) == 1);
    crate::kassert!(call(Op::body_leave, [0; 6]) == 0);
    crate::kassert!(call(Op::counter_max_at, [uid, 0, 0, 0, 0, 0]) == 0);
    crate::kassert!(call(Op::set_counter_max_at, [uid, 0, 0, 0, 0, 0]) == 0);
}

// ---------------------------------------------------------------------------
// G05 effective-card surface (§6.2)
// ---------------------------------------------------------------------------

/// Invariant `placed_instance_answers_with_its_effective_card`: each instance
/// resolves its **own** surface -- an unmarked copy its row's card, a marked
/// copy the stand-in -- and clearing the marker restores the row's card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn placed_instance_answers_with_its_effective_card() {
    install_empty();
    const PLAIN: &str = "Mor:（NNM）稍微努力了一下";
    const OTHER: &str = "HHW:whatever";
    const STAND_IN: &str = "PP:[冰川日菜]会发出怎样的声音呢？";
    let (a, b) = world::with_world_mut(|w| {
        w.n_players = 2;
        (w.place_instance(0, PLAIN), w.place_instance(1, OTHER))
    });
    let plain = crate::intern::intern(PLAIN);
    let other = crate::intern::intern(OTHER);
    let stand_in = crate::intern::intern(STAND_IN);

    // Unmarked: the row's own card.
    crate::kassert!(
        card_sdk::ctx::effective_card_at(a) == Some(plain),
        "an unmarked instance answers its own card"
    );
    // Marked: the stand-in's surface, and the sibling is unaffected.
    card_sdk::ctx::set_effective_card_at(b, STAND_IN);
    crate::kassert!(
        card_sdk::ctx::effective_card_at(b) == Some(stand_in),
        "a marked instance answers the stand-in"
    );
    crate::kassert!(
        card_sdk::ctx::effective_card_at(a) == Some(plain),
        "the sibling is unaffected by the other's marker"
    );
    kani::cover!(true, "both instances read independently");
    // Clearing restores the row's own card.
    card_sdk::ctx::set_effective_card_at(b, "");
    crate::kassert!(
        card_sdk::ctx::effective_card_at(b) == Some(other),
        "clearing returns the row's card"
    );
}

// ---------------------------------------------------------------------------
// G10 outcome surface (§10.2) -- one presence proof per new op, so an
// unmodeled op cannot answer a silent 0
// ---------------------------------------------------------------------------

/// `trigger_by_card_instance` / `trigger_redirected_from` /
/// `trigger_settle_facts` / `trigger_settle_paid_value` answer the stored
/// trigger facts verbatim (sentinels `-1` outside their raise).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn presence_g10_trigger_reads() {
    install_empty();
    world::with_world_mut(|w| {
        w.trig_by_card_instance = 7;
        w.trig_redirected_from = 3;
        w.trig_settle_facts = 4;
        w.trig_settle_paid_value = 160;
    });
    crate::kassert!(call(Op::trig_by_card_instance, [0; 6]) == 7);
    crate::kassert!(call(Op::trig_redirected_from, [0; 6]) == 3);
    crate::kassert!(call(Op::trig_settle_facts, [0; 6]) == 4);
    crate::kassert!(call(Op::trig_settle_paid_value, [0; 6]) == 160);
    // A board raise: no cause, no redirect origin, no settle.
    world::with_world_mut(|w| {
        w.trig_by_card_instance = -1;
        w.trig_redirected_from = -1;
        w.trig_settle_facts = 0;
    });
    crate::kassert!(call(Op::trig_by_card_instance, [0; 6]) == -1);
    crate::kassert!(call(Op::trig_redirected_from, [0; 6]) == -1);
    crate::kassert!(call(Op::trig_settle_facts, [0; 6]) == 0);
    kani::cover!(true, "sentinels");
}

/// `draw_event_as` answers 1 and records the `(player, tile)` attribution.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn presence_g10_draw_event_as() {
    install_empty();
    crate::kassert!(call(Op::draw_event_as, [1, 42, 0, 0, 0, 0]) == 1);
    world::with_world(|w| {
        crate::kassert!(w.last_event_draw == (1, 42), "the attribution is stored");
    });
    kani::cover!(true, "attributed");
}

/// `event_defer` claims once per pair: the first claimant answers 1, its own
/// re-run answers 1 without a second record, and another instance answers 0.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn presence_g10_event_defer_is_once() {
    install_empty();
    world::with_world_mut(|w| {
        w.trig_kind = "event";
        w.trig_card = 5;
        w.trig_player = 0;
        w.trig_tile = 12;
        w.bound_uid = 1;
    });
    crate::kassert!(call(Op::event_defer, [0; 6]) == 1, "first claim");
    crate::kassert!(call(Op::event_defer, [0; 6]) == 1, "own re-run");
    world::with_world_mut(|w| w.bound_uid = 2);
    crate::kassert!(call(Op::event_defer, [0; 6]) == 0, "another instance");
    world::with_world(|w| {
        crate::kassert!(w.deferred_n == 1, "one record");
        crate::kassert!(w.deferred[0].deferrer_uid == 1, "owner");
    });
    // Outside an `event` raise: 0, nothing pushed.
    world::with_world_mut(|w| w.trig_kind = "turnStart");
    crate::kassert!(call(Op::event_defer, [0; 6]) == 0, "not an event");
    world::with_world(|w| crate::kassert!(w.deferred_n == 1, "still one"));
    kani::cover!(true, "once");
}

/// `world_snapshot` / `world_restore` / `world_snapshot_drop`: a key from 1,
/// an absolute idempotent restore for an owner, `0` for a stranger or a
/// missing key, and the last owner frees the key.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
pub fn presence_g10_world_snapshot() {
    install_empty();
    world::with_world_mut(|w| {
        w.bound_uid = 1;
        w.money[0] = 1_234;
        w.pos[0] = 20;
        w.tiles[5].owner = 1;
    });
    let key = call(Op::world_snapshot, [0; 6]) as i32;
    crate::kassert!(key == 1, "first key is 1");
    world::with_world_mut(|w| {
        w.money[0] = 0;
        w.pos[0] = 5;
        w.tiles[5].owner = -1;
    });
    crate::kassert!(call(Op::world_restore, [key, 0, 0, 0, 0, 0]) == 1);
    world::with_world(|w| {
        crate::kassert!(w.money[0] == 1_234 && w.pos[0] == 20, "restored");
        crate::kassert!(w.tiles[5].owner == 1, "owners restored");
    });
    crate::kassert!(call(Op::world_restore, [key, 0, 0, 0, 0, 0]) == 1);
    // A stranger answers 0 and changes nothing.
    world::with_world_mut(|w| {
        w.bound_uid = 2;
        w.money[0] = 7;
    });
    crate::kassert!(call(Op::world_restore, [key, 0, 0, 0, 0, 0]) == 0);
    world::with_world(|w| crate::kassert!(w.money[0] == 7, "no write"));
    // A missing key answers 0.
    crate::kassert!(call(Op::world_restore, [key + 9, 0, 0, 0, 0, 0]) == 0);
    // The owner's drop frees the key; a later restore answers 0.
    world::with_world_mut(|w| w.bound_uid = 1);
    crate::kassert!(call(Op::world_snapshot_drop, [key, 0, 0, 0, 0, 0]) == 1);
    crate::kassert!(call(Op::world_restore, [key, 0, 0, 0, 0, 0]) == 0);
    world::with_world(|w| crate::kassert!(!w.snapshots[0].used, "freed"));
    kani::cover!(true, "snapshot cycle");
}

/// MUTANT: a marked instance keeps answering its row's card.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(16)]
#[kani::should_panic]
pub fn placed_instance_answers_with_its_effective_card_mutant() {
    install_empty();
    const OTHER: &str = "HHW:whatever";
    const STAND_IN: &str = "PP:[冰川日菜]会发出怎样的声音呢？";
    let b = world::with_world_mut(|w| {
        w.n_players = 2;
        w.place_instance(1, OTHER)
    });
    card_sdk::ctx::set_effective_card_at(b, STAND_IN);
    crate::kassert!(
        card_sdk::ctx::effective_card_at(b) == Some(crate::intern::intern(OTHER)),
        "mutant: marker ignored"
    );
}
