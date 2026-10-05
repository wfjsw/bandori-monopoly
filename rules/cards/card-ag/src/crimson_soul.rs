//! `AG:绯红之魂` -- C# `CardCrimsonSoul` (MatchHost.cs:1583-1680):
//! place with N crystals; spend a crystal to cut a payment by 1,000.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:绯红之魂`）:
//! > 绯红之魂：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在这张卡上放置对应数量的[奇迹水晶]。
//! > [持续]：
//! >
//! > （1）
//! > [反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）。
//! >
//! > （2）[拥有者]使用自己原有的技能
//! > （2）时移除此卡的1个[奇迹水晶]。
//! >
//! > （3）此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const CRIMSON_SOUL: CardDef = CardDef {
    id: "AG:绯红之魂",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardCrimsonSoul.WhyNot`: refuses under 500.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::money(seat) >= 500 {
        return None;
    }
    Some(Msg::new(key!("crimson_soul_no_money")))
}

fn play(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」 -- C# `H.PlaceFromPlay(c, -1, -1, num)`.
    ctx::set_dest(ctx::Dest::Field);
    // 规则书[手]: 「选择[消耗]1到5次500资金」 -- C# `H.AskNumber(i, ..., 1, Math.Max(1, min(5, money/500)))`.
    let max = (ctx::money(seat) / 500).min(5);
    let n = ctx::ask_number(
        seat,
        &Msg::new(key!("crimson_soul_title")),
        &Msg::new(key!("crimson_soul_ask")),
        1,
        max.max(1),
    );
    // C# `n = Math.Max(1, Math.Min(max, r.value))`.
    let n = n.min(max).max(1);
    // 规则书[手]: 「[消耗]1到5次500资金」 -- C# `PayCtx { kind: "lose", must: false, amount = 500 * n }`.
    let paid = ctx::pay(seat, 500 * n, &Msg::new(key!("crimson_soul_why")));
    let _ = paid;
    ctx::place_card(seat, "AG:绯红之魂", &Msg::new(key!("crimson_soul_note")));
    ctx::log(seat, &Msg::new(key!("crimson_soul_placed")).seat("who", seat));
    // TODO(规则书[手]): 「并在这张卡上放置对应数量的[奇迹水晶]」 -- the placement's
    // crystal charge needs card_crystals on a placed card (C#
    // `H.PlaceFromPlay(c, -1, -1, num)` with `num = p.paid ? n : 0`; 0 crystals
    // discards the card immediately via `H.Unplace`).
    // TODO(规则书[持续]（1）): 「[反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，此次[消耗]或[支付]金额减少1000…」
    // -- needs the Fx.PayChoose hook (C# `CardCrimsonSoul.PayChoose` / `Use`) plus
    // card crystals; the 500 to the payee (「若为[支付]则被[支付]玩家[获得]500资金」)
    // needs Fx.PayAfter (C# `CardCrimsonSoul.PayAfter`).
    // TODO(规则书[持续]（2）): 「[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]」
    // -- needs the Fx.SkillUsed hook (C# `CardCrimsonSoul.SkillUsed`) plus card
    // crystals.
    // TODO(规则书[持续]（3）): 「此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区」
    // -- needs the crystal count + unplace-to-discard (C# `Check` -> `H.Unplace`).
}