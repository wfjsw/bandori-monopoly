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
//! `TurnStart` are hook kinds now, but the walk-with-settle and the
//! 「视为你的主要移动」 teleport are still held (see the TODOs below).

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const WANT_TO_GRAB: CardDef = CardDef::new("CRYCHIC:想要抓住...", &[
    On::Play(want_to_grab),
]);

fn want_to_grab(player_id: i32) {
    // 规则书（1）: 「获得一层[停留]。」
    ctx::give_stay(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("want_to_grab_note")).player_id("who", player_id));
    // TODO(规则书)（2）: 「当第一位其他玩家经过你，在那名玩家[触发结算]前，你立刻向前移动一格并[触发结算]。」
    //   -- `TriggerKind::PassPlayer` / `SettleBefore` exist now (C# `GrabFx.PassSeat`
    //   remembers the passer, `GrabFx.SettleBefore` runs the grab), but the grab
    //   body is `H.Walk(Seat, 1, resolve: true)` -- an immediate walk-with-settle
    //   -- and no walk/settle routine is in the guest ABI (`ctx::teleport_to`
    //   never settles). The `GrabFx` attachment (C# `H.ExtraOf<GrabFx>`) that
    //   remembers the passer is also still held.
    // TODO(规则书)（3）: 「[传送]至一个与自身所在格正上，正下，正左，正右直线距离最近的格子…并[触发结算]，视为你的主要移动。」
    //   -- `TriggerKind::TurnStart` and `ctx::ask_tile` exist now; still missing
    //   the board grid geometry (C# `Grid` / `FromGrid` ray-nearest search over
    //   the four axis directions) and `H.MainMoveAs` (teleport + settle
    //   **as the main move** -- the held `ctx::card_move` family).
}
