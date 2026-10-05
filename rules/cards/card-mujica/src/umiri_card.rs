//! `Mujica:（海铃）` -- C# `CardUmiriCard` (MatchHost.cs:6117-6258): a travelling
//! placed card that walks one seat per turn and takes band skill cards.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（海铃）`）:
//! > （海铃）
//! >
//! > （1）将此卡放置于在此卡使用者下一名行动的玩家场上，轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。
//! >
//! > （2）使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。
//! >
//! > （3）当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const UMIRI_CARD: CardDef = CardDef {
    id: "Mujica:（海铃）",
    play: Some(umiri_card),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardUmiriCard.WhyNot` -- 「没有别的玩家」 when `H.Others` is empty.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("x_no_others")));
    }
    None // playable
}

fn umiri_card(seat: i32) {
    // 规则书（1）: 「将此卡放置于在此卡使用者下一名行动的玩家场上」 -- C#
    // `H.PlaceFromPlay(c, NextOf(seat))` puts the card on the next live seat's
    // field. `place_card` always places at the given seat; the "next actor" is
    // the next live seat in turn order (`NextOf` walks `seat + 1 ..` skipping
    // outed seats).
    let next = next_of(seat);
    // 规则书（1）: 「将此卡放置于在此卡使用者下一名行动的玩家场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(next, "Mujica:（海铃）", &Msg::new(key!("umiri_note")));
    ctx::log(
        seat,
        &Msg::new(key!("umiri_placed"))
            .seat("who", seat)
            .seat("holder", next),
    );
    // 规则书（2）: 「使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。」
    // C# `CardUmiriCard.Take` copies every non-extra `BandBase` of the holder
    // onto the user as `extra: true` (stackable band skills, but "not that
    // band's character"). The ABI has no band-skill-card inventory (only
    // `band_crystals` / `add_band_crystals` for the crystal counter).
    // TODO(ABI): `H._fx[seat].bands` enumeration + `H.MakeBand(band, user, extra)`
    // so the take / drop bookkeeping (C# `_taken`) can run.
    // 规则书（1）: 「轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。」
    // TODO(ABI): the Fx.TurnStart hook (C# `CardUmiriCard.TurnStart`) to
    // re-place the card on `NextOf(holder)` (or back to the user when the next
    // seat is the user).
    // 规则书（3）: 「当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。」
    // TODO(ABI): the Fx.TurnEnd hook (C# `CardUmiriCard.TurnEnd` -> `End`) --
    // `ctx::unplace_card` + `ctx::draw(user, 1)`, plus the Drop of the taken
    // band cards.
}

/// C# `CardUmiriCard.NextOf` -- the next live seat after `holder` in turn order
/// (wraps to `holder` when there is none). Not `H.Neighbor` / `ctx::neighbor`:
/// that one skips exiled seats too and returns -1 instead of `holder`.
fn next_of(holder: i32) -> i32 {
    let n = ctx::seat_count();
    if n <= 0 {
        return holder;
    }
    for i in 1..=n {
        let s = (holder + i).rem_euclid(n);
        if !ctx::seat_out(s) {
            return s;
        }
    }
    holder
}