//! Character skills as card rules -- one file per skill, one crate for the lot.
//!
//! Deliberately **not** in `rules/cards/`: those are the cards you draw and
//! play, and these are the abilities a character comes with. Same rule language
//! ([`CardDef`]), same guest runtime, separate crate -- so a card author never
//! has to scroll past them and a skill author never edits a card.
//!
//! The rule text lives in `data/characters.json` (`skill` is the name, `text` /
//! `simple` the wording); what is here is the behavior. A skill that mandates a
//! fire-pot cap is the whole reason the keyed state carries bounds -- it writes
//! `state::set_bounds(player_id, state_key::FIRE, 0, 1)` and every card reads it
//! back with `state::max(player_id, state_key::FIRE)`. The engine holds the
//! number and enforces nothing; see `game_core::state::StateVar`.
//!
//! # Hooks: use the two that exist
//!
//! A skill is not a third category. Everything it does is either **started by
//! the user** or **called by a field event**, and both already have a hook:
//!
//! - **Started by the user** -- a skill button (the viewer's `MatchView.skills`
//!   list). The player presses it, the effect runs. That is
//!   [`On::Play`], gate and effect: `On::Play("", Some(why_not), run)` where the
//!   gate is the "can I press this right now?" query (C# `Card.WhyNot`) and
//!   `run` is what the press does.
//! - **Called by a field event** -- the skill is a field card on the player
//!   (C# `Fx`, from `MatchPlayer.field`), which is why [`On::Hook`] reaches it.
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
//! engine places them at match start (`bind_skills`), so `On::Hook` reaches
//! them like any field card and `On::Play` answers the skill button. A card
//! that reaches a skill by name uses `ctx::character_skill` /
//! `ctx::band_skill` + `ctx::invoke_skill` (ABI v35).
//!
//! Adding a skill is one file here plus an entry in [`CARDS`].

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

use card_sdk::abi::{state_key, TriggerKind};
use card_sdk::ctx::{self, state};
use card_sdk::CardDef;

/// 「初始N，上限M」 -- the fire-pot bounds, plus the initial count at the
/// after-match-start point (where initial tokens/resources are created).
///
/// Hook this from both `TurnStartBefore` (so the cap holds from the first turn
/// even when the skill is placed mid-game) and `DeckAtGameStart` (where
/// 「初始N」 is created), e.g.
/// `On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], "", None, declare_cap)`.
///
/// The cap is written **only when unset** (0), so a later `add_fire_max`
/// (「火罐上限加1」, e.g. `PPP:[衍生]拍卖撤下来了`) survives the per-turn
/// re-hook. A character swap (`Sumimi:两个都想要`) writes the new bounds
/// explicitly and is unaffected.
pub fn fire_pot(player_id: i32, initial: i32, cap: i32) {
    if state::max(player_id, state_key::FIRE) == 0 {
        state::set_bounds(player_id, state_key::FIRE, 0, cap);
    }
    if ctx::trigger::kind() == TriggerKind::DeckAtGameStart {
        state::set(player_id, state_key::FIRE, initial);
    }
}

pub mod anon_restart;
use anon_restart::ANON_RESTART;
pub mod arisa_bonsai;
use arisa_bonsai::ARISA_BONSAI;
pub mod kaoru_prince;
use kaoru_prince::KAORU_PRINCE;
pub mod kiritani_zenith;
pub mod lisa_goddess;
use kiritani_zenith::KIRITANI_ZENITH;
use lisa_goddess::LISA_GODDESS;
pub mod misaki_other;
use misaki_other::MISAKI_OTHER;
pub mod moca_self;
pub mod mutsumi_actor;
use mutsumi_actor::MUTSUMI_ACTOR;
pub mod mutsumi_crychic;
pub mod nanami_ordinary;
use mutsumi_crychic::MUTSUMI_CRYCHIC;
use nanami_ordinary::NANAMI_ORDINARY;
pub mod tukushi_try;
use moca_self::MOCA_SELF;
use tukushi_try::TUKUSHI_TRY;
pub mod numazu_maid;
pub mod tomoe_ramen;
use numazu_maid::NUMAZU_MAID;
use tomoe_ramen::TOMOE_RAMEN;
pub mod rana_parking;
use rana_parking::RANA_PARKING;
pub mod rimi_resolve;
use rimi_resolve::RIMI_RESOLVE;
pub mod rinko_1cm;
pub mod tae_police;
use rinko_1cm::RINKO_1CM;
use tae_police::TAE_POLICE;
pub mod asahi_aim;
use asahi_aim::ASAHI_AIM;
pub mod uika_idol;
use uika_idol::UIKA_IDOL;
pub mod hagumi_homerun;
use hagumi_homerun::HAGUMI_HOMERUN;
pub mod hina_lottery;
pub mod mana_donut;
use hina_lottery::HINA_LOTTERY;
use mana_donut::MANA_DONUT;
pub mod himari_step;
use himari_step::HIMARI_STEP;
pub mod ran_red;
use ran_red::RAN_RED;
pub mod raise_effort;
pub mod tsugumi_plain;
pub mod tsugushi_monitor;
use tsugushi_monitor::TSUGUSHI_MONITOR;
pub mod yuri_crit;
use raise_effort::RAISE_EFFORT;
use tsugumi_plain::TSUGUMI_PLAIN;
use yuri_crit::YURI_CRIT;
pub mod sayo_thorns;
use sayo_thorns::SAYO_THORNS;
pub mod soyo_clear;
pub mod soyo_crychic;
use soyo_crychic::SOYO_CRYCHIC;
pub mod tomori_crychic;
use soyo_clear::SOYO_CLEAR;
use tomori_crychic::TOMORI_CRYCHIC;
pub mod sato_red;
use sato_red::SATO_RED;
pub mod taki_meeting;
use taki_meeting::TAKI_MEETING;
pub mod chisato_frank;
pub mod uika_imprisoned;
use chisato_frank::CHISATO_FRANK;
use uika_imprisoned::UIKA_IMPRISONED;
pub mod eve_unify;
use eve_unify::EVE_UNIFY;
pub mod extraordinary_star;
pub mod maya_dawn;
use extraordinary_star::EXTRAORDINARY_STAR;
use maya_dawn::MAYA_DAWN;
pub mod kurata_speed;
use kurata_speed::KURATA_SPEED;
pub mod tamade_producer;
use tamade_producer::TAMADE_PRODUCER;
pub mod tomori_poem;
use tomori_poem::TOMORI_POEM;
pub mod akao_cool;
use akao_cool::AKAO_COOL;
pub mod aya_with;
use aya_with::AYA_WITH;
pub mod kokoro_practice;
use kokoro_practice::KOKORO_PRACTICE;
pub mod kanon_lost;
pub mod kasumi_group;
use kanon_lost::KANON_LOST;
pub mod saaya_sky;
pub mod saki_crychic;
use kasumi_group::KASUMI_GROUP;
use saaya_sky::SAAYA_SKY;
use saki_crychic::SAKI_CRYCHIC;
pub mod marina_gifts;
use marina_gifts::MARINA_GIFTS;
pub mod kaede_support;
use kaede_support::KAEDE_SUPPORT;
pub mod mumei_streamer;
use mumei_streamer::MUMEI_STREAMER;
pub mod sakiko_life;
use sakiko_life::SAKIKO_LIFE;
pub mod taki_crychic;
use taki_crychic::TAKI_CRYCHIC;

// The 规则书 text for these is a Google Sheet (see `skill-rulebook-sheets` in
// the project memory, or fetch `export?format=csv&gid=951422372` on the
// `1xZ3avBsNBXbl3bQ74lmPs0YQFgZkPD0Sdzmx7ZGGEDY` document), column `技能`.
// `data/characters.json` (`text`) is a faithful copy of it -- 54/54 rows match
// modulo the sheet's literal backslash-n escapes -- so the JSON is safe to
// code against, and the sheet is where to check a body against.
//
// All 54 character skills are written and bound (`bind_skills`). What is left
// on each is its own `TODO(规则书)` / `TODO(ABI)` note -- 11 of the 54 carry
// one (2026-10-06); the module docs describe the hook/binding shape.
//
pub const CARDS: &[CardDef] = &[
    ANON_RESTART,
    ARISA_BONSAI,
    AKAO_COOL,
    ASAHI_AIM,
    AYA_WITH,
    CHISATO_FRANK,
    UIKA_IDOL,
    UIKA_IMPRISONED,
    EVE_UNIFY,
    EXTRAORDINARY_STAR,
    KASUMI_GROUP,
    KAORU_PRINCE,
    KANON_LOST,
    KIRITANI_ZENITH,
    KOKORO_PRACTICE,
    HAGUMI_HOMERUN,
    HINA_LOTTERY,
    HIMARI_STEP,
    KURATA_SPEED,
    LISA_GODDESS,
    MANA_DONUT,
    MARINA_GIFTS,
    MAYA_DAWN,
    MISAKI_OTHER,
    MOCA_SELF,
    MUTSUMI_ACTOR,
    MUTSUMI_CRYCHIC,
    NANAMI_ORDINARY,
    NUMAZU_MAID,
    RANA_PARKING,
    RAN_RED,
    RAISE_EFFORT,
    RIMI_RESOLVE,
    RINKO_1CM,
    SAAYA_SKY,
    SAKI_CRYCHIC,
    SATO_RED,
    SAYO_THORNS,
    SOYO_CLEAR,
    SOYO_CRYCHIC,
    TAKI_MEETING,
    TAE_POLICE,
    TAMADE_PRODUCER,
    TOMOE_RAMEN,
    TSUGUMI_PLAIN,
    TSUGUSHI_MONITOR,
    YURI_CRIT,
    TUKUSHI_TRY,
    TOMORI_CRYCHIC,
    TOMORI_POEM,
    TAKI_CRYCHIC,
    SAKIKO_LIFE,
    MUMEI_STREAMER,
    KAEDE_SUPPORT,
];
