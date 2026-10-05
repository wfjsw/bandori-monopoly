//! `Mor:夏日合宿` -- C# `CardSummerCamp` (MatchHost.cs:4584-4643): stay untargetable
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:夏日合宿`）:
//! > 夏日合宿：
//! >  打出此卡，直到下个自己的回合开始前，你只会被自己发动的效果指定。当此卡效果结束，你没有因为此卡效果无效化任何影响则抽一张牌
//!
//! by others until your next turn, then draw if nothing was blocked.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "Mor:夏日合宿";

pub const SUMMER_CAMP: CardDef = CardDef::new("Mor:夏日合宿", &[
    On::Play(summer_camp),
    On::Hook(&[TriggerKind::TurnStart], turn_start),
]);

fn summer_camp(player_id: i32) {
    // C# `AiPlay => false` -- bots never play this card; `CardDef` has no AiPlay
    // hook yet, so a bot prompt will still offer it.
    // 规则书: 「打出此卡，直到下个自己的回合开始前」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("summer_camp_note")));
    ctx::log(player_id, &Msg::new(key!("summer_camp_placed")).player_id("who", player_id));
    // TODO(规则书): 「你只会被自己发动的效果指定」 -- needs the Fx.Untargetable hook
    // (C# `CardSummerCamp.Untargetable` refuses any `by != Player` and counts
    // `Mem["blocked"]`).
    // The Fx.TurnStart hook below ends the effect on the owner's next turn.
}

/// `Fx.TurnStart` (C# `CardSummerCamp.TurnStart` -> `End`): the effect ends at
/// the owner's next turn start; draw 1 when nothing was negated.
fn turn_start(player_id: i32) {
    if trigger::kind() != TriggerKind::TurnStart
        || trigger::player_id() != player_id
        || !ctx::is_placed(player_id)
    {
        return;
    }
    // 规则书: 「当此卡效果结束」 -- C# `H.Unplace(this, "discard", "效果结束了")`.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    ctx::log(player_id, &Msg::new(key!("summer_camp_end")).player_id("who", player_id));
    // 规则书: 「你没有因为此卡效果无效化任何影响则抽一张牌」 -- C# `End`:
    // `H.DrawR(Seat, 1, ...)` when `Blocked == 0`.
    // TODO(规则书): the `Blocked == 0` gate needs the Fx.Untargetable hook (above)
    //   to count negations; with nothing able to negate, the draw always fires.
    ctx::draw(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("summer_camp_draw")).player_id("who", player_id));
}
