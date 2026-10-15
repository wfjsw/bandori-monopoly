//! BanG Dream Monopoly / this ruleset's vocabulary over `card-sdk`'s generic
//! mechanisms.
//!
//! `card-sdk` holds only generalized card-runtime machinery (and the guest ABI
//! wire codes the host must decode). Anything that names this game's concepts
//! -- 火罐 / 奇迹水晶 / CP点 / CiRCLE / RiNG / Live House / band skills -- lives
//! here as readable sugar. Card bodies `use ruleset_core::ctx` (a facade that
//! re-exports the generic surface plus these names).
//!
//! Host ABI v56: kind checks are `tile_is(t, "ring")`, field reads are
//! `tile_attr`, class membership is `tile_in`. The `is_ring` / `is_circle` /
//! `is_buyable` / … spellings below are sugar over that generic trio.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::ctx::raw;
use card_sdk::ctx::{add_card_counter, add_counter, add_counter_at, counter, counter_at, set_counter};
use card_sdk::msg::Msg;
use card_sdk::Prompt;

/// Named-counter wire names this ruleset's cards use. The strings are ABI
/// codes (the host matches on them); the *names* are game vocabulary.
/// (Also kept as `card_sdk::abi::counter` so the wire module stays self-contained.)
pub mod counter {
    /// On-card [CP点].
    pub const CP: &str = "cp";
    /// 奇迹水晶.
    pub const CRYSTALS: &str = "crystals";
}

/// Roster metadata types (`PersonaDef` / `GroupDef`, `meta_key`) and the
/// canonical data-file order tables (ABI v63, `docs/CARDS.md`).
pub mod roster;
/// Generated `(key -> ordinal)` tables restoring `data/*.json` order.
pub mod roster_order;

/// State-map keys that name this ruleset's pots / statuses.
pub mod state_key {
    /// Fire pots held.
    pub const CHARGE: &str = "charge";
}

/// Generic `card_sdk::ctx` plus this ruleset's sugar. Replace
/// `use card_sdk::ctx` with `use ruleset_core::ctx` in card bodies.
pub mod ctx {
    pub use card_sdk::ctx::*;

    pub use crate::{
        add_group_crystals, add_group_skill, add_card_crystals, add_crystals, add_crystals_at,
        add_charge_max, add_rent_unit_bonus, add_ring_bonus, group_crystals, group_skill,
        group_skills, card_crystals, persona_is, persona_skill, crystals, crystals_at, decay,
        charge, charge_max, gain_charge, in_group, invoke_skill, is_agent, is_buyable, is_circle,
        is_color, is_live_house, is_live_house_for, is_ring, is_shop, offer_build, offer_buy,
        offer_force_buy, pay_rent, rent_unit_mult, ring_multiplier, set_crystals, skill_blocked,
        spend_charge,
    };
}

// ------------------------------------------------------------- tile kinds

/// A deed tile (property / RiNG) -- class `buyable`.
pub fn is_buyable(tile: i32) -> bool {
    card_sdk::ctx::tile_in(-1, tile, "buyable")
}

/// A 「商店街」 deed (buyable and colour group 10).
pub fn is_shop(tile: i32) -> bool {
    card_sdk::ctx::tile_in(-1, tile, "shop")
}

/// A RiNG deed (never holds houses).
pub fn is_ring(tile: i32) -> bool {
    card_sdk::ctx::tile_is(tile, "ring")
}

/// The CiRCLE tile.
pub fn is_circle(tile: i32) -> bool {
    card_sdk::ctx::tile_is(tile, "circle")
}

/// Is this the 地产商 tile?
pub fn is_agent(tile: i32) -> bool {
    card_sdk::ctx::tile_is(tile, "agent")
}

/// A Live House deed (buyable, colour group 6).
pub fn is_live_house(tile: i32) -> bool {
    card_sdk::ctx::tile_in(-1, tile, "livehouse")
}

/// Live-House check for one player: the base check plus their extra-colour
/// band-skill colours (「使其对你视为live house格子」 / anyColor / colorFor).
pub fn is_live_house_for(player_id: i32, tile: i32) -> bool {
    card_sdk::ctx::tile_in(player_id, tile, "livehouse")
}

/// Does `tile` count as colour `group` for `player_id`?
pub fn is_color(player_id: i32, tile: i32, group: i32) -> bool {
    let class = format!("color:{}", group);
    card_sdk::ctx::tile_in(player_id, tile, &class)
}

// ------------------------------------------------------------- 奇迹水晶

/// Miracle crystals on **this card instance**.
pub fn crystals() -> i32 {
    counter("crystals")
}

/// Set this card instance's crystals; returns the new count.
pub fn set_crystals(n: i32) -> i32 {
    set_counter("crystals", n)
}

/// Adjust this card instance's crystals by `n`, clamped at 0 and at `max`.
pub fn add_crystals(n: i32, max: i32) -> Result<i32, Prompt> {
    add_counter("crystals", n, max)
}

/// [奇迹水晶] on a named placed card.
pub fn card_crystals(player_id: i32, card: &str) -> i32 {
    card_sdk::ctx::card_counter(player_id, card, "crystals")
}

/// Adjust [奇迹水晶] on a named placed card; `max` caps (0 = uncapped).
pub fn add_card_crystals(player_id: i32, card: &str, n: i32, max: i32) -> i32 {
    add_card_counter(player_id, card, "crystals", n, max)
}

/// [奇迹水晶] on the instance at `uid`.
pub fn crystals_at(uid: i32) -> i32 {
    counter_at(uid, "crystals")
}

/// Adjust [奇迹水晶] on the instance at `uid`; `max` caps (0 = uncapped).
pub fn add_crystals_at(uid: i32, n: i32, max: i32) -> i32 {
    add_counter_at(uid, "crystals", n, max)
}

/// Burn one of this card's crystals (the decay tick). Returns the count left.
pub fn decay() -> Result<i32, Prompt> {
    add_crystals(-1, 0)
}

// ------------------------------------------------------------- 火罐

pub fn charge(player_id: i32) -> i32 {
    unsafe { raw::sys::charge(player_id) }
}

pub fn charge_max(player_id: i32) -> i32 {
    unsafe { raw::sys::charge_max(player_id) }
}

/// Gain fire pots, capped by the player's own cap.
pub fn gain_charge(player_id: i32, n: i32, why: &Msg) -> Result<i32, Prompt> {
    let (p, l) = raw::mj(why);
    raw::asked(unsafe { raw::sys::gain_charge(player_id, n, p, l) })
}

/// Spend `n` [火罐]; false when the player has fewer. Logs the spend.
///
/// Marker window (user ruling 2026-10-07): `markerSpend` opens **before** the
/// fire moves and may cancel it (`Err(Prompt)` to pause, `Ok(false)` when a
/// counteraction cancelled the link).
///
/// 「以代替此次技能的火罐消耗」 -- `ChargePaying` runs before the pots move.
/// A hook that substitutes the cost cancels the link and performs its own
/// payment; the spend is then treated as satisfied.
pub fn spend_charge(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
    if n <= 0 {
        return Ok(true);
    }
    if !card_sdk::ctx::raise(player_id, card_sdk::abi::TriggerKind::ChargePaying, n)? {
        return Ok(true);
    }
    let (p, l) = raw::mj(why);
    raw::asked(unsafe { raw::sys::spend_charge(player_id, n, p, l) }).map(|v| v != 0)
}

/// Raise (or lower) `player_id`'s [火罐] cap; returns the new cap.
pub fn add_charge_max(player_id: i32, n: i32) -> i32 {
    unsafe { raw::sys::add_charge_max(player_id, n) }
}

// ------------------------------------------------------------- band / character

/// 「乐队卡 / 团卡」 [奇迹水晶] on `player_id`'s band-skill field instance.
/// Game sugar: the group-skill pool (ruleset counter name).
pub fn group_crystals(player_id: i32) -> i32 {
    group_pool(player_id, "crystals")
}

/// A named pool on the player's group-skill instance.
pub fn group_pool(player_id: i32, name: &str) -> i32 {
    unsafe {
        let b = name.as_bytes();
        raw::sys::group_pool(player_id, b.as_ptr() as i32, b.len() as i32)
    }
}

pub fn add_group_crystals(player_id: i32, n: i32, max: i32) -> i32 {
    add_group_pool(player_id, "crystals", n, max)
}

pub fn add_group_pool(player_id: i32, name: &str, n: i32, max: i32) -> i32 {
    unsafe {
        let b = name.as_bytes();
        raw::sys::add_group_pool(player_id, b.as_ptr() as i32, b.len() as i32, n, max)
    }
}

/// Rule id of `player_id`'s band skill, or `None`.
pub fn group_skill(player_id: i32) -> Option<String> {
    let cap = 1024;
    let (p, scratch) = raw::out_buf(cap as usize);
    let n = unsafe { raw::sys::group_skill(player_id, p, cap) };
    if n <= 0 || n > cap {
        return None;
    }
    let buf = raw::out_read(p, n, scratch);
    let s: String = postcard::from_bytes(&buf[..n as usize]).unwrap_or_default();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Rule id of `player_id`'s character skill, or `None`.
pub fn persona_skill(player_id: i32) -> Option<String> {
    let cap = 1024;
    let (p, scratch) = raw::out_buf(cap as usize);
    let n = unsafe { raw::sys::persona_skill(player_id, p, cap) };
    if n <= 0 || n > cap {
        return None;
    }
    let buf = raw::out_read(p, n, scratch);
    let s: String = postcard::from_bytes(&buf[..n as usize]).unwrap_or_default();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Band-skill attachments: `(uid, rule id, extra)` in placement order.
pub fn group_skills(player_id: i32) -> Vec<(i32, String, i32)> {
    let cap = 8192;
    let (p, scratch) = raw::out_buf(cap as usize);
    let n = unsafe { raw::sys::group_skills(player_id, p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = raw::out_read(p, n, scratch);
    postcard::from_bytes(&buf[..n as usize]).unwrap_or_default()
}

/// Attach a band-skill instance to `player_id`. Returns the uid, or -1.
pub fn add_group_skill(player_id: i32, id: &str, extra: bool) -> i32 {
    let (p, l) = raw::s(id);
    unsafe { raw::sys::add_group_skill(player_id, p, l, extra as i32) }
}

/// Run a skill rule's press entry for `player_id`.
pub fn invoke_skill(player_id: i32, id: &str) -> Result<card_sdk::ctx::Dest, Prompt> {
    let (p, l) = raw::s(id);
    let v = raw::asked(unsafe { raw::sys::invoke_skill(player_id, p, l) })?;
    Ok(card_sdk::ctx::Dest::from_i32(v))
}

/// Is `player_id` playing character `name`?
pub fn persona_is(player_id: i32, name: &str) -> bool {
    let (p, l) = raw::s(name);
    unsafe { raw::sys::persona_is(player_id, p, l) != 0 }
}

/// Is `player_id` in band `name`?
pub fn in_group(player_id: i32, name: &str) -> bool {
    let (p, l) = raw::s(name);
    unsafe { raw::sys::in_group(player_id, p, l) != 0 }
}

/// 「位于此卡所在格子上的玩家无法使用角色及乐队技能」 / one band's skills.
pub fn skill_blocked(player_id: i32, group: &str) -> bool {
    if card_sdk::ctx::tok(player_id, "skillBlock") > 0 {
        return true;
    }
    if !group.is_empty() {
        let key = format!("skillBlock:{}", group);
        if card_sdk::ctx::tok(player_id, &key) > 0 {
            return true;
        }
    }
    let t = card_sdk::ctx::player_pos(player_id);
    t >= 0
        && card_sdk::ctx::count_marks(t, &card_sdk::abi::MarkFilter::any().kind("skillBlock")) > 0
}

// ------------------------------------------------------------- RiNG / rent / buy

/// The RiNG rent multiplier.
pub fn rent_unit_mult() -> i32 {
    unsafe { raw::sys::rent_unit_mult() }
}

/// Add `n` to the RiNG rent multiplier; returns the new multiplier.
pub fn add_rent_unit_bonus(n: i32) -> i32 {
    unsafe { raw::sys::add_rent_unit_bonus(n) }
}

/// Game sugar for [`rent_unit_mult`] (the RiNG dice-rent convention).
pub fn ring_multiplier() -> i32 {
    rent_unit_mult()
}

/// Game sugar for [`add_rent_unit_bonus`].
pub fn add_ring_bonus(n: i32) -> i32 {
    add_rent_unit_bonus(n)
}

/// Pay `tile`'s rent (`half` = 「租金减半」).
pub fn pay_rent(player_id: i32, tile: i32, half: bool) -> Result<(), Prompt> {
    raw::asked(unsafe { raw::sys::pay_rent(player_id, tile, half as i32) })?;
    Ok(())
}

/// Offer to buy `tile`.
pub fn offer_buy(player_id: i32, tile: i32) -> Result<(), Prompt> {
    raw::asked(unsafe { raw::sys::offer_buy(player_id, tile) })?;
    Ok(())
}

/// Force-buy `tile`.
pub fn offer_force_buy(player_id: i32, tile: i32) -> Result<(), Prompt> {
    raw::asked(unsafe { raw::sys::offer_force_buy(player_id, tile) })?;
    Ok(())
}

/// Offer to build on `tile`.
pub fn offer_build(player_id: i32, tile: i32) -> Result<(), Prompt> {
    raw::asked(unsafe { raw::sys::offer_build(player_id, tile) })?;
    Ok(())
}

// ------------------------------------------------------------- shared guards

/// Base half of the x11 `crystals_changed_guard` copies: this instance's
/// crystals counter changed. Cards add their own residual (value vs 0, …).
pub fn crystals_changed_of(card_id: &str) -> bool {
    card_sdk::ctx::trigger::card_is(card_id)
        && card_sdk::ctx::trigger::name() == "crystals"
}

/// Always-true guard (the x4 `always` copy).
pub fn always(_player_id: i32) -> bool {
    true
}