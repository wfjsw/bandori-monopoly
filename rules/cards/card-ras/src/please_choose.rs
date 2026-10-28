//! `RAS:PLEASE CHOOSE` -- C# `CardPleaseChoose` (MatchHost.cs:9565-9636):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:PLEASE CHOOSE`）:
//! > PLEASE CHOOSE：
//! >  [反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行：
//! > （1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一），
//! > （2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）
//!
//! [反击] the settler picks a forced play at you, or you teleport onto them.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::{it, items, para, sec, CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const PLEASE_CHOOSE: CardDef = CardDef::new(
    "RAS:PLEASE CHOOSE",
    &[
        On::Counteract(&[ChainKind::Settle], "", Some(can_counteract), counteract)
            .counter_reason(key!("please_choose_when")),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"PLEASE CHOOSE"#)
        .raw_name(r#"PLEASE CHOOSE"#)
        .tags(&[r#"反击"#])
        .text(CardText::of(&[sec(
            "",
            &[
                para(r#"[反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行："#),
                items(&[
                    it(r#"立即打出一张可将你指定为目标的牌并将你指定为目标（之一），"#),
                    it(r#"使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）"#),
                ]),
            ],
        )]))
        // sheet cell H7
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]),
);

/// 规则书[反击]: 「当有玩家在livehouse格子上结算时」
fn can_counteract(_player_id: i32) -> bool {
    // 规则书[反击]: 「当有玩家在livehouse格子上结算时」 -- any settle, the
    // holder's own included (「有玩家」 is any player).
    if trigger::kind() != TriggerKind::Settle {
        return false;
    }
    let t = trigger::tile();
    if t < 0 {
        return false;
    }
    // 规则书[反击]: 「在livehouse格子上结算」 -- C# `H.IsLiveHouse(t.Seat, t.Tile)`
    // + `IsBuyable`: the class `livehouse` as the *settler* sees it (buyable,
    // group 6, this player's `Fx.ExtraColor` or a colour/class override).
    // `Live House` the agent is not buyable, so not a livehouse.
    ctx::is_live_house_for(trigger::player_id(), t)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let other = trigger::player_id();
    // 规则书[反击]: 「使对方选择以下效果之一执行」 -- C# `H.AskPick` of `other`.
    // TODO(规则书)[judgement][反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
    //   the clause under-specifies -- see the note above it
    // -- C# filters `other`'s hand for `Normal && Targeting && WhyNot == null`,
    // `H.AskCard`, then `H.PlayCard` with `Tags["force"] = player_id`. Hand listing
    // (`ctx::cards_in`) and the press gate (`ctx::query_guard(p, id, ctx::entry::PRESS)`) are
    // ready; the targeting flags (`H.Target` / `Card.Targeting`) and the
    // force-target play tag are still missing, so option (1) is offered but
    // not executed.
    let opt1 = Msg::new(key!("please_choose_opt1")).player_id("who", player_id);
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」
    let opt2 = Msg::new(key!("please_choose_opt2")).player_id("who", player_id);
    let pick = ctx::ask_pick(
        other,
        &Msg::new(key!("please_choose_title")),
        &Msg::new(key!("please_choose_ask"))
            .player_id("who", player_id)
            .card("card", "RAS:PLEASE CHOOSE"),
        &[opt1, opt2],
    )?;
    // 规则书[反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
    if pick == 0 {
        // TODO(规则书)[judgement][反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
        //   the clause under-specifies -- see the note above it
        // -- see above; the forced play needs `CardDef.targeting()` / `Normal`
        // (hand filter) and a force-target play tag. `ctx::cards_in(other,
        // CardPile::Hand)` lists the hand now.
        ctx::log(
            other,
            &Msg::new(key!("please_choose_opt1_todo"))
                .player_id("who", other)
                .player_id("counteractor", player_id),
        );
        return Ok(());
    }
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子」 -- C# `H.ForceTeleport(i,
    // to, resolve: false, ...)`. `teleport_as` is that teleport as a move of its
    // own: the landing raises `passTile` (「视为可触发乐队技能」; `resolve: false`
    // then runs no settle). A bare `teleport_to` would be a position write here
    // (the counteraction resolves inside the settler's move) and raise nothing.
    let to = ctx::player_pos(other);
    if to >= 0 {
        // `player_pos` only reads -1 for a removed player: keep the write
        // guarded, but still log below so the pick never resolves invisibly.
        // `teleport_as` re-reads the shared plan's `resolve` after the pass
        // window (Play::teleport_as_depth), and a nested call would inherit the
        // settling outer move's `true`; the `TeleportAs` handler makes the
        // request's own `resolve` authoritative across the call, so `false`
        // here means the landing raises `passTile` and never settles.
        ctx::teleport_as(player_id, to, false, false);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("please_choose_moved"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // TODO(规则书)[judgement][反击]（2）: 「（不触发结算但视为可触发乐队技能）」 -- the
    //   the clause under-specifies -- see the note above it
    // The landing raise is `passTile` (what band skills hang on), but it also
    // reaches field cards' `PassTile` hooks, which C# (`H.EachOf(i, f =>
    // f.PassTile(m, to))`) did not fan.
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
