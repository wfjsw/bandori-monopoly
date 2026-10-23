//! Arrangement generator + action driver, adapted from
//! `crates/game-rules/tests/fuzz/` (which stays untouched).
//!
//! Differences from the original:
//!
//! * Every player input goes through [`spec_kit::observe::observe`] (via
//!   [`crate::table::Table`]), so a soak checks the armed clauses after every
//!   act. The **arranged** start state is never checked -- `gen::arrange`
//!   writes non-canonical states on purpose (random deeds, random statuses),
//!   and only the transitions out of it are observations.
//! * `check_money` / `check_nonneg` / `check_cards` are not duplicated here.
//!   The book clauses that they approximated live in
//!   [`crate::global`]; a soak arms those via `CLAUSES`.
//! * No known-findings skip list. A discrepancy is a plain failing soak.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use spec_kit::{Clause, Rng, StepAct, Violation};

use crate::table::Table;
use crate::{characters, rules};

// ---------------------------------------------------------------------------
// footprints bias (optional surface pairing)
// ---------------------------------------------------------------------------

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

/// Footprints from `docs/rulebook/footprints.json`. Missing file = empty maps
/// (the generator then draws uniformly).
pub fn footprints() -> &'static Footprints {
    FOOTPRINTS.get_or_init(|| {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/rulebook/footprints.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Footprints {
                by_rule: BTreeMap::new(),
                writers_of: BTreeMap::new(),
            };
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Footprints {
                by_rule: BTreeMap::new(),
                writers_of: BTreeMap::new(),
            };
        };
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
/// `TEST:*` fixtures are excluded -- they are harness probes, not the pool.
pub fn all_card_ids() -> &'static Vec<String> {
    static IDS: OnceLock<Vec<String>> = OnceLock::new();
    IDS.get_or_init(|| {
        let mut v: Vec<String> = rules()
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

// ---------------------------------------------------------------------------
// arrange
// ---------------------------------------------------------------------------

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
///
/// **The arranged start state is never checked.** Only the transitions out of
/// it are observations. Arm clauses *after* this returns.
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
            // Never arrange a resource above its cap -- that would be our bug,
            // not the engine's. Cap first, then fill within it.
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

// ---------------------------------------------------------------------------
// drive
// ---------------------------------------------------------------------------

/// One recorded answer, enough to replay.
#[derive(Clone, Debug, PartialEq)]
pub struct AnswerRec {
    pub who: usize,
    pub kind: String,
    pub value: i32,
    pub cards: Vec<String>,
}

/// Per-run bookkeeping.
pub struct Trace {
    pub answers: Vec<AnswerRec>,
    pub cards_touched: Vec<String>,
    pub turns: u32,
    pub acts: u32,
    pub prompts: u32,
    pub counteracts: u32,
}

impl Trace {
    pub fn new() -> Self {
        Self {
            answers: vec![],
            cards_touched: vec![],
            turns: 0,
            acts: 0,
            prompts: 0,
            counteracts: 0,
        }
    }
}

/// How the driver should pick answers: free-running, or replaying a recording.
pub enum AnswerMode {
    Free,
    Replay(Vec<AnswerRec>),
}

/// Drive `turns` turns from the table's current position. Returns the trace.
///
/// `rng` drives *action* choice only; answers use a parallel stream derived
/// from the same seed so a replayed run consumes action randomness identically.
/// Every input goes through the table's observer (so the armed clauses fire).
pub fn drive(t: &mut Table, rng: &mut Rng, turns: u32, mode: AnswerMode) -> Result<Trace, String> {
    let mut tr = Trace::new();
    let mut replay = match mode {
        AnswerMode::Free => None,
        AnswerMode::Replay(v) => Some(v.into_iter()),
    };
    let mut ans_rng = Rng::new(rng.next_u64() ^ 0x4E53_5752);

    for _turn in 0..turns {
        tr.turns += 1;
        t.settle();
        drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
        let acts = rng.range(3, 8) as u32;
        for _ in 0..acts {
            if t.st().phase != "play" {
                break;
            }
            if t.prompt().is_some() {
                drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
                continue;
            }
            try_one_act(t, rng, &mut tr)?;
            tr.acts += 1;
            drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
        }

        // Close the turn.
        if t.prompt().is_none() {
            let who = t.turn();
            let _ = t.end(who);
        }
        drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
    }
    drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
    Ok(tr)
}

/// Answer every open prompt with a legal (possibly random) answer.
fn drain_prompts(
    t: &mut Table,
    rng: &mut Rng,
    tr: &mut Trace,
    replay: &mut Option<std::vec::IntoIter<AnswerRec>>,
) -> Result<(), String> {
    let mut hist: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    for _ in 0..400 {
        t.settle();
        let Some(p) = t.prompt() else { return Ok(()) };
        let asked = t.asked();
        if asked.is_empty() {
            return Ok(());
        }
        for who in asked {
            tr.prompts += 1;
            let st = t.st();
            hist.push_back(format!(
                "{}|{}|opts={} kind={} who={} turn={} step={} phase={} pid={}",
                p.title.key(),
                p.text.key(),
                p.options.len(),
                p.kind,
                who,
                st.turn,
                st.step,
                st.phase,
                p.id,
            ));
            while hist.len() > 24 {
                hist.pop_front();
            }
            let rec = match replay {
                Some(it) => it
                    .next()
                    .ok_or_else(|| "replay: answer sequence exhausted".to_string())?,
                None => {
                    let r = pick_answer(t, who, rng);
                    if p.title.key() == "ask.counteract.title" {
                        tr.counteracts += 1;
                    }
                    r
                }
            };
            apply_answer(t, who, &rec)?;
            tr.answers.push(rec);
        }
    }
    Err(format!(
        "drain_prompts: prompts never stopped (last: {}; hist: {:?})",
        t.prompt()
            .map(|p| p.title.key().to_string())
            .unwrap_or_default(),
        hist.into_iter().collect::<Vec<_>>(),
    ))
}

/// A legal random answer for `who` on the open prompt.
fn pick_answer(t: &Table, who: usize, rng: &mut Rng) -> AnswerRec {
    let p = t.prompt().expect("pick_answer with no prompt");
    let kind = p.kind.clone();
    match kind.as_str() {
        "tile" => {
            let v = rng.below(p.items.len() as u64 + 1) as i32;
            AnswerRec {
                who,
                kind,
                value: v,
                cards: vec![],
            }
        }
        "pick" => {
            let n = p.count.max(0) as usize;
            let mut cards = p.items.clone();
            rng.shuffle(&mut cards);
            cards.truncate(n.min(cards.len()));
            AnswerRec {
                who,
                kind,
                value: 0,
                cards,
            }
        }
        "mortgage" => {
            // Always take every offered deed: the engine only lists deeds the
            // player may mortgage, and `raise_funds` only opens this prompt
            // when their combined half-price covers `need`.
            AnswerRec {
                who,
                kind,
                value: 0,
                cards: p.items.clone(),
            }
        }
        "auction" => {
            let pass = rng.chance(70);
            AnswerRec {
                who,
                kind,
                value: if pass { -1 } else { 100 },
                cards: vec![],
            }
        }
        _ => {
            let n = p.options.len().max(1) as i32;
            let is_counter = p.title.key() == "ask.counteract.title";
            let skip = if is_counter { rng.chance(55) } else { false };
            let v = if skip {
                p.fallback
            } else {
                rng.below(n as u64) as i32
            };
            AnswerRec {
                who,
                kind,
                value: v,
                cards: vec![],
            }
        }
    }
}

fn apply_answer(t: &mut Table, who: usize, rec: &AnswerRec) -> Result<(), String> {
    let p = t.prompt().ok_or("apply_answer: prompt gone")?;
    let is_decline = rec.value == p.fallback
        && (p.title.key() == "ask.counteract.title"
            || p.kind == "tile" && rec.value as usize >= p.items.len());
    if is_decline {
        return t
            .decline_one(who, p.id, rec.value)
            .map_err(|e| format!("decline {:?}: {}", rec, e));
    }
    let act = StepAct::Answer {
        who: who as i32 + 1,
        prompt: p.id,
        value: rec.value,
        cards: if rec.cards.is_empty() && (rec.kind == "pick" || rec.kind == "mortgage") {
            p.items
                .iter()
                .take(p.count.max(0) as usize)
                .cloned()
                .collect()
        } else {
            rec.cards.clone()
        },
    };
    t.observed(act)
        .map_err(|e| format!("answer {:?}: {}", rec, e))
}

/// Try one random game action for the current player (or a card play from any
/// hand). Errors are expected and ignored -- the driver only wants legal ones
/// to stick.
fn try_one_act(t: &mut Table, rng: &mut Rng, tr: &mut Trace) -> Result<(), String> {
    let who = t.turn();
    let n = t.n;
    match rng.below(8) {
        0 => {
            let hand = t.hand(who);
            if let Some(c) = rng.pick(&hand) {
                let c = c.clone();
                let _ = t.play(who, &c);
                tr.cards_touched.push(c);
            }
        }
        1 => {
            let skills = t.skills(who);
            if let Some(s) = rng.pick(&skills) {
                let s = s.clone();
                let _ = t.skill(who, &s);
                tr.cards_touched.push(s);
            }
        }
        2 => {
            let _ = t.roll(who);
        }
        3 => {
            let _ = t.buy(who);
        }
        4 => {
            let _ = t.build(who);
        }
        5 => {
            let _ = t.end(who);
        }
        6 => {
            let tiles = t.m.world().st.owners.len();
            let tile = rng.pick_idx(tiles.max(1));
            if t.mortgaged(tile) {
                let _ = t.redeem(who, tile);
            } else {
                let _ = t.mortgage(who, tile);
            }
        }
        _ => {
            let all = all_card_ids();
            if let Some(c) = rng.pick(all) {
                let c = c.clone();
                t.give(who, &[&c]);
                let _ = t.play(who, &c);
                tr.cards_touched.push(c);
            }
            if rng.chance(25) {
                let other = rng.pick_idx(n);
                if other != who {
                    let hand = t.hand(other);
                    if let Some(c) = rng.pick(&hand) {
                        let c = c.clone();
                        let _ = t.play(other, &c);
                        tr.cards_touched.push(c);
                    }
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// soak
// ---------------------------------------------------------------------------

/// Run `iters` random arrangements + a few turns each, with `clauses` armed on
/// every observed input. Returns `Ok(())` when every case is clean; `Err`
/// names the seed, the iteration, and the first violation.
///
/// `FUZZ_ITERS` overrides `iters` when set. The arranged start state is never
/// checked -- only the transitions out of it.
pub fn soak(clauses: &[Clause], iters: u32) -> Result<(), String> {
    let iters: u32 = std::env::var("FUZZ_ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(iters);
    for i in 0..iters {
        // A fresh, printable seed per iteration so a failure is replayable.
        let seed = 0x5EED_0000_0000_0000u64
            .wrapping_add(i as u64)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        if let Err(e) = soak_one(seed, clauses) {
            return Err(format!("soak case {i}/{iters} (seed {seed:#x}): {e}"));
        }
    }
    Ok(())
}

/// One soak case: arrange (unchecked), then drive a few turns with `clauses`
/// armed. A [`Violation`] becomes an `Err` naming the clause.
pub fn soak_one(seed: u64, clauses: &[Clause]) -> Result<(), String> {
    let (mut t, _played) = arrange(seed);
    // Arm *after* arrange: the arranged start state is not a transition.
    t.set_clauses(clauses);
    let mut rng = Rng::new(seed ^ 0xA5A5);
    let tr = drive(&mut t, &mut rng, 3, AnswerMode::Free)?;
    let _ = tr;
    Ok(())
}

/// Like [`soak_one`], but returns the first violation instead of panicking /
/// erroring through the observer. Used when a caller wants to collect findings
/// rather than fail fast. The table is built with checking off and the
/// violations are gathered by a custom observe pass -- for the simple case,
/// just use [`soak_one`] and catch the panic, or arm the clauses and let
/// [`Table::observe`] panic with the full [`Violation`] display.
pub fn soak_one_violations(seed: u64, clauses: &[Clause]) -> Vec<Violation> {
    // The observer panics on violation via the table; catch it and parse is
    // lossy. Instead drive with checking off and re-check each step is not
    // available through the table -- so this helper just runs `soak_one` and
    // reports an empty vec when clean. Prefer `soak_one` + panic for tests.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| soak_one(seed, clauses))) {
        Ok(Ok(())) => vec![],
        Ok(Err(e)) => vec![Violation {
            id: "SOAK",
            quote: "",
            detail: e,
            event_keys: vec![],
        }],
        Err(panic) => {
            let what = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "soak panicked".to_string());
            vec![Violation {
                id: "SOAK",
                quote: "",
                detail: what,
                event_keys: vec![],
            }]
        }
    }
}
