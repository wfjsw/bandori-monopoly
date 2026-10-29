//! `CRYCHIC:（睦）从没有觉得...` -- C# `CardMutsumiNever` (MatchHost.cs:3164-3216):
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（睦）从没有觉得...`）:
//! > （睦）从没有觉得... ：
//!
//! > （1）打出此卡时，使用者可以选择
//! > （2）或
//! > （3）效果之一发动。
//!
//! > （2）消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的
//! > （2）效果，然后弃一张卡。
//!
//! > （3）此卡[移除]并向抽牌堆中加入一张“表演的本能”，将手牌与弃牌堆全部放入抽牌堆并洗切，然后抽2张卡。你本回合的移动以“CiRCLE”为起点（不触发起点地块效果）
//!
//! pick (2) burn crystals + band skill, or (3) shuffle back and redraw.

use alloc::vec::Vec;

use bandori_std::ctx;
use card_sdk::{it, CardText};
use card_sdk::{key, CardDef, Msg, On};

pub const MUTSUMI_NEVER: CardDef = CardDef::new(
    "CRYCHIC:（睦）从没有觉得...",
    &[On::Play("", None, mutsumi_never)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"从没有觉得..."#)
        .raw_name(r#"（睦）从没有觉得..."#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"打出此卡时，使用者可以选择（2）或（3）效果之一发动。"#),
            it(r#"消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的（2）效果，然后弃一张卡。"#),
            it(r#"此卡[移除]并向抽牌堆中加入一张“表演的本能”，将手牌与弃牌堆全部放入抽牌堆并洗切，然后抽2张卡。你本回合的移动以“CiRCLE”为起点（不触发起点地块效果）"#),
        ]))
        // sheet cell M12
        .fields(&[("group", r#"CRYCHIC"#), ("owner", r#"若叶睦（CRYCHIC）"#)]));

fn mutsumi_never(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「打出此卡时，使用者可以选择（2）或（3）效果之一发动。」 -- C#
    // `H.AskPick` between the two effects (default the last option). The sheet
    // grants the choice unconditionally; with no band skill attached a (2) pick
    // is still legal (0 crystals = 「不足3个则改为全部消耗」, then discard) and
    // only the band-skill invoke is skipped (`ctx::group_skill` is `None`).
    let options: Vec<Msg> = Vec::from([
        Msg::new(key!("mutsumi_never_opt2")).card("card", "CRYCHIC:（睦）从没有觉得..."),
        Msg::new(key!("mutsumi_never_opt3")).card("card", "CRYCHIC:（睦）从没有觉得..."),
    ]);
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("mutsumi_never_title")),
        &Msg::new(key!("mutsumi_never_ask")),
        &options,
    )?;
    if pick == 0 {
        branch_crystals(player_id)?;
    } else {
        branch_shuffle(player_id)?;
    }
    Ok(())
}

/// 规则书（2） -- spend up to 3 band crystals, run the band skill, discard a card.
fn branch_crystals(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗）」
    let n = ctx::group_crystals(player_id).min(3);
    if n > 0 {
        ctx::add_group_crystals(player_id, -n, 0);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("mutsumi_never_crystals"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    // 规则书（2）: 「立即执行乐队技能的（2）效果」 -- C# `BandCrychic.TransformNow()`.
    // A band skill exposes its numbered active effect as `On::Play` (the press),
    // so the invoke is `ctx::invoke_skill` on the bound band skill
    // (`ctx::group_skill` -- C# `H._fx[i].groups`). The CRYCHIC band skill's (2)
    // is exactly that entry (`transform_now`), and it re-checks its own
    // 「若你的抽牌堆与弃牌堆中都没有卡」 gate before doing anything.
    if let Some(skill) = ctx::group_skill(player_id) {
        ctx::invoke_skill(player_id, &skill)?;
    }
    // 规则书（2）: 「然后弃一张卡」 -- C# `H.AskCard(i, ..., H._hidden[i].hand.ToList())`.
    let hand = ctx::cards_in(player_id, ctx::CardPile::Hand);
    // TODO(规则书)（然后弃一张卡）: the discard is unconditional -- we skip it when
    //   the hand is empty.
    if !hand.is_empty() {
        let ids: Vec<&str> = hand.iter().map(|c| c.as_str()).collect();
        let pick = ctx::ask_card(
            player_id,
            &Msg::new(key!("mutsumi_never_discard_title")),
            &Msg::new(key!("mutsumi_never_discard_ask")),
            &ids,
        )?;
        ctx::discard_from_hand(player_id, ids[pick.min(ids.len() - 1)]);
    }
    // 规则书（2）: 「消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的（2）效果，然后弃一张卡。」 -- the cell
    // prints no fate for the played card.
    // `Dest::Banished` is the out-of-game fate (C# `c.Dest = "gone"`) and it
    // does post the [移除] log from the play pipeline, even though (2) prints
    // no [移除] (only (3) does).
    // TODO(规则书)（然后弃一张卡）: (2) prints no [移除] for the played card -- we
    //   banish it, which logs an unprinted 「被 [移除]」.
    ctx::set_dest(ctx::Dest::Banished);
    Ok(())
}

/// 规则书（3） -- remove this card, shuffle everything back, draw 2 from CiRCLE.
fn branch_shuffle(player_id: i32) -> card_sdk::Asked {
    // 规则书（3）: 「此卡[移除]」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书（3）: 「并向抽牌堆中加入一张“表演的本能”」 -- C#
    // `H.AddToDeck(i, "Mujica:（睦/mortis）表演的本能")`.
    ctx::add_to_dest(
        player_id,
        "Mujica:（睦/mortis）表演的本能",
        ctx::Dest::DeckRandom,
    );
    ctx::sweep_to_deck(player_id); // 规则书（3）: 「将手牌与弃牌堆全部放入抽牌堆并洗切」
    ctx::log(
        player_id,
        &Msg::new(key!("mutsumi_never_added")).player_id("who", player_id),
    );
    // 规则书（3）: 「然后抽2张卡」
    ctx::draw(player_id, 2)?;
    // 规则书: （3） 「你本回合的移动以“CiRCLE”为起点（不触发起点地块效果）」
    //   -- C# `H._turnCtx.Plan.Start = 0; Plan.StartWhy = CardName` =
    //   `plan::set_start(0, ...)`. CiRCLE is tile 0.
    ctx::plan::set_start(0, "（睦）从没有觉得...");
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn mutsumi_never_kani(player_id: i32) -> card_sdk::Asked {
    mutsumi_never(player_id)
}
