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
    card_ag::CARDS,
    card_crychic::CARDS,
    card_general::CARDS,
    card_hhw::CARDS,
    card_morfonica::CARDS,
    card_mujica::CARDS,
    card_mygo::CARDS,
    card_pp::CARDS,
    card_ppp::CARDS,
    card_ras::CARDS,
    card_roselia::CARDS,
    card_sumimi::CARDS,
    skill_bands::CARDS,
    skill_characters::CARDS,
    rules_tiles::CARDS,
    rules_tile_marks::CARDS,
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

/// Index `idx` of the flattened [`RULESET`] (integer arithmetic, **no**
/// string compare). Fails loudly on out-of-range.
pub fn by_idx(idx: usize) -> &'static card_sdk::CardDef {
    let mut left = idx;
    let mut b = 0usize;
    while b < RULESET.len() {
        let band = RULESET[b];
        if left < band.len() {
            return &band[left];
        }
        left -= band.len();
        b += 1;
    }
    fail_unknown_idx(idx)
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
/// Use [`by_idx`] + `card_id!` in proofs.
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
pub fn entry_idx(idx: usize, entry_index: usize, player: i32) -> Asked {
    let card = by_idx(idx);
    let on = match card.on.get(entry_index) {
        Some(o) => o,
        None => fail_entry_oob(idx, entry_index),
    };
    body_of(on)(player)
}

/// First `On::Play` of card `idx`.
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
/// `kind` / `hook_name` are short ASCII tags (not card ids) — the compare is
/// a handful of bytes, fine under CBMC.
pub fn hook_idx(idx: usize, kind: &str, hook_name: &str, player: i32) -> Asked {
    let card = by_idx(idx);
    for (i, on) in card.on.iter().enumerate() {
        let hit = match (kind, on) {
            ("Play", On::Play(..)) => true,
            ("Counteract", On::Counteract(..)) => true,
            ("Hook", On::Hook(kinds, ..)) => {
                hook_name.is_empty() || hook_kind_matches(kinds, hook_name)
            }
            ("Gate", On::Gate(..)) => true,
            ("AtEnd", On::AtEnd(..)) => true,
            ("RollPlan", On::RollPlan(..)) => true,
            ("Settle", On::Settle(..)) => true,
            ("Message", On::Message(..)) => true,
            _ => false,
        };
        if hit {
            return entry_idx(idx, i, player);
        }
    }
    fail_no_kind(kind)
}

/// First `On::Counteract` of card `idx`.
pub fn counteract_idx(idx: usize, player: i32) -> Asked {
    hook_idx(idx, "Counteract", "", player)
}

/// First `On::Hook` of card `idx`, optionally filtered by hook-kind name.
pub fn hook_only_idx(idx: usize, hook_name: &str, player: i32) -> Asked {
    hook_idx(idx, "Hook", hook_name, player)
}

/// Skill entry: alias of [`play_idx`].
pub fn skill_play_idx(idx: usize, player: i32) -> Asked {
    play_idx(idx, player)
}

/// Event entry: alias of [`play_idx`].
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
fn hook_kind_matches(kinds: &[card_sdk::abi::HookKind], hook_name: &str) -> bool {
    kinds.iter().any(|k| hook_kind_name(*k) == hook_name)
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
pub fn aya_longing_play(player_id: i32) -> Asked {
    play_idx(crate::card_id!("PP:[丸山彩]憧憬的前方"), player_id)
}

/// `PP:[丸山彩]憧憬的前方` -- `On::Hook(PayAdd)`.
pub fn aya_longing_pay_add(player_id: i32) -> Asked {
    hook_only_idx(crate::card_id!("PP:[丸山彩]憧憬的前方"), "PayAdd", player_id)
}

/// `PPP:（多惠）寻找更好的声音` -- `On::Play`.
pub fn tae_sound_play(player_id: i32) -> Asked {
    play_idx(crate::card_id!("PPP:（多惠）寻找更好的声音"), player_id)
}

/// `MyGO:轮符雨` -- `On::Play`.
pub fn rinne_rain_play(player_id: i32) -> Asked {
    play_idx(crate::card_id!("MyGO:轮符雨"), player_id)
}