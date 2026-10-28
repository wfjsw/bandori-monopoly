//! `Mor:（筑紫）迷茫的庭园` -- C# `CardTsukushiGarden` (MatchHost.cs:5261-5306): roll
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:（筑紫）迷茫的庭园`）:
//! > （筑紫）迷茫的庭园：获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动
//!
//! 1d6 to pick a player and move to the tile in front of them.

use bandori_std::ctx;
use card_sdk::abi::{roll_source, RollPurpose};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TSUKUSHI_GARDEN: CardDef = CardDef::new(
    "Mor:（筑紫）迷茫的庭园",
    &[On::Play("", None, tsukushi_garden)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"迷茫的庭园"#)
        .raw_name(r#"（筑紫）迷茫的庭园"#)
        .tags(&[])
        .text(CardText::para(r#"获得100资金。投掷1d6并根据结果传送到行动条上对应玩家前一格并视为主要移动（选中自己则前进一格，若玩家数量不足6则超出部分重新计算，ex：在五人局roll到6时，视为选中第一位玩家），可以选择是否触发结算。直到下个你的回合开始时，你无法被异常移动"#))
        // sheet cell G16
        .fields(&[("group", r#"Morfonica"#), ("owner", r#"二叶筑紫"#)]));

/// `ctx::slot` marker: the `to + 1` tile of the last hop this card ran for
/// the seat. Written before the move so a replayed body can recognise its own
/// landing even if a `moveBefore` counteraction redirected it.
const MOVED: &str = "tsukushi_garden_moved";

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
    let r = ctx::roll(player_id, 1, 6, RollPurpose::Effect, roll_source::CARD)?;
    let who = list[((r - 1).rem_euclid(count)) as usize];
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    let to = if who == player_id {
        // 规则书: 「选中自己则前进一格」 -- one step in the turn's walk direction
        // (a reverse-armed turn walks backwards); the log and the settle prompt
        // must name the tile actually entered.
        ctx::tile_steps_ahead(who, ctx::plan::dir())
    } else {
        ctx::tile_steps_ahead(who, 1)
    };
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
    // 规则书: 「并视为主要移动」 -- C# `H.CardMove(c, new MoveCtx { ... })`.
    // 「可以选择是否触发结算」 is asked *after* the move (the choice is about
    // the tile actually entered), so the move runs `Resolve = false` here and
    // the settle below is applied to the landed tile.
    // TODO(规则书)（并视为主要移动）: the book prints no play gate, so the card
    // stays playable and 「获得100资金」/the protection land even with the main
    // move spent or [停留] latched; only this move arm reads the gate.
    // Whether it should be gated at all is the same open question as
    // LOY-TSUGU-01.
    // The guard must not be re-read on a replay: the body re-runs with the move
    // already applied, so `move_blocked` would answer "already moved" and skip
    // the settle prompt (LOY-TSUGU-01). The landed position or the `MOVED`
    // marker says this play already moved; only a first run consults the gate.
    let moved_here = ctx::player_pos(player_id) == to || ctx::slot(player_id, MOVED) == to + 1;
    if moved_here || rules_status::move_blocked(player_id).is_none() {
        ctx::set_slot(player_id, MOVED, to + 1);
        if who == player_id {
            // 规则书: 「选中自己则前进一格」 -- C# `new MoveCtx { Steps = 1 }`.
            ctx::plan::set_steps(1);
        } else {
            // 规则书: 「传送到行动条上对应玩家前一格」 -- C# `new MoveCtx {
            //   TeleportTo = to }`.
            ctx::plan::set_kind(card_sdk::abi::MoveKind::Teleport);
            ctx::plan::set_teleport_to(to);
        }
        ctx::plan::set_resolve(false);
        ctx::card_move(player_id);
        // 规则书: 「可以选择是否触发结算」 -- C# `H.AskYes(..., "这次移动要 [结算]
        //   吗？", default yes when the tile is buyable and unowned-or-yours)`;
        //   the generic `ctx::ask_yes` has no default parameter, so its
        //   fallback is always No.
        let settle = ctx::ask_yes(
            player_id,
            &Msg::new(key!("tsukushi_garden_settle_title")),
            &Msg::new(key!("tsukushi_garden_settle_text")).tile("tile", to),
        )?;
        if settle {
            // 规则书: 「可以选择是否触发结算」 -- the landing is settled by hand
            // (the move ran without it), as the turn's landing.
            ctx::log(
                player_id,
                &Msg::new(key!("tsukushi_garden_will_settle")).tile("tile", to),
            );
            ctx::card_settle_at(player_id, to, true);
        }
    }
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书: 「直到下个你的回合开始时，你无法被异常移动」 -- `expires: TurnStart`
    // is the 「直到下个你的回合开始时」 half (it wears off at the top of the next
    // turn); the clause is granted as [不可阻挡], whose other halves ([停留]
    // bypass, no new [晕眩]/[除外] layers) come with the status.
    // TODO(规则书)（你无法被异常移动）: the book refuses abnormal moves -- the engine's
    // `abnormal_gate` never consults [不可阻挡], so a received teleport / forced
    // move / forced stop (and a new [停留] layer) still land.
    ctx::state::add(player_id, bandori_std::state_key::UNSTOPPABLE, 1);
    bandori_std::status::declare_companion(player_id, bandori_std::state_key::UNSTOPPABLE);
    ctx::state::set_expires(
        player_id,
        bandori_std::state_key::UNSTOPPABLE,
        card_sdk::ctx::state::TURN_START,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("tsukushi_garden_guard")).player_id("who", player_id),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn tsukushi_garden_kani(player_id: i32) -> card_sdk::Asked {
    tsukushi_garden(player_id)
}
