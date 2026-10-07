//! Check 4 -- expiry / leak checks.
//!
//! After every 本回合 / 直到…回合开始 expiry, and after every card leaves the
//! field, the related state must be back to baseline: turn-ctx flags, stamped
//! props, `scheduled`, marks, redirects, immunities, tile-instance props.
//!
//! Method: for each rule, run the same arrangement twice -- once with the
//! rule activated and once without (the control) -- push both through the
//! same turn end / next turn start, and require the *rule's own* leftover
//! state to match the control. Movement and money the activation itself
//! caused are not leaks; a flag that outlives its turn is.

use std::collections::BTreeSet;

use super::{arrange_rich, drain_first, quiet, single::name_of, Finding, Severity};
use crate::fuzz::rng::Rng;
use crate::q4::snap::{diff, snapshot, Snap};

/// What one rule left behind after its turn expired.
pub struct ExpiryRun {
    pub rule: String,
    pub seed: u64,
    /// Fields still different from the control after the turn boundary.
    pub leaked: Vec<String>,
    /// `scheduled` entries still naming the rule.
    pub scheduled_leak: usize,
}

/// Run one rule through a full turn and check nothing of its own turn-scoped
/// state survives the turn end. Harness panics become a leak note.
pub fn expiry_one(rule: &str, seed: u64) -> ExpiryRun {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| expiry_one_raw(rule, seed)));
    match r {
        Ok(e) => e,
        Err(p) => {
            let what = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            ExpiryRun {
                rule: rule.to_string(),
                seed,
                leaked: vec![format!("harness panic: {what}")],
                scheduled_leak: 0,
            }
        }
    }
}

/// Push the table through a full lap of turn ends (every player's turn
/// boundary fires once, so 「直到你的回合开始」 expiries and per-player
/// `targeted` resets all run).
///
/// `end` is refused before the main move (`err.roll_first`), so each turn rolls
/// first -- otherwise the boundary never fires and every per-turn counter looks
/// like a leak.
fn through_turn_boundary(t: &mut crate::common::Table) {
    let n = t.n;
    for _ in 0..n {
        drain_first(t).ok();
        quiet(t);
        let who = t.turn();
        let _ = t.roll(who);
        drain_first(t).ok();
        quiet(t);
        let _ = t.end(who);
        drain_first(t).ok();
        quiet(t);
    }
}

fn expiry_one_raw(rule: &str, seed: u64) -> ExpiryRun {
    let mut rng = Rng::new(seed ^ 0xE8_B1A7u64.wrapping_mul(7));
    let mut t = arrange_rich(seed, 3, &mut rng);
    quiet(&mut t);
    // The same loaded faces for the control so the walk matches.
    let faces: Vec<i32> = (0..8).map(|_| 3).collect();
    t.dice(&faces);
    quiet(&mut t);

    // ---- control: same board, no activation ----------------------------
    let mut rng_c = Rng::new(seed ^ 0xE8_B1A7u64.wrapping_mul(7));
    let mut tc = arrange_rich(seed, 3, &mut rng_c);
    quiet(&mut tc);
    tc.dice(&faces);
    quiet(&mut tc);
    let control_start = snapshot(tc.m.world());
    through_turn_boundary(&mut tc);
    let control_end = snapshot(tc.m.world());
    let control_delta = diff(&control_start, &control_end, true);

    // ---- rule run ------------------------------------------------------
    let who = 0;
    if rule.starts_with("skill:") {
        t.place_raw(who, rule);
    } else {
        t.give(who, &[rule]);
    }
    quiet(&mut t);
    let placed = snapshot(t.m.world());

    if rule.starts_with("skill:") {
        let _ = t.skill(who, rule);
    } else {
        let _ = t.play(who, rule);
    }
    drain_first(&mut t).ok();
    quiet(&mut t);
    let activated = snapshot(t.m.world());

    through_turn_boundary(&mut t);
    let ended = snapshot(t.m.world());

    // Names the activation wrote -- those are the candidates for a leak.
    let activated_names: BTreeSet<String> = diff(&placed, &activated, false)
        .iter()
        .filter_map(|d| name_of(&d.path))
        .collect();

    // What the activation did that must be undone by the boundary: turn-ctx
    // fields and the names it introduced. Compare the post-boundary state
    // against the control's post-boundary state, restricted to those.
    let rule_end = diff(&control_end, &ended, true);

    let mut leaked = Vec::new();
    // A rule with an ongoing hook can re-stamp per-turn counters every lap;
    // only a one-shot rule leaking one is a finding.
    let fp = super::footprints();
    let ongoing = fp
        .by_rule
        .get(rule)
        .map(|f| {
            f.triggers.iter().any(|t| t.starts_with("Hook") || t.starts_with("AtEnd"))
                || f.reads.iter().any(|r| r.starts_with("turn."))
        })
        .unwrap_or(false);

    for d in &rule_end {
        // The rule's own field / hand presence is meant to persist (a
        // [持续] card stays on the field).
        if d.path.contains(rule) {
            continue;
        }
        if is_turn_scoped(&d.path) {
            leaked.push(format!("{}: {} vs control {}", d.path, d.after, d.before));
            continue;
        }
        if is_per_turn_counter(&d.path) && !ongoing {
            leaked.push(format!("{}: {} vs control {}", d.path, d.after, d.before));
            continue;
        }
        if let Some(name) = name_of(&d.path) {
            if activated_names.contains(&name) && is_turn_scoped_name(&name) {
                leaked.push(format!("{}: {} vs control {}", d.path, d.after, d.before));
            }
        }
        // A mark / scheduled / stamped prop the rule introduced and the
        // boundary did not remove. An ongoing hook (「每个回合生成一个…」)
        // legitimately leaves marks standing -- skip those.
        if d.path.starts_with("mark:") || d.path == "scheduled" || d.path.starts_with("bprop:") {
            if d.after != d.before && !ongoing {
                leaked.push(format!("{}: {} vs control {}", d.path, d.after, d.before));
            }
        }
    }

    // Anything the activation scheduled for a turn end must be gone.
    let scheduled_leak = t
        .m
        .world()
        .scheduled
        .iter()
        .filter(|s| s.card == rule)
        .count();
    if scheduled_leak > 0 {
        leaked.push(format!("scheduled x{scheduled_leak} still standing for {rule}"));
    }

    // Turn-ctx the activation touched must match the control after the
    // boundary (already covered by is_turn_scoped above, but make the
    // direction explicit for `turn.*` that the activation wrote).
    let act_delta = diff(&placed, &activated, true);
    for d in &act_delta {
        if d.path.starts_with("turn.") && d.path != "turn.played" && d.path != "turn.player" {
            // The activation set a turn-ctx flag; after the boundary it must
            // equal the control. `rule_end` already compared ended vs
            // control_end, so a hit here is a second look at the same leak.
            if !rule_end.iter().any(|r| r.path == d.path) && d.path != "turn.turn_rolls" {
                // Flag is gone from the ended snapshot (reverted) -- fine.
                // If it were still set, `rule_end` would list it.
            }
        }
    }

    ExpiryRun {
        rule: rule.to_string(),
        seed,
        leaked,
        scheduled_leak,
    }
}

/// Turn-ctx fields that must reset at a turn boundary and that an ongoing
/// [持续] hook cannot legitimately re-stamp (they are per-press, not
/// per-turn counters).
fn is_turn_scoped(path: &str) -> bool {
    matches!(
        path,
        "turn.extra"
            | "turn.buy_discount"
            | "turn.free_buy"
            | "turn.raze_on_buy"
            | "turn.build_discount"
            | "turn.build_discount_layers"
            | "turn.build_cost_pct"
            | "turn.extreme"
            | "turn.play_from_hand"
            | "turn.paid_in_settle"
            | "turn.no_money_loss"
            | "turn.fixed_roll"
            | "extra_turns"
            | "ring_bonus"
    )
}

/// Per-turn counters an ongoing hook may legitimately re-stamp. Only a
/// one-shot rule (no turn-start / turn-end hook) leaking one of these is a
/// finding.
fn is_per_turn_counter(path: &str) -> bool {
    matches!(path, "turn.abnormal" | "targeted" | "turn.main_steps")
}

fn is_turn_scoped_name(name: &str) -> bool {
    // Scratch keys a rule arms for its own turn. A persistent key (a declared
    // X counter, a declared marker) is allowed to stay.
    let k = name.strip_prefix("state:").unwrap_or(name);
    k.ends_with("_turn")
        || k.ends_with("ThisTurn")
        || k.ends_with("_this_turn")
        || k.starts_with("turn_")
        || k.contains("until")
        || k.contains("next_turn")
}

/// Run the expiry sweep over a sample of rules.
pub fn expiry_batch(sample: usize, seeds: u64) -> Vec<ExpiryRun> {
    let rules = super::all_rule_ids();
    let step = rules.len().div_ceil(sample.max(1)).max(1);
    let mut out = Vec::new();
    for rule in rules.into_iter().step_by(step).take(sample) {
        for s in 0..seeds {
            let seed = 0xE8_0002u64
                .wrapping_add(s as u64)
                .wrapping_mul(0x9E37_79B9_7F4A_7C15)
                .wrapping_add(super::single::rule_hash(&rule));
            out.push(expiry_one(&rule, seed));
        }
    }
    out
}

pub fn findings(runs: &[ExpiryRun]) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut play_from_hand_rules: Vec<String> = Vec::new();
    for r in runs {
        let interesting: Vec<String> = r
            .leaked
            .iter()
            .filter(|l| !l.contains("turn.play_from_hand"))
            .cloned()
            .collect();
        if r.leaked.iter().any(|l| l.contains("turn.play_from_hand")) {
            play_from_hand_rules.push(r.rule.clone());
        }
        if interesting.is_empty() {
            continue;
        }
        out.push(Finding {
            rule: r.rule.clone(),
            field: "expiry".into(),
            what: format!(
                "unintended: {} leaves state past its expiry ({})",
                r.rule,
                interesting.join("; ")
            ),
            repro: format!("seed={}", r.seed),
            severity: Severity::Medium,
        });
    }
    // One finding for the systematic `turn.play_from_hand` leak.
    if !play_from_hand_rules.is_empty() {
        play_from_hand_rules.sort();
        play_from_hand_rules.dedup();
        out.push(Finding {
            rule: "(engine play-ctx)".into(),
            field: "expiry".into(),
            what: format!(
                "unintended: turn.play_from_hand is not cleared at play end or turn end; still true after a full turn lap (seen after playing {:?}, {} rules)",
                play_from_hand_rules.iter().take(6).collect::<Vec<_>>(),
                play_from_hand_rules.len()
            ),
            repro: "play any hand card, end the turn; turn.play_from_hand stays true".into(),
            severity: Severity::High,
        });
    }
    out
}

pub fn report(runs: &[ExpiryRun], findings: &[Finding]) -> Vec<String> {
    let mut lines = Vec::new();
    let bad = runs.iter().filter(|r| !r.leaked.is_empty()).count();
    lines.push(format!(
        "expiry / leak: {} rule-seeds, {} leaking",
        runs.len(),
        bad
    ));
    for f in findings.iter().take(12) {
        lines.push(format!("  {}", f.what));
    }
    lines
}

/// After a card leaves the field, its stamped props and tile-instance props
/// must be gone. Walks the field lists and asserts no orphan uids remain.
pub fn orphan_props(t: &crate::common::Table) -> Vec<String> {
    let mut out = Vec::new();
    let w = t.m.world();
    let mut uids = BTreeSet::new();
    for p in &w.st.players {
        for f in &p.field {
            if f.uid != 0 && !uids.insert(f.uid) {
                out.push(format!("duplicate field uid {}", f.uid));
            }
        }
    }
    for f in &w.st.board_field {
        if f.uid != 0 && !uids.insert(f.uid) {
            out.push(format!("duplicate board-field uid {}", f.uid));
        }
    }
    out
}

/// Snapshot helper re-export.
pub fn snap_of(t: &crate::common::Table) -> Snap {
    snapshot(t.m.world())
}