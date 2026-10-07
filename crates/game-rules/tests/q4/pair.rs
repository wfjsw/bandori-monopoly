//! Check 2 -- pairwise non-interference.
//!
//! For two rules whose footprints share no surface, running B must produce
//! the same delta whether or not A is already in play. A divergence is an
//! unintended interaction through shared state, an over-broad hook, or a
//! missed expiry.

use std::collections::BTreeSet;

use super::{
    arrange_rich, drain_first, footprints, quiet, surface_of, Finding, Severity,
};
use crate::fuzz::rng::Rng;
use crate::q4::snap::{diff, snapshot, Delta};

/// A pair whose `writes ∪ reads` surfaces are disjoint.
pub fn disjoint_pairs(limit: usize) -> Vec<(String, String)> {
    let fp = footprints();
    let ids: Vec<String> = fp.by_rule.keys().cloned().collect();
    let mut out = Vec::new();
    // Deterministic walk: stride through the id list so a small sample still
    // spans the alphabet, and take pairs whose surfaces do not overlap.
    let n = ids.len();
    if n < 2 {
        return out;
    }
    let stride = (n / 2).max(1);
    let mut i = 0usize;
    let mut guard = 0usize;
    while out.len() < limit && guard < n * 8 {
        let a = &ids[i % n];
        let b = &ids[(i + stride + guard) % n];
        guard += 1;
        if a == b {
            continue;
        }
        let sa: BTreeSet<String> = fp.by_rule[a]
            .writes
            .iter()
            .chain(fp.by_rule[a].reads.iter())
            .cloned()
            .collect();
        let sb: BTreeSet<String> = fp.by_rule[b]
            .writes
            .iter()
            .chain(fp.by_rule[b].reads.iter())
            .cloned()
            .collect();
        if sa.is_disjoint(&sb) {
            let key = if a < b {
                (a.clone(), b.clone())
            } else {
                (b.clone(), a.clone())
            };
            if !out.contains(&key) {
                out.push(key);
            }
        }
        i += 1;
    }
    out
}

/// Activate one rule and return whether the press stuck.
fn activate(t: &mut crate::common::Table, who: usize, rule: &str) -> bool {
    if rule.starts_with("skill:") {
        t.place_raw(who, rule);
        quiet(t);
        t.skill(who, rule).is_ok()
    } else {
        t.give(who, &[rule]);
        quiet(t);
        t.play(who, rule).is_ok()
    }
}

/// Delta of `rule`'s activation, restricted to the surfaces `rule` declares
/// (plus the play cost). Used as the comparison key.
fn scoped_delta(deltas: &[Delta], rule: &str) -> Vec<(String, String, String)> {
    let fp = footprints();
    let declared: BTreeSet<String> = fp
        .by_rule
        .get(rule)
        .map(|f| f.writes.iter().cloned().collect())
        .unwrap_or_default();
    deltas
        .iter()
        .filter_map(|d| {
            let s = surface_of(&d.path)?;
            if declared.contains(s) {
                Some((s.to_string(), d.path.clone(), format!("{}->{}", d.before, d.after)))
            } else {
                None
            }
        })
        .collect()
}

/// One pairwise observation.
pub struct PairRun {
    pub a: String,
    pub b: String,
    pub seed: u64,
    /// First field where `delta(B | A)` diverged from `delta(B alone)`.
    pub diverge: Option<(String, String, String)>,
    pub b_alone_len: usize,
    pub b_after_a_len: usize,
}

/// Run B alone and A-then-B from the same arrangement seed and the same
/// deterministic answers. Compare B's scoped delta. Harness panics become a
/// recorded divergence rather than aborting the sweep.
pub fn pair_once(a: &str, b: &str, seed: u64) -> PairRun {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pair_once_raw(a, b, seed)));
    match r {
        Ok(p) => p,
        Err(p) => {
            let what = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            PairRun {
                a: a.to_string(),
                b: b.to_string(),
                seed,
                diverge: Some(("harness".into(), "panic".into(), what)),
                b_alone_len: 0,
                b_after_a_len: 0,
            }
        }
    }
}

fn pair_once_raw(a: &str, b: &str, seed: u64) -> PairRun {
    let mut out = PairRun {
        a: a.to_string(),
        b: b.to_string(),
        seed,
        diverge: None,
        b_alone_len: 0,
        b_after_a_len: 0,
    };

    // ---- run 1: B alone -------------------------------------------------
    let mut rng = Rng::new(seed ^ 0xA11E);
    let mut t1 = arrange_rich(seed, 3, &mut rng);
    quiet(&mut t1);
    // Freeze the loaded-dice stream so both runs roll the same faces.
    let faces: Vec<i32> = (0..8).map(|_| rng.range(1, 6)).collect();
    t1.dice(&faces);
    quiet(&mut t1);
    let s0 = snapshot(t1.m.world());
    let who_b = 0;
    activate(&mut t1, who_b, b);
    drain_first(&mut t1).ok();
    quiet(&mut t1);
    let s1 = snapshot(t1.m.world());
    let d_alone = diff(&s0, &s1, false);

    // ---- run 2: A then B ----------------------------------------------
    let mut rng2 = Rng::new(seed ^ 0xA11E);
    let mut t2 = arrange_rich(seed, 3, &mut rng2);
    quiet(&mut t2);
    t2.dice(&faces);
    quiet(&mut t2);
    // A plays as player 1 so B (player 0) is not the same seat -- that is
    // the interesting case ("A is already in play elsewhere").
    activate(&mut t2, 1, a);
    drain_first(&mut t2).ok();
    quiet(&mut t2);
    // B's baseline is *after* A is settled.
    let s2 = snapshot(t2.m.world());
    activate(&mut t2, who_b, b);
    drain_first(&mut t2).ok();
    quiet(&mut t2);
    let s3 = snapshot(t2.m.world());
    let d_after = diff(&s2, &s3, false);

    let sc_alone = scoped_delta(&d_alone, b);
    let sc_after = scoped_delta(&d_after, b);
    out.b_alone_len = sc_alone.len();
    out.b_after_a_len = sc_after.len();

    // Vacuity guard: if B's activation changed nothing in either run, the
    // pair is not informative (the press was refused). Leave `diverge` None
    // and note the sizes so the report can say how many pairs were live.
    if sc_alone.is_empty() && sc_after.is_empty() {
        return out;
    }

    // Compare as ordered lists of (surface, path, value). A path present in
    // one and not the other is a divergence; so is a differing value.
    let map_a: std::collections::BTreeMap<&str, &str> = sc_alone
        .iter()
        .map(|(_, p, v)| (p.as_str(), v.as_str()))
        .collect();
    let map_b: std::collections::BTreeMap<&str, &str> = sc_after
        .iter()
        .map(|(_, p, v)| (p.as_str(), v.as_str()))
        .collect();
    let keys: BTreeSet<&str> = map_a.keys().chain(map_b.keys()).copied().collect();
    for k in keys {
        let va = map_a.get(k).copied().unwrap_or("<absent>");
        let vb = map_b.get(k).copied().unwrap_or("<absent>");
        if va != vb {
            let surf = surface_of(k).unwrap_or("state").to_string();
            out.diverge = Some((surf, k.to_string(), format!("alone={va} afterA={vb}")));
            break;
        }
    }
    out
}

/// Sampled pairwise run.
pub fn pair_batch(sample: usize) -> Vec<PairRun> {
    let pairs = disjoint_pairs(sample.max(1));
    let mut out = Vec::new();
    for (i, (a, b)) in pairs.iter().enumerate() {
        let seed = 0xB2B0u64
            .wrapping_add(i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        out.push(pair_once(a, b, seed));
    }
    out
}

pub fn findings(runs: &[PairRun]) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in runs {
        let Some((surf, path, detail)) = &r.diverge else {
            continue;
        };
        out.push(Finding {
            rule: format!("{} x {}", r.a, r.b),
            field: surf.clone(),
            what: format!(
                "unintended: disjoint pair {} x {} diverges on {} ({})",
                r.a, r.b, path, detail
            ),
            repro: format!("seed={} b_alone={} b_after_a={}", r.seed, r.b_alone_len, r.b_after_a_len),
            severity: severity_of(surf),
        });
    }
    out.sort_by_key(|f| f.severity);
    out
}

fn severity_of(s: &str) -> Severity {
    super::severity_of_surface(s)
}

pub fn report(runs: &[PairRun]) -> Vec<String> {
    let mut lines = Vec::new();
    let bad = runs.iter().filter(|r| r.diverge.is_some()).count();
    let live = runs
        .iter()
        .filter(|r| r.b_alone_len > 0 || r.b_after_a_len > 0)
        .count();
    lines.push(format!(
        "pairwise non-interference: {} pairs ({} live, B changed something), {} diverging",
        runs.len(),
        live,
        bad
    ));
    for r in runs.iter().filter(|r| r.diverge.is_some()).take(15) {
        let (s, p, d) = r.diverge.as_ref().unwrap();
        lines.push(format!("  diverge {} x {} on {} {} {}", r.a, r.b, s, p, d));
    }
    lines
}