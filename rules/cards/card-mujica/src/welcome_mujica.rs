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

use bandori_std::state_key;
use card_sdk::abi::ChainKind;
use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const WELCOME_MUJICA: CardDef = CardDef::new(
    "Mujica:欢迎来到ave mujica的世界",
    &[
        On::Play("", None, None, play),
        On::Counteract(&[ChainKind::State], "actor != owner", Some(key!("welcome_mujica_when")), None, counteract),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"欢迎来到ave mujica的世界"#)
        .raw_name(r#"欢迎来到ave mujica的世界"#)
        .tags(&[r#"反击"#])
        .text(r#"选择以下效果其一发动：
（1）转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）；（2）[反击] 当有其他玩家切换状态时，你与所有本回合切换了状态的玩家同时切换一次状态"#)
        // sheet cell J3
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「选择以下效果其一发动：转换任意一名玩家的状态」 -- 「任意一名
    // 玩家」 includes the user. The C# only listed others; the book does not.
    let mut candidates = alloc::vec::Vec::new();
    candidates.push(player_id);
    candidates.extend(ctx::others(player_id));
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
    switch_state(player_id, hit)
}

/// 规则书（1）: 「转换任意一名玩家的状态（若指定了不存在状态2的玩家则无效果）」.
/// A seat with no 状态1/状态2 axis (`skillState` not in {1,2}) is a no-op.
fn switch_state(player_id: i32, hit: i32) -> card_sdk::Asked {
    let cur = ctx::state::get(hit, state_key::SKILL_STATE);
    if cur != 1 && cur != 2 {
        // 「若指定了不存在状态2的玩家则无效果」
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("welcome_mujica_switched")).player_id("who", hit),
    );
    // 「转换任意一名玩家的状态」 -- a straight toggle of the keyed `skillState`.
    // The host stamps `switchedRound = turn_key` on every real 1↔2 toggle
    // (wasm_rules `note_state_toggle`), so the explicit mark below is redundant
    // but kept as a local belt-and-suspenders for the same value.
    let to = if cur == 2 { 1 } else { 2 };
    ctx::state::set(hit, state_key::SKILL_STATE, to);
    ctx::state::set(hit, "switchedRound", ctx::turn_key());
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）[反击]: 「你与所有本回合切换了状态的玩家同时切换一次状态」
    // C# lists `p == c.Seat || H.V(p, "switchedRound") == H.TurnKey`, then
    // `H.SwitchState` on each.
    let mut targets = alloc::vec::Vec::new();
    targets.push(player_id);
    let key = ctx::turn_key();
    for p in ctx::others(player_id) {
        // C# `H.V(p, "switchedRound") == H.TurnKey`. The host stamps this flag
        // on every skill-state 1↔2 toggle (`note_state_toggle`), including
        // character skills that never wrote it themselves.
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
    // play half runs. The host stamps `switchedRound` on each real toggle and
    // suppresses further `state` windows from this re-toggle
    // (`Cx::state_raise_depth`), so the counter cannot ping-pong.
    // Same 「若指定了不存在状态2的玩家则无效果」 no-op on a seat without the axis.
    for &t in &targets {
        switch_state(player_id, t)?;
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
