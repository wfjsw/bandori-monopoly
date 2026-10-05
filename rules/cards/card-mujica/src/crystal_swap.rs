//! `Mujica:会被骗着买水晶的人` -- C# `CardCrystalSwap` (MatchHost.cs:5981-6049):
//! move one miracle crystal from one card to another.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:会被骗着买水晶的人`）:
//! > 会被骗着买水晶的人：
//! >  将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。
//!

use alloc::vec::Vec;

use card_sdk::ctx::{self, CardPile};
use card_sdk::{key, CardDef, Msg, On};


pub const CRYSTAL_SWAP: CardDef = CardDef::new("Mujica:会被骗着买水晶的人", &[
    On::Play(None, crystal_swap)]);

/// One pool entry (C# `CardCrystalSwap.Slots`'s `(label, get, add)` tuple).
/// `Placed(player_id)` is a card in play at the player; `Band(player_id)` is the player's
/// band card.
enum Slot {
    Placed(i32),
    Band(i32),
}

impl Slot {
    /// C# `get` -- the crystal count on this card.
    fn get(&self) -> i32 {
        match *self {
            Slot::Placed(player_id) => ctx::crystals(player_id),
            Slot::Band(player_id) => ctx::band_crystals(player_id),
        }
}

    /// C# `add` -- `Card.AddCrystals(n, ...)` / `band.AddCr(n, ...)`.
    fn add(&self, n: i32) {
        match *self {
            // `max = 0` = uncapped; the C# clamps at the card's own
            // `MaxCrystals`, which the ABI does not expose.
            Slot::Placed(player_id) => {
                ctx::add_crystals(player_id, n, 0);
            }
            Slot::Band(player_id) => {
                ctx::add_band_crystals(player_id, n, 0);
            }
        }
}

    /// C# `label` -- `"{{who}}'s \"{{card}}\" ({{n}})"` / `"{{who}}'s band card ({{n}})"`.
    fn label(&self) -> Msg {
        match *self {
            Slot::Placed(player_id) => {
                let id = ctx::cards_in(player_id, CardPile::Field)
                    .first()
                    .cloned()
                    .unwrap_or_default();
                Msg::new(key!("crystal_swap_card"))
                    .player_id("who", player_id)
                    .card("card", &id)
                    .i("n", self.get() as i64)
            }
            Slot::Band(player_id) => Msg::new(key!("crystal_swap_band"))
                .player_id("who", player_id)
                .i("n", self.get() as i64),
        }
}
}

/// C# `CardCrystalSwap.Slots` -- every placed card and every player's band card.
fn slots() -> Vec<Slot> {
    let mut v: Vec<Slot> = Vec::new();
    for s in 0..ctx::player_count() {
        if ctx::player_out(s) {
            continue;
        }
        // C# `H._placed.Where(p => p.Live && !p.Immune && text.Contains("奇迹水晶"))`.
        for c in ctx::placed_cards(s) {
            if ctx::card_face_down(s, &c) || ctx::card_immune(s, &c) {
                continue;
            }
            if ctx::card_text_mentions(&c, "奇迹水晶") {
                v.push(Slot::Placed(s));
                break;
            }
        }
        // C# `H._fx[player_id].bands.First()` when `bands.Count > 0`. `band_crystals`
        // is the band card's counter; the `bands` inventory has no ctx query
        // (see the TODO in `crystal_swap`).
        v.push(Slot::Band(s));
    }
    v
}

fn crystal_swap(player_id: i32) {
    // 规则书: 「将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上」
    // C# `CardCrystalSwap.Slots` enumerates every placed card whose text
    // mentions 奇迹水晶 (plus each player's band card). The per-card crystal
    // counters are `ctx::crystals` / `ctx::band_crystals`.
    // Still held: `H._fx[player_id].bands` enumeration (`bands.Count == 0`
    // skip, `band.Extra` in `add`) -- the band-card side of the pool.
    let all = slots();
    // C# `from = slots.Where(x => x.get() > 0)`.
    let from: Vec<usize> = (0..all.len()).filter(|&i| all[i].get() > 0).collect();
    if from.is_empty() || all.len() < 2 {
        // C# `c.Effective = false` + a log line when no crystal can move.
        ctx::log(player_id, &Msg::new(key!("crystal_swap_none")).player_id("who", player_id));
        return;
    }
    // 规则书: 「将场上一张卡上的一个奇迹水晶移动」 -- pick the source.
    let from_labels: Vec<Msg> = from.iter().map(|&i| all[i].label()).collect();
    let src_i = ctx::ask_pick(
        player_id,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_from")),
        &from_labels,
    );
    let src = from[src_i];
    // 规则书: 「到另一张可以放置奇迹水晶的卡上」 -- pick the destination
    // (C# `slots.Where(x => x.label != src.label)`).
    let to: Vec<usize> = (0..all.len()).filter(|&i| i != src).collect();
    let to_labels: Vec<Msg> = to.iter().map(|&i| all[i].label()).collect();
    let dst_i = ctx::ask_pick(
        player_id,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_to")),
        &to_labels,
    );
    let dst = to[dst_i];
    // 规则书: the move itself (C# `src.AddCrystals(-1)` + `dst.AddCrystals(1)`).
    all[src].add(-1);
    all[dst].add(1);
    ctx::log(
        player_id,
        &Msg::new(key!("crystal_swap_moved"))
            .player_id("src", slot_player(&all[src]))
            .player_id("dst", slot_player(&all[dst])),
    );
}

fn slot_player(s: &Slot) -> i32 {
    match *s {
        Slot::Placed(player_id) | Slot::Band(player_id) => player_id,
    }
}
