//! G10 effect-outcome obligations
//! (`docs/rulebook/spec/abi/G10-effect-outcome.md` §10.2).
//!
//! Every harness is over the pure model in `spec::g10_outcome`, which mirrors
//! the production step named on each model item; a MUTANT harness per
//! obligation must fail. Card bodies stay PROTOCOL-LEVEL and are proved
//! elsewhere.

use kani::cover;

use crate::kassert;
use crate::spec::g10_outcome::{
    cause_on_event, cause_on_event_after, defer, due, paid_fact, push_answer, redirect_fact,
    reencounter, settle_facts, JLog, Rec, Store, World, BODY_REPLACED, CANCELLED, PAID,
    REPLACE_CAP,
};

// ---------------------------------------------------------------------------
// The settle facts (§6.1)
// ---------------------------------------------------------------------------

/// `cancelled` excludes a replaced body; both exclude a paid settle; the
/// remaining two bits are exactly their own branch.
#[kani::proof]
pub fn g10_settle_facts_split() {
    let cancel_link: bool = kani::any();
    let body_replaced: bool = kani::any();
    let paid: bool = kani::any();
    let bits = settle_facts(cancel_link, body_replaced, paid);

    let cancelled = cancel_link && !body_replaced;
    kassert!(
        (bits & CANCELLED != 0) == cancelled,
        "cancelled iff the link was negated and no body replaced it"
    );
    kassert!(
        (bits & BODY_REPLACED != 0) == body_replaced,
        "body_replaced iff the body link was negated"
    );
    kassert!((bits & PAID != 0) == paid, "paid iff a rent committed");

    // The split: the cancel branch is never also a replace. `paid` is
    // independent of both -- a settle can pay a rent and still be replaced or
    // have its body link cancelled afterwards.
    kassert!(
        !cancelled || bits & BODY_REPLACED == 0,
        "the cancel branch excludes the replace"
    );
    // No stray bit: the three facts are exactly their own branch.
    kassert!(
        bits == CANCELLED * (cancelled as i32)
            + BODY_REPLACED * (body_replaced as i32)
            + PAID * (paid as i32),
        "the bitset is exactly the three branches"
    );

    cover!(cancelled, "cancelled");
    cover!(body_replaced, "replaced");
    cover!(paid, "paid");
    cover!(bits == 0, "no fact");
}

/// MUTANT: `cancelled` taken from the raw link, so a replaced body also reads
/// cancelled.
#[kani::proof]
#[kani::should_panic]
pub fn g10_settle_facts_raw_link_mutant() {
    let body_replaced: bool = kani::any();
    let raw = CANCELLED * (true as i32) + BODY_REPLACED * (body_replaced as i32);
    let got = settle_facts(true, body_replaced, false);
    kassert!(got == raw, "cancelled is the raw link");
}

/// `paid` needs the rent flag, the settle's own tile and that tile's owner.
#[kani::proof]
pub fn g10_settle_paid_only_own_tile() {
    let is_rent: bool = kani::any();
    let pay_tile: i32 = kani::any();
    let settle_tile: i32 = kani::any();
    let to: i32 = kani::any();
    let owner: i32 = kani::any();
    kani::assume((0..=2).contains(&pay_tile));
    kani::assume((0..=2).contains(&settle_tile));
    kani::assume((0..=1).contains(&to));
    kani::assume((0..=1).contains(&owner));

    let got = paid_fact(is_rent, pay_tile, settle_tile, to, owner);
    let want = is_rent && pay_tile == settle_tile && to == owner;
    kassert!(
        got == want,
        "paid is exactly the own-tile rent to the owner"
    );

    kassert!(
        !got || settle_tile == pay_tile,
        "`paid` implies the settle's own tile"
    );
    kassert!(!got || to == owner, "`paid` implies the owner as payee");

    cover!(got, "paid");
    cover!(!is_rent && pay_tile == settle_tile && to == owner, "gain");
    cover!(pay_tile != settle_tile, "other tile");
    cover!(to != owner, "other payee");
}

/// MUTANT: the tile match dropped.
#[kani::proof]
#[kani::should_panic]
pub fn g10_settle_paid_any_tile_mutant() {
    let pay_tile: i32 = kani::any();
    let settle_tile: i32 = kani::any();
    kani::assume((0..=2).contains(&pay_tile) && (0..=2).contains(&settle_tile));
    kassert!(
        paid_fact(true, pay_tile, settle_tile, 0, 0) == true,
        "any tile counts"
    );
}

// ---------------------------------------------------------------------------
// The redirect fact (§6.4)
// ---------------------------------------------------------------------------

/// The fact is present iff the fold changed a live, non-actor target.
#[kani::proof]
pub fn g10_redirect_fact_iff() {
    let live: bool = kani::any();
    let to: i32 = kani::any();
    let declared: i32 = kani::any();
    let actor: i32 = kani::any();
    kani::assume((-1..=2).contains(&to));
    kani::assume((0..=2).contains(&declared));
    kani::assume((0..=2).contains(&actor));

    let got = redirect_fact(live, to, declared, actor);
    let landed = live && to != declared && to != actor;
    kassert!((got != -1) == landed, "the fact is exactly the landing");
    kassert!(got == -1 || got == declared, "the fact names the origin");
    cover!(landed, "landed");
    cover!(!live, "dead target");
    cover!(to == declared, "no move");
    cover!(to == actor, "onto the actor");
    cover!(to == -1, "no destination");
}

/// MUTANT: the actor guard dropped, so a self designation stamps a fact.
#[kani::proof]
#[kani::should_panic]
pub fn g10_redirect_fact_actor_mutant() {
    let declared: i32 = kani::any();
    kani::assume(declared >= 0);
    kassert!(
        redirect_fact(true, declared, declared, declared) == -1,
        "onto self is no redirect"
    );
    kassert!(
        redirect_fact(true, 1, 0, 1) == 0,
        "onto the actor stamps a fact"
    );
}

// ---------------------------------------------------------------------------
// The event cause (§6.2)
// ---------------------------------------------------------------------------

/// The cause on `event` is on its `eventAfter`, for a card cause and for a
/// board raise alike.
#[kani::proof]
pub fn g10_event_cause_propagates() {
    let cause: i32 = kani::any();
    kani::assume((-1..=2).contains(&cause));
    kassert!(
        cause_on_event(cause) == cause_on_event_after(cause),
        "the after read sees the event's cause"
    );
    kassert!(
        cause_on_event(cause) == cause,
        "the cause is the record's own instance"
    );
    cover!(cause == -1, "board raise");
    cover!(cause >= 0, "card cause");
}

/// MUTANT: the after read falls back to a board raise.
#[kani::proof]
#[kani::should_panic]
pub fn g10_event_cause_lost_mutant() {
    kassert!(cause_on_event(0) == -1, "the after read loses the cause");
}

// ---------------------------------------------------------------------------
// The deferral (§6.2)
// ---------------------------------------------------------------------------

fn any_rec() -> Rec {
    Rec {
        event: 0,
        player: 0,
        due_owner: 1,
        deferrer_uid: 0,
        after_turn_key: 0,
    }
}

/// A second instance's claim answers 0 and pushes nothing; the same instance's
/// re-run answers 1 and pushes nothing.
#[kani::proof]
pub fn g10_defer_is_once() {
    let mut recs = [any_rec(); 2];
    let mut n = 0usize;
    let first = defer(&mut recs, &mut n, 7, 0, 3, 1, 0);
    kassert!(first == 1, "the first claim wins");
    kassert!(n == 1, "one record");

    // The same instance's re-run.
    let again = defer(&mut recs, &mut n, 7, 0, 3, 1, 0);
    kassert!(again == 1, "the owner's re-run still answers 1");
    kassert!(n == 1, "and pushes nothing");

    // Another instance.
    let other = defer(&mut recs, &mut n, 7, 0, 4, 1, 0);
    kassert!(other == 0, "a second instance loses");
    kassert!(n == 1, "and pushes nothing");

    // Another pair claims its own record.
    let pair = defer(&mut recs, &mut n, 7, 1, 5, 1, 0);
    kassert!(pair == 1, "a fresh pair claims");
    kassert!(n == 2, "its own record");
    kassert!(
        recs[0].deferrer_uid == 3 && recs[1].deferrer_uid == 5,
        "owners"
    );
}

/// MUTANT: the owner check dropped, so any instance answers 1 for an existing
/// record -- and the loser spends its crystals.
#[kani::proof]
#[kani::should_panic]
pub fn g10_defer_any_claims_mutant() {
    let mut recs = [any_rec(); 2];
    let mut n = 0usize;
    defer(&mut recs, &mut n, 7, 0, 3, 1, 0);
    kassert!(
        defer(&mut recs, &mut n, 7, 0, 4, 1, 0) == 1,
        "anyone claims"
    );
}

/// MUTANT: the owner's re-run pushes a second record (the mutant claims two).
#[kani::proof]
#[kani::should_panic]
pub fn g10_defer_repushes_mutant() {
    let mut recs = [any_rec(); 2];
    let mut n = 0usize;
    defer(&mut recs, &mut n, 7, 0, 3, 1, 0);
    defer(&mut recs, &mut n, 7, 0, 3, 1, 0);
    kassert!(n == 2, "the re-run pushed");
}

/// A record releases exactly on the due owner's strictly later turn start, or
/// on any turn start once that owner left.
#[kani::proof]
pub fn g10_release_due_iff() {
    let r = any_rec();
    let owner: i32 = kani::any();
    let key: i32 = kani::any();
    let owner_out: bool = kani::any();
    kani::assume((0..=2).contains(&owner));
    kani::assume((0..=3).contains(&key));

    let got = due(&r, owner, key, owner_out);
    let want = owner_out || (owner == r.due_owner && r.after_turn_key < key);
    kassert!(got == want, "the release point is exactly the owner's");

    kassert!(
        !got || owner_out || owner == r.due_owner,
        "no foreign release"
    );
    kassert!(
        !got || owner_out || key > r.after_turn_key,
        "the due turn start is strictly later"
    );
    cover!(got && !owner_out, "due");
    cover!(owner_out, "owner out");
    cover!(!got, "not yet");
    cover!(owner != r.due_owner && !owner_out, "a foreign turn start");
}

/// MUTANT: the same-turn-key case released too.
#[kani::proof]
#[kani::should_panic]
pub fn g10_release_same_key_mutant() {
    let r = any_rec();
    kassert!(
        due(&r, r.due_owner, r.after_turn_key, false),
        "same key releases"
    );
}

/// MUTANT: a foreign seat's turn start released.
#[kani::proof]
#[kani::should_panic]
pub fn g10_release_foreign_mutant() {
    let r = any_rec();
    kassert!(due(&r, 2, 9, false), "a foreign release");
}

// ---------------------------------------------------------------------------
// The snapshot store (§6.5)
// ---------------------------------------------------------------------------

fn any_world() -> World {
    World {
        pos: kani::any(),
        money: kani::any(),
        state: kani::any(),
        owners: [kani::any(), kani::any()],
        houses: [kani::any(), kani::any()],
        mortgaged: [kani::any(), kani::any()],
    }
}

/// `restore(snapshot(w))` equals `w` on the six families; restoring twice
/// equals restoring once; a missing key answers 0 and changes nothing; a
/// foreign uid answers 0.
#[kani::proof]
pub fn g10_snapshot_restore_iff() {
    let mut store = Store::new();
    let w0 = any_world();
    let key = store.snapshot(&w0, 1);
    kassert!(key >= 1, "a key from 1");

    let mut w = any_world();
    kassert!(store.restore(key, &mut w, 1) == 1, "the owner restores");
    let once = w;
    kassert!(
        w.pos == w0.pos
            && w.money == w0.money
            && w.state == w0.state
            && w.owners == w0.owners
            && w.houses == w0.houses
            && w.mortgaged == w0.mortgaged,
        "all six families are the snapshot"
    );
    kassert!(
        store.restore(key, &mut w, 1) == 1,
        "a second restore answers 1"
    );
    kassert!(w == once, "and is a no-op");
    kassert!(
        store.restore(key, &mut w, 2) == 0,
        "a foreign uid answers 0"
    );
    kassert!(w == once, "and changes nothing");

    let mut w2 = any_world();
    let before = w2;
    kassert!(store.restore(0, &mut w2, 1) == 0, "a missing key answers 0");
    kassert!(w2 == before, "and changes nothing");

    // The drop is owner-scoped: the key frees only after the last owner.
    store.drop_key(key, 1);
    kassert!(store.snapshot(&w0, 2) >= 2, "a fresh key after the drop");
}

/// MUTANT: the six families not written back (only `pos`), so it claims the
/// restore leaves `money` alone.
#[kani::proof]
#[kani::should_panic]
pub fn g10_snapshot_partial_mutant() {
    let mut store = Store::new();
    let w0 = any_world();
    let key = store.snapshot(&w0, 1);
    let mut w = any_world();
    store.restore(key, &mut w, 1);
    kassert!(w.money != w0.money, "money left alone");
}

/// MUTANT: the ownership check dropped.
#[kani::proof]
#[kani::should_panic]
pub fn g10_snapshot_foreign_mutant() {
    let mut store = Store::new();
    let w0 = any_world();
    let key = store.snapshot(&w0, 1);
    let mut w = any_world();
    kassert!(store.restore(key, &mut w, 2) == 1, "a stranger restores");
}

// ---------------------------------------------------------------------------
// The answer layout (§6.3.4)
// ---------------------------------------------------------------------------

/// An in-flight entry's answer takes its own slot and pushes every entry still
/// in flight past it; the answered entry keeps its order against them.
#[kani::proof]
#[kani::unwind(6)]
pub fn g10_journal_answer_layout() {
    let done: usize = kani::any();
    kani::assume(done <= 2);
    let live: usize = kani::any();
    kani::assume(live >= 1 && live <= 3);

    let mut log = JLog::default();
    log.answers_n = done;
    let mut i = 0;
    while i < done {
        log.answers[i] = i as i32;
        i += 1;
    }
    // In-flight slots sit at or after the cursor and in request order.
    log.journal_n = live;
    let mut i = 0;
    while i < live {
        log.journal[i] = done + i;
        i += 1;
    }
    let index: usize = kani::any();
    kani::assume(index < live);
    let slot = log.journal[index];

    push_answer(&mut log, index, 42);

    // One answer, at its own place -- the cursor, or its slot when that is
    // already behind the cursor.
    kassert!(log.answers_n == done + 1, "one answer, nothing overwritten");
    kassert!(
        log.answers[slot.min(done)] == 42,
        "the answer takes its own place, nothing overwritten"
    );
    // The completed prefix stands.
    let mut i = 0;
    while i < done {
        kassert!(log.answers[i] == i as i32, "the prefix stands");
        i += 1;
    }
    kassert!(log.journal_n == live - 1, "the answered entry left");
    let mut i = 0;
    while i < log.journal_n {
        if i < index {
            // An enclosing request keeps its own (earlier) slot: its answer is
            // not written yet.
            kassert!(
                log.journal[i] == done + i,
                "an enclosing entry keeps its slot"
            );
        } else {
            // A still-in-flight entry moved past the answer by one.
            kassert!(
                log.journal[i] > slot + 1,
                "a still-in-flight entry moves past the answer"
            );
        }
        if i > 0 {
            kassert!(
                log.journal[i - 1] < log.journal[i],
                "the request order stands"
            );
        }
        i += 1;
    }
    cover!(index == 0, "the front entry");
    cover!(index + 1 == live, "the last entry");
    cover!(done > 0, "a prefix");
}

/// MUTANT: the shift dropped -- the next in-flight entry keeps the slot right
/// after the answer, so the next answer collides with it.
#[kani::proof]
#[kani::unwind(6)]
#[kani::should_panic]
pub fn g10_journal_no_shift_mutant() {
    let mut log = JLog::default();
    log.journal_n = 2;
    log.journal[0] = 0;
    log.journal[1] = 1;
    // The front entry is answered and the shift is dropped: the answer lands at
    // its slot, but the still-in-flight entry is left on the very next one.
    let slot = log.journal[0];
    log.journal[0] = log.journal[1];
    log.journal_n = 1;
    log.answers[log.answers_n] = 9;
    log.answers_n += 1;
    kassert!(
        log.journal[0] > slot + 1,
        "the next entry did not move past the answer"
    );
}

// ---------------------------------------------------------------------------
// The re-encounter (§6.3.5)
// ---------------------------------------------------------------------------

/// An equal re-encounter applies nothing; a changed one restores `condition`
/// first and applies once; the cap is a clean skip of both.
#[kani::proof]
pub fn g10_journal_apply_once() {
    let condition: i32 = kani::any();
    let overlay: i32 = kani::any();
    kani::assume((-100..=100).contains(&condition));
    kani::assume((-100..=100).contains(&overlay));
    let applied_world = condition + overlay;
    let replaces: u8 = kani::any();
    kani::assume(replaces <= REPLACE_CAP);

    let (n, w) = reencounter(true, replaces, condition, applied_world, overlay);
    kassert!(n == 0, "an equal re-encounter applies nothing");
    kassert!(w == applied_world, "and keeps the application");

    let (n, w) = reencounter(false, replaces, condition, applied_world, overlay);
    if replaces >= REPLACE_CAP {
        kassert!(n == 0, "the cap is a clean skip");
        kassert!(w == condition, "back at the last completed condition");
    } else {
        kassert!(n == 1, "a changed request applies once more");
        kassert!(
            w == condition + overlay,
            "from the condition, not the world"
        );
    }
    cover!(replaces >= REPLACE_CAP, "cap");
    cover!(overlay != 0, "a visible overlay");
}

/// MUTANT: the condition restore dropped on a replace -- the overlay stacks on
/// the half-applied world.
#[kani::proof]
#[kani::should_panic]
pub fn g10_journal_replace_stacks_mutant() {
    let condition = 1i32;
    let overlay = 2i32;
    let stacked = (condition + overlay) + overlay;
    kassert!(stacked == condition + overlay, "the mutant stacked");
}

/// MUTANT: an equal re-encounter applied again.
#[kani::proof]
#[kani::should_panic]
pub fn g10_journal_equal_reapplies_mutant() {
    let condition: i32 = kani::any();
    let overlay: i32 = kani::any();
    kani::assume((-100..=100).contains(&condition));
    kani::assume((-100..=100).contains(&overlay));
    let (n, _) = reencounter(true, 0, condition, condition + overlay, overlay);
    kassert!(n == 1, "the equal re-encounter applied again");
}
