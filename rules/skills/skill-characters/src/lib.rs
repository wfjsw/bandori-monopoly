//! Character skills as card rules -- one file per skill, one crate for the lot.
//!
//! Deliberately **not** in `rules/cards/`: those are the cards you draw and
//! play, and these are the abilities a character comes with. Same rule language
//! ([`CardDef`]), same guest runtime, separate crate -- so a card author never
//! has to scroll past them and a skill author never edits a card.
//!
//! The rule text lives in `data/characters.json` (`skill` is the name, `text` /
//! `simple` the wording); what is here is the behavior. A skill that mandates a
//! [火罐] cap is the whole reason the keyed state carries bounds -- it writes
//! `state::set_bounds(player_id, state_key::FIRE, 0, 1)` and every card reads it
//! back with `state::max(player_id, state_key::FIRE)`. The engine holds the
//! number and enforces nothing; see `game_core::state::StateVar`.
//!
//! # Activation -- the one open shape
//!
//! A skill is not a card you play and not a card placed on the board, so none
//! of the existing `On` variants fit it:
//!
//! - `On::Play` fires from hand; a skill is never in hand.
//! - `On::Hook` fires for *placed* field cards (`wasm_rules` gates on
//!   `t.card`, i.e. the mark on the board). A skill is never placed, and it
//!   must not be removable the way a field card is.
//! - `On::React` is a [反击] window; a skill is always on.
//!
//! What is needed is an always-active variant -- `On::Skill(&[TriggerKind],
//! fn(player_id))` -- dispatched for every player whose character or band owns
//! the skill, with no placement gate and no removal. That is a new `OnKind`, a
//! manifest entry, and a dispatch arm; it is **not built yet**, so the table
//! below is empty rather than holding rules that would silently never run.
//!
//! The binding is the second half: a skill's [`CardDef`] id has to say which
//! character or band it belongs to (the proposed shape is
//! `skill:户山香澄:非凡之星` / `skill:Poppin' Party:星之鼓动`), and the engine
//! resolves a player's two skills from the character they picked and their
//! band. Both halves land together -- an id with no dispatcher is the
//! `plan::set_bonus` trap all over again.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use card_sdk::CardDef;

/// Every character skill. Empty until the `On::Skill` activation lands; see the
/// module docs. Adding one is a file in this directory plus an entry here.
pub const CARDS: &[CardDef] = &[];