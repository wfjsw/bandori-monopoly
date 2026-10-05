//! `通用:登上武道馆` -- C# `CardBudokan`: every other living player pays you X.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:登上武道馆`）:
//! > 登上武道馆：
//! > [手]：
//! > 将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，被[指定]的玩家[支付][使用者]X资金。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const BUDOKAN: CardDef = CardDef {
    id: "通用:登上武道馆",
    play: Some(budokan),
    can_react: None,
    react: None,
    why_not: None,
};

fn budokan(seat: i32) {
    // 规则书[手]: 「[使用者]以外的[存活]玩家」 (`ctx::others` drops out seats)
    let list = ctx::others(seat);
    let n = list.len() as i32;
    if n == 0 {
        return;
    }
    // 规则书[手]: 「将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10」
    let x = ((2000 + 10 * n - 1) / (10 * n)) * 10;
    // 规则书[手]: 「被[指定]的玩家[支付][使用者]X资金」
    let why = Msg::new(key!("budokan_why"));
    for p in list {
        ctx::transfer(p, seat, x, &why);
    }
    // 规则书[手]: 「Y设为[存活]玩家数量减1」 -- Y is defined but unused in the passage.
    // TODO(规则书): 「[指定][使用者]以外的所有玩家」 -- needs the `H.TargetAll` /
    // `H.Target` targeting pipeline (the [指定] designation: immunity tags,
    // redirect, the `target` reaction window) and the `Targeting` flag on
    // `CardDef`, so counters like 「网络链接异常」 can intercept a designation.
}