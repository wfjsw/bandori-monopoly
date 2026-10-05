//! `Mor:（筑紫）迷茫的庭园` -- C# `CardTsukushiGarden` (MatchHost.cs:5261-5306): roll
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（筑紫）迷茫的庭园`）:
//! > （筑紫）迷茫的庭园：获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动
//!
//! 1d6 to pick a player and move to the tile in front of them.

use card_sdk::{ctx, key, CardDef, Msg};

pub const TSUKUSHI_GARDEN: CardDef = CardDef {
    id: "Mor:（筑紫）迷茫的庭园",
    play: Some(tsukushi_garden),
    can_react: None,
    react: None,
    why_not: None,
};

fn tsukushi_garden(seat: i32) {
    // C# `WhyNot` is `H.MoveWhyNot` (only playable while the turn's main move is
    // still open); `CardDef` has no why_not hook yet.
    // 规则书: 「获得100资金」 -- C# `H.GainR(i, 100, CardName)`.
    ctx::gain(seat, 100, &Msg::new(key!("tsukushi_garden_why")));
    // 规则书: 「投掷1d6并根据结果传送到行动条上对应玩家前一格」
    // 「若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家」
    // -- C# `list` is the still-in seats in index order,
    // `who = list[(num - 1) % list.Count]`.
    let mut list: [i32; 10] = [0; 10];
    let mut count = 0;
    let n_seats = ctx::seat_count();
    for p in 0..n_seats {
        if !ctx::seat_out(p) && (count as usize) < list.len() {
            list[count as usize] = p;
            count += 1;
        }
    }
    if count <= 0 {
        return;
    }
    let r = ctx::roll(seat, 1, 6);
    let who = list[((r - 1).rem_euclid(count)) as usize];
    let n = ctx::tile_count();
    if n <= 0 {
        return;
    }
    let to = ctx::tile_steps_ahead(who, 1);
    if to < 0 {
        return;
    }
    if who == seat {
        // 规则书: 「选中自己则前进一格」
        ctx::log(seat, &Msg::new(key!("tsukushi_garden_self")).seat("who", seat).tile("tile", to));
    } else {
        // 规则书: 「传送到行动条上对应玩家前一格」
        ctx::log(
            seat,
            &Msg::new(key!("tsukushi_garden_other")).seat("who", who).tile("tile", to).i("roll", r as i64),
        );
    }
    // 规则书: 「可以选择是否触发结算」 -- C# `H.AskYes(..., "这次移动要 [结算] 吗？",
    //   default yes when the tile is buyable and unowned-or-yours)`.
    let settle = ctx::ask_yes(
        seat,
        &Msg::new(key!("tsukushi_garden_settle_title")),
        &Msg::new(key!("tsukushi_garden_settle_text")).tile("tile", to),
    );
    // 规则书: 「并视为主要移动」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo = to,
    //   Resolve = rs.yes })` (or `Steps = 1` when `who == i`).
    // TODO(规则书): 「并视为主要移动」 and 「（不触发结算 / 触发结算）」 -- needs the
    //   H.CardMove / main-move routine so the hop consumes the turn's main move and
    //   honours the settle answer (`MoveCtx.Resolve`). `ctx::teleport_to` always
    //   skips settling; a self-picked 「前进一格」 is approximated as a teleport to
    //   the tile ahead.
    if settle {
        ctx::log(seat, &Msg::new(key!("tsukushi_garden_will_settle")).tile("tile", to)); // 规则书: 「可以选择是否触发结算」
    }
    // 规则书: 「传送到行动条上对应玩家前一格」 / 「选中自己则前进一格」
    ctx::teleport_to(seat, to);
    if ctx::seat_out(seat) {
        return;
    }
    // 规则书: 「直到下个你的回合开始时，你无法被异常移动」 -- C#
    // `H.ExtraOf<GardenGuardFx>(i).Until = H.State.round`.
    // TODO(规则书): 「直到下个你的回合开始时，你无法被异常移动」 -- needs the
    //   GardenGuard Fx (C# `GardenGuardFx`, an `H.ExtraOf` attachment that refuses
    //   abnormal movement until the owner's next turn starts).
    ctx::log(seat, &Msg::new(key!("tsukushi_garden_guard")).seat("who", seat));
}