//! `HHW:（花音）Wacha Mocha 啪嗒进行曲` -- C# `CardKanonMarch` (MatchHost.cs:4234-4291).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（花音）Wacha Mocha 啪嗒进行曲`）:
//! > （花音）Wacha Mocha 啪嗒进行曲：[场]花音每次倒走获得一个水母标记，当水母标记到达9个时可以清除所有标记传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后置入弃牌堆。
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const KANON_MARCH: CardDef = CardDef::new("HHW:（花音）Wacha Mocha 啪嗒进行曲", &[
    On::Play(play),
]);

fn play(player_id: i32) {
    // 规则书: 「[场]」 -- a field card; C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "HHW:（花音）Wacha Mocha 啪嗒进行曲", &Msg::new(key!("kanon_march_note")));
    ctx::log(player_id, &Msg::new(key!("kanon_march_placed")).player_id("who", player_id));
    // TODO(规则书): 「花音每次倒走获得一个水母标记」 -- needs the Fx.Arrive hook
    // (C# `CardKanonMarch.Arrive`, `m.Reverse && !m.Teleport`) to add one 水母标记
    // per backward move (`ctx::add_tok(player_id, "kanon_march_tok", 1, ...)`).
    // TODO(规则书): 「当水母标记到达9个时可以清除所有标记传送到#4水族馆或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后置入弃牌堆」
    // -- the body is expressible with the landed ops (`AskTileOf` over
    // 水族馆 / 弦卷豪宅 -> `plan::set_kind(MoveKind::Teleport)` +
    // `plan::set_teleport_to(to)` + `plan::set_resolve(true)` + unplace;
    // 9-mark check is `ctx::tok(player_id, "kanon_march_tok") >= 9`) but no
    // `TriggerKind::MoveBefore` hook point exists (C#
    // `CardKanonMarch.MoveBefore` -> `Jump`), so it cannot fire. `On::RollPlan`
    // is the closest pre-dice hook but `main_move` does not branch on
    // `teleport_to` the way C# `MainMove` does (`m.TeleportTo >= 0 && m.Steps < 0`
    // -> `TeleportMove`), so the rewrite would still walk.
}