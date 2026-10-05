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
//! grab / jump effects are the GrabFx hooks (TODO).

use card_sdk::{ctx, key, CardDef, Msg};

pub const WANT_TO_GRAB: CardDef = CardDef {
    id: "CRYCHIC:想要抓住...",
    play: Some(want_to_grab),
    can_react: None,
    react: None,
    why_not: None,
};

fn want_to_grab(seat: i32) {
    // 规则书（1）: 「获得一层[停留]。」
    ctx::give_stay(seat, 1);
    ctx::log(seat, &Msg::new(key!("want_to_grab_note")).seat("who", seat));
    // TODO(规则书)（2）: 「当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]。」
    //   -- needs the Fx.PassSeat + Fx.SettleBefore persistent hooks and the
    //   H.Walk movement routine (walk 1 tile with resolve).
    // TODO(规则书)（3）: 「[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子…并[触发结算]，视为你的主要移动。」
    //   -- needs the Fx.TurnStart hook, board grid geometry (H.Grid / H.FromGrid),
    //   H.AskTileOf over the four ray-nearest tiles, and the H.MainMoveAs
    //   movement routine (teleport + settle as the main move).
}
