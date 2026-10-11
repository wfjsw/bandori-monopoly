//! Registry-based invocation of shipped `CardDef` / skill / event entries.
//!
//! Replaces hand-written per-card invokers: look the definition up by id in
//! the same `RULESET` table `rules-native` links, then call the entry at
//! `entry_index` (or the first entry of a given kind). Unknown id / kind /
//! index fails **loudly** (`panic!` / `kani::assert(false)`), never silently.

use card_sdk::{Asked, On, Prompt};

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

/// Linear scan of [`RULESET`] for `id`. `None` when unknown.
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

/// Call entry `entry_index` of `id` (Play / Hook / …).
pub fn entry(id: &str, entry_index: usize, player: i32) -> Asked {
    let card = find_or_fail(id);
    let on = match card.on.get(entry_index) {
        Some(o) => o,
        None => {
            #[cfg(kani)]
            kani::assert(false, "entry index out of range");
            #[cfg(not(kani))]
            panic!("entry index out of range: {id}[{entry_index}]");
            #[allow(unreachable_code)]
            unreachable!()
        }
    };
    body_of(on)(player)
}

/// First `On::Play` of `id`.
pub fn play(id: &str, player: i32) -> Asked {
    let card = find_or_fail(id);
    for (i, on) in card.on.iter().enumerate() {
        if matches!(on, On::Play(..)) {
            return entry(id, i, player);
        }
    }
    fail_no_kind(id, "Play");
    unreachable!()
}

/// First hook/counteract/gate/settle/message entry whose kind matches `kind`
/// as a string (`"Hook"`, `"Counteract"`, `"Gate"`, `"Settle"`, `"Message"`,
/// `"AtEnd"`, `"RollPlan"`). For hooks that also carry a `HookKind`, pass
/// `hook_name` (e.g. `"PayAdd"`).
pub fn hook(id: &str, kind: &str, hook_name: &str, player: i32) -> Asked {
    let card = find_or_fail(id);
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
            return entry(id, i, player);
        }
    }
    fail_no_kind(id, kind);
    unreachable!()
}

/// First `On::Counteract` of `id` (any chain kind).
pub fn counteract(id: &str, player: i32) -> Asked {
    hook(id, "Counteract", "", player)
}

/// First `On::Hook` of `id`, optionally filtered by hook-kind debug name.
pub fn hook_only(id: &str, hook_name: &str, player: i32) -> Asked {
    hook(id, "Hook", hook_name, player)
}

/// Skill entry: skills live in `skill-bands` / `skill-characters` tables and
/// are ordinary `CardDef`s. Alias of [`play`] / [`hook`].
pub fn skill_play(id: &str, player: i32) -> Asked {
    play(id, player)
}

/// Event entry: events live in `rules-events`.
pub fn event_play(id: &str, player: i32) -> Asked {
    play(id, player)
}

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

fn fail_no_kind(id: &str, kind: &str) {
    #[cfg(kani)]
    kani::assert(false, "no such entry kind on card");
    #[cfg(not(kani))]
    panic!("no {kind} entry on {id}");
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
// compiling; do not edit that file).
// ---------------------------------------------------------------------------

/// `PP:[丸山彩]憧憬的前方` -- `On::Play`.
pub fn aya_longing_play(player_id: i32) -> Asked {
    play("PP:[丸山彩]憧憬的前方", player_id)
}

/// `PP:[丸山彩]憧憬的前方` -- `On::Hook(PayAdd)`.
pub fn aya_longing_pay_add(player_id: i32) -> Asked {
    hook_only("PP:[丸山彩]憧憬的前方", "PayAdd", player_id)
}

/// `PPP:（多惠）寻找更好的声音` -- `On::Play`.
pub fn tae_sound_play(player_id: i32) -> Asked {
    play("PPP:（多惠）寻找更好的声音", player_id)
}

/// `MyGO:轮符雨` -- `On::Play`.
pub fn rinne_rain_play(player_id: i32) -> Asked {
    play("MyGO:轮符雨", player_id)
}