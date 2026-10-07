//! `.bdrec` codec tests (`docs/REPLAY.md` "Format"): the zstd framing that
//! storage and download use, the legacy gzip / plain-JSON framings the loader
//! still accepts, and the size comparison the choice rests on.
//!
//! The encode path is `game_core::record::encode_record_zst`; the decode path
//! is the shared `decode_record` / `parse_header_bytes`, which sniff the magic
//! and expand any of the three framings.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, StubRules, SAVE_VERSION};
use game_core::net::RoomMember;
use game_core::record::{
    decode_record, encode_record_gz, encode_record_json, encode_record_zst, expand_record,
    is_gzip, is_zstd, parse_header_bytes, parse_record, sniff_record, EngineStamp, MatchSetup,
    RecordedMatch, ReplayError, RecordEncoding, RECORD_VERSION,
};
use game_core::scoring::ScoreWeights;
use game_core::state::BotMentality;
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

fn rules() -> Arc<dyn CardRules> {
    Arc::new(StubRules)
}

fn stamp() -> EngineStamp {
    EngineStamp {
        format: RECORD_VERSION,
        save_version: SAVE_VERSION,
        abi: 35,
        ruleset_sha256: "stub".into(),
        data_sha256: "data-test".into(),
        engine: "game-core".into(),
        build: "test".into(),
    }
}

/// Play an all-bot match to `max_rounds` and export it.
fn run_bot_game(seed: u64, n: i32, max_rounds: i32) -> game_core::record::RecordFile {
    let members = (1..=n)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    let setup = MatchSetup {
        members,
        seed,
        weights: ScoreWeights::default(),
    };
    let mut rm = RecordedMatch::new(data(), rules(), setup, MatchMode::Casual);
    rm.quick_start();
    let mut steps = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(3);
        steps += 1;
        assert!(steps < 2_000_000, "seed {seed}: game did not progress");
        if rm.inner().state().round > max_rounds {
            rm.finish();
        }
    }
    rm.export(stamp(), "2026-10-07 00:00")
}

// ---------------------------------------------------------------- round trip

#[test]
fn round_trip_zstd_gzip_and_json() {
    let file = run_bot_game(7, 4, 4);
    let json = encode_record_json(&file);
    let zst = encode_record_zst(&file);
    let gz = encode_record_gz(&file);

    for (label, bytes) in [
        ("zstd", zst.as_slice()),
        ("gzip", gz.as_slice()),
        ("json", json.as_bytes()),
    ] {
        let back = decode_record(bytes).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(back, file, "{label}: record round trip");
        let header = parse_header_bytes(bytes).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(header, file.header, "{label}: header round trip");
    }
}

#[test]
fn sniffing_sees_zstd_gzip_and_json() {
    let file = run_bot_game(11, 3, 2);
    let json = encode_record_json(&file);
    let zst = encode_record_zst(&file);
    let gz = encode_record_gz(&file);

    assert_eq!(sniff_record(&zst), RecordEncoding::Zstd);
    assert_eq!(sniff_record(&gz), RecordEncoding::Gzip);
    assert_eq!(sniff_record(json.as_bytes()), RecordEncoding::Json);
    assert!(is_zstd(&zst) && !is_gzip(&zst));
    assert!(is_gzip(&gz) && !is_zstd(&gz));
    assert!(!is_zstd(json.as_bytes()) && !is_gzip(json.as_bytes()));
    // The magics themselves, spelled out: `28 B5 2F FD` and `1F 8B`.
    assert_eq!(&zst[..4], &[0x28, 0xB5, 0x2F, 0xFD], "zstd magic");
    assert_eq!(&gz[..2], &[0x1F, 0x8B], "gzip magic");
}

#[test]
fn expand_record_yields_the_same_json_from_every_framing() {
    let file = run_bot_game(3, 2, 2);
    let json = encode_record_json(&file);
    let want = json.as_bytes();
    assert_eq!(expand_record(encode_record_zst(&file).as_slice()).unwrap(), want);
    assert_eq!(expand_record(encode_record_gz(&file).as_slice()).unwrap(), want);
    assert_eq!(expand_record(want).unwrap(), want);
}

#[test]
fn zstd_frames_are_standard() {
    // The claim the format makes: any zstd decoder reads what we write. The
    // `zstd` crate is the reference binding, so it is the strictest witness
    // available without leaving the process. Both encoders -- the reference
    // binding (what `encode_record_zst` emits on native) and the pure-Rust
    // browser encoder (`zst_encode_pure`, what wasm32 emits) -- are checked
    // against it, in both directions.
    let file = run_bot_game(5, 3, 2);
    let json = encode_record_json(&file).into_bytes();
    for (label, zst) in [
        ("reference / native", encode_record_zst(&file)),
        ("structured-zstd / browser", game_core::record::zst_encode_pure(&json)),
    ] {
        let back = zstd::stream::decode_all(&zst[..])
            .unwrap_or_else(|e| panic!("reference zstd decodes {label} frame: {e}"));
        assert_eq!(back, json, "{label}: reference reads our frame");
        // ... and the other way: a reference frame decodes with ours.
        let ref_zst = zstd::stream::encode_all(&json[..], 3).expect("reference zstd encodes");
        assert_eq!(
            expand_record(&ref_zst).unwrap(),
            json,
            "{label}: our decoder reads a reference frame"
        );
    }
}

#[test]
fn corrupt_input_is_corrupt_not_format() {
    let file = run_bot_game(13, 2, 2);
    let zst = encode_record_zst(&file);
    // A frame cut in half: the decoder cannot finish it.
    let half = &zst[..zst.len() / 2];
    assert!(
        matches!(decode_record(half), Err(ReplayError::Corrupt(_))),
        "truncated frame: {:?}",
        decode_record(half).map(|_| ())
    );
    // A zstd magic with garbage behind it is not "format" either -- it decodes
    // to nothing parseable, so the record is corrupt.
    let mut fake = vec![0x28, 0xB5, 0x2F, 0xFD, 0, 0, 0];
    fake.extend_from_slice(b"not a frame");
    assert!(decode_record(&fake).is_err(), "garbage behind the magic");
    // Plain JSON that is not a record: Format (magic) or Corrupt (body).
    let err = parse_record("{}").unwrap_err();
    assert!(matches!(err, ReplayError::Corrupt(_) | ReplayError::Format(_)));
}

// ---------------------------------------------------------------- sizes

/// The number the format choice rests on: a real 200-round bot game as raw
/// JSON, as zstd (what we store and download) and as gzip (what the browser
/// used to write). Also measures the events-bundled form, which is the bigger
/// download.
#[test]
fn size_comparison_on_a_real_200_round_bot_game() {
    let members = (1..=4)
        .map(|i| RoomMember {
            id: i,
            player: format!("P{i}"),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    let mut rm = RecordedMatch::new(
        data(),
        rules(),
        MatchSetup {
            members,
            seed: 0xB07,
            weights: ScoreWeights::default(),
        },
        MatchMode::Casual,
    );
    rm.quick_start();
    while !rm.inner().ended() {
        rm.tick_steps(3);
        if rm.inner().state().round > 200 {
            rm.finish();
        }
    }
    let file = rm.export(stamp(), "2026-10-07 00:00");
    report_sizes("200-round input log", &file);
    // The same match with the public event log bundled: bigger, and the shape
    // the post-match download takes.
    let bundled = rm
        .export_with_events(data(), rules(), stamp(), "2026-10-07 00:00")
        .unwrap();
    report_sizes("200-round input log + events", &bundled);
}

/// Print raw / native-seal / browser / ref-3 / gzip sizes for one record,
/// plus a native encode-time proxy for the browser encoder.
fn report_sizes(label: &str, file: &game_core::record::RecordFile) {
    use std::time::Instant;
    let json = encode_record_json(&file).into_bytes();
    let shipped = encode_record_zst(&file);
    // What wasm32 writes: the pure-Rust `structured-zstd` level-1 encoder.
    let browser = game_core::record::zst_encode_pure(&json);
    let ref1 = zstd::stream::encode_all(&json[..], 1).unwrap();
    let ref3 = zstd::stream::encode_all(&json[..], 3).unwrap();
    let gz = encode_record_gz(file);
    let ratio = |n: usize| json.len() as f64 / n as f64;
    // Native encode time as a proxy for the browser (3 runs, best of).
    let ms = |f: &dyn Fn(&[u8]) -> Vec<u8>| {
        let t = Instant::now();
        for _ in 0..3 {
            let _ = f(&json);
        }
        t.elapsed().as_secs_f64() * 1000.0 / 3.0
    };
    let t_browser = ms(&|b| game_core::record::zst_encode_pure(b));
    let t_ref = ms(&|b| zstd::stream::encode_all(b, 1).unwrap());
    eprintln!(
        "{label}: rounds {} ticks {} inputs {}\n  json          {:>8} bytes\n  zstd shipped  {:>8} bytes ({:.2}x)\n  zstd browser  {:>8} bytes ({:.2}x)   encode {:>6.2} ms (ref-1 {:>6.2} ms)\n  zstd ref-1    {:>8} bytes ({:.2}x)\n  zstd ref-3    {:>8} bytes ({:.2}x)\n  gzip          {:>8} bytes ({:.2}x)",
        file.header.rounds,
        file.header.total_ticks,
        file.body.inputs.len(),
        json.len(),
        shipped.len(),
        ratio(shipped.len()),
        browser.len(),
        ratio(browser.len()),
        t_browser,
        t_ref,
        ref1.len(),
        ratio(ref1.len()),
        ref3.len(),
        ratio(ref3.len()),
        gz.len(),
        ratio(gz.len()),
    );
    assert!(shipped.len() < json.len() / 3, "{label}: zstd well under a third of the JSON");
    // The browser encoder must stay in the reference's league -- that is the
    // whole point of swapping `ruzstd` out for `structured-zstd` -- and must
    // clearly beat the gzip it replaced.
    assert!(
        browser.len() < gz.len(),
        "{label}: browser zstd ({}) beats gzip ({})",
        browser.len(),
        gz.len()
    );
    assert!(
        browser.len() <= ref1.len() * 11 / 10,
        "{label}: browser zstd ({}) within 10% of the reference level 1 ({})",
        browser.len(),
        ref1.len()
    );
}