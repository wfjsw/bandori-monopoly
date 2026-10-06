//! `Mujica:无法将视线移开` -- C# `CardCantLookAway` (MatchHost.cs:5654-5710):
//! force-move everyone who [反击]'d you this turn 1-4 tiles with settle.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:无法将视线移开`）:
//! > 无法将视线移开：
//! >  使当前回合内对你打出过[反击]的所有玩家向你选择的方向强制移动1~4以内的任意步数并[触发结算] （此卡可作为[反击]在有玩家对你使用[反击]后立即使用），若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动
//!

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const CANT_LOOK_AWAY: CardDef = CardDef::new(
    "Mujica:无法将视线移开",
    &[
        On::Play(None, play),
        On::CounterAct(&[ChainKind::Reacted], can_react, react),
    ],
);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「此卡可作为[反击]在有玩家对你使用[反击]后立即使用」 -- C#
    // `t.Kind == "reacted" && t.Target == seat && t.Seat != seat`.
    trigger::kind() == TriggerKind::Reacted
        && trigger::target() == player_id
        && trigger::player_id() != player_id
}

fn play(player_id: i32) -> card_sdk::Asked {
    run(player_id);
    Ok(())
}

fn react(player_id: i32) -> card_sdk::Asked {
    run(player_id);
    Ok(())
}

fn run(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「使当前回合内对你打出过[反击]的所有玩家」 -- C#
    // `H._reactedAgainst` filtered by `target == player_id && turn == H.TurnKey`.
    // TODO(规则书)[judgement](ABI): the reaction history (C# `H._reactedAgainst`); `ctx::turn_key()`
    //   the clause under-specifies -- see the note above it
    // covers the `turn == H.TurnKey` half of the filter, but the list itself
    // cannot be built without the history. We still walk the ordinary player list
    // so the prompts below are exercised.
    let reactors: alloc::vec::Vec<i32> = ctx::others(player_id);
    for p in reactors {
        // 规则书: 「向你选择的方向强制移动1~4以内的任意步数」
        // C# `H.AskNumber(i, ..., 1, 4, ...)` (an AskPick over the range).
        let n = ctx::ask_number(
            player_id,
            &Msg::new(key!("cant_look_away_title")),
            &Msg::new(key!("cant_look_away_steps")).player_id("who", p),
            1,
            4,
        )?;
        let forward = ctx::ask_pick(
            player_id,
            &Msg::new(key!("cant_look_away_dir_title")),
            &Msg::new(key!("cant_look_away_dir_ask")),
            &[
                Msg::new(key!("cant_look_away_forward")),
                Msg::new(key!("cant_look_away_backward")),
            ],
        )? == 0;
        // 规则书: 「强制移动1~4以内的任意步数并[触发结算]」 -- C# `H.ForceWalk(p,
        // forward ? n : -n, resolve: true, ...)` (MatchHost.cs:5708) builds
        // `MoveCtx { Steps = n, Reverse = !forward, Resolve = true,
        // Forced = true }`. The MoveCtx shape maps onto `ctx::plan::*`.
        ctx::plan::set_steps(n);
        ctx::plan::set_reverse(!forward);
        ctx::plan::set_resolve(true);
        ctx::log(
            player_id,
            &Msg::new(key!("cant_look_away_ordered"))
                .player_id("who", p)
                .i("n", n as i64),
        );
        // 规则书: 「强制移动1~4以内的任意步数并[触发结算]」 -- run the shaped walk
        // now (C# `H.ForceWalk`, MatchHost.cs:23453-23480, through the move
        // plan).
        ctx::card_move(p);
    }
    // 规则书: 「若使用者在自身回合内选择了使用者自己进行强制移动，则视为其主要移动」
    // -- `card_move` runs `MainMoveAs` (MatchHost.cs:23102-23120), which sets
    // `_turnCtx.MainMoved` when the walked player is the turn player, so a
    // self walk on one's own turn consumes the main move. (The C# `Reactors`
    // list is others-only, so the clause is vacuous there; the bookkeeping is
    // right either way.)
    Ok(())
}
