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

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const CHILDHOOD_CHEER: CardDef = CardDef::new("Sumimi:(初华（Sumimi）)儿时玩伴的鼓励", &[
    On::Play(childhood_cheer),
    On::CantPlay(cant_play),
    On::Hook(&[TriggerKind::SettleAfter], after_move),
    On::AtEnd(at_end),
]);

const ID: &str = "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励";

/// C# `CardChildhoodCheer.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「可在移动掷骰前打出此卡」 -- only while the turn's main move is
    // still open (C# `H.MoveWhyNot`).
    ctx::cant_move(player_id)
}

fn childhood_cheer(player_id: i32) {
    // 规则书: 「使本次移动以“小豆岛”为起点」 -- C# `H._turnCtx.Plan.Start =
    // H.TileNamed("小豆岛")`. The move plan hook is missing, so approximate by
    // putting the player on 小豆岛 without settling; the dice move then starts there.
    let start = ctx::tile_named("小豆岛");
    if start >= 0 {
        ctx::teleport_to(player_id, start);
        ctx::log(player_id, &Msg::new(key!("childhood_cheer_start")).player_id("who", player_id).tile("tile", start));
    }
    // TODO(规则书): the C# `Plan.Start` form of 「以“小豆岛”为起点」 keeps the player
    // put until the move runs (and rolls back cleanly if the move never happens);
    // the `teleport_to` stand-in above cannot. There is no plan-start setter
    // (`On::RollPlan` + `set_next_steps` / `set_fixed_roll` shape steps/roll only).
    // 规则书: 「并在移动后获得一个火罐」 -- C# `H.ExtraOf<AfterMoveFireFx>(seat)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("childhood_cheer_note")));
    // C# `AfterMoveFireFx.TurnEndAfter` drops the attachment at the owner's
    // turn end if the move never landed it.
    ctx::at_turn_end(player_id);
}

/// C# `AfterMoveFireFx.Arrive` -- after the owner's main move, +1 fire and drop.
/// Runs through the Fx hook dispatch (`settleAfter`), so this is a field effect,
/// not a [反击].
fn after_move(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    if trigger::kind() != TriggerKind::SettleAfter {
        return;
    }
    // C# `if (m.Seat != Player || !m.Main) return null`.
    if trigger::player_id() != player_id || !trigger::move_is_main() {
        return;
    }
    // 规则书: 「并在移动后获得一个火罐」
    ctx::gain_fire(player_id, 1, &Msg::new(key!("childhood_cheer_fire")));
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    // TODO(规则书): 「并在移动后获得一个火罐」 -- C# `AfterMoveFireFx.Arrive` fires
    // when the walk *arrives* (before the settle), and also on a main move that
    // does not settle. The only after-move kind here is `settleAfter`, so the
    // fire lands after `land()` and only when the move settles; the Arrive half
    // is unmapped.
}

/// C# `AfterMoveFireFx.TurnEndAfter` -- drop the attachment at the owner's turn
/// end if `Arrive` never ran. Scheduled by `ctx::at_turn_end` in the play body.
fn at_end(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
}
