//! Kernel purity: `rules/cond` + `rules/cond-ir` library source must not
//! name game vocabulary. Content spellings live in the `CondExt` the ruleset
//! installs (`docs/GUARDS.md` §4.2b).

use std::path::Path;

/// Forbidden spellings. Matched as identifier segments (so `discharge` does
/// not hit `charge`, but `owner_charge` / `persona_is` do) and as exact
/// quoted tile-kind literals. `Exile` is the content kind; the engine status
/// is `SeatField::Exile` / `owner_exile` and is allowed.
const FORBIDDEN: &[&str] = &[
    "bandori",
    "crystals",
    "persona",
    "charge",
    "no_hand",
    "\"circle\"",
    "\"livehouse\"",
    "\"edogawa\"",
];

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// True when `word` appears in `src` as a whole identifier segment: bounded
/// by non-alphanumeric chars (`_` counts as a boundary so `persona_is` hits
/// `persona`), or by another identifier character only when that character is
/// `_`.
fn segment_hits(src: &str, word: &str) -> Vec<usize> {
    let hay = src.as_bytes();
    let needle = word.as_bytes();
    let mut hits = Vec::new();
    let mut i = 0;
    while i + needle.len() <= hay.len() {
        if &hay[i..i + needle.len()] == needle {
            let before_ok = i == 0 || !is_word_byte(hay[i - 1]) || hay[i - 1] == b'_';
            let after = i + needle.len();
            let after_ok = after >= hay.len() || !is_word_byte(hay[after]) || hay[after] == b'_';
            // Reject `discharge` (alnum before) but allow `owner_charge`
            // (`_` before) and `persona_is` (`_` after).
            let before_alnum = i > 0 && hay[i - 1].is_ascii_alphanumeric();
            let after_alnum = after < hay.len() && hay[after].is_ascii_alphanumeric();
            if before_ok && after_ok && !(before_alnum && after_alnum) {
                hits.push(i);
            }
        }
        i += 1;
    }
    hits
}

/// `Exile` is allowed as the engine status (`SeatField::Exile`, the
/// `SeatField::Exile` enum variant definition, `OwnerExile`) and forbidden as
/// the content kind spelling (`kind == Exile`).
fn exile_hits(src: &str) -> Vec<usize> {
    segment_hits(src, "Exile")
        .into_iter()
        .filter(|&pos| {
            let before = &src[..pos];
            if before.ends_with("SeatField::") {
                return false;
            }
            // Bare enum variant definition: the line is just `Exile,`.
            let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line = &src[line_start..pos + "Exile".len()];
            let trimmed = line.trim_start();
            if trimmed == "Exile" {
                let after = &src[pos + "Exile".len()..];
                if after.starts_with(',') || after.starts_with(' ') {
                    return false;
                }
            }
            true
        })
        .collect()
}

fn src_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for crate_dir in ["src"] {
        let dir = root.join(crate_dir);
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

#[test]
fn kernel_src_has_no_game_spellings() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut violations = Vec::new();
    for crate_root in [manifest.to_path_buf(), manifest.join("../cond-ir")] {
        for file in src_files(&crate_root) {
            let Ok(src) = std::fs::read_to_string(&file) else {
                continue;
            };
            for word in FORBIDDEN {
                for pos in segment_hits(&src, word) {
                    let line = src[..pos].matches('\n').count() + 1;
                    violations.push(format!("{}:{line}: `{word}`", file.display()));
                }
            }
            for pos in exile_hits(&src) {
                let line = src[..pos].matches('\n').count() + 1;
                violations.push(format!("{}:{line}: `Exile`", file.display()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "kernel source names game vocabulary:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn purity_matcher_covers_segments_not_substrings() {
    // `discharge` must not hit `charge`.
    assert!(segment_hits("let discharge = 1;", "charge").is_empty());
    // `owner_charge` / `persona_is` must hit.
    assert_eq!(segment_hits("owner_charge", "charge").len(), 1);
    assert_eq!(segment_hits("persona_is", "persona").len(), 1);
    // Standalone word.
    assert_eq!(segment_hits("charge(p)", "charge").len(), 1);
    // Quoted tile kind.
    assert_eq!(segment_hits("tile_is(t, \"circle\")", "\"circle\"").len(), 1);
    // `SeatField::Exile` is allowed; bare `Exile` is not.
    assert!(exile_hits("SeatField::Exile").is_empty());
    assert_eq!(exile_hits("kind == Exile").len(), 1);
    // `OwnerExile` is one identifier and does not segment-match `Exile`.
    assert!(segment_hits("OwnerExile", "Exile").is_empty());
}