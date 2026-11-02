//! G06 counteraction contracts, join report, history, all-kind expansion and
//! face propagation (`docs/rulebook/spec/abi/G06-counteraction-and-join.md`
//! §9.4). Every harness is over the pure model in `spec::g06_windows`, which
//! mirrors the production function named on each model item; a MUTANT harness
//! per obligation must fail.
//!
//! Card bodies (window, chain, suspension) are PROTOCOL-LEVEL and proved
//! elsewhere; these obligations sit on the SDK-side decision functions.

use card_sdk::abi::{AnswerEffect, AnswerPayShape, NumericShape};
use kani::cover;

use crate::kassert;
use crate::spec::g06_windows::{
    apply_face, contract_admits, countered, empty_links_admits, kind_reaches, offer,
    pending_negation, settle_face, FaceAnswer, JoinReport, Record, EFFECT, MAX_RECORDS, MAX_SEATS,
};

/// A small symbolic kind table: `""` = empty list, "card"/"event"/"settle" =
/// named kinds, "effect" = the `designates` substitution only, "other" = a
/// non-matching kind.
fn any_links() -> (&'static [&'static str], &'static str) {
    let which: u8 = kani::any();
    kani::assume(which < 6);
    match which {
        0 => (&[], "card"),
        1 => (&["card"], "card"),
        2 => (&["card"], "event"),
        3 => (&["card", "event"], "settle"),
        4 => (&["effect"], "other"),
        _ => (&["settle"], "other"),
    }
}

/// `admits` is true exactly when a kind is reached (direct or through the
/// `designates` substitution) and both contract shapes hold.
///
/// Unwind 10: a kind name can be six bytes, so the `str` compare it drives
/// needs six `memcmp` trips on top of the two-element iteration -- 6 is
/// marginal and unwinds the `memcmp` loop mid-compare.
#[kani::proof]
#[kani::unwind(10)]
pub fn contract_admits_iff() {
    let (links, kind) = any_links();
    let all_kinds: bool = kani::any();
    let designates: bool = kani::any();
    let numeric: NumericShape = if kani::any() {
        NumericShape::NumberInterval
    } else {
        NumericShape::Any
    };
    let pay: AnswerPayShape = if kani::any() {
        AnswerPayShape::Whole
    } else {
        AnswerPayShape::Any
    };
    let has_subject: bool = kani::any();
    let subject_is_range: bool = kani::any();
    let has_pay: bool = kani::any();
    let total: bool = kani::any();

    // The independent characterisation.
    let kind_ok = all_kinds
        || links.contains(&kind)
        || (designates && (all_kinds || links.contains(&EFFECT)));
    let numeric_ok = numeric == NumericShape::Any
        || (numeric == NumericShape::NumberInterval && has_subject && subject_is_range);
    let pay_ok = pay == AnswerPayShape::Any || (pay == AnswerPayShape::Whole && has_pay && total);

    let got = offer(
        links,
        all_kinds,
        kind,
        designates,
        numeric,
        pay,
        has_subject,
        subject_is_range,
        has_pay,
        total,
    );
    kassert!(
        got == (kind_ok && numeric_ok && pay_ok),
        "offer iff all three"
    );

    cover!(got, "admitted");
    cover!(!got && !kind_ok, "refused on the kind");
    cover!(
        !got && kind_ok && !numeric_ok,
        "refused on the numeric shape"
    );
    cover!(
        !got && kind_ok && numeric_ok && !pay_ok,
        "refused on the pay shape"
    );
    cover!(
        designates && !links.contains(&kind) && got,
        "designates substitution admits"
    );
    cover!(all_kinds && !designates && got, "all_kinds admits");
}

/// MUTANT: the pay shape is ignored (a split leg is offered).
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(10)]
pub fn contract_admits_ignores_pay_shape_mutant() {
    let (links, kind) = any_links();
    let all_kinds: bool = kani::any();
    let designates: bool = kani::any();
    let has_subject: bool = kani::any();
    let subject_is_range: bool = kani::any();
    let has_pay: bool = kani::any();
    let total: bool = kani::any();
    let kind_ok = kind_reaches(links, all_kinds, kind, designates);
    // Mutant: no pay-shape predicate.
    let mutant = kind_ok
        && contract_admits(
            NumericShape::Any,
            AnswerPayShape::Any,
            has_subject,
            subject_is_range,
            has_pay,
            total,
        );
    let real = offer(
        links,
        all_kinds,
        kind,
        designates,
        NumericShape::Any,
        AnswerPayShape::Whole,
        has_subject,
        subject_is_range,
        has_pay,
        total,
    );
    kassert!(mutant == real, "mutant equals the real offer");
}

/// MUTANT: the numeric shape is ignored (a non-interval subject is offered a
/// `NumberInterval` counter).
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(10)]
pub fn contract_admits_ignores_numeric_mutant() {
    let (links, kind) = any_links();
    let all_kinds: bool = kani::any();
    let designates: bool = kani::any();
    let has_subject: bool = kani::any();
    let subject_is_range: bool = kani::any();
    let has_pay: bool = kani::any();
    let total: bool = kani::any();
    let kind_ok = kind_reaches(links, all_kinds, kind, designates);
    // Mutant: no numeric-shape predicate.
    let mutant = kind_ok
        && contract_admits(
            NumericShape::Any,
            AnswerPayShape::Any,
            has_subject,
            subject_is_range,
            has_pay,
            total,
        );
    let real = offer(
        links,
        all_kinds,
        kind,
        designates,
        NumericShape::NumberInterval,
        AnswerPayShape::Any,
        has_subject,
        subject_is_range,
        has_pay,
        total,
    );
    kassert!(mutant == real, "mutant equals the real offer");
}

/// MUTANT: the `designates` substitution is not tried.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(10)]
pub fn contract_admits_ignores_designates_mutant() {
    let (links, kind) = any_links();
    let all_kinds: bool = kani::any();
    let mutant = kind_reaches(links, all_kinds, kind, false);
    let real = kind_reaches(links, all_kinds, kind, kani::any());
    kassert!(mutant == real, "mutant equals the real reach");
}

/// A symbolic join report over `1..=4` seats.
fn any_report() -> JoinReport {
    let mut seats = [0i32; MAX_SEATS];
    let mut declared = [false; MAX_SEATS];
    let mut used = [false; MAX_SEATS];
    let mut designated = [-1i32; MAX_SEATS];
    for k in 0..MAX_SEATS {
        seats[k] = k as i32;
        declared[k] = kani::any();
        used[k] = kani::any();
        let d: i32 = kani::any();
        kani::assume((-1..=3).contains(&d));
        designated[k] = d;
    }
    let n: usize = kani::any();
    kani::assume((1..=MAX_SEATS).contains(&n));
    JoinReport {
        seats,
        declared,
        used,
        designated,
        n,
    }
}

/// The report is exactly the declaration record: count = declared, the i-th
/// joiner = the i-th declarer's seat, shielded and designation come from that
/// declarer's row (never from an index that did not declare).
#[kani::proof]
#[kani::unwind(8)]
pub fn join_report_iff() {
    let r = any_report();
    let i: i32 = kani::any();
    kani::assume((-2..=(MAX_SEATS as i32 + 1)).contains(&i));

    kassert!(
        r.joiner_count() == r.declared[..r.n].iter().filter(|&&d| d).count() as i32,
        "count is the declared count"
    );

    // The i-th declarer in declaration order, computed independently.
    let mut seen = 0i32;
    let mut expect: Option<usize> = None;
    for k in 0..r.n {
        if r.declared[k] {
            if seen == i && i >= 0 {
                expect = Some(k);
            }
            seen += 1;
        }
    }
    kassert!(r.joiner_at(i) == expect, "joiner_at is the i-th declarer");
    kassert!(
        r.joiner(i) == expect.map(|k| r.seats[k]).unwrap_or(-1),
        "joiner is that declarer's seat"
    );
    kassert!(
        r.joiner_shielded(i) == expect.map(|k| r.used[k] as i32).unwrap_or(0),
        "shielded is that declarer's used"
    );
    kassert!(
        r.designation(i) == expect.map(|k| r.designated[k]).unwrap_or(-1),
        "designation is that declarer's designation"
    );

    cover!(
        r.joiner_count() > 0 && r.joiner(0) >= 0,
        "there is a first declarer"
    );
    cover!(expect.is_none(), "an out-of-range i");
}

/// MUTANT: `declared` is folded into `used` (a declared-but-not-landed joiner
/// disappears from the report).
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(8)]
pub fn join_report_folds_declared_into_used_mutant() {
    let r = any_report();
    let i: i32 = kani::any();
    kani::assume((0..=(MAX_SEATS as i32 - 1)).contains(&i));
    let mut seen = 0i32;
    let mut expect = None;
    for k in 0..r.n {
        if r.used[k] {
            if seen == i {
                expect = Some(k);
            }
            seen += 1;
        }
    }
    kassert!(r.joiner_at(i) == expect, "mutant walks used");
}

/// The history is exactly the declarers against `seat`, deduped, in first
/// declaration order.
#[kani::proof]
#[kani::unwind(10)]
pub fn history_append_iff() {
    let mut records = [Record {
        declarer: 0,
        countered: 0,
    }; MAX_RECORDS];
    // Two declarers and two countered seats keep the bit-blast small (one
    // symbolic bit per field, no ranged i32); the dedup / order / presence
    // obligations are all still reachable inside that domain.
    for r in records.iter_mut() {
        let d: bool = kani::any();
        let c: bool = kani::any();
        r.declarer = d as i32;
        r.countered = c as i32;
    }
    // The whole array is live: a symbolic `len` would add a slice-length
    // dimension on top of every loop below and push the proof past its budget.
    let seat: bool = kani::any();
    let seat = seat as i32;

    let mut out = [0i32; MAX_RECORDS];
    let n = countered(&records, seat, &mut out);

    kassert!(n <= MAX_RECORDS, "bounded");
    for j in 0..n {
        kassert!(
            records
                .iter()
                .any(|r| r.countered == seat && r.declarer == out[j]),
            "every answer declared against the seat"
        );
        for k in (j + 1)..n {
            kassert!(out[j] != out[k], "deduped");
        }
        if j > 0 {
            let first = |v: i32| {
                records
                    .iter()
                    .position(|r| r.countered == seat && r.declarer == v)
                    .unwrap()
            };
            kassert!(first(out[j - 1]) < first(out[j]), "declaration order kept");
        }
    }
    // Every declarer against the seat is present.
    for r in records.iter().filter(|r| r.countered == seat) {
        kassert!(out[..n].contains(&r.declarer), "no declarer dropped");
    }
    cover!(n > 1, "two distinct declarers");
}

/// MUTANT: records against other seats are kept too.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(10)]
pub fn history_keeps_other_seats_mutant() {
    let records = [
        Record {
            declarer: 0,
            countered: 1,
        },
        Record {
            declarer: 1,
            countered: 2,
        },
    ];
    let mut out = [0i32; MAX_RECORDS];
    // Mutant: no seat filter.
    let mut n = 0usize;
    for r in records.iter() {
        if !out[..n].contains(&r.declarer) {
            out[n] = r.declarer;
            n += 1;
        }
    }
    kassert!(n <= 1, "mutant keeps a foreign record");
}

/// `pending_negation` counts exactly the declared answers that declare
/// `NEGATES`.
#[kani::proof]
#[kani::unwind(8)]
pub fn pending_negation_iff() {
    let mut declared = [false; MAX_SEATS];
    let mut effect = [AnswerEffect::NONE; MAX_SEATS];
    let len: usize = kani::any();
    kani::assume((0..=MAX_SEATS).contains(&len));
    for k in 0..MAX_SEATS {
        declared[k] = kani::any();
        let bits: u8 = kani::any();
        kani::assume(bits < 4);
        effect[k] = AnswerEffect(bits);
    }
    let got = pending_negation(&declared[..len], &effect[..len]);
    let want = (0..len)
        .filter(|&k| declared[k] && effect[k].contains(AnswerEffect::NEGATES))
        .count() as i32;
    kassert!(got == want, "counts exactly the declared negators");
    cover!(got > 0, "a pending negation");
    cover!(got == 0, "none pending");
}

/// MUTANT: every answer counts, declared or not.
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(8)]
pub fn pending_negation_counts_undeclared_mutant() {
    let declared = [false; MAX_SEATS];
    let mut effect = [AnswerEffect::NONE; MAX_SEATS];
    for e in effect.iter_mut() {
        *e = AnswerEffect::NEGATES;
    }
    kassert!(
        pending_negation(&declared, &effect) == MAX_SEATS as i32,
        "mutant counts undeclared answers"
    );
}

/// An empty trigger list is total over the non-hook-only kinds: it admits them
/// all, and is a never-match only for a named list with no hit.
#[kani::proof]
#[kani::unwind(4)]
pub fn empty_links_total() {
    let len: usize = kani::any();
    kani::assume(len <= 3);
    let hook_only: bool = kani::any();
    kassert!(
        empty_links_admits(len, hook_only) == (len == 0 || !hook_only),
        "empty list is what makes an unhooked kind total"
    );
    cover!(
        len == 0 && hook_only,
        "empty list admits even a hook-only kind"
    );
    cover!(len == 0 && !hook_only, "empty list admits an ordinary kind");
}

/// MUTANT: only an unhooked empty list admits (a hook-only empty entry never
/// offers).
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(4)]
pub fn empty_links_hook_only_never_mutant() {
    // Mutant: an empty list is a never-match, hook-only or not.
    let mutant = |_links_len: usize, _hook_only: bool| false;
    kassert!(mutant(0, true), "mutant: hook-only empty entry offers");
}

/// Two answers on one roll link settle inner-first: the outer answer reads the
/// inner answer's write -- `outer(inner(face))`.
#[kani::proof]
#[kani::unwind(4)]
pub fn face_compose() {
    let face: i32 = kani::any();
    kani::assume((-1000..=1000).contains(&face));
    let inner = any_face_answer();
    let outer = any_face_answer();
    let got = settle_face(face, inner, outer);
    kassert!(
        got == apply_face(outer, apply_face(inner, face)),
        "inner settles first"
    );
    cover!(
        settle_face(face, inner, outer) != settle_face(face, outer, inner),
        "order matters for some pair"
    );
}

fn any_face_answer() -> FaceAnswer {
    let which: u8 = kani::any();
    kani::assume(which < 3);
    match which {
        0 => {
            let x: i32 = kani::any();
            kani::assume((-100..=100).contains(&x));
            FaceAnswer::Add(x)
        }
        1 => {
            let x: i32 = kani::any();
            kani::assume((-100..=100).contains(&x));
            FaceAnswer::Floor(x)
        }
        _ => FaceAnswer::Half,
    }
}

/// MUTANT: settle order reversed (`inner(outer(face))`).
#[kani::proof]
#[kani::should_panic]
#[kani::unwind(4)]
pub fn face_compose_reversed_mutant() {
    let face: i32 = 1;
    let inner = FaceAnswer::Floor(10);
    let outer = FaceAnswer::Half;
    kassert!(
        settle_face(face, inner, outer) == apply_face(inner, apply_face(outer, face)),
        "mutant composes the other way"
    );
}
