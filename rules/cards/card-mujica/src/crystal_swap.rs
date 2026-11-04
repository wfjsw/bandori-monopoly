//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:会被骗着买水晶的人`）:
//! > 会被骗着买水晶的人：
//! >  将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。
//!
//! move **one** miracle crystal from one card on the field to another. The two
//! ends are addressed by card **uid**, so the crystal leaves exactly the chosen
//! card and lands on exactly the chosen card -- never the running card, and
//! never the whole pool.

use alloc::vec::Vec;

use bandori_std::counter;
use bandori_std::ctx;
use card_sdk::ctx::NameId;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const CRYSTAL_SWAP: CardDef = CardDef::new(
    "Mujica:会被骗着买水晶的人",
    &[On::Play("", None, crystal_swap)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"会被骗着买水晶的人"#)
        .raw_name(r#"会被骗着买水晶的人"#)
        .tags(&[])
        .text(CardText::para(
            r#"将场上一张卡上的一个奇迹水晶移动到另一张可以放置奇迹水晶的卡上。"#,
        ))
        // sheet cell J13
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]),
);

/// One pool entry: a card instance on some player's field.
///
/// `band` only picks the label: a band-skill instance is called 「团卡」
/// (`crystal_swap_band`), everything else is named by its card id
/// (`crystal_swap_card`).
struct Slot {
    uid: i32,
    player_id: i32,
    card: NameId,
    group_skill: bool,
}

impl Slot {
    fn get(&self) -> i32 {
        ctx::counter_at(self.uid, counter::CRYSTALS)
    }

    /// Add `n` to this instance's crystals. `max = 0` is the call-local bound;
    /// the instance's stored `counter_max.crystals` (its printed 上限) is the
    /// separate clamp `add_counter_at` applies itself.
    fn add(&self, n: i32) {
        ctx::add_counter_at(self.uid, counter::CRYSTALS, n, 0);
    }

    fn label(&self) -> Msg {
        let n = self.get() as i64;
        if self.group_skill {
            Msg::new(key!("crystal_swap_band"))
                .player_id("who", self.player_id)
                .i("n", n)
        } else {
            Msg::new(key!("crystal_swap_card"))
                .player_id("who", self.player_id)
                .card("card", self.card.as_str())
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
fn is_group_label(card: &str, player_id: i32) -> bool {
    let Some(rest) = card.strip_prefix("skill:") else {
        return false;
    };
    let Some((owner, _)) = rest.split_once(':') else {
        return false;
    };
    !ctx::persona_is(player_id, owner)
}

fn slots(by: i32) -> Vec<Slot> {
    let mut v: Vec<Slot> = Vec::new();
    for s in 0..ctx::player_count() {
        if ctx::player_out(s) {
            continue;
        }
        for (uid, c) in ctx::field_instances(s) {
            // TODO(规则书)（「场上一张卡」）: the cell names no face-down exclusion -- face-down cards
            // are dropped from both endpoints; only the immune half is grounded in
            // 「不受任何效果影响」.
            if ctx::is_face_down_at(uid) || ctx::is_immune_from_at(uid, by) {
                continue;
            }
            v.push(Slot {
                uid,
                player_id: s,
                group_skill: is_group_label(c.as_str(), s),
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
    let all = slots(player_id);
    let from: Vec<usize> = (0..all.len()).filter(|&i| all[i].get() > 0).collect();
    if from.is_empty() || all.len() < 2 {
        ctx::log(
            player_id,
            &Msg::new(key!("crystal_swap_none")).player_id("who", player_id),
        );
        return Ok(());
    }
    // 规则书: 「将场上一张卡上的一个奇迹水晶移动」 -- pick the source.
    let from_labels: Vec<Msg> = from.iter().map(|&i| all[i].label()).collect();
    let src_i = ctx::ask_choice(
        player_id,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_from")),
        &from_labels,
    )?;
    let src = from[src_i.min(from.len() - 1)];
    // 规则书: 「到另一张可以放置奇迹水晶的卡上」 -- pick the destination, never
    // the source itself.
    // 「可以放置」: a card at its own printed 上限 cannot take one. The cap is the
    // per-instance `counter_max.crystals` store; no cap (0) is uncapped.
    let can_take = |s: &Slot| {
        let cap = ctx::counter_max_at(s.uid, counter::CRYSTALS);
        cap == 0 || s.get() < cap
    };
    let to: Vec<usize> = (0..all.len())
        .filter(|&i| i != src && can_take(&all[i]))
        .collect();
    if to.is_empty() {
        // Every other card is full: nothing can move.
        ctx::log(
            player_id,
            &Msg::new(key!("crystal_swap_none")).player_id("who", player_id),
        );
        return Ok(());
    }
    let to_labels: Vec<Msg> = to.iter().map(|&i| all[i].label()).collect();
    let dst_i = ctx::ask_choice(
        player_id,
        &Msg::new(key!("crystal_swap_title")),
        &Msg::new(key!("crystal_swap_to")),
        &to_labels,
    )?;
    let dst = to[dst_i.min(to.len() - 1)];
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

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn crystal_swap_kani(player_id: i32) -> card_sdk::Asked {
    crystal_swap(player_id)
}
