//! `Mujica:会被骗着买水晶的人` -- C# `CardCrystalSwap` (MatchHost.cs:5981-6049):
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:会被骗着买水晶的人`）:
//! > 会被骗着买水晶的人：
//! >  将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。
//!
//! move **one** miracle crystal from one card on the field to another. The two
//! ends are addressed by card **uid**, so the crystal leaves exactly the chosen
//! card and lands on exactly the chosen card -- never the running card, and
//! never the whole pool.

use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const CRYSTAL_SWAP: CardDef =
    CardDef::new("Mujica:会被骗着买水晶的人", &[On::Play(None, crystal_swap)]);

/// One pool entry: a card instance on some player's field.
///
/// C# `CardCrystalSwap.Slots`'s `(label, get, add)` tuple, but per **uid** --
/// `get` / `add` are [`ctx::crystals_at`] / [`ctx::add_crystals_at`] on this
/// instance, not on the running card. `band` only picks the label: a band-skill
/// instance is called 「团卡」 (`crystal_swap_band`), everything else is named by
/// its card id (`crystal_swap_card`).
struct Slot {
    uid: i32,
    player_id: i32,
    card: String,
    band: bool,
}

impl Slot {
    /// C# `get` -- the crystal count on this card.
    fn get(&self) -> i32 {
        ctx::crystals_at(self.uid)
    }

    /// C# `add` -- `Card.AddCrystals(n, ...)` on this instance. `max = 0` is
    /// uncapped; the C# clamps at the card's own `MaxCrystals`, which the ABI
    /// does not expose.
    fn add(&self, n: i32) {
        ctx::add_crystals_at(self.uid, n, 0);
    }

    /// C# `label` -- `"{{who}}'s band card ({{n}})"` / `"{{who}}'s \"{{card}}\"
    /// ({{n}})"`.
    fn label(&self) -> Msg {
        let n = self.get() as i64;
        if self.band {
            Msg::new(key!("crystal_swap_band"))
                .player_id("who", self.player_id)
                .i("n", n)
        } else {
            Msg::new(key!("crystal_swap_card"))
                .player_id("who", self.player_id)
                .card("card", &self.card)
                .i("n", n)
        }
    }
}

/// Is this field-card id the player's **band** skill (`skill:<band>:<skill>`)
/// rather than their character skill (`skill:<character>:<skill>`)?
///
/// Label-only: the crystal read/write is per uid either way. A borrowed
/// *character* skill (another player's `skill:<character>:…` placed here) would
/// be mislabelled 「团卡」; the count still moves on the right instance.
fn is_band_label(card: &str, player_id: i32) -> bool {
    let Some(rest) = card.strip_prefix("skill:") else {
        return false;
    };
    let Some((owner, _)) = rest.split_once(':') else {
        return false;
    };
    !ctx::character_is(player_id, owner)
}

/// C# `CardCrystalSwap.Slots` -- every card on every live player's field that
/// can hold [奇迹水晶]. 「可以放置奇迹水晶的卡」 is any card in play (the
/// instance carries `Card.Crystals`); face-down and 「不受任何效果影响」 cards
/// are not touchable (C# `p.Live && !p.Immune`).
fn slots() -> Vec<Slot> {
    let mut v: Vec<Slot> = Vec::new();
    for s in 0..ctx::player_count() {
        if ctx::player_out(s) {
            continue;
        }
        for (uid, c) in ctx::field_instances(s) {
            if ctx::is_face_down_at(uid) || ctx::is_immune_at(uid) {
                continue;
            }
            v.push(Slot {
                uid,
                player_id: s,
                band: is_band_label(&c, s),
                card: c,
            });
        }
    }
    v
}

fn crystal_swap(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上」
    // -- pick the source among the cards that hold one, then the destination
    // among the rest, and move exactly one crystal between the two uids.
    let all = slots();
    // C# `from = slots.Where(x => x.get() > 0)`.
    let from: Vec<usize> = (0..all.len()).filter(|&i| all[i].get() > 0).collect();
    if from.is_empty() || all.len() < 2 {
        // C# `c.Effective = false` + a log line when no crystal can move.
        ctx::log(
            player_id,
            &Msg::new(key!("crystal_swap_none")).player_id("who", player_id),
        );
        return Ok(());
    }
    // 规则书: 「将场上一张卡上的一个奇迹水晶移动」 -- pick the source.
    let from_labels: Vec<Msg> = from.iter().map(|&i| all[i].label()).collect();
    let src_i = ctx::ask_pick(
        player_id,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_from")),
        &from_labels,
    )?;
    let src = from[src_i.min(from.len() - 1)];
    // 规则书: 「到另一张可以放置奇迹水晶的卡上」 -- pick the destination
    // (C# `slots.Where(x => x.label != src.label)`).
    let to: Vec<usize> = (0..all.len()).filter(|&i| i != src).collect();
    let to_labels: Vec<Msg> = to.iter().map(|&i| all[i].label()).collect();
    let dst_i = ctx::ask_pick(
        player_id,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_to")),
        &to_labels,
    )?;
    let dst = to[dst_i.min(to.len() - 1)];
    // 规则书: 「一个奇迹水晶」 -- exactly one, from the chosen uid to the
    // chosen uid (C# `src.AddCrystals(-1)` + `dst.AddCrystals(1)`).
    all[src].add(-1);
    all[dst].add(1);
    ctx::log(
        player_id,
        &Msg::new(key!("crystal_swap_moved"))
            .player_id("src", all[src].player_id)
            .player_id("dst", all[dst].player_id),
    );
    Ok(())
}