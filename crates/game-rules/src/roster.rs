//! Roster metadata conversion (ABI v63, `docs/CARDS.md` 「Card metadata」).
//!
//! The ruleset exports a generic [`card_sdk::meta::MetaDoc`]; this module maps
//! it onto [`game_core::data::Roster`] -- the plain `CardData` / `CharacterData`
//! / `BandData` vectors `GameData` iterates in roster order. The field keys are
//! this game's data-contract names (the roster row columns), mirrored by
//! `ruleset_core::roster::meta_key`.

use card_sdk::meta::{MetaDoc, MetaRow};
use game_core::data::{BandData, CardData, CharacterData, Roster};

/// Decode a `bandori_metadata` postcard blob into [`MetaDoc`].
pub fn decode_metadata(bytes: &[u8]) -> Result<MetaDoc, String> {
    postcard::from_bytes(bytes).map_err(|e| format!("metadata: {e}"))
}

/// Convert the generic rows into the game's roster vectors, one row kind each.
///
/// Rows arrive sorted by `(kind, ordinal)`; this function keeps that order so
/// the rebuilt vectors equal the roster order exactly. A kind this
/// converter does not know is ignored (the host may carry other lists).
pub fn roster_from_meta(doc: &MetaDoc) -> Roster {
    let mut cards = Vec::new();
    let mut characters = Vec::new();
    let mut bands = Vec::new();
    for row in &doc.rows {
        match row.kind.as_str() {
            "card" => cards.push(card_from_row(row)),
            "character" => characters.push(character_from_row(row)),
            "band" => bands.push(band_from_row(row)),
            _ => {}
        }
    }
    Roster {
        cards,
        characters,
        bands,
    }
}

fn card_from_row(r: &MetaRow) -> CardData {
    CardData {
        id: r.id.clone(),
        band: r.field("band").to_string(),
        name: r.name.clone(),
        raw_name: r.raw_name.clone(),
        owner: r.field("owner").to_string(),
        derived: r.derived,
        tags: r.tags.clone(),
        text: r.text.clone(),
        cell: r.cell.clone(),
    }
}

fn character_from_row(r: &MetaRow) -> CharacterData {
    CharacterData {
        name: r.name.clone(),
        display: r.field("display").to_string(),
        band: r.field("band").to_string(),
        color: r.field("color").to_string(),
        skill: r.field("skill").to_string(),
        text: r.text.clone(),
        cn_id: r.field("cnId").to_string(),
        art: r.field("art").to_string(),
        costume: r.field("costume").to_string(),
        exclusive_cards: r.list("exclusiveCards").to_vec(),
        preset: r.list("preset").to_vec(),
        variant_of: r.field("variantOf").to_string(),
        variant_tag: r.field("variantTag").to_string(),
    }
}

fn band_from_row(r: &MetaRow) -> BandData {
    BandData {
        name: r.name.clone(),
        short_name: r.field("shortName").to_string(),
        color: r.field("color").to_string(),
        skill: r.field("skill").to_string(),
        text: r.text.clone(),
        logo: r.field("logo").to_string(),
        shares_exclusives: r.flag("sharesExclusives"),
        boot_art: r.field("bootArt").to_string(),
    }
}