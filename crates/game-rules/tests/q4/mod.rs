//! Q4 -- unintended-interaction checks.
//!
//! Black-box: nothing under `rules/cards/`, `rules/skills/`, `rules/fixtures/`
//! or `target/scratch/tainted/` is opened. Rule ids and footprints come from
//! the shipped manifest (`ruleset().cards()`) and
//! `docs/rulebook/footprints.json`; rule text from `target/scratch/rb/*.md`
//! and `data/rules.txt`.

pub mod expiry;
pub mod names;
pub mod pair;
pub mod seat;
pub mod single;
pub mod snap;
pub mod trig;

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::common::Table;
use crate::fuzz::rng::Rng;

pub use snap::{diff, render_deltas, snapshot, surface_of, surfaces_of, Delta, Snap};

/// One rule's declared footprint.
#[derive(Clone, Debug, Default)]
pub struct Footprint {
    pub writes: Vec<String>,
    pub reads: Vec<String>,
    pub triggers: Vec<String>,
}

/// `docs/rulebook/footprints.json`.
pub struct Footprints {
    pub by_rule: BTreeMap<String, Footprint>,
    pub writers_of: BTreeMap<String, Vec<String>>,
}

static FOOTPRINTS: OnceLock<Footprints> = OnceLock::new();

pub fn footprints() -> &'static Footprints {
    FOOTPRINTS.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/rulebook/footprints.json");
        let raw = std::fs::read_to_string(&path).expect("footprints.json");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("footprints parse");
        let mut by_rule = BTreeMap::new();
        let mut writers_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (rule, fp) in v.as_object().expect("footprints object") {
            let get = |k: &str| -> Vec<String> {
                fp.get(k)
                    .and_then(|a| a.as_array())
                    .map(|a| {
                        a.iter()
                            .filter_map(|s| s.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            let w = get("writes");
            for s in &w {
                writers_of.entry(s.clone()).or_default().push(rule.clone());
            }
            by_rule.insert(
                rule.clone(),
                Footprint {
                    writes: w,
                    reads: get("reads"),
                    triggers: get("triggers"),
                },
            );
        }
        Footprints {
            by_rule,
            writers_of,
        }
    })
}

/// Every rule id in the footprints, sorted.
pub fn all_rule_ids() -> Vec<String> {
    footprints().by_rule.keys().cloned().collect()
}

/// Card ids (non-skill) from the shipped manifest.
pub fn all_card_ids() -> Vec<String> {
    crate::fuzz::gen::all_card_ids().clone()
}

/// Skill ids from the footprints (the manifest has no separate skill list).
pub fn all_skill_ids() -> Vec<String> {
    all_rule_ids()
        .into_iter()
        .filter(|r| r.starts_with("skill:"))
        .collect()
}

/// A known finding: the default run stays green by recognising these
/// signatures. Only confirmed findings belong here.
pub struct KnownFinding {
    pub signature: &'static str,
    pub points_at: &'static str,
}

pub const KNOWN_FINDINGS: &[KnownFinding] = &[
    KnownFinding {
        signature: "charity_",
        points_at: "rb_unintended_found::charity_writes_undeclared_state",
    },
    KnownFinding {
        signature: "exist_used",
        points_at: "rb_unintended_found::exist_writes_undeclared_state",
    },
    KnownFinding {
        signature: "skillBlock",
        points_at: "rb_unintended_found::stage_accident_writes_two_skillblock_names",
    },
];

pub fn is_known(err: &str) -> Option<&'static str> {
    KNOWN_FINDINGS
        .iter()
        .find(|k| err.contains(k.signature))
        .map(|k| k.points_at)
}

/// A filed finding.
#[derive(Clone, Debug)]
pub struct Finding {
    pub rule: String,
    pub field: String,
    pub what: String,
    pub repro: String,
    pub severity: Severity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Money / movement -- the highest-impact unintended coupling.
    High,
    /// Resources, deeds, hands, field.
    Medium,
    /// Names, marks, bookkeeping-adjacent state.
    Low,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
        }
    }
}

/// Rank a surface for the severity table.
pub fn severity_of_surface(s: &str) -> Severity {
    if s == "money" || s.starts_with("path.") || s == "roll.result" || s == "roll.set" {
        Severity::High
    } else if s.starts_with("tile.")
        || s == "hand"
        || s == "draw.pile"
        || s == "discard.pile"
        || s == "field"
        || s == "crystals"
        || s == "fire"
        || s == "effects"
    {
        Severity::Medium
    } else {
        Severity::Low
    }
}

// ---------------------------------------------------------------- arrange

/// A rich, randomized board: deeds / houses / mortgages / money / positions /
/// statuses / fire, but **no other field cards and no bound skills**, so a
/// single-rule activation has unambiguous attribution. `rng` is the only
/// source of arrangement randomness.
pub fn arrange_rich(seed: u64, n: usize, rng: &mut Rng) -> Table {
    let chars = crate::common::characters();
    let chosen: Vec<String> = (0..n).map(|i| chars[i % chars.len()].clone()).collect();
    let refs: Vec<&str> = chosen.iter().map(String::as_str).collect();
    let mut t = Table::with_seed(&refs, seed);
    t.strip_skills();
    t.clean();

    let tiles = t.m.world().st.owners.len();
    for tile in 0..tiles {
        if rng.chance(40) {
            t.set_owner(tile, Some(rng.pick_idx(n)));
        }
        if rng.chance(30) {
            t.set_houses(tile, rng.range(0, 3));
        }
        if rng.chance(12) {
            t.set_mortgaged(tile, true);
        }
    }
    for who in 0..n {
        t.set_money(who, rng.range(3_000, 30_000));
        t.set_pos(who, rng.pick_idx(tiles.max(1)));
        if rng.chance(18) {
            t.set_state(who, "stay", rng.range(1, 2));
        }
        if rng.chance(10) {
            t.set_state(who, "stun", rng.range(1, 2));
        }
        if rng.chance(8) {
            let cap = rng.range(1, 3);
            t.set_fire(who, rng.range(0, cap), cap);
        }
    }
    // A couple of hands so `hand` / `draw.pile` / `discard.pile` diffs have
    // somewhere to land.
    let all = all_card_ids();
    if !all.is_empty() {
        for who in 0..n {
            let mut hand: Vec<String> = vec![];
            for _ in 0..rng.range(1, 3) {
                hand.push(all[rng.pick_idx(all.len())].clone());
            }
            t.give(who, &hand.iter().map(String::as_str).collect::<Vec<_>>());
        }
    }
    // Loaded dice derived from the seed so rolls are reproducible.
    let faces: Vec<i32> = (0..6).map(|_| rng.range(1, 6)).collect();
    t.dice(&faces);
    t.begin_turn(0);
    t
}

/// Decline every open prompt so the table is at rest and the next diff is
/// attributable to the action we are about to take.
pub fn quiet(t: &mut Table) {
    for _ in 0..64 {
        t.settle();
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// Answer every open prompt deterministically. Used by the pairwise driver
/// so two runs from the same seed take the same branch.
///
/// Optional offers (`ask.buy` / `ask.build` / [反击] windows) are declined
/// -- a driver that always presses "buy" turns every settle into a purchase
/// and the footprint audit then blames the rule for the tile's price. A
/// forced buy (`ask.force_buy`) is taken. Everything else takes option 0.
pub fn drain_first(t: &mut Table) -> Result<(), String> {
    for _ in 0..200 {
        t.settle();
        let Some(p) = t.prompt() else {
            return Ok(());
        };
        let asked = t.asked();
        if asked.is_empty() {
            return Ok(());
        }
        let title = p.title.key().to_string();
        let decline = title == "ask.counteract.title"
            || title.starts_with("ask.buy.")
            || title.starts_with("ask.build.")
            || title == "ask.circle.title";
        let force = title.starts_with("ask.force_buy.");
        for who in asked {
            let v = if p.kind == "tile" {
                if decline && !force {
                    p.items.len() as i32
                } else {
                    0
                }
            } else if p.kind == "auction" {
                -1
            } else if p.kind == "pick" || p.kind == "mortgage" {
                0
            } else if decline && !force {
                p.fallback
            } else {
                0
            };
            let mut msg = game_core::net::NetMessage {
                prompt: p.id,
                value: v,
                ..game_core::net::NetMessage::act("answer")
            };
            if p.kind == "pick" || p.kind == "mortgage" {
                msg.cards = p.items.iter().take(p.count.max(0) as usize).cloned().collect();
            }
            t.m.act(who as i32 + 1, &msg)
                .map_err(|e| format!("answer: {}", e.key()))?;
            t.settle();
        }
    }
    Err("drain_first: prompts never stopped".into())
}

// ---------------------------------------------------------------- runtime

/// `Q4_SEEDS` -- arrangement seeds per rule (default 2).
pub fn seeds_per_rule() -> u64 {
    std::env::var("Q4_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2)
}

/// `Q4_PAIRS` -- how many disjoint pairs to sample (default 40).
pub fn pair_sample() -> usize {
    std::env::var("Q4_PAIRS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(40)
}

/// `Q4_SAMPLE` -- how many rules the single-rule audit samples (default 60;
/// 0 = every rule).
pub fn single_sample() -> usize {
    std::env::var("Q4_SAMPLE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60)
}

/// A finding's minimal repro line.
pub fn repro_line(what: &str, seed: u64, extra: &str) -> String {
    format!("seed={seed} {extra} :: {what}")
}