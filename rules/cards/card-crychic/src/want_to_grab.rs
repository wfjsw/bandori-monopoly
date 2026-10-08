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
//! `card_move`, but the `GrabFx` attachment and the grid-nearest teleport
//! (「视为你的主要移动」) are still held (see the TODOs below).

use card_sdk::abi::HookKind;
use card_sdk::ctx::{plan, trigger};
use card_sdk::{ctx, key, CardDef, Msg, On};

/// The passer this card is waiting for, or -1. A slot rather than a `GrabFx`
/// attachment: it is a value the card stores under its own key.
const PASSER: &str = "want_to_grab_passer";

pub const WANT_TO_GRAB: CardDef = CardDef::new(
    "CRYCHIC:想要抓住...",
    &[
        On::Play(None, want_to_grab, ""),
        // TODO(规则书) R5 (`SETTLE-STAGES.md` §9): 「当第一位其他玩家经过你」 is
        // 经过 (行动阶段 12, a mid-route pass of my tile) or 重叠 (`passPlayer`,
        // the end-tile overlap)? The C# `GrabFx` used `PassPlayer`; the text
        // says 「经过」. Pending ruling -- left as `PassPlayer` (C#-carried).
        On::Hook(&[HookKind::PassPlayer], None, on_pass, ""),
        On::Hook(&[HookKind::SettleBefore], None, grab, ""),
    ],
);

fn want_to_grab(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「获得一层[停留]。」
    ctx::give_stay(player_id, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("want_to_grab_note")).player_id("who", player_id),
    );
    // 规则书（2）: 「当第一位其他玩家经过你」 -- arm the grab; the first passer
    // is the one it fires on.
    ctx::state::set(player_id, PASSER, -1);
    Ok(())
}

/// 「当第一位其他玩家经过你」 -- remember the *first* passer only.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let passer = ctx::trigger::player_id();
    if passer == player_id || passer < 0 {
        return Ok(());
    }
    if ctx::state::get(player_id, PASSER) >= 0 {
        return Ok(());
    }
    ctx::state::set(player_id, PASSER, passer);
    Ok(())
}

/// 「在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]」 -- the grab runs
/// before the remembered passer settles.
fn grab(player_id: i32) -> card_sdk::Asked {
    let passer = ctx::state::get(player_id, PASSER);
    if passer < 0 || ctx::trigger::player_id() != passer {
        return Ok(());
    }
    ctx::state::set(player_id, PASSER, -1);
    // 「你立刻向前移动一格并[触发结算]」 -- a one-step walk that settles.
    plan::set_steps(1);
    plan::set_resolve(true);
    ctx::card_move(player_id);
    Ok(())
}

// TODO(规则书)（3）: 「[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子…并[触发结算]，视为你的主要移动。」
//   -- `TriggerKind::TurnStart` and `ctx::ask_tile` exist now; still missing
//   the board grid geometry (C# `Grid` / `FromGrid` ray-nearest search over
//   the four axis directions) and `plan::set_teleport_to` (the
//   teleport-with-settle shape for `H.MainMoveAs`).
