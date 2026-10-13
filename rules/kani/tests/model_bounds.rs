//! Loop-bound and index-table soundness for the tuned symbolic model.
//!
//! These pin the compile-time tables that replaced linear scans:
//! `board::TILE_NAME_HASHES` (FNV binary search) and `invoke::BAND_START`
//! (const band offsets). A drift would make `tile_named` / `by_idx` return
//! the wrong entry — a soundness bug, not a performance one.

// Force-link `game_rules::native_shims` so the `bandori_*` C symbols that
// `card-sdk`'s `guest` feature declares resolve at test-binary link time.
// (Same trick conf_model uses by calling `install_host`.)
#[test]
fn native_shims_are_linked() {
    // Referencing any symbol from the rlib pulls the `#[no_mangle]` shims in.
    let f = game_rules::native_shims::take_host as *const ();
    assert!(!f.is_null());
}

#[test]
fn board_hash_table_is_sound() {
    use rules_kani::board::{
        tile_named, tile_named_bytes, TILE_NAME_HASHES, TILE_NAMES,
    };
    for i in 0..60 {
        assert_eq!(
            tile_named(TILE_NAMES[i]),
            i as i32,
            "TILE_NAMES[{i}] = {:?} did not resolve to itself",
            TILE_NAMES[i]
        );
        assert_eq!(
            tile_named_bytes(TILE_NAMES[i].as_bytes()),
            i as i32,
            "bytes path mismatch at {i}"
        );
    }
    for w in TILE_NAME_HASHES.windows(2) {
        assert!(w[0] < w[1], "TILE_NAME_HASHES not strictly sorted");
    }
    assert_eq!(tile_named("no such tile"), -1);
    assert_eq!(tile_named(""), -1);
}

#[test]
fn band_starts_match_ruleset() {
    use rules_kani::invoke::{by_idx, ruleset_len, BAND_START, RULESET};
    assert_eq!(BAND_START.len(), RULESET.len() + 1);
    let mut acc = 0usize;
    for (b, band) in RULESET.iter().enumerate() {
        assert_eq!(BAND_START[b], acc, "band {b} start drifted");
        acc += band.len();
    }
    assert_eq!(BAND_START[RULESET.len()], acc);
    assert_eq!(acc, rules_kani::generated::card_ids::CARD_COUNT);
    assert_eq!(ruleset_len(), acc);
    for i in 0..acc {
        let c = by_idx(i);
        assert!(!c.id.is_empty(), "idx {i} has empty id");
    }
}

#[test]
fn intern_fixed_loop_bounds() {
    // card-sdk intern: open-addressed, 8 probes, 8×8 byte walks.
    card_sdk::intern::reset();
    let a = card_sdk::intern::intern("PP:[丸山彩]憧憬的前方");
    let b = card_sdk::intern::intern("PP:[丸山彩]憧憬的前方");
    assert_eq!(a, b);
    assert_eq!(
        card_sdk::intern::resolve_str(a),
        "PP:[丸山彩]憧憬的前方"
    );
    let long = "x".repeat(52);
    assert!(card_sdk::intern::intern(&long).is_valid());
    let too_long = "y".repeat(card_sdk::intern::MAX_NAME + 1);
    assert!(!card_sdk::intern::intern(&too_long).is_valid());
}

/// Kani intern identity is `(FNV-1a, len)` (bytes_eq dropped from the probe
/// hit — it was the money_shave setup cliff). This pins the proof corpus:
/// every pilot/known name must intern to a distinct `NameId`, so a 64-bit
/// collision cannot silently alias two of them.
#[test]
fn pilot_intern_hashes_are_unique() {
    use rules_kani::host::PILOT_INTERN_NAMES;
    card_sdk::intern::reset();
    let mut seen = std::collections::BTreeSet::new();
    for name in PILOT_INTERN_NAMES {
        let id = card_sdk::intern::intern(name);
        assert!(id.is_valid(), "pilot name {name:?} failed to intern");
        assert!(
            seen.insert(id),
            "pilot name {name:?} collided with an earlier name (hash+len alias)"
        );
        assert_eq!(
            card_sdk::intern::resolve_str(id),
            *name,
            "intern/resolve drift for {name:?}"
        );
    }
}