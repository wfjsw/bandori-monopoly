//! Match state broadcast to clients (`BandoriMonopoly.Net/Match*.cs`).
//!
//! Same field names and C# initializer defaults as the original, except that all
//! display text is a localizable [`Msg`] instead of a finished string.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
    pub tile_colors: Vec<i32>,
    pub event_deck: i32,
    pub event_top: Vec<String>,
    pub event_discard: Vec<String>,
    pub event_removed: Vec<String>,
    pub event_active: Vec<ActiveEvent>,
    pub prompt: MatchPrompt,
    pub vote: MatchVote,
    pub events: Vec<MatchEvent>,
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
            tile_colors: vec![],
            event_deck: 0,
            event_top: vec![],
            event_discard: vec![],
            event_removed: vec![],
            event_active: vec![],
            prompt: MatchPrompt::default(),
            vote: MatchVote::default(),
            events: vec![],
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
    /// Per-player `Fx.ExtraColor`: `extraColor:<tile>` = the group that tile
    /// counts as **for this player** (`-1` clears).
    pub const EXTRA_COLOR: &str = "extraColor:";
    /// The `tile_colors` / `extraColor` value that means 「该格获得所有颜色」.
    pub const ALL_COLORS: i32 = -2;
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
        ) || key.starts_with(EXTRA_COLOR)
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
    /// 「购买格子时[消耗]资金降低N（最低0）」. Replaces `buy_discount`.
    pub const BUY_DISCOUNT: &str = "buyDiscount";
    /// 「购买格子不[消耗]资金」. Replaces `free_buy`.
    pub const FREE_BUY: &str = "freeBuy";
    /// 「如果购买则拆除那个格子上的所有房屋」. Replaces `raze_on_buy`.
    pub const RAZE_ON_BUY: &str = "razeOnBuy";
    /// 「[拥有者]不可盖房」. Replaces the `noBuild` state key.
    pub const NO_BUILD: &str = "noBuild";
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
}

impl BotMentality {
    pub fn as_str(&self) -> &'static str {
        match self {
            BotMentality::Standard => "standard",
            BotMentality::Chaos => "chaos",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "standard" | "" => Some(BotMentality::Standard),
            "chaos" => Some(BotMentality::Chaos),
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
    pub fn state_set(&mut self, key: &str, value: i32) -> i32 {
        let e = self.state.entry(key.to_string()).or_default();
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
            face_down: false,
            immune: false,
            props: BTreeMap::new(),
            band_skill: false,
            extra: false,
            note: Msg::default(),
        }
    }
}

/// `TileMark.cs` -- a marker placed on a tile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TileMark {
    pub uid: i32,
    pub tile: i32,
    /// `card` for a card placed on the tile (see `card`); otherwise the i18n key
    /// naming the mark (e.g. `cards:hhw-hagumi-marks.mark`).
    pub kind: String,
    pub owner: i32,
    pub count: i32,
    pub card: String,
    pub note: Msg,
}

impl Default for TileMark {
    fn default() -> Self {
        Self {
            uid: 0,
            tile: 0,
            kind: String::new(),
            owner: -1,
            count: 1,
            card: String::new(),
            note: Msg::default(),
        }
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
