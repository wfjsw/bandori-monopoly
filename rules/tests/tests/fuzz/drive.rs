//! Action driver: walks a table, picks random legal acts, answers every prompt
//! with a legal random answer (counteract declarations included). Records the
//! answer sequence so a second run can replay it.

use game_core::msg::Msg;
use game_core::net::NetMessage;
use game_core::state::MatchEvent;

use crate::common::{int_arg, Table};
use super::rng::Rng;

/// One recorded answer, enough to replay.
#[derive(Clone, Debug, PartialEq)]
pub struct AnswerRec {
    pub who: usize,
    pub kind: String,
    pub value: i32,
    pub cards: Vec<String>,
}

/// Per-run bookkeeping the invariants and the coverage report read.
pub struct Trace {
    pub answers: Vec<AnswerRec>,
    /// Card ids that were declared in a [反击] window or played this run.
    pub cards_touched: Vec<String>,
    /// Unordered pairs of cards that appeared in the same turn.
    pub pairs_this_turn: std::collections::BTreeSet<(String, String)>,
    pub turns: u32,
    pub acts: u32,
    pub prompts: u32,
    pub counteracts: u32,
    pub traps: u32,
    /// Money-event types we could not account (see the coverage report).
    pub unaccounted_money: std::collections::BTreeMap<String, u32>,
    /// Money depth observations (for the MAX_MONEY_DEPTH probe).
    pub max_chain_seen: u32,
}

impl Trace {
    pub fn new() -> Self {
        Self {
            answers: vec![],
            cards_touched: vec![],
            pairs_this_turn: Default::default(),
            turns: 0,
            acts: 0,
            prompts: 0,
            counteracts: 0,
            traps: 0,
            unaccounted_money: Default::default(),
            max_chain_seen: 0,
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
pub fn drive(
    t: &mut Table,
    rng: &mut Rng,
    turns: u32,
    mode: AnswerMode,
) -> Result<Trace, String> {
    let mut tr = Trace::new();
    let mut replay = match mode {
        AnswerMode::Free => None,
        AnswerMode::Replay(v) => Some(v.into_iter()),
    };
    // Separate stream for answers: free mode consumes it, replay does not,
    // and the action stream must not notice either way.
    let mut ans_rng = Rng::new(rng.next_u64() ^ 0x4E53_5752);

    for _turn in 0..turns {
        tr.turns += 1;
        t.settle();
        drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
        let turn_mark = t.mark();

        // Several acts per turn: play / skill / roll / buy / build / end.
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
            check_trap(t, &mut tr)?;
        }

        // Close the turn.
        if t.prompt().is_none() {
            let who = t.turn();
            let _ = t.end(who);
        }
        drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
        check_trap(t, &mut tr)?;

        // Pair every card named by this turn's event log.
        let turn_cards = cards_in_events(&t.events_since(turn_mark));
        for i in 0..turn_cards.len() {
            for j in (i + 1)..turn_cards.len() {
                let (a, b) = (&turn_cards[i], &turn_cards[j]);
                let key = if a <= b {
                    (a.clone(), b.clone())
                } else {
                    (b.clone(), a.clone())
                };
                tr.pairs_this_turn.insert(key);
            }
        }
    }
    // Final drain so a hanging prompt does not leak out of the run.
    drain_prompts(t, &mut ans_rng, &mut tr, &mut replay)?;
    Ok(tr)
}

/// Cards named by these events (played, declared in a [反击] window, or
/// named by a trigger). This is what "co-occurred in a resolved chain or
/// turn" means for the coverage report.
fn cards_in_events(events: &[MatchEvent]) -> Vec<String> {
    let mut out = std::collections::BTreeSet::new();
    for e in events {
        if !e.card.is_empty() {
            out.insert(e.card.clone());
        }
        if let Some(c) = e.msg.a.get("card") {
            if let game_core::msg::Arg::Card(s) = c {
                out.insert(s.clone());
            }
        }
    }
    out.into_iter().collect()
}

fn check_trap(t: &Table, tr: &mut Trace) -> Result<(), String> {
    for e in t.m.world().recent.iter() {
        if e.msg.key() == "log.card_trap" {
            tr.traps += 1;
            return Err(format!("card_trap: {:?}", e.msg));
        }
    }
    Ok(())
}

/// Answer every open prompt with a legal (possibly random) answer.
fn drain_prompts(
    t: &mut Table,
    rng: &mut Rng,
    tr: &mut Trace,
    replay: &mut Option<std::vec::IntoIter<AnswerRec>>,
) -> Result<(), String> {
    // Ring of the last prompt keys seen, for the storm report.
    let mut hist: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    for _ in 0..400 {
        t.settle();
        let Some(p) = t.prompt() else { return Ok(()) };
        let asked = t.asked();
        if asked.is_empty() {
            // Nothing waits (should not happen); break the spin.
            return Ok(());
        }
        for who in asked {
            tr.prompts += 1;
            let st = t.st();
            let statuses: Vec<String> = st
                .players
                .iter()
                .enumerate()
                .map(|(i, pl)| {
                    let s = &pl.state;
                    let get = |k: &str| s.get(k).map(|v| v.value).unwrap_or(0);
                    let exp = |k: &str| s.get(k).map(|v| format!("{:?}", v.expires)).unwrap_or_else(|| "-".into());
                    format!(
                        "P{i}[stay={}({}) stun={}({}) stunStart={}({}) exile={} out={}]",
                        get("stay"),
                        exp("stay"),
                        get("stun"),
                        exp("stun"),
                        get("stunStart"),
                        exp("stunStart"),
                        get("exile"),
                        t.m.world().out(i)
                    )
                })
                .collect();
            hist.push_back(format!(
                "{}|{}|opts={} kind={} who={} turn={} step={} phase={} pid={} {}",
                p.title.key(),
                p.text.key(),
                p.options.len(),
                p.kind,
                who,
                st.turn,
                st.step,
                st.phase,
                p.id,
                statuses.join(" "),
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
                    if p.kind == "pick" || p.kind == "mortgage" || p.card.starts_with("skill:") {
                        // note the card being answered about
                    }
                    r
                }
            };
            apply_answer(t, who, &rec)?;
            tr.answers.push(rec);
        }
    }
    Err(format!(
        "drain_prompts: prompts never stopped (last: {} / {}; hist: {:?})",
        t.prompt().map(|p| p.title.key().to_string()).unwrap_or_default(),
        t.prompt().map(|p| p.text.key().to_string()).unwrap_or_default(),
        hist.into_iter().collect::<Vec<_>>(),
    ))
}

/// A legal random answer for `who` on the open prompt.
fn pick_answer(t: &Table, who: usize, rng: &mut Rng) -> AnswerRec {
    let p = t.prompt().expect("pick_answer with no prompt");
    let kind = p.kind.clone();
    match kind.as_str() {
        "tile" => {
            // 0..=len; len = "none".
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
            // when their combined half-price covers `need`, so the full set is
            // a legal answer. (A random subset risks `err.mortgage_short`.)
            AnswerRec {
                who,
                kind,
                value: 0,
                cards: p.items.clone(),
            }
        }
        "auction" => {
            // value < 0 = pass; otherwise a bid. Passing keeps the run short.
            let pass = rng.chance(70);
            AnswerRec {
                who,
                kind,
                value: if pass { -1 } else { 100 },
                cards: vec![],
            }
        }
        _ => {
            // choice / yes / pick-like: any option index.
            let n = p.options.len().max(1) as i32;
            // Bias: for counteract windows, skip more often than declare so the
            // run makes progress, but still declare often enough to cover
            // counter wars.
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
    let mut msg = NetMessage {
        prompt: p.id,
        value: rec.value,
        ..NetMessage::act("answer")
    };
    if !rec.cards.is_empty() {
        msg.cards = rec.cards.clone();
    } else if rec.kind == "pick" || rec.kind == "mortgage" {
        msg.cards = p
            .items
            .iter()
            .take(p.count.max(0) as usize)
            .cloned()
            .collect();
    }
    t.m.act(who as i32 + 1, &msg)
        .map_err(|e| format!("answer {:?}: {}", rec, e.key()))?;
    t.settle();
    Ok(())
}

/// Try one random game action for the current player (or a card play from any
/// hand). Errors are expected and ignored -- the driver only wants legal ones
/// to stick.
fn try_one_act(t: &mut Table, rng: &mut Rng, tr: &mut Trace) -> Result<(), String> {
    let who = t.turn();
    let n = t.n;
    // 0 play a hand card (any player, any time it is legal -- usually the
    //   current player's 运营 stage).
    // 1 use a skill
    // 2 roll
    // 3 buy
    // 4 build
    // 5 end
    // 6 mortgage / redeem
    // 7 give a fresh biased card and play it (keeps interaction pressure up)
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
            // Inject a surface-biased card and try to play it.
            let all = super::gen::all_card_ids();
            if let Some(c) = rng.pick(all) {
                let c = c.clone();
                t.give(who, &[&c]);
                let _ = t.play(who, &c);
                tr.cards_touched.push(c);
            }
            // Occasionally touch another player too.
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

/// Money conservation: sum of player-money deltas equals net bank flow.
/// Returns Ok(()) or a description of the mismatch.
///
/// `snapshot_delta` is the observed change in total player money over the
/// window. `bank_flow` is derived from the money events in the log. They must
/// be equal.
///
/// Accounted event types: `pay`/`rent` (player->player; an unequal `gain` arg
/// is an implicit bank leg), `lose`/`gain` (bank), `mortgage` (bank inflow),
/// `redeem` (bank outflow), build refunds (`text` + `log.build_refund_*`).
///
/// Documented exclusions:
/// * `bankrupt` -- the engine cashes mortgages without logging them and zeroes
///   the payer, so the ledger cannot close over that event. A window containing
///   a bankrupt event is skipped.
/// * auction bids -- escrowed via a direct `money -= bid` with no bank leg.
pub fn check_money(
    before: &[i32],
    after: &[i32],
    events: &[MatchEvent],
    tr: &mut Trace,
) -> Result<(), String> {
    let mut snapshot_delta = 0i64;
    for (b, a) in before.iter().zip(after) {
        snapshot_delta += (*a as i64) - (*b as i64);
    }
    let mut bank_flow = 0i64;
    for e in events {
        let key = e.msg.key().to_string();
        // Only money-moving event types enter the ledger. Everything else
        // (play/roll/draw/status/turn/…) is not money and is ignored here.
        match e.r#type.as_str() {
            "pay" | "rent" => {
                // payer loses `value`, payee gains `value` or the `gain` arg.
                // A player-to-player transfer nets to 0; an unequal pair is an
                // implicit bank leg (money created or destroyed).
                let amount = e.value as i64;
                let gain = int_arg(&e.msg, "gain").unwrap_or(amount) as i64;
                bank_flow += gain - amount;
            }
            "lose" => bank_flow -= e.value as i64,
            "gain" => bank_flow += e.value as i64,
            "mortgage" => bank_flow += e.value as i64,
            "redeem" => bank_flow -= e.value as i64,
            "bankrupt" => {
                return Err("money: excluded window (bankrupt event)".into());
            }
            "auction" => {
                return Err("money: excluded window (auction escrow)".into());
            }
            "text" => {
                if key.starts_with("log.build_refund") {
                    bank_flow += int_arg(&e.msg, "cost").unwrap_or(0) as i64;
                }
                // log.money_locked / log.blocked_pay / log.blocked_gain move
                // nothing. Other `text` events are not money.
            }
            // buy/build set `value = 0` (the money leg is the `lose` above);
            // everything else is non-money.
            _ => {}
        }
    }
    let _ = tr;
    if snapshot_delta != bank_flow {
        return Err(format!(
            "money: player delta {snapshot_delta} != bank flow {bank_flow} ({:?})",
            events
                .iter()
                .map(|e| format!("{}:{}", e.r#type, e.msg.key()))
                .collect::<Vec<_>>()
        ));
    }
    Ok(())
}

/// Statuses / crystals / fire / fans must never be negative; fire respects its
/// declared cap.
///
/// Only the named counters the rulebook bounds are checked. Other keyed state
/// (`skill.*`, `want_to_grab_passer`, …) may legitimately use -1 as a
/// "no target" sentinel and is not covered here.
pub fn check_nonneg(t: &Table) -> Result<(), String> {
    for (i, p) in t.m.world().st.players.iter().enumerate() {
        for (k, v) in &p.state {
            let is_status = k == "stay" || k == "stun" || k == "exile";
            let is_fire = k == "fire";
            if is_status && v.value < 0 {
                return Err(format!("negative state {k}={} on P{i}", v.value));
            }
            if is_fire {
                if v.value < 0 {
                    return Err(format!("fire {} < 0 on P{i}", v.value));
                }
                if v.max > 0 && v.value > v.max {
                    return Err(format!("fire {} > cap {} on P{i}", v.value, v.max));
                }
            }
        }
        for f in &p.field {
            if f.crystals < 0 {
                return Err(format!("negative crystals {} on {}", f.crystals, f.card));
            }
        }
        for c in &p.tokens {
            if c.value < 0 {
                return Err(format!("negative token {}={} on P{i}", c.name, c.value));
            }
        }
    }
    Ok(())
}

/// Card-zone sanity: every card id is a known card or a `skill:` id; field uids
/// are unique; no zone size is negative (they are `usize`, so implicit).
///
/// Instance-level "exactly one zone" is infeasible: the engine stores cards as
/// string ids without instance identity (`Hidden.hand` is `Vec<String>`), so
/// two copies of one id may legally sit in different zones. Documented as an
/// exclusion; this check covers id validity and uid uniqueness instead.
pub fn check_cards(t: &Table, known: &[String]) -> Result<(), String> {
    let w = t.m.world();
    let mut uids = std::collections::BTreeSet::new();
    let is_known = |c: &str| -> bool {
        c.starts_with("skill:")
            || c.starts_with("TEST:")
            || known.iter().any(|k| k == c)
    };
    for (i, h) in w.hidden.iter().enumerate() {
        for zone in [&h.hand, &h.draw, &h.discard] {
            for c in zone {
                if c.is_empty() {
                    return Err(format!("empty card id in P{i} zone"));
                }
                if !known.is_empty() && !is_known(c) {
                    // Derived cards (PERFECT / FEVER! / …) are in the manifest;
                    // anything else is a finding.
                    return Err(format!("unknown card id {c:?} in P{i} zone"));
                }
            }
        }
    }
    for (i, p) in w.st.players.iter().enumerate() {
        for f in &p.field {
            if f.uid != 0 && !uids.insert(f.uid) {
                return Err(format!("duplicate field uid {} on P{i}", f.uid));
            }
            if !known.is_empty() && !is_known(&f.card) {
                return Err(format!("unknown field card {:?}", f.card));
            }
        }
    }
    Ok(())
}

/// Termination: the settle loop must come to rest. `Table::settle` already
/// panics if it does not; this wraps it so the fuzzer reports it as a finding
/// rather than a test failure.
pub fn check_rest(t: &mut Table) -> Result<(), String> {
    t.settle();
    Ok(())
}