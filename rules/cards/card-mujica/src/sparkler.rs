//! `Mujica:燃尽前的线香花火` -- C# `CardSparkler` (MatchHost.cs:5542-5580): place
//! with 2 miracle crystals; each turn end burns one for an extra turn.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:燃尽前的线香花火`）:
//! > 燃尽前的线香花火：
//! > 将此卡放置于场上并放置2个奇迹水晶（上限2），你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合，最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SPARKLER: CardDef = CardDef {
    id: "Mujica:燃尽前的线香花火",
    play: Some(sparkler),
    can_react: None,
    react: None,
    why_not: None,
};

fn sparkler(seat: i32) {
    // 规则书: 「将此卡放置于场上并放置2个奇迹水晶（上限2）」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mujica:燃尽前的线香花火", &Msg::new(key!("sparkler_note")));
    ctx::log(seat, &Msg::new(key!("sparkler_placed")).seat("who", seat));
    // TODO(规则书): 「并放置2个奇迹水晶（上限2）」 -- C# `H.PlaceFromPlay(c, -1, -1, 2)`
    // loads the card with 2 crystals; `place_card` has no crystal count (needs
    // `H.PlaceFromPlay(..., crystals)` / a per-card `Crystals` field in the ABI).
    // TODO(规则书): 「你的回合结束后自动移除一个奇迹水晶并使你获得一个额外回合」
    // -- needs the Fx.TurnEndAfter hook (C# `CardSparkler.TurnEndAfter` -> `Burn`)
    // to spend a crystal and call `ctx::give_extra_turn`.
    // TODO(规则书): 「最后一个奇迹水晶移除后将此卡置入弃牌堆并立刻使你获得2层[眩晕]」
    // -- needs the same TurnEndAfter hook (C# `Burn` -> `H.Unplace(..., "discard")`
    // + `H.GiveStun(Seat, 2)`), plus `ctx::give_stun(seat, 2)`.
}