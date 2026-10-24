//! `R:必然的联系（莉莎）` -- C# `CardLisaBond` (MatchHost.cs:10960-10999): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:必然的联系（莉莎）`）:
//! > 必然的联系（莉莎）：【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。
//!
//! after a skill teleport: park this card on a same-tile character's field so
//! your skills on them may skip the 「距离最近」 limit.
//!
//! Raised on the unified `MoveAfter` (every movement case) and discriminated
//! by cond: `move.kind == Teleport && move.by_skill` selects 「使用技能进行传送」.
//! The pre reads the POST-move position (`players_on(owner.pos, …)`).

use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::abi::ChainKind;
use card_sdk::{CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const LISA_BOND: CardDef = CardDef::new(
    "R:必然的联系（莉莎）",
    // TODO(规则书): 「后」 names no stage number (closest is stage 13
    // 「传送后在终点触发重叠」). `MoveAfter` is 「移动后」 (行动阶段 13) --
    // the move-end point every movement case raises.
    &[On::Counteract(
        &[ChainKind::MoveAfter],
        "actor == owner && move.kind == Teleport && move.by_skill && players_on(owner.pos, owner) > 0",
        None,
        counteract).counter_reason(key!("lisa_bond_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"必然的联系"#)
        .raw_name(r#"必然的联系（莉莎）"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。"#))
        // sheet cell E16
        .fields(&[("group", r#"Roselia"#), ("owner", r#"今井莉莎"#)]));

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor == owner && players_on(owner.pos, owner) > 0` is the pre.
    let candidates: Vec<i32> = ctx::players_on(ctx::player_pos(player_id), player_id);
    // 规则书[反击]: 「并指定一个和你在同一地块的角色」 -- C# `H.PickTarget` over the
    // same-tile players: `H.AskSeat` then the `H.Target` gate (`SingleTarget`).
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("lisa_bond_ask_title")),
        &Msg::new(key!("lisa_bond_ask_text")),
        &candidates,
    )?;
    // C# `H.Target(c, r.index, t)` -> `res.index = t.yes ? t.index : -1`: out /
    // exile / `ImmuneAll` / `Untargetable` / the `target` [反击] window all fail
    // the designation, and a `redirect` hook may move the hit.
    let hit = ctx::target(who);
    // 规则书[反击]: 「将此卡放在对方的游戏区」 -- C# `if (r.index >= 0)
    // H.PlaceFromPlay(c, r.index)` places at the hit player_id's field
    // (`PlaceCard(owner = r.index, user = c.Seat, ...)`), and only when the gate
    // let the designation through.
    if let Some(who) = hit {
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card_at(
            who,
            "R:必然的联系（莉莎）",
            &Msg::new(key!("lisa_bond_note")).player_id("who", player_id),
        );
        ctx::log(
            player_id,
            &Msg::new(key!("lisa_bond_placed"))
                .player_id("who", player_id)
                .player_id("them", who),
        );
    }
    // TODO(规则书)[反击]: 「你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆」
    //   -- needs a persistent skill-targeting exemption on the placed card (C#
    //   `CardLisaBond.NoteText` / the skill system's 「最近」 gate) plus the discard
    //   once the exemption is used.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
