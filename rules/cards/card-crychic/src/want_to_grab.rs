//! `CRYCHIC:想要抓住...` -- C# `CardWantToGrab`: gain one [Stay]; the follow-up
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:想要抓住...`）:
//! > 想要抓住...：
//! >  
//! > （1）获得一层[停留]。
//!
//! > （2）你的下回合开始前，当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]。
//!
//! > （3）你的下回合开始前，若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。
//!
//! grab / jump effects are the GrabFx hooks: `PassPlayer` / `SettleBefore` /
//! `TurnStart` are hook kinds now; the walk body is expressible with
//! `card_move`, and the card stays in play (`Dest::Field`) so those hooks
//! attach; only the grid-nearest teleport (「视为你的主要移动」) is still held
//! (see the TODO on `on_turn_start`).

use bandori_std::ctx;
use bandori_std::ctx::{plan, trigger};
use card_sdk::abi::ActionKind;
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

/// The passer this card is waiting for. A slot rather than a `GrabFx`
/// attachment: it is a value the card stores under its own key.
const PASSER: &str = "want_to_grab_passer";

/// `PASSER` sentinel: nobody has passed in this card's window.
const NEVER: i32 = -1;
/// `PASSER` sentinel: the first passer already fired the grab, so (2) is spent
/// and clause (3)'s 「若没有玩家经过你」 is off.
const FIRED: i32 = -2;

pub const WANT_TO_GRAB: CardDef = CardDef::new(
    "CRYCHIC:想要抓住...",
    &[
        On::Play("", None, want_to_grab),
        // TODO(规则书) R5 (`SETTLE-STAGES.md` §9): 「当第一位其他玩家经过你」 is
        // 经过 (行动阶段 12, a mid-route pass of my tile) or 重叠 (`passPlayer`,
        // the end-tile overlap)? The C# `GrabFx` used `PassPlayer`; the text
        // says 「经过」. Pending ruling -- left as `PassPlayer` (C#-carried).
        // 规则书（2）: 「当第一位其他玩家经过你」
        On::Action(
            Action::PassPlayer("actor != owner && target == owner", None, on_pass),
        )
        .silent(),
        // 规则书（2）: 「在那名玩家[触发结算]前」
        On::Action(Action::SettleBefore("", None, grab)),
        // 规则书（2）: 「你的下回合开始前」 -- close the window at the owner's turn start.
        On::Action(Action::TurnStart("actor == owner && card.placed", None, on_turn_start)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"想要抓住..."#)
        .raw_name(r#"想要抓住..."#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"获得一层[停留]。"#),
            it(r#"你的下回合开始前，当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]。"#),
            it(r#"你的下回合开始前，若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。"#),
        ]))
        // sheet cell M5
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]));

fn want_to_grab(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「获得一层[停留]。」
    rules_status::stay::add(player_id, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("want_to_grab_note")).player_id("who", player_id),
    );
    // 规则书（2）: 「当第一位其他玩家经过你」 -- arm the grab; only the *first*
    // passer may fill the latch (`on_pass` skips anything but `NEVER`).
    ctx::state::set(player_id, PASSER, NEVER);
    // 规则书（2）: 「你的下回合开始前」 -- the card waits in play for its window:
    // the `PassPlayer` / `SettleBefore` hooks only reach a placed field card.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "CRYCHIC:想要抓住...",
        &Msg::new(key!("want_to_grab_note")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书（2）: 「当第一位其他玩家经过你」 -- remember the *first* passer only;
/// a latched passer (or `FIRED`) keeps a later one out.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    // `actor != owner && target == owner` is the pre (another player, my tile).
    // The latch keeps its own `NEVER` = unset sentinel, because a missing slot
    // reads 0 and 0 is also seat 0's id.
    if ctx::state::get(player_id, PASSER) != NEVER {
        return Ok(());
    }
    let passer = ctx::trigger::player_id();
    ctx::state::set(player_id, PASSER, passer);
    Ok(())
}

/// 规则书（2）: 「在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]」 --
/// the grab runs before the remembered passer settles.
fn grab(player_id: i32) -> card_sdk::Asked {
    let passer = ctx::state::get(player_id, PASSER);
    if passer < 0 || ctx::trigger::player_id() != passer {
        return Ok(());
    }
    // (2) is spent: no later passer may fire it again, and `FIRED` tells the
    // turn-start hook that (3)'s 「若没有玩家经过你」 is off.
    ctx::state::set(player_id, PASSER, FIRED);
    // 规则书（2）: 「你立刻向前移动一格并[触发结算]」 -- a one-step walk that settles.
    plan::set_steps(1);
    plan::set_resolve(true);
    ctx::card_move(player_id);
    Ok(())
}

/// 规则书（2）: 「你的下回合开始前」 -- at the owner's turn start the window
/// closes and the card leaves play (with it the two hooks above).
fn on_turn_start(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（若没有玩家经过你，你在回合开始时立即选择并[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子（可穿过地图，不可为自身所在格）并[触发结算]，视为你的主要移动。）:
    //   -- when the latch is still `NEVER`, (3) runs first, right here:
    //   `ctx::ask_tile` and `plan::set_teleport_to` exist, but the board grid
    //   ray-nearest search over the four axis directions is still missing
    //   (C# `Grid` / `FromGrid`; only private copies in `happy_lucky.rs`).
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::state::set(player_id, PASSER, NEVER);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn want_to_grab_kani(player_id: i32) -> card_sdk::Asked {
    want_to_grab(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn grab_kani(player_id: i32) -> card_sdk::Asked {
    grab(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_start(player_id)
}
