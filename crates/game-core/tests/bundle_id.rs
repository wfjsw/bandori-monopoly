//! The engine-bundle id recipe (`docs/REPLAY.md` §9) must stay in lockstep
//! with `tools/engine-bundle.mjs` -- a record's `EngineStamp::bundle` is how
//! the replay player finds the archived engine that wrote it. The known
//! vector below is produced by the JS twin; `tools/test-replay-archive.mjs`
//! additionally pins both against a real glue build.

use game_core::record::{bundle_id, EngineStamp};

#[test]
fn bundle_id_recipe_matches_the_js_twin() {
    let s = EngineStamp {
        format: 1,
        save_version: 4,
        abi: 40,
        ruleset_sha256: "r".into(),
        data_sha256: "d".into(),
        engine: "game-core".into(),
        build: "test".into(),
        bundle: String::new(),
    };
    assert_eq!(
        bundle_id("g", &s),
        "6f0ef2546742b5ae8ae8f4db4b2f46411b19457beed4d54e5fde3599607e5787"
    );
}

#[test]
fn empty_glue_sha_is_an_unknown_bundle() {
    let s = EngineStamp::default();
    assert_eq!(bundle_id("", &s), "");
}

#[test]
fn bundle_round_trips_through_the_stamp() {
    let mut s = EngineStamp {
        ruleset_sha256: "rules".into(),
        data_sha256: "data".into(),
        ..EngineStamp::default()
    };
    s.bundle = bundle_id("glue", &s);
    let json = serde_json::to_string(&s).unwrap();
    let back: EngineStamp = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    // A record from before bundles: the field defaults to "" and everything
    // else still parses (the loader falls back to matching stamp fields).
    let old = r#"{"format":1,"save_version":4,"abi":40,"ruleset_sha256":"x","data_sha256":"y","engine":"game-core","build":"0.1.0"}"#;
    let old: EngineStamp = serde_json::from_str(old).unwrap();
    assert_eq!(old.bundle, "");
}