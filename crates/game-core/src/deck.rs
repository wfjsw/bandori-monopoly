//! 10-card decks: who may use which card, and the named saved decks per
//! character (id 0 = the read-only preset).
//!
//! Note `clean` returns cards in **pool order**, not input order, and `fill`
//! cleans again after padding -- so a filled deck's order is always the
//! pool's, never the submitter's.

use std::collections::HashSet;

use crate::data::{CardData, CharacterData, GameData};
use crate::msg::Msg;
use crate::profile::{sanitize_deck_name, DeckChoice, PlayerProfile, SavedDeck};

/// Cards per deck.
pub const SIZE: usize = 10;
/// Saved custom decks per character -- an arbitrary number, bounded so one
/// profile cannot grow without limit.
pub const DECKS_MAX: usize = 100;

/// `None` if `c` may put `card` in a starting deck.
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

/// The designer's preset (`CharacterData::preset`), filled up to 10.
pub fn preset(data: &GameData, c: &CharacterData) -> Vec<String> {
    fill(data, c, &c.preset)
}

/// A random legal deck: `SIZE` cards drawn without replacement from [`pool`],
/// then put back in pool order. The caller supplies the index picker (the
/// match RNG -- never `thread_rng`, so a chaos bot stays replayable). Falls
/// short of [`SIZE`] only when the pool itself is smaller; the caller then
/// falls back to [`preset`].
pub fn random(
    data: &GameData,
    c: &CharacterData,
    mut below: impl FnMut(usize) -> usize,
) -> Vec<String> {
    let mut ids: Vec<String> = pool(data, c).into_iter().map(|k| k.id.clone()).collect();
    let mut out = Vec::with_capacity(SIZE);
    while out.len() < SIZE && !ids.is_empty() {
        let k = below(ids.len()).min(ids.len() - 1);
        out.push(ids.remove(k));
    }
    clean(data, c, &out)
}

pub fn is_complete<S: AsRef<str>>(data: &GameData, c: &CharacterData, ids: &[S]) -> bool {
    clean(data, c, ids).len() == SIZE
}

// --- DeckService: named saved decks on the profile --------------------------------

fn find<'p>(p: &'p PlayerProfile, c: &CharacterData, id: i32) -> Option<&'p SavedDeck> {
    p.decks
        .iter()
        .find(|d| d.character == c.name && d.slot == id)
}

/// Saved custom decks of `c`, in display order. Id 0 (the preset) is not one.
pub fn list<'p>(p: &'p PlayerProfile, c: &CharacterData) -> Vec<&'p SavedDeck> {
    p.decks.iter().filter(|d| d.character == c.name).collect()
}

/// Next free deck id for `c` (one past its highest), or `None` at [`DECKS_MAX`].
fn next_id(p: &PlayerProfile, c: &CharacterData) -> Option<i32> {
    let decks = list(p, c);
    if decks.len() >= DECKS_MAX {
        return None;
    }
    Some(decks.iter().map(|d| d.slot).max().unwrap_or(0) + 1)
}

/// Deck in `id`; id 0 is the preset.
pub fn cards(data: &GameData, p: &PlayerProfile, c: &CharacterData, id: i32) -> Vec<String> {
    if id <= 0 {
        return preset(data, c);
    }
    find(p, c, id)
        .map(|d| clean(data, c, &d.cards))
        .unwrap_or_default()
}

pub fn is_empty(data: &GameData, p: &PlayerProfile, c: &CharacterData, id: i32) -> bool {
    id > 0 && cards(data, p, c, id).is_empty()
}

pub fn slot_complete(data: &GameData, p: &PlayerProfile, c: &CharacterData, id: i32) -> bool {
    cards(data, p, c, id).len() == SIZE
}

/// Save cards into an existing custom deck. An empty list clears the deck -- it
/// stays, with its name; [`delete`] removes it. Returns whether the profile
/// changed.
pub fn save<S: AsRef<str>>(
    data: &GameData,
    p: &mut PlayerProfile,
    c: &CharacterData,
    id: i32,
    ids: &[S],
) -> bool {
    let Some(d) = p
        .decks
        .iter_mut()
        .find(|d| d.character == c.name && d.slot == id)
    else {
        return false;
    };
    let list = clean(data, c, ids);
    if d.cards == list {
        return false;
    }
    d.cards = list;
    true
}

/// Create a custom deck from `ids` (cleaned; may be empty) under `name`
/// ([`sanitize_deck_name`]; empty = auto). Appended after `c`'s other decks.
/// Returns the new id, or `None` at [`DECKS_MAX`].
pub fn create<S: AsRef<str>>(
    data: &GameData,
    p: &mut PlayerProfile,
    c: &CharacterData,
    name: &str,
    ids: &[S],
) -> Option<i32> {
    let id = next_id(p, c)?;
    let cards = clean(data, c, ids);
    let at = p
        .decks
        .iter()
        .rposition(|d| d.character == c.name)
        .map(|i| i + 1)
        .unwrap_or(p.decks.len());
    p.decks.insert(
        at,
        SavedDeck {
            character: c.name.clone(),
            slot: id,
            name: sanitize_deck_name(name),
            cards,
        },
    );
    Some(id)
}

/// Copy deck `id` to a new deck right after it (auto-named). Returns the new
/// id, or `None` if `id` is missing or at [`DECKS_MAX`].
pub fn duplicate(data: &GameData, p: &mut PlayerProfile, c: &CharacterData, id: i32) -> Option<i32> {
    let at = p
        .decks
        .iter()
        .position(|d| d.character == c.name && d.slot == id)?;
    let new_id = next_id(p, c)?;
    let cards = clean(data, c, &p.decks[at].cards);
    p.decks.insert(
        at + 1,
        SavedDeck {
            character: c.name.clone(),
            slot: new_id,
            name: String::new(),
            cards,
        },
    );
    Some(new_id)
}

/// Rename deck `id` ([`sanitize_deck_name`]; empty restores the auto name).
/// Duplicates are allowed. Returns whether the profile changed.
pub fn rename(p: &mut PlayerProfile, c: &CharacterData, id: i32, name: &str) -> bool {
    let Some(d) = p
        .decks
        .iter_mut()
        .find(|d| d.character == c.name && d.slot == id)
    else {
        return false;
    };
    let name = sanitize_deck_name(name);
    if d.name == name {
        return false;
    }
    d.name = name;
    true
}

/// Delete deck `id`. A chosen default pointing at it falls back to the preset.
/// Returns whether the profile changed.
pub fn delete(p: &mut PlayerProfile, c: &CharacterData, id: i32) -> bool {
    let Some(i) = p
        .decks
        .iter()
        .position(|d| d.character == c.name && d.slot == id)
    else {
        return false;
    };
    p.decks.remove(i);
    p.deck_choices
        .retain(|d| !(d.character == c.name && d.slot == id));
    true
}

/// Move deck `id` by `delta` places in `c`'s display order (-1 up, +1 down).
/// Returns whether the profile changed.
pub fn move_by(p: &mut PlayerProfile, c: &CharacterData, id: i32, delta: i32) -> bool {
    if delta == 0 {
        return false;
    }
    let idxs: Vec<usize> = p
        .decks
        .iter()
        .enumerate()
        .filter(|(_, d)| d.character == c.name)
        .map(|(i, _)| i)
        .collect();
    let Some(pos) = idxs.iter().position(|&i| p.decks[i].slot == id) else {
        return false;
    };
    let to = pos as i32 + delta;
    if to < 0 || to as usize >= idxs.len() {
        return false;
    }
    let (a, b) = (idxs[pos], idxs[to as usize]);
    p.decks.swap(a, b);
    true
}

/// Id chosen last time, if it still holds a complete deck; else 0 (preset).
pub fn chosen_slot(data: &GameData, p: &PlayerProfile, c: &CharacterData) -> i32 {
    match p.deck_choices.iter().find(|d| d.character == c.name) {
        Some(d) if d.slot > 0 && slot_complete(data, p, c, d.slot) => d.slot,
        _ => 0,
    }
}

/// Remember the chosen deck (0 = preset). Returns whether the profile changed.
pub fn choose(p: &mut PlayerProfile, c: &CharacterData, id: i32) -> bool {
    match p.deck_choices.iter_mut().find(|d| d.character == c.name) {
        Some(d) if d.slot == id => false,
        Some(d) => {
            d.slot = id;
            true
        }
        None => {
            p.deck_choices.push(DeckChoice {
                character: c.name.clone(),
                slot: id,
            });
            true
        }
    }
}
