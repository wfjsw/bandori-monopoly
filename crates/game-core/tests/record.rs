//! Match record / replay tests (`docs/REPLAY.md` §7).
//!
//! The contract is a round trip: what the recorder writes, the replayer must
//! rebuild **byte for byte** -- same `save()` blob, every checkpoint hash, the
//! same event stream, and the same Ok/Err on every recorded act.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{Match, StubRules, SAVE_VERSION};
use game_core::net::{NetMessage, RoomMember};
use game_core::record::{
    body_check, compat, hash_save, parse_header, parse_record, seal, EngineStamp, Init, Input,
    MatchSetup, Origin, RecordedMatch, ReplayError, Replayer, RECORD_VERSION, STEP,
};
use game_core::rng::Rng;
use game_core::scoring::ScoreWeights;
use game_core::state::{BotMentality, MatchEvent};
use game_core::MatchMode;

fn data() -> Arc<GameData> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    Arc::new(
        GameData::load(|f| std::fs::read_to_string(dir.join(f)).map_err(|e| e.to_string()))
            .unwrap(),
    )
}

fn rules() -> Arc<dyn game_core::engine::CardRules> {
    Arc::new(StubRules)
}

fn member(id: i32, bot: bool, mentality: BotMentality) -> RoomMember {
    RoomMember {
        id,
        player: format!("P{id}"),
        bot,
        mentality,
        ..Default::default()
    }
}

/// A full stamp as web-glue would build it.
fn stamp() -> EngineStamp {
    EngineStamp {
        format: RECORD_VERSION,
        save_version: SAVE_VERSION,
        abi: 35,
        ruleset_sha256: "stub".into(),
        data_sha256: "data-test".into(),
        engine: "game-core".into(),
        build: "test".into(),
        bundle: String::new(),
    }
}

fn setup(members: Vec<RoomMember>, seed: u64) -> MatchSetup {
    MatchSetup {
        members,
        seed256: None,
        seed,
        weights: ScoreWeights::default(),
    }
}

/// Pull the events the engine has produced so far into `out`.
fn drain(m: &Match, last_id: &mut i32, out: &mut Vec<MatchEvent>) {
    let new = m.events_since(*last_id);
    if let Some(e) = new.last() {
        *last_id = e.id;
        out.extend(new);
    }
}

/// Play an all-bot match to the end, recording every call. `k_of` picks the
/// tick quantum each step (1..=10), which is exactly what the driver does.
fn run_bot_game(
    seed: u64,
    n: i32,
    mentality: BotMentality,
    k_of: &mut dyn FnMut(&mut Rng) -> u8,
    max_rounds: i32,
) -> (game_core::record::RecordFile, Vec<MatchEvent>) {
    let members = (1..=n).map(|i| member(i, true, mentality)).collect();
    let mut rm = RecordedMatch::new(data(), rules(), setup(members, seed), MatchMode::Casual);
    rm.quick_start();
    let mut rng = Rng::new(seed ^ 0xD1CE);
    let mut events = Vec::new();
    let mut last_id = 0;
    drain(rm.inner(), &mut last_id, &mut events);
    let mut steps = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(k_of(&mut rng));
        drain(rm.inner(), &mut last_id, &mut events);
        steps += 1;
        assert!(steps < 300_000, "seed {seed}: game did not progress");
        if rm.inner().state().round > max_rounds {
            rm.finish();
            drain(rm.inner(), &mut last_id, &mut events);
        }
    }
    (rm.export(stamp(), "2026-10-07 00:00"), events)
}

/// Replay a record to the end and assert the full round trip: byte-equal
/// `save()`, every checkpoint matched, identical events, matching final hash.
fn assert_round_trip(
    file: &game_core::record::RecordFile,
    want_events: &[MatchEvent],
    label: &str,
) -> String {
    let json = serde_json::to_string(file).unwrap();
    let parsed = parse_record(&json).unwrap_or_else(|e| panic!("{label}: {e}"));
    assert_eq!(parsed.header.total_ticks, file.header.total_ticks, "{label}");
    let mut rp = Replayer::new(data(), rules(), &parsed, false)
        .unwrap_or_else(|e| panic!("{label}: load: {e}"));
    let mut guard = 0u32;
    while !rp.status().ended {
        let st = rp.step_ticks(8);
        assert!(!st.diverged, "{label}: diverged at tick {}", st.tick);
        guard += 1;
        assert!(guard < 500_000, "{label}: replay stuck");
    }
    let save = rp.match_ref().save();
    assert_eq!(hash_save(&save), file.body.final_hash, "{label}: final hash");
    assert_eq!(
        rp.status().tick,
        file.header.total_ticks,
        "{label}: tick count"
    );
    assert_eq!(rp.events(), want_events, "{label}: event stream");
    save
}

// ---------------------------------------------------------------- round trip

/// Seeds 0..64, 2..=6 players, standard / chaos / mixed bots, random k.
#[test]
fn round_trip_across_seeds_players_and_mentalities() {
    for seed in 0..64u64 {
        let n = 2 + (seed % 5) as i32; // 2..=6
        let mentality = match seed % 3 {
            0 => BotMentality::Standard,
            1 => BotMentality::Chaos,
            _ => BotMentality::Standard, // mixed: half and half below
        };
        let members: Vec<RoomMember> = (1..=n)
            .map(|i| {
                let m = if seed % 3 == 2 && i % 2 == 0 {
                    BotMentality::Chaos
                } else {
                    mentality
                };
                member(i, true, m)
            })
            .collect();
        let mut rm = RecordedMatch::new(
            data(),
            rules(),
            setup(members, seed),
            MatchMode::Casual,
        );
        rm.quick_start();
        let mut rng = Rng::new(seed ^ 0xD1CE);
        let mut events = Vec::new();
        let mut last_id = 0;
        drain(rm.inner(), &mut last_id, &mut events);
        let mut steps = 0u32;
        while !rm.inner().ended() {
            let k = 1 + rng.below(10) as u8;
            rm.tick_steps(k);
            drain(rm.inner(), &mut last_id, &mut events);
            steps += 1;
            assert!(steps < 300_000, "seed {seed}: game did not progress");
            if rm.inner().state().round > 12 {
                rm.finish();
                drain(rm.inner(), &mut last_id, &mut events);
            }
        }
        let file = rm.export(stamp(), "2026-10-07 00:00");
        assert!(file.header.ended, "seed {seed}");
        assert!(!file.header.partial, "seed {seed}");
        assert_eq!(file.header.engine, stamp(), "seed {seed}");
        let save = assert_round_trip(&file, &events, &format!("seed {seed} n={n}"));
        assert_eq!(save, rm.inner().save(), "seed {seed}: live save");
    }
}

/// The scripted human: random legal and rejected acts at random tick offsets,
/// random k, plus disconnects and reconnections.
#[test]
fn round_trip_with_a_scripted_human() {
    for seed in 0..8u64 {
        let n = 2 + (seed % 4) as i32;
        let members: Vec<RoomMember> = (1..=n)
            .map(|i| member(i, i != 1, BotMentality::Chaos))
            .collect();
        let mut rm = RecordedMatch::new(
            data(),
            rules(),
            setup(members, seed),
            MatchMode::Casual,
        );
        rm.quick_start();
        let mut rng = Rng::new(seed ^ 0xF00D);
        let mut events = Vec::new();
        let mut last_id = 0;
        let mut outcomes: Vec<(NetMessage, bool)> = Vec::new();
        drain(rm.inner(), &mut last_id, &mut events);
        let mut steps = 0u32;
        while !rm.inner().ended() {
            let k = 1 + rng.below(10) as u8;
            if rng.below(4) == 0 {
                let msg = random_msg(&mut rng, &data());
                let r = rm.act(1, &msg);
                outcomes.push((msg, r.is_ok()));
                drain(rm.inner(), &mut last_id, &mut events);
            } else if rng.below(40) == 0 {
                if rng.chance(0.5) {
                    rm.member_left(1, rng.chance(0.5));
                } else {
                    rm.member_back(1);
                }
                drain(rm.inner(), &mut last_id, &mut events);
            } else {
                rm.tick_steps(k);
                drain(rm.inner(), &mut last_id, &mut events);
            }
            steps += 1;
            assert!(steps < 300_000, "seed {seed}: stuck");
            if rm.inner().state().round > 12 {
                rm.finish();
                drain(rm.inner(), &mut last_id, &mut events);
            }
        }
        let file = rm.export(stamp(), "2026-10-07 00:00");
        assert_round_trip(&file, &events, &format!("human seed {seed}"));
        // The log really did record both Ok and Err acts across the matrix.
        if seed == 0 {
            assert!(
                outcomes.iter().any(|(_, ok)| !*ok),
                "the script must produce some rejected acts"
            );
        }
    }
}

fn random_msg(rng: &mut Rng, d: &GameData) -> NetMessage {
    let tile = rng.below(d.tiles.len()) as i32;
    match rng.below(7) {
        0 => NetMessage::act("roll"),
        1 => NetMessage::act("end"),
        2 => NetMessage {
            act: "buy".into(),
            value: tile,
            ..Default::default()
        },
        3 => NetMessage {
            act: "build".into(),
            value: tile,
            ..Default::default()
        },
        4 => NetMessage::act("vote"),
        5 => NetMessage {
            act: "play".into(),
            card: "no-such-card".into(),
            ..Default::default()
        },
        _ => NetMessage {
            act: "answer".into(),
            value: rng.below(3) as i32,
            prompt: rng.below(4) as i32,
            ..Default::default()
        },
    }
}

// ---------------------------------------------------------------- snapshot

/// A record that starts from a `save()` is `partial` and still round-trips.
#[test]
fn partial_snapshot_records() {
    let members = vec![
        member(1, false, BotMentality::Standard),
        member(2, true, BotMentality::Chaos),
        member(3, true, BotMentality::Standard),
    ];
    let mut m = Match::new(
        data(),
        rules(),
        &members,
        7,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    for _ in 0..40 {
        m.tick(0.25);
    }
    let mut rm = RecordedMatch::from_snapshot(m);
    let mut rng = Rng::new(99);
    let mut events = Vec::new();
    let mut last_id = 0;
    drain(rm.inner(), &mut last_id, &mut events);
    let mut steps = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(1 + rng.below(6) as u8);
        drain(rm.inner(), &mut last_id, &mut events);
        steps += 1;
        assert!(steps < 200_000);
        if rm.inner().state().round > 10 {
            rm.finish();
            drain(rm.inner(), &mut last_id, &mut events);
        }
    }
    let file = rm.export(stamp(), "2026-10-07 00:00");
    assert!(file.header.partial);
    assert!(matches!(file.body.init, Init::Snapshot { .. }));
    assert_round_trip(&file, &events, "snapshot");
}

// ---------------------------------------------------------------- format

#[test]
fn corruption_is_corrupt_and_unknown_format_is_format() {
    let (file, events) = run_bot_game(
        3,
        3,
        BotMentality::Standard,
        &mut |_| 3,
        6,
    );
    assert!(!events.is_empty());
    let json = serde_json::to_string(&file).unwrap();

    // A flipped byte inside the body breaks the check.
    let mut bad = file.clone();
    let ticks = bad
        .body
        .inputs
        .iter_mut()
        .find_map(|i| match i {
            Input::Ticks { n, .. } => Some(n),
            _ => None,
        })
        .expect("a bot game records ticks");
    *ticks += 1;
    let err = parse_record(&serde_json::to_string(&bad).unwrap()).unwrap_err();
    assert!(matches!(err, ReplayError::Corrupt(_)), "{err:?}");

    // A tampered check field is caught too.
    let mut bad = file.clone();
    bad.check = "0".repeat(16);
    assert!(matches!(
        parse_record(&serde_json::to_string(&bad).unwrap()).unwrap_err(),
        ReplayError::Corrupt(_)
    ));

    // Not JSON at all.
    assert!(matches!(
        parse_record("not json").unwrap_err(),
        ReplayError::Corrupt(_)
    ));

    // Wrong magic.
    let mut bad = file.clone();
    bad.magic = "nope".into();
    assert!(matches!(
        parse_record(&serde_json::to_string(&bad).unwrap()).unwrap_err(),
        ReplayError::Format(_)
    ));

    // A format newer than we speak is refused, not misread.
    let mut bad = file.clone();
    bad.header.engine.format = RECORD_VERSION + 1;
    assert!(matches!(
        parse_record(&serde_json::to_string(&bad).unwrap()).unwrap_err(),
        ReplayError::Format(_)
    ));

    // Header-only parse works without validating the body.
    let h = parse_header(&json).unwrap();
    assert_eq!(h.engine, file.header.engine);
}

// ---------------------------------------------------------------- stamps

#[test]
fn stamp_mismatch_is_reported_refused_and_forcible() {
    let (file, events) = run_bot_game(5, 3, BotMentality::Standard, &mut |_| 4, 6);
    let mut bad = file.clone();
    bad.header.engine.data_sha256 = "other-data".into();
    bad.header.engine.abi += 1;

    // compat reports exactly what differs (`want` = record, `got` = engine).
    let mis = compat(&bad.header.engine, &stamp());
    assert_eq!(mis[0].want, (bad.header.engine.abi).to_string());
    assert_eq!(mis[0].got, stamp().abi.to_string());
    let fields: Vec<&str> = mis.iter().map(|m| m.field.as_str()).collect();
    assert_eq!(fields, ["abi", "data_sha256"], "{mis:?}");
    assert!(mis[0].fatal, "abi mismatch is fatal");
    assert!(!mis[1].fatal, "data hash mismatch is a warning");

    // Strict mode refuses; force plays.
    let err = match Replayer::new_with_stamp(data(), rules(), &bad, &stamp(), false) {
        Err(e) => e,
        Ok(_) => panic!("strict mode must refuse a stamp mismatch"),
    };
    match err {
        ReplayError::Incompatible(m) => assert_eq!(m.len(), 2, "{m:?}"),
        other => panic!("expected Incompatible, got {other:?}"),
    }
    let mut rp = Replayer::new_with_stamp(data(), rules(), &bad, &stamp(), true).unwrap();
    while !rp.status().ended {
        let st = rp.step_ticks(8);
        assert!(!st.diverged, "forced replay must still verify checkpoints");
    }
    assert_eq!(rp.events(), events);

    // `Replayer::new` only knows the two format versions: a matching record
    // loads, a save_version mismatch does not.
    assert!(Replayer::new(data(), rules(), &file, false).is_ok());
    let mut old = file.clone();
    old.header.engine.save_version = SAVE_VERSION + 1;
    assert!(matches!(
        Replayer::new(data(), rules(), &old, false).err(),
        Some(ReplayError::Incompatible(_))
    ));
    assert!(Replayer::new(data(), rules(), &old, true).is_ok());
}

// ---------------------------------------------------------------- divergence

#[test]
fn tampered_act_is_caught_as_divergence_at_the_next_checkpoint() {
    // A scripted human makes sure the log has act inputs in it.
    let members = vec![
        member(1, false, BotMentality::Standard),
        member(2, true, BotMentality::Chaos),
        member(3, true, BotMentality::Standard),
    ];
    let mut rm = RecordedMatch::new(
        data(),
        rules(),
        setup(members, 11),
        MatchMode::Casual,
    );
    rm.quick_start();
    let mut rng = Rng::new(11);
    let mut last_id = 0;
    let mut events = Vec::new();
    drain(rm.inner(), &mut last_id, &mut events);
    let mut steps = 0u32;
    while !rm.inner().ended() {
        if rng.below(3) == 0 {
            let _ = rm.act(1, &random_msg(&mut rng, &data()));
        } else {
            rm.tick_steps(1 + rng.below(5) as u8);
        }
        drain(rm.inner(), &mut last_id, &mut events);
        steps += 1;
        assert!(steps < 200_000);
        if rm.inner().state().round > 8 {
            rm.finish();
            drain(rm.inner(), &mut last_id, &mut events);
        }
    }
    let mut file = rm.export(stamp(), "2026-10-07 00:00");

    // Tamper with the first act that actually changed state (`ok: true`):
    // its arguments move, so the replayer's state must drift and the first
    // checkpoint after it has to notice (or the act's Ok/Err bit disagrees).
    let act_at = file
        .body
        .inputs
        .iter()
        .position(|i| matches!(i, Input::Act { ok: true, .. }))
        .expect("the script recorded a successful act");
    match &mut file.body.inputs[act_at] {
        Input::Act { msg, .. } => {
            msg.value += 1;
            msg.card = "tampered".into();
        }
        _ => unreachable!(),
    }
    seal(&mut file); // still a well-formed file -- just a different log

    let mut rp = Replayer::new(data(), rules(), &file, true).unwrap();
    let mut diverged_at = None;
    while !rp.status().ended {
        let st = rp.step_ticks(4);
        if st.diverged {
            diverged_at = Some(st.tick);
            break;
        }
    }
    assert!(diverged_at.is_some(), "tampered act must diverge");
    let mm = rp.first_mismatch().cloned().expect("divergence recorded");
    match mm.field.strip_prefix("checkpoint@") {
        Some(at) => {
            let at: u32 = at.parse().unwrap();
            assert!(
                at as usize > act_at,
                "checkpoint {at} should sit after the tampered input {act_at}"
            );
            let cp = file
                .body
                .checkpoints
                .iter()
                .find(|c| c.at == at)
                .expect("checkpoint exists");
            assert_eq!(cp.hash, mm.want, "mismatch carries the recorded hash");
        }
        // The act's own Ok/Err bit disagreed first -- also a divergence.
        None => assert_eq!(mm.field, "diverged", "{mm:?}"),
    }
}

// ---------------------------------------------------------------- acts

#[test]
fn rejected_acts_replay_to_the_same_err() {
    let members = vec![
        member(1, false, BotMentality::Standard),
        member(2, true, BotMentality::Chaos),
    ];
    let mut rm = RecordedMatch::new(
        data(),
        rules(),
        setup(members, 21),
        MatchMode::Casual,
    );
    rm.quick_start();
    let mut rng = Rng::new(21);
    let mut last_id = 0;
    let mut events = Vec::new();
    let mut recorded: Vec<(i32, NetMessage, Option<String>)> = Vec::new();
    drain(rm.inner(), &mut last_id, &mut events);
    let mut steps = 0u32;
    while !rm.inner().ended() {
        if rng.below(3) == 0 {
            let msg = random_msg(&mut rng, &data());
            let r = rm.act(1, &msg);
            recorded.push((1, msg.clone(), r.err().map(|e| e.key().to_string())));
        } else {
            rm.tick_steps(1 + rng.below(4) as u8);
        }
        drain(rm.inner(), &mut last_id, &mut events);
        steps += 1;
        assert!(steps < 200_000);
        if rm.inner().state().round > 8 {
            rm.finish();
            drain(rm.inner(), &mut last_id, &mut events);
        }
    }
    assert!(
        recorded.iter().any(|(_, _, e)| e.is_some()),
        "need at least one rejected act"
    );
    let file = rm.export(stamp(), "2026-10-07 00:00");

    // Replay input-by-input and compare each act's outcome, Ok and Err alike.
    let mut rp = Replayer::new(data(), rules(), &file, false).unwrap();
    let mut seen = 0usize;
    for input in &file.body.inputs {
        rp.next_input();
        if let Input::Act { m, msg, ok } = input {
            let r = rp.last_act().expect("an act was just applied");
            assert_eq!(r.is_ok(), *ok, "act {seen} ok bit");
            let got = r.as_ref().err().map(|e| e.key().to_string());
            let (want_m, want_msg, want_err) = &recorded[seen];
            assert_eq!(m, want_m, "act {seen} seat");
            assert_eq!(msg, want_msg, "act {seen} message");
            assert_eq!(&got, want_err, "act {seen} error key");
            seen += 1;
        }
    }
    assert_eq!(seen, recorded.len(), "every recorded act replayed");
    assert!(!rp.status().diverged);
}

// ---------------------------------------------------------------- seek

#[test]
fn seek_equals_a_linear_replay() {
    let (file, events) = run_bot_game(9, 4, BotMentality::Chaos, &mut |r| {
        1 + r.below(6) as u8
    }, 10);
    let total = file.header.total_ticks;
    assert!(total > 50, "need a game long enough to seek in");

    let mut indexed = Replayer::new(data(), rules(), &file, false).unwrap();
    let st = indexed.index(1_000_000);
    assert!(st.done, "one budget pass indexes a short game");
    assert!(st.keyframes >= 2, "keyframes: {}", st.keyframes);
    // A second call is idempotent.
    let st2 = indexed.index(1_000_000);
    assert_eq!(st2.keyframes, st.keyframes);

    let targets: Vec<u64> = {
        let mut v = vec![0, 1, total / 4, total / 2, total * 3 / 4, total];
        v.sort_unstable();
        v.dedup();
        v
    };
    for &t in &targets {
        let want = {
            let mut rp = Replayer::new(data(), rules(), &file, false).unwrap();
            while rp.status().tick < t && !rp.status().ended {
                rp.step_ticks(1);
            }
            rp.match_ref().save()
        };
        indexed.seek(t).unwrap();
        assert_eq!(
            indexed.status().tick,
            t.min(total),
            "seek to {t} landed on the right tick"
        );
        assert_eq!(indexed.match_ref().save(), want, "seek to {t} state");
    }

    // A seek back to 0 is the initial frame again.
    indexed.seek(0).unwrap();
    assert_eq!(indexed.status().tick, 0);
    let fresh = Replayer::new(data(), rules(), &file, false).unwrap();
    assert_eq!(indexed.match_ref().save(), fresh.match_ref().save());
    let _ = events;
}

// ---------------------------------------------------------------- values

#[test]
fn u64_and_f32_values_are_bit_exact() {
    // A seed above 2^53 must survive the string encoding.
    let big = (1u64 << 53) + 7;
    let members = vec![
        member(1, true, BotMentality::Standard),
        member(2, true, BotMentality::Standard),
    ];
    let mut rm = RecordedMatch::new(
        data(),
        rules(),
        setup(members, big),
        MatchMode::Casual,
    );
    rm.quick_start();
    let mut rng = Rng::new(big);
    let mut events = Vec::new();
    let mut last_id = 0;
    drain(rm.inner(), &mut last_id, &mut events);
    while !rm.inner().ended() {
        rm.tick_steps(1 + rng.below(5) as u8);
        drain(rm.inner(), &mut last_id, &mut events);
        if rm.inner().state().round > 6 {
            rm.finish();
            drain(rm.inner(), &mut last_id, &mut events);
        }
    }
    let file = rm.export(stamp(), "2026-10-07 00:00");
    match &file.body.init {
        Init::Seed(s) => assert_eq!(s.seed, big, "seed survives as a string"),
        other => panic!("{other:?}"),
    }
    let json = serde_json::to_string(&file).unwrap();
    assert!(
        json.contains(&format!("\"{big}\"")),
        "seed is serialized as a string"
    );
    let parsed = parse_record(&json).unwrap();
    assert_eq!(parsed.body.init, file.body.init);

    // The replayed save is byte-equal, which pins every f32 inside it
    // (`save()` serializes with serde_json's shortest round-trip form).
    let save = assert_round_trip(&file, &events, "bit-exact");
    assert_eq!(save, rm.inner().save());

    // And an explicit f32: the score weights are in the setup and must come
    // back with the same bits.
    let w = ScoreWeights {
        money: 0.5,
        property: 2.5,
        houses: 4.5,
    };
    let setup = MatchSetup {
        members: vec![member(1, true, BotMentality::Standard)],
        seed: 1,
        seed256: None,
        weights: w,
    };
    let body = game_core::record::RecordBody {
        init: Init::Seed(setup.clone()),
        ..Default::default()
    };
    let s = serde_json::to_string(&body).unwrap();
    let back: game_core::record::RecordBody = serde_json::from_str(&s).unwrap();
    match &back.init {
        Init::Seed(got) => {
            assert_eq!(got.weights.money.to_bits(), w.money.to_bits());
            assert_eq!(got.weights.property.to_bits(), w.property.to_bits());
            assert_eq!(got.weights.houses.to_bits(), w.houses.to_bits());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(body_check(&body), body_check(&back), "check is stable");
}

// ---------------------------------------------------------------- events bundle

#[test]
fn export_with_events_bundles_the_public_log() {
    let mut rm = RecordedMatch::new(
        data(),
        rules(),
        setup(
            vec![
                member(1, true, BotMentality::Standard),
                member(2, true, BotMentality::Standard),
                member(3, true, BotMentality::Standard),
            ],
            4,
        ),
        MatchMode::Casual,
    );
    rm.quick_start();
    let mut last_id = 0;
    let mut live = Vec::new();
    drain(rm.inner(), &mut last_id, &mut live);
    while !rm.inner().ended() {
        rm.tick_steps(3);
        drain(rm.inner(), &mut last_id, &mut live);
        if rm.inner().state().round > 6 {
            rm.finish();
            drain(rm.inner(), &mut last_id, &mut live);
        }
    }
    let plain = rm.export(stamp(), "2026-10-07 00:00");
    let bundled = rm
        .export_with_events(data(), rules(), stamp(), "2026-10-07 00:00")
        .unwrap();
    assert_eq!(bundled.body.events.as_deref(), Some(&live[..]));
    assert_eq!(body_check(&bundled.body), bundled.check);
    // The authoritative log is unchanged by the bundle.
    assert_eq!(bundled.body.inputs, plain.body.inputs);
    assert_eq!(bundled.body.checkpoints, plain.body.checkpoints);
}

// ---------------------------------------------------------------- persist

#[test]
fn recorder_json_round_trips_through_restore() {
    let members = vec![
        member(1, false, BotMentality::Standard),
        member(2, true, BotMentality::Chaos),
    ];
    let mut rm = RecordedMatch::new(
        data(),
        rules(),
        setup(members, 31),
        MatchMode::Casual,
    );
    rm.quick_start();
    for _ in 0..25 {
        rm.tick_steps(2);
    }
    let save = rm.inner().save();
    let rec_json = rm.recorder_json();

    let mut restored =
        RecordedMatch::restore(data(), rules(), &save, &rec_json).expect("restore");
    assert_eq!(restored.inner().save(), save);
    // Continue both by the same 25 steps: the restored recorder picks the log
    // up where it left off and produces the same record.
    for _ in 0..25 {
        rm.tick_steps(2);
        restored.tick_steps(2);
    }
    let a = rm.export(stamp(), "x");
    let b = restored.export(stamp(), "x");
    assert_eq!(a.body.inputs, b.body.inputs);
    assert_eq!(a.body.checkpoints, b.body.checkpoints);
    assert_eq!(a.header.total_ticks, b.header.total_ticks);
    assert_eq!(a.body.final_hash, b.body.final_hash);
    assert_eq!(rm.inner().save(), restored.inner().save());
}

// ---------------------------------------------------------------- origin

#[test]
fn origin_and_header_fields_survive() {
    let (file, _) = run_bot_game(2, 2, BotMentality::Standard, &mut |_| 2, 5);
    let json = serde_json::to_string(&file).unwrap();
    let h = parse_header(&json).unwrap();
    assert_eq!(h.origin, Origin::Solo);
    assert_eq!(h.step, STEP);
    assert_eq!(h.mode, MatchMode::Casual);
    assert!(h.ended);
    assert!(!h.seats.is_empty());
    assert!(h.rounds >= 1);
    assert!(h.total_ticks > 0);
    assert_eq!(h.seats[0].player, "P1");
    assert!(h.seats.iter().all(|s| s.bot));
}

// ---------------------------------------------------------------- solo preset

/// `Match::new`'s solo preset path (a character already on every seat, so the
/// timed ban / pick is skipped and the match opens in the deck phase) is the
/// path `SoloSession.start` takes. It runs inside `Match::new`, so it is part
/// of `Init::Seed` and must reproduce identically on the replayer -- including
/// the `do_pick` / `submit_deck` / `begin_play` side effects it triggers.
#[test]
fn solo_preset_characters_round_trip() {
    let d = data();
    let chars: Vec<String> = d
        .characters
        .iter()
        .take(4)
        .map(|c| c.name.clone())
        .collect();
    assert!(chars.len() >= 2, "need at least two characters in data");
    for seed in [1u64, 42, 999, 31337] {
        for n in 2..=4usize {
            let members: Vec<RoomMember> = (0..n)
                .map(|i| RoomMember {
                    id: i as i32 + 1,
                    player: format!("P{}", i + 1),
                    character: chars[i % chars.len()].clone(),
                    bot: true,
                    mentality: if i % 2 == 0 {
                        BotMentality::Standard
                    } else {
                        BotMentality::Chaos
                    },
                    ..Default::default()
                })
                .collect();
            let mut rm = RecordedMatch::new(
                d.clone(),
                rules(),
                setup(members.clone(), seed),
                MatchMode::Solo,
            );
            // The preset path drops us straight into `deck` (or `play` when
            // every seat is already deck-ready) -- no pick phase.
            let phase = rm.inner().state().phase;
            assert!(
                phase == "deck" || phase == "play",
                "seed {seed} n {n}: preset must skip ban/pick, got {phase}"
            );
            let mut steps = 0u32;
            let mut events: Vec<MatchEvent> = Vec::new();
            let mut last_id = 0;
            drain(rm.inner(), &mut last_id, &mut events);
            while !rm.inner().ended() && rm.inner().state().round < 3 {
                rm.tick_steps(1 + (steps % 3) as u8);
                steps += 1;
                drain(rm.inner(), &mut last_id, &mut events);
                assert!(steps < 50_000, "seed {seed} n {n}: no progress");
            }
            let file = rm.export(stamp(), "x");
            let label = format!("solo-preset seed {seed} n {n}");
            assert_round_trip(&file, &events, &label);
        }
    }
}

/// A record sealed under one ruleset and replayed under another diverges at
/// the first checkpoint -- the stamp must name it (`ruleset_sha256`), and the
/// mismatch must be a **warning** (the UI warns, then verifies checkpoints),
/// not silent.
#[test]
fn ruleset_mismatch_is_named_and_non_fatal() {
    let d = data();
    let chars: Vec<String> = d.characters.iter().take(2).map(|c| c.name.clone()).collect();
    let members: Vec<RoomMember> = (0..2)
        .map(|i| RoomMember {
            id: i as i32 + 1,
            player: format!("P{}", i + 1),
            character: chars[i].clone(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        })
        .collect();
    let mut rm = RecordedMatch::new(d.clone(), rules(), setup(members, 7), MatchMode::Solo);
    while !rm.inner().ended() && rm.inner().state().round < 2 {
        rm.tick_steps(2);
    }
    let mut file = rm.export(stamp(), "x");
    // Pretend the record was written under a real ruleset build.
    file.header.engine.ruleset_sha256 = "49a58b82416cca44f11a1a45986ac651a2052ab59704989470789439d914911c".into();
    seal(&mut file);

    // `want` is the record's stamp, `got` is this engine's (StubRules -> "stub").
    let mis = compat(&file.header.engine, &stamp());
    let hit = mis.iter().find(|m| m.field == "ruleset_sha256").unwrap();
    assert_eq!(hit.want, file.header.engine.ruleset_sha256);
    assert_eq!(hit.got, stamp().ruleset_sha256);
    assert!(!hit.fatal, "ruleset drift warns, it does not refuse");
    // `Replayer::new` (which only knows the two format versions) does not
    // report it -- `new_with_stamp` does.
    assert!(Replayer::new(d.clone(), rules(), &file, false).is_ok());
    assert!(matches!(
        Replayer::new_with_stamp(d.clone(), rules(), &file, &stamp(), false).err(),
        Some(ReplayError::Incompatible(_))
    ));
}

// ---------------------------------------------------------------- big

/// The full matrix, 1000 seeds. Slow on purpose -- run with
/// `cargo test -p game-core --test record -- --ignored`.
#[test]
#[ignore = "1000-seed soak; run explicitly"]
fn round_trip_1000_seeds() {
    for seed in 0..1000u64 {
        let n = 2 + (seed % 5) as i32;
        let members: Vec<RoomMember> = (1..=n)
            .map(|i| {
                member(
                    i,
                    true,
                    if seed % 2 == 0 {
                        BotMentality::Standard
                    } else {
                        BotMentality::Chaos
                    },
                )
            })
            .collect();
        let mut rm = RecordedMatch::new(
            data(),
            rules(),
            setup(members, seed),
            MatchMode::Casual,
        );
        rm.quick_start();
        let mut rng = Rng::new(seed ^ 0xD1CE);
        let mut events = Vec::new();
        let mut last_id = 0;
        drain(rm.inner(), &mut last_id, &mut events);
        let mut steps = 0u32;
        while !rm.inner().ended() {
            rm.tick_steps(1 + rng.below(10) as u8);
            drain(rm.inner(), &mut last_id, &mut events);
            steps += 1;
            assert!(steps < 300_000, "seed {seed}");
            if rm.inner().state().round > 12 {
                rm.finish();
                drain(rm.inner(), &mut last_id, &mut events);
            }
        }
        let file = rm.export(stamp(), "2026-10-07 00:00");
        assert_round_trip(&file, &events, &format!("seed {seed}"));
    }
}

// ---------------------------------------------------------------- perf

/// `RecordedMatch` overhead against the bare [`Match`]. Timing tests are
/// flaky on a loaded machine, so this is `#[ignore]`d; run it when you touch
/// the recorder.
///
///     cargo test --release -p game-core --test record -- --ignored overhead --nocapture
///
/// The recorder's only real cost is the turn-boundary `save()` behind each
/// checkpoint (`docs/REPLAY.md` §1: "At each turn change: `save()` plus FNV"),
/// so the middle variant -- a bare [`Match`] with the same `save()` + FNV at
/// every turn boundary and no recorder at all -- is what the overhead is
/// measured against. The wrapper itself is a run-length increment per tick
/// and must stay under 2% of that. The design-mandated checkpoint work as a
/// share of a bare game is reported separately (see
/// [`checkpoint_cost_breakdown`]); it is the design's cost, not the wrapper's.
#[test]
#[ignore = "timing; run explicitly"]
fn recorded_match_overhead_is_under_two_percent() {
    let games = 40u64;
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        /// Plain `Match`, no hashing (what `examples/sim` does).
        Bare,
        /// `Match` + the design's per-turn cost: `save()` + FNV, no recorder.
        Checkpoint,
        /// The real [`RecordedMatch`].
        Recorded,
    }
    let play = |mode: Mode| {
        let started = std::time::Instant::now();
        for seed in 0..games {
            let members = vec![
                member(1, true, BotMentality::Standard),
                member(2, true, BotMentality::Standard),
                member(3, true, BotMentality::Standard),
                member(4, true, BotMentality::Standard),
            ];
            let mut rng = Rng::new(seed);
            match mode {
                Mode::Bare => {
                    let mut m = Match::new(
                        data(),
                        rules(),
                        &members,
                        seed,
                        MatchMode::Casual,
                        ScoreWeights::default(),
                    );
                    m.quick_start();
                    while !m.ended() {
                        m.tick((1 + rng.below(10)) as f32 * STEP);
                        if m.world().st.round > 30 {
                            m.finish();
                        }
                    }
                    std::hint::black_box(m.save());
                }
                Mode::Checkpoint => {
                    let mut m = Match::new(
                        data(),
                        rules(),
                        &members,
                        seed,
                        MatchMode::Casual,
                        ScoreWeights::default(),
                    );
                    m.quick_start();
                    let mut last = (i32::MIN, i32::MIN);
                    while !m.ended() {
                        m.tick((1 + rng.below(10)) as f32 * STEP);
                        let st = &m.world().st;
                        let key = (st.round, st.turn);
                        if key != last {
                            last = key;
                            std::hint::black_box(m.save().len());
                        }
                        if st.round > 30 {
                            m.finish();
                        }
                    }
                }
                Mode::Recorded => {
                    let mut rm = RecordedMatch::new(
                        data(),
                        rules(),
                        setup(members, seed),
                        MatchMode::Casual,
                    );
                    rm.quick_start();
                    while !rm.inner().ended() {
                        rm.tick_steps(1 + rng.below(10) as u8);
                        if rm.inner().world().st.round > 30 {
                            rm.finish();
                        }
                    }
                    std::hint::black_box(rm.export(stamp(), "x"));
                }
            }
        }
        started.elapsed()
    };
    // Warm up every path.
    play(Mode::Bare);
    play(Mode::Checkpoint);
    play(Mode::Recorded);
    let bare = play(Mode::Bare);
    let checkpoint = play(Mode::Checkpoint);
    let rec = play(Mode::Recorded);
    let wrapper = rec.as_secs_f64() / checkpoint.as_secs_f64();
    println!(
        "bare {bare:?} checkpoint {checkpoint:?} recorded {rec:?} \
         wrapper {:.2}% design-cost-vs-bare {:.2}%",
        (wrapper - 1.0) * 100.0,
        (checkpoint.as_secs_f64() / bare.as_secs_f64() - 1.0) * 100.0,
    );
    assert!(
        wrapper < 1.02,
        "RecordedMatch wrapper overhead {:.2}% over the design's checkpoint cost \
         (checkpoint {checkpoint:?}, recorded {rec:?})",
        (wrapper - 1.0) * 100.0
    );
}

/// Absolute cost of one checkpoint (save + FNV), and how many a full sim-length
/// game needs. Reports numbers; the 2% gate is judged against `examples/sim`'s
/// 285 ms/game.
#[test]
#[ignore = "timing; run explicitly"]
fn checkpoint_cost_breakdown() {
    use game_core::record::hash_save;
    let members = vec![
        member(1, true, BotMentality::Standard),
        member(2, true, BotMentality::Standard),
        member(3, true, BotMentality::Standard),
        member(4, true, BotMentality::Standard),
    ];
    let mut m = Match::new(
        data(),
        rules(),
        &members,
        1,
        MatchMode::Casual,
        ScoreWeights::default(),
    );
    m.quick_start();
    let mut last = (i32::MIN, i32::MIN);
    let mut turns = 0u32;
    let mut save_ns = 0u64;
    let mut hash_ns = 0u64;
    let mut bytes = 0usize;
    let game0 = std::time::Instant::now();
    while !m.ended() {
        m.tick(0.25);
        let st = &m.world().st;
        let key = (st.round, st.turn);
        if key != last {
            last = key;
            let t0 = std::time::Instant::now();
            let s = m.save();
            save_ns += t0.elapsed().as_nanos() as u64;
            let t1 = std::time::Instant::now();
            let h = hash_save(&s);
            hash_ns += t1.elapsed().as_nanos() as u64;
            bytes += s.len();
            std::hint::black_box(h);
            turns += 1;
        }
        if st.round > 200 {
            m.finish();
        }
    }
    let game = game0.elapsed().as_secs_f64() * 1e3;
    println!(
        "game {game:.1}ms turns {turns} avg_save {:.1}us avg_hash {:.1}us avg_bytes {} \
         total_save {:.1}ms total_hash {:.1}ms overhead {:.2}%",
        save_ns as f64 / turns as f64 / 1e3,
        hash_ns as f64 / turns as f64 / 1e3,
        bytes / turns as usize,
        save_ns as f64 / 1e6,
        hash_ns as f64 / 1e6,
        (save_ns + hash_ns) as f64 / 1e6 / game * 100.0,
    );
}
