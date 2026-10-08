//! One seat's view -- the bot's **only** input (`docs/BOT.md` §1).
//!
//! Exactly the frame the client gets from `web-glue`'s `SoloMatch::view` /
//! the server's `match` frame: [`MatchState`] + own hand + own draw
//! composition + the open prompt + the per-viewer extras. No `World`, no seed,
//! no other seat's hand / deck order / `aiAnswer`.

use game_core::engine::Match;
use game_core::state::{MatchPlayer, MatchState};

/// The engine's own heuristic answer for **this** seat's live prompt
/// (`Match::view_extra`'s `aiAnswer`). Never another seat's entry.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AiAnswer {
    pub answer: i32,
    pub picked: Vec<String>,
    pub worth: i32,
}

/// What the searching seat sees. Built from public accessors only.
///
/// Serde shape is the **view frame** the client gets (`rules-worker`'s `view`
/// op / `web-glue`'s `SoloMatch::view`): `state` + own `hand` + sorted `draw`
/// + `you` / `playerId` + `aiAnswer` / `playable`. Extra frame fields
/// (`handNotes`) are ignored, so the bot service can take the frame verbatim.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SeatView {
    /// The public state, live prompt / vote / clocks filled in.
    pub state: MatchState,
    /// Own hand, in order.
    pub hand: Vec<String>,
    /// Own remaining draw pile, **sorted by card id** (composition only --
    /// the engine never lets the order leave it).
    pub draw: Vec<String>,
    /// The seat's member id.
    pub you: i32,
    /// The seat's player index.
    pub player_id: i32,
    /// The engine's heuristic answer for this seat's live prompt, if any.
    pub ai_answer: Option<AiAnswer>,
    /// Parallel to [`Self::hand`]: would `cant_play` allow each card right now?
    pub playable: Vec<bool>,
    /// Parallel to [`Self::hand`]: the card's bot-only **estimated execution
    /// cost** (`prop::EST_COST`, user ruling 2026-10-07). Reserve check only,
    /// never legality. `0` = unknown / assume free.
    pub est_cost: Vec<i32>,
}

impl Default for SeatView {
    fn default() -> Self {
        Self {
            state: MatchState::default(),
            hand: Vec::new(),
            draw: Vec::new(),
            you: 0,
            player_id: 0,
            ai_answer: None,
            playable: Vec::new(),
            est_cost: Vec::new(),
        }
    }
}

impl SeatView {
    /// The view frame for `member`, from the public [`Match`] accessors only
    /// (`state` / `hand_of` / `draw_of` / `view_extra`). This is what a remote
    /// bot service would be sent; nothing else is read.
    pub fn from_match(m: &Match, member: i32) -> Self {
        let state = m.state();
        let player_id = state.player_of(member);
        let extra = m.view_extra(member);
        let ai_answer = extra.get("aiAnswer").cloned().and_then(|v| {
            if v.is_null() {
                None
            } else {
                serde_json::from_value::<AiAnswer>(v).ok()
            }
        });
        let playable = extra
            .get("playable")
            .and_then(|v| serde_json::from_value::<Vec<bool>>(v.clone()).ok())
            .unwrap_or_default();
        let est_cost = extra
            .get("estCost")
            .and_then(|v| serde_json::from_value::<Vec<i32>>(v.clone()).ok())
            .unwrap_or_default();
        Self {
            hand: m.hand_of(member),
            draw: m.draw_of(member),
            state,
            you: member,
            player_id,
            ai_answer,
            playable,
            est_cost,
        }
    }

    /// My seat's public record.
    pub fn me(&self) -> &MatchPlayer {
        &self.state.players[self.player_id as usize]
    }

    /// Is a prompt open and still waiting on this seat?
    pub fn prompt_waiting(&self) -> bool {
        self.state.prompt.id != 0 && self.state.prompt.waiting(self.player_id)
    }

    /// Normalised decision key material: the public state a player knows,
    /// with the volatile clocks stripped. Two states that look identical to
    /// this seat map to the same key -- the information-set identity.
    pub fn decision_key(&self) -> u64 {
        let mut st = self.state.clone();
        st.seq = 0;
        st.time_left = 0.0;
        st.think_time = 0;
        st.shield = 0.0;
        st.bank = 0.0;
        st.prompt.time_left = 0.0;
        // The event tail is public history; keep it (it distinguishes info
        // sets) but drop nothing else.
        let mut h = Fnv::new();
        h.write(&serde_json::to_vec(&st).unwrap_or_default());
        for c in &self.hand {
            h.write(c.as_bytes());
            h.write(b"\x1f");
        }
        h.write(b"\x1e");
        // Draw is a *composition* (sorted); order is never known.
        for c in &self.draw {
            h.write(c.as_bytes());
            h.write(b"\x1f");
        }
        h.finish()
    }
}

/// Small FNV-1a hasher -- no extra dependency, stable across runs.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(0x1000_0000_01b3);
        }
    }
    fn finish(&self) -> u64 {
        self.0
    }
}