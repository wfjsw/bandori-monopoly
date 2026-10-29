//! `R:（纱夜）弹奏弹奏弹奏，继续弹奏` -- C# `CardSayoPlay` (MatchHost.cs:10746-10779).
//!
//! 规则书（docs/rulebook/cards.json, id `R:（纱夜）弹奏弹奏弹奏，继续弹奏`）:
//! > （纱夜）弹奏弹奏弹奏，继续弹奏：
//! > [反击] 时机合适时打出，打出时视为使用一次此卡使用者的技能。
//!
//! Counteraction after your non-teleport main distance is determined, before
//! moving (same timing as Sayo's skill; the printed card text is unchanged).
//! 「视为使用一次此卡使用者的技能」 means Sayo's skill (2) extension (+1 or +2):
//! C# `H.AnnounceSkill`. The skill's press body is not invoked, so the card
//! currently runs the extension without the skill's 6-[火罐] cost.

use bandori_std::ctx::{self, plan, state, trigger};
use card_sdk::abi::{prop, ChainKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const SAYO_PLAY: CardDef = CardDef::new(
    "R:（纱夜）弹奏弹奏弹奏，继续弹奏",
    &[
        // 规则书: 「[反击] 时机合适时打出」 -- a counteraction in the skill's own
        // window; `counter_only` explains the timing on a hand play.
        On::Counteract(
            &[ChainKind::MoveBefore],
            "actor == owner && move.kind == Walk && move.main && slot('skill.sayoThorns.used') != turn_key",
            Some(can_counteract),
            counteract).counter_reason(key!("sayo_play_when")),
        On::Play("", Some(counter_only), no_play).silent(),
    ],
)
.props(&[(prop::COUNTERACT_GROUP, 1)])
.meta(&card_sdk::meta::CardMeta::new()
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

/// Residual guard for [`can_counteract`] -- the bound-skill presence (「此卡使用
/// 者的技能」) and `skill_blocked` stay here (not in the condition vocabulary).
fn can_counteract(player_id: i32) -> bool {
    ctx::persona_skill(player_id).is_some() && !ctx::skill_blocked(player_id, "")
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && move.kind == Walk && move.main &&
    // slot('skill.sayoThorns.used') != turn_key` is the pre;
    // `!skill_blocked` is the residual guard.
    // Announce the user's character skill. Running its press body again would
    // add a second paid skill offer before this card's free extension.
    // TODO(规则书)（打出时视为使用一次此卡使用者的技能）: 使用一次技能 = the skill's
    //   press body, whose （2） gates on `charge(owner) >= 6` and spends 6 个[火罐]
    //   -- we only announce it and keep the +1/+2 and the `sayoThorns.used`
    //   latch inline, at no [火罐] cost. Whether 「视为使用」 pays is an open
    //   ruling (`card-ag/src/detour.rs` TODO on the same wording);
    //   `ctx::invoke_skill` would run the body.
    // TODO(ABI)（视为使用一次此卡使用者的技能）: no skill-use raise exists --
    //   v35 `invoke_skill` deliberately raises none (that raise belongs to the
    //   player's own press, `use_skill`), so 「使用角色技能时」 listeners never
    //   hear this use. `cardUsed` also fires for every placed-card declaration,
    //   so it is not a drop-in.
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
