//! `DeckRules.cs` + `DeckService.cs` -- 10-card decks, who may use which card,
//! and the 3 saved slots per character (slot 0 = the read-only preset).
//!
//! Note `clean` returns cards in **pool order**, not input order, and `fill` cleans
//! again after padding -- both exactly as in the C#.

use std::collections::HashSet;

use crate::data::{CardData, CharacterData, GameData};
use crate::msg::Msg;
use crate::profile::{DeckChoice, PlayerProfile, SavedDeck};

/// Cards per deck.
pub const SIZE: usize = 10;
/// Custom slots per character (`DeckService.Slots`).
pub const SLOTS: i32 = 3;

/// `DeckRules.WhyNot` -- `None` if `c` may put `card` in a starting deck.
pub fn cant_play(data: &GameData, c: &CharacterData, card: &CardData) -> Option<Msg> {
    if card.derived {
        return Some(Msg::new("err.deck_derived"));
    }
    if card.exclusive() && card.owner != c.name && !shares_exclusives(data, c, card) {
        return Some(Msg::new("err.deck_exclusive").chara("owner", card.owner.clone()));
    }
    if !card.general() && card.band != c.band {
        return Some(
            Msg::new("err.deck_band").arg("band", crate::msg::Arg::Band(card.band.clone())),
        );
    }
    None
}

pub fn can_use(data: &GameData, c: &CharacterData, card: &CardData) -> bool {
    cant_play(data, c, card).is_none()
}

/// Sumimi members may use each other's exclusive cards.
pub fn shares_exclusives(data: &GameData, c: &CharacterData, card: &CardData) -> bool {
    c.band == "Sumimi"
        && card.exclusive()
        && data
            .character(&card.owner)
            .is_some_and(|o| o.band == "Sumimi")
}

/// Cards `c` may use: exclusives first, then band cards, then general cards
/// (each group in `cards.json` order).
pub fn pool<'a>(data: &'a GameData, c: &CharacterData) -> Vec<&'a CardData> {
    let usable: Vec<&CardData> = data.cards.iter().filter(|k| can_use(data, c, k)).collect();
    let excl = usable.iter().copied().filter(|k| k.exclusive());
    let band = usable
        .iter()
        .copied()
        .filter(|k| !k.exclusive() && !k.general());
    let general = usable
        .iter()
        .copied()
        .filter(|k| !k.exclusive() && k.general());
    excl.chain(band).chain(general).collect()
}

/// Derived cards that can appear for `c` during a match (never in a starting deck).
pub fn derived<'a>(data: &'a GameData, c: &CharacterData) -> Vec<&'a CardData> {
    data.cards
        .iter()
        .filter(|k| k.derived && (k.general() || k.band == c.band))
        .collect()
}

/// Keep only usable ids, at most 10, in pool order.
pub fn clean<S: AsRef<str>>(data: &GameData, c: &CharacterData, ids: &[S]) -> Vec<String> {
    let wanted: HashSet<&str> = ids.iter().map(AsRef::as_ref).collect();
    pool(data, c)
        .into_iter()
        .filter(|k| wanted.contains(k.id.as_str()))
        .take(SIZE)
        .map(|k| k.id.clone())
        .collect()
}

/// `clean`, then pad from the pool up to 10.
pub fn fill<S: AsRef<str>>(data: &GameData, c: &CharacterData, ids: &[S]) -> Vec<String> {
    let mut list = clean(data, c, ids);
    for k in pool(data, c) {
        if list.len() >= SIZE {
            break;
        }
        if !list.contains(&k.id) {
            list.push(k.id.clone());
        }
    }
    clean(data, c, &list)
}

/// The designer's preset (`CharacterData.preset`), filled up to 10.
pub fn preset(data: &GameData, c: &CharacterData) -> Vec<String> {
    fill(data, c, &c.preset)
}

pub fn is_complete<S: AsRef<str>>(data: &GameData, c: &CharacterData, ids: &[S]) -> bool {
    clean(data, c, ids).len() == SIZE
}

// --- DeckService: saved slots on the profile -------------------------------------

fn find<'p>(p: &'p PlayerProfile, c: &CharacterData, slot: i32) -> Option<&'p SavedDeck> {
    p.decks
        .iter()
        .find(|d| d.character == c.name && d.slot == slot)
}

/// Deck in `slot`; slot 0 is the preset.
pub fn cards(data: &GameData, p: &PlayerProfile, c: &CharacterData, slot: i32) -> Vec<String> {
    if slot <= 0 {
        return preset(data, c);
    }
    find(p, c, slot)
        .map(|d| clean(data, c, &d.cards))
        .unwrap_or_default()
}

pub fn is_empty(data: &GameData, p: &PlayerProfile, c: &CharacterData, slot: i32) -> bool {
    slot > 0 && cards(data, p, c, slot).is_empty()
}

pub fn slot_complete(data: &GameData, p: &PlayerProfile, c: &CharacterData, slot: i32) -> bool {
    cards(data, p, c, slot).len() == SIZE
}

/// Save (or, if nothing usable remains, delete) a custom slot. Returns whether the
/// profile changed.
pub fn save<S: AsRef<str>>(
    data: &GameData,
    p: &mut PlayerProfile,
    c: &CharacterData,
    slot: i32,
    ids: &[S],
) -> bool {
    if slot <= 0 || slot > SLOTS {
        return false;
    }
    let list = clean(data, c, ids);
    let pos = p
        .decks
        .iter()
        .position(|d| d.character == c.name && d.slot == slot);
    match (pos, list.is_empty()) {
        (None, true) => false,
        (None, false) => {
            p.decks.push(SavedDeck {
                character: c.name.clone(),
                slot,
                cards: list,
            });
            true
        }
        (Some(i), true) => {
            p.decks.remove(i);
            true
        }
        (Some(i), false) => {
            p.decks[i].cards = list;
            true
        }
    }
}

pub fn first_empty_slot(data: &GameData, p: &PlayerProfile, c: &CharacterData) -> i32 {
    (1..=SLOTS).find(|&s| is_empty(data, p, c, s)).unwrap_or(0)
}

pub fn custom_count(data: &GameData, p: &PlayerProfile, c: &CharacterData) -> i32 {
    (1..=SLOTS).filter(|&s| !is_empty(data, p, c, s)).count() as i32
}

/// Slot chosen last time, if it still holds a complete deck; else 0 (preset).
pub fn chosen_slot(data: &GameData, p: &PlayerProfile, c: &CharacterData) -> i32 {
    match p.deck_choices.iter().find(|d| d.character == c.name) {
        Some(d) if d.slot > 0 && d.slot <= SLOTS && slot_complete(data, p, c, d.slot) => d.slot,
        _ => 0,
    }
}

/// Remember the chosen slot. Returns whether the profile changed.
pub fn choose(p: &mut PlayerProfile, c: &CharacterData, slot: i32) -> bool {
    match p.deck_choices.iter_mut().find(|d| d.character == c.name) {
        Some(d) if d.slot == slot => false,
        Some(d) => {
            d.slot = slot;
            true
        }
        None => {
            p.deck_choices.push(DeckChoice {
                character: c.name.clone(),
                slot,
            });
            true
        }
    }
}
