
//!
//! 规则书（docs/rulebook/cards.json, id `通用:登上武道馆`）:
//! > 登上武道馆：
//! > [手]：
//! > [使用者]以外的所有[存活]玩家[分摊]向[使用者][支付]2000资金；无法付款的玩家（包括处于[除外]状态的玩家）仍计入分摊，其付款条目在分摊后取消，其他玩家的份额不变。若没有其他[存活]玩家，则[使用者][获得]2000资金。
//!
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const BUDOKAN: CardDef = CardDef::new("通用:登上武道馆", &[On::Play("", None, budokan)])
    .props(&[(card_sdk::abi::prop::DESIGNATES, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"登上武道馆"#)
        .raw_name(r#"登上武道馆"#)
        .tags(&[r#"手"#])
        .text(CardText::of(&[sec(r#"[手]："#, &[para(r#"[使用者]以外的所有[存活]玩家[分摊]向[使用者][支付]2000资金；无法付款的玩家（包括处于[除外]状态的玩家）仍计入分摊，其付款条目在分摊后取消，其他玩家的份额不变。若没有其他[存活]玩家，则[使用者][获得]2000资金。"#)])]))
        // sheet cell A3
        .fields(&[("group", r#"通用"#), ("owner", "")]));

fn budokan(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「[使用者]以外的所有[存活]玩家[分摊]向[使用者][支付]2000资金」
    // `ctx::others` is exactly the alive others: bankrupt / departed seats
    // drop out, while [除外] seats stay in.
    let list = ctx::others(player_id);
    let reason = Msg::new(key!("budokan_reason"));
    // 规则书[手]: 「若没有其他[存活]玩家，则[使用者][获得]2000资金」 -- an
    // ordinary print, not a [分摊].
    if list.is_empty() {
        ctx::gain(player_id, 2000, &reason)?;
        return Ok(());
    }
    // 「[分摊]」 payers are designations: the full targeting pipeline runs (out /
    // Untargetable / the `target` [反击] window), and only the players actually
    // hit get a leg.
    let got = ctx::target_all(&list);
    // 「无法付款的玩家（包括处于[除外]状态的玩家）仍计入分摊，其付款条目在
    // 分摊后取消，其他玩家的份额不变」 -- the divisor is every alive other,
    // cancelled designation or not; only the entry itself drops.
    let n = list.len() as i32;
    // The command-wide pre-split stage shapes the one shared 2000 before the
    // division (ceil10 of the share).
    let Some(shaped) = ctx::pay_total(list[0], player_id, 2000, &reason)? else {
        return Ok(());
    };
    let share = ((shaped + n - 1) / n + 9) / 10 * 10;
    for p in got {
        if ctx::designation_cancelled(p) {
            continue;
        }
        ctx::pay_leg(p, player_id, share, &reason)?;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn budokan_kani(player_id: i32) -> card_sdk::Asked {
    budokan(player_id)
}
