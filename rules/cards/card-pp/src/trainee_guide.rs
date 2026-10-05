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
//! The crystal counter and the turn-end tick are live; the mark bookkeeping
//! and the cash-in still need hooks the ABI lacks.

use card_sdk::abi::HookKind;
use card_sdk::ctx;
use card_sdk::{key, CardDef, On, Msg};

pub const TRAINEE_GUIDE: CardDef = CardDef::new("PP:练习生解密指南", &[
    On::Play(trainee_guide),
    On::Hook(&[HookKind::TurnEnd], turn_end),
]);

fn trainee_guide(player_id: i32) {
    // 规则书[手]: 「为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    ctx::add_band_crystals(player_id, 3, i32::MAX);
    // 规则书[手]: 「并将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, "PP:练习生解密指南", &Msg::new(key!("trainee_guide_note")));
    // 规则书[手]: 「在此卡上放置“粉丝数量”÷3个[奇迹水晶]」 -- C#
    // `H.PlaceFromPlay(c, -1, -1, H.Fans(i) / 3)`.
    let fans = ctx::tok(player_id, "P✽P粉丝(正)") + ctx::tok(player_id, "P✽P粉丝(反)");
    ctx::set_crystals(player_id, fans / 3);
    // TODO(规则书)[手]: 「然后公开[使用者]的抽卡区并根据公开卡中的颜色数量添加一个单色/
    // 双色标记」 -- needs H.RevealSeen (C# `H.RevealSeen(i, deck)` shows the draw
    // pile), card-band lookup for the colour count (C# `H.Db.Card(id)?.band`),
    // and per-card Mem for the mono/dual marks (C# `Mem["mono"]` / `Mem["dual"]`,
    // kept in per-card Mem / player slots).
    // One distinct colour -> 1 mono mark; two or more -> 1 dual mark.
    // TODO(规则书)[手]: 「如果[共鸣]则[消耗]500资金并添加任意2个标记」 -- needs
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) plus a non-mandatory pay
    // (`PayCtx { amount = 500, kind = "lose", must = false }`) and an AskPick loop
    // that adds 2 more mono/dual marks to this card's Mem.
    // TODO(规则书): [持续]（1）「手卡上限数量减1」 -- needs the Fx.HandLimitDelta hook
    // (C# `Card.HandLimitDelta` returning -1 for the owner).
}

/// C# `CardTraineeGuide.TurnEnd` -> `Tick`: +1 crystal, then the 5-crystal cash-in.
fn turn_end(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    // 规则书[持续]（2）: 「回合结束时添加1个[奇迹水晶]」 -- C# `AddCrystals(1, "回合结束")`.
    ctx::add_crystals(player_id, 1, 0);
    // TODO(规则书): [持续]（3）「此卡拥有至少5个[奇迹水晶]时根据此卡上的标记进行一下操作
    // 随后进入弃卡区：1. 每个单色效果为获得1层状态“下次盖房的价格减少1000（可溢出），
    // 盖房后减少1层”；2. 每个双色效果为将自己的所有反面[P✽P粉丝]变正」 -- the
    // crystal check (`ctx::crystals(player_id) >= 5`) and the fan flip
    // (`add_tok(FANS_DOWN, -down)` + `add_tok(FANS_UP, down)` per dual mark) are
    // expressible, but the mono marks still need the H.ExtraOf BuildDiscountFx
    // attachment (C# `H.ExtraOf<BuildDiscountFx>(Seat).Add(1000, CardName)` per
    // mono mark) and unplace-to-discard (`H.Unplace(this, "discard", "结算完了")`)
    // is `unplace_card` + `to_discard` -- close it together with the marks above.
}