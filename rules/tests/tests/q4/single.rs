//! Check 1 -- single-rule footprint audit.
//!
//! Activate one rule alone on a rich board and require every non-bookkeeping
//! world change to sit inside that rule's declared `writes` (plus the play
//! cost of the activation itself). A write to a surface outside `writes` is
//! an unintended interaction: the rule is touching a resource / zone /
//! player the sheet text never mentions.

use std::collections::BTreeSet;

use super::{
    all_card_ids, all_rule_ids, all_skill_ids, arrange_rich, drain_first, footprints, quiet,
    severity_of_surface, surface_of, Finding, Severity,
};
use crate::fuzz::rng::Rng;
use crate::q4::snap::{diff, render_deltas, snapshot};

/// What one activation of a rule touched outside its footprint.
pub struct RuleAudit {
    pub rule: String,
    pub seed: u64,
    pub activated: bool,
    /// Surfaces written outside the declared footprint.
    pub stray: Vec<(String, String)>, // (surface, path: before -> after)
    /// Surfaces written that *are* declared -- recorded for the registry.
    pub in_footprint: BTreeSet<String>,
    /// Every state key / token / mark / prop name the activation wrote.
    pub names_written: BTreeSet<String>,
    /// Every such name the activation read back is not observable from a
    /// diff; the registry pass handles reads via a separate walk.
    pub deltas: usize,
}

impl RuleAudit {
    pub fn ok(&self) -> bool {
        self.stray.is_empty()
    }
}

/// Surfaces a `Play` activation is allowed to touch as the cost of pressing
/// the card: the played card leaves the hand and lands in discard or on the
/// field, and a newly placed instance starts at 0 crystals. Everything else
/// must be declared.
fn play_cost_surfaces(rule: &str, deltas: &[super::Delta]) -> BTreeSet<&'static str> {
    let mut allowed = BTreeSet::new();
    for d in deltas {
        // A path naming the rule's own card id moving between zones is the
        // play cost. Map it to the zone surfaces so the footprint check can
        // treat those as declared-by-the-press.
        let is_self = d.path.contains(rule)
            || (d.before.contains(rule) && d.path.starts_with("hand:"))
            || (d.after.contains(rule) && (d.path.starts_with("discard:") || d.path.starts_with("field:")));
        // A crystal counter appearing at 0 on a card this same window placed
        // is the instance's initial value, not a crystal write.
        let is_new_instance_zero = {
            if d.path.starts_with("crystals:") && d.before == "<absent>" && d.after == "0" {
                // `crystals:P{i}:{card}` -- take the card id.
                let card = d.path.splitn(3, ':').nth(2).unwrap_or("");
                !card.is_empty()
                    && deltas.iter().any(|o| {
                        o.path.starts_with("field:") && o.after.contains(card)
                    })
            } else {
                false
            }
        };
        if is_self || is_new_instance_zero {
            if let Some(s) = surface_of(&d.path) {
                allowed.insert(s);
            }
        }
    }
    allowed
}

/// Run the single-rule audit for one rule on one seed. Panics inside the
/// harness (a settle that never rests) become a `stray` note rather than
/// aborting the sweep.
pub fn audit_one(rule: &str, seed: u64) -> RuleAudit {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| audit_one_raw(rule, seed)));
    match r {
        Ok(a) => a,
        Err(p) => {
            let what = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            RuleAudit {
                rule: rule.to_string(),
                seed,
                activated: false,
                stray: vec![("harness".into(), what)],
                in_footprint: BTreeSet::new(),
                names_written: BTreeSet::new(),
                deltas: 0,
            }
        }
    }
}

fn audit_one_raw(rule: &str, seed: u64) -> RuleAudit {
    let fp = footprints();
    let mut rng = Rng::new(seed ^ 0xC4_51A9u64.wrapping_mul(3));
    // 3 players: enough for "other players" clauses, cheap enough to soak.
    let mut t = arrange_rich(seed, 3, &mut rng);
    quiet(&mut t);

    let is_skill = rule.starts_with("skill:");
    let who = 0;

    // ---- arrange the one rule ------------------------------------------
    let mut activated = false;
    if is_skill {
        // Drop the skill onto the field with no play body, then use it.
        t.place_raw(who, rule);
        quiet(&mut t);
    } else {
        // Give the card, leave it in hand; the snapshot after `give` is the
        // baseline so the give itself is not a rule write.
        t.give(who, &[rule]);
        quiet(&mut t);
    }

    let before = snapshot(t.m.world());

    // ---- activate -------------------------------------------------------
    if is_skill {
        let _ = t.skill(who, rule);
        activated = true;
    } else {
        // A counteract-only card has no `Play` entry; pressing play is then
        // rejected. That is fine -- we still want the "sitting in hand /
        // on the field writes nothing" half of the audit, which the
        // baseline already covers. Record whether the press stuck.
        match t.play(who, rule) {
            Ok(()) => activated = true,
            Err(_) => activated = false,
        }
    }
    drain_first(&mut t).ok();
    quiet(&mut t);

    let after = snapshot(t.m.world());
    let deltas = diff(&before, &after, false);

    // ---- classify -------------------------------------------------------
    let declared: BTreeSet<String> = fp
        .by_rule
        .get(rule)
        .map(|f| f.writes.iter().cloned().collect())
        .unwrap_or_default();
    let cost = play_cost_surfaces(rule, &deltas);

    let mut stray = Vec::new();
    let mut in_footprint = BTreeSet::new();
    let mut names_written = BTreeSet::new();

    for d in &deltas {
        // Name registry: every keyed name the activation wrote.
        if let Some(name) = name_of(&d.path) {
            names_written.insert(name);
        }
        let Some(s) = surface_of(&d.path) else {
            continue;
        };
        if declared.contains(s) || cost.contains(s) {
            in_footprint.insert(s.to_string());
            continue;
        }
        // `field` / `hand` / `discard.pile` moving the *rule's own* instance
        // is the play cost even when the footprint forgot to list it -- the
        // engine, not the rule, performs that move. Flag only *other* cards
        // appearing / disappearing.
        if matches!(s, "field" | "hand" | "discard.pile" | "draw.pile") {
            let touches_self = d.before.contains(rule) || d.after.contains(rule);
            if touches_self && activated {
                in_footprint.insert(s.to_string());
                continue;
            }
        }
        // `turn.abnormal` is the engine's record of which abnormal effects
        // landed this turn. A rule that writes `status.stay` / `status.stun`
        // is *supposed* to stamp it; that is the engine's bookkeeping of the
        // status application, not a foreign write.
        if s == "abnormal" && in_footprint.iter().any(|x| x.starts_with("status.")) {
            in_footprint.insert(s.to_string());
            continue;
        }
        stray.push((s.to_string(), format!("{}: {} -> {}", d.path, d.before, d.after)));
    }

    RuleAudit {
        rule: rule.to_string(),
        seed,
        activated,
        stray,
        in_footprint,
        names_written,
        deltas: deltas.len(),
    }
}

/// The keyed name behind an observed path, for the shared-name registry.
pub fn name_of(path: &str) -> Option<String> {
    if let Some(rest) = path.strip_prefix("state:P") {
        // `state:P{i}:{key}`
        let key = rest.split_once(':')?.1;
        return Some(format!("state:{key}"));
    }
    if let Some(rest) = path.strip_prefix("token:P") {
        // `token:P{i}:{idx}:{name}`
        let name = rest.splitn(3, ':').nth(2)?;
        return Some(format!("token:{name}"));
    }
    if let Some(rest) = path.strip_prefix("fprop:") {
        // `fprop:P{i}:{card}:{key}`
        let key = rest.splitn(4, ':').nth(3)?;
        return Some(format!("fprop:{key}"));
    }
    if let Some(rest) = path.strip_prefix("bprop:") {
        // `bprop:{i}:{key}`
        let key = rest.split_once(':')?.1;
        return Some(format!("bprop:{key}"));
    }
    if path.starts_with("mark:") {
        return Some("mark".into());
    }
    None
}

/// Audit a batch of rules. `sample` = 0 means every rule.
pub fn audit_batch(sample: usize, seeds: u64) -> Vec<RuleAudit> {
    let mut rules: Vec<String> = all_rule_ids();
    // Cards first (they are the `Play` majority), then skills.
    let mut cards: Vec<String> = all_card_ids()
        .into_iter()
        .filter(|c| footprints().by_rule.contains_key(c))
        .collect();
    cards.sort();
    let mut skills = all_skill_ids();
    skills.sort();
    rules = cards;
    rules.extend(skills);

    if sample > 0 && rules.len() > sample {
        // Deterministic stride so a default run covers the whole alphabet
        // rather than the first N ids.
        let step = rules.len().div_ceil(sample).max(1);
        rules = rules.into_iter().step_by(step).take(sample).collect();
    }

    let mut out = Vec::new();
    for rule in &rules {
        for s in 0..seeds {
            let seed = 0xC4_0001u64
                .wrapping_add(s as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(hash(rule));
            out.push(audit_one(rule, seed));
        }
    }
    out
}

fn hash(s: &str) -> u64 {
    rule_hash(s)
}

/// FNV-1a over a rule id, for deterministic per-rule seeds.
pub fn rule_hash(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01B3);
    }
    h
}

/// Turn audits into findings (highest severity first).
pub fn findings(audits: &[RuleAudit]) -> Vec<Finding> {
    let mut out = Vec::new();
    for a in audits {
        if a.ok() {
            continue;
        }
        // One finding per rule+seed, naming the first (most severe) stray.
        let mut strays = a.stray.clone();
        strays.sort_by_key(|(s, _)| severity_of_surface(s));
        let (surf, detail) = strays[0].clone();
        let sev = severity_of_surface(&surf);
        out.push(Finding {
            rule: a.rule.clone(),
            field: surf,
            what: format!(
                "unintended: {} writes outside its footprint ({}) [{} more]",
                a.rule,
                detail,
                strays.len().saturating_sub(1)
            ),
            repro: format!("seed={} activated={}", a.seed, a.activated),
            severity: sev,
        });
    }
    out.sort_by_key(|f| f.severity);
    out
}

/// Compact report lines for `--nocapture`.
pub fn report(audits: &[RuleAudit]) -> Vec<String> {
    let mut lines = Vec::new();
    let ok = audits.iter().filter(|a| a.ok()).count();
    lines.push(format!(
        "single-rule footprint audit: {ok}/{} clean ({} seeds each)",
        audits.len(),
        super::seeds_per_rule()
    ));
    for a in audits.iter().filter(|a| !a.ok()).take(20) {
        lines.push(format!(
            "  stray {} seed={} :: {}",
            a.rule,
            a.seed,
            render_deltas(
                &a.stray
                    .iter()
                    .map(|(s, d)| super::Delta {
                        path: s.clone(),
                        before: d.clone(),
                        after: String::new(),
                    })
                    .collect::<Vec<_>>(),
                4
            )
        ));
    }
    lines
}

/// Severity label helper re-exported for the report table.
pub fn sev_label(s: &str) -> &'static str {
    match severity_of_surface(s) {
        Severity::High => "high",
        Severity::Medium => "medium",
        Severity::Low => "low",
    }
}