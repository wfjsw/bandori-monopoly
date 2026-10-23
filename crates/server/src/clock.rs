//! Turn-clock deadlines and the NTP-style sync stamps that carry them.
//!
//! The server clock is **monotonic milliseconds since process start**, not
//! wall time. Deadlines and ping T2/T3 stamps are expressed on that same
//! clock, so a host wall-clock step cannot slide a live countdown. Clients
//! map their own `performance.now()` onto it (`webui/src/game/clockSync.ts`)
//! and render `remaining = deadline - (performance.now() + offset)`.
//!
//! See `docs/SERVER.md` §Turn clock.

use std::sync::OnceLock;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use game_core::state::MatchState;

/// Hard cap on how long the acting seat's clock may remain held waiting for an
/// animation ack, in seconds. Fed by engine ticks, not wall time -- this
/// constant is what the engine enforces; the server only publishes deadlines.
pub const ANIM_HOLD_CAP_S: f32 = 10.0;

static START: OnceLock<Instant> = OnceLock::new();

/// Monotonic server clock, milliseconds since process start.
///
/// Stamp T2 as late as possible (at receive) and T3 as early as possible
/// (just before send) so the round-trip sample is not inflated by handler
/// work. Deadlines are computed on this same clock.
pub fn server_time_ms() -> f64 {
    let start = START.get_or_init(Instant::now);
    start.elapsed().as_secs_f64() * 1000.0
}

/// Absolute turn-clock deadlines for the acting seat, on [`server_time_ms`].
///
/// `None` when there is no clock to show: solo, a machine seat, pre-play
/// phases, or a held (`clock_paused`) clock -- the UI then holds the display
/// at full rather than counting down.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockWire {
    /// When the acting seat's **base** pool runs out (the end of the whole
    /// clock). Bonus drains first, so this is the later of the two.
    pub base_deadline_ms: f64,
    /// When the acting seat's **bonus** pool runs out. Equal to
    /// `base_deadline_ms` when the bonus pool is empty.
    pub bonus_deadline_ms: f64,
}

/// Build the clock payload for one published view, from the engine's remaining
/// base/bonus seconds. Pure so the deadline math is unit-testable.
///
/// Drain order is **bonus first, then base** (`docs/SERVER.md` §Turn clock):
/// `bonus_deadline_ms = now + bonus`, `base_deadline_ms = now + bonus + base`.
pub fn clock_for_state(st: &MatchState, now_ms: f64) -> Option<ClockWire> {
    // Solo never expires a clock; machine seats are driven by the host.
    if st.mode == game_core::MatchMode::Solo as i32 {
        return None;
    }
    if st.phase != "play" || st.turn < 0 {
        return None;
    }
    let turn = st.turn as usize;
    let Some(cur) = st.players.get(turn) else {
        return None;
    };
    if cur.ai {
        return None;
    }
    // Held (anim ack pending): no deadline -- the client shows full and waits.
    if st.clock_paused {
        return None;
    }
    let base = st.base_time.max(0.0) as f64 * 1000.0;
    let bonus = st.bonus_time.max(0.0) as f64 * 1000.0;
    Some(ClockWire {
        base_deadline_ms: now_ms + bonus + base,
        bonus_deadline_ms: now_ms + bonus,
    })
}

/// One clock ping. `t1` is the client's `performance.now()` at send.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockPing {
    pub t1: f64,
}

/// The pong. `t2` is stamped at receive, `t3` just before this is returned.
/// Client-side: `rtt = (t4 - t1) - (t3 - t2)`,
/// `offset = ((t2 - t1) + (t3 - t4)) / 2` (server minus client).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockPong {
    pub t1: f64,
    pub t2: f64,
    pub t3: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_core::state::{MatchPlayer, MatchState};

    fn play_state() -> MatchState {
        let mut st = MatchState::default();
        st.mode = game_core::MatchMode::Casual as i32;
        st.phase = "play".into();
        st.turn = 0;
        st.base_time = 15.0;
        st.bonus_time = 10.0;
        let mut p = MatchPlayer::default();
        p.ai = false;
        st.players = vec![p];
        st
    }

    #[test]
    fn deadlines_are_absolute_on_the_server_clock() {
        // Bonus drains first: bonus expires at now+bonus, base (total) at
        // now+bonus+base.
        let st = play_state();
        let c = clock_for_state(&st, 1_000.0).expect("clock");
        assert_eq!(c.bonus_deadline_ms, 1_000.0 + 10_000.0);
        assert_eq!(c.base_deadline_ms, 1_000.0 + 25_000.0);
    }

    #[test]
    fn zero_bonus_leaves_only_the_base_phase() {
        // Bonus drains first: an empty bonus pool puts `bonus_deadline_ms` at
        // "now" (already spent), and the base deadline is the end of the clock.
        let mut st = play_state();
        st.bonus_time = 0.0;
        let c = clock_for_state(&st, 500.0).expect("clock");
        assert_eq!(c.bonus_deadline_ms, 500.0);
        assert_eq!(c.base_deadline_ms, 500.0 + 15_000.0);
    }

    #[test]
    fn solo_ai_and_paused_publish_no_deadline() {
        let mut st = play_state();
        st.mode = game_core::MatchMode::Solo as i32;
        assert!(clock_for_state(&st, 0.0).is_none());

        let mut st = play_state();
        st.players[0].ai = true;
        assert!(clock_for_state(&st, 0.0).is_none());

        let mut st = play_state();
        st.clock_paused = true;
        assert!(clock_for_state(&st, 0.0).is_none());
    }

    #[test]
    fn pre_play_phases_have_no_turn_clock() {
        let mut st = play_state();
        st.phase = "ban".into();
        assert!(clock_for_state(&st, 0.0).is_none());
    }
}
