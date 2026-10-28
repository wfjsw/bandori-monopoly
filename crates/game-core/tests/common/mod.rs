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

/// Put every non-advanced bot seat back on the engine's own schedule.
///
/// Production leaves `ai` off for bot seats and drives them over the human
/// interface (`docs/BOT-DRIVE.md`); these engine tests have no driver, so they
/// opt back into the in-engine answerer. Call between `Match::new` and
/// `quick_start` (`world_mut` needs no pending routine).
pub fn engine_bots(m: &mut game_core::engine::Match) {
    let bots: Vec<usize> = m
        .state()
        .players
        .iter()
        .enumerate()
        .filter(|(_, p)| p.bot && p.mentality != game_core::state::BotMentality::Advanced)
        .map(|(i, _)| i)
        .collect();
    for i in bots {
        m.world_mut().st.players[i].ai = true;
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
