//! `CRYCHIC:想要成为人类` -- C# `CardWantHuman` (MatchHost.cs:2502-2583): place the
//! card, declare X in 1-20, bank crystals when your move roll is under X.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:想要成为人类`）:
//! > 想要成为人类
//! > ：
//! > （1）将此卡置于场上并从1-20间选择并声明X，将其写下。每当你的移动掷骰小于X，为此卡添加一个奇迹水晶。
//! > （2）[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。
//!

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const WANT_HUMAN: CardDef = CardDef::new("CRYCHIC:想要成为人类", &[
    On::Play(None, want_human),
    On::Hook(&[HookKind::TurnStart, HookKind::RollAfter, HookKind::TurnEnd], react_guard, react)]);

/// Where the declared X is written down (C# `CardWantHuman.Mem["x"]`).
/// A per-player slot stands in for the per-field-card `Mem` map; 0 means
/// "never declared" (the C# default X is 10).
pub(crate) const SLOT_X: &str = "want_human_x";

fn want_human(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「将此卡置于场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "CRYCHIC:想要成为人类", &Msg::new(key!("want_human_note")));
    // 规则书（1）: 「从1-20间选择并声明X，将其写下」 -- C# `H.AskNumber(..., 1, 20, ...)`.
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("want_human_title")),
        &Msg::new(key!("want_human_ask")),
        1,
        20,
    )?;
    // 规则书（1）: 「将其写下」 -- `Mem["x"]`; the player slot is the stand-in.
    ctx::set_slot(player_id, SLOT_X, x);
    // Fresh placement: no boost snapshot from a previous instance (C# `Mem` is
    // per-card-instance; the player slot persists across re-places).
    ctx::set_slot(player_id, SLOT_AB_BEFORE, 0);
    ctx::log(player_id, &Msg::new(key!("want_human_declared")).player_id("who", player_id).i("n", x as i64));
    // (1) and (2)'s boost are handled in `react`, which the Fx hook dispatch
    // runs at `rollAfter` / `turnStart` / `turnEnd`.
    Ok(())
}

/// Where the armed boost is written down (C# `CardWantHuman.Mem["boost"]`).
const SLOT_BOOST: &str = "want_human_boost";

/// Snapshot of `abnormal_count` taken when the boost was consumed (C#
/// `CardWantHuman.Mem["abBefore"]`), stored as `count + 1` so that 0 means
/// "boost not consumed this turn".
const SLOT_AB_BEFORE: &str = "want_human_ab_before";

/// `Fx.RollAfter` / `Fx.TurnStart` (C# `CardWantHuman.RollAfter` / `TurnStart`).
/// Runs through the Fx hook dispatch, so this is a field effect, not a [反击].
/// Pure guard for [`react`] -- the activation gate. `false`
/// means the card is not activated at all.
fn react_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn react(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书（1）: 「每当你的移动掷骰小于X，为此卡添加一个奇迹水晶。」
        TriggerKind::RollAfter => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            let roll = trigger::value();
            let x = ctx::slot(player_id, SLOT_X);
            if x > 0 && roll < x {
                ctx::add_crystals(1, 0);
                ctx::log(player_id, &Msg::new(key!("want_human_crystal")).player_id("who", player_id).i("n", roll as i64));
            }
            // 规则书（2）: the boost armed last turn start lands on this roll.
            let boost = ctx::slot(player_id, SLOT_BOOST);
            if boost > 0 {
                ctx::set_slot(player_id, SLOT_BOOST, 0);
                // 规则书（2）: snapshot the abnormal counter so 「若该次移动过程中受到
                // 异常移动效果影响」 can be checked at turn end (C# `Mem["abBefore"]`).
                ctx::set_slot(player_id, SLOT_AB_BEFORE, ctx::abnormal_count(player_id) + 1);
                trigger::set_move_roll(roll + boost);
                ctx::log(player_id, &Msg::new(key!("want_human_boost")).player_id("who", player_id).i("n", boost as i64));
            }
        }
        // 规则书（2）: 「若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部
        // 奇迹水晶并使你下次的移动掷骰结果额外增加20-X。」
        TriggerKind::TurnStart => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            if ctx::crystals() < 2 {
                return Ok(());
            }
            let x = ctx::slot(player_id, SLOT_X);
            ctx::set_crystals(0);
            ctx::set_slot(player_id, SLOT_BOOST, (20 - x).max(0));
            ctx::log(player_id, &Msg::new(key!("want_human_drained")).player_id("who", player_id).i("n", (20 - x).max(0) as i64));
        }
        // 规则书（2）: 「若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹
        // 水晶。」 -- C# `TurnEnd`: `H._abnormalTurn[Player] > Mem["abBefore"]` when
        // the boost was consumed this turn.
        TriggerKind::TurnEnd => {
            if trigger::player_id() != player_id {
                return Ok(());
            }
            let ab_before = ctx::slot(player_id, SLOT_AB_BEFORE);
            if ab_before <= 0 {
                return Ok(());
            }
            ctx::set_slot(player_id, SLOT_AB_BEFORE, 0);
            if ctx::abnormal_count(player_id) > ab_before - 1 {
                ctx::add_crystals(2, 0);
                ctx::log(player_id, &Msg::new(key!("want_human_abnormal")).player_id("who", player_id));
            }
        }
        _ => {}
    }
    Ok(())
}