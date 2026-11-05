//!
//! 规则书（docs/rulebook/cards.json, id `RAS:PLEASE CHOOSE`）:
//! > PLEASE CHOOSE：
//! >  [反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行：
//! > （1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一），
//! > （2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）
//!
//! [反击] the settler picks a forced play at you, or you teleport onto them.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, ChainKind, TriggerKind};
use card_sdk::ctx::NameId;
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
    // `Live House` the agent is not buyable, so not a livehouse.
    ctx::is_live_house_for(trigger::player_id(), t)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let other = trigger::player_id();
    // 规则书[反击]: 「使对方选择以下效果之一执行」 -- the `other` seat picks.
    // 规则书[反击]（1）: 「立即打出一张可将你指定为目标的牌…」 -- the cards in
    // `other`'s hand that designate (`prop::DESIGNATES`) and pass their own
    // press gate for `other`. Offered only when the list is non-empty.
    let mut cards: Vec<NameId> = Vec::new();
    let mut opts: Vec<Msg> = Vec::new();
    for id in ctx::cards_in(other, ctx::CardPile::Hand) {
        let name = id.as_str();
        if ctx::card_prop(name, prop::DESIGNATES) <= 0 {
            continue;
        }
        if !ctx::query_guard(other, name, ctx::entry::PRESS) {
            continue;
        }
        cards.push(id);
        opts.push(
            Msg::new(key!("please_choose_opt1_card"))
                .player_id("who", player_id)
                .card("card", name),
        );
    }
    // 规则书[反击]（2）: 「使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）」
    opts.push(Msg::new(key!("please_choose_opt2")).player_id("who", player_id));
    let pick = ctx::ask_choice(
        other,
        &Msg::new(key!("please_choose_title")),
        &Msg::new(key!("please_choose_ask"))
            .player_id("who", player_id)
            .card("card", "RAS:PLEASE CHOOSE"),
        &opts,
    )?;
    // 规则书[反击]（1）: 「立即打出一张可将你指定为目标的牌并将你指定为目标（之一）」
    if let Some(id) = cards.get(pick as usize) {
        let name = id.as_str();
        // TODO(ABI-04)（将你指定为目标（之一））: the forced play runs the
        //   card's own body, so its target declare still asks `other`; the
        //   force-target play tag (`Tags["force"] = player_id`) is ABI-04's.
        let dest = ctx::play_card(name, other)?;
        apply_dest(other, name, dest);
        ctx::log(
            other,
            &Msg::new("log.play")
                .player_id("who", other)
                .card("card", name),
        );
        return Ok(());
    }
    // `teleport_as` is that teleport as a move of its own: the landing raises
    // `passTile` (「视为可触发乐队技能」; `settles: false` then runs no settle). A bare
    // `teleport_to` would be a position write here (the counteraction resolves
    // inside the settler's move) and raise nothing.
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
    // TODO(规则书)[judgement]
    Ok(())
}

/// Land the forcibly played hand card on its own `Dest` (C# `PlayCard`'s
/// `switch (c.Dest)`); the inner play already ran as that card.
fn apply_dest(player_id: i32, id: &str, dest: ctx::Dest) {
    match dest {
        ctx::Dest::Graveyard => ctx::to_discard(player_id, id),
        ctx::Dest::Field | ctx::Dest::Banished => {}
        ctx::Dest::Hand | ctx::Dest::DeckTop | ctx::Dest::DeckBottom | ctx::Dest::DeckRandom => {
            ctx::add_to_dest(player_id, id, dest, false)
        }
    }
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
