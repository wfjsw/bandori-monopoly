//! Match state broadcast to clients (`BandoriMonopoly.Net/Match*.cs`).
//!
//! Same field names and C# initializer defaults as the original, except that all
//! display text is a localizable [`Msg`] instead of a finished string.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::msg::Msg;

/// `MovePlan.cs` -- the movement the current turn is taking: the precomputed
/// `reach[]` of tiles and how far along it the player has walked. `player_id == -1`
/// when there is no plan (teleports and stay-settle walks may skip it).
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
}

impl MovePlan {
    /// `MovePlan.Landing` -- where the walk ends at the current `steps`.
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
    /// 开始阶段 -- the turn starts: status ticks, `turnStart`, the stun/stay checks.
    pub const START: i32 = 1;
    /// 运营阶段 -- play cards, roll the dice, buy / redeem / mortgage.
    pub const OPS: i32 = 2;
    /// 移动阶段 -- the main move.
    pub const MOVE: i32 = 3;
    /// 结束阶段 -- the move has resolved; buy and build happen here.
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
/// `value` is the figure; `max` is the upper bound of a dice range (RiNG
/// rent is `rings × ring_multiplier × 1d20`, so `value` is the minimum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileQuote {
    pub kind: TileQuoteKind,
    pub flow: MoneyFlow,
    pub value: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<i32>,
}

/// `MatchState.cs`
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
    /// `ThinkTime` room setting: 0 Standard / 1 Relaxed / 2 VeryRelaxed / 3 Unlimited.
    pub think_time: i32,
    /// `State.buyPrice` -- a preview of the price to buy where the turn stands
    /// (`-1` = nothing buyable now); `BuyPriceFor` is the computation behind it.
    pub buy_price: i32,
    /// `State.buildCost` -- the same preview for building (`-1` = cannot build).
    pub build_cost: i32,
    /// `State.canBuyHere` -- would `act {act:"buy"}` be accepted at the turn's
    /// position right now? The engine's own gate (`why_not_act`'s buy branch =
    /// `buyable_here` + the quoted funds check): buyable shape, the plan's
    /// no-buy flag, the quote's `eligible` gate, and cash plus mortgageable
    /// deeds. Together with
    /// [`Self::buy_price`] this is the view's why-not information for a buy --
    /// the bot action abstraction gates Buy on it instead of re-implementing
    /// the check. `false` when nothing is buyable, and when the engine would
    /// refuse (`err.cannot_buy` / `err.buy_poor`).
    pub can_buy_here: bool,
    /// `State.canBuildHere` -- the same for `act {act:"build"}`
    /// (`why_not_build` + the funds half of `why_not_act`'s build branch).
    /// `false` when the engine would refuse (`err.build_*` / `err.poor`).
    pub can_build_here: bool,
    /// `State.canRollHere` -- would `act {act:"roll"}` be accepted
    /// (`why_not_act`'s roll branch: the live `skip_move` latch, the
    /// `main_moved` flag, and the roller).
    pub can_roll_here: bool,
    /// `State.canEndHere` -- the same for `act {act:"end"}` (the end branch:
    /// `roll_first` while a main move is still owed, `moving`, `over_hand`).
    /// The public [`Self::skip_move`] re-derives from stay / exile and can
    /// disagree with this gate (unstoppable, mid-turn [停留], [除外]) -- a bot
    /// that trusted it sent `end` into `err.roll_first`.
    pub can_end_here: bool,
    /// `State.plan` -- the movement this turn is taking.
    pub plan: MovePlan,
    pub time_left: f32,
    pub shield: f32,
    pub bank: f32,
    pub bought: bool,
    pub built: bool,
    /// Next uid to hand out for a card instance. **Global** across the match, not
    /// per player: every card in the game has one id, so an instance is addressed
    /// by `uid` alone and one player may hold several copies of the same card in
    /// play at once. `#[serde(default)]` so a state saved before this existed
    /// still loads (it re-numbers from 0 on the next placement).
    #[serde(default)]
    pub next_card_uid: i32,
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
            buy_price: -1,
            build_cost: -1,
            can_buy_here: false,
            can_build_here: false,
            can_roll_here: false,
            can_end_here: false,
            plan: MovePlan::default(),
            time_left: 0.0,
            shield: 0.0,
            bank: 0.0,
            bought: false,
            built: false,
            next_card_uid: 1,
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

    pub fn asking(&self) -> bool {
        self.prompt.id != 0
    }

    pub fn voting(&self) -> bool {
        self.vote.id != 0
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

/// `MatchPlayer.cs`
/// When a keyed state item wears off (see [`StateVar::expires`]).
///
/// The engine's turn flow ticks every item whose expiry is due, so "this
/// counter lasts a turn" is data carried by the item, not a key the engine has
/// to know by name. That is what keeps [`MatchPlayer::tick_state`] free of
/// `match key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tick {
    /// Loses one layer at its owner's start of turn (C# `stun_start`).
    TurnStart,
    /// Loses one layer at its owner's end of turn (C# `stay` / `stun`).
    TurnEnd,
}

/// One keyed state item: `value`, the bounds a consumer may enforce, and when
/// it wears off.
///
/// The engine holds these and enforces **nothing** -- it is storage, no more. A
/// character skill that mandates a fire-pot cap writes `max` (C#
/// `SkillBase.MaxFire`), and whoever moves `value` is free to honour the bounds
/// or ignore them: enforcement belongs to the consumer, not to the holder.
/// `min`/`max` default to 0, matching the C# counters, where a declared cap of
/// 0 means "none yet".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct StateVar {
    pub value: i32,
    /// Lower bound a consumer may enforce. The engine floors status counters
    /// at 0 regardless; `min` is informational.
    pub min: i32,
    /// Upper bound a consumer may enforce -- the mandated cap. `max > 0` is
    /// enforced on write; `max == 0` is uncapped.
    pub max: i32,
    /// When this wears off, if it is a timed counter. See [`Tick`].
    pub expires: Option<Tick>,
}

/// The keys the engine's own game flow reads back (its turn rules and the
/// status gates). Content is free to invent more; these are only the ones the
/// engine names. Mirrored by `card_sdk::abi::state_key`.
pub mod key {
    /// Stay layers -- wear off at end of turn (C# `MatchSeat.stay`).
    pub const STAY: &str = "stay";
    /// Stun layers -- wear off at end of turn (C# `MatchSeat.stun`).
    pub const STUN: &str = "stun";
    /// Stun applied this turn, which only starts counting next turn. A
    /// timed counter: see [`Tick::TurnStart`].
    pub const STUN_START: &str = "stunStart";
    /// Turns spent off the board (C# `MatchSeat.exile`).
    pub const EXILE: &str = "exile";
    /// Tile the exile returns to (C# `MatchSeat.exile_to`), or -1 for none.
    pub const EXILE_TO: &str = "exileTo";
    /// 「在[除外]层数归0后[传送]至该格子，视为当回合的主要移动」 -- when the
    /// exile ticks out, the return teleport **is** that turn's main move
    /// (MyGO:无路矢, C# `H.SetV(i, "exileMain", 1)`). The expiry tick reads and
    /// consumes it: `TurnCtx::main_moved` is set so the player cannot also roll.
    pub const EXILE_MAIN: &str = "exileMain";
    /// Fire pots held. Its `max` is the mandated cap (C# `MatchSeat.fireMax`),
    /// written by the character skill -- and that is the one number to show:
    /// there is no separate "effective cap" beside it.
    pub const FIRE: &str = "fire";
    /// Layers of "may hold no hand cards" (C# `MatchSeat.no_hand`).
    pub const NO_HAND: &str = "noHand";
    /// Layers of "cannot be stopped" (C# `MatchSeat.unstoppable`).
    pub const UNSTOPPABLE: &str = "unstoppable";
    /// Hand size limit (C# `MatchSeat.hand_limit`).
    pub const HAND_LIMIT: &str = "handLimit";
    /// The authoritative opening hand size (default 2; effects may lower it,
    /// minimum 0). Written at the before-match-start point and read by the
    /// opening draw. Mirrors `card_sdk::abi::state_key::START_HAND`.
    pub const START_HAND: &str = "startHand";
    /// Skill-system scratch (C# `MatchSeat.skill_state`).
    pub const SKILL_STATE: &str = "skillState";
    /// This player has built this turn (set by the build step).
    pub const BUILT: &str = "built";
    /// This player has bought this turn (set by the buy step).
    pub const BOUGHT: &str = "bought";
    /// This player has redeemed a deed this turn (「本回合进行过赎回操作」).
    pub const REDEEMED: &str = "redeemed";
    /// This player has mortgaged a deed this turn (「本回合进行过抵押操作」).
    pub const MORTGAGED: &str = "mortgaged";
    /// A card has closed building for this turn (「本回合无法加盖房屋」).
    pub const NO_BUILD: &str = "noBuild";

    /// Is `key` one the engine itself owns (a status gate the engine ticks or
    /// enforces), as opposed to a card's own scratch / latch key? Only the
    /// latter may be carried across a pause from a card's world copy -- a
    /// stale copy of a status gate must not clobber the live one.
    pub fn is_engine_owned(key: &str) -> bool {
        matches!(
            key,
            STAY | STUN | STUN_START | EXILE | EXILE_TO | FIRE | NO_HAND | UNSTOPPABLE
                | HAND_LIMIT | START_HAND | SKILL_STATE | BUILT | BOUGHT | REDEEMED | MORTGAGED
                | NO_BUILD
        )
            // `skill.*` is skill scratch -- a hook writes it and the hook's own
            // `swap_world` commits it; it must not be pulled back off a stale
            // body copy at a pause.
            || key.starts_with("skill.")
    }
}

/// Named **card properties** -- the keys of a card rule's declared property
/// map (`CardDef::props`, carried through the ruleset manifest). A property is
/// a static fact about the card rule, not an effect: the engine reads it back
/// by key and never by matching rulebook prose. Mirrors
/// `card_sdk::abi::prop` (the two crates cannot share a definition; keep them
/// in step). Every key has a defined default of `0` when a card does not
/// declare it.
pub mod prop {
    /// Continuous 「手卡上限数量减1」 (C# `Card.HandLimitDelta`) while the card
    /// sits on the field. Stamped onto the field instance at placement and
    /// gone with the card. `-1` cuts the owner's hand limit.
    pub const HAND_LIMIT_DELTA: &str = "handLimitDelta";
    /// 「可在眩晕时打出」 (C# `Card.PlayableStunned`): `1` = the card skips the
    /// stun gate when played from hand.
    pub const PLAYABLE_STUNNED: &str = "playableStunned";
    /// Bot-only **estimated execution cost**. Mirrors
    /// `card_sdk::abi::prop::EST_COST`. Read only by bots / autopilot as a
    /// reserve check -- never by legality. `0` = unknown / assume free.
    pub const EST_COST: &str = "estCost";
    /// Opt-in field counteraction / shared-choice metadata (ABI v47).
    pub const COUNTERACT_FROM_FIELD: &str = "counteractFromField";
    pub const COUNTERACT_GROUP: &str = "counteractGroup";
    pub const COUNTERACT_FIRE_COST: &str = "counteractFireCost";
    /// 「有[指定]目标」 (C# `Card.Def.Targeting`): `1` = this play names
    /// recipients (the play's other living players). Mirrors
    /// `card_sdk::abi::prop::DESIGNATES`.
    pub const DESIGNATES: &str = "designates";
    /// Virtual **rent** house count (「房屋数视为…」). Presence is the override
    /// (a count of `0` is legitimate); real `st.houses` is untouched. On a
    /// placed card of the tile's owner (`ctx::set_prop`, gone with the card) or
    /// on the tile's rule instance (`ctx::set_tile_prop`). Read by
    /// [`crate::engine::ops::World::rent_houses`]; `houses_of` stays real.
    pub const RENT_HOUSES: &str = "rentHouses";

    // ---------------------------------------------------------- tile data
    // Stamped onto a board-owned tile rule instance at `bind_tiles` (from
    // `TileData`), and read back by the settle bodies and by the end-step
    // buy/build gates. Mirrors `card_sdk::abi::prop`; see `docs/TILES.md`.

    /// Land price (houses are extra). `TileData.price`.
    pub const PRICE: &str = "price";
    /// Build cost per level. `TileData.house`.
    pub const HOUSE: &str = "house";
    /// Colour group (`TileData.group`). [`ALL_COLORS`] is 「该格获得所有颜色」.
    pub const GROUP: &str = "group";
    /// The tile's value that means 「该格获得所有颜色」.
    pub const ALL_COLORS: i32 = -2;
    /// 「每块地有标注的等级上限」 -- max houses. `TileData.rent.len() - 1`.
    pub const BUILD_MAX: &str = "buildMax";
    /// Length of the rent table (levels = houses + 1).
    pub const RENT_LEN: &str = "rentLen";
    /// Rent at level N: `rent:0` … `rent:rentLen-1` (`TileData.rent`).
    pub const RENT_PREFIX: &str = "rent:";
    /// RiNG rent multiplier (`match_rules.ring_multiplier`). TODO(规则书).
    pub const RING_MULT: &str = "ringMult";

    // ------------------------------------------------- tile rule modifiers
    // What a card that bends a tile writes onto its rule instance, replacing
    // the engine flags these used to be. See `docs/TILES.md`.

    /// 「无法获取[CiRCLE奖励]」 on this tile's `tile:circle` instance.
    pub const NO_REWARD: &str = "noReward";
    /// Rent scale in milli-units (500 = x0.5). Replaces `rent_factor`.
    pub const RENT_FACTOR: &str = "rentFactor";
    /// Payment scale in milli-units (500 = x0.5). Replaces `pay_factor`.
    pub const PAY_FACTOR: &str = "payFactor";
    /// 「[拥有者]不可盖房」. Replaces the `noBuild` state key.
    pub const NO_BUILD: &str = "noBuild";
    /// 「不可在造价N及以上的格子上加盖房屋」 (卡池BUG) -- `why_not_build_on`
    /// refuses a build whose house cost is `>=` this. On a board-owned instance;
    /// `0` disables. Mirrors `card_sdk::abi::prop::NO_BUILD_ABOVE`.
    pub const NO_BUILD_ABOVE: &str = "noBuildAbove";

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

/// Is `key` one of the engine's status counters, whose value cannot go
/// negative (「清除」 floors at 0)?
fn is_status_key(key: &str) -> bool {
    matches!(
        key,
        key::STAY | key::STUN | key::STUN_START | key::EXILE | key::FIRE
    )
}

/// The tick the rulebook names for a timed status counter (规则书:
/// [停留]/[晕眩] 「玩家的每回合结束时移除一层」; `stunStart` is the layer that
/// starts counting next turn, `Tick::TurnStart`). `state_set` stamps this on
/// any write whose item has no tick, so the book's wear-off holds no matter
/// which writer landed the layer.
fn default_expiry(key: &str) -> Option<Tick> {
    match key {
        key::STAY | key::STUN => Some(Tick::TurnEnd),
        key::STUN_START => Some(Tick::TurnStart),
        _ => None,
    }
}

/// How a bot player decides. Serde-defaults to [`Self::Standard`] so older
/// saves and room records load unchanged.
///
/// Only `bot` seats take a mentality: a human who times out or disconnects is
/// still answered with the standard policy (`ai` flips on, `bot` stays off).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotMentality {
    /// The ported C# bot (`ai.rs`) -- reserved money, card-play odds, cap per turn.
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
    /// stays engine-side.
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
    pub character: String,
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
    pub skill_character: String,
    pub bands: String,
    pub tokens: Vec<Counter>,
    /// Keyed state: the counters this player carries (C# `MatchPlayer`'s pots
    /// plus the `H.V` slots) as `{value, min, max}` items. See [`StateVar`] --
    /// the engine holds these and enforces nothing. Keys are content
    /// vocabulary; [`key`] names the ones the engine itself reads back.
    ///
    /// The key *names* are a wire contract, not just internal labels: the
    /// client reads this map by name and TS types are structural, so renaming
    /// a key breaks the board render silently rather than at compile time. The
    /// names the client actually reads today are `stay`, `stun`, `stunStart`,
    /// `exile`, `exileTo`, `fire`, and `handLimit`. Treat those as frozen;
    /// invent new ones freely.
    pub state: BTreeMap<String, StateVar>,
    pub field: Vec<FieldCard>,
    pub actions: Vec<SkillAction>,
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
            character: String::new(),
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
            skill_character: String::new(),
            bands: String::new(),
            tokens: vec![],
            state: {
                // The two pots whose C# field initializers are not 0. Everything
                // else falls out of .
                let mut m = BTreeMap::new();
                m.insert(
                    key::HAND_LIMIT.to_string(),
                    StateVar {
                        value: 5,
                        ..StateVar::default()
                    },
                );
                m.insert(
                    key::EXILE_TO.to_string(),
                    StateVar {
                        value: -1,
                        ..StateVar::default()
                    },
                );
                m
            },
            field: vec![],
            actions: vec![],
            assets: 0,
            score: 0,
            rank: 0,
        }
    }
}

impl MatchPlayer {
    /// Does the machine handle this seat's **setup** (ban / pick / deck) without
    /// waiting on a human or the server? True for every engine-driven seat
    /// (`ai`) and for an Advanced bot: its play-phase decisions belong to the
    /// server's `bot-service` (`docs/BOT.md` B5), but setup stays engine-side
    /// and runs the standard policy, exactly like a standard bot.
    pub fn auto_setup(&self) -> bool {
        self.ai || (self.bot && self.mentality == BotMentality::Advanced)
    }

    // ---------------------------------------------------------- keyed state
    // Dumb storage. The engine holds `{value, min, max}` items and enforces
    // none of it -- see [`StateVar`]. Everything below that looks like a rule
    // is a *reader*: the engine's own game flow asking for a value it named.

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

    /// Write `value`, clamped to the item's declared bounds.
    ///
    /// The engine holds the bounds and enforces them on write: a status counter
    /// (stay / stun / exile / fire) never goes negative (「清除」 floors at 0),
    /// and a declared `max > 0` is a real cap (fire pots 「上限M」). `max == 0`
    /// still means "uncapped". A consumer that wants a different policy (e.g.
    /// `gain_fire`'s partial gain + log) does its own arithmetic first.
    ///
    /// A **fresh** stay / stun / stunStart item also picks up the rulebook's
    /// wear-off tick (see [`default_expiry`]): 「每回合结束时移除一层」 is a
    /// property of the status, not of whichever writer happened to land the
    /// layer, so a raw `state_set("stun", n)` cannot leave a permanent stun
    /// that skips every later turn. The tick is re-asserted whenever it has
    /// been cleared (`state_set_expires(..., None)`) -- the book has no
    /// non-ticking [晕眩] -- but a non-default tick the writer asked for stays.
    pub fn state_set(&mut self, key: &str, value: i32) -> i32 {
        let e = self.state.entry(key.to_string()).or_default();
        if e.expires.is_none() {
            e.expires = default_expiry(key);
        }
        let mut v = value;
        // Status counters cannot go negative -- clearing floors at 0.
        if is_status_key(key) {
            v = v.max(0);
        }
        // A declared cap is honoured (`max > 0`; `max == 0` is uncapped).
        if e.max > 0 {
            v = v.min(e.max);
        }
        e.value = v;
        v
    }

    /// Add `delta`, clamped like [`Self::state_set`].
    pub fn state_add(&mut self, key: &str, delta: i32) -> i32 {
        let v = self.state_get(key).saturating_add(delta);
        self.state_set(key, v)
    }

    /// Declare the bounds a consumer may enforce. `max > 0` is enforced on
    /// the next write; `max == 0` is uncapped.
    pub fn state_set_bounds(&mut self, key: &str, min: i32, max: i32) {
        let e = self.state.entry(key.to_string()).or_default();
        e.min = min;
        e.max = max;
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

    // ------------------------------------------- the engine's named readers

    /// Stay layers (C# `stay`).
    pub fn stay(&self) -> i32 {
        self.state_get(key::STAY)
    }

    /// Stun layers (C# `stun`).
    pub fn stun(&self) -> i32 {
        self.state_get(key::STUN)
    }

    /// Stun applied this turn (C# `stun_start`).
    pub fn stun_start(&self) -> i32 {
        self.state_get(key::STUN_START)
    }

    pub fn stunned(&self) -> bool {
        self.stun() + self.stun_start() > 0
    }

    /// Turns left off the board (C# `exile`).
    pub fn exile(&self) -> i32 {
        self.state_get(key::EXILE)
    }

    /// Tile the exile returns to (C# `exile_to`), or -1.
    pub fn exile_to(&self) -> i32 {
        self.state_get(key::EXILE_TO)
    }

    /// Fire pots held (C# `fire`).
    pub fn fire(&self) -> i32 {
        self.state_get(key::FIRE)
    }

    /// The mandated fire-pot cap (C# `fireMax`) -- the `max` of the `fire` item,
    /// written by a character skill. Not something the engine imposes.
    pub fn fire_max(&self) -> i32 {
        self.state_max(key::FIRE)
    }

    pub fn no_hand(&self) -> i32 {
        self.state_get(key::NO_HAND)
    }

    pub fn unstoppable(&self) -> i32 {
        self.state_get(key::UNSTOPPABLE)
    }

    /// Hand size limit: the `handLimit` state (base 5, cards may cut or lift
    /// it) plus every placed card's continuous `prop::HAND_LIMIT_DELTA`
    /// (「手卡上限数量减1」).
    pub fn hand_limit(&self) -> i32 {
        let field: i32 = self
            .field
            .iter()
            .map(|f| f.props.get(prop::HAND_LIMIT_DELTA).copied().unwrap_or(0))
            .sum();
        self.state_get(key::HAND_LIMIT) + field
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
}

/// `MatchPrompt.cs`
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

/// `MatchVote.cs` -- the vote to end the match early.
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

/// `MatchEvent.cs` -- one log/animation event. `id` increases monotonically per match,
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
}

/// The [`MatchEvent::kind`] values of a `"card"` activation event.
pub mod card_trigger {
    /// A card played from hand (`PlayFromHand`).
    pub const PLAY: &str = "play";
    /// A skill press (`use_skill`).
    pub const SKILL: &str = "skill";
    /// A drawn event card's effect.
    pub const EVENT: &str = "event";
    /// A [反击] card's body.
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
        }
    }
}

/// `ActiveEvent.cs`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ActiveEvent {
    pub id: String,
    pub player_id: i32,
    pub counter: i32,
    pub counter2: i32,
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
            note: Msg::default(),
            face_down: false,
        }
    }
}

/// `FieldCard.cs` -- a card on a player's field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FieldCard {
    pub uid: i32,
    pub card: String,
    pub owner: i32,
    pub user: i32,
    pub tile: i32,
    pub crystals: i32,
    /// On-card [CP点] -- 「自己[场上]N个[CP点]」, the CP points **attached to this
    /// card** (user ruling 2026-10-07: 「自己[场上]1个[CP点] referred to the cp
    /// point attached to the card」). One of the two [CP点] kinds: this is the
    /// card rule's own stock (通用:该清CP了 [手] 「在自己[场上]添加6个[CP点]」),
    /// as against the neutral [CP点] **tile marks** in [`TileMark`] (category
    /// [`mark_category::CP`], mandated by `mark:cp`). Crystals-like: lives on
    /// the instance, rides the view's field-card counter badge, and is what
    /// `HookKind::CpChanged` watches. Serde-defaulted so pre-CP saves load.
    #[serde(default)]
    pub cp: i32,
    pub face_down: bool,
    /// C# `Card.Immune` -- 「此卡不受…效果影响」. A value on the card, not a
    /// subclass override: effects that would touch it read this and skip.
    pub immune: bool,
    /// The card rule's declared static **properties** (see [`prop`]), stamped
    /// onto the instance at placement and gone with it. A continuous effect
    /// like 「手卡上限数量减1」 (`prop::HAND_LIMIT_DELTA`) rides here, so a
    /// `place_raw` test arrangement sees it too.
    #[serde(default)]
    pub props: BTreeMap<String, i32>,
    /// This instance is a **band skill** (`skill:<band>:<skill>`, stamped at
    /// placement from `GameData::is_band_skill`). 「乐队卡 / 团卡」 crystal
    /// reads and writes land on this instance's [`Self::crystals`] -- see
    /// `World::band_crystals`. A character skill is `skill:<character>:<skill>`
    /// and is *not* flagged, even when it sits on someone else's field.
    #[serde(default)]
    pub band_skill: bool,
    /// A 「拿取」ed band-skill copy (C# `BandBase.Extra`, `H.MakeBand(.., extra)`).
    /// 「相同乐队技能卡的效果不可叠加」 -- the hook dispatch skips an extra when
    /// a non-extra attachment of the same id is already on the field -- and
    /// 「不视为那个乐队的角色」 (`in_band` reads only the character).
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
            crystals: 0,
            cp: 0,
            face_down: false,
            immune: false,
            props: BTreeMap::new(),
            band_skill: false,
            extra: false,
            note: Msg::default(),
        }
    }
}

/// Tile-mark **categories** -- what sort of thing a [`TileMark`] is. A category
/// is the mark's identity for the board, as against `TileMark::kind` (the i18n
/// key that names a particular mark) and `TileMark::owner` (which seat placed a
/// player mark). Mirrors `card_sdk::abi::mark` (the two crates cannot share a
/// definition; keep them in step).
///
/// [CP点] is its own category, not a `kind` string mixed in with the player
/// marks (`data/rules.txt` 125: 「CP点：放置于路面上的指示物」), and it carries
/// **no player owner** -- see [`TileMark::owner`].
pub mod mark_category {
    /// A player/generic mark (the default): 兔子 / 奇迹水晶 / PAREO / 抹茶巴菲 …
    /// Placed by a seat, coloured by that seat in the view.
    pub const PLAYER: &str = "";
    /// [CP点] -- 「放置于路面上的指示物」. Neutral: `owner` is always
    /// [`super::BOARD_OWNER`], never a seat.
    pub const CP: &str = "cp";
}

/// Well-known tile-mark kinds (the i18n key / stable id on `TileMark::kind`).
/// Mirrors `card_sdk::abi::mark`.
pub mod mark_kind {
    /// The [CP点] mark's stable kind. Its display label comes from the
    /// category (「CP点」), not from this string.
    pub const CP: &str = "mark:cp";
}

/// `TileMark.cs` -- a marker placed on a tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TileMark {
    pub uid: i32,
    pub tile: i32,
    /// `card` for a card placed on the tile (see `card`); otherwise the i18n key
    /// naming the mark (e.g. `cards:hhw-hagumi-marks.mark`). [CP点] uses the
    /// stable [`mark_kind::CP`] -- the label comes from [`Self::category`].
    pub kind: String,
    /// Which category of mark this is: [`mark_category::PLAYER`] (default) or
    /// [`mark_category::CP`]. Serde-defaulted so pre-category saves still load.
    pub category: String,
    /// The seat that placed a **player** mark (colours it in the view), or
    /// [`BOARD_OWNER`] (`-1`) for a neutral mark. [CP点] is always neutral --
    /// 「These marks should not be owned by any player」 -- so a CP mark's owner
    /// is [`BOARD_OWNER`] and its provenance lives in [`Self::src`] /
    /// [`Self::card`] instead.
    pub owner: i32,
    pub count: i32,
    /// Provenance for the log / the view's 「来自」: the card id that placed the
    /// mark (empty when none). Not an owner.
    pub card: String,
    /// Provenance for the rules: the **card instance** (`FieldCard::uid`) that
    /// placed the mark, or `-1`. 「此卡在格子上添加的[CP点]及其产物」
    /// (通用:该清CP了 (1)) keys on this, not on [`Self::owner`]. Serde-defaulted
    /// so pre-`src` saves still load.
    pub src: i32,
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
            src: -1,
            note: Msg::default(),
        }
    }
}

impl TileMark {
    /// Is this a [CP点]? CP marks are a category of their own, not a `kind`
    /// string among the player marks.
    pub fn is_cp(&self) -> bool {
        self.category == mark_category::CP
    }
}

/// `Counter.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Counter {
    pub name: String,
    pub value: i32,
}

/// `SkillAction.cs` -- a skill button available to a player.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SkillAction {
    pub id: String,
    pub source: String,
    pub title: Msg,
    pub text: Msg,
    pub enabled: bool,
    pub reason: Msg,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 规则书 [停留]/[晕眩] 「玩家的每回合结束时移除一层」 and `stunStart`'s
    /// turn-start tick are properties of the status: a write that names no
    /// expiry still wears off. (Fuzz soak iter 1517 -- a raw `state_set("stun",
    /// 1)` left a permanent stun that skipped every later turn and turned
    /// PPP:Returns' turn-start ask into a prompt storm.)
    #[test]
    fn status_writes_carry_the_books_tick() {
        let mut p = MatchPlayer::default();
        p.state_set(key::STAY, 1);
        p.state_set(key::STUN, 1);
        p.state_set(key::STUN_START, 1);
        assert_eq!(p.state.get(key::STAY).unwrap().expires, Some(Tick::TurnEnd));
        assert_eq!(p.state.get(key::STUN).unwrap().expires, Some(Tick::TurnEnd));
        assert_eq!(
            p.state.get(key::STUN_START).unwrap().expires,
            Some(Tick::TurnStart)
        );
        // The tick actually moves the counter.
        p.tick_state(Tick::TurnEnd);
        assert_eq!(p.stay(), 0);
        assert_eq!(p.stun(), 0);
        assert_eq!(p.stun_start(), 1, "stunStart waits for its own tick");
        p.tick_state(Tick::TurnStart);
        assert_eq!(p.stun_start(), 0);
    }

    /// An explicit non-default tick the writer asked for stays.
    #[test]
    fn explicit_expiry_is_not_overridden() {
        let mut p = MatchPlayer::default();
        p.state_set(key::STUN, 1);
        p.state_set_expires(key::STUN, Some(Tick::TurnStart));
        p.state_set(key::STUN, 2);
        assert_eq!(p.state.get(key::STUN).unwrap().expires, Some(Tick::TurnStart));
    }
}
