//! `CRYCHIC:优雅的呐喊` -- C# `CardElegantShout` (MatchHost.cs:2912-2936): [反击]
//! on an out-of-turn draw, draw 1 at the end of your next turn.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:优雅的呐喊`）:
//! > 优雅的呐喊
//! >  ：[反击] 当你在回合外受到抽卡效果时打出，你的下回合结束时抽一张卡。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const ELEGANT_SHOUT: CardDef = CardDef {
    id: "CRYCHIC:优雅的呐喊",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当你在回合外受到抽卡效果时打出」 -- C#
    // `t.Kind == "drawOut" && t.Seat == seat`. The "回合外" window is what the
    // `drawOut` trigger means (a draw that hit you outside your own turn); the
    // ABI cannot see the turn context, so the kind + seat check mirrors the C#.
    trigger::kind() == TriggerKind::DrawOut && trigger::seat() == seat
}

fn react(seat: i32) {
    // 规则书[反击]: 「你的下回合结束时抽一张卡」
    ctx::log(seat, &Msg::new(key!("elegant_shout_note")).seat("who", seat));
    // TODO(ABI): 「你的下回合结束时抽一张卡」 -- needs the Fx.TurnEnd persistent
    //   hook (C# `ElegantFx.TurnEnd` draws `Count` at the end of this seat's next
    //   turn; each reaction stacks `ElegantFx.Count++`). `ctx::draw(seat, n)` is
    //   ready once the hook schedules it.
}