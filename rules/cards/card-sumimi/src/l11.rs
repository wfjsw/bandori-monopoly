//! `Sumimi:#L11` -- C# `CardL11` (MatchHost.cs:11592-11620): place with 2 miracle
//!
//! 规则书（data/cards.json, id `Sumimi:#L11`）:
//! > 将此卡置于自身场上，为其添加2个[奇迹水晶]，每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替，此卡[奇迹水晶]数为0时放入弃牌堆。
//!
//! crystals that stand in for band-skill crystal removal. Not in
//! docs/rulebook/cards.json -- translated from the C# class and the card text.

use card_sdk::{ctx, key, CardDef, Msg};

pub const L11: CardDef = CardDef {
    id: "Sumimi:#L11",
    play: Some(l11),
    can_react: None,
    react: None,
    why_not: None,
};

fn l11(seat: i32) {
    // 规则书: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 2)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Sumimi:#L11", &Msg::new(key!("l11_note")));
    ctx::log(seat, &Msg::new(key!("l11_placed")).seat("who", seat));
    // 规则书: 「为其添加2个[奇迹水晶]」 -- C# `PlaceFromPlay(..., crystals: 2)` sets
    // `Card.Crystals = 2` on the placed copy.
    // TODO(规则书): 「为其添加2个[奇迹水晶]」 -- needs a per-field-card crystal
    // counter (C# `Card.Crystals` / `AddCrystals`; the ABI only has
    // `band_crystals` and seat tokens).
    // TODO(规则书): 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替」
    // -- needs the band-skill crystal-removal call site to consult
    // `CardL11.Cover(n)` (spend this card's crystals first).
    // TODO(规则书): 「此卡[奇迹水晶]数为0时放入弃牌堆」 -- needs the same counter plus
    // `H.Unplace(this, "discard", ...)` when it hits 0 (C# `CardL11.Cover`).
    // C# `NoteText` shows the crystal count; CardDef has no NoteText hook.
}