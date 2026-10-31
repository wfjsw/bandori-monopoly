//! Character skills as card rules -- one file per skill, one crate for the lot.
//!
//! Deliberately **not** in `rules/cards/`: those are the cards you draw and
//! play, and these are the abilities a character comes with. Same rule language
//! ([`CardDef`]), same guest runtime, separate crate -- so a card author never
//! has to scroll past them and a skill author never edits a card.
//!
//! The rule text lives in the roster character row (`skill` is the name, `text` /
//! `simple` the wording); what is here is the behavior. A skill that prints
//! 「（初始N，上限M）」 owns those numbers on its own [`CardDef`] (`.props(&[(fire::FIRE_INITIAL, N), (fire::FIRE_CAP, M)])`
//! / `bandori_std::fire::fire_pots`), and `fire_pot(player)` →
//! `bandori_std::fire::declare_own` reads them back through `ctx::card_prop`.
//! Every card reads the pots with `bandori_std::fire::{get, max}`. The
//! engine holds the number and enforces nothing; see `game_core::state::StateVar`.
//!
//! # Hooks: use the two that exist
//!
//! A skill is not a third category. Everything it does is either **started by
//! the user** or **called by a field event**, and both already have a hook:
//!
//! - **Started by the user** -- a skill button (the viewer's `MatchView.skills`
//!   list). The player presses it, the effect runs. That is
//!   [`On::Play`], gate and effect: `On::Play("", Some(why_not), run, None)` where the
//!   gate is the "can I press this right now?" query (C# `Card.WhyNot`) and
//!   `run` is what the press does.
//! - **Called by a field event** -- the skill is a field card on the player
//!   (C# `Fx`, from `MatchPlayer.field`), which is why [`On::Action`] reaches it.
//!   It answers settlement points automatically, with no declaration and no
//!   press. A fire cap that has to hold from the moment the character is taken
//!   belongs here.
//!
//! So there is no `On::Skill`. Inventing one would be a third axis for a
//! question the two above already answer.
//!
//! # Binding
//!
//! The rule needs to know whose it is. The id carries that --
//! `skill:户山香澄:非凡之星` -- and `GameData::skill_rules_of` resolves a
//! player's two skills from the character they picked and their band. The
//! engine places them at match start (`bind_skills`), so `On::Action` reaches
//! them like any field card and `On::Play` answers the skill button. A card
//! that reaches a skill by name uses `ctx::persona_skill` /
//! `ctx::group_skill` + `ctx::invoke_skill` (ABI v35).
//!
//! Adding a skill is one file here plus an entry in [`CARDS`].

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use card_sdk::kinds::Action;
use card_sdk::CardDef;

/// 「初始N，上限M」 -- declare this seat's fire pots from **this skill's own**
/// `CardDef` props (`bandori_std::fire::{FIRE_INITIAL, FIRE_CAP}`, written by
/// the two `fire::` props). Bodies call `fire_pot(player)`; they do not carry
/// literals.
///
/// Hook this from both `TurnStartBefore` (so the cap holds from the first turn
/// even when the skill is placed mid-game) and `DeckAtGameStart` (where
/// 「初始N」 is created), e.g.
/// `On::Action(Action::TurnStartBefore("", None, declare_cap, None))` and
/// `On::Action(Action::DeckAtGameStart("", None, declare_cap, None))`.
///
/// The cap is written **only when unset** (0), so a later `adjust_cap`
/// (「火罐上限加1」, e.g. `PPP:[衍生]拍卖撤下来了`) survives the per-turn
/// re-hook. A character swap (`Sumimi:两个都想要`) re-declares from the new
/// face's `CardDef` props and is unaffected.
pub fn fire_pot(player_id: i32) {
    bandori_std::fire::declare_own(player_id);
}

pub mod anon;
use anon::ANON;
pub mod arisa;
use arisa::ARISA;
pub mod kaoru;
use kaoru::KAORU;
pub mod lisa;
pub mod toko;
use lisa::LISA;
use toko::TOKO;
pub mod misaki;
use misaki::MISAKI;
pub mod moca;
pub mod mutsumi;
use mutsumi::MUTSUMI;
pub mod mutsumi_crychic;
pub mod nanami;
use mutsumi_crychic::MUTSUMI_CRYCHIC;
use nanami::NANAMI;
pub mod shifune;
use moca::MOCA;
use shifune::SHIFUNE;
pub mod pareo;
pub mod tomoe;
use pareo::PAREO;
use tomoe::TOMOE;
pub mod rana;
use rana::RANA;
pub mod rimi;
use rimi::RIMI;
pub mod rinko;
pub mod tae;
use rinko::RINKO;
use tae::TAE;
pub mod lock;
use lock::LOCK;
pub mod uika_sumimi;
use uika_sumimi::UIKA_SUMIMI;
pub mod hagumi;
use hagumi::HAGUMI;
pub mod hina;
pub mod mana;
use hina::HINA;
use mana::MANA;
pub mod himari;
use himari::HIMARI;
pub mod ran;
use ran::RAN;
pub mod layer;
pub mod tsugumi;
pub mod tsukushi;
use tsukushi::TSUKUSHI;
pub mod rui;
use layer::LAYER;
use rui::RUI;
use tsugumi::TSUGUMI;
pub mod sayo;
use sayo::SAYO;
pub mod soyo;
pub mod soyo_crychic;
use soyo_crychic::SOYO_CRYCHIC;
pub mod tomori_crychic;
use soyo::SOYO;
use tomori_crychic::TOMORI_CRYCHIC;
pub mod masking;
use masking::MASKING;
pub mod taki;
use taki::TAKI;
pub mod chisato;
pub mod uika;
use chisato::CHISATO;
use uika::UIKA;
pub mod eve;
use eve::EVE;
pub mod kasumi;
pub mod maya;
use kasumi::KASUMI;
use maya::MAYA;
pub mod mashiro;
use mashiro::MASHIRO;
pub mod chuchu;
use chuchu::CHUCHU;
pub mod tomori;
use tomori::TOMORI;
pub mod ako;
use ako::AKO;
pub mod aya;
use aya::AYA;
pub mod yukina;
use yukina::YUKINA;
pub mod kanon;
pub mod kokoro;
use kanon::KANON;
pub mod saaya;
pub mod sakiko_crychic;
use kokoro::KOKORO;
use saaya::SAAYA;
use sakiko_crychic::SAKIKO_CRYCHIC;
pub mod marina;
use marina::MARINA;
pub mod umiri;
use umiri::UMIRI;
pub mod nyamu;
use nyamu::NYAMU;
pub mod sakiko;
use sakiko::SAKIKO;
pub mod taki_crychic;
use taki_crychic::TAKI_CRYCHIC;

// The 规则书 text for these is a Google Sheet (see `skill-rulebook-sheets` in
// the project memory, or fetch `export?format=csv&gid=951422372` on the
// `1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY` document), column `技能`.
// the roster character row (`text`) is a faithful copy of it -- 54/54 rows match
// modulo the sheet's literal backslash-n escapes -- so the JSON is safe to
// code against, and the sheet is where to check a body against.
//
// All 54 character skills are written and bound (`bind_skills`). What is left
// on each is its own `TODO(规则书)` / `TODO(ABI)` note -- 11 of the 54 carry
// one (2026-10-06); the module docs describe the hook/binding shape.
//
pub const CARDS: &[CardDef] = &[
    ANON,
    ARISA,
    AKO,
    LOCK,
    AYA,
    CHISATO,
    UIKA_SUMIMI,
    UIKA,
    EVE,
    KASUMI,
    KOKORO,
    KAORU,
    KANON,
    TOKO,
    YUKINA,
    HAGUMI,
    HINA,
    HIMARI,
    MASHIRO,
    LISA,
    MANA,
    MARINA,
    MAYA,
    MISAKI,
    MOCA,
    MUTSUMI,
    MUTSUMI_CRYCHIC,
    NANAMI,
    PAREO,
    RANA,
    RAN,
    LAYER,
    RIMI,
    RINKO,
    SAAYA,
    SAKIKO_CRYCHIC,
    MASKING,
    SAYO,
    SOYO,
    SOYO_CRYCHIC,
    TAKI,
    TAE,
    CHUCHU,
    TOMOE,
    TSUGUMI,
    TSUKUSHI,
    RUI,
    SHIFUNE,
    TOMORI_CRYCHIC,
    TOMORI,
    TAKI_CRYCHIC,
    SAKIKO,
    NYAMU,
    UMIRI,
];
