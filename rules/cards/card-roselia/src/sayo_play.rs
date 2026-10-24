//! `R:（纱夜）弹奏弹奏弹奏，继续弹奏` -- C# `CardSayoPlay` (MatchHost.cs:10746-10779).
//!
//! 规则书（docs/rulebook/cards.json, id `R:（纱夜）弹奏弹奏弹奏，继续弹奏`）:
//! > （纱夜）弹奏弹奏弹奏，继续弹奏：
//! > [反击] 时机合适时打出，打出时视为使用一次此卡使用者的技能。
//!
//! Counteraction after your non-teleport main distance is determined, before
//! moving (same timing as Sayo's skill; the printed card text is unchanged).
//! The C# realises 「视为使用一次此卡使用者的技能」
//! as H.AnnounceSkill plus SkillSayo's extension (+1 or +2, no fire cost).

use bandori_std::ctx::{self, plan, state, trigger};
use card_sdk::abi::{prop, ChainKind};
use card_sdk::{CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const SAYO_PLAY: CardDef = CardDef::new(
    "R:（纱夜）弹奏弹奏弹奏，继续弹奏",
    &[
        On::Counteract(
            &[ChainKind::MoveBefore],
            "actor == owner && move.kind == Walk && move.main && slot('skill.sayoThorns.used') != turn_key",
            Some(can_counteract),
            counteract).counter_reason(key!("sayo_play_when")),
        On::Play("", Some(counter_only), no_play),
    ],
)
.props(&[(prop::COUNTERACT_GROUP, 1)])
.silent(&[1]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"弹奏弹奏弹奏，继续弹奏"#)
        .raw_name(r#"（纱夜）弹奏弹奏弹奏，继续弹奏"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 时机合适时打出，打出时视为使用一次此卡使用者的技能。"#))
        // sheet cell E10
        .fields(&[("group", r#"Roselia"#), ("owner", r#"冰川纱夜"#)]));

fn counter_only(_player_id: i32) -> Option<Msg> {
    Some(Msg::new(key!("sayo_play_timing")))
}

fn no_play(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

/// Residual guard for [`can_counteract`] -- `skill_blocked` stays here (not
/// yet in the condition vocabulary).
fn can_counteract(player_id: i32) -> bool {
    !ctx::skill_blocked(player_id, "")
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.kind == Walk && move.main &&
    // slot('skill.sayoThorns.used') != turn_key` is the pre;
    // `!skill_blocked` is the residual guard.
    // Announce the user's character skill. Running its press body again would
    // add a second paid skill offer before this card's free extension.
    if let Some(skill) = ctx::persona_skill(player_id) {
        ctx::log(
            player_id,
            &Msg::new(key!("sayo_play_skill")).card("card", &skill),
        );
    }
    let before = trigger::value().max(0);
    let extra = (trigger::move_total() - trigger::value().max(0)).max(0);
    let total = before + extra;
    let n = trigger::move_tag(card_sdk::abi::COUNTERACT_MOVE_EXTENSION);
    if !matches!(n, 1 | 2) {
        return Ok(());
    }
    // 规则书: 「打出时视为使用一次此卡使用者的技能」-- the C# body bumps the move.
    state::set(player_id, "skill.sayoThorns.used", ctx::turn_key());
    plan::set_steps(before + n);
    ctx::log(
        player_id,
        &Msg::new(key!("sayo_play_applied"))
            .player_id("who", player_id)
            .card("card", "R:（纱夜）弹奏弹奏弹奏，继续弹奏")
            .i("n", n as i64)
            .i("total", (total + n) as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn no_play_kani(player_id: i32) -> card_sdk::Asked {
    no_play(player_id)
}
