//! `HHW:梦幻的回礼` -- C# `CardDreamReturn` (MatchHost.cs:4462-4523): [反击] an
//! incoming rent, pay double rent at one rival tile to subsidise the payer.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:梦幻的回礼`）:
//! > 梦幻的回礼：[反击] 在场上其他玩家即将被不属于你的格子收费时打出，向场上你以外的任意一名玩家的一个格子进行一次支付。若成功进行支付，则自动使用一次你的乐队技能进行双倍支付并为乐队技能卡上添加两个分别记录这两名玩家的奇迹水晶，并为将要进行支付的那名玩家减免相当于你支付金额的数额。
//!

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const DREAM_RETURN: CardDef = CardDef {
    id: "HHW:梦幻的回礼",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `CardDreamReturn.Targets` -- tiles you may pay at: owned by another living
/// player, unmortgaged, and with a positive rent.
fn targets(seat: i32) -> Vec<i32> {
    let mut v: Vec<i32> = Vec::new();
    for t in 0..ctx::tile_count() {
        let owner = ctx::tile_owner(t);
        if owner < 0 || owner == seat || ctx::seat_out(owner) {
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

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「在场上其他玩家即将被不属于你的格子收费时打出」 -- C#
    // `CanReact`: `t.Kind == "pay" && t.Pay != null && t.Pay.IsRent &&
    // t.Pay.from >= 0 && t.Pay.from != seat && t.Pay.to != seat &&
    // Targets(seat).Count > 0 && H.CanPay(seat)`.
    if trigger::kind() != TriggerKind::Pay {
        return false;
    }
    // On pay triggers `t.Seat == t.Pay.from` and `t.Target == t.Pay.to`.
    let from = trigger::seat();
    let to = trigger::target();
    if from < 0 || from == seat || to == seat {
        return false;
    }
    // TODO(ABI): `t.Pay.IsRent` (`t.Pay.kind == "rent"`) -- the pay trigger does
    // not carry the pay kind, so every other-to-other pay matches. Approximate
    // conservatively: the react body still only pays a real rent of a target tile.
    // 规则书[反击]: 「向场上你以外的任意一名玩家的一个格子进行一次支付」 -- there
    // must be such a tile (C# `Targets(seat).Count > 0`).
    if targets(seat).is_empty() {
        return false;
    }
    // C# `H.CanPay(seat)`.
    ctx::can_pay(seat)
}

fn react(seat: i32) {
    let mut list = targets(seat);
    if list.is_empty() {
        return;
    }
    // C# `H.AskTileOf` defaults to the cheapest rent among the targets.
    list.sort_by_key(|&t| ctx::rent_of(t));
    // 规则书[反击]: 「向场上你以外的任意一名玩家的一个格子进行一次支付」
    let to_tile = ctx::ask_tile(
        seat,
        &Msg::new(key!("dream_return_title")),
        &Msg::new(key!("dream_return_ask")),
        &list,
    );
    let owner = ctx::tile_owner(to_tile);
    if owner < 0 || owner == seat {
        return;
    }
    // 规则书[反击]: 「若成功进行支付，则自动使用一次你的乐队技能进行双倍支付」 -- C#
    // pays `H.RentOf(num) * 2` to the tile owner, source
    // 「（传播笑容：双倍）」 (the band skill 「传播笑容」 auto-doubles).
    let amount = ctx::rent_of(to_tile) * 2;
    let paid = ctx::transfer(seat, owner, amount, &Msg::new(key!("dream_return_pay")));
    // C# `if (p.paid)` runs the bookkeeping after `H.Money`.
    if paid <= 0 {
        return;
    }
    // 规则书[反击]: 「并为乐队技能卡上添加两个...奇迹水晶」 -- C#
    // `H.AddBandCrystals(i, 2, CardName + "：记录 ...")`.
    ctx::add_band_crystals(seat, 2, i32::MAX);
    // TODO(规则书): 「分别记录这两名玩家」 -- needs the band-skill recorded-seat
    // set (C# `BandHHW.Record(owner)` + `Record(pay.from)`). `add_band_crystals`
    // only raises the crystal count, and the record's payoff (`BandHHW.PayAfter`
    // / `BuildCost`) is itself an Fx hook.
    // 规则书[反击]: 「并为将要进行支付的那名玩家减免相当于你支付金额的数额」 -- C#
    // `pay.amount = Math.Max(0, pay.amount - p.finalLoss)` on the in-flight pay.
    // TODO(规则书): mutating the react-to pay amount needs a trigger-payload
    // write (C# `Trigger.Pay.amount`); `trigger::value` is read-only.
}