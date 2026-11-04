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
/// Events kept for `events_since`.
pub const EVENT_KEEP: usize = 400;
/// Events carried in every `MatchState` snapshot.
pub const STATE_EVENTS: usize = 80;

/// Per-player private card zones (hand / draw / discard).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hidden {
    pub hand: Vec<String>,
    /// Top of the pile is the **end** of the vec.
    pub deck: Vec<String>,
    pub graveyard: Vec<String>,
    /// The player's next main move walks exactly this many steps instead of
    /// the roll; consumed by that move.
    #[serde(default)]
    pub next_steps: Option<i32>,
    /// The persona this seat last clicked on the pick screen, before confirming.
    /// Private: never part of `MatchState`, so opponents never see it. Consumed
    /// by the pick-timeout fallback (`tick_choice`). Skipped when empty so a
    /// match that never preselected keeps a byte-identical save (checkpoint
    /// hashes).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub pick_intent: String,
}

/// One open deck inspection: the cards a look has turned over and not yet
/// released, top-first, and who may look at them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionFrame {
    pub id: i32,
    pub looker: i32,
    pub deck_owner: i32,
    pub public: bool,
    /// Top-first. `revealed` is false for entries a not-yet-finished
    /// `inspection_begin` has moved but not raised `inspected` for.
    pub cards: Vec<InspectionCard>,
    /// The begin that owns the frame has returned. A resumed begin (its first
    /// pass suspended on an `inspected` hook, so no answer was recorded)
    /// adopts the innermost unfinished frame of its looker and deck instead of
    /// moving a second prefix.
    pub finished: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InspectionCard {
    pub id: String,
    pub revealed: bool,
}

/// One seat's public reveal: the ids published, and the clause that published
/// them. `piles` marks the lists that are a whole pile (`hand`, `deck`,
/// `discard`): such a list is kept in step with the pile while the record
/// lives, so a card that moves out mid-look leaves the published list too.
/// Single shown cards are appended and never rewritten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RevealRecord {
    pub player: i32,
    pub reason: Msg,
    pub hand: Vec<String>,
    pub deck: Vec<String>,
    pub discard: Vec<String>,
    #[serde(default)]
    pub piles: [bool; 3],
}

fn full_build_cost() -> i32 {
    100
}

/// Serde default for a milli factor (`1000` = ×1).
fn milli_one() -> i32 {
    1000
}

/// Serde default for an `f64` pay factor (`1.0` = ×1).
fn one() -> f64 {
    1.0
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

/// One player's turn-start snapshot: the starting square, a copy of the whole
/// keyed state map, that player's money and the whole board ownership arrays.
/// The guest picks which keys to restore; the engine names none of them.
/// 「取消所有受到的效果」 restores the money and the ownership arrays too.
///
/// The wire field order is mirrored by `card_sdk::ctx::TurnSnapshot`; keep the
/// two in step (the postcard payload has no field names).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TurnSnapshot {
    pub pos: i32,
    pub state: std::collections::BTreeMap<String, i32>,
    pub money: i32,
    pub owners: Vec<i32>,
    pub houses: Vec<i32>,
    pub mortgaged: Vec<bool>,
}

/// A fixed face the turn's main move must use instead of casting (去唱卡拉ok吧,
/// a kept die), with the source it was obtained from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedFace {
    pub value: i32,
    pub source: crate::engine::rules::RollSource,
}

/// One cast in [`TurnCtx::roll_history`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollHistoryEntry {
    pub roller: usize,
    pub value: i32,
    /// `0` = effect roll, `1` = move roll.
    pub purpose: i32,
    pub source: crate::engine::rules::RollSource,
    /// The face came from `fixed_roll`, not a throw.
    pub fixed: bool,
}

/// A 「是否触发结算」 answer (选择自己的舞台). Mirrors
/// `card_sdk::abi::SettleAnswer`; `Unanswered` = the card said nothing.
#[repr(i32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SettleAnswer {
    #[default]
    Unanswered = 0,
    Settle = 1,
    Skip = 2,
}

impl SettleAnswer {
    /// The `resolveOverride` slot value (`take_resolve_override` reads 1 =
    /// settle, 2 = skip), `None` for `Unanswered`.
    pub const fn slot(self) -> Option<i32> {
        match self {
            SettleAnswer::Unanswered => None,
            SettleAnswer::Settle => Some(1),
            SettleAnswer::Skip => Some(2),
        }
    }

    /// Decode the guest code.
    pub const fn from_i32(v: i32) -> Self {
        match v {
            1 => SettleAnswer::Settle,
            2 => SettleAnswer::Skip,
            _ => SettleAnswer::Unanswered,
        }
    }
}

/// Per-turn bookkeeping the shell uses.
///
/// The derived `Default` would zero `build_cost_pct` (only the serde default is
/// 100), so a fresh turn would build for free; the manual impl keeps full price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnCtx {
    /// The movement being planned / walked. The wire still carries only the
    /// broadcast summary (`MatchState::plan`); this full context is what
    /// `Match::save` must round-trip -- a shaped `base` / `dice` table or a
    /// `steps` fix written by a card lives here and dropping it made
    /// save/restore diverge on the next roll (`fuzz_found::save_restore_break`).
    #[serde(default)]
    pub plan: crate::engine::move_ctx::MoveCtx,
    /// The live plan frames, innermost last. `plan` above is the staging buffer
    /// when this is empty; see [`World::plan`].
    #[serde(default)]
    pub plan_stack: Vec<crate::engine::move_ctx::PlanFrame>,
    /// The activation body's command scope (see [`World::enter_plan_command`]).
    #[serde(default)]
    pub plan_scope: crate::engine::move_ctx::PlanScope,
    pub player_id: usize,
    pub extra: bool,
    pub main_moved: bool,
    /// One higher per routine; stamped onto a trigger by `counteract`.
    #[serde(default)]
    pub activation_ordinal: i32,
    /// The activation that consumed this turn's main move; 0 = none.
    #[serde(default)]
    pub move_activation_ordinal: i32,
    /// A landed abnormal during this turn's main move (read at turn end).
    #[serde(default)]
    pub move_abnormal: bool,
    /// 「打出此卡的回合」: hand plays of dice-adding cards skip the `card` window.
    #[serde(default)]
    pub dice_immune: bool,
    pub played: Vec<String>,
    /// Hand-card plays this turn (skills do not count). 「当有人同一回合内打出
    /// 两张卡时」 (TwoCards) reads this -- the second card play raises it.
    #[serde(default)]
    pub cards_played: usize,
    /// Players whose money cannot drop for the rest of this turn. Auctions are
    /// not payments, so they are exempt.
    #[serde(default)]
    pub no_money_loss: Vec<usize>,
    /// This turn's main move roll is fixed to this face (and the source it
    /// was obtained with).
    #[serde(default)]
    pub fixed_roll: Option<FixedFace>,
    /// Steps this turn's main move walked; 0 = none yet.
    #[serde(default)]
    pub main_steps: i32,
    /// Abnormal effects that got through to each player this turn (indexed by
    /// player; a new turn starts them all at 0).
    #[serde(default)]
    pub abnormal: Vec<i32>,
    /// The play being resolved settles its number-range dice at the theoretical
    /// extreme: `1` = max, `-1` = min, `0` = plain. Set mid-play by a counteraction
    /// (「以理论最大值或最小值结算」) and cleared when the play ends, so it does
    /// not leak into the next one.
    #[serde(default)]
    pub extreme: Extreme,
    /// The activation body being resolved (a hand Play, an event, a skill
    /// press) drove at least one move. An activation that did is *spent* when
    /// it ends -- its per-move plan writes were for the move it drove, and its
    /// replay re-applies them afterwards; one that drove none wrote for a move
    /// still to come (ran 「本次移动向后」, sakiko_lead's half price) and
    /// keeps them (`PlayCtx::drop_spent_plan_shape`). Reset at the activation's
    /// entry alongside `extreme`.
    #[serde(default)]
    pub play_drove_move: bool,
    /// Whether the play being resolved came from the hand (`false` when it came
    /// from somewhere else). 「若此卡从手牌以外的地方打出」 reads this.
    #[serde(default)]
    pub play_from_hand: bool,
    /// The activation argument `On::declare` collected for the play being
    /// resolved (docs/ACTIVATION.md §2). Scoped to one play, like
    /// `play_from_hand`. `rules.play` reads it into `Call::Play`'s `arg`.
    #[serde(default)]
    pub play_arg: Vec<u8>,
    /// Did the play being resolved place a card on the field? Armed by the
    /// placement host functions, reset per play, read into
    /// [`PlayRecord::placed`] when the play completes (design G06 §5.6).
    /// A copy skips a source that placed.
    #[serde(default)]
    pub placed_during_play: bool,
    /// The card id of the play being resolved (`play_from_hand` scope). The
    /// copy record's 「效果带有放置」 test tells the play's own effect from a
    /// counteraction's placement by comparing against the innermost resolving
    /// frame (design G06 §2.15). `None` outside a play.
    #[serde(default)]
    pub play_card: Option<String>,
    /// Seats whose designation on the play being resolved is cancelled
    /// (「取消其对目标之一的designate」). A per-pair cancel: the rest of the play's
    /// designations still land. Cleared at the start of each play.
    #[serde(default)]
    pub cancelled_designations: Vec<i32>,
    /// Which of the play's tagged `ctx::n` numbers the the start square group skill has
    /// doubled (-1 = none). Lives on the play, not on a guest run, so a
    /// pre-effect hook can arm it for the body that follows. Cleared at the
    /// start of each play.
    #[serde(default, with = "opt_i32_neg1")]
    pub play_doubled: Option<i32>,
    /// Money paid to other players during this turn's [触发结算]s.
    /// 「本回合的settle向其他玩家支付了至少1000资金」.
    #[serde(default)]
    pub paid_in_settle: i32,
    /// The same figure per **payer** seat: guerrilla's 「本回合的[结算]向其他
    /// 玩家支付了至少1000资金」 counts the owner's own payments only
    /// (`ctx::paid_in_settle_by`). Resized at turn start.
    #[serde(default)]
    pub paid_in_settle_by: Vec<i32>,
    /// Pay factor for the **next** move, in milli (1000 = ×1). Folded into
    /// the plan at move start, then reset.
    #[serde(default = "milli_one")]
    pub next_move_pay_factor: i32,
    /// Pay factor for the **next** settle, in milli. Moved into the settle
    /// entry when it is pushed, then reset.
    #[serde(default = "milli_one")]
    pub next_settle_pay_factor: i32,
    /// Fixed pay addend for the **next** settle, moved in and reset the same
    /// way.
    #[serde(default)]
    pub next_settle_pay_add: i32,
    /// A move is in flight (walk / teleport through its tail). The `thisMove`
    /// factor scope is open while this holds.
    #[serde(default)]
    pub move_open: bool,
    /// Depth of the open settles (`PlayCtx::settle_stack`). The guest factor API
    /// needs the depth without the stack; the stack stays the truth and this
    /// is kept level with it.
    #[serde(default)]
    pub settle_open: u32,
    /// The product of every open settle's factor (`thisSettle` composes on
    /// top of [`Self::settle_base`]). This is what the payment scale and the
    /// factor getter read.
    #[serde(default = "one")]
    pub settle_pay_factor: f64,
    /// The product of the **outer** settle factors -- the innermost entry's
    /// `base`. A `thisSettle` write replaces the innermost factor, so it
    /// recomposes from here instead of dividing.
    #[serde(default = "one")]
    pub settle_base: f64,
    /// The same pair for the settle's 「地租」 scale (`rentMul` composes here;
    /// [`Self::settle_rent_base`] is the outer product).
    #[serde(default = "one")]
    pub settle_rent_factor: f64,
    #[serde(default = "one")]
    pub settle_rent_base: f64,
    /// The `settleAdd` / `rentAdd` sums of every open settle (规则书 支付阶段 2:
    /// live fixed-value adjustments sum). Each leg of a settle carries the same
    /// addend; the innermost entry keeps the outer sums in `base_add` /
    /// `base_rent_add` and restores them on its way out.
    #[serde(default)]
    pub settle_pay_add: i32,
    #[serde(default)]
    pub settle_rent_add: i32,
    /// The factor writes that already landed this body execution, as
    /// `(execution, writer, ordinal)` keys. A suspended body is re-executed from its start
    /// on the resume (STACK-01), replaying the prefix; the set makes the
    /// replay's write land on its own earlier contribution instead of
    /// composing it in twice. Reset by the drive, one execution at a time.
    #[serde(default)]
    pub factor_applied: Vec<(u32, String, u32)>,
    /// The running ordinal `Run::factor_write_key` hands out. Part of the same
    /// per-execution transport as [`Self::factor_applied`].
    #[serde(default)]
    pub factor_ordinal: u32,
    /// The last execution id the factor ledger handed out. World-scoped, so
    /// two firings of the same card stay apart even though each drive builds a
    /// fresh context (a context-local counter restarts at 1 and would collide).
    #[serde(default)]
    pub factor_gen: u32,
    /// Where each player stood when the turn started. 「在场地地块开始
    /// 回合时」 is a question about that square, not the one a mid-turn walk has
    /// since reached.
    #[serde(default)]
    pub turn_start_pos: Vec<i32>,
    /// Each player's pos / the layer counters when the turn started.
    /// 「回到起始地点并取消所有受到的效果」 restores from here.
    #[serde(default)]
    pub turn_snapshot: Vec<TurnSnapshot>,
    /// Every face rolled this turn, in order. 「与本回合内你骰出过的所有骰点
    /// 都不同」 compares against this. Mirrors [`Self::roll_history`] (which
    /// also carries the roller); kept for readers that only want the faces.
    #[serde(default)]
    pub turn_rolls: Vec<i32>,
    /// Every cast this turn, with its roller. Written at cast time, before any
    /// roll window opens. Cleared with the turn.
    #[serde(default)]
    pub roll_history: Vec<RollHistoryEntry>,
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
    /// effects: a card that wants 「本回合购买格子时spend资金降低N」 lingers with a
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
            plan_stack: Vec::new(),
            plan_scope: Default::default(),
            player_id: 0,
            extra: false,
            main_moved: false,
            activation_ordinal: 0,
            move_activation_ordinal: 0,
            move_abnormal: false,
            dice_immune: false,
            played: Vec::new(),
            cards_played: 0,
            no_money_loss: Vec::new(),
            fixed_roll: None,
            main_steps: 0,
            abnormal: Vec::new(),
            extreme: Extreme::Plain,
            play_drove_move: false,
            play_from_hand: false,
            placed_during_play: false,
            play_card: None,
            play_arg: Vec::new(),
            cancelled_designations: Vec::new(),
            play_doubled: None,
            paid_in_settle: 0,
            paid_in_settle_by: Vec::new(),
            next_move_pay_factor: milli_one(),
            next_settle_pay_factor: milli_one(),
            next_settle_pay_add: 0,
            move_open: false,
            settle_open: 0,
            settle_pay_factor: one(),
            settle_base: one(),
            settle_rent_factor: one(),
            settle_rent_base: one(),
            settle_pay_add: 0,
            settle_rent_add: 0,
            factor_applied: Vec::new(),
            factor_ordinal: 0,
            factor_gen: 0,
            turn_start_pos: Vec::new(),
            turn_snapshot: Vec::new(),
            turn_rolls: Vec::new(),
            roll_history: Vec::new(),
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
    /// Whose turn end raises it.
    pub target: usize,
    /// Let one `target` turn end pass first -- set when "the end of your next
    /// turn" is scheduled during that player's own current turn.
    pub skip: bool,
    /// Charge before the status wear-off, rather than after it.
    #[serde(default)]
    pub early: bool,
    /// The card **instance** that asked for the callback, captured at schedule
    /// time: the instance identity belongs here and not to whatever happens to
    /// be on the field when the turn ends.
    ///
    /// `-1` when the card was not in play when it scheduled (`On::Schedule` may
    /// run for a card in a hand or pile); the run then reads 0 named counters, which
    /// is right -- there is no instance to read them from. Two copies of a card
    /// that both schedule produce two entries with two uids, so the callback is
    /// never ambiguous. `#[serde(default)]` so a save from before this field
    /// existed still loads.
    #[serde(default = "no_card_uid")]
    pub uid: i32,
    /// Which `On::Schedule` entry to run (its `id` slug). `""` on a save from
    /// before this field existed -- resolve to the card's first Schedule entry.
    #[serde(default)]
    pub entry: String,
}

/// The "no instance" card uid: the callback is not anchored to a field card.
fn no_card_uid() -> i32 {
    -1
}

/// Things a routine asks the host to do once it commits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    /// A turn began for this player: refill its bonus time and start the clock.
    /// Carries the hooked (`turnBonusTime` / `turnBaseTime`) refill and base.
    StartTimer(usize, f32, f32),
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

/// One pending walk segment awaiting lazy flush (`PlayCtx::walk`).
///
/// A walk's `roll` / `move` event is not logged per step: steps accumulate
/// here and the segment is published when the walk reaches the start square / its last
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
    /// Steps the walk still has after `to` ([`MatchEvent::ahead`]).
    pub ahead: i32,
    pub pending_first: bool,
    /// The walk is the turn's main move: the first publish is kind `roll`
    /// carrying `dice = roll`, not a bare `move`.
    pub is_main_move: bool,
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
    /// Innermost last: `PlayCtx::walk` pushes, so a nested walk's first event must
    /// still flush the outer approach first (see [`World::flush_walk`]).
    stack: Vec<WalkSeg>,
    /// Presentation time owed to the host for flushed segments. [`crate::engine::cx::PlayCtx::wait`]
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

/// One mark mutation, replayed as a `markChanged` raise at commit.
///
/// `card` is the row's provenance captured at mutation time, because the
/// creating instance may already be gone by the drain (`unplace_at` removes
/// before the commit).
#[derive(Debug, Clone, PartialEq)]
pub struct MarkLog {
    pub uid: i32,
    pub tile: i32,
    pub kind: String,
    pub instance: i32,
    pub owner: i32,
    pub delta: i32,
    pub card: Option<String>,
}

/// One [反击] declaration of the current turn (无法将视线移开, design G06 §5.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounteractRecord {
    /// The seat that declared.
    pub declarer: i32,
    /// The seat whose effect/play was countered.
    pub countered: i32,
    /// The declared card.
    pub card: String,
}

/// One completed card play (the copy source 「上一张卡」, design G06 §2.15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayRecord {
    pub card: String,
    pub player: i32,
    /// The play's effect placed a card on the field (excluded from a copy).
    pub placed: bool,
}

/// Most recent [`PlayRecord`]s kept on the world.
pub const MAX_PLAY_LOG: usize = 32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub st: MatchState,
    pub hidden: Vec<Hidden>,
    /// Open inspection frames, innermost last. Ids come from
    /// `next_inspection_id` so the commit net can name what belongs to a
    /// drive.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inspection_frames: Vec<InspectionFrame>,
    #[serde(default)]
    pub next_inspection_id: i32,
    /// Seat whose deck a **just-closed** look still holds: set when the
    /// drive's `inspection_end` closes a frame over that deck, cleared by the
    /// next play's own card filing. Pile writes stay suppressed for the rest
    /// of the activation, including that filing (design G11 I2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub look_hold: Vec<bool>,
    /// Public reveal records, cleared at the outermost routine's end.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reveals: Vec<RevealRecord>,
    pub rng: Rng,
    pub next_event: i32,
    /// The event tail. See [`EventTail`]: a routine snapshot needs this only to
    /// re-derive it on replay, never to mutate what came before, so cloning a
    /// [`World`] bumps refcounts instead of deep-copying `EVENT_KEEP` messages --
    /// which was the single hottest cost in a bot match.
    pub recent: EventTail,
    pub turn: TurnCtx,
    /// Per-seat withheld turn-end settle answer (G03 §4.7). It lives here, not
    /// on [`TurnCtx`], because the answer is given during the turn that applied
    /// the blocking effect -- possibly another player's -- and must survive
    /// into the blocked seat's own turn.
    #[serde(default)]
    pub blocked_settle: Vec<SettleAnswer>,
    pub extra_turns: Vec<usize>,
    pub out_count: i32,
    pub event_deck: Vec<String>,
    pub event_discard: Vec<String>,
    pub event_removed: Vec<String>,
    /// The current turn is over; the host starts the next one.
    pub next_turn_pending: bool,
    /// Deeds of players who left, waiting to be auctioned.
    pub leftovers: VecDeque<Vec<usize>>,
    /// Per-name bonuses added to [`Self::rule_params`] by cards (generic named
    /// rule parameters).
    #[serde(default)]
    pub rule_param_bonus: std::collections::BTreeMap<String, i32>,
    /// Named rule parameters in force (from [`crate::record::MatchSetup::rule_params`]).
    /// Empty = the code default ([`crate::data::default_rule_params`]). Card
    /// bonuses live beside this in [`Self::rule_param_bonus`].
    /// Serde-skipped when empty so a default-param match's `save()` is
    /// byte-identical to a pre-field save (checkpoint hashes).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub rule_params: std::collections::BTreeMap<String, i32>,
    /// The tile table in force (from [`crate::record::MatchSetup::board`]).
    /// `None` = the host's `data.tiles`. Serde-skipped when absent so a
    /// host-board match's `save()` is byte-identical to a pre-field save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<Vec<crate::data::TileData>>,
    /// v78: host-supplied recorded roll for a Filter body. `None` = none.
    /// The host pre-rolls into the recorded RNG stream before a DiceRollPlan /
    /// DiceRollValue Filter raise so the body can read it without calling `roll`.
    #[serde(default)]
    pub recorded_roll: Option<i32>,
    pub ask_seq: i32,
    /// The open roll transaction stack (a roll can nest: an effect roll inside
    /// a move window, a recast inside a counteraction).
    #[serde(default)]
    pub open_rolls: Vec<crate::engine::roll::RollTransaction>,
    /// The next roll-transaction id (`rolls_by_player` / `roll_face` key on
    /// it; never written to a card slot).
    #[serde(default)]
    pub next_roll_id: i32,
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
    /// `gain_fixed`). Zeroed for everyone at each turn start, so a counteraction read at
    /// a turn's end sees just that turn. The count a hand card reads without any
    /// field stand-in (HHW:（育美）(2)).
    #[serde(default)]
    pub gains: Vec<i32>,
    /// [反击] declarations made this turn, in declaration order. Cleared at
    /// each turn start (design G06 §6.5).
    #[serde(default)]
    pub countered_this_turn: Vec<CounteractRecord>,
    /// The most recent completed card plays, oldest first, capped at
    /// [`MAX_PLAY_LOG`]. Not cleared at a turn start -- 「上一张卡」 has no
    /// turn bound (design G06 §2.15).
    #[serde(default)]
    pub played_cards: Vec<PlayRecord>,
    /// Marker **ownership** (user ruling 2026-10-07): a marker is owned by the
    /// **rule that creates it**, wherever its copies sit. Maps a marker name
    /// (「抹茶芭菲」, 「P✽P粉丝」, …) to the owning rule's card id
    /// (`skill:要乐奈:投币式停车场的猫` owns **all** 抹茶芭菲 -- its player's
    /// counter, every other player's counter, and those on tiles).
    ///
    /// Stamped on first creation from the creating run's `current_card`.
    /// Bankruptcy of the owning rule's player removes every copy of the
    /// marker, wherever it sits (`remove_from_game`). Neutral board marks
    /// ([tile marks], `owner == -1`) have no player-owned rule and remain.
    #[serde(default)]
    pub marker_owner: std::collections::BTreeMap<String, String>,
    /// Pending walk segments + flush pacing (see [`WalkSeg`]). Transient: not
    /// serialized, not compared.
    #[serde(skip)]
    pub walk_flush: WalkFlush,
    /// Mark mutations of the current run, drained by `commit_after` into
    /// `markChanged` raises. Transient: a save at a routine boundary has no
    /// live run and the action replays from its pre-action state.
    #[serde(skip)]
    pub mark_log: Vec<MarkLog>,
    /// When set, `log` does not push an event (the caller posts write-through).
    #[serde(skip)]
    pub skip_log: bool,
    /// Generic "end this turn" latch. A guest `end_turn_now(player)` sets it;
    /// `turn_start` honors it after the `turnStartBefore` / `turnStart` raises
    /// (log nothing, `wait(1.0)`, `end_turn`) -- the auto-skip pacing every
    /// status host uses. Transient: never serialized (save byte-stability).
    #[serde(skip)]
    pub end_turn_request: Option<i32>,
    /// Stub event returned when [`Self::skip_log`] is set.
    #[serde(skip)]
    pub stub: MatchEvent,
    /// Once-per-flash identity (ABI v55), shared across `World::clone` so a
    /// prompt's routine re-run (which restarts from a snapshot clone) sees the
    /// flashes the pre-prompt pass already posted. A flash already shown
    /// before the prompt must not play again after the answer. Transient.
    #[serde(skip)]
    pub flash: SharedFlash,
    /// Match clock / timer settings (see [`crate::clock`]). Constant for the
    /// life of the match; rides here so a routine's [`super::PlayCtx`] can read the
    /// setup windows and refill caps. `#[serde(default)]` so a World snapshot
    /// written before this field still loads (pre-settings values).
    #[serde(default)]
    pub clock: crate::clock::ClockSettings,
    /// Legacy clock semantics -- see `engine::Match` / `record::MatchSetup::clock`.
    /// `true` only on a pre-settings record restore: base-first drain, base
    /// reset-to-hook-value, prompts never draw on a pool.
    #[serde(default)]
    pub legacy_clock: bool,
    /// The rollers whose move roll windows are open, innermost last. A
    /// `run_planned_move` issued from the owner's own window replaces the
    /// pending walk (`spec/abi/G04-move-plan.md` §4.4).
    #[serde(default)]
    pub move_roll_windows: Vec<usize>,
}

/// Once-per-flash state (ABI v55), shared across `World::clone` via [`Arc`].
/// `PartialEq` is always-equal: the state is transient and not part of the
/// compared match state (like `walk_flush`).
#[derive(Debug, Default)]
pub struct FlashState {
    /// Identity key -> already-posted event id.
    pub seen: std::collections::HashMap<String, i32>,
    /// Next flash ordinal within the current drive.
    pub ordinal: u32,
    /// `ctx::flash` was called.
    pub requested: bool,
    /// `ctx::no_flash` was called.
    pub no_flash: bool,
    /// Flash events already posted, carried across a prompt's re-run (the
    /// re-run starts from a snapshot and would drop them).
    pub saved: Vec<MatchEvent>,
}

/// Shared handle to [`FlashState`] (COW-free: clones share, which is the point).
#[derive(Debug, Clone, Default)]
pub struct SharedFlash {
    inner: Arc<std::sync::Mutex<FlashState>>,
}

impl SharedFlash {
    pub fn lock(&self) -> std::sync::MutexGuard<'_, FlashState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl PartialEq for SharedFlash {
    fn eq(&self, _other: &Self) -> bool {
        true // transient; not part of compared state
    }
}

impl World {
    // ------------------------------------------------ hand limit

    /// Clear the permitted-over-limit state once the hand fits again
    /// (「可超过上限」). Called from every hand-shrink site; a no-op while the
    /// hand is still over.
    pub fn settle_over_hand_allowance(&mut self, i: usize) {
        let fits = self
            .hidden
            .get(i)
            .is_some_and(|h| !self.st.players[i].over_hand(h.hand.len()));
        if fits {
            self.st.players[i].over_hand_allowed = false;
        }
    }

    // ------------------------------------------------ plan frames

    /// The plan the move currently reads: the innermost frame, else the
    /// staging buffer.
    pub fn plan(&self) -> &crate::engine::move_ctx::MoveCtx {
        self.turn
            .plan_stack
            .last()
            .map_or(&self.turn.plan, |f| &f.plan)
    }

    /// The plan a card write lands on: the innermost frame, else staging.
    pub fn plan_mut(&mut self) -> &mut crate::engine::move_ctx::MoveCtx {
        self.turn
            .plan_stack
            .last_mut()
            .map_or(&mut self.turn.plan, |f| &mut f.plan)
    }

    /// Open a plan frame for a move `owner` is about to run. `target` is the
    /// plan the caller passed; at depth 0 a Main frame takes it whole and a
    /// Side frame takes the command delta, while a nested frame takes the delta
    /// against the innermost command (`spec/abi/G04-move-plan.md` §3.2).
    pub fn begin_plan(
        &mut self,
        owner: usize,
        kind: crate::engine::move_ctx::PlanKind,
        target: crate::engine::move_ctx::MoveCtx,
    ) {
        use crate::engine::move_ctx::MoveCtx;
        let command = self.turn.plan_scope.commands.last().cloned();
        let depth = self.turn.plan_stack.len();
        let saved = command
            .as_ref()
            .map_or_else(|| self.turn.plan.clone(), |c| c.staging_before.clone());
        let mut seed = if depth == 0 {
            match kind {
                crate::engine::move_ctx::PlanKind::MainMove => target,
                crate::engine::move_ctx::PlanKind::SideMove => command.as_ref().map_or_else(
                    || self.turn.plan.clone(),
                    |c| MoveCtx::staging_seed(&c.plan_before, &self.turn.plan, &target),
                ),
            }
        } else {
            // A nested frame is a side frame; its `saved` is unused.
            let current = self.plan().clone();
            let base = command.as_ref().map_or(&current, |c| &c.plan_before);
            MoveCtx::staging_seed(base, &current, &target)
        };
        if depth == 0 {
            // G02 §5.5: the `nextMove` factor is for exactly this move.
            let next = std::mem::take(&mut self.turn.next_move_pay_factor);
            self.turn.next_move_pay_factor = 1000;
            seed.pay_factor *= f64::from(next) / 1000.0;
        }
        if kind == crate::engine::move_ctx::PlanKind::MainMove && depth == 0 {
            // `is_main_move` is a Command field, so the frame carries it while
            // a roll window hook reads the plan.
            seed.is_main_move = true;
            if let Some(steps) = self.hidden.get_mut(owner).and_then(|h| h.next_steps.take()) {
                seed.next_steps = Some(steps);
            }
        }
        let kind = if depth > 0 {
            crate::engine::move_ctx::PlanKind::SideMove
        } else {
            kind
        };
        self.turn
            .plan_stack
            .push(crate::engine::move_ctx::PlanFrame {
                plan: seed,
                saved,
                kind,
                owner,
            });
        self.turn.move_open = true;
    }

    /// Pop the innermost frame and apply the restore table.
    pub fn end_plan(&mut self, ran: bool) {
        use crate::engine::move_ctx::{MoveCtx, PlanKind};
        let Some(frame) = self.turn.plan_stack.pop() else {
            return;
        };
        // The span is the whole walk: a landing inside a nested move belongs
        // to the walk it sits in, so the bit climbs to the frame still open.
        if let Some(parent) = self.turn.plan_stack.last_mut() {
            parent.plan.abnormal |= frame.plan.abnormal;
        }
        // The move span outlives its frame: a later move does not clear the
        // latch (only the turn start does), and 「该次移动过程中」 at the
        // turn end still needs it.
        if frame.kind == PlanKind::MainMove {
            self.turn.move_abnormal |= frame.plan.abnormal;
        }
        if self.turn.plan_stack.is_empty() {
            self.turn.plan = match (frame.kind, ran) {
                (PlanKind::MainMove, true) => MoveCtx::default(),
                _ => frame.saved,
            };
            self.turn.move_open = false;
        }
    }

    /// Enter an activation body's command scope (`resumed`: the body is being
    /// re-run after a host pause and the saved world still holds it, so no
    /// second snapshot is taken).
    pub fn enter_plan_command(&mut self, resumed: bool) {
        if resumed {
            return;
        }
        let command = crate::engine::move_ctx::PlanCommand {
            staging_before: self.turn.plan.clone(),
            plan_before: self.plan().clone(),
            moves_at_entry: self.turn.plan_scope.moves,
        };
        self.turn.plan_scope.commands.push(command);
    }

    /// Leave the innermost command scope. An outermost command that drove a
    /// move drops the writes the replayed body made after its last move; one
    /// that drove none keeps them (they stage the next main move).
    pub fn leave_plan_command(&mut self) {
        let Some(command) = self.turn.plan_scope.commands.pop() else {
            return;
        };
        // Per-pair designation cancels are **not** cleared here: they are written
        // by the counteraction body of the play's `card` window, which runs as
        // its own command (and so leaves before the play's body). They are
        // scoped to the play instead -- cleared at its start.
        if self.turn.plan_scope.commands.is_empty()
            && self.turn.plan_scope.moves > command.moves_at_entry
        {
            self.turn.plan = command.staging_before;
        }
    }

    /// The withheld turn-end settle answer for `player` (`Unanswered` when
    /// never stamped; a save predating the field has a short vector).
    pub fn blocked_settle_answer(&self, player: usize) -> SettleAnswer {
        self.blocked_settle.get(player).copied().unwrap_or_default()
    }

    /// Arm / clear `player`'s withheld turn-end settle answer.
    pub fn set_blocked_settle_answer(&mut self, player: usize, a: SettleAnswer) {
        if self.blocked_settle.len() <= player {
            self.blocked_settle
                .resize(player + 1, SettleAnswer::Unanswered);
        }
        self.blocked_settle[player] = a;
    }

    /// Has the turn end this `end` asks for already happened? 结束 with the
    /// next turn queued, or the same seat's **extra turn** already open -- a
    /// late duplicate lands on that 开始 (the queued turn kept the seat, and
    /// an end from any other seat is refused as not their turn), so it must
    /// not run the end phase again. A duplicate `end` folds into both.
    pub fn end_already_queued(&self) -> bool {
        match self.st.step {
            crate::state::stage::END => self.next_turn_pending,
            crate::state::stage::START => true,
            _ => false,
        }
    }

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
            blocked_settle: vec![SettleAnswer::Unanswered; players],
            extra_turns: vec![],
            out_count: 0,
            inspection_frames: Vec::new(),
            next_inspection_id: 1,
            look_hold: vec![false; players],
            reveals: Vec::new(),
            event_deck: vec![],
            event_discard: vec![],
            event_removed: vec![],
            next_turn_pending: false,
            end_turn_request: None,
            leftovers: VecDeque::new(),
            rule_param_bonus: Default::default(),
            rule_params: Default::default(),
            board: None,
            recorded_roll: None,
            ask_seq: 0,
            open_rolls: Vec::new(),
            next_roll_id: 1,
            signals: vec![],
            scheduled: vec![],
            targeted: vec![],
            gains: vec![],
            countered_this_turn: vec![],
            played_cards: vec![],
            marker_owner: std::collections::BTreeMap::new(),
            walk_flush: WalkFlush::default(),
            mark_log: Vec::new(),
            skip_log: false,
            stub: MatchEvent::default(),
            flash: SharedFlash::default(),
            clock: crate::clock::ClockSettings::default(),
            legacy_clock: false,
            move_roll_windows: Vec::new(),
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

    /// What `player_id` paid other players during this turn's settles
    /// (`ctx::paid_in_settle_by`). 0 outside a seat's range.
    /// The ordinary-settle rent figure of `t` as `pay_rent` charges it, plus
    /// its detail note (`log.part.rent_houses` / `log.part.rent_dice`).
    /// **Single source** for the engine's `pay_rent` / `card_charge_rent` and
    /// the guest `rent_quote` reader. Needs no `PlayCtx`, so a card `Run` can call
    /// it.
    pub fn rent_figure(
        &mut self,
        data: &crate::data::GameData,
        owner: usize,
        t: usize,
    ) -> (i32, Option<crate::msg::Msg>) {
        use crate::msg::Msg;
        // The tile prop marks dice rent; the unit comes from the live
        // `rentUnitMult` rule param (a card's +N lands there).
        if self.tile_prop(t as i32, crate::state::prop::RENT_DICE_MULT) > 0 {
            let dice_mult = self.rent_unit_mult();
            let d = self.rng.d(20);
            let kind = data.tiles[t].kind.as_str().to_string();
            let unit = super::play::purchase::dice_rent_unit(data, self, owner, &kind, dice_mult);
            let n = super::play::purchase::count_owned_kind(data, self, owner, &kind).max(1);
            (
                unit * d,
                Some(
                    Msg::new("log.part.rent_dice")
                        .i("n", n)
                        .i("mult", dice_mult)
                        .i("d", d),
                ),
            )
        } else {
            // The counted house count ([`Self::rent_houses`]): a
            // 「房屋数视为…」 override rides here, and real `st.houses` is
            // untouched.
            let h = self.rent_houses(t as i32);
            let rent = super::play::purchase::table_rent(data, self, t);
            (
                rent,
                (h > 0).then(|| Msg::new("log.part.rent_houses").i("h", h)),
            )
        }
    }

    /// The figure a fresh `payer -> owner` rent charge on `t` would move here:
    /// base x the tile's own rent scale x the standing plan and open-settle
    /// pay factors, one end-of-calculation round, and **not** the in-flight
    /// payment's own rent factor or `milli_scale`. `-1` = no rent charge
    /// (unowned, own tile, mortgaged, an endpoint out, or a zero table).
    /// Moves no money but rolls a fresh d20 on a dice-rent tile.
    pub fn rent_quote(&mut self, data: &crate::data::GameData, payer: usize, t: usize) -> i32 {
        let owner = self.st.owners.get(t).copied().unwrap_or(-1);
        if owner < 0
            || owner as usize == payer
            || self.out(payer)
            || self.out(owner as usize)
            || self.st.mortgaged.get(t).copied().unwrap_or(false)
        {
            return -1;
        }
        let (mut amount, _) = self.rent_figure(data, owner as usize, t);
        if amount <= 0 {
            return -1;
        }
        // The open settle's addends ride the quote exactly as the charge takes
        // them (add phase before the factors): the pay addend on every leg and
        // the rent addend on this one -- a quote is always a rent leg.
        let add = self
            .turn
            .settle_pay_add
            .saturating_add(self.turn.settle_rent_add);
        let mut modified = false;
        if add != 0 {
            amount = amount.saturating_add(add);
            modified = true;
        }
        // The same arithmetic [`super::play::Play::scale_settle_payment`] runs
        // for a rent, minus the in-flight rent factor and with
        // `milli_scale = 1000`: the tile's rent half and every standing plan /
        // open-settle pay factor, then the single end-of-calculation round the
        // pay chain applies.
        // `plan()`, not staging: a quote made inside a live move must see the
        // factors that move's frame carries (`G04-move-plan.md` R13: the
        // window-time reads move to `plan()`), else a counter's ×0.5 applied
        // to the very settle being quoted is missed.
        let mut f = self.plan().pay_factor;
        f *= self.turn.settle_pay_factor;
        f *= super::play::purchase::tile_money_scale(self, t, true);
        let scaled = if (f - 1.0).abs() <= f64::EPSILON {
            amount
        } else {
            (amount as f64 * f) as i32
        };
        if (scaled != amount || modified) && scaled > 0 {
            super::play::ceil10_money(scaled)
        } else {
            scaled
        }
    }

    pub fn paid_in_settle_by(&self, player_id: i32) -> i32 {
        usize::try_from(player_id)
            .ok()
            .and_then(|s| self.turn.paid_in_settle_by.get(s).copied())
            .unwrap_or(0)
    }

    /// Zero every player's turn-gain counter (「当前回合内」 starts fresh).
    pub fn reset_gains(&mut self) {
        self.gains.iter_mut().for_each(|n| *n = 0);
    }

    /// Drop the turn's counteraction history (「当前回合内」 starts fresh).
    pub fn reset_countered(&mut self) {
        self.countered_this_turn.clear();
    }

    /// Append one completed card play to the copy-source log (「上一张卡」),
    /// dropping the oldest past [`MAX_PLAY_LOG`]. Not cleared at a turn start.
    pub fn record_play(&mut self, card: &str, player: i32, placed: bool) {
        self.played_cards.push(PlayRecord {
            card: card.to_string(),
            player,
            placed,
        });
        let over = self.played_cards.len().saturating_sub(MAX_PLAY_LOG);
        if over > 0 {
            self.played_cards.drain(..over);
        }
    }

    /// The one credit write path: add `amount` to the player's money and count
    /// the gain (「当前回合内你每获得过一次资金」). Every money-in funnels through
    /// here; a debit never bumps.
    pub fn credit(&mut self, player_id: i32, amount: i32) {
        let Ok(s) = usize::try_from(player_id) else {
            return;
        };
        if let Some(p) = self.st.players.get_mut(s) {
            p.money += amount;
            self.bump_gain(player_id, 1);
        }
    }

    /// Append an event to the log. Returns it for further fields.
    ///
    /// Lazy walk flush: any other event while a [`WalkSeg`] is pending
    /// publishes the approach first, so the client walks to the tile before
    /// showing the card flash / pay / effect that ran there. The flush's own
    /// event re-enters here under [`WalkFlush::flushing`] and is not re-flushed.
    pub fn log(&mut self, kind: &str, player_id: i32, msg: Msg) -> &mut MatchEvent {
        let _tg = crate::engine::rtimer::guard(&crate::engine::rtimer::LOG_NS);
        self.flush_walk();
        if self.skip_log {
            // The caller posts write-through; keep a stub so `.value =` works.
            self.stub = MatchEvent::default();
            return &mut self.stub;
        }
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
    /// and remains pending, so its head line lands with the first real step.
    pub fn flush_walk(&mut self) {
        if self.walk_flush.flushing {
            return;
        }
        self.walk_flush.flushing = true;
        // Snapshot what to publish (and reset the slots) before logging:
        // `log` takes `&mut self`, so the stack cannot remain borrowed.
        let mut pending = Vec::new();
        for seg in self.walk_flush.stack.iter_mut() {
            if seg.value == 0 && !(seg.pending_first && seg.is_main_move) {
                continue;
            }
            let ek = if seg.pending_first && seg.is_main_move {
                "diceRollBefore"
            } else {
                "move"
            };
            let text = if seg.pending_first {
                seg.head.clone()
            } else {
                Msg::new("log.move_on").player_id("who", seg.player)
            };
            let dice = if seg.pending_first && seg.is_main_move {
                seg.roll
            } else {
                0
            };
            pending.push((
                ek, text, seg.player, seg.from, seg.to, seg.value, dice, seg.ahead,
            ));
            seg.from = seg.to;
            seg.value = 0;
            seg.pending_first = false;
        }
        for (ek, text, player, from, to, value, dice, ahead) in pending {
            let e = self.log(ek, player, text);
            e.from = from;
            e.to = to;
            e.value = value;
            e.dice = dice;
            e.ahead = ahead;
            // Same pacing the old per-publish `PlayCtx::wait` applied (1.5s for the
            // head `roll`, 0.3s for a continuation, plus 0.15s/step).
            let pace = if ek == "diceRollBefore" { 1.5 } else { 0.3 };
            self.walk_flush.delay += pace + value.unsigned_abs() as f32 * 0.15;
        }
        self.walk_flush.flushing = false;
    }

    /// Drain the presentation time [`Self::flush_walk`] accumulated since the
    /// last drain (the host's `PlayCtx::wait` equivalent).
    pub fn take_walk_delay(&mut self) -> f32 {
        std::mem::take(&mut self.walk_flush.delay)
    }

    /// Arm a new innermost walk segment (`PlayCtx::walk` on entry).
    pub fn push_walk_seg(&mut self, seg: WalkSeg) {
        self.walk_flush.stack.push(seg);
    }

    /// Drop the innermost walk segment (`PlayCtx::walk` on exit, after flushing).
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
    /// card's player, `target` the affected player and `tile` where it ran
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
        for (index, (player_id, h)) in st.players.iter_mut().zip(&self.hidden).enumerate() {
            player_id.hand = h.hand.len() as i32;
            player_id.draw = h.deck.len() as i32;
            player_id.discard = h.graveyard.clone();
            let seat = index as i32;
            let record = self.reveals.iter().rev().find(|r| r.player == seat);
            // A public frame's held cards are published draw-pile ids: they
            // are off the deck but visible. Innermost frame first.
            let held: Vec<String> = self
                .inspection_frames
                .iter()
                .rev()
                .filter(|f| f.deck_owner == seat && f.public)
                .flat_map(|f| {
                    f.cards
                        .iter()
                        .filter(|c| c.revealed)
                        .map(|c| c.id.clone())
                        .collect::<Vec<_>>()
                })
                .collect();
            player_id.reveal = match (record, held.is_empty()) {
                (None, true) => None,
                (record, _) => {
                    let mut view = crate::state::RevealView {
                        reason: record.map(|r| r.reason.clone()).unwrap_or_default(),
                        ..Default::default()
                    };
                    // A list published as a whole pile follows the pile: the
                    // overlay is the live view, not a snapshot of the reveal
                    // (a card drawn or kept during the look leaves it).
                    if let Some(r) = record {
                        view.hand = if r.piles[0] {
                            h.hand.clone()
                        } else {
                            r.hand.clone()
                        };
                        // A public look publishes the deck for as long as its
                        // clause runs -- open frame, or the filing right after
                        // it (`look_hold`) -- so the deck list follows the
                        // pile, not the reveal moment.
                        let published = r.piles[1]
                            || self.look_hold.get(index).copied().unwrap_or(false)
                            || self
                                .inspection_frames
                                .iter()
                                .any(|f| f.deck_owner == seat && f.public);
                        view.deck = if published {
                            h.deck.iter().rev().cloned().collect()
                        } else {
                            r.deck.clone()
                        };
                        view.discard = if r.piles[2] {
                            h.graveyard.clone()
                        } else {
                            r.discard.clone()
                        };
                    }
                    // The frame is the true top; a deck reveal made while it
                    // was open already listed the held cards, so drop that
                    // copy (one occurrence each) before splicing them in
                    // front, and each card shows exactly once.
                    for id in &held {
                        if let Some(pos) = view.deck.iter().position(|c| c == id) {
                            view.deck.remove(pos);
                        }
                    }
                    view.deck.splice(0..0, held);
                    Some(view)
                }
            };
        }
        // The recent-event window: shared via `Arc`, rebuilt in `EventTail`
        // only when something was logged since the last read.
        st.events = self.recent.window();
        st.event_deck = self.event_deck.len() as i32;
        st.event_discard = self.event_discard.clone();
        st.event_removed = self.event_removed.clone();
        // Active events ride their rule instance (`docs/EVENTS.md`): the view's
        // counters mirror the instance's named counters / props so the client
        // shows what the body is counting without a second store.
        for e in st.event_active.iter_mut() {
            let rid = crate::data::event_rule_id(&e.id);
            if let Some(f) = st.board_field.iter().find(|f| f.card == rid && f.tile < 0) {
                e.counter = f.counters.values().sum();
                e.counter2 = f.props.get("count2").copied().unwrap_or(0);
                e.turns_left = f
                    .props
                    .get(crate::state::prop::TURNS_LEFT)
                    .copied()
                    .unwrap_or(0);
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
