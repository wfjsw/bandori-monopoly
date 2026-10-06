//! `Sumimi:#L11` -- C# `CardL11` (MatchHost.cs:11592-11620): place with 2 miracle
//!
//! 规则书（data/cards.json, id `Sumimi:#L11`）:
//! > 将此卡置于自身场上，为其添加2个[奇迹水晶]，每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替，此卡[奇迹水晶]数为0时放入弃牌堆。
//!
//! crystals that stand in for band-skill crystal removal. Not in
//! docs/rulebook/cards.json -- translated from the C# class and the card text.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const L11: CardDef = CardDef::new("Sumimi:#L11", &[
    On::Play(None, l11),
    On::Hook(&[card_sdk::abi::HookKind::TurnEnd], mine, sweep)]);

fn l11(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 2)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "Sumimi:#L11", &Msg::new(key!("l11_note")));
    // 规则书: 「为其添加2个[奇迹水晶]」 -- C# `PlaceFromPlay(..., crystals: 2)` sets
    // `Card.Crystals = 2` on the placed copy.
    ctx::set_crystals(2);
    ctx::log(player_id, &Msg::new(key!("l11_placed")).player_id("who", player_id));
    // 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替」 --
    // the two band skills that spend crystals (`ave_mujica::halve`,
    // `poppin::cash`) drain `Sumimi:#L11` first, which is `CardL11.Cover(n)`.
    // 「此卡[奇迹水晶]数为0时放入弃牌堆」 -- swept at the turn end rather than
    // inside `Cover`, so it also catches a spend from another card.
    // C# `NoteText` shows the crystal count; CardDef has no NoteText hook.
    Ok(())
}

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// 「此卡[奇迹水晶]数为0时放入弃牌堆」.
fn sweep(_player_id: i32) -> card_sdk::Asked {
    if ctx::crystals() > 0 {
        return Ok(());
    }
    if !ctx::is_placed() {
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}
