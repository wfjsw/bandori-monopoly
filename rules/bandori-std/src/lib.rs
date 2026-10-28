//! `bandori-std` -- BanG Dream Monopoly's rule-code standard library: this
//! ruleset's vocabulary and sugar over `card-sdk`'s generic mechanisms.
//!
//! `card-sdk` holds only generalized card-runtime machinery (and the guest ABI
//! wire codes the host must decode). Anything that names this game's concepts
//! -- 火罐 / 奇迹水晶 / CP点 / CiRCLE / RiNG / Live House / band skills / the
//! status markers -- lives here as readable sugar. Card bodies
//! `use bandori_std::ctx` (a facade that re-exports the generic surface plus
//! these names). Shared game vocabulary used by more than one rule crate goes
//! here, never in `crates/` or `card-sdk`.
//!
//! Host ABI v56: kind checks are `tile_is(t, "ring")`, field reads are
//! `tile_attr`, class membership is `tile_in`. The `is_deed` / `is_start` /
//! `is_buyable` / … spellings below are sugar over that generic trio.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::ctx::raw;
use card_sdk::ctx::{
    add_card_counter, add_counter, add_counter_at, counter, counter_at, set_counter,
};
use card_sdk::msg::Msg;
use card_sdk::Prompt;

/// `[Msg; N]` with every slot `Msg::new(key)`: a fixed-size option buffer for
/// allocation-free prompt lists (fill a prefix, pass `&buf[..len]`).
#[macro_export]
macro_rules! filled_msgs {
    ($n:expr, $key:expr) => {{
        let key: &str = $key;
        ::core::array::from_fn::<$crate::__Msg, { $n }, _>(|_| $crate::__Msg::new(key))
    }};
}

#[doc(hidden)]
pub use card_sdk::msg::Msg as __Msg;

/// Named-counter wire names this ruleset's cards use -- see the marker
/// vocabulary in [`marker::counter`].
pub use marker::counter;

/// Card-face marker vocabulary: the counter names, the countdown prop and its
/// helpers, and the display row of each marker kind (`docs/DISPLAY.md`).
pub mod marker;

/// CEL sugar tables / kind constants / guard-const expansions for this
/// ruleset. Plain data; the typed `CondExt` is assembled in `game-rules`.
pub mod cel;
/// 火罐 -- the capped 「技能指示物」 kernel and the face's 「（初始N，上限M）」
/// declaration (`fire_pots` / `FIRE_INITIAL` / `FIRE_CAP`). Not a status.
pub mod fire;
/// Shared CEL guards.
pub mod guards;
/// Ruleset-declared raise-point kind ids and tile kind/class strings.
pub mod kind;
/// Roster metadata types (`PersonaDef` / `GroupDef`, `meta_key`) and the
/// canonical data-file order tables (ABI v63, `docs/CARDS.md`).
pub mod roster;
/// Generated `(key -> ordinal)` tables restoring `data/*.json` order.
pub mod roster_order;
/// Shared status verbs (stay / stun / exile) over the generic engine services.
pub mod status;

/// State-map keys that name this ruleset's pots / statuses. Frozen webui wire.
///
/// Engine-generic keys (hand limit, skill scratch, turn latches) are
/// re-exported from `card_sdk::abi::state_key` so a card body needs one
/// import (`use bandori_std::state_key`).
pub mod state_key {
    /// Fire pots held (火罐).
    pub const CHARGE: &str = "charge";
    /// [停留] layers.
    pub const STAY: &str = "stay";
    /// [晕眩] layers.
    pub const STUN: &str = "stun";
    /// [晕眩] applied this turn, counting from next turn.
    pub const STUN_START: &str = "stunStart";
    /// [除外] layers.
    pub const EXILE: &str = "exile";
    /// Tile the exile returns to, or -1 for none.
    pub const EXILE_TO: &str = "exileTo";
    /// 「视为当回合的主要移动」 -- the return teleport is that turn's main move.
    pub const EXILE_MAIN: &str = "exileMain";
    /// Duration grant (「获得[除外]直至X」) marker.
    pub const EXILE_HOLD: &str = "exileHold";
    /// Layers of "may hold no hand cards".
    pub const NO_HAND: &str = "noHand";
    /// Layers of "cannot be stopped".
    pub const UNSTOPPABLE: &str = "unstoppable";

    // Engine-generic keys (no game vocabulary): one import for card bodies.
    pub use card_sdk::abi::state_key::{
        ABNORMAL_HIT, HAND_LIMIT, NO_COUNTERACT_TURN, NO_HAND_LIMIT, SKILL_STATE, START_HAND,
    };
}

/// Generic [`state_flag`] bits this ruleset's writers declare (copied from
/// `game_core::state::state_flag` so the guest never names engine internals).
/// The engine stores the `u32` opaquely; these are the meanings it enforces.
pub mod state_flag {
    /// Status-like counter: `state::add` floors at 0 (「清除」).
    pub const NEVER_NEGATIVE: u32 = 1;
    /// Engine-owned key (turn rules / status gates), not a card's scratch.
    pub const ENGINE_OWNED: u32 = 2;
}

/// Card static-prop keys this ruleset owns (frozen engine queries as data).
pub mod prop {
    /// 「（此卡可在眩晕时打出）」 -- `1` = a play of this card skips the act
    /// gate (`CardDef::exemption_prop` names this key to the engine).
    pub const PLAYABLE_STUNNED: &str = "playableStunned";
}

/// Generic `card_sdk::ctx` plus this ruleset's sugar. Replace
/// `use card_sdk::ctx` with `use bandori_std::ctx` in card bodies.
pub mod ctx {
    pub use card_sdk::ctx::*;

    pub use crate::marker::{dec_turns_left, set_turns_left, turns_left};
    pub use crate::{
        add_card_crystals, add_crystals, add_crystals_at, add_group_crystals, add_group_skill,
        add_rent_unit_bonus, add_ring_bonus, card_crystals, crystals, crystals_at, group_crystals,
        group_skill, group_skills, in_group, invoke_skill, is_agent, is_buyable, is_color, is_deed,
        is_live_house_for, is_shop, is_start, is_venue, offer_build, offer_buy, offer_force_buy,
        pay_rent, persona_is, persona_skill, remove_crystal, rent_unit_mult, ring_multiplier,
        set_crystals, skill_blocked, turn_snap,
    };
}

/// Turn-start snapshot of the three layer counters 「回到起始地点并取消所有
/// 受到的效果」 restores: `(pos, stay, stun, exile)`. Wrapper over the generic
/// keyed `card_sdk::ctx::turn_snap` with this ruleset's key names.
pub fn turn_snap(player_id: i32) -> (i32, i32, i32, i32) {
    card_sdk::ctx::turn_snap(
        player_id,
        state_key::STAY,
        state_key::STUN,
        state_key::EXILE,
    )
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
pub fn is_deed(tile: i32) -> bool {
    card_sdk::ctx::tile_is(tile, "ring")
}

/// The CiRCLE tile.
pub fn is_start(tile: i32) -> bool {
    card_sdk::ctx::tile_is(tile, "circle")
}

/// Is this the 地产商 tile?
pub fn is_agent(tile: i32) -> bool {
    card_sdk::ctx::tile_is(tile, "agent")
}

/// A Live House deed (buyable, colour group 6).
pub fn is_venue(tile: i32) -> bool {
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
    counter(counter::CRYSTALS)
}

/// Set this card instance's crystals; returns the new count.
pub fn set_crystals(n: i32) -> i32 {
    set_counter(counter::CRYSTALS, n)
}

/// Adjust this card instance's crystals by `n`, clamped at 0 and at `max`.
pub fn add_crystals(n: i32, max: i32) -> Result<i32, Prompt> {
    add_counter(counter::CRYSTALS, n, max)
}

/// [奇迹水晶] on a named placed card.
pub fn card_crystals(player_id: i32, card: &str) -> i32 {
    card_sdk::ctx::card_counter(player_id, card, counter::CRYSTALS)
}

/// Adjust [奇迹水晶] on a named placed card; `max` caps (0 = uncapped).
pub fn add_card_crystals(player_id: i32, card: &str, n: i32, max: i32) -> i32 {
    add_card_counter(player_id, card, counter::CRYSTALS, n, max)
}

/// [奇迹水晶] on the instance at `uid`.
pub fn crystals_at(uid: i32) -> i32 {
    counter_at(uid, counter::CRYSTALS)
}

/// Adjust [奇迹水晶] on the instance at `uid`; `max` caps (0 = uncapped).
pub fn add_crystals_at(uid: i32, n: i32, max: i32) -> i32 {
    add_counter_at(uid, counter::CRYSTALS, n, max)
}

/// Burn one of this card's crystals (the decay tick). Returns the count left.
pub fn remove_crystal() -> Result<i32, Prompt> {
    add_crystals(-1, 0)
}

// 火罐: the policy kernel is [`crate::fire`] (held tokens of the persona-skill
// instance). The uncapped `gain_charge` / `spend_charge` / `add_charge_max`
// sugar is gone -- it was a bypass footgun. Use `fire::{gain, spend, adjust_cap}`.

// ------------------------------------------------------------- band / character

/// 「乐队卡 / 团卡」 [奇迹水晶] on `player_id`'s band-skill field instance.
/// Game sugar: the group-skill pool (ruleset counter name).
pub fn group_crystals(player_id: i32) -> i32 {
    group_pool(player_id, counter::CRYSTALS)
}

/// A named pool on the player's group-skill instance.
pub fn group_pool(player_id: i32, name: &str) -> i32 {
    let (p, l) = raw::s(name);
    unsafe { raw::sys::group_pool(player_id, p, l) }
}

pub fn add_group_crystals(player_id: i32, n: i32, max: i32) -> i32 {
    add_group_pool(player_id, counter::CRYSTALS, n, max)
}

pub fn add_group_pool(player_id: i32, name: &str, n: i32, max: i32) -> i32 {
    let (p, l) = raw::s(name);
    unsafe { raw::sys::add_group_pool(player_id, p, l, n, max) }
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
///
/// The rule id is an interned [`NameId`] (identity, not display) — compare
/// with a literal (`id == "skill:…"`) or pass `id.as_str()` to `invoke_skill`.
/// Wire format is still `postcard::to_allocvec(&Vec<(i32, String, i32)>)`;
/// the guest interns on decode (no `String` construction).
pub fn group_skills(player_id: i32) -> Vec<(i32, card_sdk::ctx::NameId, i32)> {
    let cap = 8192;
    let (p, scratch) = raw::out_buf(cap as usize);
    let n = unsafe { raw::sys::group_skills(player_id, p, cap) };
    if n <= 0 || n > cap {
        return Vec::new();
    }
    let buf = raw::out_read(p, n, scratch);
    let borrowed: Vec<(i32, &str, i32)> =
        postcard::from_bytes(&buf[..n as usize]).unwrap_or_default();
    let mut out = Vec::with_capacity(borrowed.len());
    for (uid, id, extra) in borrowed {
        out.push((uid, card_sdk::intern::intern(id), extra));
    }
    out
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

/// 规则书 基础规则 5: 「抵押后[获得]地契购买价格50%的资金」.
/// Land price only -- standing houses are not part of the mortgage payout.
/// This is the **rules-side** integer form; `rules/tiles/src/deed.rs` is the
/// tile-facing alias and `purchase::mortgage_value` is the engine-side copy
/// pinned equal to this by `game-rules`'s `deed_math` test.
#[inline]
pub fn mortgage_value(land_price: i32) -> i32 {
    land_price / 2
}

/// 规则书 基础规则 6: 「赎回后[消耗]地契购买价格60%的资金」.
/// Integer form of `round_ties_even(price * 0.6)` (the product of an integer
/// price and 0.6 never lands on `.5`, so this matches). This is the
/// **rules-side** integer form; `rules/tiles/src/deed.rs` is the tile-facing
/// alias and `purchase::redeem_cost` is the engine-side copy pinned equal to
/// this by `game-rules`'s `deed_math` test.
#[inline]
pub fn redeem_cost(land_price: i32) -> i32 {
    (land_price * 3 + 2) / 5
}

/// 「向上取整10」 on a half charge: `ceil(full / 2 / 10) * 10`.
/// Integer ceil-div (toward +inf), so this is `no_std`-clean. The half-rent
/// rounding (「半价收费…向上取整10」). Engine-side twin is
/// `purchase::half_ceil10`, pinned equal by `game-rules`'s `deed_math` test.
#[inline]
pub fn half_ceil10(full: i32) -> i32 {
    let q = full.div_euclid(20);
    let ceil = if full.rem_euclid(20) == 0 { q } else { q + 1 };
    ceil * 10
}

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
    card_sdk::ctx::trigger::card_is(card_id) && card_sdk::ctx::trigger::name() == counter::CRYSTALS
}

/// Always-true guard (the x4 `always` copy).
pub fn always(_player_id: i32) -> bool {
    true
}

/// Tile-class table packed for the host (ABI v70).
pub fn encode_class_defs() -> i64 {
    let defs = kind::class_defs();
    // Why this goes through `rt::leak` and not a local `Box::leak` +
    // `(ptr << 32) | len`: the host reads the bytes after this call returns,
    // so they must outlive our frame -- that part *is* a leak on the pure
    // wasm32 guest path, and `rt::leak` documents and bounds it. But on native
    // (and the `native-rules` monolith) a raw heap pointer truncated to 32
    // bits is a garbage handle; `rt::leak` interns into the per-run arena
    // there instead. Hand-rolling the pack here silently produced that garbage
    // handle. One shared hand-off path, one set of rules.
    card_sdk::rt::leak(postcard::to_allocvec(&defs).unwrap_or_default())
}

/// Guest export: the engine's `tile_in` class table.
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn ruleset_class_defs() -> i64 {
    encode_class_defs()
}

/// Ruleset-kind hook-only flags packed for the host (see [`kind::kind_hook_only`]).
pub fn encode_kind_flags() -> i64 {
    let flags = kind::kind_hook_only();
    // Same hand-off as [`encode_class_defs`] -- see the note there. The leak
    // (wasm32 guest) or arena slot (native) is justified inside
    // [`card_sdk::rt::leak`]; do not re-implement the packing locally.
    card_sdk::rt::leak(postcard::to_allocvec(&flags).unwrap_or_default())
}

/// Guest export: which ruleset kinds are hook-only (no counteraction window).
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn ruleset_kind_flags() -> i64 {
    encode_kind_flags()
}
