//! G06 counteraction-contract, join-report and history models
//! (`docs/rulebook/spec/abi/G06-counteraction-and-join.md` §9.4).
//!
//! The card bodies stay PROTOCOL-LEVEL (window / chain / suspension). What is
//! modelled here is the pure decision each body reads:
//!
//! * [`kind_reaches`] / [`contract_admits`] — the offer gate. Production is
//!   `Ruleset::counteract_entry` → `first_admitting_entry` → `contract_admits`
//!   (`crates/game-rules/src/host.rs`), over the manifest entry's `triggers` /
//!   `all_kinds` and the answered link's subject / payment.
//! * [`JoinReport`] — the opener-side report. Production is
//!   `hostfns::join_joiner_count` / `join_joiner` / `join_joiner_shielded` /
//!   `join_designation` over `world::TriggerJoin`; the indexed readers walk
//!   `declared` and index `seats` / `used` / `designated` through it.
//! * [`countered`] — `hostfns::countered_by_this_turn`: records whose
//!   `countered` is the seat, declarers deduped in declaration order.
//! * [`pending_negation`] — the stamped answer effect bits on a link,
//!   counting only `NEGATES`.
//! * [`empty_links_admits`] — an empty trigger list means every non-hook-only
//!   kind (`kind_flags::all_non_hook_only`, `ManifestOn.all_kinds`).
//! * [`settle_face`] — `propagate_face` (`wasm_rules.rs`): an answer writes
//!   the face it computed to the roll root, and the next (outer) answer reads
//!   that write, so the settled result is `outer(inner(face))`.

use card_sdk::abi::{AnswerEffect, AnswerPayShape, NumericShape};

/// The `Effect` kind a `designates` link substitutes in the offer (`TriggerKind::Effect`).
pub const EFFECT: &str = "effect";
/// Seats a join round is modelled over (the §7 four-player model).
pub const MAX_SEATS: usize = 4;
/// Declarations a turn's counteraction history is modelled over (four seats,
/// one declaration each -- the shortest list that can exercise dedup, order and
/// a foreign-seat record together).
pub const MAX_RECORDS: usize = 4;

/// Does an entry's kind list reach the answered `kind`?
///
/// `all_kinds` (empty links, design G06 §6.1) reaches everything; otherwise
/// the list must name the kind, or (`designates`) the substitution `Effect`.
/// Production tries the direct kind first and falls back to `Effect`, so the
/// answer is the disjunction.
pub fn kind_reaches(links: &[&str], all_kinds: bool, kind: &str, designates: bool) -> bool {
    let hit = |k: &str| all_kinds || links.iter().any(|s| *s == k);
    hit(kind) || (designates && hit(EFFECT))
}

/// Does the entry's answer contract admit the answered link?
///
/// `numeric = NumberInterval` needs a subject whose card carries
/// `prop::RANGE_CARD` (the answered link's card, never the counter's);
/// `pay_shape = Whole` needs a payment that is not a split leg
/// (`total_stage`). `Any` admits. Production: `Ruleset::contract_admits`.
pub fn contract_admits(
    numeric: NumericShape,
    pay_shape: AnswerPayShape,
    has_subject: bool,
    subject_is_range_card: bool,
    has_pay: bool,
    pay_total_stage: bool,
) -> bool {
    if numeric == NumericShape::NumberInterval && !(has_subject && subject_is_range_card) {
        return false;
    }
    if pay_shape == AnswerPayShape::Whole && !(has_pay && pay_total_stage) {
        return false;
    }
    true
}

/// The full offer decision: a reached kind whose contract admits.
pub fn offer(
    links: &[&str],
    all_kinds: bool,
    kind: &str,
    designates: bool,
    numeric: NumericShape,
    pay_shape: AnswerPayShape,
    has_subject: bool,
    subject_is_range_card: bool,
    has_pay: bool,
    pay_total_stage: bool,
) -> bool {
    kind_reaches(links, all_kinds, kind, designates)
        && contract_admits(
            numeric,
            pay_shape,
            has_subject,
            subject_is_range_card,
            has_pay,
            pay_total_stage,
        )
}

/// One join round's report, as the opener reads it back.
#[derive(Clone, Copy)]
pub struct JoinReport {
    /// The participating seats, seat order.
    pub seats: [i32; MAX_SEATS],
    /// Did the seat at this index declare?
    pub declared: [bool; MAX_SEATS],
    /// Did its declaration land (call `shield`)?
    pub used: [bool; MAX_SEATS],
    /// The seat its declaration designated, `-1` for none.
    pub designated: [i32; MAX_SEATS],
    /// Live entries (the seats list's length).
    pub n: usize,
}

impl JoinReport {
    /// The declared count (`hostfns::join_joiner_count`): the fallback bar.
    /// Only the live entries count -- past `n` the array is padding.
    pub fn joiner_count(&self) -> i32 {
        self.declared[..self.n].iter().filter(|&&d| d).count() as i32
    }

    /// The storage index of the `i`-th declarer in declaration order, or
    /// `None` (`hostfns::joiner_at`). A negative `i` names nothing.
    pub fn joiner_at(&self, i: i32) -> Option<usize> {
        if i < 0 {
            return None;
        }
        let mut seen = 0usize;
        for k in 0..self.n {
            if self.declared[k] {
                if seen == i as usize {
                    return Some(k);
                }
                seen += 1;
            }
        }
        None
    }

    /// `hostfns::join_joiner`: the seat, `-1` when there is no such declarer.
    pub fn joiner(&self, i: i32) -> i32 {
        self.joiner_at(i).map(|k| self.seats[k]).unwrap_or(-1)
    }

    /// `hostfns::join_joiner_shielded`: `1` only for a declarer that landed.
    pub fn joiner_shielded(&self, i: i32) -> i32 {
        match self.joiner_at(i) {
            Some(k) => self.used[k] as i32,
            None => 0,
        }
    }

    /// `hostfns::join_designation`: the designated seat, `-1` when none.
    pub fn designation(&self, i: i32) -> i32 {
        self.joiner_at(i)
            .and_then(|k| {
                let d = self.designated[k];
                (d >= 0).then_some(d)
            })
            .unwrap_or(-1)
    }
}

/// One counteraction declaration in the turn's history.
#[derive(Clone, Copy)]
pub struct Record {
    pub declarer: i32,
    pub countered: i32,
}

/// `hostfns::countered_by_this_turn`: the distinct declarers against `seat`, in
/// declaration order, appended to `out`; answers the count.
pub fn countered(records: &[Record], seat: i32, out: &mut [i32; MAX_RECORDS]) -> usize {
    let mut n = 0usize;
    for r in records {
        if r.countered == seat && !out[..n].contains(&r.declarer) {
            out[n] = r.declarer;
            n += 1;
        }
    }
    n
}

/// The number of declared, unresolved answers that declare `NEGATES`.
pub fn pending_negation(declared: &[bool], effect: &[AnswerEffect]) -> i32 {
    let mut n = 0;
    for (k, &d) in declared.iter().enumerate() {
        if d && effect[k].contains(AnswerEffect::NEGATES) {
            n += 1;
        }
    }
    n
}

/// Does a kind with trigger list `links` get admitted? An empty list is every
/// non-hook-only kind (`all_kinds` expansion), never a never-match.
pub fn empty_links_admits(links_len: usize, hook_only: bool) -> bool {
    links_len == 0 || !hook_only
}

/// One answer's face write, as a nameable transform.
#[derive(Clone, Copy)]
pub enum FaceAnswer {
    /// Add the amount.
    Add(i32),
    /// Clamp up to a floor (`max`).
    Floor(i32),
    /// Halve, rounding down.
    Half,
}

/// Apply one answer to the face it read (`propagate_face` writes the result
/// back to the roll root).
pub fn apply_face(a: FaceAnswer, face: i32) -> i32 {
    match a {
        FaceAnswer::Add(x) => face.saturating_add(x),
        FaceAnswer::Floor(x) => face.max(x),
        FaceAnswer::Half => face / 2,
    }
}

/// The settled face of two answers on one roll link: the inner (`at`) settles
/// first and writes, the outer reads that write -- `outer(inner(face))`.
pub fn settle_face(face: i32, inner: FaceAnswer, outer: FaceAnswer) -> i32 {
    apply_face(outer, apply_face(inner, face))
}
