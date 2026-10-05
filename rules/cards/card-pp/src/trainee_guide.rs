//! `PP:练习生解密指南` -- C# `CardTraineeGuide` (MatchHost.cs:7870-7994): stay
//! in play with crystals and mono/dual marks; cash the marks in at 5 crystals.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:练习生解密指南`）:
//! > 练习生解密指南：
//! > [手]：
//! > 为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的[场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]，然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/双色标记，如果[共鸣]则[消耗]500资金并添加任意2个标记。
//! > [持续]：
//! >
//! > （1）手卡上限数量减1。
//! >
//! > （2）回合结束时添加1个[奇迹水晶]。
//! >
//! > （3）此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作随后进入弃卡区：
//! > 1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），盖房后减少1层”；
//! > 2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正。
//!
//! Only the band crystals and the placement are expressible; the rest needs
//! hooks the ABI lacks (below).

use card_sdk::{ctx, key, CardDef, Msg};

pub const TRAINEE_GUIDE: CardDef = CardDef {
    id: "PP:练习生解密指南",
    play: Some(trainee_guide),
    can_react: None,
    react: None,
    why_not: None,
};

fn trainee_guide(seat: i32) {
    // 规则书[手]: 「为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_band_crystals(seat, 3, i32::MAX);
    // 规则书[手]: 「并将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:练习生解密指南", &Msg::new(key!("trainee_guide_note")));
    // TODO(规则书)[手]: 「在此卡上放置“粉丝数量”÷3个[奇迹水晶]」 -- needs the
    // per-card crystal counter (C# `H.PlaceFromPlay(c, -1, -1, H.Fans(i) / 3)`).
    // The count is `(tok("P✽P粉丝(正)") + tok("P✽P粉丝(反)")) / 3`.
    // TODO(规则书)[手]: 「然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/
    // 双色标记」 -- needs H.RevealSeen (C# `H.RevealSeen(i, deck)` shows the draw
    // pile), card-band lookup for the colour count (C# `H.Db.Card(id)?.band`), and
    // per-card Mem for the mono/dual marks (C# `Mem["mono"]` / `Mem["dual"]`).
    // One distinct colour -> 1 mono mark; two or more -> 1 dual mark.
    // TODO(规则书)[手]: 「如果[共鸣]则[消耗]500资金并添加任意2个标记」 -- needs
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) plus a non-mandatory pay
    // (`PayCtx { amount = 500, kind = "lose", must = false }`) and an AskPick loop
    // that adds 2 more mono/dual marks to this card's Mem.
    // TODO(规则书): [持续]（1）「手卡上限数量减1」 -- needs the Fx.HandLimitDelta hook
    // (C# `Card.HandLimitDelta` returning -1 for the owner).
    // TODO(规则书): [持续]（2）「回合结束时添加1个[奇迹水晶]」 -- needs the Fx.TurnEnd
    // hook (C# `Card.TurnEnd` -> `AddCrystals(1, "回合结束")`) and the per-card
    // crystal counter.
    // TODO(规则书): [持续]（3）「此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作
    // 随后进入弃卡区：1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），
    // 盖房后减少1层”；2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正」 -- needs the
    // per-card crystal/Mem state, the H.ExtraOf BuildDiscountFx attachment (C#
    // `H.ExtraOf<BuildDiscountFx>(Seat).Add(1000, CardName)` per mono mark), the
    // face-down fan flip (`H.FlipUp(Seat, H.FansDown(Seat), CardName)` per dual
    // mark), and unplace-to-discard (`H.Unplace(this, "discard", "结算完了")`).
}