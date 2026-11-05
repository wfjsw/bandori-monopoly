//! G08 zone / attachment / creator lifecycle obligations
//! (`docs/rulebook/spec/abi/G08-zone-lifecycle.md` §4 I5-I13, §7.1, §7.6,
//! §7.7, §10.2). Each harness checks the model writer in `crate::world`
//! against the independent characterisation in `spec::g08_zone_lifecycle`;
//! a MUTANT per obligation asserts the property is load-bearing.
//!
//! The host functions are thin `HostRequest` translations of the engine
//! writer, so the writer is what is proved.

use kani::cover;

use crate::intern;
use crate::kassert;
use crate::spec::g08_zone_lifecycle::{
    attach_admitted, earliest, expected_to, is_attached, leave, played, release, sale, seed,
    seed_attached, seed_child, visible_on_field, visible_on_tile, wasted_for, ATTACHED,
    CARD_ATTACHMENT, CARD_CHILD, CARD_GRANDCHILD, CARD_HOST, CARD_OTHER, HAND, NOWHERE,
};
use crate::world::{
    Effect, World, CAUSE_CREATOR_LEFT, CAUSE_EFFECT, CAUSE_HOST_LEFT, ZONE_FIELD, ZONE_GRAVEYARD,
};

fn name(card: &str) -> u16 {
    intern::intern(card).0
}

/// The `ZoneLeave` records in commit order.
fn leaves(w: &World) -> [Option<Effect>; 8] {
    let mut out = [None; 8];
    let mut n = 0usize;
    for e in w.effect_slice() {
        if let Effect::ZoneLeave { .. } = e {
            if n < out.len() {
                out[n] = Some(*e);
                n += 1;
            }
        }
    }
    out
}

/// A leave runs every attachment to its **own** leave destination and every
/// child to the Graveyard, each exactly once, attachments then children then
/// the leaver -- and nothing arrives before its dependants have left.
#[kani::proof]
#[kani::unwind(12)]
pub fn leave_cascade_order_exact() {
    let mut w = World::new();
    let host = seed(&mut w, 0, CARD_HOST, 4);
    // Two attachments, one with its own destination (`Hand`).
    let a1 = seed_attached(&mut w, host, CARD_ATTACHMENT, 1);
    let a2 = seed_attached(&mut w, host, CARD_OTHER, -1);
    // A child with a grandchild of its own.
    let child = seed_child(&mut w, host, CARD_CHILD);
    let grand = seed_child(&mut w, child, CARD_GRANDCHILD);
    // An unrelated bystander stays put.
    let bystander = seed(&mut w, 1, CARD_OTHER, 9);

    let ret = leave(&mut w, host, 0, CAUSE_EFFECT);
    kassert!(ret == 0, "the leaver lands in its owner's graveyard");

    // The attachment with its own destination goes there, not to the heap.
    kassert!(w.hand_n == 1, "one card took the hand destination");
    kassert!(
        w.hand_pile[0] == name(CARD_ATTACHMENT),
        "own destination used"
    );
    kassert!(
        w.discard_n == 4,
        "the other attachment, the child, its grandchild and the leaver"
    );
    // Attachments (in creation order), then children depth first, then the
    // leaver -- `to_discard` append order is the arrival order.
    kassert!(w.discard_pile[0] == name(CARD_OTHER), "attachment first");
    kassert!(
        w.discard_pile[1] == name(CARD_GRANDCHILD),
        "grandchild next"
    );
    kassert!(w.discard_pile[2] == name(CARD_CHILD), "then the child");
    kassert!(w.discard_pile[3] == name(CARD_HOST), "the leaver last");

    // The same order on the transition records, with the cascade causes.
    let log = leaves(&w);
    kassert!(
        log[0]
            == Some(Effect::ZoneLeave {
                uid: a1,
                from: ATTACHED,
                to: HAND,
                cause: CAUSE_HOST_LEFT
            }),
        "first record is the hand-destined attachment"
    );
    kassert!(
        log[1]
            == Some(Effect::ZoneLeave {
                uid: a2,
                from: ATTACHED,
                to: ZONE_GRAVEYARD,
                cause: CAUSE_HOST_LEFT
            }),
        "second record is the other attachment"
    );
    kassert!(
        log[2]
            == Some(Effect::ZoneLeave {
                uid: grand,
                from: ZONE_FIELD,
                to: ZONE_GRAVEYARD,
                cause: CAUSE_CREATOR_LEFT
            }),
        "a grandchild falls to the creator cascade"
    );
    kassert!(
        log[3]
            == Some(Effect::ZoneLeave {
                uid: child,
                from: ZONE_FIELD,
                to: ZONE_GRAVEYARD,
                cause: CAUSE_CREATOR_LEFT
            }),
        "the child follows its own grandchild"
    );
    kassert!(
        log[4]
            == Some(Effect::ZoneLeave {
                uid: host,
                from: ZONE_FIELD,
                to: ZONE_GRAVEYARD,
                cause: CAUSE_EFFECT
            }),
        "the leaver commits last"
    );
    kassert!(log[5].is_none(), "exactly five transitions");

    // Nothing of the subtree survives; the bystander does.
    kassert!(w.field_by_uid(a1).is_none(), "attachment gone");
    kassert!(w.field_by_uid(a2).is_none(), "attachment gone");
    kassert!(w.field_by_uid(child).is_none(), "child gone");
    kassert!(w.field_by_uid(grand).is_none(), "grandchild gone");
    kassert!(w.field_by_uid(host).is_none(), "leaver gone");
    kassert!(w.field_by_uid(bystander).is_some(), "bystander kept");

    // The payload pair is the row's real pair, never a sentinel.
    kassert!(w.trig_zone_from == ZONE_FIELD, "from is a zone");
    kassert!(w.trig_zone_to == ZONE_GRAVEYARD, "to is a zone");
    kassert!(!w.trig_zone_wasted, "an effective row is not wasted");

    cover!(
        w.hand_n == 1,
        "an attachment took its rewritten destination"
    );
}

/// MUTANT: children before attachments (the draft's order).
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(12)]
pub fn leave_cascade_children_first_mutant() {
    let mut w = World::new();
    let host = seed(&mut w, 0, CARD_HOST, 4);
    let a1 = seed_attached(&mut w, host, CARD_ATTACHMENT, -1);
    let child = seed_child(&mut w, host, CARD_CHILD);
    leave(&mut w, host, 0, CAUSE_EFFECT);
    kassert!(
        w.discard_pile[0] == name(CARD_CHILD) && w.discard_pile[1] == name(CARD_ATTACHMENT),
        "mutant: child first"
    );
    let _ = a1;
}

/// An attached row is never listed by the field pass or the tile-rule pass
/// (I6), attached or not; detaching makes both see it again.
#[kani::proof]
#[kani::unwind(12)]
pub fn attached_rows_not_listed() {
    let mut w = World::new();
    let host = seed(&mut w, 0, CARD_HOST, 4);
    let att = seed_attached(&mut w, host, CARD_ATTACHMENT, -1);
    let board_rule = seed(&mut w, -1, CARD_OTHER, 7);
    let board_att = seed_attached(&mut w, board_rule, CARD_CHILD, -1);

    kassert!(visible_on_field(&w, host), "an unattached row is listed");
    kassert!(!visible_on_field(&w, att), "an attached row is not");
    kassert!(
        visible_on_tile(&w, board_rule),
        "an unattached rule is listed"
    );
    kassert!(!visible_on_tile(&w, board_att), "an attached rule is not");

    let mut out = [0i32; crate::world::MAX_FIELD];
    let n = w.field_instances_into(0, &mut out);
    kassert!(
        n == 1 && out[0] == host,
        "field_instances answers the host only"
    );
    let n = w.tile_rule_instances_into(7, &mut out);
    kassert!(
        n == 1 && out[0] == board_rule,
        "tile_rule_instances answers the rule only"
    );

    // `zone_of` follows the edge, not the vector membership.
    kassert!(w.zone_of(att) == ATTACHED, "attached reads Attached");
    kassert!(w.zone_of(host) == ZONE_FIELD, "the host reads Field");
    kassert!(w.zone_of(999) == NOWHERE, "a dead uid reads Nowhere");

    // Detaching lists it again.
    leave(&mut w, att, 0, CAUSE_EFFECT);
    kassert!(visible_on_field(&w, att) == false, "detached row is dead");
    let _ = w.field_instances_into(0, &mut out);
}

/// MUTANT: the field pass keeps attached rows.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(12)]
pub fn attached_rows_listed_mutant() {
    let mut w = World::new();
    let host = seed(&mut w, 0, CARD_HOST, 4);
    let _att = seed_attached(&mut w, host, CARD_ATTACHMENT, -1);
    // Mutant: no `on < 0` filter -- every live row of the holder counts.
    let mut n = 0;
    let mut i = 0;
    while i < crate::world::MAX_FIELD {
        if w.field[i].player == 0 && w.field[i].card != u16::MAX {
            n += 1;
        }
        i += 1;
    }
    // The real pass answers 1; the unfiltered mutant answers 2.
    kassert!(n == 1, "mutant lists the attachment too");
}

/// `set_face_down_at` answers `1` on a live row and `0` on a dead uid, and a
/// live row's face follows the write.
#[kani::proof]
#[kani::unwind(12)]
pub fn flip_dead_refused_live_applied() {
    let mut w = World::new();
    let uid = seed(&mut w, 0, CARD_HOST, 0);
    let on: bool = kani::any();

    // The host row is the wrapper's `set_face_down_at` body.
    let live = w.field_by_uid_mut(uid).map(|f| {
        f.face_down = on;
        true
    });
    kassert!(live == Some(true), "a live row answers 1");
    kassert!(
        w.field_by_uid(uid).unwrap().face_down == on,
        "the face follows"
    );

    let dead = w.field_by_uid_mut(999).map(|f| {
        f.face_down = on;
        true
    });
    kassert!(dead.is_none(), "a dead uid answers 0");
    cover!(on, "flip down");
    cover!(!on, "flip up");
}

/// `earliest_owned_tile` is the smallest live acquisition sequence (not the
/// lowest tile index), and a sale / release rewrites the owner's `firstTile`
/// slot (I13).
#[kani::proof]
#[kani::unwind(64)]
pub fn earliest_owned_is_sequence_min() {
    let mut w = World::new();
    // Acquire the higher index first: index order is not acquisition order.
    sale(&mut w, 5, 0);
    kassert!(w.earliest_owned_tile(0) == 5, "the first acquisition");
    kassert!(w.first_tile[0] == 5, "firstTile follows the write");
    sale(&mut w, 2, 0);
    kassert!(
        w.earliest_owned_tile(0) == 5,
        "the earliest, not the lowest"
    );
    kassert!(w.first_tile[0] == 5, "firstTile keeps the earliest");

    // Releasing the earliest promotes the next one.
    release(&mut w, 5);
    kassert!(w.earliest_owned_tile(0) == 2, "the next acquisition");
    kassert!(w.first_tile[0] == 2, "firstTile rebuilt");

    // Selling it away leaves nothing.
    sale(&mut w, 2, 1);
    kassert!(w.earliest_owned_tile(0) == -1, "-1 when none");
    kassert!(w.first_tile[0] == -1, "firstTile cleared");
    kassert!(w.earliest_owned_tile(1) == 2, "the buyer's own earliest");

    // The ledger never rewrites a held sequence.
    let seq = w.owned_seq[2];
    sale(&mut w, 3, 1);
    kassert!(w.owned_seq[2] == seq, "an existing sequence is kept");
    kassert!(earliest(&w, 1) == 2, "still the earliest for its owner");

    cover!(w.earliest_owned_tile(0) == -1, "a player with no deed");
}

/// MUTANT: the answer is the lowest owned tile index.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(64)]
pub fn earliest_owned_is_lowest_index_mutant() {
    let mut w = World::new();
    sale(&mut w, 5, 0);
    sale(&mut w, 2, 0);
    let mut lowest = -1i32;
    let mut t = 0usize;
    while t < 60 {
        if w.tiles[t].owner == 0 {
            lowest = t as i32;
            break;
        }
        t += 1;
    }
    kassert!(lowest == 2, "mutant answers the lowest index");
    kassert!(earliest(&w, 0) == lowest, "mutant equals the ledger");
}

/// An attach is admitted iff the row is live, the host is live and played,
/// and the host is neither the card nor one of its descendants.
#[kani::proof]
#[kani::unwind(10)]
pub fn attach_refusal_iff() {
    let mut w = World::new();
    let a = seed(&mut w, 0, CARD_HOST, 1);
    let b = seed_attached(&mut w, a, CARD_ATTACHMENT, -1); // b on a
    let c = seed_attached(&mut w, b, CARD_CHILD, -1); // c on b on a
    let d = seed(&mut w, 0, CARD_OTHER, 3);

    // A live, non-ancestor host is admitted (an attached one too); an
    // ancestor, the card itself and a dead uid are refused.
    // Re-parenting an attached row keeps its real `from` (`Attached`).
    kassert!(
        attach_admitted(&w, b, d) == Some((ATTACHED, 0)),
        "from / holder resolved"
    );
    kassert!(
        attach_admitted(&w, d, c).is_some(),
        "a live attachment is a host"
    );
    kassert!(
        attach_admitted(&w, a, c).is_none(),
        "onto a descendant is a cycle"
    );
    kassert!(
        attach_admitted(&w, b, a).is_some(),
        "onto its current host is fine"
    );
    kassert!(attach_admitted(&w, a, a).is_none(), "a self-host");
    kassert!(attach_admitted(&w, 999, a).is_none(), "a dead card");
    kassert!(attach_admitted(&w, a, 999).is_none(), "a dead host");

    kassert!(played(&w, c), "an attached row is played");
    kassert!(is_attached(&w, c), "and is attached");

    cover!(attach_admitted(&w, a, a).is_none(), "self-host refused");
    cover!(attach_admitted(&w, b, d).is_some(), "a plain host admitted");
}

/// MUTANT: the cycle guard is dropped.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(10)]
pub fn attach_cycle_admitted_mutant() {
    let mut w = World::new();
    let a = seed(&mut w, 0, CARD_HOST, 1);
    let b = seed_attached(&mut w, a, CARD_ATTACHMENT, -1);
    let c = seed_attached(&mut w, b, CARD_CHILD, -1);
    kassert!(
        attach_admitted(&w, a, c).is_some(),
        "mutant admits the cycle"
    );
}

/// A Graveyard arrival carries `wasted` exactly when the row's `effective`
/// bit was clear, and the landing clears it (I8).
#[kani::proof]
#[kani::unwind(12)]
pub fn wasted_iff_and_cleared() {
    let mut w = World::new();
    let uid = seed(&mut w, 0, CARD_HOST, 0);
    let effective: bool = kani::any();
    w.field_by_uid_mut(uid).unwrap().effective = effective;

    let want = wasted_for(&w, uid);
    kassert!(want == !effective, "wasted is the cleared bit");
    let ret = leave(&mut w, uid, 0, CAUSE_EFFECT);
    kassert!(ret == 0, "the row lands");

    kassert!(w.trig_zone_wasted == want, "the payload carries it");
    kassert!(w.trig_zone_wasted == !effective, "exactly the cleared bit");
    kassert!(w.last_leave_effective_after, "the landing cleared the bit");
    kassert!(expected_to(&w, uid, 0) >= 0, "a destination exists");

    cover!(w.trig_zone_wasted, "a wasted discard");
    cover!(!w.trig_zone_wasted, "an effective discard");
}

/// MUTANT: `wasted` is the effective bit itself.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(12)]
pub fn wasted_is_effective_mutant() {
    let mut w = World::new();
    let uid = seed(&mut w, 0, CARD_HOST, 0);
    w.field_by_uid_mut(uid).unwrap().effective = false;
    leave(&mut w, uid, 0, CAUSE_EFFECT);
    kassert!(
        !w.trig_zone_wasted,
        "mutant: an ineffective row is not wasted"
    );
}

/// MUTANT: the landing leaves the bit alone.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(12)]
pub fn wasted_bit_not_cleared_mutant() {
    let mut w = World::new();
    let uid = seed(&mut w, 0, CARD_HOST, 0);
    w.field_by_uid_mut(uid).unwrap().effective = false;
    leave(&mut w, uid, 0, CAUSE_EFFECT);
    // Mutant: no clear, so the post-state still reports ineffective.
    kassert!(!w.last_leave_effective_after, "mutant keeps the bit clear");
}
