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
//! Counteraction-only (`Normal => false`).

use card_sdk::abi::{ChainKind, ActionKind, FilterKind};
use bandori_std::ctx::{self, trigger};
use card_sdk::{Action, CardDef, Filter, key, Msg, On};

const ID: &str = "Mor:再次牵起手来";

pub const HOLD_HANDS_AGAIN: CardDef = CardDef::new(
    "Mor:再次牵起手来",
    &[
        On::Counteract(
            &[ChainKind::Paid],
            "actor != owner && actor == neighbor(owner, -1) && value > 0", Some(key!("hold_hands_again_when")),
            None,
            counteract,
        ),
        On::Filter(Filter::PaySplitAt("actor == owner && card.placed && value > 0", None, pay_at)),
        On::Action(Action::PayAfter("actor == owner && card.placed", None, None, pay_after)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"再次牵起手来"#)
        .raw_name(r#"再次牵起手来"#)
        .tags(&[r#"手"#, r#"反击"#, r#"持续"#])
        .text(r#"[手]：
[反击]当[使用者]的行动序列前一名玩家[消耗]或[支付]大于0资金后将此卡放置在[使用者]的[场地]并[消耗]等量资金。
[持续]：
[消耗]或[支付]时取消此次资金变动并将此卡放置到弃卡区。"#)
        .cell(r#"G12"#)
        .fields(&[("group", r#"Morfonica"#), ("owner", "")]));

fn counteract(player_id: i32) -> card_sdk::Asked {
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
/// `CardHoldHandsAgain.PaySplitAt` cancels the next `PayCtx` with `p.from == Player` and
/// `H.Unplace(this, "discard", "用掉了")`. Runs through the Fx hook dispatch at
/// `paySplitAt` (after `PaySplitChoose`, before the `pay` [反击] window), so this is a
/// field effect, not a [反击].
///
/// Split (ABI v74): the Filter returns 0 (「取消此次资金变动」) and logs; the
/// `set_dest(Graveyard)` half is `pay_after` on `PayAfter`.
fn pay_at(player_id: i32, _value: i32) -> i32 {
    // 规则书[持续]: 「取消此次资金变动」
    ctx::log(
        player_id,
        &Msg::new(key!("hold_hands_again_used")).player_id("who", player_id),
    );
    0
}

/// 规则书[持续]: 「将此卡放置到弃卡区」 -- the Filter half already logged.
fn pay_after(_player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_at_kani(player_id: i32, value: i32) -> i32 {
    pay_at(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn pay_after_kani(player_id: i32) -> card_sdk::Asked {
    pay_after(player_id)
}
