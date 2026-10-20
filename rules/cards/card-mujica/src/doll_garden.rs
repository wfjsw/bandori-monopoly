//! `Mujica:人偶的箱庭` -- C# `CardDollGarden` (MatchHost.cs:5929-5980): every other
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:人偶的箱庭`）:
//! > 人偶的箱庭： 
//! >  使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。”  无法移动的玩家只能选择向你支付。
//! >  然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。
//!
//! player either moves without settling or pays you X*20, then you move the sum.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const DOLL_GARDEN: CardDef = CardDef::new(
    "Mujica:人偶的箱庭",
    &[On::Play("", "", Some(cant_play), doll_garden)],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"人偶的箱庭"#)
        .raw_name(r#"人偶的箱庭"#)
        .tags(&[])
        .text(r#"使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。” 无法移动的玩家只能选择向你支付。
然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。"#)
        .cell(r#"J12"#)
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

/// C# `CardDollGarden.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「视为你本回合的主要移动」 -- the forced walk is the main move, so
    // the C# `H.MoveWhyNot` gate applies (own turn, main move still available,
    // turn's move not skipped).
    rules_status::move_blocked(player_id)
}

fn doll_garden(player_id: i32) -> card_sdk::Asked {
    let mut sum = 0;
    for p in ctx::others(player_id) {
        // 规则书: 「X为该玩家正常移动到你所在格子所需的移动数」 -- C#
        // `H.Forward(p.pos, i.pos)`.
        let x = ctx::tile_forward(ctx::player_pos(p), ctx::player_pos(player_id));
        // 规则书: 「无法移动的玩家只能选择向你支付」 -- C# gates on
        // `stay <= 0 && !Stunned` (`MatchPlayer::stunned()` = `stun + stunStart > 0`;
        // `stun_of` is the `stun` field alone).
        let can_move = rules_status::stay::get(p) <= 0 && rules_status::stun::get(p) <= 0;
        if can_move {
            let pick = ctx::ask_pick(
                p,
                &Msg::new(key!("doll_garden_ask_title")),
                &Msg::new(key!("doll_garden_ask_text"))
                    .player_id("user", player_id)
                    .i("x", x as i64)
                    .card("card", "Mujica:人偶的箱庭"),
                &[
                    Msg::new(key!("doll_garden_move")),
                    Msg::new(key!("doll_garden_pay"))
                        .player_id("user", player_id)
                        .n("money", (x * 20) as i64),
                ],
            )?;
            if pick == 1 {
                // 规则书: 「向你支付X*20资金」
                ctx::transfer(p, player_id, x * 20, &Msg::new(key!("doll_garden_why")))?;
                continue;
            }
        } else {
            // 规则书: 「无法移动的玩家只能选择向你支付」 -- no prompt, straight to
            // the payment (C# leaves `num2 = 1` and pays).
            ctx::transfer(p, player_id, x * 20, &Msg::new(key!("doll_garden_why")))?;
            continue;
        }
        // 规则书: 「进行一次移动掷骰并移动对应步数（不[触发结算]）」
        let r = ctx::roll(p, 1, 20);
        sum += r;
        // C# `H.Walk(p, r, resolve: false, ...)` (MatchHost.cs:5965) builds
        // `MoveCtx { Steps = r, Resolve = false }` -- a walk without settling.
        // The MoveCtx shape maps onto `ctx::plan::*`.
        ctx::plan::set_steps(r);
        ctx::plan::set_resolve(false);
        // 规则书: 「进行一次移动掷骰并移动对应步数（不[触发结算]）」 -- run the
        // shaped walk now (C# `H.Walk`, MatchHost.cs:23072-23088, through the
        // move plan).
        ctx::card_move(p);
        ctx::log(
            player_id,
            &Msg::new(key!("doll_garden_walked"))
                .player_id("who", p)
                .i("n", r as i64),
        );
    }
    if sum <= 0 {
        // C# `H.Log("text", i, "没有人移动：... 不移动（人偶的箱庭）")`.
        ctx::log(
            player_id,
            &Msg::new(key!("doll_garden_no_move")).player_id("who", player_id),
        );
        return Ok(());
    }
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。」
    ctx::log(
        player_id,
        &Msg::new(key!("doll_garden_sum"))
            .player_id("who", player_id)
            .i("n", sum as i64),
    );
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和」 -- C# `H.CardMove(c, new
    // MoveCtx { Steps = sum, Forced = true })` (MatchHost.cs:5973-5977) builds
    // `MoveCtx { Steps = sum, Resolve = true, Forced = true }`. The MoveCtx
    // shape maps onto `ctx::plan::*`.
    ctx::plan::set_steps(sum);
    ctx::plan::set_resolve(true);
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和」 -- run the shaped walk now
    // (C# `H.CardMove`, MatchHost.cs:18073-18080, through the move plan).
    ctx::card_move(player_id);
    // 规则书: 「视为你本回合的主要移动」 -- `H.CardMove` runs `MainMoveAs`
    // (MatchHost.cs:23102-23120), which sets `_turnCtx.MainMoved` on the turn
    // player; that is exactly `card_move`'s bookkeeping.
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn doll_garden_kani(player_id: i32) -> card_sdk::Asked {
    doll_garden(player_id)
}
