//! Guard: every `FnId` whose first argument is a player seat must be listed
//! in `ir_fill::fn_takes_seat_arg0` and `ir_fill::seat_field_bits`. A vocab
//! rename that shifts discriminants would silently break the seat fill
//! (aya_01 VACUOUS regression) unless this test catches it.
//!
//! Dead discriminants (7/8/11/12/14/15/16) are reserved holes and must not
//! reappear. `CandHas = 34` and `CardIs = 35` are append-only.

use rules_cond_ir::ctx::{FnId, SeatField};

/// Live FnId variants, in declaration order (holes excluded). Numeric value
/// is the discriminant; the gaps are the reserved holes.
const LIVE_FN_IDS: &[FnId] = &[
    FnId::Slot,
    FnId::Tok,
    FnId::MoveTag,
    FnId::CardCounter,
    FnId::CounterIs,
    FnId::TileNamed,
    FnId::Money,
    FnId::Hand,
    FnId::Pos,
    FnId::Tiles,
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
    FnId::CandHas,
    FnId::CardIs,
];

/// Fns that read a seat's `SeatField` (must appear in `seat_field_bits`).
/// Sugar that used to map to `Fire`/`Crystals`/`Character`/`Band` now goes
/// through `State` and the `IrCtx.state` rows.
const SEAT_FIELD_FNS: &[(FnId, SeatField)] = &[
    (FnId::Money, SeatField::Money),
    (FnId::Hand, SeatField::Hand),
    (FnId::Pos, SeatField::Pos),
    (FnId::Tiles, SeatField::Tiles),
    (FnId::GainsThisTurn, SeatField::Gains),
    (FnId::TargetedCount, SeatField::Targeted),
    (FnId::Between, SeatField::Between),
];

/// Fns whose arg 0 is a player seat (must return true from
/// `fn_takes_seat_arg0`). `CandHas` is `(key, member)` — not a seat arg.
const SEAT_ARG0_FNS: &[FnId] = &[
    FnId::Money,
    FnId::Hand,
    FnId::Pos,
    FnId::Tiles,
    FnId::Neighbor,
    FnId::GainsThisTurn,
    FnId::TargetedCount,
    FnId::Between,
    FnId::State,
];

#[test]
fn fnid_discriminants_are_stable() {
    // The numeric values are load-bearing: generated cond_irs nodes store
    // the discriminant as `payload`. Reserved holes keep a stale table from
    // silently remapping.
    assert_eq!(FnId::Slot as i64, 0);
    assert_eq!(FnId::CounterIs as i64, 4);
    assert_eq!(FnId::Money as i64, 6);
    assert_eq!(FnId::Hand as i64, 9);
    assert_eq!(FnId::Pos as i64, 10);
    assert_eq!(FnId::Tiles as i64, 13);
    assert_eq!(FnId::Neighbor as i64, 17);
    assert_eq!(FnId::State as i64, 33);
    assert_eq!(FnId::CandHas as i64, 34);
    assert_eq!(FnId::CardIs as i64, 35);
    // Holes stay holes: the live list skips 7/8/11/12/14/15/16.
    let mut expected = vec![0i64, 1, 2, 3, 4, 5, 6, 9, 10, 13];
    expected.extend(17..=35);
    let got: Vec<i64> = LIVE_FN_IDS.iter().map(|f| *f as i64).collect();
    assert_eq!(got, expected, "FnId hole layout drifted");
}

#[test]
fn seat_field_bits_covers_every_seat_reading_fn() {
    let bit = |f: SeatField| 1u32 << (f as u32);
    for &(fid, field) in SEAT_FIELD_FNS {
        let expected = bit(field);
        assert_ne!(expected, 0, "SeatField::{field:?} must have a bit");
        let _ = fid;
    }
    // SeatField is the compact 11-wide layout.
    assert_eq!(SeatField::Between as usize, 10);
}

#[test]
fn seat_arg0_fns_are_all_covered() {
    for &fid in SEAT_ARG0_FNS {
        let d = fid as i64;
        assert!(
            (0..=35).contains(&d),
            "FnId::{fid:?} discriminant {d} out of range"
        );
    }
}

#[test]
fn no_stale_magic_numbers_in_ir_fill() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ir_fill.rs"),
    )
    .expect("read ir_fill.rs");
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
    assert!(
        src.contains("fn_takes_seat_arg0"),
        "ir_fill.rs must define fn_takes_seat_arg0"
    );
}
