//! Commit-reveal fairness tests (`docs/FAIRNESS.md`).
//!
//! The round trip that matters: a seeded match sealed with its openings must
//! **verify**, and any single-field tamper of those openings must fail the
//! step that covers it.

use std::sync::Arc;

use game_core::data::GameData;
use game_core::engine::{CardRules, Match, StubRules, SAVE_VERSION};
use game_core::fair::{
    commit_hex, commit_hex_v1, derive_match_seed, hex32, live_seed, unhex32, verify, verify_replay,
    CanonSettings, Fairness, NonceEntry, FAIR_VERSION,
};
use game_core::net::RoomMember;
use game_core::record::{
    EngineStamp, Init, MatchSetup, Origin, RecordedMatch, RECORD_VERSION,
};
use game_core::rng::{Rng, Seed256};
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
        ruleset_sha256: "rules-test".into(),
        data_sha256: "data-test".into(),
        engine: "game-core".into(),
        build: "test".into(),
        bundle: "bundle-test".into(),
    }
}

fn members() -> Vec<RoomMember> {
    vec![
        RoomMember {
            id: 1,
            player: "P1".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 2,
            player: "P2".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
        RoomMember {
            id: 3,
            player: "P3".into(),
            bot: true,
            mentality: BotMentality::Standard,
            ..Default::default()
        },
    ]
}

const STEP: f32 = 0.05;

/// The openings a server would draw: secret seed + salt, one human nonce.
struct Openings {
    seed: [u8; 32],
    salt: [u8; 32],
    nonces: Vec<(i32, [u8; 32])>,
}

fn openings() -> Openings {
    let mut seed = [0u8; 32];
    let mut salt = [0u8; 32];
    for i in 0..32 {
        seed[i] = (i as u8).wrapping_mul(17).wrapping_add(3);
        salt[i] = (i as u8).wrapping_mul(91).wrapping_add(7);
    }
    Openings {
        seed,
        salt,
        nonces: vec![(2, [0x5a; 32])],
    }
}

fn settings_of(members: &[RoomMember]) -> String {
    CanonSettings::new(MatchMode::Casual, STEP, &ScoreWeights::default(), members).canon()
}

/// Build the commit + derived seed the server would produce, run a short bot
/// game on that seed, and seal it with the full [`Fairness`]. The commitment
/// is the v2 one -- drawn at room creation over the openings and the engine
/// identity only (`docs/FAIRNESS.md` §1.1).
fn seeded_record(o: &Openings) -> game_core::record::RecordFile {
    seeded_record_v(o, FAIR_VERSION)
}

/// [`seeded_record`] under an explicit scheme version, so a v1 record (the
/// 2026-10-08 recipe) can be built and checked against its own recipe.
fn seeded_record_v(o: &Openings, v: u32) -> game_core::record::RecordFile {
    let members = members();
    let settings = settings_of(&members);
    let st = stamp();
    let commit = if v == 1 {
        commit_hex_v1(
            &o.seed,
            &o.salt,
            &st.bundle,
            &st.ruleset_sha256,
            &settings,
        )
    } else {
        commit_hex(&o.seed, &o.salt, &st.bundle, &st.ruleset_sha256)
    };
    let derived = derive_match_seed(&o.seed, &o.nonces);
    let setup = MatchSetup {
        members: members.clone(),
        seed: 0,
        seed256: Some(Seed256(derived)),
        weights: ScoreWeights::default(),
    };
    let mut rm = RecordedMatch::new(data(), rules(), setup, MatchMode::Casual);
    rm.set_fair(Fairness {
        v,
        commit,
        seed: hex32(&o.seed),
        salt: hex32(&o.salt),
        nonces: o
            .nonces
            .iter()
            .map(|(m, n)| NonceEntry {
                member: *m,
                nonce: hex32(n),
            })
            .collect(),
        settings,
    });
    rm.quick_start();
    let mut steps = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(4);
        steps += 1;
        assert!(steps < 200_000, "game did not finish");
    }
    rm.export(stamp(), "2026-10-08 12:00")
}

#[test]
fn commit_reveal_round_trip_verifies() {
    let o = openings();
    let file = seeded_record(&o);
    let rep = verify(&file);
    assert!(rep.ok, "{:?}", rep.steps);
    assert!(rep.present);
    let rep = verify_replay(&file, data(), rules());
    assert!(rep.ok, "{:?}", rep.steps);
    // Every named step is there and green.
    let names: Vec<&str> = rep.steps.iter().map(|s| s.step.as_str()).collect();
    for want in [
        "settings",
        "nonces",
        "commit",
        "bundle",
        "ruleset",
        "derived_seed",
        "initial_rng",
        "replay",
    ] {
        assert!(names.contains(&want), "missing step {want}: {names:?}");
    }
}

#[test]
fn tampered_seed_fails_the_commit() {
    let o = openings();
    let mut file = seeded_record(&o);
    let mut fair = file.header.fair.clone().unwrap();
    // Flip one nibble of the server seed.
    let mut s = fair.seed.clone();
    s.replace_range(0..1, if s.starts_with('0') { "1" } else { "0" });
    fair.seed = s;
    file.header.fair = Some(fair);
    let rep = verify(&file);
    assert!(!rep.ok);
    let commit = rep.steps.iter().find(|s| s.step == "commit").unwrap();
    assert!(!commit.ok, "{:?}", rep.steps);
}

#[test]
fn tampered_salt_fails_the_commit() {
    let o = openings();
    let mut file = seeded_record(&o);
    let mut fair = file.header.fair.clone().unwrap();
    let mut t = fair.salt.clone();
    t.replace_range(0..1, if t.starts_with('0') { "1" } else { "0" });
    fair.salt = t;
    file.header.fair = Some(fair);
    let rep = verify(&file);
    assert!(!rep.ok);
    let commit = rep.steps.iter().find(|s| s.step == "commit").unwrap();
    assert!(!commit.ok, "{:?}", rep.steps);
}

#[test]
fn tampered_nonce_fails_the_derived_seed() {
    let o = openings();
    let mut file = seeded_record(&o);
    let mut fair = file.header.fair.clone().unwrap();
    fair.nonces[0].nonce = hex32(&[0x5b; 32]);
    file.header.fair = Some(fair);
    let rep = verify(&file);
    assert!(!rep.ok);
    // The commitment still opens (it does not cover the nonces); what fails
    // is that the derived seed no longer matches the one the match ran on.
    let commit = rep.steps.iter().find(|s| s.step == "commit").unwrap();
    assert!(commit.ok, "{:?}", rep.steps);
    let derived = rep.steps.iter().find(|s| s.step == "derived_seed").unwrap();
    assert!(!derived.ok, "{:?}", rep.steps);
}

#[test]
fn tampered_engine_identity_fails_the_commit() {
    for field in ["bundle", "ruleset"] {
        let o = openings();
        let mut file = seeded_record(&o);
        if field == "bundle" {
            file.header.engine.bundle = "bundle-evil".into();
        } else {
            file.header.engine.ruleset_sha256 = "rules-evil".into();
        }
        let rep = verify(&file);
        assert!(!rep.ok, "{field}");
        let commit = rep.steps.iter().find(|s| s.step == "commit").unwrap();
        assert!(!commit.ok, "{field}: {:?}", rep.steps);
    }
}

#[test]
fn tampered_settings_fails() {
    let o = openings();
    let mut file = seeded_record(&o);
    let mut fair = file.header.fair.clone().unwrap();
    fair.settings = fair.settings.replace("P1", "P9");
    file.header.fair = Some(fair);
    let rep = verify(&file);
    assert!(!rep.ok);
    // The rebuilt canonical string from the record's own setup disagrees.
    // Under v2 the commitment no longer folds the settings in (it is drawn at
    // room creation, before the roster is fixed), so this is the step that
    // covers the edit -- the commitment itself still opens.
    let settings = rep.steps.iter().find(|s| s.step == "settings").unwrap();
    assert!(!settings.ok, "{:?}", rep.steps);
    let commit = rep.steps.iter().find(|s| s.step == "commit").unwrap();
    assert!(commit.ok, "{:?}", rep.steps);
}

#[test]
fn a_nonce_from_a_non_participant_fails() {
    // The record only mixes in the nonces of the members who sat down; a
    // nonce smuggled in for someone else must not verify (`docs/FAIRNESS.md`
    // -- nonces are dropped for anyone outside the match).
    let o = openings();
    let mut file = seeded_record(&o);
    let mut fair = file.header.fair.clone().unwrap();
    fair.nonces.push(NonceEntry {
        member: 99,
        nonce: hex32(&[0x77; 32]),
    });
    file.header.fair = Some(fair);
    let rep = verify(&file);
    assert!(!rep.ok, "{:?}", rep.steps);
    let nonces = rep.steps.iter().find(|s| s.step == "nonces").unwrap();
    assert!(!nonces.ok, "{:?}", rep.steps);
}

#[test]
fn a_v1_record_still_verifies_under_the_v1_recipe() {
    // Records written 2026-10-08 carry `v: 1` and a commitment that folded
    // the canonical settings in. They keep verifying -- against *their*
    // recorded scheme version, not the current one.
    let o = openings();
    let file = seeded_record_v(&o, 1);
    let fair = file.header.fair.as_ref().unwrap();
    assert_eq!(fair.v, 1);
    let rep = verify(&file);
    assert!(rep.ok, "{:?}", rep.steps);
    let rep = verify_replay(&file, data(), rules());
    assert!(rep.ok, "{:?}", rep.steps);
    // ...and the v1 recipe is still the one that opens it: swapping in the v2
    // hash fails.
    let mut tampered = file.clone();
    let mut f = tampered.header.fair.clone().unwrap();
    f.commit = commit_hex(&o.seed, &o.salt, &stamp().bundle, &stamp().ruleset_sha256);
    tampered.header.fair = Some(f);
    assert!(!verify(&tampered).ok);
}

#[test]
fn a_v2_commit_covers_the_openings_and_the_engine_only() {
    // The creation-time commitment pins seed/salt/engine identity. What it
    // deliberately does not pin (the roster and the settings can still change
    // before the start) is covered by the header checks instead.
    let o = openings();
    let file = seeded_record(&o);
    let fair = file.header.fair.clone().unwrap();
    assert_eq!(fair.v, FAIR_VERSION);
    assert_eq!(
        fair.commit,
        commit_hex(&o.seed, &o.salt, &stamp().bundle, &stamp().ruleset_sha256)
    );
    // A different roster with the same openings re-derives a different
    // settings string -- and the `settings` step is what catches it.
    let mut swapped = file.clone();
    if let Init::Seed(setup) = &mut swapped.body.init {
        setup.members[0].player = "Mallory".into();
    }
    game_core::record::seal(&mut swapped);
    let rep = verify(&swapped);
    assert!(!rep.ok, "{:?}", rep.steps);
    let settings = rep.steps.iter().find(|s| s.step == "settings").unwrap();
    assert!(!settings.ok, "{:?}", rep.steps);
}

#[test]
fn tampered_setup_seed_fails_the_derived_seed() {
    let o = openings();
    let mut file = seeded_record(&o);
    if let Init::Seed(setup) = &mut file.body.init {
        setup.seed256 = Some(Seed256([0xff; 32]));
    }
    // The body changed: re-seal it the way a forger would.
    game_core::record::seal(&mut file);
    let rep = verify(&file);
    assert!(!rep.ok);
    let derived = rep.steps.iter().find(|s| s.step == "derived_seed").unwrap();
    assert!(!derived.ok, "{:?}", rep.steps);
}

#[test]
fn tampered_replay_input_fails_the_replay_step() {
    let o = openings();
    let mut file = seeded_record(&o);
    // Flip one recorded act's Ok bit (or drop a tick) -- the log and its
    // checkpoints then disagree.
    for input in file.body.inputs.iter_mut() {
        if let game_core::record::Input::Ticks { n, .. } = input {
            *n = n.saturating_sub(1);
            break;
        }
    }
    game_core::record::seal(&mut file);
    let rep = verify_replay(&file, data(), rules());
    assert!(!rep.ok, "{:?}", rep.steps);
    let replay = rep.steps.iter().find(|s| s.step == "replay").unwrap();
    assert!(!replay.ok, "{:?}", rep.steps);
}

#[test]
fn a_record_without_fairness_material_is_reported_not_faked() {
    // The shape of a pre-scheme record: seeded, no fair header, no seed256.
    let setup = MatchSetup {
        members: members(),
        seed: 42,
        seed256: None,
        weights: ScoreWeights::default(),
    };
    let mut rm = RecordedMatch::new(data(), rules(), setup, MatchMode::Casual);
    rm.quick_start();
    let mut steps = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(4);
        steps += 1;
        assert!(steps < 200_000);
    }
    let file = rm.export(stamp(), "2026-10-08 12:00");
    assert!(file.header.fair.is_none());
    let rep = verify(&file);
    assert!(!rep.ok);
    assert!(!rep.present, "{:?}", rep.steps);
}

#[test]
fn the_match_rng_is_keyed_by_the_derived_seed() {
    let o = openings();
    let derived = derive_match_seed(&o.seed, &o.nonces);
    let setup = MatchSetup {
        members: members(),
        seed: 0,
        seed256: Some(Seed256(derived)),
        weights: ScoreWeights::default(),
    };
    let m = Match::new_seeded(
        data(),
        rules(),
        &setup.members,
        Seed256(derived),
        MatchMode::Casual,
        setup.weights,
    );
    let (game_key, live_key) = m.seed256s();
    assert_eq!(game_key, Some(derived));
    assert_eq!(live_key, Some(live_seed(&derived)));
    // And the RNG state in the save names that key.
    let parsed: serde_json::Value = serde_json::from_str(&m.save()).unwrap();
    let key_hex = parsed["world"]["rng"]["c"]["key"].as_str().expect("chaCha rng");
    assert_eq!(key_hex, hex32(&derived));
    // `match_id` carries no seed bits (it is hashed).
    assert_eq!(m.state().match_id, game_core::fair::match_id(&derived));
}

#[test]
fn legacy_u64_records_still_replay_on_the_legacy_stream() {
    // A v1-style record (u64 seed, xoshiro) must still round trip after the
    // ChaCha switch -- this is the `Init::Seed` half of the compat story
    // (`docs/FAIRNESS.md` "old records").
    let setup = MatchSetup {
        members: members(),
        seed: 7,
        seed256: None,
        weights: ScoreWeights::default(),
    };
    let mut rm = RecordedMatch::new(data(), rules(), setup, MatchMode::Casual);
    rm.quick_start();
    let mut steps = 0u32;
    while !rm.inner().ended() {
        rm.tick_steps(4);
        steps += 1;
        assert!(steps < 200_000);
    }
    let file = rm.export(stamp(), "2026-10-08 12:00");
    let mut rp = game_core::record::Replayer::new(data(), rules(), &file, true).unwrap();
    let mut guard = 0u32;
    while !rp.status().ended {
        rp.step_ticks(8);
        guard += 1;
        assert!(guard < 200_000);
    }
    assert!(!rp.status().diverged, "{:?}", rp.first_mismatch());
    // The save still carries the legacy `"s": [...]` stream.
    let parsed: serde_json::Value = serde_json::from_str(&rp.match_ref().save()).unwrap();
    assert!(parsed["world"]["rng"]["s"].is_array());
}

#[test]
fn old_save_json_resumes_on_the_legacy_stream() {
    // Exactly what `Match::save` wrote before the switch: `s: [u64; 4]`.
    let mut r: Rng = serde_json::from_str(r#"{"s":[1,2,3,4]}"#).unwrap();
    assert!(r.seed256().is_none());
    let mut again: Rng = serde_json::from_str(r#"{"s":[1,2,3,4]}"#).unwrap();
    assert_eq!(r.next_u64(), again.next_u64());
    // And a derived-seed construction is a different, pinned stream.
    let k = unhex32(&hex32(&[9u8; 32])).unwrap();
    let mut c = Rng::from_seed256(k);
    assert_ne!(c.next_u64(), r.next_u64());
}

#[test]
fn openings_round_trip_through_the_header() {
    let o = openings();
    let file = seeded_record(&o);
    let json = serde_json::to_string(&file).unwrap();
    let back: game_core::record::RecordFile = serde_json::from_str(&json).unwrap();
    assert_eq!(back.header.fair, file.header.fair);
    // And the record is Origin::Solo here; a server record would be Online.
    assert!(matches!(file.header.origin, Origin::Solo));
    assert_eq!(file.header.mode, MatchMode::Casual);
}