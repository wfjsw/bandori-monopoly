//! `R:必然的联系（莉莎）` -- C# `CardLisaBond` (MatchHost.cs:10960-10999): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:必然的联系（莉莎）`）:
//! > 必然的联系（莉莎）：【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。
//!
//! after a skill teleport: park this card on a same-tile character's field so
//! your skills on them may skip the 「距离最近」 limit.

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const LISA_BOND: CardDef = CardDef::new("R:必然的联系（莉莎）", &[
    On::CounterAct(&[ChainKind::SkillTeleport], can_react, react),
]);

/// 规则书[反击]: 「【反击】当你使用技能进行传送后，你可以打出此卡」
fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当你使用技能进行传送后」 -- C# `t.Kind == "skillTeleport" && t.Seat == seat`.
    if trigger::kind() != TriggerKind::SkillTeleport || trigger::player_id() != player_id {
        return false;
    }
    // 规则书[反击]: 「指定一个和你在同一地块的角色」 -- only worth reacting with someone
    // else on your tile (C# `H.SeatsOn(pos, seat).Count > 0`).
    !ctx::players_on(ctx::player_pos(player_id), player_id).is_empty()
}

fn react(player_id: i32) {
    let candidates: Vec<i32> = ctx::players_on(ctx::player_pos(player_id), player_id);
    if candidates.is_empty() {
        return;
    }
    // 规则书[反击]: 「并指定一个和你在同一地块的角色」 -- C# `H.PickTarget` over the
    // same-tile players: `H.AskSeat` then the `H.Target` gate (`SingleTarget`).
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("lisa_bond_ask_title")),
        &Msg::new(key!("lisa_bond_ask_text")),
        &candidates,
    );
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
        ctx::place_card_at(who, "R:必然的联系（莉莎）", &Msg::new(key!("lisa_bond_note")).player_id("who", player_id));
        ctx::log(
            player_id,
            &Msg::new(key!("lisa_bond_placed")).player_id("who", player_id).player_id("them", who),
        );
    }
    // TODO(规则书)[反击]: 「你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆」
    //   -- needs a persistent skill-targeting exemption on the placed card (C#
    //   `CardLisaBond.NoteText` / the skill system's 「最近」 gate) plus the discard
    //   once the exemption is used.
}