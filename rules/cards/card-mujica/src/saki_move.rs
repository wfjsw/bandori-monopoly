//! `Mujica:祥，移动` -- C# `CardSakiMove` (MatchHost.cs:5497-5541): force a player
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:祥，移动`）:
//! > 祥，移动：
//! >  强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半
//!
//! 3 tiles in a chosen direction, settling at half price.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const SAKI_MOVE: CardDef = CardDef::new("Mujica:祥，移动", &[On::Play(None, saki_move, "")]);

fn saki_move(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「强制一名玩家向你选择的方向移动3格」 -- pick the target.
    // C# `H.MoveWhyNot(i) == null` gates the self-option (you may choose
    // yourself only before your main move).
    let mut candidates = ctx::others(player_id);
    // 规则书: 「（可在掷骰前选择自己以代替主要移动）」
    if ctx::cant_move(player_id).is_none() {
        candidates.insert(0, player_id);
    }
    if candidates.is_empty() {
        return Ok(());
    }
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("saki_move_title")),
        &Msg::new(key!("saki_move_ask")),
        &candidates,
    )?;
    // C# `H.PickTarget` (MatchHost.cs:18036-18057) = `AskSeat` + `H.Target`:
    // the pick runs the targeting pipeline and lands on the player actually
    // hit (redirect may move it). None = designation failed, the play folds.
    let Some(hit) = ctx::target(who) else {
        return Ok(());
    };
    // 规则书: 「向你选择的方向」
    let forward = ctx::ask_pick(
        player_id,
        &Msg::new(key!("saki_move_dir_title")),
        &Msg::new(key!("saki_move_dir_ask")),
        &[
            Msg::new(key!("saki_move_forward")),
            Msg::new(key!("saki_move_backward")),
        ],
    )? == 0;
    // 规则书: 「移动3格并[触发结算]」 -- C# `H.CardMove` (self,
    // MatchHost.cs:5526-5532) / `H.ForceWalk(hit, forward ? 3 : -3,
    // resolve: true, ...)` (others, MatchHost.cs:5533-5536) build
    // `MoveCtx { Steps = 3, Reverse = !forward, Resolve = true, Forced = true,
    // PayFactor = 0.5 }`. The MoveCtx shape maps onto `ctx::plan::*`.
    ctx::plan::set_steps(3);
    ctx::plan::set_reverse(!forward);
    ctx::plan::set_resolve(true);
    // 规则书: 「触发结算时进行的支付价格减半」 -- C# `MoveCtx.PayFactor = 0.5`
    // (milli-units: 500 = x0.5); the settle of this walk pays half.
    ctx::plan::set_pay_factor(500);
    ctx::log(
        player_id,
        &Msg::new(key!("saki_move_ordered"))
            .player_id("who", hit)
            .i("n", 3),
    );
    // 规则书: 「移动3格并[触发结算]」 -- run the shaped walk now (C# `H.CardMove`
    // for self / `H.ForceWalk` for others, both through the move plan).
    ctx::card_move(hit);
    // 规则书: 「（可在掷骰前选择自己以代替主要移动）」 -- C# `H.CardMove(Forced =
    // true)` runs `MainMoveAs` (MatchHost.cs:23102-23120), which sets
    // `_turnCtx.MainMoved` when the walked player is the turn player; that is
    // exactly `card_move`'s bookkeeping, so choosing self replaces the main
    // move and choosing another player does not.
    Ok(())
}
