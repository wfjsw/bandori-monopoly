//!
//! 规则书（docs/rulebook/cards.json, id `PPP:向着未来的路标`）:
//! > 向着未来的路标：
//! > 本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金
//!
//! lap without settling, then pay 1,000 at end of turn.

use card_sdk::abi::ActionKind;
use card_sdk::CardText;
use card_sdk::{ctx, key, Action, CardDef, Msg, On};

pub const SIGNPOST: CardDef = CardDef::new(
    "PPP:向着未来的路标",
    // The body-top `player_out(owner)` early-out is the condition now: a
    // bankrupt seat pays nobody, so the callback must not run (or flash).
    &[
        On::Play("", Some(cant_play), play),
        // TODO(规则书)（回合结束时[失去]1000资金）: the book is unconditional -- we
        // skip the loss when the owner is out (benign: an out seat holds no
        // money to take).
        On::Schedule("at_end", "owner.out == 0", None, at_end),
    ],
)
.props(&[(card_sdk::abi::prop::EST_COST, 1000)])
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"向着未来的路标"#)
        .raw_name(r#"向着未来的路标"#)
        .tags(&[])
        .text(CardText::para(
            r#"本回合的主要移动设为移动60格子并不触发结算，回合结束时[失去]1000资金"#,
        ))
        // sheet cell B14
        .fields(&[("group", r#"Poppin' Party"#), ("owner", "")]),
);

/// 规则书: 「本回合的主要移动设为移动60格子」 -- the card replaces the main move, so only while it
/// is still available.
fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    ctx::plan::set_steps(60);
    ctx::plan::set_settles(false);
    ctx::log(
        player_id,
        &Msg::new(key!("signpost_move"))
            .player_id("who", player_id)
            .i("n", 60),
    );
    ctx::run_planned_move(player_id);
    ctx::before_turn_end(player_id, "at_end");
    ctx::log(
        player_id,
        &Msg::new(key!("signpost_pending"))
            .player_id("who", player_id)
            .n("n", 1000),
    );
    Ok(())
}

/// The "still in the game" gate is the entry's condition (`owner.out == 0`).
fn at_end(player_id: i32) -> card_sdk::Asked {
    ctx::pay(player_id, 1000, &Msg::new(key!("signpost_lose")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_end_kani(player_id: i32) -> card_sdk::Asked {
    at_end(player_id)
}
