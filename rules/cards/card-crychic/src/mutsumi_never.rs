//! `CRYCHIC:（睦）从没有觉得...` -- C# `CardMutsumiNever` (MatchHost.cs:3164-3216):
//! pick (2) burn crystals + band skill, or (3) shuffle back and redraw.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（睦）从没有觉得...`）:
//! > （睦）从没有觉得... ：
//! >
//! > （1）打出此卡时，使用者可以选择
//! > （2）或
//! > （3）效果之一发动。
//! >
//! > （2）消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗），立即执行乐队技能的
//! > （2）效果，然后弃一张卡。
//! >
//! > （3）此卡[移除]并向抽牌堆中加入一张“表演的本能”，将手牌与弃牌堆全部放入抽牌堆并洗切，然后抽2张卡。你本回合的移动以“CiRCLE”为起点（不触发起点地块效果）
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const MUTSUMI_NEVER: CardDef = CardDef {
    id: "CRYCHIC:（睦）从没有觉得...",
    play: Some(mutsumi_never),
    can_react: None,
    react: None,
    why_not: None,
};

fn mutsumi_never(seat: i32) {
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
        seat,
        &Msg::new(key!("mutsumi_never_title")),
        &Msg::new(key!("mutsumi_never_ask")),
        &options,
    );
    if pick == 0 {
        branch_crystals(seat);
    } else {
        branch_shuffle(seat);
    }
}

/// 规则书（2） -- spend up to 3 band crystals, run the band skill, discard a card.
fn branch_crystals(seat: i32) {
    // 规则书（2）: 「消耗乐队技能卡上的3个奇迹水晶（不足3个则改为全部消耗）」
    let n = ctx::band_crystals(seat).min(3);
    if n > 0 {
        ctx::add_band_crystals(seat, -n, 0);
    }
    ctx::log(seat, &Msg::new(key!("mutsumi_never_crystals")).seat("who", seat).i("n", n as i64));
    // TODO(ABI): （2） 「立即执行乐队技能的（2）效果」 -- needs the band-skill
    //   attachment surface (C# `BandCrychic.TransformNow()`, the CRYCHIC band
    //   skill (2) that swaps in a new band).
    // TODO(ABI): （2） 「然后弃一张卡」 -- needs hand enumeration so the player can
    //   name a card id (C# `H.AskCard(i, ..., H._hidden[i].hand.ToList())`).
    //   `ctx::discard_from_hand` is now host-bound, but there is still no
    //   hand-list query to build the `ask_card` option list from.
    // C# `c.Dest = "gone"` -- the card is consumed without the [移除] log; the
    // nearest `ctx::Dest` is `Removed` (same out-of-game destination).
    ctx::set_dest(ctx::Dest::Banished);
}

/// 规则书（3） -- remove this card, shuffle everything back, draw 2 from CiRCLE.
fn branch_shuffle(seat: i32) {
    // 规则书（3）: 「此卡[移除]」
    ctx::set_dest(ctx::Dest::Banished);
    // 规则书（3）: 「并向抽牌堆中加入一张“表演的本能”」 -- C#
    // `H.AddToDeck(i, "Mujica:（睦/mortis）表演的本能")`.
    ctx::add_to_deck(seat, "Mujica:（睦/mortis）表演的本能", true);
    ctx::sweep_to_deck(seat); // 规则书（3）: 「将手牌与弃牌堆全部放入抽牌堆并洗切」
    ctx::log(seat, &Msg::new(key!("mutsumi_never_added")).seat("who", seat));
    // 规则书（3）: 「然后抽2张卡」
    ctx::draw(seat, 2);
    // TODO(规则书): （3） 「你本回合的移动以“CiRCLE”为起点（不触发起点地块效果）」
    //   -- needs the main-move plan override (C# `H._turnCtx.Plan.Start = 0` /
    //   `StartWhy`, and the "skip the start-tile effect" flag on that move).
}