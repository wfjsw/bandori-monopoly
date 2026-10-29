//! Shared test harness: [`GameData`] from `data/` with a synthetic roster
//! fixture.
//!
//! `game-core` must not depend on the card crates, so tests cannot ask a
//! ruleset for the roster. The checked-in fixture
//! (`crates/game-core/tests/fixtures/roster.json`) is a tiny engine-generic
//! roster (Alpha/Beta/... in GroupA/GroupB) loaded via [`Roster::from_fixture`]
//! so the crate never touches the card crates or a shipped ruleset roster.
#![allow(dead_code)] // each test binary uses a subset of the helpers

use std::path::Path;
use std::sync::Arc;

use game_core::data::{GameData, Roster};

pub fn data_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// `GameData` from `data/`, roster from the synthetic fixture.
pub fn data() -> GameData {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let dir = root.join("../../data");
    let roster = Roster::from_fixture(
        &std::fs::read_to_string(root.join("tests/fixtures/roster.json")).expect("roster fixture"),
    )
    .expect("roster fixture parses");
    GameData::load(
        |f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()),
        roster,
    )
    .expect("data/ should load -- run tools/asset-pipe/extract.py")
}

/// [`data`], wrapped for the match engine.
pub fn data_arc() -> Arc<GameData> {
    Arc::new(data())
}

/// The policy command for one machine-driven seat, if it has anything to
/// decide.
///
/// Production drives every bot seat over the human interface
/// (`docs/BOT-DRIVE.md`) and the engine decides nothing itself; a test host is
/// a driver the same way, and follows the same list the server claims by
/// (`state.drive`).
pub fn bot_msg(m: &game_core::engine::Match, member: i32) -> Option<game_core::net::NetMessage> {
    let st = m.state();
    let me = st.player_of(member);
    let asked = st.prompt.id > 0 && st.prompt.waiting(me);
    // Off its turn and unasked, the seat has nothing to do. (Setup surfaces
    // other than a prompt are engine-side.)
    if !asked && (st.phase != "play" || st.busy || st.vote.id > 0 || st.turn != me) {
        return None;
    }
    bot_core::policy::act_for(m, member)
}

/// The members a harness must drive now: the engine's drive list, except in
/// Solo, where the list is empty by design (one client drives the whole
/// table) and the harness plays the bots itself.
pub fn drive_members(m: &game_core::engine::Match) -> Vec<i32> {
    let st = m.state();
    if !st.drive.is_empty() {
        return st.drive.clone();
    }
    st.players
        .iter()
        .filter(|p| p.bot && !p.bankrupt && !p.left)
        .map(|p| p.member)
        .collect()
}

/// Let every member of [`drive_members`] take one policy command. Returns
/// whether any applied.
pub fn drive(m: &mut game_core::engine::Match) -> bool {
    let members = drive_members(m);
    let mut any = false;
    for member in members {
        let Some(msg) = bot_msg(m, member) else {
            continue;
        };
        if m.act(member, &msg).is_ok() {
            any = true;
        }
    }
    any
}

/// `Match::tick` plus [`drive`] -- one clock quantum, then the bots' answers,
/// the cadence a host runs at.
pub trait Pump {
    fn pump_tick(&mut self, dt: f32);
}

impl Pump for game_core::engine::Match {
    fn pump_tick(&mut self, dt: f32) {
        self.tick(dt);
        drive(self);
    }
}

/// Answer the opening-hand re-roll, if it is up, for every seat.
///
/// 初始重抽 is a whole-table barrier with no advance until every seat answers
/// (rulebook 时点流程 step 6), so a harness that only ticks stalls in the
/// opening. Every seat takes the fallback -- what an idle player would do.
/// Tolerant of a seat that already answered.
pub fn cross_mulligan(m: &mut game_core::engine::Match) {
    let st = m.state();
    if st.prompt.kind != "mulligan" {
        return;
    }
    let members: Vec<i32> = st.players.iter().map(|p| p.member).collect();
    let (id, fallback) = (st.prompt.id, st.prompt.fallback);
    for member in members {
        let _ = m.act(
            member,
            &game_core::net::NetMessage {
                prompt: id,
                value: fallback,
                ..game_core::net::NetMessage::act("answer")
            },
        );
    }
}
