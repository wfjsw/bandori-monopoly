//! `Mujica:欢迎来到ave mujica的世界` -- C# `CardWelcomeMujica` (MatchHost.cs:5436-5496):
//! toggle one player's skill state; as [反击], re-toggle everyone who switched.
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:欢迎来到ave mujica的世界`）:
//! > 欢迎来到ave mujica的世界：
//! >  选择以下效果其一发动：
//! >
//! > （1）转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）；
//! >
//! > （2）[反击] 当有其他玩家切换状态时，你与所有本回合切换了状态的玩家同时切换一次状态
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const WELCOME_MUJICA: CardDef = CardDef::new("Mujica:欢迎来到ave mujica的世界", &[
    On::Play(play),
    On::React(&[TriggerKind::State], can_react, react),
]);

fn play(player_id: i32) {
    // 规则书（1）: 「选择以下效果其一发动：转换任意一名玩家的状态」
    // The C# only offers players with `H.HasStates(p)` (skillState > 0); without a
    // state query the whole player list is offered and the toggle folds below.
    let candidates = ctx::others(player_id);
    // TODO(规则书)（1）: 「若指定了不存在状态2的玩家则无效果」 -- C# toggles
    // `skillState` 2 -> 1 or (1|0) -> 2 via `H.SwitchState`; needs the
    // skill-state query / switch hook in the ABI.
    if candidates.is_empty() {
        return;
    }
    let who = ctx::ask_player(
        player_id,
        &Msg::new(key!("welcome_mujica_title")),
        &Msg::new(key!("welcome_mujica_ask")),
        &candidates,
    );
    // 规则书（1）: 「转换任意一名玩家的状态」
    ctx::log(
        player_id,
        &Msg::new(key!("welcome_mujica_switched")).player_id("who", who),
    );
    // TODO(ABI): `H.SwitchState(who, (skillState == 2) ? 1 : 2, ...)` -- the
    // skill-state toggle has no ctx counterpart.
}

fn can_react(player_id: i32) -> bool {
    // 规则书（2）[反击]: 「当有其他玩家切换状态时」 -- C# `t.Kind == "state" &&
    // t.Seat != player`.
    trigger::kind() == TriggerKind::State && trigger::player_id() != player_id
}

fn react(player_id: i32) {
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
    // TODO(ABI): `H.SwitchState` for every target (which also writes the
    // `switchedRound` slot -- C# `H.SetV(seat, "switchedRound", H.TurnKey)`).
}