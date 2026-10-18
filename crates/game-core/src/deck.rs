//! 10-card deck-construction rules only: who may use which card, and the
//! pool / clean / fill / preset / random helpers. Named saved decks live in
//! `player_meta::decks`.
//!
//! Note `clean` returns cards in **pool order**, not input order, and `fill`
//! cleans again after padding -- so a filled deck's order is always the
//! pool's, never the submitter's.

use std::collections::HashSet;

use crate::data::{CardData, PersonaData, GameData};
use crate::msg::Msg;

/// Cards per deck.
pub const SIZE: usize = 10;

/// `None` if `c` may put `card` in a starting deck.
pub fn cant_play(data: &GameData, c: &PersonaData, card: &CardData) -> Option<Msg> {
    if card.derived {
        return Some(Msg::new("err.deck_derived"));
    }
    if card.exclusive() && card.owner != c.name && !shares_exclusives(data, c, card) {
        return Some(Msg::new("err.deck_exclusive").persona("owner", card.owner.clone()));
    }
    if !card.is_universal() && card.group != c.group {
        return Some(
            Msg::new("err.deck_group").arg("group", crate::msg::Arg::Group(card.group.clone())),
        );
    }
    None
}

pub fn can_use(data: &GameData, c: &PersonaData, card: &CardData) -> bool {
    cant_play(data, c, card).is_none()
}

/// Members of a group with the `shares_exclusives` trait may use each
/// other's exclusive cards.
pub fn shares_exclusives(data: &GameData, c: &PersonaData, card: &CardData) -> bool {
    data.group_shares_exclusives(c)
        && card.exclusive()
        && data.persona(&card.owner).is_some_and(|o| o.group == c.group)
}

/// Cards `c` may use: exclusives first, then group cards, then general cards
/// (each group in the roster's canonical deck-pool order).
pub fn pool<'a>(data: &'a GameData, c: &PersonaData) -> Vec<&'a CardData> {
    let usable: Vec<&CardData> = data.cards.iter().filter(|k| can_use(data, c, k)).collect();
    let excl = usable.iter().copied().filter(|k| k.exclusive());
    let group_cards = usable
        .iter()
        .copied()
        .filter(|k| !k.exclusive() && !k.is_universal());
    let general = usable
        .iter()
        .copied()
        .filter(|k| !k.exclusive() && k.is_universal());
    excl.chain(group_cards).chain(general).collect()
}

/// Derived cards that can appear for `c` during a match (never in a starting deck).
pub fn derived<'a>(data: &'a GameData, c: &PersonaData) -> Vec<&'a CardData> {
    data.cards
        .iter()
        .filter(|k| k.derived && (k.is_universal() || k.group == c.group))
        .collect()
}

/// Keep only usable ids, at most 10, in pool order.
pub fn clean<S: AsRef<str>>(data: &GameData, c: &PersonaData, ids: &[S]) -> Vec<String> {
    let wanted: HashSet<&str> = ids.iter().map(AsRef::as_ref).collect();
    pool(data, c)
        .into_iter()
        .filter(|k| wanted.contains(k.id.as_str()))
        .take(SIZE)
        .map(|k| k.id.clone())
        .collect()
}

/// `clean`, then pad from the pool up to 10.
pub fn fill<S: AsRef<str>>(data: &GameData, c: &PersonaData, ids: &[S]) -> Vec<String> {
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

/// The designer's preset (`PersonaData::preset`), filled up to 10.
pub fn preset(data: &GameData, c: &PersonaData) -> Vec<String> {
    fill(data, c, &c.preset)
}

/// A random legal deck: `SIZE` cards drawn without replacement from [`pool`],
/// then put back in pool order. The caller supplies the index picker (the
/// match RNG -- never `thread_rng`, so a chaos bot stays replayable). Falls
/// short of [`SIZE`] only when the pool itself is smaller; the caller then
/// falls back to [`preset`].
pub fn random(
    data: &GameData,
    c: &PersonaData,
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

pub fn is_complete<S: AsRef<str>>(data: &GameData, c: &PersonaData, ids: &[S]) -> bool {
    clean(data, c, ids).len() == SIZE
}
