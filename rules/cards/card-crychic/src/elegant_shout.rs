//! `CRYCHIC:优雅的呐喊` -- C# `CardElegantShout` (MatchHost.cs:2912-2936): [反击]
//! on an out-of-turn draw, draw 1 at the end of your next turn.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:优雅的呐喊`）:
//! > 优雅的呐喊
//! >  ：[反击] 当你在回合外受到抽卡效果时打出，你的下回合结束时抽一张卡。
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const ELEGANT_SHOUT: CardDef = CardDef::new("CRYCHIC:优雅的呐喊", &[
    On::React(&[ChainKind::DrawOut], can_react, react),
    On::AtEnd(at_end),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当你在回合外受到抽卡效果时打出」 -- C#
    // `t.Kind == "drawOut" && t.Seat == seat`. The "回合外" window is what the
    // `drawOut` trigger means (a draw that hit you outside your own turn); the
    // ABI cannot see the turn context, so the kind + player check mirrors the C#.
    trigger::kind() == TriggerKind::DrawOut && trigger::player_id() == player_id
}

fn react(player_id: i32) {
    // 规则书[反击]: 「你的下回合结束时抽一张卡」
    ctx::log(player_id, &Msg::new(key!("elegant_shout_note")).player_id("who", player_id));
    // C# `H.ExtraOf<ElegantFx>(c.Seat).Count++` -- each reaction stacks one
    // draw; scheduling one `AtEnd` per reaction stacks the same way.
    ctx::at_next_turn_end(player_id);
}

/// C# `ElegantFx.TurnEnd` -- one draw at the end of the player's next turn.
fn at_end(player_id: i32) {
    // 规则书[反击]: 「你的下回合结束时抽一张卡」
    ctx::draw(player_id, 1);
}
