//!
//! 规则书（docs/rulebook/cards.json, id `AG:（巴）商店街的救世主`）:
//! > （巴）商店街的救世主：【反击】当其他玩家抵押商店街地契时，你可以打出此卡，立刻支付常规收购价一半的价格从该玩家处收购该地契。当你抵押商店街地契时，你可以打出此卡，额外获得一份抵押收益并将地契翻回
//!
//! [反击] on a shop-street mortgage: buy it cheap, or double your own mortgage.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const TOMOE_SAVIOR: CardDef = CardDef::new(
    "AG:（巴）商店街的救世主",
    // G4: kind (Mortgage) is the category. The money threshold moves to the
    // condition (`owner.money >= tile.price / 2` on the non-self arm); the
    // shop-street check is a derived predicate (GUARDS.md §6) and stays in the
    // residual guard.
    // 规则书[反击]: 「当其他玩家抵押商店街地契时，你可以打出此卡」
    &[On::Counteract(
        &[ChainKind::Mortgage],
        // 规则书[反击]: 「立刻支付常规收购价一半的价格」
        "actor == owner || owner.money >= tile.price / 2",
        Some(can_counteract),
        counteract).counter_reason(key!("tomoe_savior_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"商店街的救世主"#)
        .raw_name(r#"（巴）商店街的救世主"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"【反击】当其他玩家抵押商店街地契时，你可以打出此卡，立刻支付常规收购价一半的价格从该玩家处收购该地契。当你抵押商店街地契时，你可以打出此卡，额外获得一份抵押收益并将地契翻回"#))
        // sheet cell C14
        .fields(&[("group", r#"Afterglow"#), ("owner", r#"宇田川巴"#)]));

fn is_shop(t: i32) -> bool {
    t >= 0 && ctx::is_shop(t)
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「商店街地契」 -- the shop-street check is a derived
    // predicate (GUARDS.md §6) and stays here. The money threshold is the
    // condition (`owner.money >= tile.price / 2`).
    let _ = player_id;
    is_shop(trigger::tile())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let t = trigger::tile();
    let from = trigger::player_id();
    if from == player_id {
        // TODO(规则书)（额外获得一份抵押收益并将地契翻回）: 规则书[反击]: 「当你抵押商店街地契时，你可以打出此卡」 -- the self arm.
        if ctx::tile_owner(t) != player_id || !ctx::mortgaged_of(t) {
            return Ok(());
        }
        let v = ctx::mortgage_value(t);
        if v > 0 {
            ctx::gain(player_id, v, &Msg::new(key!("tomoe_savior_reason")))?;
        }
        ctx::set_mortgaged(t, false);
        // TODO(规则书)[judgement]: 「额外获得一份抵押收益」 does not say whether the extra payout goes
        // through the gain pipeline or lands raw. Not a missing capability -- the clause
        // just does not decide. Flagging rather than guessing.
        return Ok(());
    }
    if ctx::tile_owner(t) != from {
        return Ok(());
    }
    // TODO(规则书) ruling 8: 「常规收购价一半」 -- the base (land / land + houses / 2× force) is
    // undecided.
    let price = ctx::tile_price(t) / 2;
    if price <= 0 {
        return Ok(());
    }
    // 规则书[反击]: 「从该玩家处收购该地契」 -- `ctx::acquire` (`docs/PURCHASE.md`)
    // is the 收购 pipeline: pay to `from`, then assign → `bought` → `buyAfter`.
    // HOST-06: `acquire` answers whether the deed actually changed hands. A
    // refused acquire (`BuyGate` / unpaid) must not log one. The cell calls it
    // 「收购」, not 「购买」, but `acquire` runs the purchase gate `buyGate` (a
    // 「不能购买」 refuses the 收购) and raises the 「购买…时」 listeners (C-Q4 4b).
    // The bought-back deed leaves the mortgage (C-Q4 4c): 「抵押」 in the cell is
    // the *trigger*, not the state the deed keeps.
    if ctx::acquire(player_id, from, t, price) {
        ctx::log(
            player_id,
            &Msg::new(key!("tomoe_savior_bought"))
                .tile("tile", t)
                .player_id("from", from)
                .player_id("who", player_id),
        );
    }
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
