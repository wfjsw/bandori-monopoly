//! `Sumimi:#L11` -- C# `CardL11` (MatchHost.cs:11592-11620): place with 2 miracle
//!
//! 规则书（data/cards.json, id `Sumimi:#L11`）:
//! > 将此卡置于自身场上，为其添加2个[奇迹水晶]，每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替，此卡[奇迹水晶]数为0时放入弃牌堆。
//!
//! crystals that stand in for band-skill crystal removal. Not in
//! docs/rulebook/cards.json -- translated from the C# class and the card text.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const L11: CardDef = CardDef::new("Sumimi:#L11", &[
    On::Play(l11),
]);

fn l11(player_id: i32) {
    // 规则书: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 2)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "Sumimi:#L11", &Msg::new(key!("l11_note")));
    // 规则书: 「为其添加2个[奇迹水晶]」 -- C# `PlaceFromPlay(..., crystals: 2)` sets
    // `Card.Crystals = 2` on the placed copy.
    ctx::set_crystals(player_id, 2);
    ctx::log(player_id, &Msg::new(key!("l11_placed")).player_id("who", player_id));
    // TODO(规则书): 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替」
    // -- needs the band-skill crystal-removal call site to consult
    // `CardL11.Cover(n)` (spend this card's crystals first). The counter half is
    // ready (`ctx::add_crystals`); only the call site is missing.
    // TODO(规则书): 「此卡[奇迹水晶]数为0时放入弃牌堆」 -- C# `CardL11.Cover`'s empty
    // branch (`H.Unplace(this, "discard", ...)`). `ctx::unplace_card` +
    // `ctx::to_discard` are ready; the branch only runs inside the same missing
    // `Cover(n)` call site.
    // C# `NoteText` shows the crystal count; CardDef has no NoteText hook.
}
