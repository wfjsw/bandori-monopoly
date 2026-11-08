//! The persona crossing itself: a face a body writes before a pause lands on
//! live, and a second write of the same drive (even back to the pass-start
//! face) crosses again against the value it replaces.

use game_core::data::GameData;
use game_core::engine::{Match, PassBase, PlayCtx, StubRules, World};
use game_core::net::RoomMember;
use game_core::scoring::ScoreWeights;
use game_core::MatchMode;
use std::sync::Arc;
mod common;
use common::data_arc;

fn setup() -> (Arc<GameData>, World) {
    let data = data_arc();
    let members = (1..=2)
        .map(|id| RoomMember {
            id,
            player: format!("P{id}"),
            bot: true,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let mut m = Match::new(
        data.clone(),
        Arc::new(StubRules),
        &members,
        7,
        MatchMode::Solo,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut w = m.world().clone();
    w.st.phase = "play".into();
    for p in &mut w.st.players {
        p.ai = false;
        p.bot = false;
        p.persona = "A".into();
    }
    (data, w)
}

fn base(face: &str) -> PassBase {
    PassBase {
        persona: vec![face.into(); 2],
        ..Default::default()
    }
}

#[test]
fn a_written_face_lands_and_the_second_write_rebases_on_it() {
    let (data, w) = setup();
    let rules = StubRules;
    let mut cx = PlayCtx::new(w.clone(), &data, &rules, &[]);
    let b = base("A");

    // A write crosses; the row's base is the face it replaced.
    let mut guest = w;
    guest.st.players[0].persona = "B".into();
    let rows = cx.capture_guest_persona_from(&b, &guest);
    assert_eq!(rows, vec![(0, "A".to_string(), "B".to_string())]);
    assert_eq!(cx.world_copy().st.players[0].persona, "B");
    assert_eq!(cx.world_copy().st.players[1].persona, "A");

    // The same drive writes the pass-start face back. The copy now equals the
    // pass base, but live holds "B", so the write must still cross -- and the
    // row records "B" as what it replaced.
    let mut guest = cx.world_copy();
    guest.st.players[0].persona = "A".into();
    let rows = cx.capture_guest_persona_from(&b, &guest);
    assert_eq!(rows, vec![(0, "B".to_string(), "A".to_string())]);
    assert_eq!(cx.world_copy().st.players[0].persona, "A");

    // No write, no row, nothing touched.
    let guest = cx.world_copy();
    assert!(cx.capture_guest_persona_from(&b, &guest).is_empty());
    assert_eq!(cx.world_copy().st.players[0].persona, "A");
}

#[test]
fn a_row_reverts_to_the_face_it_replaced() {
    let (data, w) = setup();
    let rules = StubRules;
    let mut cx = PlayCtx::new(w.clone(), &data, &rules, &[]);
    let mut guest = w;
    guest.st.players[0].persona = "B".into();
    let row = cx.capture_guest_persona_from(&base("A"), &guest);
    let mut live = cx.world_copy();
    let overlay = game_core::engine::GuestOverlay {
        persona: row,
        ..Default::default()
    };
    overlay.restore(&mut live);
    assert_eq!(live.st.players[0].persona, "A");
    assert_eq!(live.st.players[1].persona, "A");
}
