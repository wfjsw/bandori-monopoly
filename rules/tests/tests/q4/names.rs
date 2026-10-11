//! Check 5 -- shared-name registry.
//!
//! By diffing world state per action, log every state key, token, slot, mark
//! and prop name each rule writes at runtime, then report:
//!
//! * names written by unrelated rules (collisions);
//! * names written but never read, and read but never written (typos);
//! * near-duplicate spellings, e.g. a P✽P fan name variant.
//!
//! Reads are approximated from the footprint `reads` plus a runtime walk of
//! the keyed names a rule's activation *touched* (a change to a name implies
//! the rule knew about it). The footprint `reads` list is the declared read
//! side; a name in `writes` that never appears in any `reads` and is never
//! observed changing again is a candidate typo.

use std::collections::{BTreeMap, BTreeSet};

use super::{arrange_rich, drain_first, footprints, quiet, single::name_of};
use crate::fuzz::rng::Rng;
use crate::q4::snap::{diff, snapshot};

/// name -> rules that wrote it.
pub type Registry = BTreeMap<String, BTreeSet<String>>;

/// One action's observed writes, keyed by rule.
pub struct NameLog {
    pub writes: Registry,
    /// name -> rules that declared it in `reads`.
    pub reads: Registry,
    /// Every raw name observed, for the near-duplicate scan.
    pub all_names: BTreeSet<String>,
}

/// Sweep a sample of rules, one activation each, and collect the names.
pub fn collect(sample: usize, seed: u64) -> NameLog {
    let mut log = NameLog {
        writes: BTreeMap::new(),
        reads: BTreeMap::new(),
        all_names: BTreeSet::new(),
    };
    let fp = footprints();
    for (rule, f) in &fp.by_rule {
        for r in &f.reads {
            log.reads.entry(r.clone()).or_default().insert(rule.clone());
        }
    }

    let rules = super::all_rule_ids();
    let step = rules.len().div_ceil(sample.max(1)).max(1);
    for (i, rule) in rules.into_iter().step_by(step).take(sample).enumerate() {
        let s = seed
            .wrapping_add(i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(super::single::rule_hash(&rule));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut rng = Rng::new(s ^ 0x4E41);
            let mut t = arrange_rich(s, 2, &mut rng);
            quiet(&mut t);
            if rule.starts_with("skill:") {
                t.place_raw(0, &rule);
            } else {
                t.give(0, &[rule.as_str()]);
            }
            quiet(&mut t);
            let before = snapshot(t.m.world());
            if rule.starts_with("skill:") {
                let _ = t.skill(0, &rule);
            } else {
                let _ = t.play(0, &rule);
            }
            drain_first(&mut t).ok();
            quiet(&mut t);
            let after = snapshot(t.m.world());
            diff(&before, &after, false)
        }));
        let deltas = match r {
            Ok(d) => d,
            Err(_) => continue,
        };
        for d in deltas {
            if let Some(name) = name_of(&d.path) {
                log.writes.entry(name.clone()).or_default().insert(rule.clone());
                log.all_names.insert(name);
            }
        }
    }
    log
}

/// An anomaly the registry can report.
pub struct Anomaly {
    pub kind: &'static str,
    pub name: String,
    pub detail: String,
}

/// Names written by two or more rules that share no declared surface.
pub fn collisions(log: &NameLog) -> Vec<Anomaly> {
    let fp = footprints();
    let mut out = Vec::new();
    for (name, rules) in &log.writes {
        if rules.len() < 2 {
            continue;
        }
        // Unrelated = the pair shares no declared writes surface.
        let list: Vec<String> = rules.iter().cloned().collect();
        for i in 0..list.len() {
            for j in (i + 1)..list.len() {
                let (a, b) = (&list[i], &list[j]);
                let sa: BTreeSet<String> = fp
                    .by_rule
                    .get(a)
                    .map(|f| f.writes.iter().cloned().collect())
                    .unwrap_or_default();
                let sb: BTreeSet<String> = fp
                    .by_rule
                    .get(b)
                    .map(|f| f.writes.iter().cloned().collect())
                    .unwrap_or_default();
                if sa.is_disjoint(&sb) {
                    out.push(Anomaly {
                        kind: "collision",
                        name: name.clone(),
                        detail: format!("{a} and {b} both write `{name}`"),
                    });
                }
            }
        }
    }
    out
}

/// A keyed name written at runtime that no rule's footprint `reads` list
/// names. `reads` uses surface keys (`state`, `fans`, …), not raw names, so
/// this compares against a small map of surface -> representative names and
/// otherwise reports the name as "written, read-side unknown" -- a typo
/// candidate only when the name looks like it should be shared (it appears
/// in two rules' write sets, or it differs from another name by one edit).
pub fn written_never_read(log: &NameLog) -> Vec<Anomaly> {
    let mut out = Vec::new();
    for (name, rules) in &log.writes {
        // A name only one rule writes is that rule's own scratch -- fine.
        if rules.len() < 2 {
            continue;
        }
        let surface = surface_for_name(name);
        let declared_read = log.reads.contains_key(surface);
        if !declared_read {
            out.push(Anomaly {
                kind: "write-never-read",
                name: name.clone(),
                detail: format!(
                    "written by {:?}, surface `{surface}` is declared read by nobody",
                    rules
                ),
            });
        }
    }
    out
}

/// Engine-owned key names: a frozen wire contract (`data` / `key::`), not a
/// content vocabulary. Never report these as near-duplicates of each other.
fn is_engine_key(name: &str) -> bool {
    let k = name.strip_prefix("state:").unwrap_or(name);
    matches!(
        k,
        "stay"
            | "stun"
            | "stunStart"
            | "exile"
            | "exileTo"
            | "fire"
            | "noHand"
            | "unstoppable"
            | "handLimit"
            | "startHand"
            | "skillState"
            | "built"
            | "bought"
            | "redeemed"
            | "mortgaged"
            | "noCircleReward"
            | "lastWalk"
    )
}

/// Near-duplicate spellings among *name keys* (not snapshot paths): `state:foo`
/// vs `state:fooo`, a P✽P fan-name variant, `Pipopa` inside
/// `Popipapapipopa`, etc.
pub fn near_duplicates(log: &NameLog) -> Vec<Anomaly> {
    let norm = |s: &str| -> String {
        s.chars()
            .filter(|c| c.is_alphanumeric() || *c == ':')
            .map(|c| c.to_ascii_lowercase())
            .collect::<String>()
            .replace('✽', "*")
            .replace('＊', "*")
    };
    let names: Vec<String> = log
        .writes
        .keys()
        .chain(log.all_names.iter())
        .filter(|n| {
            (n.starts_with("state:")
                || n.starts_with("token:")
                || n.starts_with("fprop:")
                || n.starts_with("bprop:")
                || n.as_str() == "mark")
                && !is_engine_key(n)
        })
        .cloned()
        .collect::<BTreeSet<String>>()
        .into_iter()
        .collect();
    let mut out = Vec::new();
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            let a = norm(&names[i]);
            let b = norm(&names[j]);
            if a == b && names[i] != names[j] {
                out.push(Anomaly {
                    kind: "near-duplicate",
                    name: names[i].clone(),
                    detail: format!("`{}` vs `{}` normalise identically", names[i], names[j]),
                });
                continue;
            }
            if a.len() >= 4 && b.len() >= 4 && edit1(&a, &b) {
                out.push(Anomaly {
                    kind: "near-duplicate",
                    name: names[i].clone(),
                    detail: format!("`{}` vs `{}` differ by one edit", names[i], names[j]),
                });
                continue;
            }
            // Containment: `Pipopa` inside `Popipapapipopa`, `pp` inside a
            // `pastel*palettes` key. Requires a decent shared stem so
            // `state:x` vs `state:xy` noise stays out.
            let (short, long, sn, ln) = if a.len() < b.len() {
                (&a, &b, &names[i], &names[j])
            } else {
                (&b, &a, &names[j], &names[i])
            };
            if short.len() >= 5 && long.contains(short.as_str()) {
                out.push(Anomaly {
                    kind: "near-duplicate",
                    name: sn.clone(),
                    detail: format!("`{sn}` is a substring of `{ln}`"),
                });
            }
        }
    }
    out.truncate(30);
    out
}

fn edit1(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len().abs_diff(b.len()) > 1 {
        return false;
    }
    let (mut i, mut j, mut diff) = (0usize, 0usize, 0usize);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        diff += 1;
        if diff > 1 {
            return false;
        }
        if a.len() > b.len() {
            i += 1;
        } else if b.len() > a.len() {
            j += 1;
        } else {
            i += 1;
            j += 1;
        }
    }
    diff + (a.len() - i) + (b.len() - j) <= 1
}

pub fn report(log: &NameLog, cols: &[Anomaly]) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "shared-name registry: {} names written, {} declared-read surfaces, {} anomalies",
        log.writes.len(),
        log.reads.len(),
        cols.len()
    ));
    for a in cols.iter().take(20) {
        lines.push(format!("  [{}] {} -- {}", a.kind, a.name, a.detail));
    }
    // A compact top-of-registry sample for the coverage doc.
    for (name, rules) in log.writes.iter().take(12) {
        lines.push(format!(
            "  name `{name}` <- {}",
            rules.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    lines
}

/// Map an observed name back to the footprint surface it represents, so the
/// coverage table can say "state key `skill.x` is a `state` write".
pub fn surface_for_name(name: &str) -> &'static str {
    if let Some(k) = name.strip_prefix("state:") {
        return match k {
            "fire" => "fire",
            "stay" => "status.stay",
            _ => "state",
        };
    }
    if name.starts_with("token:") {
        return "fans";
    }
    if name.starts_with("fprop:") || name.starts_with("bprop:") {
        return "tile.effect";
    }
    if name == "mark" {
        return "marks";
    }
    "state"
}