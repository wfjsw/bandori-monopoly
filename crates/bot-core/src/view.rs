//! One seat's view -- the bot's **only** input (`docs/BOT.md` §1).
//!
//! Exactly the frame the client gets from `web-glue`'s `SoloMatch::view` /
//! the server's `match` frame: [`MatchState`] + own hand + own draw
//! composition + the open prompt + the per-viewer extras. No `World`, no seed,
//! no other seat's hand / deck order / `aiAnswer`.

use game_core::engine::Match;
use game_core::state::{stage, MatchPlayer, MatchPrompt, MatchState, MovePlan};

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

// ------------------------------------------- speculative next-decision view

/// Is this seat's **next own decision** near enough for a speculative search
/// to be worth its CPU (`docs/BOT.md` §3.5)?
///
/// True when the bot is about to face a surface of its own:
///
/// * it is already our turn (a routine is running or the turn just began --
///   the next decision is this turn's 运营 / 结束 surface);
/// * the current player's turn is at 结束 (about to end, so our turn is next);
/// * we are the next active seat in the ring.
///
/// False for a long wait (three other seats still to act) and for setup
/// phases. This is the gate that replaces "send a `ponder` on every idle
/// probe" -- an idle view has no searchable surface and today 0 of ~14 k idle
/// ponders ever searched (`target/scratch/profile/REPORT.md` §6).
pub fn next_turn_near(st: &MatchState, player_id: i32) -> bool {
    if st.phase != "play" || player_id < 0 {
        return false;
    }
    if st.busy && st.turn != player_id {
        // Someone else's mid-routine pause: the turn still has to finish.
        return next_active(st) == Some(player_id) || st.step == stage::END;
    }
    if st.turn == player_id {
        // Our turn. The next surface is ours unless we are already parked on
        // a decision the drive would have claimed (`decision_at` handles that
        // case -- this function is only consulted when it returned `None`).
        return true;
    }
    if st.turn < 0 {
        return false;
    }
    // The current player's turn is ending (结束), or we are next in the ring.
    st.step == stage::END || next_active(st) == Some(player_id)
}

/// The next non-out seat after `st.turn` in the engine's ring (no extra-turn
/// bookkeeping -- a public state cannot see `extra_turns`, and an extra turn
/// for the current player only makes "we are next" false for us, which is
/// the safe direction).
fn next_active(st: &MatchState) -> Option<i32> {
    let n = st.players.len() as i32;
    if n <= 0 || st.turn < 0 {
        return None;
    }
    for k in 1..=n {
        let c = ((st.turn + k) % n + n) % n;
        let Some(p) = st.players.get(c as usize) else {
            continue;
        };
        if !p.out() {
            return Some(c);
        }
    }
    None
}

/// The view the seat will face at its **turn-start 运营 surface** -- the
/// speculative `ponder` target (`docs/BOT.md` §3.5). `None` when the next
/// decision is not a predictable turn start: a mid-move 结束 surface depends
/// on where the dice land, a prompt that has not opened yet is anyone's
/// guess, and setup phases have no turn.
///
/// The prediction rewrites exactly the turn-boundary fields `next_turn`
/// resets (`engine/play.rs`): `turn` / `step` / `roller` / `landed` /
/// `bought` / `built` / `skip_move` / the empty prompt, plus the `can_*_here`
/// gates and the buy/build previews for a position that has not landed yet.
/// Hand, draw composition and every other seat's public record are taken
/// as-is.
///
/// The result's [`SeatView::decision_key`] matches the real turn-start
/// decision **when nothing else about the public state changes before that
/// turn begins** -- then a later `decide` on the same key answers from the
/// ponder cache without spending its budget. Otherwise the pondered tree is
/// still a warm start for the seat's real search (tree reuse, `retain_after`
/// / the per-`(room, seat)` trees) and the CPU went to a real decision
/// surface instead of a dead idle view.
pub fn predict_upcoming_view(view: &SeatView) -> Option<SeatView> {
    let st = &view.state;
    if st.phase != "play" {
        return None;
    }
    let me = view.player_id;
    if me < 0 {
        return None;
    }
    // A prompt open on someone else's seat is not a predictable turn start.
    // (A prompt already waiting on us is a live decision -- the drive claims
    // it and never reaches this path.)
    if st.prompt.id > 0 {
        return None;
    }
    // Mid-move: the 结束 surface's `landed` (and therefore the buy / build
    // menu) depends on the dice -- do not guess it.
    if st.turn == me && st.step == stage::MOVE {
        return None;
    }
    // Mid-routine on our own turn past the start: the routine may land on a
    // prompt or the 结束 surface. Only the 开始阶段 → 运营 transition is
    // predictable.
    if st.turn == me && st.step != stage::START && st.busy {
        return None;
    }
    if !next_turn_near(st, me) {
        return None;
    }

    let mut next = view.clone();
    let wrap = st.turn >= 0 && {
        // `next_turn` bumps `round` when the ring wraps to an equal-or-earlier
        // index (`engine/play.rs`). Only applies when we are taking over from
        // someone else's turn.
        let n = st.players.len() as i32;
        n > 0 && me <= st.turn && st.turn != me
    };
    {
        let s = &mut next.state;
        s.turn = me;
        s.step = stage::OPS;
        s.roller = me;
        s.busy = false;
        s.landed = -1;
        s.bought = false;
        s.built = false;
        s.skip_move = false;
        s.prompt = MatchPrompt::default();
        s.plan = MovePlan::default();
        // Nothing is buyable / buildable at a position that has not been
        // reached; roll is owed, so `end` is gated off (`why_not_act`).
        s.can_buy_here = false;
        s.can_build_here = false;
        s.can_roll_here = true;
        s.can_end_here = false;
        s.buy_price = -1;
        s.build_cost = -1;
        if wrap {
            s.round += 1;
        }
        // Clocks / seq are stripped from `decision_key`; zero them anyway so
        // the predicted frame does not carry a stale countdown into a search.
        // (`think_time` is the room setting, not a clock -- kept.)
        s.seq = 0;
        s.time_left = 0.0;
        s.shield = 0.0;
        s.bank = 0.0;
        s.prompt.time_left = 0.0;
    }
    // The turn-start 运营 surface's per-card playability is **not** the
    // current frame's: `playable` is `cant_play` for the live state, and while
    // another seat acts most cards answer "no" (it is not our 运营). Keeping
    // it would make the predicted root look empty and the speculative search
    // a no-op -- which is exactly what the pre-C1 idle polls did. Mark every
    // hand card as a candidate instead: the search's own `MatchSim` recomputes
    // real `cant_play` on each fork and only branches on what the engine
    // accepts. `est_cost` is a static card property and stays.
    next.playable = vec![true; next.hand.len()];
    next.ai_answer = None;
    Some(next)
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