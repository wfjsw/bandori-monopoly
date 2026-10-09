//! The archived reference records (`archive/engine/refs/`) must stay readable
//! and keep their replay contract (`docs/REPLAY.md` §9, `docs/FAIRNESS.md`
//! "old records").
//!
//! Each one is a short seeded match sealed at archive time by an engine that
//! is now old -- the commit-reveal scheme did not exist yet, so they carry no
//! fairness material (they predate it: either the legacy `u64` seed of the
//! xoshiro stream, or a pinned `seed256` from the ChaCha switch but still
//! before the scheme). What this test pins:
//!
//! * they still parse, and their `check` holds;
//! * the stamp's `bundle` id is the file name -- what the loader routes on;
//! * the legacy `Init::Seed` path is intact (a seed of one kind or the other);
//! * `verify` reports "no fairness material" instead of pretending.
//!
//! A full checkpoint replay of an old ref against **today's** rules is not
//! expected to be clean once cards have drifted since the seal -- that is
//! exactly what the engine archive is for (the record plays on the bundle
//! that wrote it; `node tools/test-replay-archive.mjs` and
//! `node tools/archive-engine.mjs --check` are the behaviour / store halves).
//! The legacy stream itself is round-tripped in
//! `game-core/tests/fair.rs` (`legacy_u64_records_still_replay_on_the_legacy_stream`).

use std::path::PathBuf;

use game_core::fair::verify;
use game_core::record::{decode_record, parse_record, Init};

fn ref_records() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../archive/engine/refs");
    let mut out: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "bdrec"))
        .collect();
    out.sort();
    out
}

#[test]
fn every_reference_record_still_parses_and_names_its_bundle() {
    let refs = ref_records();
    assert!(!refs.is_empty(), "no reference records found");
    for path in refs {
        let bytes = std::fs::read(&path).expect("ref readable");
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let file = decode_record(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        // The body `check` re-verifies after a JSON round trip.
        let json = serde_json::to_string(&file).unwrap();
        parse_record(&json).unwrap_or_else(|e| panic!("{name}: reparse: {e}"));
        // The record names the bundle that wrote it -- what the loader routes
        // on (`docs/REPLAY.md` §9). The file name IS that id.
        let want = name.trim_end_matches(".bdrec");
        // An empty bundle is "unknown, never wrong" (a host that could not see
        // the glue identity sealed it); anything else must be the file name.
        assert!(
            file.header.engine.bundle == want || file.header.engine.bundle.is_empty(),
            "{name}: bundle id {:?} != {want}",
            file.header.engine.bundle
        );
        // Pre-scheme: no fairness material, and `verify` says so rather than
        // pretending (`docs/FAIRNESS.md`).
        let rep = verify(&file);
        assert!(!rep.present, "{name}: {:?}", rep.steps);
        assert!(!rep.ok, "{name}: {:?}", rep.steps);
        // Seeded, pre-fairness: the setup carries its entropy as the legacy
        // u64 `seed` (the xoshiro stream) or -- on a ChaCha-era ref sealed
        // before the commit-reveal scheme -- as a pinned `seed256`. In every
        // case `verify` above says there is no fairness material, which is
        // what "old records keep verifying" means (`docs/FAIRNESS.md` §1.4).
        match &file.body.init {
            Init::Seed(s) => {
                assert!(
                    s.seed256.is_some() || s.seed != 0,
                    "{name}: seeded record carries no seed"
                );
            }
            Init::Snapshot { .. } => {}
        }
    }
}
