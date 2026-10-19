//! Shared CEL guards this ruleset repeats on many entries.
//!
//! Conditions are generic (`tile_is` / `tile_in` / `tile_attr`); the kind and
//! class strings come from [`crate::kind`].

/// `actor == owner && tile_is(tile.id, \"circle\")` -- the owner passed the
/// start square.
pub const OWN_START: &str = "actor == owner && tile_is(tile.id, \"circle\")";

/// `actor == owner && tile_is(tile.id, \"ring\")` -- the owner passed a flat deed.
pub const OWN_DEED: &str = "actor == owner && tile_is(tile.id, \"ring\")";
