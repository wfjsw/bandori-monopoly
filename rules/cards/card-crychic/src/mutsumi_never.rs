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

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const MUTSUMI_NEVER: CardDef = CardDef::new(
    "CRYCHIC:（睦）从没有觉得...",
    &[On::Play(None, mutsumi_never)],
);

fn mutsumi_never(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「打出此卡时，使用者可以选择（2）或（3）效果之一发动。」 -- C#
    // `H.AskPick` between the two effects (default the last option).
    // C# only offers (2) when a real (non-Extra) `BandCrychic` skill is attached
    // (`H._fx[i].bands.Any(b => b is BandCrychic && !b.Extra)`); the skill
    // attachments are not in the vocabulary, so both options are offered and the
    // band-skill part of (2) is TODO'd below.
    let mut options: Vec<Msg> = Vec::new();
    options.push(Msg::new(key!("mutsumi_never_opt2")));
    options.push(Msg::new(key!("mutsumi_never_opt3")));
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
    let n = ctx::band_crystals(player_id).min(3);
    if n > 0 {
        ctx::add_band_crystals(player_id, -n, 0);
    }
    ctx::log(
        player_id,
        &Msg::new(key!("mutsumi_never_crystals"))
            .player_id("who", player_id)
            .i("n", n as i64),
    );
    // TODO(ABI): （2） 「立即执行乐队技能的（2）效果」 -- needs the band-skill
    //   attachment surface (C# `BandCrychic.TransformNow()`, the CRYCHIC band
    //   skill (2) that swaps in a new band).
    // 规则书（2）: 「然后弃一张卡」 -- C# `H.AskCard(i, ..., H._hidden[i].hand.ToList())`.
    let hand = ctx::cards_in(player_id, ctx::CardPile::Hand);
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
    // C# `c.Dest = "gone"` -- the card is consumed without the [移除] log; the
    // nearest `ctx::Dest` is `Removed` (same out-of-game destination).
    ctx::set_dest(ctx::Dest::Banished);
    Ok(())
}

/// 规则书（3） -- remove this card, shuffle everything back, draw 2 from CiRCLE.
fn branch_shuffle(player_id: i32) -> card_sdk::Asked {
    // 规则书（3）: 「此卡[移除]」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书（3）: 「并向抽牌堆中加入一张“表演的本能”」 -- C#
    // `H.AddToDeck(i, "Mujica:（睦/mortis）表演的本能")`.
    ctx::add_to_deck(player_id, "Mujica:（睦/mortis）表演的本能", true);
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
