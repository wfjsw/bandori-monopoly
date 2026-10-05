//! `R:必然的联系（莉莎）` -- C# `CardLisaBond` (MatchHost.cs:10960-10999): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:必然的联系（莉莎）`）:
//! > 必然的联系（莉莎）：【反击】当你使用技能进行传送后，你可以打出此卡并指定一个和你在同一地块的角色，将此卡放在对方的游戏区，你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆。
//!
//! after a skill teleport: park this card on a same-tile character's field so
//! your skills on them may skip the 「距离最近」 limit.

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const LISA_BOND: CardDef = CardDef {
    id: "R:必然的联系（莉莎）",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书[反击]: 「【反击】当你使用技能进行传送后，你可以打出此卡」
fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当你使用技能进行传送后」 -- C# `t.Kind == "skillTeleport" && t.Seat == seat`.
    if trigger::kind() != TriggerKind::SkillTeleport || trigger::seat() != seat {
        return false;
    }
    // 规则书[反击]: 「指定一个和你在同一地块的角色」 -- only worth reacting with someone
    // else on your tile (C# `H.SeatsOn(pos, seat).Count > 0`).
    !ctx::seats_on(ctx::seat_pos(seat), seat).is_empty()
}

fn react(seat: i32) {
    let candidates: Vec<i32> = ctx::seats_on(ctx::seat_pos(seat), seat);
    if candidates.is_empty() {
        return;
    }
    // 规则书[反击]: 「并指定一个和你在同一地块的角色」 -- C# `H.PickTarget` over the
    // same-tile seats.
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("lisa_bond_ask_title")),
        &Msg::new(key!("lisa_bond_ask_text")),
        &candidates,
    );
    // 规则书[反击]: 「将此卡放在对方的游戏区」 -- C# `H.PlaceFromPlay(c, r.index)` places
    // at the target's field (`PlaceCard(owner = r.index, user = c.Seat, ...)`).
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_at(who, "R:必然的联系（莉莎）", &Msg::new(key!("lisa_bond_note")).seat("who", seat));
    ctx::log(
        seat,
        &Msg::new(key!("lisa_bond_placed")).seat("who", seat).seat("them", who),
    );
    // TODO(规则书)[反击]: 「你对对方使用技能将可以无视【距离最近】这一限制。当你发动该效果后，此卡进入弃牌堆」
    //   -- needs a persistent skill-targeting exemption on the placed card (C#
    //   `CardLisaBond.NoteText` / the skill system's 「最近」 gate) plus the discard
    //   once the exemption is used. Also `H.PickTarget` runs the `H.Target`
    //   targeting gate after the seat prompt -- no such gate in the vocabulary yet.
}