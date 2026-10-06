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

use card_sdk::abi::{ChainKind, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "Mor:再次牵起手来";

pub const HOLD_HANDS_AGAIN: CardDef = CardDef::new(
    "Mor:再次牵起手来",
    &[
        On::CounterAct(&[ChainKind::Paid], can_react, react),
        On::Hook(&[HookKind::PayAt], |_| true, pay_at),
    ],
);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后」
    // C# `t.Kind == "paid" && t.Seat == H.Neighbor(seat, -1) && t.Seat != seat && t.Value > 0`.
    if trigger::kind() != TriggerKind::Paid {
        return false;
    }
    let payer = trigger::player_id();
    // C# takes the player-index neighbour (`H.Neighbor(seat, -1)`), not a true
    // action-order predecessor -- `ctx::neighbor` is that same hook.
    payer != player_id && payer == ctx::neighbor(player_id, -1) && trigger::value() > 0
}

fn react(player_id: i32) -> card_sdk::Asked {
    let amount = trigger::value();
    // 规则书[反击]: 「并[消耗]等量资金」 -- C# `H.LoseR(i, c.Trigger.Value, CardName)`.
    ctx::pay(
        player_id,
        amount,
        &Msg::new(key!("hold_hands_again_why")).n("money", amount as i64),
    )?;
    if ctx::player_out(player_id) {
        return Ok(());
    }
    // 规则书[反击]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("hold_hands_again_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("hold_hands_again_placed"))
            .player_id("who", player_id)
            .n("money", amount as i64),
    );
    Ok(())
}

/// 规则书[持续]: 「[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区」 -- C#
/// `CardHoldHandsAgain.PayAt` cancels the next `PayCtx` with `p.from == Player` and
/// `H.Unplace(this, "discard", "用掉了")`. Runs through the Fx hook dispatch at
/// `payAt` (after `PayChoose`, before the `pay` [反击] window), so this is a
/// field effect, not a [反击].
fn pay_at(player_id: i32) -> card_sdk::Asked {
    if trigger::kind() != TriggerKind::PayAt
        || trigger::player_id() != player_id
        || !ctx::is_placed()
        || trigger::value() <= 0
    {
        return Ok(());
    }
    // 规则书[持续]: 「取消此次资金变动」
    trigger::set_pay_amount(0);
    // 规则书[持续]: 「将此卡放置到弃卡区」
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("hold_hands_again_used")).player_id("who", player_id),
    );
    Ok(())
}
