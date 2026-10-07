//! The replayable part of a match.
//!
//! Everything a routine reads or writes lives here, including the RNG and the
//! event/prompt counters, so re-running a routine from a snapshot reproduces the same
//! dice, the same event ids and the same prompt ids. Clocks, the live prompt, auction
//! bidding and the end-match vote belong to the host ([`super::Match`]) instead.

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::msg::Msg;
use crate::rng::Rng;
use crate::state::{MatchEvent, MatchState};

/// Money each player starts with (`BeginPlay`).
pub const START_MONEY: i32 = 10_000;
/// Opening hand size (`StartHandOf`).
pub const START_HAND: usize = 2;
/// Base hand limit (`HandLimitOf`).
pub const HAND_LIMIT: usize = 5;
/// CiRCLE reward in money (`CircleReward`).
pub const CIRCLE_MONEY: i32 = 2000;
/// Events kept for `events_since` (`EventKeep`).
pub const EVENT_KEEP: usize = 400;
/// Events carried in every `MatchState` snapshot.
pub const STATE_EVENTS: usize = 80;

/// Per-player private card zones (C# `Hidden`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hidden {
    pub hand: Vec<String>,
    /// Top of the pile is the **end** of the vec.
    pub draw: Vec<String>,
    pub discard: Vec<String>,
    /// The player's next main move walks exactly this many steps instead of the
    /// roll (C# `NextStepsFx.Steps`); consumed by that move.
    #[serde(default)]
    pub next_steps: Option<i32>,
}

fn full_build_cost() -> i32 {
    100
}

/// One player's `_turnSnap[i]`: the status a turn-end undo restores to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TurnSnap {
    pub pos: i32,
    pub stay: i32,
    pub stun: i32,
    pub exile: i32,
}

/// Per-turn bookkeeping (the parts of C# `TurnCtx` the shell uses).
///
/// The derived `Default` would zero `build_cost_pct` (only the serde default is
/// 100), so a fresh turn would build for free; the manual impl keeps full price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnCtx {
    /// `C# TurnCtx.Plan` -- the movement being planned / walked. Runtime-only:
    /// it carries roll tables and closures, so it never reaches the wire (the
    /// broadcast summary is `MatchState::plan`).
    #[serde(skip)]
    pub plan: crate::engine::move_ctx::MoveCtx,
    pub player_id: usize,
    pub extra: bool,
    pub main_moved: bool,
    pub played: Vec<String>,
    /// Players whose money cannot drop for the rest of this turn (C#
    /// `TurnCtx.NoMoneyLoss`). Auctions are not payments, so they are exempt.
    #[serde(default)]
    pub no_money_loss: Vec<usize>,
    /// This turn's main-move roll is fixed (C# `TurnCtx.Plan.FixedRoll`).
    #[serde(default)]
    pub fixed_roll: Option<i32>,
    /// Steps this turn's main move walked (C# `TurnCtx.LastMain`); 0 = none yet.
    #[serde(default)]
    pub main_steps: i32,
    /// C# `_abnormalTurn` -- abnormal effects that got through to each player
    /// this turn (indexed by player; a new turn starts them all at 0).
    #[serde(default)]
    pub abnormal: Vec<i32>,
    /// C# `PlayCtx.Extreme` -- the play being resolved settles its number-range
    /// dice at the theoretical extreme: `1` = max, `-1` = min, `0` = plain.
    /// Set mid-play by a [反击] (「以理论最大值或最小值结算」) and cleared when
    /// the play ends, so it does not leak into the next one.
    #[serde(default)]
    pub extreme: i32,
    /// C# `PlayCtx.FromDeck` -- the play being resolved came from somewhere
    /// other than the hand (`false`). 「若此卡从手牌以外的地方打出」 reads this.
    #[serde(default)]
    pub play_from_hand: bool,
    /// C# `play.Tags["immune"+seat]` -- seats whose designation on the play
    /// being resolved is cancelled (「取消其对目标之一的[指定]」). A per-pair
    /// cancel: the rest of the play's designations still land. Cleared at the
    /// start of each play.
    #[serde(default)]
    pub cancelled_designations: Vec<i32>,
    /// C# `TurnCtx.BuyDiscount` -- 「本回合购买格子时[消耗]资金时降低N」. A value,
    /// not a policy: `buy()` subtracts it from the price (floored at 0).
    #[serde(default)]
    pub buy_discount: i32,
    /// C# `TurnCtx.PaidInSettle` -- money paid to other players during this
    /// turn's [触发结算]s. 「本回合的[结算]向其他玩家支付了至少1000资金」.
    #[serde(default)]
    pub paid_in_settle: i32,
    /// C# `TurnCtx.FreeBuy` -- 「本回合购买格子不[消耗]资金」.
    #[serde(default)]
    pub free_buy: bool,
    /// C# `TurnCtx.RazeOnBuy` -- 「如果购买则拆除那个格子上的所有房屋」.
    #[serde(default)]
    pub raze_on_buy: bool,
    /// C# `_turnSnap[i].pos` -- where each player stood when the turn started.
    /// 「在Livehouse地块开始回合时」 is a question about that square, not the
    /// one a mid-turn walk has since reached.
    #[serde(default)]
    pub turn_start_pos: Vec<i32>,
    /// C# `_turnSnap[i]` -- each player's pos / stay / stun / exile when the turn
    /// started. 「回到起始地点并取消所有受到的效果」 restores from here.
    #[serde(default)]
    pub turn_snap: Vec<TurnSnap>,
    /// C# `_turnCtx.Rolls` -- every face rolled this turn, in order. 「与本回合内
    /// 你骰出过的所有骰点都不同」 compares against this.
    #[serde(default)]
    pub turn_rolls: Vec<i32>,
    /// C# `BuildDiscountFx` -- 「下次盖房时减免N（可溢出），盖房后减少1层」.
    /// A layered cut on the build cost; each build pops one layer.
    #[serde(default)]
    pub build_discount: i32,
    pub build_discount_layers: i32,
    /// 「加盖房屋时半价」 -- the build cost as a percentage of its table price.
    /// 100 = full, 50 = half, 0 = free (「本回合加盖房屋变为免费」).
    #[serde(default = "full_build_cost")]
    pub build_cost_pct: i32,
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
            extreme: 0,
            play_from_hand: false,
            cancelled_designations: Vec::new(),
            buy_discount: 0,
            paid_in_settle: 0,
            free_buy: false,
            raze_on_buy: false,
            turn_start_pos: Vec::new(),
            turn_snap: Vec::new(),
            turn_rolls: Vec::new(),
            build_discount: 0,
            build_discount_layers: 0,
            build_cost_pct: full_build_cost(),
        }
    }
}

/// A card that asked to be called at a turn end (C# `TurnCtx.AfterEnd` /
/// `AtEnd`, and the "end of your next turn" effects). The host runs the card's
/// `counteract` with kind `turnEnd` when `target`'s turn ends, then drops it.
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
    /// C# `AtEnd`: before the status wear-off (`TurnEndBefore`), rather than
    /// `AfterEnd` after it (`TurnEndAfter`).
    #[serde(default)]
    pub early: bool,
    /// The card **instance** that asked for the callback, captured at schedule
    /// time -- the C# `H._turnCtx.AtEnd.Add(() => ...)` closure captures that
    /// card object, so the instance identity belongs here and not to whatever
    /// happens to be on the field when the turn ends.
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
}

impl Clone for EventTail {
    fn clone(&self) -> Self {
        Self {
            chunks: self.chunks.clone(),
            start: self.start,
            tail: Arc::clone(&self.tail),
            len: self.len,
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
        Arc::make_mut(&mut self.tail).last_mut()
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
    }

    /// Drop `n` events from the front.
    pub fn drain_front(&mut self, n: usize) {
        let n = n.min(self.len);
        self.start += n;
        self.len -= n;
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
    /// `H._ringBonus` -- added to the RiNG rent multiplier by cards.
    pub ring_bonus: i32,
    pub ask_seq: i32,
    pub signals: Vec<Signal>,
    /// Turn-end callbacks waiting for their turn end.
    #[serde(default)]
    pub scheduled: Vec<Scheduled>,
    /// C# `H._targeted` -- per player, times other players' cards targeted it since
    /// its own turn last started.
    #[serde(default)]
    pub targeted: Vec<i32>,
    /// 「当前回合内你每获得过一次资金」 -- per player, times money landed on them
    /// during the **current turn** (a print, a pay-player credit, or a
    /// `gain_fixed`). Zeroed for everyone at each turn start, so a [反击] read at
    /// a turn's end sees just that turn. The count a hand card reads without any
    /// field stand-in (HHW:（育美）(2)).
    #[serde(default)]
    pub gains: Vec<i32>,
}

impl World {
    pub fn new(st: MatchState, players: usize, seed: u64) -> Self {
        Self {
            st,
            hidden: vec![Hidden::default(); players],
            rng: Rng::new(seed),
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

    /// Zero every player's turn-gain counter (「当前回合内」 starts fresh).
    pub fn reset_gains(&mut self) {
        self.gains.iter_mut().for_each(|n| *n = 0);
    }

    /// Append an event (`MatchHost.Log`). Returns it for further fields.
    pub fn log(&mut self, kind: &str, player_id: i32, msg: Msg) -> &mut MatchEvent {
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

    pub fn player_count(&self) -> usize {
        self.st.players.len()
    }

    pub fn out(&self, i: usize) -> bool {
        self.st.players.get(i).is_none_or(|s| s.out())
    }

    /// Public state: hidden-zone sizes and the event tail filled in (`SyncAll`).
    pub fn public_state(&self) -> MatchState {
        let mut st = self.st.clone();
        for (player_id, h) in st.players.iter_mut().zip(&self.hidden) {
            player_id.hand = h.hand.len() as i32;
            player_id.draw = h.draw.len() as i32;
            player_id.discard = h.discard.clone();
        }
        let n = self.recent.len();
        st.events = self
            .recent
            .iter()
            .skip(n.saturating_sub(STATE_EVENTS))
            .cloned()
            .collect();
        st.event_deck = self.event_deck.len() as i32;
        st.event_discard = self.event_discard.clone();
        st.event_removed = self.event_removed.clone();
        st
    }
}
