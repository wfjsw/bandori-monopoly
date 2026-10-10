//! The replayable part of a match.
//!
//! Everything a routine reads or writes lives here, including the RNG and the
//! event/prompt counters, so re-running a routine from a snapshot reproduces the same
//! dice, the same event ids and the same prompt ids. Clocks, the live prompt, auction
//! bidding and the end-match vote belong to the host ([`super::Match`]) instead.

use std::collections::VecDeque;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::msg::Msg;
use crate::rng::Rng;
use crate::state::{MatchEvent, MatchState};

/// Money each player starts with.
pub const START_MONEY: i32 = 10_000;
/// Opening hand size.
pub const START_HAND: usize = 2;
/// Base hand limit.
pub const HAND_LIMIT: usize = 5;
/// CiRCLE reward in money.
pub const CIRCLE_MONEY: i32 = 2000;
/// Events kept for `events_since`.
pub const EVENT_KEEP: usize = 400;
/// Events carried in every `MatchState` snapshot.
pub const STATE_EVENTS: usize = 80;

/// Per-player private card zones (hand / draw / discard).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hidden {
    pub hand: Vec<String>,
    /// Top of the pile is the **end** of the vec.
    pub draw: Vec<String>,
    pub discard: Vec<String>,
    /// The player's next [主要移动] walks exactly this many steps instead of
    /// the roll; consumed by that move.
    #[serde(default)]
    pub next_steps: Option<i32>,
}

fn full_build_cost() -> i32 {
    100
}

/// How a play settles its number-range dice (「以理论最大值或最小值结算」).
/// The wire encoding is the old `1` / `0` / `-1` triple; only the Rust side is
/// an enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Extreme {
    /// Settle at the theoretical minimum (the wire `-1`).
    Min,
    /// Plain rolls (the wire `0`).
    #[default]
    Plain,
    /// Settle at the theoretical maximum (the wire `1`).
    Max,
}

impl Extreme {
    /// The guest / wire encoding.
    pub fn as_i32(self) -> i32 {
        match self {
            Extreme::Min => -1,
            Extreme::Plain => 0,
            Extreme::Max => 1,
        }
    }

    /// Decode the guest / wire encoding.
    pub fn from_i32(v: i32) -> Self {
        if v > 0 {
            Extreme::Max
        } else if v < 0 {
            Extreme::Min
        } else {
            Extreme::Plain
        }
    }
}

impl std::fmt::Display for Extreme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_i32())
    }
}

impl Serialize for Extreme {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i32(self.as_i32())
    }
}

impl<'de> Deserialize<'de> for Extreme {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Extreme::from_i32(i32::deserialize(d)?))
    }
}

/// `play_doubled`'s wire shape: the old field was an `i32` with `-1` = none.
/// Serde keeps that spelling so `SAVE_VERSION` need not move.
mod opt_i32_neg1 {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<i32>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_i32(v.unwrap_or(-1))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
        let raw = i32::deserialize(d)?;
        Ok(if raw < 0 { None } else { Some(raw) })
    }
}

/// One player's turn-start snapshot: the status a turn-end undo restores to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TurnSnap {
    pub pos: i32,
    pub stay: i32,
    pub stun: i32,
    pub exile: i32,
}

/// Per-turn bookkeeping the shell uses.
///
/// The derived `Default` would zero `build_cost_pct` (only the serde default is
/// 100), so a fresh turn would build for free; the manual impl keeps full price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnCtx {
    /// The movement being planned / walked. Runtime-only: it carries roll
    /// tables, so it never reaches the wire (the broadcast summary is
    /// `MatchState::plan`).
    #[serde(skip)]
    pub plan: crate::engine::move_ctx::MoveCtx,
    pub player_id: usize,
    pub extra: bool,
    pub main_moved: bool,
    pub played: Vec<String>,
    /// Players whose money cannot drop for the rest of this turn. Auctions are
    /// not payments, so they are exempt.
    #[serde(default)]
    pub no_money_loss: Vec<usize>,
    /// This turn's [主要移动] roll is fixed to this face.
    #[serde(default)]
    pub fixed_roll: Option<i32>,
    /// Steps this turn's [主要移动] walked; 0 = none yet.
    #[serde(default)]
    pub main_steps: i32,
    /// Abnormal effects that got through to each player this turn (indexed by
    /// player; a new turn starts them all at 0).
    #[serde(default)]
    pub abnormal: Vec<i32>,
    /// The play being resolved settles its number-range dice at the theoretical
    /// extreme: `1` = max, `-1` = min, `0` = plain. Set mid-play by a [反击]
    /// (「以理论最大值或最小值结算」) and cleared when the play ends, so it does
    /// not leak into the next one.
    #[serde(default)]
    pub extreme: Extreme,
    /// Whether the play being resolved came from the hand (`false` when it came
    /// from somewhere else). 「若此卡从手牌以外的地方打出」 reads this.
    #[serde(default)]
    pub play_from_hand: bool,
    /// Seats whose designation on the play being resolved is cancelled
    /// (「取消其对目标之一的[指定]」). A per-pair cancel: the rest of the play's
    /// designations still land. Cleared at the start of each play.
    #[serde(default)]
    pub cancelled_designations: Vec<i32>,
    /// Which of the play's tagged `ctx::n` numbers the CiRCLE band skill has
    /// doubled (-1 = none). Lives on the play, not on a guest run, so a
    /// pre-effect hook can arm it for the body that follows. Cleared at the
    /// start of each play.
    #[serde(default, with = "opt_i32_neg1")]
    pub play_doubled: Option<i32>,
    /// Money paid to other players during this turn's [触发结算]s.
    /// 「本回合的[结算]向其他玩家支付了至少1000资金」.
    #[serde(default)]
    pub paid_in_settle: i32,
    /// Where each player stood when the turn started. 「在Livehouse地块开始
    /// 回合时」 is a question about that square, not the one a mid-turn walk has
    /// since reached.
    #[serde(default)]
    pub turn_start_pos: Vec<i32>,
    /// Each player's pos / [停留] / [晕眩] / [除外] when the turn started.
    /// 「回到起始地点并取消所有受到的效果」 restores from here.
    #[serde(default)]
    pub turn_snap: Vec<TurnSnap>,
    /// Every face rolled this turn, in order. 「与本回合内你骰出过的所有骰点
    /// 都不同」 compares against this.
    #[serde(default)]
    pub turn_rolls: Vec<i32>,
    /// 「下次盖房时减免N（可溢出），盖房后减少1层」. A layered cut on the build
    /// cost; each build pops one layer.
    #[serde(default)]
    pub build_discount: i32,
    pub build_discount_layers: i32,
    /// 「加盖房屋时半价」 -- the build cost as a percentage of its table price.
    /// 100 = full, 50 = half, 0 = free (「本回合加盖房屋变为免费」).
    #[serde(default = "full_build_cost")]
    pub build_cost_pct: i32,
    /// Turn-scoped lingering card instances (`docs/PURCHASE.md` P5). Each entry
    /// is `(card_id, owner, expires_turn)`; the entry lives until `expires_turn`
    /// and is cleared at turn start. This is the hand-card home for 「本回合」
    /// effects: a card that wants 「本回合购买格子时[消耗]资金降低N」 lingers with a
    /// `BuyAdd` hook on its own def, and one that wants 「本回合无法加盖房屋」
    /// lingers carrying `prop::NO_BUILD`.
    #[serde(default)]
    pub lingering: Vec<Lingering>,
}

/// One turn-scoped lingering card instance (`docs/PURCHASE.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lingering {
    /// The card rule's id (the `CardDef` the instance binds).
    pub card: String,
    /// The player the instance acts for.
    pub owner: i32,
    /// The turn number it expires on (`0` = this turn only).
    pub expires: i32,
    /// Props the instance carries (e.g. `prop::NO_BUILD`).
    #[serde(default)]
    pub props: std::collections::BTreeMap<String, i32>,
}

impl Default for TurnCtx {
    fn default() -> Self {
        Self {
            plan: Default::default(),
            player_id: 0,
            extra: false,
            main_moved: false,
            played: Vec::new(),
            no_money_loss: Vec::new(),
            fixed_roll: None,
            main_steps: 0,
            abnormal: Vec::new(),
            extreme: Extreme::Plain,
            play_from_hand: false,
            cancelled_designations: Vec::new(),
            play_doubled: None,
            paid_in_settle: 0,
            turn_start_pos: Vec::new(),
            turn_snap: Vec::new(),
            turn_rolls: Vec::new(),
            build_discount: 0,
            build_discount_layers: 0,
            build_cost_pct: full_build_cost(),
            lingering: Vec::new(),
        }
    }
}

/// A card that asked to be called at a turn end (and the "end of your next
/// turn" effects). The host runs the card's `counteract` with kind `turnEnd`
/// when `target`'s turn ends, then drops it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scheduled {
    pub card: String,
    /// The player the card acts for (passed to its `counteract`).
    pub owner: i32,
    /// Whose turn end fires it.
    pub target: usize,
    /// Let one `target` turn end pass first -- set when "the end of your next
    /// turn" is scheduled during that player's own current turn.
    pub skip: bool,
    /// Fire before the status wear-off, rather than after it.
    #[serde(default)]
    pub early: bool,
    /// The card **instance** that asked for the callback, captured at schedule
    /// time: the instance identity belongs here and not to whatever happens to
    /// be on the field when the turn ends.
    ///
    /// `-1` when the card was not in play when it scheduled (`On::AtEnd` may
    /// run for a card in a hand or pile); the run then reads 0 crystals, which
    /// is right -- there is no instance to read them from. Two copies of a card
    /// that both schedule produce two entries with two uids, so the callback is
    /// never ambiguous. `#[serde(default)]` so a save from before this field
    /// existed still loads.
    #[serde(default = "no_card_uid")]
    pub uid: i32,
}

/// The "no instance" card uid: the callback is not anchored to a field card.
fn no_card_uid() -> i32 {
    -1
}

/// Things a routine asks the host to do once it commits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    /// A turn began for this player: refill its time bank and start the clock.
    StartTimer(usize),
    /// A new turn record was created (clears the host's time-out flag).
    TurnBegan,
}

/// How many events are frozen into one immutable chunk.
const CHUNK: usize = 32;

/// The event tail: a frozen, shared prefix plus a small copy-on-write suffix.
///
/// A [`World`] clone happens twice per routine -- the replay snapshot and the
/// working copy -- and the tail holds up to [`EVENT_KEEP`] messages, each
/// carrying a [`Msg`]. Deep-copying that was the dominant cost of a bot match
/// (measured: 1559 -> 374 ms/game just by shortening the tail). Here the frozen
/// chunks sit behind `Arc`, so a clone is a handful of refcount bumps; only the
/// small unfrozen suffix is ever copied, and only when a world that shares it
/// actually logs.
#[derive(Debug, Default)]
pub struct EventTail {
    /// Frozen chunks, oldest first. Immutable once created.
    chunks: Vec<Arc<Vec<MatchEvent>>>,
    /// How many events at the front of `chunks[0]` are no longer visible.
    start: usize,
    /// The chunk currently being written. Copy-on-write.
    tail: Arc<Vec<MatchEvent>>,
    /// Visible event count, so `len` is O(1).
    len: usize,
    /// Bumped on every visible-content change (`push_back` / `back_mut`).
    gen: u64,
    /// Cached [`STATE_EVENTS`] window for [`MatchState::events`], behind a
    /// mutex so `window()` can take `&self` (the view builder is a read).
    window: std::sync::Mutex<WindowCache>,
}

/// The shared [`EventTail::window`] cache: the window plus the `gen` it was
/// built at. Rebuilt only when the tail has changed since the last read.
#[derive(Debug)]
struct WindowCache {
    gen: u64,
    events: Arc<Vec<MatchEvent>>,
}

/// One pending walk segment awaiting lazy flush (`Cx::walk`).
///
/// A walk's `roll` / `move` event is not logged per step: steps accumulate
/// here and the segment is published when the walk reaches CiRCLE / its last
/// step -- **or** when anything else is about to be logged (a hook's `"card"`
/// flash, a pay, a prompt's log line), so the client animates the approach
/// before the thing that interrupted it. Silent hooks do not split the walk.
///
/// `from` / `to` are board tiles (`to` is the segment's endpoint, not
/// `from + value` -- the walk wraps the board). `value` is signed steps
/// (`seg_steps * dir`). `pending_first` says the walk's head line (`log.roll`
/// / `log.move_forward` / `log.move_back`) has not been published yet; the
/// first flush takes `head`, later ones `log.move_on`.
#[derive(Debug, Clone, PartialEq)]
pub struct WalkSeg {
    pub player: i32,
    pub from: i32,
    pub to: i32,
    pub value: i32,
    pub pending_first: bool,
    /// The walk is the turn's main move: the first publish is kind `roll`
    /// carrying `dice = roll`, not a bare `move`.
    pub main: bool,
    /// The dice face the head `roll` event shows (`m.roll`).
    pub roll: i32,
    /// Prebuilt head text (`head(false)` -- 「…n格」 without 「原地」).
    pub head: Msg,
}

/// Transient walk-flush state riding a [`World`] (see [`WalkSeg`]).
///
/// Not serialized and never compared: it is empty at every checkpointable
/// moment (flushed before each log line and each prompt), so it cannot affect
/// a save or a checkpoint-equivalence check. The derived `PartialEq` on
/// [`World`] would otherwise drag a mid-walk `Some` into a mismatch.
#[derive(Debug, Default, Clone)]
pub struct WalkFlush {
    /// Innermost last: `Cx::walk` pushes, so a nested walk's first event must
    /// still flush the outer approach first (see [`World::flush_walk`]).
    stack: Vec<WalkSeg>,
    /// Presentation time owed to the host for flushed segments. [`crate::engine::cx::Cx::wait`]
    /// drains it into `delay`.
    delay: f32,
    /// Re-entrancy guard: the flush itself logs.
    flushing: bool,
}

impl PartialEq for WalkFlush {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Default for WindowCache {
    fn default() -> Self {
        Self {
            gen: u64::MAX, // never matches a real gen -> first read rebuilds
            events: Arc::new(Vec::new()),
        }
    }
}

impl Clone for EventTail {
    fn clone(&self) -> Self {
        Self {
            chunks: self.chunks.clone(),
            start: self.start,
            tail: Arc::clone(&self.tail),
            len: self.len,
            gen: self.gen,
            // The cache is per-instance: a clone diverges as soon as either
            // side logs, and a rebuild is cheap. Start it cold.
            window: std::sync::Mutex::new(WindowCache::default()),
        }
    }
}

impl PartialEq for EventTail {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl Serialize for EventTail {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.iter())
    }
}

impl<'de> Deserialize<'de> for EventTail {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = Vec::<MatchEvent>::deserialize(d)?;
        let mut t = EventTail::default();
        for e in v {
            t.push_back(e);
        }
        Ok(t)
    }
}

impl EventTail {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Every visible event, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &MatchEvent> {
        let head = self
            .chunks
            .first()
            .map_or(&[][..], |c| &c[self.start.min(c.len())..]);
        let mid = self.chunks.iter().skip(1).flat_map(|c| c.iter());
        head.iter().chain(mid).chain(self.tail.iter())
    }

    pub fn back(&self) -> Option<&MatchEvent> {
        self.tail
            .last()
            .or_else(|| self.chunks.last().and_then(|c| c.last()))
    }

    pub fn back_mut(&mut self) -> Option<&mut MatchEvent> {
        self.gen = self.gen.wrapping_add(1);
        Arc::make_mut(&mut self.tail).last_mut()
    }

    /// Stamp `parent` on every event from index `since` onward that does not
    /// already carry one. Rebuilds the tail (O(window)) so a frozen chunk can
    /// be rewritten; the window is small and this runs once per host request.
    pub fn stamp_parent_since(&mut self, since: usize, parent: i32) {
        if parent < 0 || since >= self.len {
            return;
        }
        let mut evs: Vec<MatchEvent> = self.iter().cloned().collect();
        let mut touched = false;
        for e in evs.iter_mut().skip(since) {
            if e.parent < 0 {
                e.parent = parent;
                touched = true;
            }
        }
        if !touched {
            return;
        }
        *self = EventTail::default();
        for e in evs {
            self.push_back(e);
        }
    }

    /// Replace the message of the event with `id`. `false` when not found.
    pub fn replace_msg(&mut self, id: i32, msg: crate::msg::Msg) -> bool {
        let mut evs: Vec<MatchEvent> = self.iter().cloned().collect();
        let Some(e) = evs.iter_mut().find(|e| e.id == id) else {
            return false;
        };
        e.msg = msg;
        *self = EventTail::default();
        for e in evs {
            self.push_back(e);
        }
        true
    }

    /// Set `value` on the event with `id`. No-op when not found.
    pub fn set_value(&mut self, id: i32, value: i32) {
        let mut evs: Vec<MatchEvent> = self.iter().cloned().collect();
        let Some(e) = evs.iter_mut().find(|e| e.id == id) else {
            return;
        };
        e.value = value;
        *self = EventTail::default();
        for e in evs {
            self.push_back(e);
        }
    }

    /// Drop every event whose id is in `ids`. Rebuilds the tail.
    pub fn drop_ids(&mut self, ids: &[i32]) {
        if ids.is_empty() {
            return;
        }
        let keep: Vec<MatchEvent> = self
            .iter()
            .filter(|e| !ids.contains(&e.id))
            .cloned()
            .collect();
        *self = EventTail::default();
        for e in keep {
            self.push_back(e);
        }
    }

    pub fn push_back(&mut self, e: MatchEvent) {
        if self.tail.len() >= CHUNK {
            // Freeze what has been written and start a fresh chunk. O(1): the
            // old tail moves into `chunks`, nothing is copied.
            let done = std::mem::take(&mut self.tail);
            self.chunks.push(done);
        }
        Arc::make_mut(&mut self.tail).push(e);
        self.len += 1;
        self.gen = self.gen.wrapping_add(1);
    }

    /// The [`STATE_EVENTS`] window a [`MatchState`] snapshot carries, shared
    /// behind an `Arc`. Rebuilt only when the tail changed since the previous
    /// read, so a `state()` poll between log lines is a refcount bump.
    pub fn window(&self) -> Arc<Vec<MatchEvent>> {
        let mut g = self.window.lock().unwrap_or_else(|e| e.into_inner());
        if g.gen != self.gen {
            let n = self.len;
            let v: Vec<MatchEvent> = self
                .iter()
                .skip(n.saturating_sub(STATE_EVENTS))
                .cloned()
                .collect();
            g.events = Arc::new(v);
            g.gen = self.gen;
        }
        Arc::clone(&g.events)
    }

    /// Drop `n` events from the front.
    pub fn drain_front(&mut self, n: usize) {
        let n = n.min(self.len);
        if n == 0 {
            return;
        }
        self.start += n;
        self.len -= n;
        self.gen = self.gen.wrapping_add(1);
        while let Some(c) = self.chunks.first() {
            if self.start >= c.len() {
                self.start -= c.len();
                self.chunks.remove(0);
            } else {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub st: MatchState,
    pub hidden: Vec<Hidden>,
    pub rng: Rng,
    pub next_event: i32,
    /// The event tail. See [`EventTail`]: a routine snapshot needs this only to
    /// re-derive it on replay, never to mutate what came before, so cloning a
    /// [`World`] bumps refcounts instead of deep-copying `EVENT_KEEP` messages --
    /// which was the single hottest cost in a bot match.
    pub recent: EventTail,
    pub turn: TurnCtx,
    pub extra_turns: Vec<usize>,
    pub out_count: i32,
    pub event_deck: Vec<String>,
    pub event_discard: Vec<String>,
    pub event_removed: Vec<String>,
    /// The current turn is over; the host starts the next one.
    pub next_turn_pending: bool,
    /// Deeds of players who left, waiting to be auctioned.
    pub leftovers: VecDeque<Vec<usize>>,
    /// Added to the RiNG rent multiplier by cards.
    pub ring_bonus: i32,
    pub ask_seq: i32,
    pub signals: Vec<Signal>,
    /// Turn-end callbacks waiting for their turn end.
    #[serde(default)]
    pub scheduled: Vec<Scheduled>,
    /// Per player, times other players' cards targeted it since its own turn
    /// last started.
    #[serde(default)]
    pub targeted: Vec<i32>,
    /// 「当前回合内你每获得过一次资金」 -- per player, times money landed on them
    /// during the **current turn** (a print, a pay-player credit, or a
    /// `gain_fixed`). Zeroed for everyone at each turn start, so a [反击] read at
    /// a turn's end sees just that turn. The count a hand card reads without any
    /// field stand-in (HHW:（育美）(2)).
    #[serde(default)]
    pub gains: Vec<i32>,
    /// Marker **ownership** (user ruling 2026-10-07): a marker is owned by the
    /// **rule that creates it**, wherever its copies sit. Maps a marker name
    /// (「抹茶芭菲」, 「P✽P粉丝」, …) to the owning rule's card id
    /// (`skill:要乐奈:投币式停车场的猫` owns **all** 抹茶芭菲 -- its player's
    /// counter, every other player's counter, and those on tiles).
    ///
    /// Stamped on first creation from the creating run's `current_card`.
    /// Bankruptcy of the owning rule's player removes every copy of the
    /// marker, wherever it sits (`remove_from_game`). Neutral board marks
    /// ([CP点], `owner == -1`) have no player-owned rule and stay.
    #[serde(default)]
    pub marker_owner: std::collections::BTreeMap<String, String>,
    /// Pending walk segments + flush pacing (see [`WalkSeg`]). Transient: not
    /// serialized, not compared.
    #[serde(skip)]
    pub walk_flush: WalkFlush,
}

impl World {
    /// Assemble a world around an already-built [`Rng`]: which stream the
    /// match runs on is the caller's choice (`Match::new` legacy xoshiro vs
    /// `Match::new_seeded` ChaCha12, `docs/FAIRNESS.md`).
    pub fn new(st: MatchState, players: usize, rng: Rng) -> Self {
        Self {
            st,
            hidden: vec![Hidden::default(); players],
            rng,
            next_event: 1,
            recent: EventTail::default(),
            turn: TurnCtx::default(),
            extra_turns: vec![],
            out_count: 0,
            event_deck: vec![],
            event_discard: vec![],
            event_removed: vec![],
            next_turn_pending: false,
            leftovers: VecDeque::new(),
            ring_bonus: 0,
            ask_seq: 0,
            signals: vec![],
            scheduled: vec![],
            targeted: vec![],
            gains: vec![],
            marker_owner: std::collections::BTreeMap::new(),
            walk_flush: WalkFlush::default(),
        }
    }

    /// 「当前回合内你每获得过一次资金」 -- count a money-in for `player_id`.
    /// Every credit path funnels through here (`money`'s settlement,
    /// `gain_fixed`, `gain_money`).
    pub fn bump_gain(&mut self, player_id: i32, times: i32) {
        let Ok(s) = usize::try_from(player_id) else {
            return;
        };
        if times <= 0 {
            return;
        }
        if self.gains.len() <= s {
            self.gains.resize(s + 1, 0);
        }
        self.gains[s] += times;
    }

    /// The current turn's gain count for `player_id`.
    pub fn gains_this_turn(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.gains.get(s).copied())
            .unwrap_or(0)
    }

    /// Stamp marker ownership (user ruling 2026-10-07): a marker is owned by
    /// the **rule that creates it**. First writer wins -- a later rule that
    /// moves someone else's marker does not take it over.
    pub fn note_marker_owner(&mut self, name: &str, rule: &str) {
        if name.is_empty() || rule.is_empty() {
            return;
        }
        self.marker_owner
            .entry(name.to_string())
            .or_insert_with(|| rule.to_string());
    }

    /// The rule that owns marker `name`, if any.
    pub fn marker_owner_of(&self, name: &str) -> Option<&str> {
        self.marker_owner.get(name).map(|s| s.as_str())
    }

    /// Zero every player's turn-gain counter (「当前回合内」 starts fresh).
    pub fn reset_gains(&mut self) {
        self.gains.iter_mut().for_each(|n| *n = 0);
    }

    /// Append an event to the log. Returns it for further fields.
    ///
    /// Lazy walk flush: any other event while a [`WalkSeg`] is pending
    /// publishes the approach first, so the client walks to the tile before
    /// showing the card flash / pay / effect that fired there. The flush's own
    /// event re-enters here under [`WalkFlush::flushing`] and is not re-flushed.
    pub fn log(&mut self, kind: &str, player_id: i32, msg: Msg) -> &mut MatchEvent {
        let _tg = crate::engine::rtimer::guard(&crate::engine::rtimer::LOG_NS);
        self.flush_walk();
        let e = MatchEvent {
            id: self.next_event,
            r#type: kind.into(),
            player_id,
            msg,
            ..MatchEvent::default()
        };
        self.next_event += 1;
        self.recent.push_back(e);
        if self.recent.len() > EVENT_KEEP + 50 {
            let drop = self.recent.len() - EVENT_KEEP;
            self.recent.drain_front(drop);
        }
        self.recent.back_mut().expect("just pushed")
    }

    /// Publish every pending [`WalkSeg`] that has something to say, outermost
    /// first (a nested walk's events must not jump the queue ahead of the
    /// approach that spawned them). Idempotent; safe to call from a halt.
    ///
    /// A segment is published when it has walked at least one step, **or**
    /// when it is a main move's un-published head (the dice-only `roll` on the
    /// very first `passBefore` -- the client must see the dice before the card
    /// that reacted to them). A non-main segment with no steps says nothing
    /// and stays pending, so its head line lands with the first real step.
    pub fn flush_walk(&mut self) {
        if self.walk_flush.flushing {
            return;
        }
        self.walk_flush.flushing = true;
        // Snapshot what to publish (and reset the slots) before logging:
        // `log` takes `&mut self`, so the stack cannot stay borrowed.
        let mut pending = Vec::new();
        for seg in self.walk_flush.stack.iter_mut() {
            if seg.value == 0 && !(seg.pending_first && seg.main) {
                continue;
            }
            let ek = if seg.pending_first && seg.main {
                "roll"
            } else {
                "move"
            };
            let text = if seg.pending_first {
                seg.head.clone()
            } else {
                Msg::new("log.move_on").player_id("who", seg.player)
            };
            let dice = if seg.pending_first && seg.main {
                seg.roll
            } else {
                0
            };
            pending.push((ek, text, seg.player, seg.from, seg.to, seg.value, dice));
            seg.from = seg.to;
            seg.value = 0;
            seg.pending_first = false;
        }
        for (ek, text, player, from, to, value, dice) in pending {
            let e = self.log(ek, player, text);
            e.from = from;
            e.to = to;
            e.value = value;
            e.dice = dice;
            // Same pacing the old per-publish `Cx::wait` applied (1.5s for the
            // head `roll`, 0.3s for a continuation, plus 0.15s/step).
            let pace = if ek == "roll" { 1.5 } else { 0.3 };
            self.walk_flush.delay += pace + value.unsigned_abs() as f32 * 0.15;
        }
        self.walk_flush.flushing = false;
    }

    /// Drain the presentation time [`Self::flush_walk`] accumulated since the
    /// last drain (the host's `Cx::wait` equivalent).
    pub fn take_walk_delay(&mut self) -> f32 {
        std::mem::take(&mut self.walk_flush.delay)
    }

    /// Arm a new innermost walk segment (`Cx::walk` on entry).
    pub fn push_walk_seg(&mut self, seg: WalkSeg) {
        self.walk_flush.stack.push(seg);
    }

    /// Drop the innermost walk segment (`Cx::walk` on exit, after flushing).
    pub fn pop_walk_seg(&mut self) {
        self.walk_flush.stack.pop();
    }

    /// The walk currently stepping: the innermost armed segment.
    pub fn walk_seg_mut(&mut self) -> Option<&mut WalkSeg> {
        self.walk_flush.stack.last_mut()
    }

    /// How many walk segments are armed (a nested walk pushes another).
    pub fn walk_depth(&self) -> usize {
        self.walk_flush.stack.len()
    }

    /// A card's effect activated -- or was negated before its body could run.
    /// One `"card"` event carries both the log line and the client's card flash:
    /// `kind` is the trigger kind ([`crate::state::card_trigger`]), `owner` the
    /// card's player, `target` the affected player and `tile` where it fired
    /// (`-1` when not applicable). `msg` names the card via [`Msg::card`].
    /// `parent` groups this activation under another one (`-1` = top-level).
    pub fn card_activation(
        &mut self,
        kind: &str,
        owner: i32,
        card: &str,
        target: i32,
        tile: i32,
        negated: bool,
        msg: Msg,
    ) -> &mut MatchEvent {
        let e = self.log("card", owner, msg);
        e.card = card.to_string();
        e.other = target;
        e.value = tile;
        e.kind = kind.to_string();
        e.negated = negated;
        e
    }

    pub fn player_count(&self) -> usize {
        self.st.players.len()
    }

    pub fn out(&self, i: usize) -> bool {
        self.st.players.get(i).is_none_or(|s| s.out())
    }

    /// Public state: hidden-zone sizes and the event tail filled in.
    pub fn public_state(&self) -> MatchState {
        let mut st = self.st.clone();
        // `st.buy_price` / `st.build_cost` are view previews (`docs/PURCHASE.md`):
        // the quoted price at the player's position when a buy/build is on the
        // table, `-1` otherwise. Written on the public copy; the live fields are
        // refreshed by `Cx::refresh_buy_preview` at the END step. The matching
        // `can_buy_here` / `can_build_here` act-legality flags are recomputed
        // alongside them in `Match::state`.
        st.buy_price = -1;
        st.build_cost = -1;
        st.can_buy_here = false;
        st.can_build_here = false;
        for (player_id, h) in st.players.iter_mut().zip(&self.hidden) {
            player_id.hand = h.hand.len() as i32;
            player_id.draw = h.draw.len() as i32;
            player_id.discard = h.discard.clone();
        }
        // The recent-event window: shared via `Arc`, rebuilt in `EventTail`
        // only when something was logged since the last read.
        st.events = self.recent.window();
        st.event_deck = self.event_deck.len() as i32;
        st.event_discard = self.event_discard.clone();
        st.event_removed = self.event_removed.clone();
        // Active events ride their rule instance (`docs/EVENTS.md`): the view's
        // counters mirror the instance's crystals / props so the client shows
        // what the body is counting without a second store.
        for e in st.event_active.iter_mut() {
            let rid = crate::data::event_rule_id(&e.id);
            if let Some(f) = st.board_field.iter().find(|f| f.card == rid && f.tile < 0) {
                e.counter = f.crystals;
                e.counter2 = f.props.get("count2").copied().unwrap_or(0);
                e.note = f.note.clone();
                e.face_down = f.face_down;
            }
        }
        st
    }
}

/// Copy-on-write handle to a [`World`] (engine efficiency fix B,
/// `docs/BOT.md` "engine efficiency A–C").
///
/// Reads deref straight to the inner world -- **no clone**. The first write
/// through [`DerefMut`] clones only when the handle is shared
/// ([`Arc::make_mut`]); a private handle is mutated in place. Pure checks
/// (`cant_play` probes, buy quotes, guard bodies) share one handle with the
/// live world, so a check whose body never writes never copies the world --
/// and `Run` clones handed to a rules store are refcount bumps, not deep
/// copies.
///
/// [`Self::stamp`] changes on every write. Play-gate memos key on it, so a
/// cached `cant_play` verdict is dropped the moment anything writes.
#[derive(Clone)]
pub struct SharedWorld {
    inner: Arc<World>,
    stamp: u64,
}

impl SharedWorld {
    /// Take ownership of `w` behind a fresh handle.
    pub fn new(w: World) -> Self {
        Self {
            inner: Arc::new(w),
            stamp: 0,
        }
    }

    /// Another handle to the same world -- a refcount bump, **no copy**.
    pub fn share(&self) -> Self {
        self.clone()
    }

    /// A monotonic id for this world's current contents. Changes on every
    /// write; equal stamps mean equal state (the handle was shared and
    /// neither side wrote).
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    /// Unwrap the world, deep-cloning only if the handle is still shared.
    pub fn into_inner(self) -> World {
        match Arc::try_unwrap(self.inner) {
            Ok(w) => w,
            Err(arc) => (*arc).clone(),
        }
    }

    /// A plain deep copy (the old [`World::clone`] cost). Prefer
    /// [`Self::share`] when the caller only needs a read view or a COW
    /// sandbox.
    pub fn deep_clone(&self) -> World {
        (*self.inner).clone()
    }
}

impl Deref for SharedWorld {
    type Target = World;
    fn deref(&self) -> &World {
        &self.inner
    }
}

impl DerefMut for SharedWorld {
    fn deref_mut(&mut self) -> &mut World {
        // Cheap monotonic stamp (not a global counter: only same-handle
        // comparisons matter, and a shared clone carries the stamp of the
        // state it captured).
        self.stamp = self.stamp.wrapping_add(1);
        Arc::make_mut(&mut self.inner)
    }
}

impl From<World> for SharedWorld {
    fn from(w: World) -> Self {
        Self::new(w)
    }
}

impl std::fmt::Debug for SharedWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedWorld")
            .field("stamp", &self.stamp)
            .field("world", &*self.inner)
            .finish()
    }
}
