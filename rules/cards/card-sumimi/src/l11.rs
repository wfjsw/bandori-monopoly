//! `Sumimi:#L11` -- C# `CardL11` (MatchHost.cs:11592-11620): place with 2 miracle
//!
//! 规则书（the roster, id `Sumimi:#L11`）:
//! > 将此卡置于自身场上，为其添加2个[奇迹水晶]，每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替，此卡[奇迹水晶]数为0时放入弃牌堆。
//!
//! crystals that stand in for band-skill crystal removal.

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

pub const L11: CardDef = CardDef::new(
    "Sumimi:#L11",
    &[
        On::Play("", None, l11),
        // 规则书: 「此卡[奇迹水晶]数为0时放入弃牌堆」 -- a state trigger on the
        // counter itself (`counterChanged`), so the very write that empties it
        // files the card. The guard (`crystals_changed_of`) pins the instance
        // (the raise is name-keyed) and the negative-delta check keeps the
        // `set_crystals(2)` of the play from firing it.
        On::Action(Action::CounterChanged(
            "card.placed && counter_is('crystals') && card.counter('crystals') <= 0 && value <= 0",
            Some(crystals_changed_guard),
            sweep)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        // sheet cell L11 -- the name line is blank; `#L11` is the name.
        .name("#L11")
        .raw_name("")
        .tags(&[])
        .text(CardText::para(r#"将此卡置于自身场上，为其添加2个[奇迹水晶]，每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替，此卡[奇迹水晶]数为0时放入弃牌堆。"#))
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

fn l11(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「将此卡置于自身场上」 -- C# `H.PlaceFromPlay(c, -1, -1, 2)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "Sumimi:#L11", &Msg::new(key!("l11_note")));
    // 规则书: 「为其添加2个[奇迹水晶]」 -- C# `PlaceFromPlay(..., crystals: 2)` sets
    // `Card.Crystals = 2` on the placed copy.
    ctx::set_crystals(2);
    ctx::log(
        player_id,
        &Msg::new(key!("l11_placed")).player_id("who", player_id),
    );
    // 规则书: 「每当乐队技能需要移除[奇迹水晶]时，可移除此卡上的一个[奇迹水晶]代替」 --
    // every band skill's crystal cost drains this card first through
    // `skill_groups::spend_crystals` (see `poppin::cash`, `ave_mujica::halve`
    // and the other band skills' spends), which is C# `CardL11.Cover(n)`.
    // 规则书: 「此卡[奇迹水晶]数为0时放入弃牌堆」 -- the `counterChanged` entry
    // above, not a turn-end re-check, so a spend by another card is caught too.
    // C# `NoteText` shows the crystal count; CardDef has no NoteText hook.
    Ok(())
}

/// 规则书: 「此卡[奇迹水晶]数为0时放入弃牌堆」 -- run by the `counterChanged`
/// entry above: the card is placed and its `crystals` counter was just written
/// to 0 (or below).
fn sweep(_player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}

/// Residual the condition cannot express: the changed card is this instance
/// (the raise carries the card id, not the uid) and the counter is `crystals`.
fn crystals_changed_guard(_player_id: i32) -> bool {
    bandori_std::crystals_changed_of("Sumimi:#L11")
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn l11_kani(player_id: i32) -> card_sdk::Asked {
    l11(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn sweep_kani(player_id: i32) -> card_sdk::Asked {
    sweep(player_id)
}
