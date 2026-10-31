//!
//! 规则书（docs/rulebook/cards.json, id `HHW:梦幻的回礼`）:
//! > 梦幻的回礼：[反击] 在场上其他玩家即将被不属于你的格子收费时打出，向场上你以外的任意一名玩家的一个格子进行一次支付。若成功进行支付，则自动使用一次你的乐队技能进行双倍支付并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶，并为将要进行支付的那名玩家减免相当于你支付金额的数额。
//!
//! incoming rent, pay double rent at one rival tile to subsidise the payer.

use alloc::vec::Vec;

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const DREAM_RETURN: CardDef = CardDef::new(
    "HHW:梦幻的回礼",
    &[On::Counteract(
        &[ChainKind::Effect],
        // The rent-table `Targets` loop is engine logic.
        "pay_is_rent && actor >= 0 && actor != owner && target != owner",
        Some(can_counteract),
        counteract).counter_reason(key!("dream_return_when"))],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"梦幻的回礼"#)
        .raw_name(r#"梦幻的回礼"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 在场上其他玩家即将被不属于你的格子收费时打出，向场上你以外的任意一名玩家的一个格子进行一次支付。若成功进行支付，则自动使用一次你的乐队技能进行双倍支付并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶，并为将要进行支付的那名玩家减免相当于你支付金额的数额。"#))
        // sheet cell F16
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", "")]));

fn targets(player_id: i32) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        let owner = ctx::tile_owner(t);
        if owner < 0 || owner == player_id || ctx::player_out(owner) {
            continue;
        }
        // TODO(规则书)（向场上你以外的任意一名玩家的一个格子进行一次支付）: the book allows any tile -- we require it
        // unmortgaged and with positive rent.
        if ctx::mortgaged_of(t) {
            continue;
        }
        if ctx::rent_of(t) > 0 {
            v.push(t);
        }
    }
    v
}

fn can_counteract(player_id: i32) -> bool {
    !targets(player_id).is_empty() && rules_status::can_pay(player_id)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let mut list = targets(player_id);
    if list.is_empty() {
        return Ok(());
    }
    list.sort_by_key(|&t| ctx::rent_of(t));
    // 规则书[反击]: 「向场上你以外的任意一名玩家的一个格子进行一次支付」
    let to_tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("dream_return_title")),
        &Msg::new(key!("dream_return_ask")),
        &list,
    )?;
    let owner = ctx::tile_owner(to_tile);
    if owner < 0 || owner == player_id {
        return Ok(());
    }
    // TODO(规则书)（向场上你以外的任意一名玩家的一个格子进行一次支付）: The band skill 「传播笑容」 (1) is emulated by
    // hand here: a card `transfer` runs as `PayKind::Card`, not rent-shaped, so the
    // skill's hook cannot fire (ENG-CHARGE-RENT).
    let amount = ctx::rent_of(to_tile) * 2;
    let paid = ctx::transfer(
        player_id,
        owner,
        amount,
        &Msg::new(key!("dream_return_pay")),
    )?;
    if paid <= 0 {
        return Ok(());
    }
    // TODO(规则书)（并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶）: each crystal is supposed to record a
    // player -- ours is one pool and either name spends any of its crystals
    // (OPEN-HHW-RETURN-BOOK). The record/crystals are also written only after
    // `transfer` returns, so the skill's (2) cannot consume them for this payment
    // and records a later outer value.
    ctx::add_group_crystals(player_id, 2, i32::MAX);
    // 「分别记录这两名玩家」 -- the band skill 「传播笑容」 keeps the record in
    // `skill.hhw.who` / `skill.hhw.who2`; this writes both.
    ctx::state::set(player_id, "skill.hhw.who", owner);
    ctx::state::set(player_id, "skill.hhw.who2", trigger::player_id());
    // TODO(规则书)（你支付金额）: could be the base or the doubled figure -- we use the amount
    // actually paid (OPEN-HHW-RETURN-RELIEF).
    let relief = paid.min(trigger::value()).max(0);
    let left = (trigger::value() - relief).max(0);
    trigger::set_pay_amount(left);
    ctx::log(
        player_id,
        &Msg::new(key!("dream_return_relief"))
            .player_id("who", trigger::player_id())
            .n("money", relief as i64),
    );
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
