//! `Mujica:人偶的箱庭` -- C# `CardDollGarden` (MatchHost.cs:5929-5980): every other
//! player either moves without settling or pays you X*20, then you move the sum.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:人偶的箱庭`）:
//! > 人偶的箱庭：
//! >  使场上所有其他玩家选择其一执行：“进行一次移动掷骰并移动对应步数（不[触发结算]）”或“向你支付X*20资金，X为该玩家正常移动到你所在格子所需的移动数。”  无法移动的玩家只能选择向你支付。
//! >  然后，你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const DOLL_GARDEN: CardDef = CardDef {
    id: "Mujica:人偶的箱庭",
    play: Some(doll_garden),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardDollGarden.WhyNot` = `H.MoveWhyNot(seat)`.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「视为你本回合的主要移动」 -- the forced walk is the main move, so
    // the C# `H.MoveWhyNot` gate applies: own turn first (`只能在自己的回合`).
    if ctx::turn_seat() != seat {
        return Some(Msg::new(key!("x_not_your_turn")));
    }
    // TODO(规则书): the rest of C# `H.MoveWhyNot` -- refuses after this turn's
    // main move (`_turnCtx.MainMoved` -> 「这回合已经移动过了」) and when the
    // turn's move is skipped (`State.skipMove` -> 「本回合不能移动」); needs
    // `H.MoveWhyNot` in the ABI.
    None // playable
}

fn doll_garden(seat: i32) {
    let mut sum = 0;
    for p in ctx::others(seat) {
        // 规则书: 「X为该玩家正常移动到你所在格子所需的移动数」 -- C#
        // `H.Forward(p.pos, i.pos)`.
        let x = ctx::tile_forward(ctx::seat_pos(p), ctx::seat_pos(seat));
        // 规则书: 「无法移动的玩家只能选择向你支付」 -- C# gates on
        // `stay <= 0 && !Stunned` (`MatchSeat.Stunned` = `stun + stunStart > 0`;
        // `stun_of` is the `stun` field alone).
        let can_move = ctx::stay_of(p) <= 0 && ctx::stun_of(p) <= 0;
        if can_move {
            let pick = ctx::ask_pick(
                p,
                &Msg::new(key!("doll_garden_ask_title")),
                &Msg::new(key!("doll_garden_ask_text"))
                    .seat("user", seat)
                    .i("x", x as i64),
                &[
                    Msg::new(key!("doll_garden_move")),
                    Msg::new(key!("doll_garden_pay")).n("money", (x * 20) as i64),
                ],
            );
            if pick == 1 {
                // 规则书: 「向你支付X*20资金」
                ctx::transfer(p, seat, x * 20, &Msg::new(key!("doll_garden_why")));
                continue;
            }
        } else {
            // 规则书: 「无法移动的玩家只能选择向你支付」 -- no prompt, straight to
            // the payment (C# leaves `num2 = 1` and pays).
            ctx::transfer(p, seat, x * 20, &Msg::new(key!("doll_garden_why")));
            continue;
        }
        // 规则书: 「进行一次移动掷骰并移动对应步数（不[触发结算]）」
        let r = ctx::roll(p, 1, 20);
        sum += r;
        // C# `H.Walk(p, r, resolve: false, ...)` -- walk without settling.
        // `teleport_to` jumps without settling but ignores the walk length.
        // TODO(ABI): `H.Walk(p, r, resolve: false, ...)`.
        ctx::teleport_to(p, ctx::tile_steps_ahead(p, r));
        ctx::log(
            seat,
            &Msg::new(key!("doll_garden_walked")).seat("who", p).i("n", r as i64),
        );
    }
    if sum <= 0 {
        // C# `H.Log("text", i, "没有人移动：... 不移动（人偶的箱庭）")`.
        ctx::log(seat, &Msg::new(key!("doll_garden_no_move")).seat("who", seat));
        return;
    }
    // 规则书: 「你强制移动其他玩家本次移动掷骰数之和。视为你本回合的主要移动。」
    ctx::log(
        seat,
        &Msg::new(key!("doll_garden_sum")).seat("who", seat).i("n", sum as i64),
    );
    // TODO(ABI): `H.CardMove(c, Steps = sum, Forced = true)` -- the forced walk
    // with settle is not in the vocabulary; `teleport_to` does not settle.
    let to = ctx::tile_steps_ahead(seat, sum);
    ctx::teleport_to(seat, to);
    // TODO(规则书): 「视为你本回合的主要移动」 -- needs the main-move
    // bookkeeping (C# `H.MoveWhyNot` / `_turnCtx.MainMoved`).
}