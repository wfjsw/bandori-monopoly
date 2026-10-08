//! `Mor:（筑紫）迷茫的庭园` -- C# `CardTsukushiGarden` (MatchHost.cs:5261-5306): roll
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（筑紫）迷茫的庭园`）:
//! > （筑紫）迷茫的庭园：获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动
//!
//! 1d6 to pick a player and move to the tile in front of them.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TSUKUSHI_GARDEN: CardDef = CardDef::new(
    "Mor:（筑紫）迷茫的庭园",
    &[On::Play(Some(cant_play), tsukushi_garden, "")],
);

/// C# `CardTsukushiGarden.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「并视为主要移动」 -- the hop is the turn's main move, so the C#
    // `H.MoveWhyNot` gate applies (off-turn / already-moved / skip-move refuse).
    ctx::cant_move(player_id)
}

fn tsukushi_garden(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「获得100资金」 -- C# `H.GainR(i, 100, CardName)`.
    ctx::gain(player_id, 100, &Msg::new(key!("tsukushi_garden_why")))?;
    // 规则书: 「投掷1d6并根据结果传送到行动条上对应玩家前一格」
    // 「若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家」
    // -- C# `list` is the still-in players in index order,
    // `who = list[(num - 1) % list.Count]`.
    let mut list: [i32; 10] = [0; 10];
    let mut count = 0;
    let n_players = ctx::player_count();
    for p in 0..n_players {
        if !ctx::player_out(p) && (count as usize) < list.len() {
            list[count as usize] = p;
            count += 1;
        }
    }
    if count <= 0 {
        return Ok(());
    }
    let r = ctx::roll(player_id, 1, 6);
    let who = list[((r - 1).rem_euclid(count)) as usize];
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let to = ctx::tile_steps_ahead(who, 1);
    if to < 0 {
        return Ok(());
    }
    if who == player_id {
        // 规则书: 「选中自己则前进一格」
        ctx::log(
            player_id,
            &Msg::new(key!("tsukushi_garden_self"))
                .player_id("who", player_id)
                .tile("tile", to),
        );
    } else {
        // 规则书: 「传送到行动条上对应玩家前一格」
        ctx::log(
            player_id,
            &Msg::new(key!("tsukushi_garden_other"))
                .player_id("who", who)
                .tile("tile", to)
                .i("roll", r as i64),
        );
    }
    // 规则书: 「可以选择是否触发结算」 -- C# `H.AskYes(..., "这次移动要 [结算] 吗？",
    //   default yes when the tile is buyable and unowned-or-yours)`.
    let settle = ctx::ask_yes(
        player_id,
        &Msg::new(key!("tsukushi_garden_settle_title")),
        &Msg::new(key!("tsukushi_garden_settle_text")).tile("tile", to),
    )?;
    // 规则书: 「并视为主要移动」 -- C# `H.CardMove(c, new MoveCtx { ... })`.
    if settle {
        ctx::log(
            player_id,
            &Msg::new(key!("tsukushi_garden_will_settle")).tile("tile", to),
        ); // 规则书: 「可以选择是否触发结算」
    }
    if who == player_id {
        // 规则书: 「选中自己则前进一格」 -- C# `new MoveCtx { Steps = 1,
        //   Resolve = rs.yes }` = `set_steps(1)` + `set_resolve(settle)` +
        //   `card_move(player_id)`.
        ctx::plan::set_steps(1);
        ctx::plan::set_resolve(settle);
        ctx::card_move(player_id);
    } else {
        // 规则书: 「传送到行动条上对应玩家前一格」 -- C# `new MoveCtx {
        //   TeleportTo = to, Resolve = rs.yes }` = `set_kind(Teleport)` +
        //   `set_teleport_to(to)` + `set_resolve(settle)` + `card_move(player_id)`.
        ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
        ctx::plan::set_teleport_to(to);
        ctx::plan::set_resolve(settle);
        ctx::card_move(player_id);
    }
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书: 「直到下个你的回合开始时，你无法被异常移动」 -- `unstoppable` is
    // exactly that gate, and `expires: TurnStart` is the 「直到下个你的回合
    // 开始时」 half (it wears off at the top of the next turn).
    ctx::state::add(player_id, card_sdk::abi::state_key::UNSTOPPABLE, 1);
    ctx::state::set_expires(
        player_id,
        card_sdk::abi::state_key::UNSTOPPABLE,
        card_sdk::ctx::state::TURN_START,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("tsukushi_garden_guard")).player_id("who", player_id),
    );
    Ok(())
}
