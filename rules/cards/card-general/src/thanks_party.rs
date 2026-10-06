//! `通用:CiRCLE THANKS PARTY!` -- C# `CardThanksParty`: others may chip in 500,
//!
//! 规则书（docs/rulebook/cards.json, id `通用:CiRCLE THANKS PARTY!`）:
//! > CiRCLE THANKS PARTY!：
//! > [手]：所有其他玩家可选择[消耗]500资金，你消耗500资金，将X设为因此卡[消耗]资金的玩家数量加1。根据X进行以下操作之一：
//! > 1. X至少为2则[使用者]投掷Xd20，如果结果大于35则[使用者][获得]3000资金且其他因此卡[消耗]资金的玩家[获得]1500资金；
//! > 2. X等于1则[使用者]的本回合结束后获得一个额外回合。
//!
//! then Xd20 > 35 pays out; alone, an extra turn.

use alloc::vec::Vec;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const THANKS_PARTY: CardDef =
    CardDef::new("通用:CiRCLE THANKS PARTY!", &[On::Play(None, thanks_party)]);

fn thanks_party(player_id: i32) -> card_sdk::Asked {
    let why = Msg::new(key!("thanks_party_why"));
    // 规则书[手]: 「所有其他玩家可选择[消耗]500资金」
    let mut payers: Vec<i32> = Vec::new();
    for p in ctx::others(player_id) {
        // Only players who can actually put 500 on the table are asked (C#
        // `where H.CanPay(num2) && H.State.seats[num2].money >= 500`).
        if !ctx::can_pay(p) || ctx::money_of(p) < 500 {
            continue;
        }
        let title = Msg::new(key!("thanks_party_join_title"));
        let text = Msg::new(key!("thanks_party_join_text"))
            .player_id("who", player_id)
            .n("prize", 1500);
        if ctx::ask_yes(p, &title, &text)? {
            // 规则书[手]: 「[消耗]500资金」
            if ctx::pay(p, 500, &why)? > 0 {
                payers.push(p);
            }
        }
    }
    // 规则书[手]: 「你消耗500资金」
    ctx::pay(player_id, 500, &why)?;
    // 规则书[手]: 「将X设为因此卡[消耗]资金的玩家数量加1」 -- the joining others
    // plus the user (C# `payers.Count + 1`).
    let x = payers.len() as i32 + 1;
    if x == 1 {
        // 规则书[手]: 「2. X等于1则[使用者]的本回合结束后获得一个额外回合」
        ctx::give_extra_turn(player_id);
        ctx::log(
            player_id,
            &Msg::new(key!("thanks_party_extra")).player_id("who", player_id),
        );
        return Ok(());
    }
    // 规则书[手]: 「1. X至少为2则[使用者]投掷Xd20」
    let r = ctx::roll(player_id, x, 20);
    if r <= 35 {
        ctx::log(
            player_id,
            &Msg::new(key!("thanks_party_miss")).i("roll", r as i64),
        );
        return Ok(());
    }
    // 规则书[手]: 「如果结果大于35则[使用者][获得]3000资金且其他因此卡[消耗]资金的玩家[获得]1500资金」
    ctx::gain(player_id, 3000, &why);
    for p in payers {
        if !ctx::player_out(p) {
            ctx::gain(p, 1500, &why);
        }
    }
    Ok(())
}
