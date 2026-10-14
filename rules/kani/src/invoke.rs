//! Registry-based invocation of shipped `CardDef` / skill / event entries.
//!
//! ## Indexed path (Kani / proofs) — no string scan
//!
//! `card_id!("…")` resolves an id to a stable `usize` at **compile time**
//! (`generated/card_ids.rs`). `play_idx` / `entry_idx` / `hook_idx` /
//! `hook_only_idx` / `counteract_idx` index `RULESET` with integer arithmetic
//! only. **This is the path proofs must use.** The old `find` string scan over
//! ~285 CJK ids is a CBMC unwinding / OOM cliff (see `KANI.md` §5).
//!
//! ```ignore
//! invoke::unwrap_done(invoke::play_idx(card_id!("MyGO:轮符雨"), player));
//! ```
//!
//! ## String path (native / tests)
//!
//! `play` / `entry` / `hook` / … still take `&str` and call `find`. Keep them
//! for non-Kani callers; do **not** use them inside `#[kani::proof]` bodies.
//!
//! Unknown id / kind / index fails **loudly** (`panic!` /
//! `kani::assert(false)`), never silently.

use card_sdk::{Asked, On, Prompt};

/// `card_id!` is `#[macro_export]`ed at the crate root (same contract as
/// `guard_id!`). Re-exported here so `invoke::card_id!` also works.
pub use crate::card_id;

/// One band / skill / event table, same order as `rules-native::RULESET`.
pub static RULESET: &[&[card_sdk::CardDef]] = &[
    #[cfg(feature = "band-ag")]
    card_ag::CARDS,
    #[cfg(feature = "band-crychic")]
    card_crychic::CARDS,
    #[cfg(feature = "band-general")]
    card_general::CARDS,
    #[cfg(feature = "band-hhw")]
    card_hhw::CARDS,
    #[cfg(feature = "band-morfonica")]
    card_morfonica::CARDS,
    #[cfg(feature = "band-mujica")]
    card_mujica::CARDS,
    #[cfg(feature = "band-mygo")]
    card_mygo::CARDS,
    #[cfg(feature = "band-pp")]
    card_pp::CARDS,
    #[cfg(feature = "band-ppp")]
    card_ppp::CARDS,
    #[cfg(feature = "band-ras")]
    card_ras::CARDS,
    #[cfg(feature = "band-roselia")]
    card_roselia::CARDS,
    #[cfg(feature = "band-sumimi")]
    card_sumimi::CARDS,
    #[cfg(feature = "skills-bands")]
    skill_bands::CARDS,
    #[cfg(feature = "skills-characters")]
    skill_characters::CARDS,
    #[cfg(feature = "tiles")]
    rules_tiles::CARDS,
    #[cfg(feature = "tiles")]
    rules_tile_marks::CARDS,
    #[cfg(feature = "events")]
    rules_events::CARDS,
];

/// Flatten-order length of [`RULESET`]. Must equal
/// `generated::card_ids::CARD_COUNT`.
pub fn ruleset_len() -> usize {
    let mut n = 0usize;
    let mut b = 0usize;
    while b < RULESET.len() {
        n += RULESET[b].len();
        b += 1;
    }
    n
}

/// Start index of each band in the flattened [`RULESET`], plus one past the
/// end (`CARD_COUNT`). Generated at build time from the **enabled** feature
/// crates (`build.rs` `gen_body_dispatch`); `band_starts_match_ruleset` pins
/// it to the live tables.
include!(concat!(env!("OUT_DIR"), "/band_start.rs"));

/// Index `idx` of the flattened [`RULESET`] by a const offset table.
///
/// Zero-loop band select (17 integer compares) — a `while` binary search
/// unwinds 10 times under CBMC even when `idx` is a `card_id!(…)` constant.
/// Fails loudly on out-of-range.
#[inline(always)]
pub fn by_idx(idx: usize) -> &'static card_sdk::CardDef {
    if idx >= BAND_START[BAND_START.len() - 1] {
        fail_unknown_idx(idx);
    }
    let lo = band_of(idx);
    let band = RULESET[lo];
    let off = idx - BAND_START[lo];
    if off >= band.len() {
        fail_unknown_idx(idx);
    }
    &band[off]
}

fn fail_unknown_idx(idx: usize) -> ! {
    #[cfg(kani)]
    kani::assert(false, "card id index out of range");
    #[cfg(not(kani))]
    panic!("card id index out of range: {idx}");
    #[allow(unreachable_code)]
    unreachable!()
}

/// Linear scan of [`RULESET`] for `id`. `None` when unknown.
///
/// **Not for Kani** — O(n) `memcmp` over long CJK ids is a CBMC cliff.
/// Use [`by_idx`] + `card_id!` in proofs. (The temporary `AYA_LONGING`
/// fast-path hack from the prompt unit is gone: the const-index table is
/// the general solution.)
pub fn find(id: &str) -> Option<&'static card_sdk::CardDef> {
    for band in RULESET {
        for card in band.iter() {
            if card.id == id {
                return Some(card);
            }
        }
    }
    None
}

/// Runtime string → index. **Not for Kani.**
pub fn find_idx(id: &str) -> Option<usize> {
    let mut idx = 0usize;
    for band in RULESET {
        for card in band.iter() {
            if card.id == id {
                return Some(idx);
            }
            idx += 1;
        }
    }
    None
}

fn find_or_fail(id: &str) -> &'static card_sdk::CardDef {
    match find(id) {
        Some(c) => c,
        None => {
            #[cfg(kani)]
            kani::assert(false, "unknown card/skill/event id");
            #[cfg(not(kani))]
            panic!("unknown card/skill/event id: {id}");
            #[allow(unreachable_code)]
            unreachable!()
        }
    }
}

/// Entry body of `On` (the `fn(i32) -> Asked`).
fn body_of(on: &On) -> fn(i32) -> Asked {
    match on {
        On::Play(_, _, f)
        | On::Counteract(_, _, _, f)
        | On::Hook(_, _, _, f)
        | On::Gate(_, _, _, f)
        | On::RollPlan(_, _, f)
        | On::AtEnd(_, _, f)
        | On::Settle(_, _, f)
        | On::Message(_, _, _, f) => *f,
    }
}

// Per-card body dispatch (`call_body_idx`) is generated at build time by
// `build.rs` (`gen_body_dispatch`) into `$OUT_DIR/body_dispatch_fn.rs` —
// never committed (same rule as `cond_irs.rs`). Each trampoline loads one
// constant `&CardDef` so CBMC's function-pointer removal sees a per-card
// points-to set, not the union of every address-taken `fn(i32) -> Asked`
// (the 4k-target IF-chain behind `duobang_01_02`'s SAT cliff).
#[cfg(kani)]
include!(concat!(env!("OUT_DIR"), "/body_dispatch_fn.rs"));

fn fail_entry_oob(idx: usize, entry_index: usize) -> ! {
    #[cfg(kani)]
    kani::assert(false, "entry index out of range");
    #[cfg(not(kani))]
    panic!("entry index out of range: [{idx}][{entry_index}]");
    #[allow(unreachable_code)]
    unreachable!()
}

fn fail_no_kind(kind: &str) -> ! {
    #[cfg(kani)]
    kani::assert(false, "no such entry kind on card");
    #[cfg(not(kani))]
    panic!("no {kind} entry on card");
    #[allow(unreachable_code)]
    unreachable!()
}

// ---------------------------------------------------------------------------
// Indexed API (use in proofs)
// ---------------------------------------------------------------------------

/// Call entry `entry_index` of card `idx` (Play / Hook / …).
///
/// Under Kani this goes through [`call_body_idx`] (per-card direct call
/// sites). A single shared `code*` call makes CBMC expand a ~4300-target
/// function-pointer IF-chain — the events-unit SAT cliff.
///
/// `inline(always)` so a `card_id!(…)` constant folds the match to one
/// arm and only that trampoline stays reachable.
#[inline(always)]
pub fn entry_idx(idx: usize, entry_index: usize, player: i32) -> Asked {
    #[cfg(kani)]
    {
        return call_body_idx(idx, entry_index, player);
    }
    #[cfg(not(kani))]
    {
        let card = by_idx(idx);
        let on = match card.on.get(entry_index) {
            Some(o) => o,
            None => fail_entry_oob(idx, entry_index),
        };
        body_of(on)(player)
    }
}

/// First `On::Play` of card `idx`.
#[inline(always)]
pub fn play_idx(idx: usize, player: i32) -> Asked {
    let card = by_idx(idx);
    for (i, on) in card.on.iter().enumerate() {
        if matches!(on, On::Play(..)) {
            return entry_idx(idx, i, player);
        }
    }
    fail_no_kind("Play")
}

/// First entry of card `idx` whose kind matches `kind` / `hook_name`.
///
/// `kind` is one of `"Play"`, `"Counteract"`, `"Hook"`, `"Gate"`, `"AtEnd"`,
/// `"RollPlan"`, `"Settle"`, `"Message"`. For `Hook`, `hook_name` may filter
/// by `HookKind` short name (`"PayAdd"`); `""` matches the first hook.
///
/// `kind` / `hook_name` are short ASCII tags (not card ids). Parsed to
/// integers first so the `On` dispatch is enum compares, not `memcmp`.
pub fn hook_idx(idx: usize, kind: &str, hook_name: &str, player: i32) -> Asked {
    let card = by_idx(idx);
    let ktag = kind_tag(kind);
    for (i, on) in card.on.iter().enumerate() {
        let hit = match (ktag, on) {
            (KindTag::Play, On::Play(..)) => true,
            (KindTag::Counteract, On::Counteract(..)) => true,
            (KindTag::Hook, On::Hook(kinds, ..)) => {
                hook_name.is_empty() || hook_kind_matches(kinds, hook_name)
            }
            (KindTag::Gate, On::Gate(..)) => true,
            (KindTag::AtEnd, On::AtEnd(..)) => true,
            (KindTag::RollPlan, On::RollPlan(..)) => true,
            (KindTag::Settle, On::Settle(..)) => true,
            (KindTag::Message, On::Message(..)) => true,
            _ => false,
        };
        if hit {
            return entry_idx(idx, i, player);
        }
    }
    fail_no_kind(kind)
}

/// On-entry kind tag (integer, no `memcmp`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum KindTag {
    Play,
    Counteract,
    Hook,
    Gate,
    AtEnd,
    RollPlan,
    Settle,
    Message,
    Other,
}

/// memcmp-free short-tag key: `(len, first 8 bytes LE, next 8 bytes LE)`.
/// `match s { "lit" => … }` lowers to `memcmp` (6+ CBMC unrolls per arm);
/// packing to integers keeps the dispatch O(1) with zero memcmp.
#[inline(always)]
fn tag_key(s: &str) -> (u8, u64, u64) {
    let b = s.as_bytes();
    let mut lo = 0u64;
    let mut hi = 0u64;
    if let Some(&c) = b.get(0) {
        lo |= c as u64;
    }
    if let Some(&c) = b.get(1) {
        lo |= (c as u64) << 8;
    }
    if let Some(&c) = b.get(2) {
        lo |= (c as u64) << 16;
    }
    if let Some(&c) = b.get(3) {
        lo |= (c as u64) << 24;
    }
    if let Some(&c) = b.get(4) {
        lo |= (c as u64) << 32;
    }
    if let Some(&c) = b.get(5) {
        lo |= (c as u64) << 40;
    }
    if let Some(&c) = b.get(6) {
        lo |= (c as u64) << 48;
    }
    if let Some(&c) = b.get(7) {
        lo |= (c as u64) << 56;
    }
    if let Some(&c) = b.get(8) {
        hi |= c as u64;
    }
    if let Some(&c) = b.get(9) {
        hi |= (c as u64) << 8;
    }
    if let Some(&c) = b.get(10) {
        hi |= (c as u64) << 16;
    }
    if let Some(&c) = b.get(11) {
        hi |= (c as u64) << 24;
    }
    if let Some(&c) = b.get(12) {
        hi |= (c as u64) << 32;
    }
    if let Some(&c) = b.get(13) {
        hi |= (c as u64) << 40;
    }
    if let Some(&c) = b.get(14) {
        hi |= (c as u64) << 48;
    }
    if let Some(&c) = b.get(15) {
        hi |= (c as u64) << 56;
    }
    (b.len() as u8, lo, hi)
}

/// Const twin of [`tag_key`] so the match arms are integer constants.
const fn tag_key_const(s: &str) -> (u8, u64, u64) {
    let b = s.as_bytes();
    let mut lo = 0u64;
    let mut hi = 0u64;
    let mut i = 0;
    while i < 8 && i < b.len() {
        lo |= (b[i] as u64) << (8 * i);
        i += 1;
    }
    while i < 16 && i < b.len() {
        hi |= (b[i] as u64) << (8 * (i - 8));
        i += 1;
    }
    (b.len() as u8, lo, hi)
}

#[inline(always)]
fn kind_tag(kind: &str) -> KindTag {
    const PLAY: (u8, u64, u64) = tag_key_const("Play");
    const COUNTERACT: (u8, u64, u64) = tag_key_const("Counteract");
    const HOOK: (u8, u64, u64) = tag_key_const("Hook");
    const GATE: (u8, u64, u64) = tag_key_const("Gate");
    const AT_END: (u8, u64, u64) = tag_key_const("AtEnd");
    const ROLL_PLAN: (u8, u64, u64) = tag_key_const("RollPlan");
    const SETTLE: (u8, u64, u64) = tag_key_const("Settle");
    const MESSAGE: (u8, u64, u64) = tag_key_const("Message");
    match tag_key(kind) {
        PLAY => KindTag::Play,
        COUNTERACT => KindTag::Counteract,
        HOOK => KindTag::Hook,
        GATE => KindTag::Gate,
        AT_END => KindTag::AtEnd,
        ROLL_PLAN => KindTag::RollPlan,
        SETTLE => KindTag::Settle,
        MESSAGE => KindTag::Message,
        _ => KindTag::Other,
    }
}

/// First `On::Counteract` of card `idx`.
pub fn counteract_idx(idx: usize, player: i32) -> Asked {
    hook_idx(idx, "Counteract", "", player)
}

/// First `On::Hook` of card `idx`, optionally filtered by hook-kind name.
#[inline(always)]
pub fn hook_only_idx(idx: usize, hook_name: &str, player: i32) -> Asked {
    hook_idx(idx, "Hook", hook_name, player)
}

/// First `On::Hook` of card `idx` matching `want`. **Integer-only** — no
/// `&str` tags, so CBMC never lowers the dispatch to `memcmp` (the shave
/// profile's remaining 10 memcmps were `match name { "PayAdd" => … }`).
#[inline(always)]
pub fn hook_kind_only_idx(idx: usize, want: card_sdk::abi::HookKind, player: i32) -> Asked {
    let card = by_idx(idx);
    for (i, on) in card.on.iter().enumerate() {
        if let On::Hook(kinds, _, _, _) = on {
            // Zero-loop kind match (`iter().any` over a short slice unrolls
            // to the global bound under CBMC).
            let mut hit = false;
            if let Some(&k0) = kinds.get(0) {
                if k0 == want {
                    hit = true;
                }
            }
            if let Some(&k1) = kinds.get(1) {
                if k1 == want {
                    hit = true;
                }
            }
            if let Some(&k2) = kinds.get(2) {
                if k2 == want {
                    hit = true;
                }
            }
            if let Some(&k3) = kinds.get(3) {
                if k3 == want {
                    hit = true;
                }
            }
            if hit {
                return entry_idx(idx, i, player);
            }
        }
    }
    fail_no_kind("Hook")
}

/// Skill entry: alias of [`play_idx`].
#[inline(always)]
pub fn skill_play_idx(idx: usize, player: i32) -> Asked {
    play_idx(idx, player)
}

/// Event entry: alias of [`play_idx`].
#[inline(always)]
pub fn event_play_idx(idx: usize, player: i32) -> Asked {
    play_idx(idx, player)
}

// ---------------------------------------------------------------------------
// String API (native / tests — O(n) find; NOT for Kani proofs)
// ---------------------------------------------------------------------------

/// Call entry `entry_index` of `id`.
pub fn entry(id: &str, entry_index: usize, player: i32) -> Asked {
    let idx = find_idx(id).unwrap_or_else(|| {
        #[cfg(kani)]
        kani::assert(false, "unknown card/skill/event id");
        #[cfg(not(kani))]
        panic!("unknown card/skill/event id: {id}");
        #[allow(unreachable_code)]
        0
    });
    entry_idx(idx, entry_index, player)
}

/// First `On::Play` of `id`.
pub fn play(id: &str, player: i32) -> Asked {
    let idx = find_idx(id).unwrap_or_else(|| {
        #[cfg(kani)]
        kani::assert(false, "unknown card/skill/event id");
        #[cfg(not(kani))]
        panic!("unknown card/skill/event id: {id}");
        #[allow(unreachable_code)]
        0
    });
    play_idx(idx, player)
}

/// First hook/counteract/gate/settle/message entry of `id`. See [`hook_idx`].
pub fn hook(id: &str, kind: &str, hook_name: &str, player: i32) -> Asked {
    let idx = find_idx(id).unwrap_or_else(|| {
        #[cfg(kani)]
        kani::assert(false, "unknown card/skill/event id");
        #[cfg(not(kani))]
        panic!("unknown card/skill/event id: {id}");
        #[allow(unreachable_code)]
        0
    });
    hook_idx(idx, kind, hook_name, player)
}

/// First `On::Counteract` of `id`.
pub fn counteract(id: &str, player: i32) -> Asked {
    hook(id, "Counteract", "", player)
}

/// First `On::Hook` of `id`, optionally filtered by hook-kind name.
pub fn hook_only(id: &str, hook_name: &str, player: i32) -> Asked {
    hook(id, "Hook", hook_name, player)
}

/// Skill entry: alias of [`play`].
pub fn skill_play(id: &str, player: i32) -> Asked {
    play(id, player)
}

/// Event entry: alias of [`play`].
pub fn event_play(id: &str, player: i32) -> Asked {
    play(id, player)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Match a `HookKind` by its short name without `format!` (no String/UTF-8).
///
/// Parse `hook_name` to a `HookKind` first (a `match` on literals that
/// constant-folds when the caller passes `"PayAdd"`), then an integer
/// compare against `kinds`. The old `hook_kind_name(k) == hook_name` lowered
/// to `memcmp` per arm (14 unwinds in the shave profile).
fn hook_kind_matches(kinds: &[card_sdk::abi::HookKind], hook_name: &str) -> bool {
    let want = hook_kind_parse(hook_name);
    match want {
        Some(w) => {
            // Zero-loop (see `hook_kind_only_idx`).
            let mut hit = false;
            if let Some(&k0) = kinds.get(0) {
                if k0 == w {
                    hit = true;
                }
            }
            if let Some(&k1) = kinds.get(1) {
                if k1 == w {
                    hit = true;
                }
            }
            if let Some(&k2) = kinds.get(2) {
                if k2 == w {
                    hit = true;
                }
            }
            if let Some(&k3) = kinds.get(3) {
                if k3 == w {
                    hit = true;
                }
            }
            hit
        }
        None => false,
    }
}

/// Short name → `HookKind`. `None` for `""` or an unknown tag.
/// Dispatch is on [`tag_key`] integers — never `memcmp`.
#[inline(always)]
fn hook_kind_parse(name: &str) -> Option<card_sdk::abi::HookKind> {
    use card_sdk::abi::HookKind as H;
    const PAY_ADD: (u8, u64, u64) = tag_key_const("PayAdd");
    const PAY_MUL: (u8, u64, u64) = tag_key_const("PayMul");
    const PAY_CHOOSE: (u8, u64, u64) = tag_key_const("PayChoose");
    const PAY_AT: (u8, u64, u64) = tag_key_const("PayAt");
    const PAY_AFTER: (u8, u64, u64) = tag_key_const("PayAfter");
    const PAY: (u8, u64, u64) = tag_key_const("Pay");
    const TURN_END: (u8, u64, u64) = tag_key_const("TurnEnd");
    const TURN_START: (u8, u64, u64) = tag_key_const("TurnStart");
    const TURN_END_BEFORE: (u8, u64, u64) = tag_key_const("TurnEndBefore");
    const TURN_END_AFTER: (u8, u64, u64) = tag_key_const("TurnEndAfter");
    const ROLL: (u8, u64, u64) = tag_key_const("Roll");
    const ROLL_AFTER: (u8, u64, u64) = tag_key_const("RollAfter");
    const ROLL_PLAN: (u8, u64, u64) = tag_key_const("RollPlan");
    const SETTLE: (u8, u64, u64) = tag_key_const("Settle");
    const SETTLE_BODY: (u8, u64, u64) = tag_key_const("SettleBody");
    const SETTLE_AFTER: (u8, u64, u64) = tag_key_const("SettleAfter");
    const CARD_PLAYED: (u8, u64, u64) = tag_key_const("CardPlayed");
    const DISCARDED: (u8, u64, u64) = tag_key_const("Discarded");
    const DISCARD_BEFORE: (u8, u64, u64) = tag_key_const("DiscardBefore");
    const DISCARD_AFTER: (u8, u64, u64) = tag_key_const("DiscardAfter");
    const BOUGHT: (u8, u64, u64) = tag_key_const("Bought");
    const BUY_ADD: (u8, u64, u64) = tag_key_const("BuyAdd");
    const BUY_MUL: (u8, u64, u64) = tag_key_const("BuyMul");
    const BUY_SET: (u8, u64, u64) = tag_key_const("BuySet");
    const HOUSE_ADDED: (u8, u64, u64) = tag_key_const("HouseAdded");
    const COUNTER_CHANGED: (u8, u64, u64) = tag_key_const("CounterChanged");
    const FIRE_SPENT: (u8, u64, u64) = tag_key_const("FireSpent");
    const TELEPORTED: (u8, u64, u64) = tag_key_const("Teleported");
    const TARGETED: (u8, u64, u64) = tag_key_const("Targeted");
    const PASS: (u8, u64, u64) = tag_key_const("Pass");
    const PASS_TILE: (u8, u64, u64) = tag_key_const("PassTile");
    const PASS_PLAYER: (u8, u64, u64) = tag_key_const("PassPlayer");
    const EXILE: (u8, u64, u64) = tag_key_const("Exile");
    const STAY: (u8, u64, u64) = tag_key_const("Stay");
    const STUN: (u8, u64, u64) = tag_key_const("Stun");
    match tag_key(name) {
        PAY_ADD => Some(H::PayAdd),
        PAY_MUL => Some(H::PayMul),
        PAY_CHOOSE => Some(H::PayChoose),
        PAY_AT => Some(H::PayAt),
        PAY_AFTER => Some(H::PayAfter),
        PAY => Some(H::Pay),
        TURN_END => Some(H::TurnEnd),
        TURN_START => Some(H::TurnStart),
        TURN_END_BEFORE => Some(H::TurnEndBefore),
        TURN_END_AFTER => Some(H::TurnEndAfter),
        ROLL => Some(H::Roll),
        ROLL_AFTER => Some(H::RollAfter),
        ROLL_PLAN => Some(H::RollPlan),
        SETTLE => Some(H::Settle),
        SETTLE_BODY => Some(H::SettleBody),
        SETTLE_AFTER => Some(H::SettleAfter),
        CARD_PLAYED => Some(H::CardPlayed),
        DISCARDED => Some(H::Discarded),
        DISCARD_BEFORE => Some(H::DiscardBefore),
        DISCARD_AFTER => Some(H::DiscardAfter),
        BOUGHT => Some(H::Bought),
        BUY_ADD => Some(H::BuyAdd),
        BUY_MUL => Some(H::BuyMul),
        BUY_SET => Some(H::BuySet),
        HOUSE_ADDED => Some(H::HouseAdded),
        COUNTER_CHANGED => Some(H::CounterChanged),
        FIRE_SPENT => Some(H::FireSpent),
        TELEPORTED => Some(H::Teleported),
        TARGETED => Some(H::Targeted),
        PASS => Some(H::Pass),
        PASS_TILE => Some(H::PassTile),
        PASS_PLAYER => Some(H::PassPlayer),
        EXILE => Some(H::Exile),
        STAY => Some(H::Stay),
        STUN => Some(H::Stun),
        _ => None,
    }
}

fn hook_kind_name(k: card_sdk::abi::HookKind) -> &'static str {
    use card_sdk::abi::HookKind as H;
    match k {
        H::PayAdd => "PayAdd",
        H::PayMul => "PayMul",
        H::PayChoose => "PayChoose",
        H::PayAt => "PayAt",
        H::PayAfter => "PayAfter",
        H::Pay => "Pay",
        H::TurnEnd => "TurnEnd",
        H::TurnStart => "TurnStart",
        H::TurnEndBefore => "TurnEndBefore",
        H::TurnEndAfter => "TurnEndAfter",
        H::Roll => "Roll",
        H::RollAfter => "RollAfter",
        H::RollPlan => "RollPlan",
        H::Settle => "Settle",
        H::SettleBody => "SettleBody",
        H::SettleAfter => "SettleAfter",
        H::CardPlayed => "CardPlayed",
        H::Discarded => "Discarded",
        H::DiscardBefore => "DiscardBefore",
        H::DiscardAfter => "DiscardAfter",
        H::Bought => "Bought",
        H::BuyAdd => "BuyAdd",
        H::BuyMul => "BuyMul",
        H::BuySet => "BuySet",
        H::HouseAdded => "HouseAdded",
        H::CounterChanged => "CounterChanged",
        H::FireSpent => "FireSpent",
        H::Teleported => "Teleported",
        H::Targeted => "Targeted",
        H::Pass => "Pass",
        H::PassTile => "PassTile",
        H::PassPlayer => "PassPlayer",
        H::Exile => "Exile",
        H::Stay => "Stay",
        H::Stun => "Stun",
        _ => "",
    }
}

/// Treat `Err(Prompt)` as a hard failure for proofs that must complete
/// inline (prompts are answered in-host, so this should not fire).
pub fn unwrap_done(r: Asked) {
    match r {
        Ok(()) => {}
        Err(Prompt) => panic!("unexpected EXIT_NEED_INPUT"),
    }
}

// ---------------------------------------------------------------------------
// Compatibility aliases used by the pilot proofs (money_shave must keep
// compiling). These go through `card_id!` so they are scan-free under Kani.
// ---------------------------------------------------------------------------

/// `PP:[丸山彩]憧憬的前方` -- `On::Play`.
#[cfg(feature = "band-pp")]
pub fn aya_longing_play(player_id: i32) -> Asked {
    play_idx(crate::card_id!("PP:[丸山彩]憧憬的前方"), player_id)
}

/// `PP:[丸山彩]憧憬的前方` -- `On::Hook(PayAdd)`.
#[cfg(feature = "band-pp")]
pub fn aya_longing_pay_add(player_id: i32) -> Asked {
    hook_kind_only_idx(
        crate::card_id!("PP:[丸山彩]憧憬的前方"),
        card_sdk::abi::HookKind::PayAdd,
        player_id,
    )
}

/// `PPP:（多惠）寻找更好的声音` -- `On::Play`.
#[cfg(feature = "band-ppp")]
pub fn tae_sound_play(player_id: i32) -> Asked {
    play_idx(crate::card_id!("PPP:（多惠）寻找更好的声音"), player_id)
}

/// `MyGO:轮符雨` -- `On::Play`.
#[cfg(feature = "band-mygo")]
pub fn rinne_rain_play(player_id: i32) -> Asked {
    play_idx(crate::card_id!("MyGO:轮符雨"), player_id)
}