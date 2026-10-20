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
    #[cfg(feature = "skills-groups")]
    skill_groups::CARDS,
    #[cfg(feature = "skills-personas")]
    skill_personas::CARDS,
    #[cfg(feature = "tiles")]
    rules_tiles::CARDS,
    #[cfg(feature = "tiles")]
    rules_markers::CARDS,
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
        On::Play(_, _, _, f)
        | On::Counteract(_, _, _, _, f)
        | On::Action(_, _, _, _, f)
        | On::Settle(_, _, f)
        | On::Mortgage(_, _, f)
        | On::Redeem(_, _, f)
        | On::Buy(_, _, f)
        | On::Build(_, _, f)
        | On::Message(_, _, _, f) => *f,
        // Filter bodies are not `fn(i32) -> Asked`; kani proofs call them
        // through `filter_*_idx` / direct trampolines instead.
        On::Filter(..) | On::FilterFlag(..) | On::FilterValue(..) => {
            fail_no_kind("Filter (use filter_idx)")
        }
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

/// Const-generic play: `IDX` monomorphizes `call_body_idx_c` to ONE card
/// trampoline. Prefer this over [`play_idx`] in proofs — a runtime `idx`
/// keeps every trampoline arm live in CBMC (ppcost residual after the
/// code* IF-chain cut).
#[inline(always)]
pub fn play_c<const IDX: usize>(player: i32) -> Asked {
    let card = by_idx(IDX);
    for (i, on) in card.on.iter().enumerate() {
        if matches!(on, On::Play(..)) {
            #[cfg(kani)]
            {
                return call_body_idx_c::<IDX>(i, player);
            }
            #[cfg(not(kani))]
            {
                let on = match card.on.get(i) {
                    Some(o) => o,
                    None => fail_entry_oob(IDX, i),
                };
                return body_of(on)(player);
            }
        }
    }
    fail_no_kind("Play")
}

/// Const-generic entry (Play / Hook / …) of card `IDX`.
#[inline(always)]
pub fn entry_c<const IDX: usize>(entry_index: usize, player: i32) -> Asked {
    #[cfg(kani)]
    {
        return call_body_idx_c::<IDX>(entry_index, player);
    }
    #[cfg(not(kani))]
    {
        let card = by_idx(IDX);
        let on = match card.on.get(entry_index) {
            Some(o) => o,
            None => fail_entry_oob(IDX, entry_index),
        };
        body_of(on)(player)
    }
}

/// First entry of card `idx` whose kind matches `kind` / `hook_name`.
///
/// `kind` is one of `"Play"`, `"Counteract"`, `"Action"` /
/// `"Filter"`, `"Gate"`, `"AtEnd"`,
/// `"DiceRollPlan"`, `"Settle"`, `"Message"`. For `Action`/`Filter`, `hook_name` may filter
/// by kind short name (`"PaySplitAdd"`); `""` matches the first hook.
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
            (KindTag::Action, On::Action(kinds, ..)) => {
                hook_name.is_empty()
                    || hook_kind_matches(kinds, |k| k.as_cel_i64() as i32, hook_name)
            }
            (KindTag::Filter, On::Filter(kinds, ..) | On::FilterFlag(kinds, ..) | On::FilterValue(kinds, ..)) => {
                hook_name.is_empty()
                    || hook_kind_matches(kinds, |k| k.as_cel_i64() as i32, hook_name)
            }
            (KindTag::Gate, On::FilterFlag(kinds, ..) | On::FilterValue(kinds, ..)) => {
                hook_name.is_empty()
                    || hook_kind_matches(kinds, |k| k.as_cel_i64() as i32, hook_name)
            }
            (KindTag::AtEnd, On::Action(kinds, ..)) => {
                hook_name.is_empty()
                    || hook_kind_matches(kinds, |k| k.as_cel_i64() as i32, hook_name)
            }
            (KindTag::DiceRollPlan, On::FilterValue(kinds, ..)) => {
                hook_name.is_empty()
                    || hook_kind_matches(kinds, |k| k.as_cel_i64() as i32, hook_name)
            }
            (KindTag::Settle, On::Settle(..)) => true,
            (KindTag::Mortgage, On::Mortgage(..)) => true,
            (KindTag::Redeem, On::Redeem(..)) => true,
            (KindTag::Buy, On::Buy(..)) => true,
            (KindTag::Build, On::Build(..)) => true,
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
    Action,
    Filter,
    Gate,
    AtEnd,
    DiceRollPlan,
    Settle,
    Message,
    Mortgage,
    Redeem,
    Buy,
    Build,
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
    const ACTION: (u8, u64, u64) = tag_key_const("Action");
    const FILTER: (u8, u64, u64) = tag_key_const("Filter");
    const GATE: (u8, u64, u64) = tag_key_const("Gate");
    const AT_END: (u8, u64, u64) = tag_key_const("AtEnd");
    const ROLL_PLAN: (u8, u64, u64) = tag_key_const("DiceRollPlan");
    const SETTLE: (u8, u64, u64) = tag_key_const("Settle");
    const MESSAGE: (u8, u64, u64) = tag_key_const("Message");
    const MORTGAGE: (u8, u64, u64) = tag_key_const("Mortgage");
    const REDEEM: (u8, u64, u64) = tag_key_const("Redeem");
    const BUY: (u8, u64, u64) = tag_key_const("Buy");
    const BUILD: (u8, u64, u64) = tag_key_const("Build");
    match tag_key(kind) {
        PLAY => KindTag::Play,
        COUNTERACT => KindTag::Counteract,
        ACTION => KindTag::Action,
        FILTER => KindTag::Filter,
        GATE => KindTag::Gate,
        AT_END => KindTag::AtEnd,
        ROLL_PLAN => KindTag::DiceRollPlan,
        SETTLE => KindTag::Settle,
        MESSAGE => KindTag::Message,
        MORTGAGE => KindTag::Mortgage,
        REDEEM => KindTag::Redeem,
        BUY => KindTag::Buy,
        BUILD => KindTag::Build,
        _ => KindTag::Other,
    }
}

/// First `On::Counteract` of card `idx`.
pub fn counteract_idx(idx: usize, player: i32) -> Asked {
    hook_idx(idx, "Counteract", "", player)
}

/// First `On::Action` of card `idx`, optionally filtered by hook-kind name.
#[inline(always)]
pub fn hook_only_idx(idx: usize, hook_name: &str, player: i32) -> Asked {
    hook_idx(idx, "Action", hook_name, player)
}

/// First `On::Action` of card `idx` matching `want`. **Integer-only** — no
/// `&str` tags, so CBMC never lowers the dispatch to `memcmp` (the shave
/// profile's remaining 10 memcmps were `match name { "PaySplitAdd" => … }`).
#[inline(always)]
pub fn hook_kind_only_idx(idx: usize, want: card_sdk::abi::ActionKind, player: i32) -> Asked {
    let card = by_idx(idx);
    for (i, on) in card.on.iter().enumerate() {
        if let On::Action(kinds, _, _, _, _) = on {
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
    fail_no_kind("Action")
}

/// First `On::Filter` of card `idx` matching `want`; returns the transformed
/// value (v74 pure body). **Integer-only** kind match.
#[inline(always)]
pub fn filter_kind_only_idx(
    idx: usize,
    want: card_sdk::abi::FilterKind,
    player: i32,
    value: i32,
) -> i32 {
    let card = by_idx(idx);
    for on in card.on.iter() {
        if let On::Filter(kinds, _, _, run) = on {
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
            if hit {
                return run(player, value);
            }
        }
    }
    fail_no_kind("Filter")
}

/// First `On::FilterFlag` of card `idx` matching `want` (true = keep).
#[inline(always)]
pub fn filter_flag_kind_only_idx(
    idx: usize,
    want: card_sdk::abi::FilterKind,
    player: i32,
    value: i32,
) -> bool {
    let card = by_idx(idx);
    for on in card.on.iter() {
        if let On::FilterFlag(kinds, _, _, run) = on {
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
            if hit {
                return run(player, value);
            }
        }
    }
    fail_no_kind("FilterFlag")
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

/// First `On::Action` of `id`, optionally filtered by hook-kind name.
pub fn hook_only(id: &str, hook_name: &str, player: i32) -> Asked {
    hook(id, "Action", hook_name, player)
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

/// Match a kind list entry by its short name without `format!` (no String/UTF-8).
///
/// Parse `hook_name` to a [`card_sdk::abi::TriggerKind`] first (a `match` on
/// literals that constant-folds when the caller passes `"PaySplitAdd"`), then
/// an integer compare against `kinds`. Works for both [`card_sdk::abi::ActionKind`]
/// and [`card_sdk::abi::FilterKind`] slices (same wire id space). The old
/// `hook_kind_name(k) == hook_name` lowered to `memcmp` per arm (14 unwinds in
/// the shave profile).
fn hook_kind_matches<K: Copy>(kinds: &[K], id_of: fn(K) -> i32, hook_name: &str) -> bool {
    let want = match hook_kind_parse(hook_name) {
        Some(w) => w.as_cel_i64() as i32,
        None => return false,
    };
    // Zero-loop (see `hook_kind_only_idx`).
    let mut hit = false;
    if let Some(&k0) = kinds.get(0) {
        if id_of(k0) == want {
            hit = true;
        }
    }
    if let Some(&k1) = kinds.get(1) {
        if id_of(k1) == want {
            hit = true;
        }
    }
    if let Some(&k2) = kinds.get(2) {
        if id_of(k2) == want {
            hit = true;
        }
    }
    if let Some(&k3) = kinds.get(3) {
        if id_of(k3) == want {
            hit = true;
        }
    }
    hit
}

/// Short name → wire [`card_sdk::abi::TriggerKind`]. `None` for `""` or an
/// unknown tag. Dispatch is on [`tag_key`] integers — never `memcmp`.
#[inline(always)]
fn hook_kind_parse(name: &str) -> Option<card_sdk::abi::TriggerKind> {
    use card_sdk::abi::TriggerKind as T;
    const PAY_ADD: (u8, u64, u64) = tag_key_const("PaySplitAdd");
    const PAY_MUL: (u8, u64, u64) = tag_key_const("PaySplitMul");
    const PAY_CHOOSE: (u8, u64, u64) = tag_key_const("PaySplitChoose");
    const PAY_AT: (u8, u64, u64) = tag_key_const("PaySplitAt");
    const PAY_AFTER: (u8, u64, u64) = tag_key_const("PayAfter");
    const PAY: (u8, u64, u64) = tag_key_const("Pay");
    const TURN_END: (u8, u64, u64) = tag_key_const("TurnEnd");
    const TURN_START: (u8, u64, u64) = tag_key_const("TurnStart");
    const TURN_END_BEFORE: (u8, u64, u64) = tag_key_const("TurnEndBefore");
    const TURN_END_AFTER: (u8, u64, u64) = tag_key_const("TurnEndAfter");
    const ROLL: (u8, u64, u64) = tag_key_const("Roll");
    const ROLL_AFTER: (u8, u64, u64) = tag_key_const("DiceRollAfter");
    const ROLL_PLAN: (u8, u64, u64) = tag_key_const("DiceRollPlan");
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
    const MARKER_SPENT: (u8, u64, u64) = tag_key_const("MarkerSpent");
    const CARD_USED: (u8, u64, u64) = tag_key_const("CardUsed");
    const COUNTER_CHANGED: (u8, u64, u64) = tag_key_const("CounterChanged");
    const FANS_FLIP: (u8, u64, u64) = tag_key_const("FansFlip");
    const TELEPORTED: (u8, u64, u64) = tag_key_const("Teleported");
    const TARGETED: (u8, u64, u64) = tag_key_const("Targeted");
    const PASS: (u8, u64, u64) = tag_key_const("Pass");
    const PASS_TILE: (u8, u64, u64) = tag_key_const("PassTile");
    const PASS_PLAYER: (u8, u64, u64) = tag_key_const("PassPlayer");
    const EXILE: (u8, u64, u64) = tag_key_const("Exile");
    match tag_key(name) {
        PAY_ADD => Some(T::PaySplitAdd),
        PAY_MUL => Some(T::PaySplitMul),
        PAY_CHOOSE => Some(T::PaySplitChoose),
        PAY_AT => Some(T::PaySplitAt),
        PAY_AFTER => Some(T::PayAfter),
        PAY => Some(T::Pay),
        TURN_END => Some(T::TurnEnd),
        TURN_START => Some(T::TurnStart),
        TURN_END_BEFORE => Some(T::TurnEndBefore),
        TURN_END_AFTER => Some(T::TurnEndAfter),
        ROLL => Some(T::DiceRollBefore),
        ROLL_AFTER => Some(T::DiceRollAfter),
        ROLL_PLAN => Some(T::DiceRollPlan),
        SETTLE => Some(T::Settle),
        SETTLE_BODY => Some(T::SettleBody),
        SETTLE_AFTER => Some(T::SettleAfter),
        CARD_PLAYED => Some(T::CardPlayed),
        DISCARDED => Some(T::Discarded),
        DISCARD_BEFORE => Some(T::DiscardBefore),
        DISCARD_AFTER => Some(T::DiscardAfter),
        BOUGHT => Some(T::Bought),
        BUY_ADD => Some(T::BuyAdd),
        BUY_MUL => Some(T::BuyMul),
        BUY_SET => Some(T::BuySet),
        HOUSE_ADDED => Some(T::HouseAdded),
        MARKER_SPENT => Some(T::MarkerSpent),
        CARD_USED => Some(T::CardUsed),
        COUNTER_CHANGED => Some(T::CounterChanged),
        FANS_FLIP => Some(bandori_std::kind::FansFlip.as_trigger()),
        TELEPORTED => Some(T::Teleported),
        TARGETED => Some(T::Targeted),
        PASS => Some(T::Pass),
        PASS_TILE => Some(T::PassTile),
        PASS_PLAYER => Some(T::PassPlayer),
        EXILE => Some(bandori_std::kind::Exile.as_trigger()),
        _ => None,
    }
}

fn hook_kind_name(k: card_sdk::abi::TriggerKind) -> &'static str {
    use card_sdk::abi::TriggerKind as T;
    if k == T::HouseAdded {
        return "HouseAdded";
    }
    if k == T::MarkerSpent {
        return "MarkerSpent";
    }
    if k == T::CardUsed {
        return "CardUsed";
    }
    if k == bandori_std::kind::FansFlip.as_trigger() {
        return "FansFlip";
    }
    match k {
        T::PaySplitAdd => "PaySplitAdd",
        T::PaySplitMul => "PaySplitMul",
        T::PaySplitChoose => "PaySplitChoose",
        T::PaySplitAt => "PaySplitAt",
        T::PayAfter => "PayAfter",
        T::Pay => "Pay",
        T::TurnEnd => "TurnEnd",
        T::TurnStart => "TurnStart",
        T::TurnEndBefore => "TurnEndBefore",
        T::TurnEndAfter => "TurnEndAfter",
        T::DiceRollBefore => "Roll",
        T::DiceRollAfter => "DiceRollAfter",
        T::DiceRollPlan => "DiceRollPlan",
        T::Settle => "Settle",
        T::SettleBody => "SettleBody",
        T::SettleAfter => "SettleAfter",
        T::CardPlayed => "CardPlayed",
        T::Discarded => "Discarded",
        T::DiscardBefore => "DiscardBefore",
        T::DiscardAfter => "DiscardAfter",
        T::Bought => "Bought",
        T::BuyAdd => "BuyAdd",
        T::BuyMul => "BuyMul",
        T::BuySet => "BuySet",
        T::CounterChanged => "CounterChanged",
        T::Teleported => "Teleported",
        T::Targeted => "Targeted",
        T::Pass => "Pass",
        T::PassTile => "PassTile",
        T::PassPlayer => "PassPlayer",
        _ if k == bandori_std::kind::Exile.as_trigger() => "Exile",
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

/// `PP:[丸山彩]憧憬的前方` -- `On::Action(Effect)` (was PaySplitAdd; the interactive
/// resonance shave moved to the payment declaration so `set_pay_amount` still
/// lands -- see the card's Pattern E comment).
#[cfg(feature = "band-pp")]
pub fn aya_longing_pay_add(player_id: i32) -> Asked {
    hook_kind_only_idx(
        crate::card_id!("PP:[丸山彩]憧憬的前方"),
        card_sdk::abi::ActionKind::Effect,
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