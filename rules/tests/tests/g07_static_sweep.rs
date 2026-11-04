//! G07 §10.3 static sweep.
//!
//! The bond's grant is a plain call after a placement and the nearest query is
//! a host call, neither of which the compiler ties to the clause. This test
//! reads the card and skill sources as text and pins:
//!
//! * a file that places a **waiver** card (`无视【距离最近】` in the clause)
//!   grants the waiver on the returned uid (`nearest::grant(` after the
//!   placement), so the instance carries it;
//! * a file that hand-rolls a `min` / `filter == best` over `ctx::dist` /
//!   `ctx::tile_forward` carries a `TODO(ABI)` naming `ctx::nearest` until it
//!   is migrated;
//! * no card body writes the key string `"nearestExemption"` by hand -- the
//!   `bandori_std::nearest::EXEMPTION` constant is the one source.

use std::path::{Path, PathBuf};

fn sources(root: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(root);
    let mut out = Vec::new();
    for band in std::fs::read_dir(&root).expect("rules dir") {
        let band = band.expect("band entry").path().join("src");
        let Ok(entries) = std::fs::read_dir(&band) else {
            continue;
        };
        for e in entries {
            let p = e.expect("source entry").path();
            if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn all_sources() -> Vec<PathBuf> {
    let mut v = sources("../cards");
    v.extend(sources("../skills"));
    v.sort();
    v
}

/// Every placement in a waiver file is followed by a grant on its answer.
#[test]
fn g07_waiver_placements_grant_on_the_uid() {
    let mut waived = 0;
    for p in all_sources() {
        let src = std::fs::read_to_string(&p).expect("source");
        if !src.contains("无视【距离最近】") {
            continue;
        }
        waived += 1;
        for (i, line) in src.lines().enumerate() {
            if !line.contains("place_card_at(") || line.trim_start().starts_with("//") {
                continue;
            }
            let tail: String = src.lines().skip(i).take(12).collect();
            assert!(
                tail.contains("nearest::grant("),
                "{}:{}: a waiver placement must grant on its uid",
                p.display(),
                i + 1
            );
        }
    }
    assert!(waived >= 1, "the waiver card is found");
}

/// Hand-rolled nearest selection carries the migration note.
#[test]
fn g07_hand_rolled_nearest_selection_is_tagged() {
    for p in all_sources() {
        let src = std::fs::read_to_string(&p).expect("source");
        let distance = src.contains("ctx::dist(") || src.contains("ctx::tile_forward(");
        let selection = src.contains("best") || src.contains(".max()");
        if !distance || !selection || src.contains("ctx::nearest(") {
            continue;
        }
        assert!(
            src.contains("TODO(ABI)") && src.contains("nearest"),
            "{}: a hand-rolled nearest selection needs `TODO(ABI)` naming `ctx::nearest`",
            p.display()
        );
    }
}

/// The key literal lives in one place.
#[test]
fn g07_exemption_key_is_not_spelled_by_hand() {
    for p in all_sources() {
        let src = std::fs::read_to_string(&p).expect("source");
        assert!(
            !src.contains("\"nearestExemption\""),
            "{}: use `bandori_std::nearest::EXEMPTION`",
            p.display()
        );
    }
}
