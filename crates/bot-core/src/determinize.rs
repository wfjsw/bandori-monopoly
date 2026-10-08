//! Hidden-state determinizer (`docs/BOT.md` §3.4).
//!
//! `determinize(view, rng) -> fork` builds a playable [`Match`] consistent
//! with one seat's [`SeatView`](crate::view::SeatView): every hidden zone is
//! *sampled*, never read. The fork is materialised through the engine's save
//! format ([`crate::saved`]) and the public [`Match::restore`].
//!
//! ## What is sampled
//!
//! | zone | source of truth in the view | sample |
//! |---|---|---|
//! | own hand | `view.hand` (exact) | none |
//! | own draw pile | `view.draw` (sorted composition) | order shuffled |
//! | own discard | `MatchPlayer.discard` (exact, public) | none |
//! | opponent hand + draw | counts only | split of (allowed contents − public sightings) |
//! | opponent discard | `MatchPlayer.discard` (exact, public) | none |
//! | event deck order | `event_deck` count only | shuffle of unseen events; `event_top` slots pinned on top |
//! | RNG | never in the view | fresh seed per fork |
//!
//! Discards are fully public in `MatchState` (`World::public_state` copies
//! `Hidden::discard` verbatim), so the only shuffle left around them is the
//! future reshuffle of an emptied draw pile -- which the fork's fresh RNG
//! decides. `event_top` carries the ids of face-down cards stacked on top of
//! the event deck; those are pinned in order and the unseen remainder under
//! them is shuffled.
//!
//! Allowed contents for an opponent (the consistency tests check exactly
//! this): `deck::pool(character) ∪ deck::derived(character)` minus every
//! public sighting (their discard + their field cards + board fields they
//! own). Mid-game created copies can make the count exceed what the starting
//! deck accounts for; the sampler then draws with replacement from the
//! allowed set and reports the shortfall in [`SampleReport`].

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use game_core::data::GameData;
use game_core::deck;
use game_core::engine::{CardRules, Hidden, Match, World};
use game_core::rng::Rng;
use game_core::state::MatchState;

use crate::saved::{self, PendingGaps, PendingMirror};
use crate::view::SeatView;

/// Why a determinization refused to build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeterminizeError {
    /// The searching seat is not in the state.
    NoSeat,
    /// `Match::restore` rejected the materialised save.
    Restore(String),
}

impl std::fmt::Display for DeterminizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeterminizeError::NoSeat => write!(f, "seat not in match state"),
            DeterminizeError::Restore(e) => write!(f, "restore failed: {e}"),
        }
    }
}

impl std::error::Error for DeterminizeError {}

/// Search-side RNG. Seeded per decision (`docs/BOT.md` §1: never the match
/// RNG); holds the determinizer's sampling stream.
pub struct DeterminizerRng {
    inner: Rng,
}

impl DeterminizerRng {
    pub fn new(seed: u64) -> Self {
        Self {
            inner: Rng::new(seed),
        }
    }
    pub fn below(&mut self, n: usize) -> usize {
        self.inner.below(n.max(1))
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        self.inner.shuffle(v);
    }
    pub fn next_u64(&mut self) -> u64 {
        self.inner.next_u64()
    }
}

/// What one determinization sampled (and where it had to degrade).
#[derive(Debug, Clone, Default)]
pub struct SampleReport {
    /// Per opponent seat: sampled hand size / draw size.
    pub opponent_zones: BTreeMap<usize, (usize, usize)>,
    /// Opponent seats whose allowed set was smaller than hand+draw, so the
    /// sampler had to draw with replacement (derived / created copies).
    pub short_seats: Vec<usize>,
    /// Unseen event ids placed under the pinned `event_top` slots.
    pub event_deck_len: usize,
    /// `event_top` ids pinned on top of the event deck.
    pub event_top: Vec<String>,
    /// The stubbed `Pending` gaps, when a prompt was open.
    pub pending: Option<PendingGaps>,
    /// Fresh match-RNG seed the fork was given.
    pub seed: u64,
}

/// Determinize: sample every hidden zone and materialise a playable fork.
///
/// The fork reproduces `view` exactly for the searching seat (same public
/// state, same own hand / draw composition, same hidden-zone counts) -- the
/// invariant the consistency tests pin down.
pub fn determinize(
    view: &SeatView,
    data: &Arc<GameData>,
    rules: &Arc<dyn CardRules>,
    rng: &mut DeterminizerRng,
) -> Result<(Match, SampleReport), DeterminizeError> {
    let (world, mut report) = sample_world(view, data.as_ref(), rng)?;
    let pending = build_pending(view, data.as_ref(), &world);
    let live_seed = rng.next_u64();
    let saved = saved::saved(
        world,
        pending.clone().map(|p| p.0),
        Rng::new(live_seed),
        view.state.seq,
    );
    report.seed = live_seed;
    if let Some((_, gaps)) = pending {
        report.pending = Some(gaps);
    }
    // In-memory fork materialisation (`docs/BOT.md` B2/B4): build the save
    // document as a value and hand it to `restore_value`, skipping the string
    // encode + parse the old `to_json` + `restore` pair paid (~10 ms per
    // iteration). `Match::fork` is the cheap clone for the iterations after
    // this one.
    let v = saved.to_value();
    let m = Match::restore_value(data.clone(), rules.clone(), v)
        .map_err(|e| DeterminizeError::Restore(format!("{:?}", e)))?;
    Ok((m, report))
}

/// The save document one [`determinize`] would materialise, as a value -- for
/// a caller that wants to edit it (the searching seat's `ai` flag) before
/// [`game_core::engine::Match::restore_value`]. Skips the string round trip.
pub fn determinize_value(
    view: &SeatView,
    data: &GameData,
    rng: &mut DeterminizerRng,
) -> Result<(serde_json::Value, SampleReport), DeterminizeError> {
    let (world, mut report) = sample_world(view, data, rng)?;
    let pending = build_pending(view, data, &world);
    let live_seed = rng.next_u64();
    let saved = saved::saved(
        world,
        pending.clone().map(|p| p.0),
        Rng::new(live_seed),
        view.state.seq,
    );
    report.seed = live_seed;
    if let Some((_, gaps)) = pending {
        report.pending = Some(gaps);
    }
    Ok((saved.to_value(), report))
}

/// The save JSON one [`determinize`] would materialise -- for tests and
/// debugging without restoring a `Match`.
pub fn determinize_json(
    view: &SeatView,
    data: &GameData,
    rng: &mut DeterminizerRng,
) -> Result<(String, SampleReport), DeterminizeError> {
    let (v, report) = determinize_value(view, data, rng)?;
    Ok((v.to_string(), report))
}

fn build_pending(
    view: &SeatView,
    data: &GameData,
    world: &World,
) -> Option<(PendingMirror, PendingGaps)> {
    if view.state.prompt.id == 0 {
        return None;
    }
    let turn = view.state.turn.max(0) as usize;
    let ai_self = view.ai_answer.as_ref().map(|a| (view.player_id, a));
    Some(saved::stub_pending(
        data,
        world,
        &view.state.prompt,
        turn,
        ai_self.as_ref().map(|(p, a)| (*p, *a)),
    ))
}

/// Sample every hidden zone and assemble the fork's [`World`].
fn sample_world(
    view: &SeatView,
    data: &GameData,
    rng: &mut DeterminizerRng,
) -> Result<(World, SampleReport), DeterminizeError> {
    let st = &view.state;
    let me = view.player_id;
    if me < 0 || me as usize >= st.players.len() {
        return Err(DeterminizeError::NoSeat);
    }
    let me = me as usize;
    let mut report = SampleReport::default();

    // ---- per-seat hidden zones ------------------------------------------
    let mut hidden: Vec<Hidden> = Vec::with_capacity(st.players.len());
    for (i, p) in st.players.iter().enumerate() {
        let discard = p.discard.clone();
        if i == me {
            // Own zones: composition known, order of the draw pile is not.
            let mut draw = view.draw.clone();
            rng.shuffle(&mut draw);
            hidden.push(Hidden {
                hand: view.hand.clone(),
                draw,
                discard,
                next_steps: None,
            });
        } else {
            let (hand, draw, short) = sample_opponent(data, st, i, rng);
            report.opponent_zones.insert(i, (hand.len(), draw.len()));
            if short {
                report.short_seats.push(i);
            }
            hidden.push(Hidden {
                hand,
                draw,
                discard,
                next_steps: None,
            });
        }
    }

    // ---- event deck ------------------------------------------------------
    let (event_deck, event_top) = sample_event_deck(data, st, rng);
    report.event_deck_len = event_deck.len();
    report.event_top = event_top.clone();

    // ---- world assembly --------------------------------------------------
    let mut st = st.clone();
    for (p, h) in st.players.iter_mut().zip(&hidden) {
        p.hand = h.hand.len() as i32;
        p.draw = h.draw.len() as i32;
        p.discard = h.discard.clone();
    }
    st.event_top = event_top;

    let next_event = st.events.iter().map(|e| e.id).max().unwrap_or(0) + 1;
    let ask_seq = st.prompt.id;

    let mut world = World::new(st, hidden.len(), rng.next_u64());
    world.hidden = hidden;
    for e in view.state.events.iter() {
        world.recent.push_back(e.clone());
    }
    world.next_event = next_event;
    world.event_deck = event_deck;
    world.event_discard = view.state.event_discard.clone();
    world.event_removed = view.state.event_removed.clone();
    world.ask_seq = ask_seq;
    world.turn.player_id = view.state.turn.max(0) as usize;
    Ok((world, report))
}

/// Public sightings of `seat`'s cards: discard + their field + board fields
/// they own. Everything a player can see, so the unknown set never holds them.
fn sightings(st: &MatchState, seat: usize) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = BTreeSet::new();
    if let Some(p) = st.players.get(seat) {
        for c in &p.discard {
            s.insert(c.clone());
        }
        for f in &p.field {
            s.insert(f.card.clone());
        }
    }
    for f in &st.board_field {
        if f.owner == seat as i32 {
            s.insert(f.card.clone());
        }
    }
    s
}

/// Sample an opponent's hand + draw from allowed contents minus sightings.
/// Returns `(hand, draw, had_to_pad)`.
fn sample_opponent(
    data: &GameData,
    st: &MatchState,
    seat: usize,
    rng: &mut DeterminizerRng,
) -> (Vec<String>, Vec<String>, bool) {
    let p = &st.players[seat];
    let hand_n = p.hand.max(0) as usize;
    let draw_n = p.draw.max(0) as usize;
    let need = hand_n + draw_n;
    if need == 0 {
        return (Vec::new(), Vec::new(), false);
    }

    // Allowed: what their deck rules allow (pool + mid-game derived), minus
    // every public sighting.
    let mut allowed: Vec<String> = Vec::new();
    if let Some(c) = data.character(&p.character) {
        for k in deck::pool(data, c) {
            allowed.push(k.id.clone());
        }
        for k in deck::derived(data, c) {
            allowed.push(k.id.clone());
        }
    }
    let seen = sightings(st, seat);
    allowed.retain(|id| !seen.contains(id));
    allowed.sort();
    allowed.dedup();

    let mut pool = allowed.clone();
    let mut picked: Vec<String> = Vec::with_capacity(need);
    let mut short = false;
    if pool.is_empty() {
        // Nothing legal left (every card seen / unknown character). The counts
        // still have to hold; fill with synthetic ids the engine tolerates
        // (a card no rule knows is a no-op draw). Mark as degraded.
        short = true;
        while picked.len() < need {
            picked.push(format!("bot-core:unknown{}", picked.len()));
        }
    } else {
        while picked.len() < need {
            if pool.is_empty() {
                // Created copies: sample with replacement.
                short = true;
                let k = rng.below(allowed.len());
                picked.push(allowed[k].clone());
            } else {
                let k = rng.below(pool.len());
                picked.push(pool.remove(k));
            }
        }
    }
    let hand = picked[..hand_n.min(picked.len())].to_vec();
    let draw = picked[hand_n.min(picked.len())..].to_vec();
    (hand, draw, short)
}

/// Unseen events under the pinned `event_top` slots.
fn sample_event_deck(
    data: &GameData,
    st: &MatchState,
    rng: &mut DeterminizerRng,
) -> (Vec<String>, Vec<String>) {
    let all: BTreeSet<String> = data
        .events
        .iter()
        .filter(|e| !e.derived)
        .map(|e| e.id.clone())
        .collect();
    let mut gone: BTreeSet<String> = BTreeSet::new();
    for id in &st.event_discard {
        gone.insert(id.clone());
    }
    for id in &st.event_removed {
        gone.insert(id.clone());
    }
    for e in &st.event_active {
        gone.insert(e.id.clone());
    }
    // `event_top` ids are IN the deck (stacked face-down on top) -- they are
    // not gone; they are pinned.
    let top: Vec<String> = st.event_top.clone();
    for id in &top {
        gone.remove(id);
    }
    let mut unseen: Vec<String> = all.into_iter().filter(|id| !gone.contains(id)).collect();
    // The pinned slots sit on top (drawn first = end of the vec). The unseen
    // remainder under them is shuffled.
    unseen.retain(|id| !top.contains(id));
    rng.shuffle(&mut unseen);
    for id in &top {
        unseen.push(id.clone());
    }
    (unseen, top)
}