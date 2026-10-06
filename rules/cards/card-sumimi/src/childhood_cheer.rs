//! `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励` -- C# `CardChildhoodCheer`
//! (MatchHost.cs:11481-11498): move starts from 小豆岛, +1 fire afterwards.
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励`）:
//! > (初华（Sumimi）)儿时玩伴的鼓励：
//! >  可在移动掷骰前打出此卡，使本次移动以“小豆岛”为起点并在移动后获得一个火罐。
//!
//! C# arms `H.ExtraOf<AfterMoveFireFx>(seat)` (a player attachment, not a placed
//! card). The hook surface only dispatches to *placed* cards, so the play body
//! places this card as the `AfterMoveFireFx` stand-in and files it to the
//! discard pile when the effect finishes (or at turn end, C#
//! `AfterMoveFireFx.TurnEndAfter`).

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const CHILDHOOD_CHEER: CardDef = CardDef::new("Sumimi:(初华（Sumimi）)儿时玩伴的鼓励", &[
    On::Play(Some(cant_play), childhood_cheer),
    On::Hook(&[HookKind::SettleBefore, HookKind::SettleAfter, HookKind::Teleported], after_move_guard, after_move),
    On::AtEnd(at_end)]);

const ID: &str = "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励";

/// C# `CardChildhoodCheer.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「可在移动掷骰前打出此卡」 -- only while the turn's main move is
    // still open (C# `H.MoveWhyNot`).
    ctx::cant_move(player_id)
}

fn childhood_cheer(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「使本次移动以“小豆岛”为起点」 -- C# `H._turnCtx.Plan.Start =
    // H.TileNamed("小豆岛"); Plan.StartWhy = CardName` =
    // `plan::set_start(tile, why)`. The player stays put until the move runs
    // (and the plan rolls back cleanly if the move never happens).
    let start = ctx::tile_named("小豆岛");
    if start >= 0 {
        ctx::plan::set_start(start, ID);
        ctx::log(player_id, &Msg::new(key!("childhood_cheer_start")).player_id("who", player_id).tile("tile", start));
    }
    // 规则书: 「并在移动后获得一个火罐」 -- C# `H.ExtraOf<AfterMoveFireFx>(seat)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("childhood_cheer_note")));
    // C# `AfterMoveFireFx.TurnEndAfter` drops the attachment at the owner's
    // turn end if the move never landed it.
    ctx::at_turn_end(player_id);
    Ok(())
}

/// C# `AfterMoveFireFx.Arrive` -- after the owner's main move, +1 fire and drop.
/// Runs through the Fx hook dispatch (`settleAfter`), so this is a field effect,
/// not a [反击].
/// Pure guard for [`after_move`] -- the activation gate. `false`
/// means the card is not activated at all.
fn after_move_guard(player_id: i32) -> bool {
    ctx::is_placed()
}

fn after_move(player_id: i32) -> card_sdk::Asked {
    let k = trigger::kind();
    // C# `AfterMoveFireFx.Arrive` fires when the walk *arrives* -- before the
    // settle, and on a move that does not settle. `settleBefore` is the first
    // half; `settleAfter` and `teleported` catch the two landing shapes.
    if !matches!(
        k,
        TriggerKind::SettleBefore | TriggerKind::SettleAfter | TriggerKind::Teleported
    ) {
        return Ok(());
    }
    // C# `if (m.Seat != Player || !m.Main) return null`.
    if trigger::player_id() != player_id || !trigger::move_is_main() {
        return Ok(());
    }
    if k == TriggerKind::SettleAfter || k == TriggerKind::Teleported {
        // already spent at the arrive half
        if ctx::slot(player_id, "childhood_cheer.fired") == 0 {
            return Ok(());
        }
        ctx::set_slot(player_id, "childhood_cheer.fired", 0);
        ctx::unplace_self();
        ctx::to_discard(player_id, ID);
        return Ok(());
    }
    if ctx::slot(player_id, "childhood_cheer.fired") != 0 {
        return Ok(());
    }
    ctx::set_slot(player_id, "childhood_cheer.fired", 1);
    // 规则书: 「并在移动后获得一个火罐」
    ctx::gain_fire(player_id, 1, &Msg::new(key!("childhood_cheer_fire")));
    ctx::unplace_self();
    ctx::to_discard(player_id, ID);
    Ok(())
}

/// C# `AfterMoveFireFx.TurnEndAfter` -- drop the attachment at the owner's turn
/// end if `Arrive` never ran. Scheduled by `ctx::at_turn_end` in the play body.
fn at_end(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    ctx::unplace_self();
    ctx::to_discard(player_id, ID);
    Ok(())
}
