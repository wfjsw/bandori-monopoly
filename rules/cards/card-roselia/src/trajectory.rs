//! `R:轨迹` -- C# `CardTrajectory` (MatchHost.cs:10922-10959): [反击][特] on a
//!
//! 规则书（docs/rulebook/cards.json, id `R:轨迹`）:
//! > 轨迹：
//! > [反击] [特]
//! > （1）在场上有玩家破产时，展示此卡，你获得那名玩家的任意一张地契（自动免费赎回），并拆除那个对应格子的所有房屋。
//!
//! bankruptcy: take one of the bankrupt player's deeds, free and clear.

use alloc::vec::Vec;

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TRAJECTORY: CardDef = CardDef::new("R:轨迹", &[
    On::CounterAct(&[ChainKind::Bankrupt], can_react, react),
]);

/// 规则书（1）[反击]: 「在场上有玩家破产时，展示此卡」
fn can_react(player_id: i32) -> bool {
    // 规则书（1）[反击]: 「在场上有玩家破产时」 -- C# `t.Kind == "bankrupt" && t.Target != seat`.
    if trigger::kind() != TriggerKind::Bankrupt {
        return false;
    }
    let who = trigger::target();
    if who < 0 || who == player_id {
        return false;
    }
    // 规则书（1）[反击]: 「你获得那名玩家的任意一张地契」 -- only worth reacting when
    // they still have one (C# `H.OwnedBy(t.Target).Count > 0`).
    ctx::owned_count(who) > 0
}

fn react(player_id: i32) {
    let who = trigger::target();
    if who < 0 {
        return;
    }
    let deeds: Vec<i32> = ctx::owned_tiles(who);
    if deeds.is_empty() {
        return;
    }
    // 规则书（1）[反击]: 「你获得那名玩家的任意一张地契」 -- C# `H.AskTileOf` over `deeds`
    // (the C# AI default is the most expensive deed).
    let tile = ctx::ask_tile(
        player_id,
        &Msg::new(key!("trajectory_ask_title")),
        &Msg::new(key!("trajectory_ask_text")).player_id("who", who),
        &deeds,
    );
    // 规则书（1）[反击]: 「你获得那名玩家的任意一张地契（自动免费赎回），并拆除那个对应格子的所有房屋」
    //   -- C# `H.State.owners[num] = i; H.State.mortgaged[num] = false;
    //   H.State.houses[num] = 0`.
    ctx::set_owner(tile, player_id);
    // 规则书（1）[反击]: 「（自动免费赎回）」
    ctx::set_mortgaged(tile, false);
    // 规则书（1）[反击]: 「并拆除那个对应格子的所有房屋」
    ctx::set_houses(tile, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("trajectory_take"))
            .player_id("who", player_id)
            .player_id("them", who)
            .tile("tile", tile),
    );
}