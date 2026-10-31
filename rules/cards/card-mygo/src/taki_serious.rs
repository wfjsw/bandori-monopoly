//! `MyGO:（立希）想认真去做` -- C# `CardTakiSerious` (MatchHost.cs:6749-6791):
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:（立希）想认真去做`）:
//! > （立希）想认真去做：
//! >  使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为原价的四分之一
//!
//! every player holding [停留] settles immediately on a tile you pick within
//! 2 of their position, all payments at a quarter price.

use alloc::vec::Vec;

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const TAKI_SERIOUS: CardDef = CardDef::new(
    "MyGO:（立希）想认真去做",
    // 规则书: 「使场上所有拥有[停留]的玩家立刻…」 -- the cell carries no [限], and
    // the 专有名词 「[限]」 entry makes only 「[限]X」 cards conditional on X, so an
    // empty set is still a legal (no-op) play; the C# `WhyNot` refusal is gone.
    &[On::Play("", None, taki_serious)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"想认真去做"#)
        .raw_name(r#"（立希）想认真去做"#)
        .tags(&[])
        .text(CardText::para(r#"使场上所有拥有[停留]的玩家立刻在所在格子前后2格内你选择的一个格子进行一次[触发结算]，本次结算导致的所有[支付]变为原价的四分之一"#))
        // sheet cell I10
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", r#"椎名立希"#)]));

/// C# `CardTakiSerious.Stayers` -- present players holding [停留].
fn stayers() -> Vec<i32> {
    // 规则书: 「使场上所有拥有[停留]的玩家」 -- C# `H.Present(p) && H.State.seats[p].stay > 0`.
    // `Present` is "in the game and not exiled": an exiled piece is on no tile
    // (`rules/status/src/exile.rs`), so it has no 所在格子 to settle, yet it
    // keeps its [停留] layers -- both halves are checked here.
    (0..ctx::player_count())
        .filter(|&p| {
            !ctx::player_out(p)
                && rules_status::exile::get(p) == 0
                && rules_status::stay::get(p) > 0
        })
        .collect()
}

fn taki_serious(player_id: i32) -> card_sdk::Asked {
    let n = ctx::tile_count();
    if n <= 0 {
        return Ok(());
    }
    for p in stayers() {
        if ctx::player_out(p) {
            continue;
        }
        let pos = ctx::player_pos(p);
        // 规则书: 「所在格子前后2格内」 -- C# `Enumerable.Range(-2, 5)` over the
        // ring, distinct. Ready for the settle below.
        let mut tiles: Vec<i32> = Vec::new();
        for d in -2..=2 {
            let t = ((pos + d) % n + n) % n;
            if !tiles.contains(&t) {
                tiles.push(t);
            }
        }
        if tiles.is_empty() {
            return Ok(());
        }
        let pick = ctx::ask_tile(
            player_id,
            &Msg::new(key!("taki_serious_title")),
            &Msg::new(key!("taki_serious_where")).player_id("who", p),
            &tiles,
        )?;
        // 规则书: 「本次结算导致的所有[支付]变为原价的四分之一」 -- the stayer
        // settles there at a quarter price.
        // TODO(规则书)（本次结算导致的所有[支付]变为原价的四分之一）: 只限本次结算
        //   -- `set_pay_factor` 写整回合的 `turn.plan`，之后本回合的移动落地也按
        //   四分之一付。
        ctx::plan::set_pay_factor(250);
        // TODO(规则书)（进行一次[触发结算]）: 这个强制结算记为 `main = true`（当作
        //   该回合的落点）-- 书只写[触发结算]；`main` 会改写 `landed` 并让
        //   `move.main` 的结算体生效（与轮符雨的 `false` 不同）。
        ctx::card_settle_at(p, pick, true);
        ctx::log(
            player_id,
            &Msg::new(key!("taki_serious_settled"))
                .tile("tile", pick)
                .player_id("who", p),
        );
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn taki_serious_kani(player_id: i32) -> card_sdk::Asked {
    taki_serious(player_id)
}
