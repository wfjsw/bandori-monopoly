//! Match state broadcast to clients (`BandoriMonopoly.Net/Match*.cs`).
//!
//! Same field names and C# initializer defaults as the original, except that all
//! display text is a localizable [`Msg`] instead of a finished string.

use serde::{Deserialize, Serialize};

use crate::msg::Msg;

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
    pub time_left: f32,
    pub shield: f32,
    pub bank: f32,
    pub bought: bool,
    pub built: bool,
    pub seats: Vec<MatchSeat>,
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
            time_left: 0.0,
            shield: 0.0,
            bank: 0.0,
            bought: false,
            built: false,
            seats: vec![],
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

    /// Seat whose turn it is.
    pub fn current(&self) -> Option<&MatchSeat> {
        usize::try_from(self.turn).ok().and_then(|t| self.seats.get(t))
    }

    pub fn asking(&self) -> bool {
        self.prompt.id != 0
    }

    pub fn voting(&self) -> bool {
        self.vote.id != 0
    }

    /// Seats still in the game.
    pub fn alive(&self) -> usize {
        self.seats.iter().filter(|s| !s.out()).count()
    }

    /// Seat index of a room member, or -1.
    pub fn seat_of(&self, member: i32) -> i32 {
        self.seats.iter().position(|s| s.member == member).map_or(-1, |i| i as i32)
    }

    /// Is the event active and face-up?
    pub fn event_active(&self, id: &str) -> bool {
        self.event_active.iter().any(|e| e.id == id && !e.face_down)
    }
}

/// `MatchSeat.cs`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchSeat {
    pub member: i32,
    pub player: String,
    pub bot: bool,
    pub ai: bool,
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
    pub stay: i32,
    pub stun: i32,
    pub stun_start: i32,
    pub fire_cap: i32,
    pub exile: i32,
    pub exile_to: i32,
    pub no_hand: i32,
    pub unstoppable: i32,
    pub fire: i32,
    pub fire_max: i32,
    pub skill_state: i32,
    pub skill_note: Msg,
    pub skill_character: String,
    pub band_crystals: i32,
    pub bands: String,
    pub tokens: Vec<Counter>,
    /// Per-seat value slots (C# `H.V` / `SetV` / `IncV`).
    pub slots: Vec<Counter>,
    pub field: Vec<FieldCard>,
    pub hand_limit: i32,
    pub actions: Vec<SkillAction>,
    pub assets: i32,
    pub score: i32,
    pub rank: i32,
}

impl Default for MatchSeat {
    fn default() -> Self {
        Self {
            member: 0,
            player: String::new(),
            bot: false,
            ai: false,
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
            stay: 0,
            stun: 0,
            stun_start: 0,
            fire_cap: 0,
            exile: 0,
            exile_to: -1,
            no_hand: 0,
            unstoppable: 0,
            fire: 0,
            fire_max: 0,
            skill_state: 0,
            skill_note: Msg::default(),
            skill_character: String::new(),
            band_crystals: 0,
            bands: String::new(),
            tokens: vec![],
            slots: vec![],
            field: vec![],
            hand_limit: 5,
            actions: vec![],
            assets: 0,
            score: 0,
            rank: 0,
        }
    }
}

impl MatchSeat {
    pub fn stunned(&self) -> bool {
        self.stun + self.stun_start > 0
    }

    /// Bankrupt or left the match.
    pub fn out(&self) -> bool {
        self.bankrupt || self.left
    }

    /// `MatchSeat.Token(name)` -- 0 if absent.
    pub fn token(&self, name: &str) -> i32 {
        self.tokens.iter().find(|c| c.name == name).map_or(0, |c| c.value)
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
    pub seats: Vec<i32>,
    /// Parallel to `seats`; -1 = not answered yet.
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
            seats: vec![],
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
    pub fn seat_index(&self, seat: i32) -> Option<usize> {
        self.seats.iter().position(|&s| s == seat)
    }

    /// Is `seat` asked and has not answered yet?
    pub fn waiting(&self, seat: i32) -> bool {
        self.seat_index(seat).and_then(|i| self.answers.get(i)).is_some_and(|&a| a < 0)
    }
}

/// `MatchVote.cs` -- the vote to end the match early.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchVote {
    pub id: i32,
    pub by: i32,
    pub seats: Vec<i32>,
    pub answers: Vec<i32>,
    pub time_left: f32,
}

impl Default for MatchVote {
    fn default() -> Self {
        Self { id: 0, by: -1, seats: vec![], answers: vec![], time_left: 0.0 }
    }
}

impl MatchVote {
    pub fn yes(&self) -> usize {
        self.answers.iter().filter(|&&a| a == 1).count()
    }

    pub fn waiting(&self, seat: i32) -> bool {
        self.seats.iter().position(|&s| s == seat).and_then(|i| self.answers.get(i)).is_some_and(|&a| a < 0)
    }
}

/// `MatchEvent.cs` -- one log/animation event. `id` increases monotonically per match,
/// which is what makes SSE `Last-Event-ID` resume work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MatchEvent {
    pub id: i32,
    pub r#type: String,
    pub seat: i32,
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
            seat: -1,
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
    pub seat: i32,
    pub counter: i32,
    pub counter2: i32,
    pub note: Msg,
    pub face_down: bool,
}

impl Default for ActiveEvent {
    fn default() -> Self {
        Self { id: String::new(), seat: -1, counter: 0, counter2: 0, note: Msg::default(), face_down: false }
    }
}

/// `FieldCard.cs` -- a card on a seat's field.
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
    pub note: Msg,
}

impl Default for FieldCard {
    fn default() -> Self {
        Self { uid: 0, card: String::new(), owner: -1, user: -1, tile: -1, crystals: 0, face_down: false, note: Msg::default() }
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
        Self { uid: 0, tile: 0, kind: String::new(), owner: -1, count: 1, card: String::new(), note: Msg::default() }
    }
}

/// `Counter.cs`
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Counter {
    pub name: String,
    pub value: i32,
}

/// `SkillAction.cs` -- a skill button available to a seat.
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
