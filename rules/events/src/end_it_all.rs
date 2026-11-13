//! `event:让我来结束一切` -- 事件卡「让我来结束一切」（中立事件 A27，衍生，常驻）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `让我来结束一切`）:
//! > 触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上，此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆顶部

use alloc::string::String;
use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{CardDef, Msg, On};

use crate::util::keep;

pub const END_IT_ALL: CardDef = CardDef::new("event:让我来结束一切", &[On::Play("", None, play)]).rule()
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"让我来结束一切"#)
            .text(CardText::para(r#"触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上，此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆顶部"#))
            // sheet cell A27
            .derived(true));

/// 规则书: 「触发事件的玩家从所有非衍生事件中选择3个移除」 -- the drawer picks
/// three distinct non-derived event ids and each is banished for good.
/// 「并将此卡永久放置于场上」 -- the event stays up forever (no expiry clause),
/// so it is `keep()` and nothing ever calls `expire`.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::log(
        player_id,
        &Msg::new("log.event.end_it_on").player_id("who", player_id),
    );

    // 规则书: 「从所有非衍生事件中选择3个移除」 -- the pool is the manifest's
    // event roster (`ctx::event_card_ids`) minus every card whose
    // `CardMeta.derived` is set (`ctx::card_derived`, EVENT-04). No hand table:
    // the derived flags live beside each event's own rule body.
    let mut pool: Vec<String> = ctx::event_card_ids()
        .into_iter()
        .filter(|id| !ctx::card_derived(id))
        .collect();
    for _ in 0..3 {
        if pool.is_empty() {
            break;
        }
        let options: Vec<Msg> = pool
            .iter()
            .map(|id| Msg::new("ask.cardOption").card("event", id))
            .collect();
        // 「选择3个移除」 -- without replacement; a picked id leaves the pool.
        let pick = ctx::ask_choice(
            player_id,
            &Msg::new("log.event.end_it_title"),
            &Msg::new("log.event.end_it_ask"),
            &options,
        )?;
        let id = pool.remove(pick.min(pool.len() - 1));
        // 「移除」 -- out of the game entirely (`docs/EVENTS.md`). The roster
        // carries manifest ids (`event:` prefix); the event deck spells the
        // same card bare.
        ctx::event_banish(id.strip_prefix("event:").unwrap_or(&id));
        ctx::log(
            player_id,
            &Msg::new("log.event.end_it_banish")
                .player_id("who", player_id)
                .card("event", &id),
        );
    }

    // 「此卡在场上则[衍生]让我来结束一切不会被[衍生]迷子的追逐放置在事件牌堆
    // 顶部」 -- the guard lives on `lost_chase.rs`'s win branch: it checks
    // `ctx::event_is_active("让我来结束一切")` before pushing this card. While
    // this event sits on the field (it never expires on its own), `迷子的追逐`
    // cannot stack a second copy on the event deck. Coupled files: this one
    // keeps, `lost_chase.rs` guards.
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
