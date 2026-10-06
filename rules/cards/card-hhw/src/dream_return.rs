//! `HHW:梦幻的回礼` -- C# `CardDreamReturn` (MatchHost.cs:4462-4523): [反击] an
//! incoming rent, pay double rent at one rival tile to subsidise the payer.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:梦幻的回礼`）:
//! > 梦幻的回礼：[反击] 在场上其他玩家即将被不属于你的格子收费时打出，向场上你以外的任意一名玩家的一个格子进行一次支付。若成功进行支付，则自动使用一次你的乐队技能进行双倍支付并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶，并为将要进行支付的那名玩家减免相当于你支付金额的数额。
//!

use alloc::vec::Vec;

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const DREAM_RETURN: CardDef = CardDef::new(
    "HHW:梦幻的回礼",
    &[On::CounterAct(&[ChainKind::Effect], can_react, react)],
);

/// C# `CardDreamReturn.Targets` -- tiles you may pay at: owned by another living
/// player, unmortgaged, and with a positive rent.
fn targets(player_id: i32) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        let owner = ctx::tile_owner(t);
        if owner < 0 || owner == player_id || ctx::player_out(owner) {
            continue;
        }
        // 规则书: `!H.State.mortgaged[t]` -- mortgaged tiles are not targets.
        if ctx::mortgaged_of(t) {
            continue;
        }
        if ctx::rent_of(t) > 0 {
            v.push(t);
        }
    }
    v
}

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「在场上其他玩家即将被不属于你的格子收费时打出」 -- C#
    // `CanReact`: `t.Kind == "pay" && t.Pay != null && t.Pay.IsRent &&
    // t.Pay.from >= 0 && t.Pay.from != player && t.Pay.to != player &&
    // Targets(player).Count > 0 && H.CanPay(player)`.
    if trigger::kind() != ChainKind::Effect {
        return false;
    }
    // On pay triggers `t.Seat == t.Pay.from` and `t.Target == t.Pay.to`.
    let from = trigger::player_id();
    let to = trigger::target();
    if from < 0 || from == player_id || to == player_id {
        return false;
    }
    if !trigger::pay_is_rent() {
        return false;
    }
    // 规则书[反击]: 「向场上你以外的任意一名玩家的一个格子进行一次支付」 -- there
    // must be such a tile (C# `Targets(player_id).Count > 0`).
    if targets(player_id).is_empty() {
        return false;
    }
    // C# `H.CanPay(seat)`.
    ctx::can_pay(player_id)
}

fn react(player_id: i32) -> card_sdk::Asked {
    let mut list = targets(player_id);
    if list.is_empty() {
        return Ok(());
    }
    // C# `H.AskTileOf` defaults to the cheapest rent among the targets.
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
    // 规则书[反击]: 「若成功进行支付，则自动使用一次你的乐队技能进行双倍支付」 -- C#
    // pays `H.RentOf(num) * 2` to the tile owner, source
    // 「（传播笑容：双倍）」 (the band skill 「传播笑容」 auto-doubles).
    let amount = ctx::rent_of(to_tile) * 2;
    let paid = ctx::transfer(
        player_id,
        owner,
        amount,
        &Msg::new(key!("dream_return_pay")),
    )?;
    // C# `if (p.paid)` runs the bookkeeping after `H.Money`.
    if paid <= 0 {
        return Ok(());
    }
    // 规则书[反击]: 「并为乐队技能卡上添加两个...奇迹水晶」 -- C#
    // `H.AddBandCrystals(i, 2, CardName + "：记录 ...")`.
    ctx::add_band_crystals(player_id, 2, i32::MAX);
    // 「分别记录这两名玩家」 -- the band skill 「传播笑容」 keeps the record in
    // `skill.hhw.who` / `skill.hhw.who2`; this writes both.
    ctx::state::set(player_id, "skill.hhw.who", owner);
    ctx::state::set(player_id, "skill.hhw.who2", trigger::player_id());
    // 规则书[反击]: 「并为将要进行支付的那名玩家减免相当于你支付金额的数额」 -- C#
    // `pay.amount = Math.Max(0, pay.amount - p.finalLoss)` on the in-flight pay
    // (`trigger::set_pay_amount` is that write).
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
