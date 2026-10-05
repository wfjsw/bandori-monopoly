//! `Mujica:祥，移动` -- C# `CardSakiMove` (MatchHost.cs:5497-5541): force a player
//! 3 tiles in a chosen direction, settling at half price.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:祥，移动`）:
//! > 祥，移动：
//! >  强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SAKI_MOVE: CardDef = CardDef::new("Mujica:祥，移动", &[
    On::Play(saki_move),
]);

fn saki_move(player_id: i32) {
    // 规则书: 「强制一名玩家向你选择的方向移动3格」 -- pick the target.
    // C# `H.MoveWhyNot(i) == null` gates the self-option (you may choose
    // yourself only before your main move).
    let mut candidates = ctx::others(player_id);
    // 规则书: 「（可在掷骰前选择自己以代替主要移动）」
    if ctx::cant_move(player_id).is_none() {
        candidates.insert(0, player_id);
    }
    if candidates.is_empty() {
        return;
    }
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("saki_move_title")),
        &Msg::new(key!("saki_move_ask")),
        &candidates,
    );
    // 规则书: 「向你选择的方向」
    let forward = ctx::ask_pick(
        player_id,
        &Msg::new(key!("saki_move_dir_title")),
        &Msg::new(key!("saki_move_dir_ask")),
        &[
            Msg::new(key!("saki_move_forward")),
            Msg::new(key!("saki_move_backward")),
        ],
    ) == 0;
    // 规则书: 「移动3格并[触发结算]」 -- C# `H.CardMove` (self) /
    // `H.ForceWalk(who, forward ? 3 : -3, resolve: true, ...)` (others) builds
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
            .player_id("who", who)
            .i("n", 3),
    );
    // TODO(ABI): run the shaped walk now -- C# `H.CardMove` (self, settles and
    // consumes the main move) / `H.ForceWalk` (others, a forced side walk).
    // The MoveCtx fields above are written; the walk routine itself
    // (`ctx::card_move` / a force-walk) is not in the vocabulary yet, so the
    // player does not actually move here.
    // TODO(规则书): 「可在掷骰前选择自己以代替主要移动」 -- the self-target walk
    // must consume the turn's main move (C# `H.CardMove(Forced = true)` sets
    // `_turnCtx.MainMoved`); main-move bookkeeping stays held with
    // `ctx::card_move`.
}