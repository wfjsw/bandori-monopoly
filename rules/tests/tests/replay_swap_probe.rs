//! Temporary probe: does the Two-in-One character swap survive record/replay?

mod common;
use common::*;

use game_core::engine::SAVE_VERSION;
use game_core::net::NetMessage;
use game_core::record::{seal, EngineStamp, RecordedMatch, Replayer, RECORD_VERSION};

fn stamp() -> EngineStamp {
    EngineStamp {
        format: RECORD_VERSION,
        save_version: SAVE_VERSION,
        abi: 119,
        ruleset_sha256: "cf6d9d31fb2ce03baa7797fd84b0d37f3d2cb009db9befcb8bba98782c5c70f3".into(),
        data_sha256: String::new(),
        engine: "game-core".into(),
        build: "0.1.0".into(),
        bundle: "b7111ec9c80d51098ec775be99dc6bffd0cf63a378e734280a8c2dce0bc57be8".into(),
    }
}

#[test]
fn counter_swap_survives_replay() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "纯田真奈");
    t.give(0, &["Sumimi:Sumimi是二人一体的"]);
    t.dice(&[1]);
    t.roll(0).unwrap();
    let p = t.prompt().expect("counteract window open");
    let k = t.option("Sumimi:Sumimi是二人一体的").expect("offered");
    let before = t.p(0).persona.clone();
    let mut rm = RecordedMatch::from_snapshot(t.m);
    rm.act(
        1,
        &NetMessage {
            prompt: p.id,
            value: k,
            ..NetMessage::act("answer")
        },
    )
    .expect("counteract");
    let after = rm.inner().state().players[0].persona.clone();
    eprintln!("live counter: {before:?} -> {after:?}");
    assert_ne!(before, after, "live counter swap happened");

    let mut rec = rm.export(stamp(), "test");
    seal(&mut rec);
    if let Ok(out) = std::env::var("OUT") {
        std::fs::write(&out, game_core::record::encode_record_zst(&rec)).unwrap();
        eprintln!("wrote {out}");
    }
    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let mut seen = vec![];
    while !rp.status().ended {
        let pos_before = rp.match_ref().state().players[0].persona.clone();
        rp.next_input();
        let pos_after = rp.match_ref().state().players[0].persona.clone();
        if pos_after != pos_before {
            seen.push((pos_before, pos_after));
        }
    }
    eprintln!("replay counter persona changes: {seen:?}");
    assert!(!seen.is_empty(), "replay reproduced the counter swap");
}

#[test]
fn seek_keeps_the_swap() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "纯田真奈");
    t.give(0, &["Sumimi:Sumimi是二人一体的"]);
    let mut rm = RecordedMatch::from_snapshot(t.m);
    rm.act(
        1,
        &NetMessage {
            card: "Sumimi:Sumimi是二人一体的".into(),
            ..NetMessage::act("play")
        },
    )
    .expect("play");
    // A few ticks after the swap.
    for _ in 0..40 {
        rm.tick_steps(1);
    }
    let mut rec = rm.export(stamp(), "test");
    seal(&mut rec);

    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let total = rp.total_ticks();
    let mut swap_tick = u64::MAX;
    while !rp.status().ended && rp.status().tick < total {
        let b = rp.match_ref().state().players[0].persona.clone();
        rp.next_input();
        let a = rp.match_ref().state().players[0].persona.clone();
        if a != b && swap_tick == u64::MAX {
            swap_tick = rp.status().tick;
        }
    }
    eprintln!("swap at tick {swap_tick}, total {total}");
    assert_ne!(swap_tick, u64::MAX);
    // Lay the whole keyframe index, then scrub to just after the swap.
    for _ in 0..10_000 {
        if rp.index(64).done {
            break;
        }
    }
    let st = rp.seek(swap_tick + 2).expect("seek");
    eprintln!(
        "seek landed at {} persona={:?}",
        st.tick,
        rp.match_ref().state().players[0].persona
    );
    assert_eq!(
        rp.match_ref().state().players[0].persona,
        "三角初华（Sumimi）",
        "the swapped persona survives a seek"
    );
    // Scrub back before the swap and forward again.
    rp.seek(0).expect("seek back");
    while rp.status().tick <= swap_tick + 2 && !rp.status().ended {
        rp.next_input();
    }
    assert_eq!(
        rp.match_ref().state().players[0].persona,
        "三角初华（Sumimi）",
        "the swap is re-simulated after scrubbing back"
    );
}

#[test]
fn play_swap_survives_replay() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "纯田真奈");
    t.give(0, &["Sumimi:Sumimi是二人一体的"]);
    let before = t.p(0).persona.clone();
    let mut rm = RecordedMatch::from_snapshot(t.m);
    rm.act(
        1,
        &NetMessage {
            card: "Sumimi:Sumimi是二人一体的".into(),
            ..NetMessage::act("play")
        },
    )
    .expect("play");
    let after = rm.inner().state().players[0].persona.clone();
    eprintln!("live: {before:?} -> {after:?}");
    assert_ne!(before, after, "live swap happened");

    let mut rec = rm.export(stamp(), "test");
    seal(&mut rec);
    if let Ok(out) = std::env::var("OUT") {
        std::fs::write(&out, game_core::record::encode_record_zst(&rec)).unwrap();
        eprintln!("wrote {out}");
    }
    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let mut seen = vec![];
    while !rp.status().ended {
        let pos_before = rp.match_ref().state().players[0].persona.clone();
        rp.next_input();
        let pos_after = rp.match_ref().state().players[0].persona.clone();
        if pos_after != pos_before {
            seen.push((pos_before, pos_after));
        }
    }
    eprintln!("replay persona changes: {seen:?}");
    assert!(!seen.is_empty(), "replay reproduced the swap");
}

#[test]
fn print_dev_bundle() {
    let glue: String = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../webui/src/wasm/engine_id.json"
        ))
        .unwrap(),
    )
    .unwrap()["glueSha256"]
        .as_str()
        .unwrap()
        .into();
    let idx: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../dist/cards/index.json"
        ))
        .unwrap(),
    )
    .unwrap();
    let s = EngineStamp {
        format: RECORD_VERSION,
        save_version: SAVE_VERSION,
        abi: idx["abi"].as_u64().unwrap() as u32,
        ruleset_sha256: idx["ruleset_sha256"].as_str().unwrap().into(),
        data_sha256: String::new(),
        engine: "game-core".into(),
        build: "0.1.0".into(),
        bundle: String::new(),
    };
    eprintln!("glue={glue}");
    eprintln!("stamp={s:?}");
    eprintln!("bundle={}", game_core::record::bundle_id(&glue, &s));
}
