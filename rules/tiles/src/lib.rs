//! Board **tile rules** -- what a board tile does when someone [结算]s it.
//!
//! Standardized the same way card and skill rules are: one [`CardDef`] per
//! tile kind, bound to every board tile of that kind at match start on a
//! **neutral board owner** ([`docs/TILES.md`]). A tile body is thin -- the
//! engine keeps rent / buy / build / draw-event as `ctx` primitives -- so a
//! card that bends a tile attaches, swaps or retunes rule instances instead of
//! poking an engine flag.
//!
//! | id | board kinds | rulebook |
//! |---|---|---|
//! | `tile:property` | `property` | `data/rules.txt` 100–106 |
//! | `tile:ring` | `ring` | 102 (「所有RiNG不可升级」) + TODO |
//! | `tile:agent` | `agent` | 107 |
//! | `tile:circle` | `circle` | 95–96 |
//! | `tile:edogawa` | `edogawa` | 95 |
//! | `tile:event` | `cafe`, `ryuseido` | 97–99 |
//!
//! Every body quotes the passage it implements and cites it per line, like a
//! card. Gaps in the text are `TODO(规则书)`, never silently dropped.
//!
//! Board-wide **tile marks** (`mark:*`, e.g. the [CP点] owner `mark:cp`) are
//! a sibling category, not a tile kind: see `rules/tile_marks` (one crate,
//! one `CardDef` per mark category, bound once on the board owner).

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod agent;
use agent::AGENT;
pub mod circle;
use circle::CIRCLE;
pub mod edogawa;
use edogawa::EDOGAWA;
pub mod event;
use event::EVENT;
pub mod property;
use property::PROPERTY;
pub mod ring;
use ring::RING;

/// Every tile rule, in registration order. The shipped module (`card-all`)
/// concatenates these tables with the card, skill, tile-mark and event ones.
pub static CARDS: &[card_sdk::CardDef] = &[EDOGAWA, EVENT, CIRCLE, AGENT, RING, PROPERTY];
