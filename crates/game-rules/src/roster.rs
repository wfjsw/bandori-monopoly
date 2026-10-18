//! Roster metadata conversion (ABI v63, `docs/CARDS.md` 「Card metadata」).
//!
//! The ruleset exports a generic [`card_sdk::meta::MetaDoc`]; this module maps
//! it onto [`game_core::data::Roster`] -- the plain `CardData` / `PersonaData`
//! / `GroupData` / `EventData` vectors `GameData` iterates in roster order. The
//! field keys are this game's data-contract names (the roster row columns),
//! mirrored by `ruleset_core::roster::meta_key`.

use card_sdk::meta::{MetaDoc, MetaRow};
use game_core::data::{EventData, GroupData, CardData, PersonaData, Roster};

/// Decode a `ruleset_metadata` postcard blob into [`MetaDoc`].
pub fn decode_metadata(bytes: &[u8]) -> Result<MetaDoc, String> {
    postcard::from_bytes(bytes).map_err(|e| format!("metadata: {e}"))
}

/// Convert the generic rows into the game's roster vectors, one row kind each.
///
/// Rows arrive sorted by `(kind, ordinal)`; this function keeps that order so
/// the rebuilt vectors equal the roster order exactly. A kind this
/// converter does not know is ignored (the host may carry other lists).
///
/// Events ride `kind = "event"` (`rules_events::meta_rows`, bare ids). A
/// `kind = "card"` row whose id carries the `event:` rule-id prefix is the
/// same prose exported twice (`CardMeta::to_row` always tags `"card"`); those
/// duplicates are dropped so `Roster::cards` stays the physical deck pool.
pub fn roster_from_meta(doc: &MetaDoc) -> Roster {
    let mut cards = Vec::new();
    let mut personas = Vec::new();
    let mut groups = Vec::new();
    let mut events = Vec::new();
    for row in &doc.rows {
        match row.kind.as_str() {
            "card" => {
                if row.id.starts_with("event:") {
                    continue;
                }
                cards.push(card_from_row(row));
            }
            "persona" => personas.push(persona_from_row(row)),
            "group" => groups.push(group_from_row(row)),
            "event" => events.push(event_from_row(row)),
            _ => {}
        }
    }
    Roster {
        cards,
        personas,
        groups,
        events,
    }
}

fn card_from_row(r: &MetaRow) -> CardData {
    CardData {
        id: r.id.clone(),
        group: r.field("group").to_string(),
        name: r.name.clone(),
        owner: r.field("owner").to_string(),
        derived: r.derived,
        tags: r.tags.clone(),
        text: r.text.clone(),
    }
}

fn persona_from_row(r: &MetaRow) -> PersonaData {
    PersonaData {
        name: r.name.clone(),
        group: r.field("group").to_string(),
        skill: r.field("skill").to_string(),
        exclusive_cards: r.list("exclusiveCards").to_vec(),
        preset: r.list("preset").to_vec(),
        variant_of: r.field("variantOf").to_string(),
        text: r.text.clone(),
    }
}

fn group_from_row(r: &MetaRow) -> GroupData {
    GroupData {
        name: r.name.clone(),
        skill: r.field("skill").to_string(),
        shares_exclusives: r.flag("sharesExclusives"),
        text: r.text.clone(),
    }
}

/// One event card. The row id is the **bare** event id (`对邦`), not the
/// `event:<id>` rule id -- matching `EventData::id` and `display.json`.
fn event_from_row(r: &MetaRow) -> EventData {
    let id = r.id.strip_prefix("event:").unwrap_or(&r.id);
    EventData {
        id: id.to_string(),
        name: r.name.clone(),
        derived: r.derived,
        text: r.text.clone(),
    }
}