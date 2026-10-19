//! **Tile-mark rules** -- the `mark:*` category: what a neutral board mark is
//! and what it does when someone interacts with it.
//!
//! Standardized the same way card, skill, tile and event rules are: one
//! [`CardDef`] per mark category, bound **once** at match start to the
//! **neutral board owner** (no single tile, `tile = -1`; [`docs/TILES.md`]).
//! A mark owner is not a tile *kind* rule: it governs the marks that sit on
//! many tiles, and its hooks hear every trigger of that kind wherever the
//! trigger points (`game_core::data::mark_rule_ids` -> `bind_tiles`).
//!
//! | id | file | rulebook |
//! |---|---|---|
//! | `mark:cp` | `cp.rs` | `data/rules.txt` 125 (「CP点：放置于路面上的指示物」) + 通用:该清CP了 |
//!
//! Every body quotes the passage it implements and cites it per line, like a
//! card. Gaps in the text are `TODO(规则书)`, never silently dropped.
//!
//! ## Bound-counter model (ruling 2026-10-10)
//!
//! Every tile mark is a **unit of a named counter** on the rule instance that
//! created it -- a real card instance, or the standing `mark:*` pseudo card
//! that owns the category. There are no orphan marks: when the owning instance
//! dies (discard, unplace, graveyard, player out, event expiry) every unit it
//! owns disappears with it.
//!
//! `mark:cp` is that standing owner for [CP点]. Under the ruling:
//!
//! * The **tile** [CP点] (「放置于路面上的指示物」) are units of `mark:cp`'s own
//!   named counter [`card_sdk::abi::counter::CP`], bound to tiles. They live
//!   as long as the match does -- `mark:cp` is present for the whole game and
//!   is associated with nobody.
//! * **Provenance** (`TileMark.src`) is the 该清CP了 instance that placed them
//!   (what 「此卡在格子上添加的[CP点]及其产物」 keys on). It is *not* ownership:
//!   the marks survive that card and its player.
//! * The **on-card** [CP点] (「自己[场上]N个[CP点]」) is 该清CP了's own
//!   [`counter::CP`] counter and dies with that card. `mark:cp` only spends it
//!   (the settle clause).
//!
//! So `mark:cp` carries *what a CP point does on landing*; the units are its
//! counter, not the placing card's. Cards talk to the owner through the
//! generic cross-instance channel (`ctx::send` -> [`Target::Board`], or the
//! engine-level mark verbs the owner implements) -- never a CP special case
//! outside this crate. See [`cp`] for the clause-by-clause reading.
//!
//! [`Target::Board`]: card_sdk::abi::Target::Board

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod cp;
pub use cp::MARK_CP;

/// Every tile-mark rule, in registration order. The shipped module
/// (`card-all`) concatenates these tables with the card, skill, tile and event
/// ones.
pub static CARDS: &[card_sdk::CardDef] = &[MARK_CP];
