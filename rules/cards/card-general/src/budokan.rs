//! `通用:登上武道馆` -- C# `CardBudokan`: every other living player pays you X.
//!
//! 规则书（docs/rulebook/cards.json, id `通用:登上武道馆`）:
//! > 登上武道馆：
//! > [手]：
//! > 将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10，Y设为[存活]玩家数量减1。[指定][使用者]以外的所有玩家，被[指定]的玩家[支付][使用者]X资金。
//!

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BUDOKAN: CardDef = CardDef::new("通用:登上武道馆", &[On::Play(None, budokan)])
    // 规则书: 「[指定][使用者]以外的所有玩家」 -- `Card.Def.Targeting`, so
    // 网络链接异常 「取消其对目标之一的[指定]」 sees the designations.
    .props(&[(card_sdk::abi::prop::DESIGNATES, 1)]);

fn budokan(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「[使用者]以外的[存活]玩家」 (`ctx::others` drops out players)
    let list = ctx::others(player_id);
    let n = list.len() as i32;
    if n == 0 {
        return Ok(());
    }
    // 规则书[手]: 「将X设为2000÷“[使用者]以外的[存活]玩家数量”向上取整10」
    // (the divisor is the candidate count, before the targeting gate).
    let x = ((2000 + 10 * n - 1) / (10 * n)) * 10;
    // 规则书[手]: 「[指定][使用者]以外的所有玩家」 -- C# `H.TargetAll(c, list, got)`:
    // the full targeting pipeline (out / exile / ImmuneAll / Untargetable /
    // the `target` [反击] window; no redirect on `TargetAll`), keeping only the
    // players actually hit.
    let got = ctx::target_all(&list);
    // 规则书[手]: 「被[指定]的玩家[支付][使用者]X资金」
    let why = Msg::new(key!("budokan_why"));
    for p in got {
        ctx::transfer(p, player_id, x, &why)?;
    }
    // 规则书[手]: 「Y设为[存活]玩家数量减1」 -- Y is defined but unused in the passage.
    Ok(())
}
