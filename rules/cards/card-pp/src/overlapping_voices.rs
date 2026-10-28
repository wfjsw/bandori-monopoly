//! `PP:[衍生]重叠的声音` -- C# `CardOverlappingVoices` (MatchHost.cs:7290-7331):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]重叠的声音`）:
//! > [衍生]重叠的声音：
//! > [特]：
//! > 此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上的“初次演出事故”（如果有）。
//! > [手]：
//! > 依次进行以下操作：
//! > 1. [移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；
//! > 2. 进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]；
//! > 3. 回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。
//!
//! remove itself and 「初次演出事故」, teleport to Bandori车站 without settling.
//! The teleport is the turn's main move (`ctx::plan::set_teleport_to` +
//! `ctx::card_move`); the turn-end body is scheduled with `at_turn_end`.
//! The [特] discard hook banishes the accident when this card enters a discard.

use bandori_std::ctx;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};
use skill_groups::fans;

pub const OVERLAPPING_VOICES: CardDef = CardDef::new(
    "PP:[衍生]重叠的声音",
    &[
        On::Play("", None, overlapping_voices),
        On::Schedule("at_end", "", None, at_end),
        // 规则书[特]: 「此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的
        // [场地]上的"初次演出事故"（如果有）」 -- the own-card `Discarded`
        // dispatch names this card, and `trigger_card == card.id` narrows the
        // field-instance dispatch (which also receives every owner discard) to
        // this card's own entry. Banishing the *other* card does not re-enter
        // this hook (the raise names the discarded card's id).
        // TODO(规则书)（此卡进入拥有此卡的玩家的弃卡区时）: 拥有者与弃卡区持有者可能
        // 不是同一人 -- 现在按触发玩家（弃卡区持有者）结算，标准路径下两者相同
        On::Action(Action::Discarded(
            "actor == owner && trigger_card == card.id",
            None,
            on_discarded)),
    ],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"重叠的声音"#)
        .raw_name(r#"[衍生]重叠的声音"#)
        .derived(true)
        .tags(&[r#"手"#, r#"特"#, r#"衍生"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上的“初次演出事故”（如果有）。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"依次进行以下操作："#),
                steps(&[
                    r#"[移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；"#,
                    r#"进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]；"#,
                    r#"回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”加入手卡。"#,
                ]),
            ]),
        ]))
        // sheet cell D5
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]),
);

/// 规则书[手]1 / [特]: 「[移除]…“初次演出事故”」 -- banish the named card if it is
/// on the owner's field. `unplace_card_named` is the one-call named banish
/// (same field-leave semantics as `find_card` + `unplace_at`).
fn banish_accident(player_id: i32) {
    if ctx::unplace_card_named(player_id, "PP:初次演出事故") {
        ctx::log(
            player_id,
            &Msg::new(key!("overlapping_voices_removed_accident")),
        );
    }
}

fn overlapping_voices(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]1: 「[移除]此卡」 -- C# `c.Dest = "removed"`; `[移除]` is banish
    // (`Dest::Banished`), not a discard.
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书[手]1: 「和[使用者][场地]上的“初次演出事故”（如果有）」
    banish_accident(player_id);
    // 规则书[手]3: 「回合结束后获得1层[停留]和1个正面的[P✽P粉丝]，将1张“明天见”
    // 加入手卡」 -- C# `H._turnCtx.AfterEnd.Add(() => After(i))` (before the move).
    ctx::at_turn_end(player_id, "at_end");
    // 规则书[手]2: 「进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”
    // 且不[结算]」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo =
    // H.TileNamed("Bandori车站"), Resolve = false })`: a teleport to the tile
    // that consumes the turn's main move and does not settle.
    let to = ctx::tile_named("Bandori车站");
    if to >= 0 {
        // 规则书[手]2: only the teleport is the turn's main move; steps 1 and 3
        // run even when it is spent or blocked.
        // TODO(规则书)（进入移动阶段并将本回合的[主要移动]改为[传送]到“bandori车站”且不[结算]）:
        // the book prints no [限] -- `[传送]` refuses under 无法移动, but 步骤 1/3
        // still run (same shape as BACKLOG LOY-TSUGU-01).
        if rules_status::move_blocked(player_id).is_none() {
            ctx::plan::set_teleport_to(to);
            ctx::plan::set_resolve(false);
            ctx::card_move(player_id);
        }
    }
    Ok(())
}

/// C# `CardOverlappingVoices.After` -- the scheduled turn-end body.
fn at_end(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]3: 「获得1层[停留]」
    rules_status::stay::add(player_id, 1);
    // 规则书[手]3: 「和1个正面的[P✽P粉丝]」 -- C# `H.GainFans(i, 1, up: true)`.
    let _ = fans::grant(player_id, 1, 0)?;
    // 规则书[手]3: 「将1张“明天见”加入手卡」
    ctx::add_to_dest(player_id, "PP:[衍生]明天见", ctx::Dest::Hand);
    ctx::log(
        player_id,
        &Msg::new(key!("overlapping_voices_after")).player_id("who", player_id),
    );
    Ok(())
}

/// 规则书[特]: 「此卡进入拥有此卡的玩家的弃卡区时[移除]拥有此卡的玩家的[场地]上
/// 的“初次演出事故”（如果有）」 -- `trigger_card == card.id` in the pre owns the
/// "this card, not any card" re-check.
fn on_discarded(player_id: i32) -> card_sdk::Asked {
    banish_accident(player_id);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn overlapping_voices_kani(player_id: i32) -> card_sdk::Asked {
    overlapping_voices(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_end_kani(player_id: i32) -> card_sdk::Asked {
    at_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_discarded_kani(player_id: i32) -> card_sdk::Asked {
    on_discarded(player_id)
}
