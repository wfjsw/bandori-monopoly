//! Check 6 -- bystander / seat invariance.
//!
//! An uninvolved player's final state is unchanged by an interaction between
//! two others. Rotating the seat labels gives the same outcomes, apart from
//! turn order.
//!
//! Method: a **symmetric** board (same money, same tile, no deeds) so the
//! only asymmetry is who acts. Run A and B as two seats and compare the
//! bystander's personal delta across two rotations.

use std::collections::BTreeMap;

use super::{drain_first, quiet, Finding, Severity};
use crate::common::Table;
use crate::fuzz::rng::Rng;
use crate::q4::snap::{snapshot, Snap};

/// A three-player interaction on a symmetric board.
pub struct SeatRun {
    pub rule_a: String,
    pub rule_b: String,
    pub seed: u64,
    /// Bystander fields that changed between the rotated runs (or that
    /// changed at all when the bystander was not acting).
    pub diverge: Vec<String>,
}

/// Personal (non-board) state of one player, as a flat map.
fn person(s: &Snap, who: usize) -> BTreeMap<String, String> {
    let money = format!("money:P{who}");
    s.map
        .iter()
        .filter(|(k, _)| {
            k.as_str() == money
                || k.starts_with(&format!("pos:P{who}"))
                || k.starts_with(&format!("state:P{who}:"))
                || k.starts_with(&format!("token:P{who}:"))
                || k.starts_with(&format!("hand:P{who}:"))
                || k.starts_with(&format!("draw:P{who}:"))
                || k.starts_with(&format!("discard:P{who}:"))
                || k.starts_with(&format!("field:P{who}:"))
                || k.starts_with(&format!("crystals:P{who}:"))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// A symmetric 3-player board: everyone on CiRCLE, 10000 each, no deeds.
fn symmetric(seed: u64) -> Table {
    let chars = crate::common::characters();
    let chosen: Vec<String> = (0..3).map(|i| chars[i].clone()).collect();
    let refs: Vec<&str> = chosen.iter().map(String::as_str).collect();
    let mut t = Table::with_seed(&refs, seed);
    t.strip_skills();
    t.clean();
    for who in 0..3 {
        t.set_money(who, 10_000);
        t.set_pos(who, 0);
    }
    t.begin_turn(1);
    t
}

/// Activate `rule` for `who`.
fn activate(t: &mut Table, who: usize, rule: &str) {
    if rule.starts_with("skill:") {
        t.place_raw(who, rule);
        quiet(t);
        let _ = t.skill(who, rule);
    } else {
        t.give(who, &[rule]);
        quiet(t);
        let _ = t.play(who, rule);
    }
    drain_first(t).ok();
    quiet(t);
}

/// Delta of `after - before` restricted to `who`.
fn person_delta(before: &Snap, after: &Snap, who: usize) -> BTreeMap<String, String> {
    let b = person(before, who);
    let a = person(after, who);
    let mut out = BTreeMap::new();
    let keys: std::collections::BTreeSet<String> = b.keys().chain(a.keys()).cloned().collect();
    for k in keys {
        let vb = b.get(&k).map(String::as_str).unwrap_or("<absent>");
        let va = a.get(&k).map(String::as_str).unwrap_or("<absent>");
        if vb != va {
            out.insert(k, format!("{vb}->{va}"));
        }
    }
    out
}

/// Run A and B as `a_seat` / `b_seat` on a symmetric board; return the
/// bystander's personal delta.
fn bystander_delta(a_rule: &str, b_rule: &str, seed: u64, a_seat: usize, b_seat: usize) -> BTreeMap<String, String> {
    let mut t = symmetric(seed);
    quiet(&mut t);
    let faces: Vec<i32> = (0..8).map(|_| 3).collect();
    t.dice(&faces);
    quiet(&mut t);
    let c_seat = [0usize, 1, 2]
        .into_iter()
        .find(|s| *s != a_seat && *s != b_seat)
        .unwrap();
    // The bystander is not the turn player: start on A's turn.
    t.begin_turn(a_seat);
    quiet(&mut t);
    let before = snapshot(t.m.world());
    activate(&mut t, a_seat, a_rule);
    activate(&mut t, b_seat, b_rule);
    let after = snapshot(t.m.world());
    person_delta(&before, &after, c_seat)
}

/// Rotating the seat labels: (A,B,C) = (1,2,0) then (2,0,1). The bystander's
/// delta must match.
pub fn seat_once(a: &str, b: &str, seed: u64) -> SeatRun {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| seat_once_raw(a, b, seed)));
    match r {
        Ok(s) => s,
        Err(p) => {
            let what = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            SeatRun {
                rule_a: a.to_string(),
                rule_b: b.to_string(),
                seed,
                diverge: vec![format!("harness panic: {what}")],
            }
        }
    }
}

fn seat_once_raw(a: &str, b: &str, seed: u64) -> SeatRun {
    let d1 = bystander_delta(a, b, seed, 1, 2); // C = 0
    let d2 = bystander_delta(a, b, seed, 2, 0); // C = 1

    let mut diverge = Vec::new();
    let keys: std::collections::BTreeSet<String> = d1.keys().chain(d2.keys()).cloned().collect();
    for k in keys {
        // `field:` / `crystals:` paths embed the seat index; compare by the
        // tail after the seat so a rotated instance matches.
        let norm = |p: &str| -> String {
            p.replace(":P0:", ":P*:")
                .replace(":P1:", ":P*:")
                .replace(":P2:", ":P*:")
        };
        let v1 = d1
            .iter()
            .find(|(kk, _)| norm(kk) == norm(&k))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "<absent>".into());
        let v2 = d2
            .iter()
            .find(|(kk, _)| norm(kk) == norm(&k))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "<absent>".into());
        if v1 != v2 {
            diverge.push(format!("{k}: {v1} vs {v2}"));
        }
    }

    SeatRun {
        rule_a: a.to_string(),
        rule_b: b.to_string(),
        seed,
        diverge,
    }
}

/// Sampled seat runs.
pub fn seat_batch(sample: usize) -> Vec<SeatRun> {
    let pairs = super::pair::disjoint_pairs(sample.max(1));
    let mut out = Vec::new();
    for (i, (a, b)) in pairs.iter().enumerate() {
        let seed = 0x5E_0003u64
            .wrapping_add(i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        out.push(seat_once(a, b, seed));
    }
    out
}

pub fn findings(runs: &[SeatRun]) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in runs {
        if r.diverge.is_empty() {
            continue;
        }
        out.push(Finding {
            rule: format!("{} x {}", r.rule_a, r.rule_b),
            field: "bystander".into(),
            what: format!(
                "unintended: bystander state differs across seat rotation for {} x {} ({})",
                r.rule_a,
                r.rule_b,
                r.diverge.join("; ")
            ),
            repro: format!("seed={}", r.seed),
            severity: Severity::Medium,
        });
    }
    out
}

pub fn report(runs: &[SeatRun], findings: &[Finding]) -> Vec<String> {
    let mut lines = Vec::new();
    let bad = runs.iter().filter(|r| !r.diverge.is_empty()).count();
    lines.push(format!(
        "bystander / seat invariance: {} runs, {} diverging",
        runs.len(),
        bad
    ));
    for f in findings.iter().take(10) {
        lines.push(format!("  {}", f.what));
    }
    lines
}

/// Unused-import guard.
pub fn _touch() {
    let _ = super::footprints();
}