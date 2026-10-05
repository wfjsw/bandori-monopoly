//! `Mujica:人偶的箱庭` -- C# `CardDollGarden` (MatchHost.cs:5929-5980): every other
//! player either moves without settling or pays you X*20, then you move the sum.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:人偶的箱庭`）:
//! > 人偶的箱庭：
//! >  使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。”  无法移动的玩家只能选择向你支付。
//! >  然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const DOLL_GARDEN: CardDef = CardDef::new("Mujica:人偶的箱庭", &[
    On::Play(doll_garden),
    On::CantPlay(cant_play),
]);

/// C# `CardDollGarden.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「视为你本回合的主要移动」 -- the forced walk is the main move, so
    // the C# `H.MoveWhyNot` gate applies (own turn, main move still available,
    // turn's move not skipped).
    ctx::cant_move(player_id)
}

fn doll_garden(player_id: i32) {
    let mut sum = 0;
    for p in ctx::others(player_id) {
        // 规则书: 「X为该玩家正常移动到你所在格子所需的移动数」 -- C#
        // `H.Forward(p.pos, i.pos)`.
        let x = ctx::tile_forward(ctx::player_pos(p), ctx::player_pos(player_id));
        // 规则书: 「无法移动的玩家只能选择向你支付」 -- C# gates on
        // `stay <= 0 && !Stunned` (`MatchPlayer::stunned()` = `stun + stunStart > 0`;
        // `stun_of` is the `stun` field alone).
        let can_move = ctx::stay_of(p) <= 0 && ctx::stun_of(p) <= 0;
        if can_move {
            let pick = ctx::ask_pick(
                p,
                &Msg::new(key!("doll_garden_ask_title")),
                &Msg::new(key!("doll_garden_ask_text"))
                    .player_id("user", player_id)
                    .i("x", x as i64),
                &[
                    Msg::new(key!("doll_garden_move")),
                    Msg::new(key!("doll_garden_pay")).n("money", (x * 20) as i64),
                ],
            );
            if pick == 1 {
                // 规则书: 「向你支付X*20资金」
                ctx::transfer(p, player_id, x * 20, &Msg::new(key!("doll_garden_why")));
                continue;
            }
        } else {
            // 规则书: 「无法移动的玩家只能选择向你支付」 -- no prompt, straight to
            // the payment (C# leaves `num2 = 1` and pays).
            ctx::transfer(p, player_id, x * 20, &Msg::new(key!("doll_garden_why")));
            continue;
        }
        // 规则书: 「进行一次移动掷骰并移动对应步数（不[触发结算]）」
        let r = ctx::roll(p, 1, 20);
        sum += r;
        // C# `H.Walk(p, r, resolve: false, ...)` builds `MoveCtx { Steps = r,
        // Resolve = false }` -- a walk without settling. The MoveCtx shape
        // maps onto `ctx::plan::*`; `teleport_to` stands in for the landing.
        ctx::plan::set_steps(r);
        ctx::plan::set_resolve(false);
        // TODO(ABI): run the shaped walk now -- C# `H.Walk(p, r, resolve:
        // false, ...)`. The MoveCtx fields above are written; the walk routine
        // itself is not in the vocabulary yet, so `teleport_to` jumps to the
        // landing tile without walking the path.
        ctx::teleport_to(p, ctx::tile_steps_ahead(p, r));
        ctx::log(
            player_id,
            &Msg::new(key!("doll_garden_walked")).player_id("who", p).i("n", r as i64),
        );
    }
    if sum <= 0 {
        // C# `H.Log("text", i, "没有人移动：... 不移动（人偶的箱庭）")`.
        ctx::log(player_id, &Msg::new(key!("doll_garden_no_move")).player_id("who", player_id));
        return;
    }
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。」
    ctx::log(
        player_id,
        &Msg::new(key!("doll_garden_sum")).player_id("who", player_id).i("n", sum as i64),
    );
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和」 -- C#
    // `H.CardMove(c, new MoveCtx { Steps = sum, Forced = true })` builds
    // `MoveCtx { Steps = sum, Resolve = true, Forced = true }`. The MoveCtx
    // shape maps onto `ctx::plan::*`.
    ctx::plan::set_steps(sum);
    ctx::plan::set_resolve(true);
    // TODO(ABI): run the shaped walk now -- C# `H.CardMove(c, Steps = sum,
    // Forced = true)`. The MoveCtx fields above are written; `ctx::card_move`
    // is not in the vocabulary yet, so `teleport_to` jumps without settling.
    let to = ctx::tile_steps_ahead(player_id, sum);
    ctx::teleport_to(player_id, to);
    // TODO(规则书): 「视为你本回合的主要移动」 -- this forced walk must count as
    // the turn's main move (C# `H.CardMove(Forced = true)` sets
    // `_turnCtx.MainMoved`); main-move bookkeeping stays held with
    // `ctx::card_move`.
}