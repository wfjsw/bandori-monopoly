//! `Mujica:无法将视线移开` -- C# `CardCantLookAway` (MatchHost.cs:5654-5710):
//! force-move everyone who [反击]'d you this turn 1-4 tiles with settle.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:无法将视线移开`）:
//! > 无法将视线移开：
//! >  使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算] （此卡可作为[反击]在有玩家对你使用[反击]后立即使用），若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const CANT_LOOK_AWAY: CardDef = CardDef {
    id: "Mujica:无法将视线移开",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    // C# `CardCantLookAway.WhyNot` refuses without a reactor this turn
    // (`Reactors(seat).Count != 0`) -- the same reaction history the effect
    // body needs. TODO(规则书): the `why_not` gate once `H._reactedAgainst` is
    // in the ABI (below).
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「此卡可作为[反击]在有玩家对你使用[反击]后立即使用」 -- C#
    // `t.Kind == "reacted" && t.Target == seat && t.Seat != seat`.
    trigger::kind() == TriggerKind::Reacted
        && trigger::target() == seat
        && trigger::seat() != seat
}

fn play(seat: i32) {
    run(seat);
}

fn react(seat: i32) {
    run(seat);
}

fn run(seat: i32) {
    // 规则书: 「使当前回合内对你打出过[反击]的所有玩家」 -- C#
    // `H._reactedAgainst` filtered by `target == seat && turn == H.TurnKey`.
    // TODO(ABI): the reaction history (C# `H._reactedAgainst`); `ctx::turn_key()`
    // covers the `turn == H.TurnKey` half of the filter, but the list itself
    // cannot be built without the history. We still walk the ordinary seat list
    // so the prompts below are exercised.
    let reactors: alloc::vec::Vec<i32> = ctx::others(seat);
    for p in reactors {
        // 规则书: 「向你选择的方向强制移动1~4以内的任意步数」
        // C# `H.AskNumber(i, ..., 1, 4, ...)` (an AskPick over the range).
        let n = ctx::ask_number(
            seat,
            &Msg::new(key!("cant_look_away_title")),
            &Msg::new(key!("cant_look_away_steps")).seat("who", p),
            1,
            4,
        );
        let forward = ctx::ask_pick(
            seat,
            &Msg::new(key!("cant_look_away_dir_title")),
            &Msg::new(key!("cant_look_away_dir_ask")),
            &[
                Msg::new(key!("cant_look_away_forward")),
                Msg::new(key!("cant_look_away_backward")),
            ],
        ) == 0;
        // 规则书: 「强制移动1~4以内的任意步数并[触发结算]」 -- C# `H.ForceWalk`
        // (resolve: true). The movement routine is not in the vocabulary.
        // TODO(ABI): `H.ForceWalk(p, forward ? n : -n, resolve: true, seat, ...)`.
        let _ = (p, forward, n);
        ctx::log(
            seat,
            &Msg::new(key!("cant_look_away_ordered"))
                .seat("who", p)
                .i("n", n as i64),
        );
        // 规则书: 「若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动」
        // -- self-targeting counts as the main move (C# `H.ForceWalk` with
        // `Forced = true` on your own turn); needs the main-move bookkeeping.
    }
    // TODO(规则书): 「若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动」
    // -- needs `H.MoveWhyNot` / `_turnCtx.MainMoved` in the ABI.
}