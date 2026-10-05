//! `PP:[丸山彩]憧憬的前方` -- C# `CardAyaLonging` (MatchHost.cs:7995-8063):
//! stay in play, take a card back from the discard, and shave payments while
//! poorest.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[丸山彩]憧憬的前方`）:
//! > [丸山彩]憧憬的前方：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]并将弃卡区中的一张卡加入手卡。
//! > [持续]：
//! >
//! > （1）如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100。
//! >
//! > （2）[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。
//!
//! The discard pick needs discard enumeration; the [持续] payment shaves need
//! the PayAdd / PayChoose hooks (below).

use card_sdk::{ctx, key, CardDef, Msg};

pub const AYA_LONGING: CardDef = CardDef {
    id: "PP:[丸山彩]憧憬的前方",
    play: Some(aya_longing),
    can_react: None,
    react: None,
    why_not: None,
};

fn aya_longing(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:[丸山彩]憧憬的前方", &Msg::new(key!("aya_longing_note")));
    // TODO(规则书)[手]: 「并将弃卡区中的一张卡加入手卡」 -- needs discard-pile
    // enumeration (C# `H._hidden[i].discard.Distinct()` + `H.AskCard` +
    // `H._hidden[i].discard.Remove` + `H.AddToHand`). `ctx::discard_count` /
    // `ctx::discard_size` exist for counts, but the discard still cannot be
    // listed (no `discard_at` / id enumeration), so `ask_card` has nothing to
    // offer.
    // TODO(规则书): [持续]（1）「如果[拥有者]的资金数是所有存活玩家中最少则[拥有者][消耗]
    // 或[支付]时将金额降低X（最低0）；X为100，如果[拥有者]拥有至少10个[P✽P粉丝]则X添加100」
    // -- needs the Fx.PayAdd hook (C# `Card.PayAdd(PayCtx)`, `p.amount = max(0,
    // p.amount - X)` while `Poorest`). X is 100, or 200 when
    // `tok("P✽P粉丝(正)") + tok("P✽P粉丝(反)") >= 10` (C# `CardAyaLonging.X`);
    // Poorest is every alive seat's `money >= money(owner)`
    // (C# `CardAyaLonging.Poorest`).
    // TODO(规则书): [持续]（2）「[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）」
    // -- needs the Fx.PayChoose hook (C# `Card.PayChoose(PayCtx)`) and
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) to offer the −1,500
    // (`p.amount = max(0, p.amount - 1500)`).
}