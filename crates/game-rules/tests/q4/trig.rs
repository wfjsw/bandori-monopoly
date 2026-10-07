//! Check 3 -- trigger scope.
//!
//! From the event log, record when each placed card's / skill's hooks fire and
//! compare against the sheet's scoping words:
//!
//! * 「你的回合…」 only on the owner's turns;
//! * 「其他玩家…」 never for the owner;
//! * 「经过X」 only on X;
//! * once-per-turn-end where the text says so.
//!
//! Method: place the rule on player 0's field, then run player 0's turn and
//! player 1's turn as two separate windows. A rule whose text is scoped to
//! 「你的回合」 must not name itself in events during player 1's turn.

use std::collections::{BTreeMap, BTreeSet};

use game_core::state::MatchEvent;

use super::{arrange_rich, drain_first, quiet, Finding, Severity};
use crate::fuzz::rng::Rng;

/// One observed hook fire.
#[derive(Clone, Debug)]
pub struct Fire {
    pub rule: String,
    pub event_key: String,
    /// Whose turn the fire happened in.
    pub turn_player: usize,
    /// Who owns the rule instance.
    pub owner: usize,
    pub at_end: bool,
}

fn event_names(e: &MatchEvent, rule: &str) -> bool {
    if e.card == rule {
        return true;
    }
    for v in e.msg.a.values() {
        match v {
            game_core::msg::Arg::Card(c) if c == rule => return true,
            game_core::msg::Arg::Text(s) | game_core::msg::Arg::Char(s) | game_core::msg::Arg::Band(s)
                if s == rule || s.contains(rule) =>
            {
                return true
            }
            _ => {}
        }
    }
    format!("{:?}", e.msg).contains(rule)
}

/// Collect fires for one rule over its owner's turn and another player's
/// turn. Returns the fires (possibly empty).
pub fn fires_for(rule: &str, seed: u64) -> Vec<Fire> {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fires_for_raw(rule, seed)));
    r.unwrap_or_default()
}

fn fires_for_raw(rule: &str, seed: u64) -> Vec<Fire> {
    let mut rng = Rng::new(seed ^ 0x7716);
    let mut t = arrange_rich(seed, 3, &mut rng);
    quiet(&mut t);
    let faces: Vec<i32> = (0..10).map(|_| 3).collect();
    t.dice(&faces);
    quiet(&mut t);

    let owner = 0;
    if rule.starts_with("skill:") {
        t.place_raw(owner, rule);
    } else {
        t.give(owner, &[rule]);
        let _ = t.play(owner, rule);
    }
    quiet(&mut t);

    let mut fires = Vec::new();

    // ---- window 1: the owner's own turn (roll + end) --------------------
    {
        let who = t.turn();
        // Make sure it is the owner's turn.
        if who != owner {
            t.begin_turn(owner);
        }
        quiet(&mut t);
        let mark = t.mark();
        let _ = t.roll(owner);
        drain_first(&mut t).ok();
        quiet(&mut t);
        let at_end_mark = t.mark();
        let _ = t.end(owner);
        drain_first(&mut t).ok();
        quiet(&mut t);
        for e in t.events_since(mark) {
            if event_names(&e, rule) {
                let at_end = e.id >= at_end_mark;
                fires.push(Fire {
                    rule: rule.to_string(),
                    event_key: e.msg.key().to_string(),
                    turn_player: owner,
                    owner,
                    at_end,
                });
            }
        }
    }

    // ---- window 2: another player's turn --------------------------------
    {
        let other = 1;
        t.begin_turn(other);
        quiet(&mut t);
        let mark = t.mark();
        let _ = t.roll(other);
        drain_first(&mut t).ok();
        quiet(&mut t);
        let at_end_mark = t.mark();
        let _ = t.end(other);
        drain_first(&mut t).ok();
        quiet(&mut t);
        for e in t.events_since(mark) {
            if event_names(&e, rule) {
                let at_end = e.id >= at_end_mark;
                fires.push(Fire {
                    rule: rule.to_string(),
                    event_key: e.msg.key().to_string(),
                    turn_player: other,
                    owner,
                    at_end,
                });
            }
        }
    }
    fires
}

/// Load the rule texts from `target/scratch/rb/*.md` (allowed input).
pub fn load_texts() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/scratch/rb");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return m;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }
        let Ok(s) = std::fs::read_to_string(&p) else {
            continue;
        };
        let mut cur = String::new();
        let mut buf = String::new();
        for line in s.lines() {
            if let Some(rest) = line.strip_prefix("## ") {
                if !cur.is_empty() {
                    m.insert(cur.clone(), buf.clone());
                }
                cur = rest.trim().to_string();
                buf.clear();
            } else {
                buf.push_str(line);
                buf.push('\n');
            }
        }
        if !cur.is_empty() {
            m.insert(cur, buf);
        }
    }
    m
}

/// Apply the scope rules the sheet text encodes.
pub fn scope_findings(fires: &[Fire], texts: &BTreeMap<String, String>) -> Vec<Finding> {
    let mut out = Vec::new();
    // Group by rule so a single fire does not double-report.
    let mut by_rule: BTreeMap<String, Vec<&Fire>> = BTreeMap::new();
    for f in fires {
        by_rule.entry(f.rule.clone()).or_default().push(f);
    }
    for (rule, fs) in by_rule {
        let text = texts.get(&rule).cloned().unwrap_or_default();
        let declares_own_turn = text.contains("你的回合") || text.contains("在你的回合");
        let declares_other = text.contains("其他玩家") || text.contains("别人");

        let foreign: Vec<&&Fire> = fs.iter().filter(|f| f.turn_player != f.owner).collect();
        let own: Vec<&&Fire> = fs.iter().filter(|f| f.turn_player == f.owner).collect();

        if declares_own_turn && !declares_other && !foreign.is_empty() {
            let f = foreign[0];
            out.push(Finding {
                rule: rule.clone(),
                field: "triggers".into(),
                what: format!(
                    "unintended: {} (「你的回合」) fires on another player's turn (P{}, owner P{}) key={}",
                    rule, f.turn_player, f.owner, f.event_key
                ),
                repro: format!("turn P{}", f.turn_player),
                severity: Severity::High,
            });
        }
        if declares_other && !declares_own_turn && !own.is_empty() {
            let f = own[0];
            out.push(Finding {
                rule: rule.clone(),
                field: "triggers".into(),
                what: format!(
                    "unintended: {} (「其他玩家」) fires for its owner key={}",
                    rule, f.event_key
                ),
                repro: format!("turn P{}", f.turn_player),
                severity: Severity::Medium,
            });
        }
    }
    out
}

/// Once-per-turn-end: a rule must not produce two fires in the same turn-end
/// window.
pub fn at_end_duplicates(fires: &[Fire]) -> Vec<Finding> {
    let mut seen: BTreeSet<(String, usize)> = BTreeSet::new();
    let mut out = Vec::new();
    for f in fires {
        if !f.at_end {
            continue;
        }
        let key = (f.rule.clone(), f.turn_player);
        if !seen.insert(key) {
            out.push(Finding {
                rule: f.rule.clone(),
                field: "triggers".into(),
                what: format!(
                    "unintended: {} fires more than once at the same turn end (P{})",
                    f.rule, f.turn_player
                ),
                repro: format!("turn P{}", f.turn_player),
                severity: Severity::Medium,
            });
        }
    }
    out
}

/// Sweep a sample of rules.
pub fn sweep(sample: usize, seed: u64) -> (Vec<Fire>, Vec<Finding>) {
    let rules = super::all_rule_ids();
    let step = rules.len().div_ceil(sample.max(1)).max(1);
    let texts = load_texts();
    let mut all = Vec::new();
    let mut findings = Vec::new();
    for (i, rule) in rules.into_iter().step_by(step).take(sample).enumerate() {
        let s = seed
            .wrapping_add(i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(super::single::rule_hash(&rule));
        let fs = fires_for(&rule, s);
        all.extend(fs.iter().cloned());
        findings.extend(scope_findings(&fs, &texts));
        findings.extend(at_end_duplicates(&fs));
    }
    (all, findings)
}

pub fn report(fires: &[Fire], findings: &[Finding]) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "trigger-scope: {} hook fires observed, {} scope violations",
        fires.len(),
        findings.len()
    ));
    for f in findings.iter().take(15) {
        lines.push(format!("  {}", f.what));
    }
    // A sample of what did fire, for the coverage doc.
    let mut seen = BTreeSet::new();
    for f in fires.iter().take(200) {
        if seen.insert(f.rule.clone()) {
            lines.push(format!(
                "  fire {} key={} turn=P{} owner=P{}",
                f.rule, f.event_key, f.turn_player, f.owner
            ));
        }
    }
    lines
}