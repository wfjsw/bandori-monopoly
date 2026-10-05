//! `PP:再次闪耀` -- C# `CardShineAgain` (MatchHost.cs:7145-7243): stay in play;
//! while short on cash the owner may record a colour and take a rescue.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:再次闪耀`）:
//! > 再次闪耀：
//! > [特]：
//! > 此卡不受除拥有此卡的玩家以外的玩家的效果影响。
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]。
//! > [持续]：
//! >
//! > （1）[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡未记录且[拥有者]拥有同色地契的颜色，[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”÷5的资金和1个正面[P✽P粉丝]。
//! >
//! > （2）[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面[P✽P粉丝]。
//!
//! The [持续] rescue and the [共鸣] action need hooks the ABI lacks (below).

use card_sdk::{ctx, key, CardDef, Msg};

pub const SHINE_AGAIN: CardDef = CardDef {
    id: "PP:再次闪耀",
    play: Some(shine_again),
    can_react: None,
    react: None,
    why_not: None,
};

fn shine_again(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:再次闪耀", &Msg::new(key!("shine_again_note")));
    // TODO(规则书): [特]「此卡不受除拥有此卡的玩家以外的玩家的效果影响」 -- needs the
    // C# `Card.Immune` flag so other players' effects skip this field card
    // (`public override bool Immune => true`).
    // TODO(规则书): [持续]（1）「[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡
    // 未记录且[拥有者]拥有同色地契的颜色，[拥有者][获得]“[拥有者]拥有的1个同色地契的购买价格”
    // ÷5的资金和1个正面[P✽P粉丝]」 -- needs the Fx.PayChoose hook (C#
    // `Card.PayChoose(PayCtx)`, fires when `p.from == Seat && p.amount > 0 &&
    // Me.money < p.amount`) and per-card Mem state for the recorded colours
    // (C# `HashSet<int> _colors`). The rescue itself is `H.GainFans(Seat, 1,
    // up: true, ...)` plus `H.GainR(Seat, maxPrice / 5, ...)` over the colour.
    // TODO(规则书): [持续]（2）「[共鸣]记录一个此卡未记录的颜色，[拥有者]获得1个正面
    // [P✽P粉丝]」 -- needs H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) and the
    // Fx.Actions hook (C# `Card.Actions` offering 「[共鸣] 再次闪耀」) plus the same
    // per-card colour set; the fan is `H.GainFans(Seat, 1, up: true, ...)`.
}