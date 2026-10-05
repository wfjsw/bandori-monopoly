//! `Mujica:祥，移动` -- C# `CardSakiMove` (MatchHost.cs:5497-5541): force a player
//! 3 tiles in a chosen direction, settling at half price.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:祥，移动`）:
//! > 祥，移动：
//! >  强制一名玩家向你选择的方向移动3格并[触发结算]（可在掷骰前选择自己以代替主要移动），触发结算时进行的支付价格减半
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const SAKI_MOVE: CardDef = CardDef {
    id: "Mujica:祥，移动",
    play: Some(saki_move),
    can_react: None,
    react: None,
    // C# `CardSakiMove` has no `WhyNot` override -- the card is always playable;
    // `H.MoveWhyNot` only gates the self-target option (below).
    why_not: None,
};

fn saki_move(seat: i32) {
    // 规则书: 「强制一名玩家向你选择的方向移动3格」 -- pick the target.
    // C# `H.MoveWhyNot(i) == null` gates the self-option (you may choose
    // yourself only before your main move).
    let mut candidates = ctx::others(seat);
    // 规则书: 「（可在掷骰前选择自己以代替主要移动）」 -- own turn first
    // (`H.MoveWhyNot` starts with `State.turn == seat`).
    if ctx::turn_seat() == seat {
        candidates.insert(0, seat);
    }
    // TODO(规则书): the rest of the C# `H.MoveWhyNot(i) == null` self-gate --
    // also refuses after this turn's main move (`_turnCtx.MainMoved`) and when
    // the turn's move is skipped (`State.skipMove`); needs `H.MoveWhyNot` in
    // the ABI. Until then the self-option may appear one beat late.
    if candidates.is_empty() {
        return;
    }
    let who = ctx::ask_seat(
        seat,
        &Msg::new(key!("saki_move_title")),
        &Msg::new(key!("saki_move_ask")),
        &candidates,
    );
    // 规则书: 「向你选择的方向」
    let forward = ctx::ask_pick(
        seat,
        &Msg::new(key!("saki_move_dir_title")),
        &Msg::new(key!("saki_move_dir_ask")),
        &[
            Msg::new(key!("saki_move_forward")),
            Msg::new(key!("saki_move_backward")),
        ],
    ) == 0;
    // 规则书: 「移动3格并[触发结算]」 -- C# `H.CardMove` / `H.ForceWalk`
    // (steps = 3, resolve: true). The movement routine is not in the
    // vocabulary; `teleport_to` jumps without settling and ignores direction.
    // TODO(ABI): `H.ForceWalk(who, forward ? 3 : -3, resolve: true, seat, ...)`.
    let _ = (who, forward);
    ctx::log(
        seat,
        &Msg::new(key!("saki_move_ordered"))
            .seat("who", who)
            .i("n", 3),
    );
    // 规则书: 「触发结算时进行的支付价格减半」 -- C# `MoveCtx.PayFactor = 0.5`
    // rides along with the walk; without the movement routine the halving is
    // not expressible.
    // TODO(规则书): 「触发结算时进行的支付价格减半」 -- needs the movement
    // routine's `PayFactor` (C# `H.ForceWalk(..., payFactor: 0.5)`).
    // TODO(规则书): 「可在掷骰前选择自己以代替主要移动」 -- self-targeting the
    // card replaces the main move (C# `H.CardMove` with `Forced = true`);
    // needs the main-move bookkeeping in the ABI.
}