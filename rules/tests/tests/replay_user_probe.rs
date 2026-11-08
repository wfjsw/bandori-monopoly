//! Ad-hoc probe over a user-supplied `.bdrec` (`REC=<path>`): dump it, seek
//! it, or inspect its replay stream. All three are `#[ignore]`d -- run them
//! with `REC=<path> [DUMP=<path>] cargo test -p ruleset-tests --test
//! replay_user_probe -- --ignored`.

mod common;
use common::*;

use game_core::record::{decode_record, RecordFile, Replayer};

/// A record only has to replay without divergence on the build that wrote it;
/// these probes run the working tree, which usually moved on. Say which build
/// it is instead and only assert when they match.
fn assert_clean_on_own_build(rec: &RecordFile, rp: &Replayer) {
    let here = dev_stamp().bundle;
    if rec.header.engine.bundle != here {
        eprintln!(
            "note: record build {} != this build {here} -- divergence not asserted",
            rec.header.engine.bundle
        );
        return;
    }
    assert!(
        !rp.status().diverged,
        "replay diverged at {:?}",
        rp.first_mismatch()
    );
}

#[test]
#[ignore = "ad-hoc: set REC and run --ignored"]
fn dump_user_record() {
    let Ok(path) = std::env::var("REC") else {
        eprintln!("skipped: REC unset");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let rec = decode_record(&bytes).expect("decode");
    let dump = std::env::var("DUMP").unwrap_or_else(|_| format!("{path}.dump"));
    let mut out = String::new();
    for (i, input) in rec.body.inputs.iter().enumerate() {
        out.push_str(&format!("{i}\t{}\n", serde_json::to_string(input).unwrap()));
    }
    std::fs::write(&dump, out).unwrap();
    if let Some(ev) = &rec.body.events {
        let s: Vec<String> = ev
            .iter()
            .map(|e| format!("{}\t{}", e.id, serde_json::to_string(e).unwrap()))
            .collect();
        std::fs::write(dump + ".events", s.join("\n")).unwrap();
        eprintln!("event log: {}", ev.len());
    } else {
        eprintln!("no event log in record");
    }
    eprintln!("checkpoints: {}", rec.body.checkpoints.len());
}

#[test]
#[ignore = "ad-hoc: set REC and run --ignored"]
fn seek_user_record() {
    let Ok(path) = std::env::var("REC") else {
        eprintln!("skipped: REC unset");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let rec = decode_record(&bytes).expect("decode");
    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let total = rp.total_ticks();
    eprintln!("total {total}");
    // Lay the whole keyframe index.
    let mut n = 0;
    loop {
        let st = rp.index(64);
        n += 1;
        if st.done {
            eprintln!("index done after {n} chunks, keys={:?}", st);
            break;
        }
        if n > 100_000 {
            eprintln!("index never finished");
            break;
        }
    }
    let mut landed = 0;
    for tick in [2357u64, 11787, 21217, 23338, 0] {
        let r = rp.seek(tick);
        if r.is_ok() {
            landed += 1;
        }
        let st = rp.match_ref().state();
        eprintln!(
            "seek {tick} -> {:?} players={} personas={:?} round={} turn={} ended={} diverged={}",
            r.as_ref().ok().map(|s| s.tick),
            st.players.len(),
            st.players
                .iter()
                .map(|p| p.persona.clone())
                .collect::<Vec<_>>(),
            st.round,
            st.turn,
            rp.status().ended,
            rp.status().diverged
        );
    }
    assert!(landed > 0, "no seek landed");
    assert_clean_on_own_build(&rec, &rp);
}

#[test]
#[ignore = "ad-hoc: set REC and run --ignored"]
fn inspect_user_record() {
    let Ok(path) = std::env::var("REC") else {
        eprintln!("skipped: REC unset");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let rec = match decode_record(&bytes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("DECODE FAILED: {e}");
            panic!("decode");
        }
    };
    let h = &rec.header;
    eprintln!(
        "created={:?} bundle={:?} mode={:?} ended={} partial={:?} rounds={} ticks={} inputs={}",
        h.created,
        h.engine.bundle,
        h.mode,
        h.ended,
        h.partial,
        h.rounds,
        h.total_ticks,
        rec.body.inputs.len()
    );
    eprintln!("stamp={:?}", h.engine);
    eprintln!(
        "init={:?}",
        match &rec.body.init {
            game_core::record::Init::Seed(_) => "seed".to_string(),
            game_core::record::Init::Snapshot { .. } => "snapshot".to_string(),
        }
    );
    for s in &h.seats {
        eprintln!(
            "seat m{} player={:?} bot={} persona={:?}",
            s.member, s.player, s.bot, s.persona
        );
    }
    let card = "Sumimi:Sumimi是二人一体的";
    let mut hits = 0;
    for (i, input) in rec.body.inputs.iter().enumerate() {
        let line = serde_json::to_string(input).unwrap_or_default();
        if line.contains(card) {
            hits += 1;
            if hits <= 8 {
                eprintln!("CARD INPUT {i}: {}", &line[..line.len().min(300)]);
            }
        }
    }
    eprintln!("card inputs: {hits}");
    // Cheap token/persona sweep: replay and record changes.
    let mut rp = Replayer::from_body(data(), fresh_rules(), rec.header.clone(), rec.body.clone())
        .expect("open");
    let mut step = 0u64;
    let mut swaps = 0;
    while !rp.status().ended {
        let before: Vec<String> = rp
            .match_ref()
            .state()
            .players
            .iter()
            .map(|p| p.persona.clone())
            .collect();
        let toks_before: Vec<usize> = rp
            .match_ref()
            .state()
            .players
            .iter()
            .map(|p| p.tokens.len())
            .collect();
        rp.next_input();
        step += 1;
        let after: Vec<String> = rp
            .match_ref()
            .state()
            .players
            .iter()
            .map(|p| p.persona.clone())
            .collect();
        let toks_after: Vec<usize> = rp
            .match_ref()
            .state()
            .players
            .iter()
            .map(|p| p.tokens.len())
            .collect();
        if before != after {
            swaps += 1;
            if swaps <= 20 {
                eprintln!("PERSONA CHANGE at input {step}: {before:?} -> {after:?}");
            }
        }
        if toks_before != toks_after && swaps <= 20 {
            eprintln!("TOKENS at input {step}: {toks_before:?} -> {toks_after:?}");
        }
        if rp.status().diverged {
            eprintln!("DIVERGED at input {step}");
            break;
        }
    }
    eprintln!(
        "END persona swaps={swaps} diverged={} first_mismatch={:?}",
        rp.status().diverged,
        rp.first_mismatch()
    );
    assert_clean_on_own_build(&rec, &rp);
}
