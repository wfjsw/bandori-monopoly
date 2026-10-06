//! `Mujica:欢迎来到ave mujica的世界` -- C# `CardWelcomeMujica` (MatchHost.cs:5436-5496):
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:欢迎来到ave mujica的世界`）:
//! > 欢迎来到ave mujica的世界：
//! >  选择以下效果其一发动： 
//! >  
//! > （1）转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）；
//! > （2）[反击] 当有其他玩家切换状态时，你与所有本回合切换了状态的玩家同时切换一次状态
//!
//! toggle one player's skill state; as [反击], re-toggle everyone who switched.

use card_sdk::abi::state_key;
use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const WELCOME_MUJICA: CardDef = CardDef::new(
    "Mujica:欢迎来到ave mujica的世界",
    &[
        On::Play(None, play),
        On::Counteract(&[ChainKind::State], can_counteract, counteract),
    ],
);

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「选择以下效果其一发动：转换任意一名玩家的状态」
    // The C# only offers players with `H.HasStates(p)` (skillState > 0); without a
    // state query the whole player list is offered and the toggle folds below.
    let candidates = ctx::others(player_id);
    // TODO(规则书)（1）[judgement]: 「若指定了不存在状态2的玩家则无效果」 -- read
    // one way this gates the switch on the target already being in state 2 (so
    // the only transition is 2 -> 1); read another it rules out targets for whom
    // state 2 is unreachable. The clause does not say which, so the toggle below
    // follows 「转换任意一名玩家的状态」 and nothing else.
    if candidates.is_empty() {
        return Ok(());
    }
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("welcome_mujica_title")),
        &Msg::new(key!("welcome_mujica_ask")),
        &candidates,
    )?;
    // C# `H.PickTarget` = `AskSeat` + `H.Target` (MatchHost.cs:18036-18057):
    // the pick runs the full targeting pipeline (out / exile / ImmuneAll /
    // _targeted / Untargetable / redirect / `target` [反击] window) and lands
    // on the player actually hit (`Some(hit)`, redirect may move it). None =
    // the designation failed and the play folds.
    let Some(hit) = ctx::target(who) else {
        return Ok(());
    };
    // 规则书（1）: 「转换任意一名玩家的状态」 -- C# applies `H.SwitchState` to
    // `r.index`, the post-redirect target (`res.index = t.index`).
    ctx::log(
        player_id,
        &Msg::new(key!("welcome_mujica_switched")).player_id("who", hit),
    );
    // 规则书（1）: 「转换任意一名玩家的状态」 -- a straight toggle of the keyed
    // `skillState`, 2 -> 1 and anything else -> 2.
    let cur = ctx::state::get(hit, state_key::SKILL_STATE);
    let to = if cur == 2 { 1 } else { 2 };
    ctx::state::set(hit, state_key::SKILL_STATE, to);
    ctx::state::set(hit, "switchedRound", ctx::turn_key());
    Ok(())
}

fn can_counteract(player_id: i32) -> bool {
    // 规则书（2）[反击]: 「当有其他玩家切换状态时」 -- C# `t.Kind == "state" &&
    // t.Seat != player`.
    trigger::kind() == TriggerKind::State && trigger::player_id() != player_id
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）[反击]: 「你与所有本回合切换了状态的玩家同时切换一次状态」
    // C# lists `p == c.Seat || H.V(p, "switchedRound") == H.TurnKey`, then
    // `H.SwitchState` on each.
    let mut targets = alloc::vec::Vec::new();
    targets.push(player_id);
    let key = ctx::turn_key();
    for p in ctx::others(player_id) {
        // C# `H.V(p, "switchedRound") == H.TurnKey` -- the slot is set by the
        // missing `H.SwitchState`, so the comparison is real but the flag never
        // lights until that hook lands (which also implies `H.HasStates(p)`,
        // the C#'s other filter).
        if ctx::slot(p, "switchedRound") == key {
            targets.push(p);
        }
    }
    for t in &targets {
        ctx::log(
            player_id,
            &Msg::new(key!("welcome_mujica_switched")).player_id("who", *t),
        );
    }
    // 规则书（2）: `H.SwitchState` on each -- the same skill-state toggle the
    // play half runs, plus the `switchedRound` mark the filter above keys on.
    for &t in &targets {
        let cur = ctx::state::get(t, state_key::SKILL_STATE);
        ctx::state::set(t, state_key::SKILL_STATE, if cur == 2 { 1 } else { 2 });
        ctx::state::set(t, "switchedRound", key);
    }
    Ok(())
}
