//! `MyGO:（灯）不再迷茫` -- C# `CardTomoriNoLonger` (MatchHost.cs:6308-6347):
//! take two locked [停留] layers, place this card, bin the whole hand and load
//! it with X+1 miracle crystals (X = cards discarded).
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（灯）不再迷茫`）:
//! > （灯）不再迷茫：
//! >
//! > （1）获得两层仅能被自然流失效果移除的[停留]，将此卡放置于场上并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的手牌数
//! > （2）你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐消耗
//! > （3）此卡上的奇迹水晶耗尽后，[移除]此卡
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const TOMORI_NO_LONGER: CardDef = CardDef {
    id: "MyGO:（灯）不再迷茫",
    play: Some(tomori_no_longer),
    can_react: None,
    react: None,
    why_not: None,
};

fn tomori_no_longer(seat: i32) {
    // 规则书（1）: 「获得两层仅能被自然流失效果移除的[停留]」 -- C#
    // `H.GiveStay(i, 2, i, CardName)`.
    ctx::give_stay(seat, 2);
    // 规则书（1）: 「仅能被自然流失效果移除的[停留]」 -- C# `H.IncV(i,
    // "lockedStay", 2)` marks the two layers as locked (cleared only by natural
    // decay).
    ctx::inc_slot(seat, "lockedStay", 2);
    // 规则书（1）: 「将此卡放置于场上」 -- C# `H.PlaceFromPlay(c, -1, -1, list.Count + 1)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:（灯）不再迷茫", &Msg::new(key!("tomori_no_longer_note")));
    ctx::log(seat, &Msg::new(key!("tomori_no_longer_placed")).seat("who", seat));
    // TODO(规则书)（1）: 「并将所有手卡置入弃牌堆，在此卡上放置X+1个[奇迹水晶]，X为你弃置的
    // 手牌数」 -- `discard_from_hand` is available but needs hand enumeration
    // (C# `H._hidden[i].hand`) to walk the whole hand; `hand_count(seat, id)`
    // counts one id only. The per-card crystal counter on the placed card
    // (`PlaceFromPlay(..., crystals: list.Count + 1)`) is also still missing.
    // TODO(规则书)（2）: 「你使用角色技能时可移除此卡上的一个[奇迹水晶]以代替此次技能的火罐
    // 消耗」 -- needs the skill-use attachment hook (C# `CardTomoriNoLonger.Use`
    // via `H.ExtraOf`) and the card crystal counter to spend one.
    // TODO(规则书)（3）: 「此卡上的奇迹水晶耗尽后，[移除]此卡」 -- needs the card
    // crystal counter and its exhaustion path (C# `Use` -> `H.Unplace(this,
    // "removed", ...)`).
}