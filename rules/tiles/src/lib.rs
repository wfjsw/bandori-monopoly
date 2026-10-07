//! Board **tile rules** -- what a board tile does when someone [结算]s it, and
//! the neutral marks that sit on the board.
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
//! | `mark:cp` | (none -- board-wide) | 125 (「CP点：放置于路面上的指示物」) |
//!
//! Every body quotes the passage it implements and cites it per line, like a
//! card. Gaps in the text are `TODO(规则书)`, never silently dropped.
//!
//! [`cp::MARK_CP`] is not a tile *kind* rule: it is the [CP点] tile-mark
//! **owner** (「放置于路面上的指示物」), one instance on the board owner with no
//! single tile of its own. Cards place / count / clear the **tile** [CP点]
//! through `ctx::place_cp` / `count_cp` / `clear_cp` -- the small API this
//! owner implements -- and the marks carry no player owner. The other [CP点]
//! kind is the **on-card** count (`FieldCard::cp`, 「自己[场上]N个[CP点]」),
//! the card rule's own stock (`ctx::add_cp` / `cp_attached` / `cp_at` /
//! `add_cp_at`) -- user ruling 2026-10-07.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod agent;
use agent::AGENT;
pub mod circle;
use circle::CIRCLE;
pub mod cp;
use cp::MARK_CP;
pub mod edogawa;
use edogawa::EDOGAWA;
pub mod event;
use event::EVENT;
pub mod property;
use property::PROPERTY;
pub mod ring;
use ring::RING;

/// Every tile rule, in registration order. The shipped module (`card-all`)
/// concatenates these tables with the card and skill ones.
pub static CARDS: &[card_sdk::CardDef] = &[EDOGAWA, EVENT, CIRCLE, AGENT, RING, PROPERTY, MARK_CP];