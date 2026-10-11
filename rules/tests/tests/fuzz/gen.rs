//! Arrangement generator: random table + footprint-biased card pairs.
//!
//! Bias source: `docs/rulebook/footprints.json` (`{rule_id: {writes, reads,
//! triggers}}`). Two cards that share a `writes` surface are far more likely to
//! interact, so the generator pairs them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use crate::common::{characters, Table};
use super::rng::Rng;

/// One rule's footprint.
#[derive(Clone, Debug, Default)]
pub struct Footprint {
    pub writes: Vec<String>,
    pub reads: Vec<String>,
    pub triggers: Vec<String>,
}

/// Surface -> rules that write it, plus the reverse map.
pub struct Footprints {
    pub by_rule: BTreeMap<String, Footprint>,
    pub writers_of: BTreeMap<String, Vec<String>>,
}

static FOOTPRINTS: OnceLock<Footprints> = OnceLock::new();

pub fn footprints() -> &'static Footprints {
    FOOTPRINTS.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/rulebook/footprints.json");
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

/// The full card id list from the shipped manifest (runtime introspection).
/// `TEST:*` fixtures are excluded -- they are harness probes (one of them is
/// deliberately infinitely recursive) and not part of the card pool.
pub fn all_card_ids() -> &'static Vec<String> {
    static IDS: OnceLock<Vec<String>> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut v: Vec<String> = crate::common::rules()
            .ruleset()
            .cards()
            .iter()
            .map(|c| c.id.clone())
            .filter(|id| !id.starts_with("TEST:"))
            .collect();
        v.sort();
        v.dedup();
        v
    })
}

/// Cards that share at least one `writes` surface with `card`.
pub fn share_surface(card: &str) -> Vec<String> {
    let fp = footprints();
    let Some(me) = fp.by_rule.get(card) else {
        return vec![];
    };
    let mut out = BTreeSet::new();
    for s in &me.writes {
        if let Some(ws) = fp.writers_of.get(s) {
            for w in ws {
                if w != card {
                    out.insert(w.clone());
                }
            }
        }
    }
    out.into_iter().collect()
}

/// Pick a partner for `card` biased toward shared surfaces (falling back to a
/// uniform draw over the full set).
pub fn biased_partner(rng: &mut Rng, card: &str) -> String {
    let shared = share_surface(card);
    if !shared.is_empty() && rng.chance(75) {
        if let Some(p) = rng.pick(&shared) {
            return p.clone();
        }
    }
    let all = all_card_ids();
    if let Some(p) = rng.pick(all) {
        return p.clone();
    }
    card.to_string()
}

/// Drain any open prompt by declining it (so the world seam stays usable).
fn quiet(t: &mut Table) {
    for _ in 0..64 {
        t.settle();
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// The random arrangement: table of 2-5 players, random characters with skills
/// bound, hands/fields stuffed with footprint-biased cards, random board state.
/// Leaves the table at rest (no open prompt, no pending routine).
pub fn arrange(seed: u64) -> (Table, Vec<String>) {
    let mut rng = Rng::new(seed);
    let n = rng.range(2, 5) as usize;
    let chars = characters();
    let mut chosen: Vec<String> = vec![];
    let mut pool: Vec<usize> = (0..chars.len()).collect();
    rng.shuffle(&mut pool);
    for &i in pool.iter().take(n) {
        chosen.push(chars[i].clone());
    }
    let refs: Vec<&str> = chosen.iter().map(String::as_str).collect();
    let mut t = Table::with_seed(&refs, seed);
    quiet(&mut t);

    // ---- cards: deal into hands and play some onto fields -----------------
    let all = all_card_ids();
    let mut played: Vec<String> = vec![];
    if !all.is_empty() {
        // Seed the first draw from the full set; bias the rest toward its
        // surface partners so interactions actually meet.
        let first = all[rng.pick_idx(all.len())].clone();
        for who in 0..n {
            let hand_n = rng.range(0, 4);
            let mut hand: Vec<String> = vec![];
            for k in 0..hand_n {
                let c = if k == 0 {
                    first.clone()
                } else {
                    biased_partner(&mut rng, &first)
                };
                hand.push(c.clone());
            }
            t.give(who, &hand.iter().map(String::as_str).collect::<Vec<_>>());
            played.extend(hand.iter().cloned());
            // Play a couple onto the field (best-effort; a rejected play is fine).
            for _ in 0..rng.range(0, 2) {
                if hand.is_empty() {
                    break;
                }
                let c = hand[rng.pick_idx(hand.len())].clone();
                let _ = t.play(who, &c);
                quiet(&mut t);
                played.push(c.clone());
            }
        }
        // A few extra surface-biased gifts so fields are not empty.
        for who in 0..n {
            let extra = biased_partner(&mut rng, &first);
            t.give(who, &[&extra]);
            played.push(extra);
        }
    }

    // ---- board: deeds, houses, mortgages ---------------------------------
    quiet(&mut t);
    let tiles = t.m.world().st.owners.len();
    for tile in 0..tiles {
        if rng.chance(30) {
            t.set_owner(tile, Some(rng.pick_idx(n)));
        }
        if rng.chance(25) {
            t.set_houses(tile, rng.range(0, 3));
        }
        if rng.chance(15) {
            t.set_mortgaged(tile, true);
        }
    }

    // ---- statuses, crystals, fire, fans ----------------------------------
    for who in 0..n {
        if rng.chance(20) {
            t.set_state(who, "stay", rng.range(1, 2));
        }
        if rng.chance(12) {
            t.set_state(who, "stun", rng.range(1, 2));
        }
        if rng.chance(8) {
            t.set_state(who, "exile", rng.range(1, 2));
        }
        if rng.chance(25) {
            // Never arrange a fire above its cap -- that would be our bug, not
            // the engine's. Cap first, then fill within it.
            let cap = rng.range(1, 3);
            t.set_fire(who, rng.range(0, cap), cap);
        }
        if rng.chance(20) {
            t.give(who, &["通用:[衍生]FEVER!"]);
            let _ = t.play(who, "通用:[衍生]FEVER!");
            quiet(&mut t);
            played.push("通用:[衍生]FEVER!".into());
        }
    }

    // ---- loaded dice derived from the seed -------------------------------
    quiet(&mut t);
    let faces: Vec<i32> = (0..8).map(|_| rng.range(1, 6)).collect();
    t.dice(&faces);

    (t, played)
}