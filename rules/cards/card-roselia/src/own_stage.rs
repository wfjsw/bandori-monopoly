//!
//! 规则书（docs/rulebook/cards.json, id `R:选择自己的舞台`）:
//! > 选择自己的舞台：
//! > [反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算
//!
//! on an abnormal move (not [除外]): choose whether this move (or the end of a
//! movement-locked turn) settles.

use bandori_std::ctx::{self, AbKind};
use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const OWN_STAGE: CardDef = CardDef::new(
    "R:选择自己的舞台",
    // The window follows the refusals now (G06 §6.7): both the field guard and
    // `immuneAll` raise `abnormalRefused` and answer `false` ahead of it, so a
    // protected player no longer spends the card on a move that never lands.
    &[On::Counteract(
        &[ChainKind::Effect],
        "target == owner",
        Some(can_counteract),
        counteract).counter_reason(key!("own_stage_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"选择自己的舞台"#)
        .raw_name(r#"选择自己的舞台"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 受到[除外]以外的异常移动效果影响时可打出此卡，选择自己的本次移动（或无法移动的回合结束时）是否触发结算"#))
        // sheet cell E9
        .fields(&[("group", r#"Roselia"#), ("owner", "")]));

/// The [异常移动效果] this chain link declared, as its `AbKind`.
///
fn abnormal_kind() -> Option<AbKind> {
    (0..ctx::effect::count()).find_map(|i| {
        (ctx::effect::kind(i) == TriggerKind::Abnormal)
            .then(|| AbKind::from_i32(ctx::effect::value(i)))
            .flatten()
    })
}

/// 规则书[反击]: 「[反击] 受到[除外]以外的异常移动效果影响时可打出此卡」
fn can_counteract(_player_id: i32) -> bool {
    // G4: the actor rel is the condition (`target == owner`); 「[除外]以外」 is
    // a derived chain lookup (GUARDS.md §6) and stays here.
    abnormal_kind().is_some_and(|k| k != bandori_std::kind::abnormal::Exile)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书[反击]: 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」
    let yes = ctx::ask_yes(
        player_id,
        &Msg::new(key!("own_stage_ask_title")),
        &Msg::new(key!("own_stage_ask_text")),
    )?;
    let reason = if yes {
        Msg::new(key!("own_stage_settle")).player_id("who", player_id)
    } else {
        Msg::new(key!("own_stage_no_settle")).player_id("who", player_id)
    };
    ctx::log(player_id, &reason);
    // 规则书[反击]: 「选择自己的本次移动（或无法移动的回合结束时）是否触发结算」
    // -- the old port branched on the abnormal kind (`stay`/`stun` → `stageEnd`,
    // else the per-player settle override). Both answers are engine state now
    // (G03 §5.8): the move half is consumed into the move, the turn-end half by
    // the settle runner before `turnEndBefore`.
    use card_sdk::abi::SettleAnswer::{Settle, Skip};
    // 「或无法移动的回合结束时」 vs 「本次移动」 is the landing's blocked-ness,
    // not its kind: a [停留] walked through under [不可阻挡] does not hold the
    // move, so it takes the move branch.
    if ctx::trigger::abnormal_blocked() {
        ctx::set_turn_end_settle_answer(player_id, if yes { Settle } else { Skip });
    } else {
        ctx::set_move_settle_answer(player_id, if yes { Settle } else { Skip });
    }
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
