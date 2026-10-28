//! Tile-**[CP点]** mark vocabulary.
//!
//! A mark's `kind` is its identity and its `category` groups marks for the
//! `MarkFilter` lookups; both of these are this ruleset's words, so they live
//! here rather than in `card-sdk`. The generic mark machinery (the verbs, the
//! filter type, `NO_TARGET`, the designation category) stays in
//! `card_sdk::abi::mark` / `card_sdk::ctx`.

/// Tile-mark **category**: [CP点], 「放置于路面上的指示物」 (`data/rules.txt`
/// 125). Mirrors `game_core::state::mark_category::CP`. The tile [CP点] are
/// units of the standing `mark:cp` pseudo card's [`counter::CP`] counter,
/// bound to tiles.
pub const CP_CATEGORY: &str = "cp";

/// The [CP点] mark's stable `kind`. Mirrors `game_core::state::mark_kind::CP`.
pub const CP_KIND: &str = "mark:cp";
