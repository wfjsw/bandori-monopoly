//! Match state broadcast to clients.
//!
//! Field names and defaults mirror the JSON the wire already carries: missing
//! fields take `#[serde(default)]`, and the camelCase spelling is the contract.
//! Display text is a localizable [`Msg`] rather than a finished string.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::msg::Msg;

/// The movement the current turn is taking: the precomputed `reach[]` of tiles
/// and how far along it the player has walked. `player_id == -1` when there is
/// no plan (teleports and settle walks may skip it).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MovePlan {
    pub player_id: i32,
    pub from: i32,
    pub steps: i32,
    pub started: bool,
    /// `reach[k]` is the tile after k+1 steps of the plan.
    pub reach: Vec<i32>,
    /// Whether this move can build where it lands (see `MoveCtx::can_build`).
    /// Carried here so the build gate at step 3 can still read it -- the live
    /// `MoveCtx` is gone by then.
    pub can_build: bool,
    /// Whether this move may build away from the landing
    /// (`MoveCtx::build_anywhere`, HOST-08). Was: lifts `why_not_build`'s
    /// landing-only check. Skipped when `false` so saves (and ckpt_equiv
    /// turn hashes) remain byte-identical to the pre-HOST-08 shape.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub build_anywhere: bool,
}

impl MovePlan {
    /// Where the walk ends at the current `steps`.
    pub fn landing(&self) -> i32 {
        let n = self.reach.len();
        if self.steps <= 0 || self.steps as usize > n {
            self.from
        } else {
            self.reach[self.steps as usize - 1]
        }
    }
}

/// Turn stages -- the rulebook's four, in order (`docs/rulebook/rulebook.txt:2957`):
/// 开始阶段, 运营阶段, 移动阶段, 结束阶段. [`MatchState::step`] holds one of
/// these, or [`stage::NONE`] before a turn starts. They are numbered to match
/// the rulebook so a client can show 「阶段 N」 without translating.
pub mod stage {
    /// No turn in progress: the match has not started, or is between turns.
    pub const NONE: i32 = 0;
    /// 开始阶段 -- the turn starts: timed-counter ticks, `turnStart`, the gate checks.
    pub const START: i32 = 1;
    /// 运营阶段 -- play cards, roll the dice, buy / redeem / mortgage.
    pub const OPS: i32 = 2;
    /// 移动阶段 -- the main move.
    pub const MOVE: i32 = 3;
    /// 结束阶段 -- the move has resolved. (Buy/build are ordinary OPS actions,
    /// not an end-step phase; this stage only closes the turn.)
    pub const END: i32 = 4;
}

/// Which way money flows for the viewer of a [`TileQuote`]. The UI's colour
/// channel (`--money-pay` / `--money-optional` / `--money-receive`); `kind`
/// carries the tooltip wording, `flow` the direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MoneyFlow {
    /// Forced payment out of the viewer (rent on someone else's tile).
    MustPay,
    /// Optional spend the viewer may choose (buy / build / force-buy / redeem).
    MayPay,
    /// Income the viewer would collect (rent on their own tile).
    Receive,
}

/// What a [`TileQuote`]'s number means -- the tooltip wording, not the colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TileQuoteKind {
    /// Unowned land: the price to buy it.
    Buy,
    /// Mine, next building available: its construction cost.
    Build,
    /// Someone else's: the rent I would pay on stepping onto it.
    Rent,
    /// Mine, no next building: the rent I would collect (build cap / unbuildable).
    OwnRent,
    /// Mortgaged, someone else's: the force-buy price (「强行购买」).
    ForceBuy,
    /// Mortgaged, mine: the redeem price. (Force-buy does not apply to one's
    /// own deed; redeeming is the natural cost to clear the mortgage.)
    Redeem,
}

/// One tile's "what to expect" number, from a viewer's seat.
/// Parallel to the board's tiles in [`MatchView`] (`tileQuotes[i]`).
/// `value` is the figure; `max` is the upper bound of a dice range (the flat deed
/// rent is `deed-level × rent_unit_mult × 1d20`, so `value` is the minimum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileQuote {
    pub kind: TileQuoteKind,
    pub flow: MoneyFlow,
    pub value: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,
}

/// One speaking declaration in a tile's deed-text list
/// ([`MatchState::tile_texts`]). Ordered by body precedence; the consumer
/// takes the first entry of each [`Self::slot`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct TileTextEntry {
    /// Rule id that speaks (the start-square tile rule, `event:the start-square rebuild event`, ...).
    pub from: String,
    /// `"settle"` or `"pass"` (the settle / [pass] half).
    pub slot: String,
    /// i18n key in the `ui` namespace (`deed.note.*`); `""` = this speaker
    /// suppresses the slot (renders nothing).
    pub key: String,
}

/// The public match snapshot: board, players, phase, and the recent events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchState {
    /// `"order" | "ban" | "pick" | "deck" | "play" | "ended"`; empty = no match.
    pub phase: String,
    pub match_id: i32,
    pub seq: i32,
    pub mode: i32,
    pub debug_open: bool,
    pub turn: i32,
    pub round: i32,
    pub step: i32,
    pub roller: i32,
    pub busy: bool,
    pub skip_move: bool,
    pub landed: i32,
    /// Room think-time setting: 0 Standard / 1 Relaxed / 2 VeryRelaxed / 3 Unlimited.
    pub think_time: i32,
    /// Would `act {act:"diceRollBefore"}` be accepted
    /// (`why_not_act`'s roll branch: the live `skip_move` latch, the
    /// `main_moved` flag, and the roller).
    pub can_roll_here: bool,
    /// The same for `act {act:"end"}` (the end branch:
    /// `roll_first` while a main move is still owed, `moving`, `over_hand`).
    /// [`Self::skip_move`] is the same live latch, so the two always agree
    /// (it was once re-derived from the named layer counters, which did not: the
    /// mid-turn layer grants -- a bot trusting it sent `end` into
    /// `err.roll_first`).
    pub can_end_here: bool,
    /// How many hand cards / skills the turn has already played
    /// (`world.turn.played.len()`) -- what the standard policy's
    /// per-turn play cap counts. Filled with [`Self::can_roll_here`] /
    /// [`Self::can_end_here`].
    pub played_count: i32,
    /// Members a driver must move **right now** (`docs/BOT-DRIVE.md`): every
    /// bot seat, every machine-held human (`ai` -- 托管 takeover, disconnect),
    /// and the acting seat once its own clock is spent. The host (server /
    /// page) probes exactly these; the engine decides for nobody. Empty in
    /// Solo -- the page is the host there.
    #[serde(default)]
    pub drive: Vec<i32>,
    /// The movement this turn is taking.
    pub plan: MovePlan,
    pub time_left: f32,
    pub base_time: f32,
    pub bonus_time: f32,
    /// True while the acting seat's turn clock is held waiting for that seat's
    /// `anim_done` ack (or the hold cap). The server publishes no deadline
    /// while this is set; clients show the clock held at full. Solo and
    /// machine seats never hold.
    #[serde(default)]
    pub clock_paused: bool,
    /// Next uid to hand out for a card instance. **Global** across the match, not
    /// per player: every card in the game has one id, so an instance is addressed
    /// by `uid` alone and one player may hold several copies of the same card in
    /// play at once. `#[serde(default)]` so a state saved before this existed
    /// still loads (it re-numbers from 0 on the next placement).
    #[serde(default)]
    pub next_card_uid: i32,
    /// Next [`TileMark::uid`] to hand out. Monotonic and never recycled for the
    /// lifetime of the match, so a stored mark handle either resolves to exactly
    /// the row it was issued for or reports dead -- it never aliases onto a
    /// later mark. `#[serde(default)]` so a save from before this field still
    /// loads (0); the allocator also floors at `max(st.marks.uid)+1`, so an old
    /// save can never hand out a uid that collides with a row already present.
    #[serde(default)]
    pub next_mark_uid: i32,
    pub players: Vec<MatchPlayer>,
    /// The **neutral board owner**'s field ([`BOARD_OWNER`]): one rule instance
    /// per board tile, placed at match start by `bind_tiles` the way
    /// `bind_skills` places a player's skills. Each carries `tile = <board tile
    /// index>` and the tile's data in its [`FieldCard::props`]. Not a seat --
    /// turn order, scoring and the alive/out loops never see it.
    #[serde(default)]
    pub board_field: Vec<FieldCard>,
    pub bans: Vec<String>,
    pub owners: Vec<i32>,
    pub houses: Vec<i32>,
    pub mortgaged: Vec<bool>,
    pub embers: Vec<i32>,
    pub marks: Vec<TileMark>,
    pub event_deck: i32,
    pub event_top: Vec<String>,
    pub event_discard: Vec<String>,
    pub event_removed: Vec<String>,
    pub event_active: Vec<ActiveEvent>,
    pub prompt: MatchPrompt,
    pub vote: MatchVote,
    /// The recent-event window (last [`crate::engine::world::STATE_EVENTS`]).
    /// `Arc` so a `state()` poll whose window has not changed is a refcount
    /// bump rather than 80 event clones -- the view builder is on the sim's
    /// per-tick loop and the server's broadcast path.
    pub events: Arc<Vec<MatchEvent>>,
    /// Per-tile effective deed-text speakers, in body-precedence order.
    /// Composed by the view from board-owned (and tile-carrying player) rule
    /// instances' declared texts. See `docs/TILES.md` 「Tile text」. Index is
    /// the board tile; each inner list is ordered replacements-first, base
    /// last. The consumer takes the first entry of each `slot`; a `key` of
    /// `""` suppresses that slot. Serde-defaulted so a client or save from
    /// before this field still loads (the deed popup falls back to its
    /// hard-coded notes).
    #[serde(default)]
    pub tile_texts: Vec<Vec<TileTextEntry>>,
    pub end_reason: String,
    pub winner: i32,
    pub score_money: f32,
    pub score_property: f32,
    pub score_houses: f32,
}

impl Default for MatchState {
    fn default() -> Self {
        Self {
            phase: String::new(),
            match_id: 0,
            seq: 0,
            mode: 0,
            debug_open: false,
            turn: -1,
            round: 0,
            step: 0,
            roller: -1,
            busy: false,
            skip_move: false,
            landed: -1,
            think_time: 0,
            can_roll_here: false,
            can_end_here: false,
            played_count: 0,
            drive: vec![],
            plan: MovePlan::default(),
            time_left: 0.0,
            base_time: 0.0,
            bonus_time: 0.0,
            clock_paused: false,
            next_card_uid: 1,
            next_mark_uid: 0,
            players: vec![],
            board_field: vec![],
            bans: vec![],
            owners: vec![],
            houses: vec![],
            mortgaged: vec![],
            embers: vec![],
            marks: vec![],
            event_deck: 0,
            event_top: vec![],
            event_discard: vec![],
            event_removed: vec![],
            event_active: vec![],
            prompt: MatchPrompt::default(),
            vote: MatchVote::default(),
            events: Arc::default(),
            tile_texts: vec![],
            end_reason: String::new(),
            winner: -1,
            score_money: 1.0,
            score_property: 1.0,
            score_houses: 1.0,
        }
    }
}

impl MatchState {
    pub fn active(&self) -> bool {
        !self.phase.is_empty()
    }

    pub fn ranked(&self) -> bool {
        self.mode == crate::MatchMode::Ranked as i32
    }

    /// Player whose turn it is.
    pub fn current(&self) -> Option<&MatchPlayer> {
        usize::try_from(self.turn)
            .ok()
            .and_then(|t| self.players.get(t))
    }

    /// Players still in the game.
    pub fn alive(&self) -> usize {
        self.players.iter().filter(|s| !s.out()).count()
    }

    /// Player index of a room member, or -1.
    pub fn player_of(&self, member: i32) -> i32 {
        self.players
            .iter()
            .position(|s| s.member == member)
            .map_or(-1, |i| i as i32)
    }

    /// Is the event active and face-up?
    pub fn event_active(&self, id: &str) -> bool {
        self.event_active.iter().any(|e| e.id == id && !e.face_down)
    }
}

/// When a keyed state item wears off (see [`StateVar::expires`]).
///
/// The engine's turn flow ticks every item whose expiry is due, so "this
/// counter lasts a turn" is data carried by the item, not a key the engine has
/// to know by name. That is what keeps [`MatchPlayer::tick_state`] free of
/// `match key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tick {
    /// Loses one layer at its owner's start of turn.
    TurnStart,
    /// Loses one layer at its owner's end of turn.
    TurnEnd,
}

/// Generic flags on a [`StateVar`]: how the engine treats the key, without
/// naming the key in match arms.
pub mod state_flag {
    /// Status-like counter: writes floor at 0 (「清除」).
    pub const NEVER_NEGATIVE: u8 = 1;
    /// Engine-owned key (turn rules / status gates), not a card's scratch.
    pub const ENGINE_OWNED: u8 = 2;
}

/// One keyed state item: `value`, the advertised bounds, and when it wears off.
///
/// The engine holds these and enforces **only** the writer-declared
/// [`state_flag::NEVER_NEGATIVE`] floor (「清除」 never goes negative). The
/// advertised `min`/`max` are **for display** -- a ruleset skill that prints
/// 「（初始N，上限M）」 writes `max` so the UI can show 「3/5」, and whoever
/// moves `value` polices that cap itself (enforcement belongs to the card
/// implementation, not to the holder). `min`/`max` default to 0: a declared
/// cap of 0 means "none yet" / uncapped. [`Self::flags`] carries the generic
/// declarations ([`state_flag`]) so special-casing never needs the key name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StateVar {
    pub value: i32,
    /// Advertised lower bound (informational). The engine floors
    /// [`state_flag::NEVER_NEGATIVE`] counters at 0 regardless.
    pub min: i32,
    /// Advertised upper bound -- the mandated cap, for display. `max > 0` is
    /// a real ceiling the card implementation polices; `max == 0` is uncapped.
    /// The engine stores it and does **not** clamp to it.
    pub max: i32,
    /// When this wears off, if it is a timed counter. See [`Tick`].
    pub expires: Option<Tick>,
    /// Generic [`state_flag`] bits (serde-defaulted).
    pub flags: u8,
}

/// The keys the engine's own game flow reads back (its turn rules). Content is
/// free to invent more; these are only the ones the engine names. Status-layer
/// names live in the ruleset (`rules/bandori-std`) and reach the engine only as
/// opaque strings.
pub mod key {
    /// Latch set when an abnormal move lands on the player and cleared when
    /// their next main move completes. Longer-lived than `TurnCtx::abnormal`
    /// (which is per calendar turn): 「若受到abnormal move影响，此卡不生效」
    /// (是我自己的问题 (2)) reads this so a forced move on another player's
    /// turn still suppresses the card at the owner's next main-move end.
    pub const ABNORMAL_HIT: &str = "abnormalHit";
    /// Turn key of a live 「无法从手牌中使用counteraction」 lock (骰子已经掷下). Mirrors
    /// `card_sdk::abi::state_key::NO_COUNTERACT_TURN`.
    pub const NO_COUNTERACT_TURN: &str = "noCounteractTurn";
    /// Hand size limit (base 5; effects may cut or lift it).
    pub const HAND_LIMIT: &str = "handLimit";
    /// The [`HAND_LIMIT`] value meaning 「手牌数没有上限」. Mirrors
    /// `card_sdk::abi::state_key::NO_HAND_LIMIT`; [`MatchPlayer::hand_limit`]
    /// turns it into `None`.
    pub const NO_HAND_LIMIT: i32 = i32::MAX;
    /// The authoritative opening hand size (default 2; effects may lower it,
    /// minimum 0). Written at the before-match-start point and read by the
    /// opening draw. Mirrors `card_sdk::abi::state_key::START_HAND`.
    pub const START_HAND: &str = "startHand";
    /// Skill-system scratch.
    pub const SKILL_STATE: &str = "skillState";
    /// This player has built this turn (set by the build step).
    pub const BUILT: &str = "built";
    /// This player has bought this turn (set by the buy step).
    pub const BOUGHT: &str = "bought";
    /// This player has redeemed a deed this turn (「本回合进行过赎回操作」).
    pub const REDEEMED: &str = "redeemed";
    /// This player has mortgaged a deed this turn (「本回合进行过抵押操作」).
    pub const MORTGAGED: &str = "mortgaged";
    /// This turn's walk is armed against abnormal movement (「本次移动不受异常
    /// 移动效果影响」). The walk clamp skips while this is > 0; the turn-start
    /// move-gate skip is a different question and is not cancelled.
    pub const MOVE_EXEMPT: &str = "moveExempt";
}

/// Named **card properties** -- the keys of a card rule's declared property
/// map (`CardDef::props`, carried through the ruleset manifest). A property is
/// a static fact about the card rule, not an effect: the engine reads it back
/// by key and never by matching rulebook prose. Mirrors
/// `card_sdk::abi::prop` (the two crates cannot share a definition; keep them
/// in step). Every key has a defined default of `0` when a card does not
/// declare it.
pub mod prop {
    /// Continuous 「手卡上限数量减1」 while the card sits on the field. Stamped
    /// onto the field instance at placement and gone with the card. `-1` cuts
    /// the owner's hand limit.
    pub const HAND_LIMIT_DELTA: &str = "handLimitDelta";
    /// Bot-only **estimated execution cost**. Mirrors
    /// `card_sdk::abi::prop::EST_COST`. Read only by bots / autopilot as a
    /// reserve check -- never by legality. `0` = unknown / assume free.
    pub const EST_COST: &str = "estCost";
    /// Opt-in field counteraction / shared-choice metadata (ABI v47).
    pub const COUNTERACT_FROM_FIELD: &str = "counteractFromField";
    pub const COUNTERACT_GROUP: &str = "counteractGroup";
    pub const COUNTERACT_CHARGE_COST: &str = "counteractChargeCost";
    /// 「有designate目标」: `1` = this play names recipients (the play's other
    /// living players). Mirrors `card_sdk::abi::prop::DESIGNATES`.
    pub const DESIGNATES: &str = "designates";
    /// 「此卡不受任何其他效果影响」: `1` = the placed instance is immune
    /// (`FieldCard.immune`). Stamped at placement from the card's static props
    /// so a `place_raw` / engine placement sees it without running the play
    /// body. Mirrors `card_sdk::abi::prop::IMMUNE`.
    pub const IMMUNE: &str = "immune";
    /// Virtual **rent** house count (「房屋数视为…」). Presence is the override
    /// (a count of `0` is legitimate); real `st.houses` is untouched. On a
    /// placed card of the tile's owner (`ctx::set_prop`, gone with the card) or
    /// on the tile's rule instance (`ctx::set_tile_prop`). Read by
    /// [`crate::engine::ops::World::rent_houses`]; `houses_of` remains real.
    pub const RENT_HOUSES: &str = "rentHouses";
    /// Display-only **remaining-turn countdown** marker (「…回合后」 / 「…结束时」
    /// removal). Set by a rule at placement and decremented at the card's own
    /// turn boundary; the card still owns the expiry (this counter never drives
    /// it). The client renders it as `⌛N` -- a field card's prop, or
    /// [`ActiveEvent::turns_left`] for an event instance. `0` / absent = no
    /// marker. Mirrors `bandori_std::marker::TURNS_LEFT`.
    pub const TURNS_LEFT: &str = "turnsLeft";

    // ---------------------------------------------------------- tile data
    // Stamped onto a board-owned tile rule instance at `bind_tiles` (from
    // `TileData`), and read back by the settle bodies and by the end-step
    // buy/build gates. Mirrors `card_sdk::abi::prop`; see `docs/TILES.md`.

    /// Land price (houses are extra). `TileData::price`.
    pub const PRICE: &str = "price";
    /// Build cost per level. `TileData::house`.
    pub const HOUSE: &str = "house";
    /// Colour group (`TileData::group`). [`ALL_COLORS`] is 「该格获得所有颜色」.
    pub const GROUP: &str = "group";
    /// The tile's value that means 「该格获得所有颜色」.
    pub const ALL_COLORS: i32 = -2;
    /// 「每块地有标注的等级上限」 -- max houses, `rent.len() - 1`.
    pub const BUILD_MAX: &str = "buildMax";
    /// Length of the rent table (levels = houses + 1).
    pub const RENT_LEN: &str = "rentLen";
    /// Rent at level N: `rent[0]` … `rent[len - 1]`.
    pub const RENT_PREFIX: &str = "rent:";
    /// Generic dice-rent multiplier (ABI v66). When set (`> 0`) on a tile rule
    /// instance, rent is `count_owned_kind(owner, kind) × mult × 1d20` instead
    /// of the rent table. Stamped by the tile rule's static props (or at bind
    /// from the match-rules parameter the rule declares).
    pub const RENT_DICE_MULT: &str = "rentDiceMult";
    /// This tile is the start square (from the tile rule's static props).
    pub const START: &str = "start";
    /// `1` = this deed cannot be mortgaged (tile-rule static prop).
    pub const NO_MORTGAGE: &str = "noMortgage";

    // ------------------------------------------------- tile rule modifiers
    // What a card that bends a tile writes onto its rule instance, replacing
    // the engine flags these used to be. See `docs/TILES.md`.

    /// 「无法获取[the pass reward]」 on this tile's the start-square tile rule instance.
    pub const NO_REWARD: &str = "noReward";
    /// Rent scale in milli-units (500 = x0.5). Replaces `rent_factor`.
    pub const RENT_FACTOR: &str = "rentFactor";
    /// Payment scale in milli-units (500 = x0.5). Replaces `pay_factor`.
    pub const PAY_FACTOR: &str = "payFactor";
    /// 「owner不可盖房」. Replaces the `noBuild` state key.
    pub const NO_BUILD: &str = "noBuild";
    /// 「不可在造价N及以上的格子上加盖房屋」 (卡池BUG) -- `why_not_build_on`
    /// refuses a build whose house cost is `>=` this. On a board-owned instance;
    /// `0` disables. Mirrors `card_sdk::abi::prop::NO_BUILD_ABOVE`.
    pub const NO_BUILD_ABOVE: &str = "noBuildAbove";
    /// Turn-clock override: `base_time` (恢复时间) = value - 1 seconds, `0` =
    /// none. Mirrors `card_sdk::abi::prop::BASE_TIME_SET`.
    pub const BASE_TIME_SET: &str = "baseTimeSet";
    /// Turn-clock override: the `base_time` refill = value - 1 seconds, `0` =
    /// none. Mirrors `card_sdk::abi::prop::BASE_TIME_REFILL_SET`.
    pub const BASE_TIME_REFILL_SET: &str = "baseTimeRefillSet";
    /// As [`BASE_TIME_SET`] for `bonus_time` (免费时间). Mirrors
    /// `card_sdk::abi::prop::BONUS_TIME_SET`.
    pub const BONUS_TIME_SET: &str = "bonusTimeSet";

    // ------------------------------------------------------ purchase surface
    // v40 (`docs/PURCHASE.md`). Mirrors `card_sdk::abi::prop`; keep in step.

    /// 「可购买格子」 (`data/rules.txt` line 19) -- `1` = this tile can be bought.
    pub const BUYABLE: &str = "buyable";
    /// Houses counted into the buy price (「建造已有房子的资金总价」).
    pub const BUY_HOUSES: &str = "buyHouses";
    /// Force-buy price scale in milli-units (2000 = ×2, 「两倍」).
    pub const FORCE_MULT: &str = "forceMult";
    /// 「此次购买的价格不受任何资金变动效果影响」 -- `1` = direct transfer.
    pub const FORCE_FIXED: &str = "forceFixed";
    /// 「获得的地契仍为抵押状态」 -- `1` = a force-buy keeps the mortgage.
    pub const FORCE_STAYS_MORTGAGED: &str = "forceStaysMortgaged";
    /// 「该格获得所有颜色」 / soyo 「所有颜色」.
    pub const ANY_COLOR: &str = "anyColor";
    /// Per-player colour override prefix: `colorFor:<p>` = the group tile `p`
    /// treats this tile as.
    pub const COLOR_FOR_PREFIX: &str = "colorFor:";
}

/// The **neutral board owner** of tile rule instances ([`MatchState::board_field`]).
/// Not a seat: `-1` is already "no player" everywhere in the engine, and the
/// alive/out/turn loops index `players` so they never reach this list. A card
/// that attaches a rule to a tile places it here (`place_card_on(BOARD_OWNER, tile, …)`).
pub const BOARD_OWNER: i32 = -1;

/// Default [`state_flag`] bits for a new key. Only generic engine-mechanism
/// names appear here (turn latches, the hand-size knobs) -- game vocabulary
/// never does. Ruleset-invented keys start unflagged; the writer declares
/// flooring and ownership via [`MatchPlayer::state_set_flags`].
fn default_flags(key: &str) -> u8 {
    use state_flag::ENGINE_OWNED;
    match key {
        key::HAND_LIMIT
        | key::START_HAND
        | key::SKILL_STATE
        | key::BUILT
        | key::BOUGHT
        | key::REDEEMED
        | key::MORTGAGED
        | key::ABNORMAL_HIT
        | key::MOVE_EXEMPT
        | key::NO_COUNTERACT_TURN => ENGINE_OWNED,
        _ => 0,
    }
}

/// Does this state item floor at 0 (「清除」)? Reads the item's
/// [`state_flag::NEVER_NEGATIVE`], which the writer declares.
fn floors_at_zero(v: &StateVar) -> bool {
    v.flags & state_flag::NEVER_NEGATIVE != 0
}

/// How a bot player decides. Serde-defaults to [`Self::Standard`] so older
/// saves and room records load unchanged.
///
/// Only `bot` seats take a mentality: a human who times out or disconnects is
/// still answered with the standard policy (`ai` flips on, `bot` remains off).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotMentality {
    /// The built-in heuristic bot (`ai.rs`) -- reserved money, card-play odds,
    /// cap per turn.
    #[default]
    Standard,
    /// Legal but maximally disruptive: play everything, take every offer, spend
    /// down to [`crate::engine::CHAOS_RESERVE`].
    Chaos,
    /// Search bot driven by the server's `bot-service` (`docs/BOT.md` B5).
    /// Inside the engine this is [`Self::Standard`]: the heuristic plays the
    /// seat exactly like a standard bot whenever the engine is the one driving
    /// it (Solo, or an online room with no service attached -- the server
    /// rewrites the mentality to Standard before `Match::new` in that case).
    /// With a service attached the server holds the seat (`ai` starts off) and
    /// answers every play-phase decision itself; setup (ban / pick / deck)
    /// remains engine-side.
    Advanced,
}

impl BotMentality {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotMentality::Standard => "standard",
            BotMentality::Chaos => "chaos",
            BotMentality::Advanced => "advanced",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "standard" | "" => Some(BotMentality::Standard),
            "chaos" => Some(BotMentality::Chaos),
            "advanced" => Some(BotMentality::Advanced),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchPlayer {
    pub member: i32,
    pub player: String,
    pub bot: bool,
    pub ai: bool,
    /// Bot decision policy. Meaningless on a human seat; see [`BotMentality`].
    pub mentality: BotMentality,
    pub roll: i32,
    pub ban_done: bool,
    pub ban: String,
    pub persona: String,
    pub deck_ready: bool,
    pub money: i32,
    pub pos: i32,
    /// Hand size only; the cards themselves are private (`HandOf`).
    pub hand: i32,
    pub draw: i32,
    pub discard: Vec<String>,
    pub mulligan: bool,
    pub bankrupt: bool,
    pub left: bool,
    pub out_order: i32,
    pub skill_note: Msg,
    pub skill_owner: String,
    pub affiliations: String,
    pub tokens: Vec<Counter>,
    /// Keyed state: the counters this player carries -- status-layer pots plus
    /// the arbitrary named slots content invents -- as `{value, min, max}` items.
    /// See [`StateVar`] -- the engine holds these and enforces nothing. Keys
    /// are content vocabulary; [`key`] names the few the engine itself reads
    /// back.
    ///
    /// The key *names* are a wire contract, not just internal labels: the
    /// client reads this map by name and TS types are structural, so renaming
    /// a key breaks the board render silently rather than at compile time.
    /// Status-layer names live in the ruleset and are frozen webui wire.
    pub state: BTreeMap<String, StateVar>,
    pub field: Vec<FieldCard>,
    pub assets: i32,
    pub score: i32,
    pub rank: i32,
}

impl Default for MatchPlayer {
    fn default() -> Self {
        Self {
            member: 0,
            player: String::new(),
            bot: false,
            ai: false,
            mentality: BotMentality::Standard,
            roll: 0,
            ban_done: false,
            ban: String::new(),
            persona: String::new(),
            deck_ready: false,
            money: 0,
            pos: 0,
            hand: 0,
            draw: 0,
            discard: vec![],
            mulligan: false,
            bankrupt: false,
            left: false,
            out_order: 0,
            skill_note: Msg::default(),
            skill_owner: String::new(),
            affiliations: String::new(),
            tokens: vec![],
            state: {
                // The one engine knob that starts non-zero (hand limit 5).
                // Every other key is content vocabulary and starts absent.
                let mut m = BTreeMap::new();
                m.insert(
                    key::HAND_LIMIT.to_string(),
                    StateVar {
                        value: 5,
                        ..StateVar::default()
                    },
                );
                m
            },
            field: vec![],
            assets: 0,
            score: 0,
            rank: 0,
        }
    }
}

impl MatchPlayer {
    /// Does the machine handle this seat's **setup** (ban / pick / deck) without
    /// waiting on a human or the server? True for every engine-driven seat
    /// (`ai`) and every bot: play-phase decisions reach a bot over the human
    /// interface (`docs/BOT-DRIVE.md`), but setup is engine-side and runs the
    /// standard policy for all of them.
    pub fn auto_setup(&self) -> bool {
        self.ai || self.bot
    }

    // ---------------------------------------------------------- keyed state
    // Dumb storage plus the writer-declared floor. The engine holds
    // `{value, min, max}` items and honours only `NEVER_NEGATIVE`; the
    // advertised `max` is display-only (see [`StateVar`]). Everything below
    // that looks like a rule is a *reader*: the engine's own game flow asking
    // for a value it named.

    /// The whole item, or `StateVar::default()` when the key is absent.
    pub fn state_var(&self, key: &str) -> StateVar {
        self.state.get(key).copied().unwrap_or_default()
    }

    pub fn state_get(&self, key: &str) -> i32 {
        self.state_var(key).value
    }

    pub fn state_min(&self, key: &str) -> i32 {
        self.state_var(key).min
    }

    pub fn state_max(&self, key: &str) -> i32 {
        self.state_var(key).max
    }

    /// Write `value`, honouring the writer-declared floor.
    ///
    /// The engine honours [`state_flag::NEVER_NEGATIVE`] (「清除」 floors at 0)
    /// and nothing else. The advertised `max` is stored for display -- the
    /// engine does **not** police it; the card implementation (e.g.
    /// `bandori_std::fire`'s 「在任意时刻，您持有的火罐数量不能超过持有上限」)
    /// does its own arithmetic and writes only what should land.
    ///
    /// Wear-off is **not** guessed from the key. The writer declares it via
    /// [`Self::state_set_expires`] (规则书 「每回合结束时移除一层」 lives in the
    /// ruleset's grant verb). A raw write with no expiry means "no wear-off".
    pub fn state_set(&mut self, key: &str, value: i32) -> i32 {
        let e = self.state.entry(key.to_string()).or_default();
        if e.flags == 0 {
            e.flags = default_flags(key);
        }
        let mut v = value;
        // Counters that declared NEVER_NEGATIVE cannot go negative.
        if floors_at_zero(e) {
            v = v.max(0);
        }
        e.value = v;
        v
    }

    /// Add `delta`, floored like [`Self::state_set`].
    pub fn state_add(&mut self, key: &str, delta: i32) -> i32 {
        let v = self.state_get(key).saturating_add(delta);
        self.state_set(key, v)
    }

    /// Declare the advertised bounds (for display). `max` is stored and
    /// exposed; the engine does not clamp to it. `max == 0` means uncapped.
    pub fn state_set_bounds(&mut self, key: &str, min: i32, max: i32) {
        let e = self.state.entry(key.to_string()).or_default();
        e.min = min;
        e.max = max;
    }

    /// Declare the generic [`state_flag`] bits for this key (floors at 0,
    /// engine-owned, …). The writer owns the declaration; the engine never
    /// infers it from the key name.
    pub fn state_set_flags(&mut self, key: &str, flags: u8) {
        let e = self.state.entry(key.to_string()).or_default();
        e.flags = flags;
    }

    /// Declare when this counter wears off (see [`Tick`]).
    pub fn state_set_expires(&mut self, key: &str, expires: Option<Tick>) {
        self.state.entry(key.to_string()).or_default().expires = expires;
    }

    /// Tick every timed counter whose expiry is due, returning `(key, left)`.
    /// No key names: the item says when it wears off.
    pub fn tick_state(&mut self, when: Tick) -> Vec<(String, i32)> {
        let mut moved = Vec::new();
        for (k, v) in self.state.iter_mut() {
            if v.expires == Some(when) && v.value > 0 {
                v.value -= 1;
                moved.push((k.clone(), v.value));
            }
        }
        moved
    }

    // ------------------------------------------- generic readers only
    // The engine never names a status-layer key. Callers use `state_get(name)`
    // with a name from the ruleset, or ask the ruleset's `gate_open`.

    /// Hand size limit: the `handLimit` state (base 5, cards may cut or lift
    /// it) plus every placed card's continuous `prop::HAND_LIMIT_DELTA`
    /// (「手卡上限数量减1」). `None` is 「手牌数没有上限」
    /// ([`key::NO_HAND_LIMIT`]); no delta brings a lifted limit back.
    pub fn hand_limit(&self) -> Option<i32> {
        let base = self.state_get(key::HAND_LIMIT);
        if base == key::NO_HAND_LIMIT {
            return None;
        }
        let field: i32 = self
            .field
            .iter()
            .map(|f| f.props.get(prop::HAND_LIMIT_DELTA).copied().unwrap_or(0))
            .sum();
        Some(base.saturating_add(field))
    }

    /// Holding `held` cards puts this player over the hand limit.
    pub fn over_hand(&self, held: usize) -> bool {
        self.hand_limit()
            .is_some_and(|limit| held > usize::try_from(limit).unwrap_or(0))
    }

    pub fn skill_state(&self) -> i32 {
        self.state_get(key::SKILL_STATE)
    }

    /// Bankrupt or left the match.
    pub fn out(&self) -> bool {
        self.bankrupt || self.left
    }

    /// `MatchPlayer::token(name)` -- 0 if absent.
    pub fn token(&self, name: &str) -> i32 {
        self.tokens
            .iter()
            .find(|c| c.name == name)
            .map_or(0, |c| c.value)
    }

    /// Live ceiling on the held row (`Counter::max`; 0 = uncapped / undeclared).
    pub fn token_max(&self, name: &str) -> i32 {
        self.tokens
            .iter()
            .find(|c| c.name == name)
            .map_or(0, |c| c.max)
    }
}

/// A prompt asking players to choose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchPrompt {
    /// 0 = no prompt.
    pub id: i32,
    pub kind: String,
    pub title: Msg,
    pub text: Msg,
    pub card: String,
    pub options: Vec<Msg>,
    pub fallback: i32,
    pub players: Vec<i32>,
    /// Parallel to `players`; -1 = not answered yet.
    pub answers: Vec<i32>,
    pub time_left: f32,
    pub tile: i32,
    pub bid: i32,
    pub bidder: i32,
    pub items: Vec<String>,
    pub count: i32,
    /// The quoted price on a buy / force_buy prompt (`docs/PURCHASE.md`).
    /// `-1` when the prompt is not a purchase.
    #[serde(default = "neg_one")]
    pub price: i32,
    /// Per-option prices on an agent prompt, parallel to `options`.
    #[serde(default)]
    pub prices: Vec<i32>,
    /// A tile prompt whose pick is walked to (not teleported): the board
    /// lights the path to the tile as well.
    #[serde(default)]
    pub walk: bool,
}

fn neg_one() -> i32 {
    -1
}

impl Default for MatchPrompt {
    fn default() -> Self {
        Self {
            id: 0,
            kind: String::new(),
            title: Msg::default(),
            text: Msg::default(),
            card: String::new(),
            options: vec![],
            fallback: 0,
            players: vec![],
            answers: vec![],
            time_left: 0.0,
            tile: -1,
            bid: 0,
            bidder: -1,
            items: vec![],
            count: 0,
            price: -1,
            prices: vec![],
            walk: false,
        }
    }
}

impl MatchPrompt {
    pub fn player_index(&self, player_id: i32) -> Option<usize> {
        self.players.iter().position(|&s| s == player_id)
    }

    /// Is `player_id` asked and has not answered yet?
    pub fn waiting(&self, player_id: i32) -> bool {
        self.player_index(player_id)
            .and_then(|i| self.answers.get(i))
            .is_some_and(|&a| a < 0)
    }
}

/// The vote to end the match early.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchVote {
    pub id: i32,
    pub by: i32,
    pub players: Vec<i32>,
    pub answers: Vec<i32>,
    pub time_left: f32,
}

impl Default for MatchVote {
    fn default() -> Self {
        Self {
            id: 0,
            by: -1,
            players: vec![],
            answers: vec![],
            time_left: 0.0,
        }
    }
}

impl MatchVote {
    pub fn yes(&self) -> usize {
        self.answers.iter().filter(|&&a| a == 1).count()
    }

    pub fn waiting(&self, player_id: i32) -> bool {
        self.players
            .iter()
            .position(|&s| s == player_id)
            .and_then(|i| self.answers.get(i))
            .is_some_and(|&a| a < 0)
    }
}

/// One log/animation event. `id` increases monotonically per match,
/// which is what makes SSE `Last-Event-ID` resume work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchEvent {
    pub id: i32,
    pub r#type: String,
    pub player_id: i32,
    pub other: i32,
    pub value: i32,
    pub from: i32,
    pub to: i32,
    pub dice: i32,
    pub card: String,
    pub msg: Msg,
    /// Trigger kind of a `"card"` activation -- `card_trigger::PLAY` / `SKILL` /
    /// `EVENT` / `COUNTER` / `HOOK`. Empty on every other event type.
    /// `#[serde(default)]` so an event tail written before this field loads.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// A `"card"` activation a counteraction negated: the card still flashes
    /// (marked 无效) and its log line says so, but its body did not run.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub negated: bool,
    /// Parent `"card"` activation event id this event groups under (the log
    /// nests it beneath that header; the flash rides the parent's face).
    /// `-1` = top-level. Child events remain in the stream in code order so
    /// walks, coin SFX and the like still animate -- the grouping is
    /// presentational.
    #[serde(default = "no_parent", skip_serializing_if = "is_no_parent")]
    pub parent: i32,
    /// On a walk's `roll` / `move` event: how many more steps the same walk
    /// takes after this segment's `to` (a walk publishes one segment per
    /// pass-reward tile it passes, so the client cannot see the whole route otherwise).
    /// `0` = the segment ends the walk.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ahead: i32,
    /// Presentational outcome lines on a `"card"` activation: the explicit
    /// 「无事发生」 / 「被…无效」 notes and the flash's one-line summary. The
    /// body's own `ctx::log` lines and the host effects it caused are **child
    /// events** ([`Self::parent`]), not copies here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<Msg>,
    /// The field instance (`FieldCard.uid`) a `"card"` activation belongs to
    /// -- set when the card was in play at the time, `-1` for a hand play, a
    /// skill, an event or any card not on a field. Lets the client light the
    /// card's field slot instead of flashing its face. Absent on old records.
    #[serde(default = "no_parent", skip_serializing_if = "is_no_parent")]
    pub uid: i32,
    /// Should the client show the card **flash** (and the field-slot pulse)
    /// for this activation? `false` when every produced effect is
    /// owner-self-only (own charge, counters, latches, hand-limit, bank money
    /// on self) -- the log entry (header + outcome lines) still appears; only
    /// the animation goes. `true` when anything reached another player, the
    /// board (cards, marks, houses, deeds, tile props), or movement. v54.
    #[serde(default = "default_flash", skip_serializing_if = "is_default_flash")]
    pub flash: bool,
}

fn default_flash() -> bool {
    true
}

fn is_default_flash(f: &bool) -> bool {
    *f
}

fn is_no_parent(p: &i32) -> bool {
    *p < 0
}

fn is_zero(v: &i32) -> bool {
    *v == 0
}

fn no_parent() -> i32 {
    -1
}

/// The [`MatchEvent::kind`] values of a `"card"` activation event.
pub mod card_trigger {
    /// A card played from hand (`PlayFromHand`).
    pub const PLAY: &str = "play";
    /// A skill press (`use_skill`).
    pub const SKILL: &str = "skill";
    /// A drawn event card's effect.
    pub const EVENT: &str = "event";
    /// A counteraction card's body.
    pub const COUNTER: &str = "counter";
    /// A placed / field card's hook firing (tile, rent, pass, settle, ...).
    pub const HOOK: &str = "hook";
}

impl Default for MatchEvent {
    fn default() -> Self {
        Self {
            id: 0,
            r#type: String::new(),
            player_id: -1,
            other: -1,
            value: 0,
            from: 0,
            to: 0,
            dice: 0,
            card: String::new(),
            msg: Msg::default(),
            kind: String::new(),
            negated: false,
            parent: -1,
            ahead: 0,
            results: Vec::new(),
            uid: -1,
            flash: true,
        }
    }
}

/// An event card (事件卡) currently in play: drawn, public, effect running
/// (`docs/EVENTS.md`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ActiveEvent {
    pub id: String,
    pub player_id: i32,
    pub counter: i32,
    pub counter2: i32,
    /// Display-only remaining-turn countdown (see [`prop::TURNS_LEFT`]); `0` =
    /// no marker.
    pub turns_left: i32,
    pub note: Msg,
    pub face_down: bool,
}

impl Default for ActiveEvent {
    fn default() -> Self {
        Self {
            id: String::new(),
            player_id: -1,
            counter: 0,
            counter2: 0,
            turns_left: 0,
            note: Msg::default(),
            face_down: false,
        }
    }
}

/// A card on a player's field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FieldCard {
    pub uid: i32,
    pub card: String,
    pub owner: i32,
    pub user: i32,
    pub tile: i32,
    /// Named on-card counters of this instance. The generic counter API
    /// (`ctx::counter` / `add_counter`) is the only writer. Names are
    /// ruleset-defined strings; the engine treats them as opaque keys.
    #[serde(default)]
    pub counters: BTreeMap<String, i32>,
    pub face_down: bool,
    /// 「此卡不受…效果影响」. A value on the card, not a subclass override:
    /// effects that would touch it read this and skip.
    pub immune: bool,
    /// The card rule's declared static **properties** (see [`prop`]), stamped
    /// onto the instance at placement and gone with it. A continuous effect
    /// like 「手卡上限数量减1」 (`prop::HAND_LIMIT_DELTA`) rides here, so a
    /// `place_raw` test arrangement sees it too.
    #[serde(default)]
    pub props: BTreeMap<String, i32>,
    /// This instance is a **group skill** (`skill:<group>:<skill>`, stamped at
    /// placement from `GameData::is_group_skill`). the group-skill named counter
    /// reads and writes land on this instance's [`Self::counters`] -- see
    /// `World::group_pool`. A persona skill is `skill:<persona>:<skill>`
    /// and is *not* flagged, even when it sits on someone else's field.
    #[serde(default)]
    pub group_skill: bool,
    /// A taken group-skill copy. 「group-skill card effects do not stack」 -- the
    /// hook dispatch skips an extra when a non-extra attachment of the same id
    /// is already on the field -- and 「not treated as that group's persona」 (`in_group` reads
    /// only the persona).
    #[serde(default)]
    pub extra: bool,
    pub note: Msg,
}

impl Default for FieldCard {
    fn default() -> Self {
        Self {
            uid: 0,
            card: String::new(),
            owner: -1,
            user: -1,
            tile: -1,
            counters: BTreeMap::new(),
            face_down: false,
            immune: false,
            props: BTreeMap::new(),
            group_skill: false,
            extra: false,
            note: Msg::default(),
        }
    }
}

/// Named counters on a field-card instance live in [`FieldCard::counters`]
/// under ruleset-defined name strings. The engine holds no well-known names.

/// Tile-mark **categories** -- what sort of thing a [`TileMark`] is. A category
/// is the mark's identity for the board, as against `TileMark::kind` (the i18n
/// key that names a particular mark) and `TileMark::owner` (which seat placed a
/// player mark). Mirrors `card_sdk::abi::mark` (the two crates cannot share a
/// definition; keep them in step).
///
/// [tile marks] is its own category, not a `kind` string mixed in with the player
/// marks (`data/rules.txt` 125: 「tile marks：放置于路面上的指示物」), and it carries
/// **no player owner** -- see [`TileMark::owner`].
pub mod mark_category {
    /// A player/generic mark (the default): a player token / named counter / cosmetic mark / 抹茶巴菲 …
    /// Placed by a seat, coloured by that seat in the view.
    pub const PLAYER: &str = "";
    /// A designate of tiles by a field card / skill while it is in play. Owned by
    /// the designating instance; dies with it. `owner` is the designating
    /// seat (label colour). Mirrors `card_sdk::abi::mark::DESIGNATE_CATEGORY`.
    pub const DESIGNATE: &str = "designate";
}

/// Well-known tile-mark kinds (the i18n key / stable id on `TileMark::kind`).
/// Mirrors `card_sdk::abi::mark`.
pub mod mark_kind {
    /// The designation mark's stable kind. Display comes from
    /// [`TileMark::card`] (the designating card id). Mirrors
    /// `card_sdk::abi::mark::DESIGNATE_KIND`.
    pub const DESIGNATE: &str = "mark:designate";
}

/// Wire shape of an optional card-instance id: the old field was an `i32`
/// with `-1` = none. Serde keeps that spelling so `SAVE_VERSION` need not move.
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

/// A marker placed on a tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TileMark {
    pub uid: i32,
    pub tile: i32,
    /// `card` for a card placed on the tile (see `card`); otherwise the i18n key
    /// naming the mark (e.g. a named-counter mark). The label comes from
    /// [`Self::category`] when the mark is a named counter of a field card.
    pub kind: String,
    /// Which category of mark this is: [`mark_category::PLAYER`] (default) or
    /// [`mark_category::DESIGNATE`]. Serde-defaulted so pre-category saves still load.
    pub category: String,
    /// The seat that placed a **player** mark (colours it in the view), or
    /// [`BOARD_OWNER`] (`-1`) for a neutral mark. Neutral marks (tile marks /
    /// named-counter marks) remain unowned -- their provenance lives in
    /// [`Self::src`] / [`Self::card`] instead.
    pub owner: i32,
    pub count: i32,
    /// Provenance for the log / the view's 「来自」: the card id that placed the
    /// mark (empty when none). Not an owner.
    pub card: String,
    /// Provenance for the rules: the **card instance** (`FieldCard::uid`) that
    /// placed the mark, or `None`. 「此卡在格子上添加的[tile marks]及其产物」
    /// (通用:the mark-emptying card (1)) keys on this, not on [`Self::owner`]. Per-unit
    /// attribute -- not ownership. Serde-defaulted so pre-`src` saves still load
    /// (the wire spelling is still `-1` for none).
    #[serde(default, with = "opt_i32_neg1")]
    pub src: Option<i32>,
    /// The **owning card instance** (`FieldCard::uid`) whose named counter
    /// these units are bound to (user ruling 2026-10-10: every tile mark is a
    /// counter of some card or pseudo card). Destroying that instance destroys
    /// these units. `None` = legacy row whose creator could not be determined
    /// at save-migration time (kept, never auto-purged). Serde-defaulted (the
    /// wire spelling is still `-1` for none).
    #[serde(default, with = "opt_i32_neg1")]
    pub instance: Option<i32>,
    /// The other tile of a **paired** mark ([`Self::partner`] is the cross-end
    /// of one unit bound to two tiles, placed by `place_mark_pair`). `None` =
    /// an unpaired row (the default placement). The row on `a` names `b` and
    /// the row on `b` names `a`; both die with the creating instance like any
    /// mark. Serde-defaulted so pre-`partner` saves still load (the wire
    /// spelling is still `-1` for none).
    #[serde(default, with = "opt_i32_neg1")]
    pub partner: Option<i32>,
    pub note: Msg,
}

impl Default for TileMark {
    fn default() -> Self {
        Self {
            uid: 0,
            tile: 0,
            kind: String::new(),
            category: String::new(),
            owner: -1,
            count: 1,
            card: String::new(),
            src: None,
            instance: None,
            partner: None,
            note: Msg::default(),
        }
    }
}

impl TileMark {}

/// Named units held by a player. Under the bound-counter model
/// these are units of some card instance's named counter, bound to a holder
/// (user ruling 2026-10-10). [`Self::instance`] is that owner; `-1` = legacy
/// row whose creator could not be determined at save-migration time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Counter {
    pub name: String,
    pub value: i32,
    /// Owning card instance (`FieldCard::uid`), or `None` when unknown (the
    /// wire spelling is still `-1`).
    #[serde(default, with = "opt_i32_neg1")]
    pub instance: Option<i32>,
    /// Live ceiling the rules-layer kernel polices (「上限M」). `0` = uncapped
    /// (the default for ephemeral tokens and for rows saved before this
    /// field existed). Stored for display and for the pull-under; the engine
    /// does not clamp writes to it.
    #[serde(default)]
    pub max: i32,
}

impl Default for Counter {
    fn default() -> Self {
        Self {
            name: String::new(),
            value: 0,
            instance: None,
            max: 0,
        }
    }
}

/// A filter over [`TileMark`] rows (the generic tile-mark API). Empty
/// `kind` / `category` and `None` `owner` / `src` / `instance` mean "any".
/// `instance` is the owning counter's card instance; `src` is provenance.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkFilter<'a> {
    pub kind: &'a str,
    pub category: &'a str,
    pub owner: Option<i32>,
    pub src: Option<i32>,
    pub instance: Option<i32>,
}

impl MarkFilter<'_> {
    /// Match every mark on a tile.
    pub const ANY: MarkFilter<'static> = MarkFilter {
        kind: "",
        category: "",
        owner: None,
        src: None,
        instance: None,
    };

    pub fn matches(&self, m: &TileMark) -> bool {
        (self.kind.is_empty() || m.kind == self.kind)
            && (self.category.is_empty() || m.category == self.category)
            && (self.owner.is_none() || self.owner == Some(m.owner))
            && (self.src.is_none() || self.src == m.src)
            && (self.instance.is_none() || self.instance == m.instance)
    }
}

/// One pressable skill in a viewer's list (`Match::view_extra`'s `skills`).
/// View-only -- not part of saved state. `id` is the rule id
/// (`skill:<owner>:<name>`); the client resolves name / body from it
/// (`cardTitle` / `cardText`). `enabled` / `reason` come from the same gate
/// `act: "skill"` is validated with, so "shown enabled" ⇔ "accepted".
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillAction {
    /// Rule id (`skill:<owner>:<name>`).
    pub id: String,
    /// The field instance that holds it, as `owner:skill` (display source).
    pub source: String,
    /// The skill's name, as a card reference (`{{card}}`).
    pub title: Msg,
    /// The skill body, as a card reference (the client may prefer `cardText`).
    pub text: Msg,
    pub enabled: bool,
    /// Why the gate refuses, when `enabled` is false.
    pub reason: Msg,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wear-off is declared by the writer (`state_set_expires`), not guessed
    /// from the key. A timed counter's tick is carried on the item.
    #[test]
    fn timed_counters_wear_off_on_their_tick() {
        let mut p = MatchPlayer::default();
        p.state_set_flags("k1", state_flag::NEVER_NEGATIVE);
        p.state_set_flags("k2", state_flag::NEVER_NEGATIVE);
        p.state_set("k1", 1);
        p.state_set_expires("k1", Some(Tick::TurnEnd));
        p.state_set("k2", 1);
        p.state_set_expires("k2", Some(Tick::TurnStart));
        assert_eq!(p.state.get("k1").unwrap().expires, Some(Tick::TurnEnd));
        assert_eq!(p.state.get("k2").unwrap().expires, Some(Tick::TurnStart));
        // The tick actually moves the counter.
        p.tick_state(Tick::TurnEnd);
        assert_eq!(p.state_get("k1"), 0);
        assert_eq!(p.state_get("k2"), 1, "k2 waits for its own tick");
        p.tick_state(Tick::TurnStart);
        assert_eq!(p.state_get("k2"), 0);
    }

    /// A raw write with no expiry means "no wear-off" -- the engine does not
    /// know which keys are timed.
    #[test]
    fn raw_write_carries_no_default_tick() {
        let mut p = MatchPlayer::default();
        p.state_set("k1", 1);
        assert_eq!(p.state.get("k1").unwrap().expires, None);
        p.tick_state(Tick::TurnEnd);
        assert_eq!(p.state_get("k1"), 1);
    }

    /// An explicit non-default tick the writer asked for remains.
    #[test]
    fn explicit_expiry_is_not_overridden() {
        let mut p = MatchPlayer::default();
        p.state_set("k1", 1);
        p.state_set_expires("k1", Some(Tick::TurnStart));
        p.state_set("k1", 2);
        assert_eq!(p.state.get("k1").unwrap().expires, Some(Tick::TurnStart));
    }

    /// NEVER_NEGATIVE floors a clear at 0.
    #[test]
    fn never_negative_floors_at_zero() {
        let mut p = MatchPlayer::default();
        p.state_set_flags("k1", state_flag::NEVER_NEGATIVE);
        p.state_set("k1", 1);
        p.state_set("k1", -5);
        assert_eq!(p.state_get("k1"), 0);
    }
}
