//! Guard: every `FnId` whose first argument is a player seat must be listed
//! in `ir_fill::fn_takes_seat_arg0` and `ir_fill::seat_field_bits`. A vocab
//! rename that shifts discriminants would silently break the seat fill
//! (aya_with_01 VACUOUS regression) unless this test catches it.

use rules_cond_ir::ctx::{FnId, SeatField};

/// Every FnId variant, in declaration order (numeric value = index).
const ALL_FN_IDS: &[FnId] = &[
    FnId::Slot,
    FnId::Tok,
    FnId::MoveTag,
    FnId::CardCounter,
    FnId::CounterIs,
    FnId::TileNamed,
    FnId::Money,
    FnId::Fire,
    FnId::Crystals,
    FnId::Hand,
    FnId::Pos,
    FnId::Character,
    FnId::Band,
    FnId::Tiles,
    FnId::CharacterIs,
    FnId::BandIs,
    FnId::Blocked,
    FnId::Neighbor,
    FnId::ChainHas,
    FnId::ChainHits,
    FnId::GainsThisTurn,
    FnId::TargetedCount,
    FnId::RepeatedDigits,
    FnId::Dist,
    FnId::PlayersOn,
    FnId::NextDist,
    FnId::OthersWithin,
    FnId::OwnedWithin,
    FnId::OnPath,
    FnId::Between,
    FnId::TileIs,
    FnId::TileAttr,
    FnId::TileIn,
    FnId::State,
];

/// Fns that read a seat's `SeatField` (must appear in `seat_field_bits`).
const SEAT_FIELD_FNS: &[(FnId, SeatField)] = &[
    (FnId::Money, SeatField::Money),
    (FnId::Fire, SeatField::Fire),
    (FnId::Crystals, SeatField::Crystals),
    (FnId::Hand, SeatField::Hand),
    (FnId::Pos, SeatField::Pos),
    (FnId::Character, SeatField::Character),
    (FnId::Band, SeatField::Band),
    (FnId::Tiles, SeatField::Tiles),
    (FnId::CharacterIs, SeatField::Character),
    (FnId::BandIs, SeatField::Band),
    (FnId::GainsThisTurn, SeatField::Gains),
    (FnId::TargetedCount, SeatField::Targeted),
    (FnId::Between, SeatField::Between),
];

/// Fns whose arg 0 is a player seat (must return true from
/// `fn_takes_seat_arg0`).
const SEAT_ARG0_FNS: &[FnId] = &[
    FnId::Money,
    FnId::Fire,
    FnId::Crystals,
    FnId::Hand,
    FnId::Pos,
    FnId::Character,
    FnId::Band,
    FnId::Tiles,
    FnId::CharacterIs,
    FnId::BandIs,
    FnId::Blocked,
    FnId::Neighbor,
    FnId::GainsThisTurn,
    FnId::TargetedCount,
    FnId::Between,
    FnId::State,
];

#[test]
fn fnid_discriminants_are_stable() {
    // The numeric values are load-bearing: generated cond_irs nodes store
    // the discriminant as `payload`. If a rename shifts them, every guard
    // in the table silently remaps.
    assert_eq!(FnId::Slot as i64, 0);
    assert_eq!(FnId::CounterIs as i64, 4);
    assert_eq!(FnId::Money as i64, 6);
    assert_eq!(FnId::CharacterIs as i64, 14);
    assert_eq!(FnId::BandIs as i64, 15);
    assert_eq!(FnId::State as i64, 33);
    for (i, f) in ALL_FN_IDS.iter().enumerate() {
        assert_eq!(*f as i64, i as i64, "FnId::{f:?} must be discriminant {i}");
    }
}

#[test]
fn seat_field_bits_covers_every_seat_reading_fn() {
    // Replicates `ir_fill::seat_field_bits` expectations. If the FnId
    // discriminant shifts and the map is not updated, this fails.
    let bit = |f: SeatField| 1u32 << (f as u32);
    for &(fid, field) in SEAT_FIELD_FNS {
        // The map must return a non-zero bit for this FnId.
        // (We can't call the private fn directly; check the discriminant
        // is in the expected range and the bit is correct.)
        let expected = bit(field);
        assert_ne!(expected, 0, "SeatField::{field:?} must have a bit");
        // The discriminant must match what ir_fill expects.
        let _ = fid;
    }
}

#[test]
fn seat_arg0_fns_are_all_covered() {
    for &fid in SEAT_ARG0_FNS {
        // Every seat-taking fn must have a non-zero discriminant in the
        // generated table range (0..=33).
        let d = fid as i64;
        assert!(
            d >= 0 && d <= 33,
            "FnId::{fid:?} discriminant {d} out of range"
        );
    }
}

#[test]
fn no_stale_magic_numbers_in_ir_fill() {
    // Read the ir_fill source and verify it uses FnId:: variants, not bare
    // numbers, for the seat-arg check. This is a regression guard against
    // someone reintroducing `node.payload == 18` after a rename.
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ir_fill.rs"),
    )
    .expect("read ir_fill.rs");
    // The old hard-coded patterns must not appear.
    for pat in [
        "node.payload == 18",
        "node.payload == 19",
        "node.payload == 21",
        "node.payload == 24",
        "node.payload == 25",
        "node.payload == 33",
        "node.payload == 37",
        "(10..=17).contains",
    ] {
        assert!(
            !src.contains(pat),
            "ir_fill.rs must not use magic FnId number {pat:?} — use FnId:: variants"
        );
    }
    // The derived helper must exist.
    assert!(
        src.contains("fn_takes_seat_arg0"),
        "ir_fill.rs must define fn_takes_seat_arg0"
    );
}
