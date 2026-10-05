//! `Mor:再次牵起手来` -- C# `CardHoldHandsAgain` (MatchHost.cs:5108-5155): mirror the
//!
//! 规则书（docs/rulebook/cards.json, id `Mor:再次牵起手来`）:
//! > 再次牵起手来：
//! > [手]：
//! > [反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金。
//! > [持续]：
//! > [消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。
//!
//! previous player's payment, then cancel your next money change.
//!
//! Reaction-only (`Normal => false`).

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const HOLD_HANDS_AGAIN: CardDef = CardDef {
    id: "Mor:再次牵起手来",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后」
    // C# `t.Kind == "paid" && t.Seat == H.Neighbor(seat, -1) && t.Seat != seat && t.Value > 0`.
    if trigger::kind() != TriggerKind::Paid {
        return false;
    }
    let payer = trigger::seat();
    // C# takes the seat-index neighbour (`H.Neighbor(seat, -1)`), not a true
    // action-order predecessor -- `ctx::neighbor` is that same hook.
    payer != seat && payer == ctx::neighbor(seat, -1) && trigger::value() > 0
}

fn react(seat: i32) {
    let amount = trigger::value();
    // 规则书[反击]: 「并[消耗]等量资金」 -- C# `H.LoseR(i, c.Trigger.Value, CardName)`.
    ctx::pay(seat, amount, &Msg::new(key!("hold_hands_again_why")).n("money", amount as i64));
    if ctx::seat_out(seat) {
        return;
    }
    // 规则书[反击]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "Mor:再次牵起手来", &Msg::new(key!("hold_hands_again_note")));
    ctx::log(seat, &Msg::new(key!("hold_hands_again_placed")).seat("who", seat).n("money", amount as i64));
    // TODO(规则书)[持续]: 「[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」 -- needs
    // the Fx.PayAt hook (C# `CardHoldHandsAgain.PayAt` cancels the next `PayCtx` with
    // `p.from == Seat` and `H.Unplace(this, "discard", "用掉了")`).
}