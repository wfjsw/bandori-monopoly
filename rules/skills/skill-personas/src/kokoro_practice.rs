//! `skill:凑友希那:来练习吧`
//!
//! 规则书（skill sheet, 凑友希那）:
//! > （1）初始获得"RiNG 4"格子，从"RiNG 4"格子开始游戏，首次经过CiRCLE不获得经过奖励
//! > （2）如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动
//! > （3）移动终点为任意"RiNG"时，获得一个火罐（上限1）。任何时刻当你不位于RiNG时，
//! > 失去所有的火罐。当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家
//! > 强制停下并触发结算。
//!
//! （1） is three grants at game start: a deed, a starting square, and a waived
//! first pass. The deed is `set_owner`; the start is a position write; the
//! waiver is a one-shot latch.
//!
//! （2） 「可以在[经过]的第一个RiNG停止移动」 is a stop the player opts into
//! while walking past, which is the `Pass` moment onto a RiNG tile.
//!
//! （3） has three parts: a pot on ending on a RiNG, losing them all off a RiNG,
//! and a forced stop bought with a pot when someone passes you.

use bandori_std::ctx::{self, state};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// 「首次经过CiRCLE不获得经过奖励」.
const WAIVED: &str = "skill.kokoroPractice.waived";

/// A RiNG tile is any of the four the board names.
fn is_deed(t: i32) -> bool {
    t >= 0 && ctx::is_deed(t)
}

pub const KOKORO_PRACTICE: CardDef = CardDef::new(
    "skill:凑友希那:来练习吧",
    &[
        // （1）'s starting square and deed: the before-match-start point decides
        // start positions.
        On::Action(Action::DeckBeforeGame("", None, at_start)),
        On::Action(Action::Pass(card_sdk::pre::MINE, None, on_pass)),
        // （3）「当其他玩家移动[经过]您时」 -- 行动阶段 12 [经过], per step
        // (`SETTLE-STAGES.md` §4 M4), not the end-tile [重叠]. The guard reads
        // the mover and the tile being entered, so it is `passed_by`, not `mine`.
        On::Action(Action::PassTile(
            "actor != owner && tile.id == owner.pos && charge(owner) >= 1 && move.remaining > 0",
            None,
            on_passed)),
        // （3）「移动终点为任意"RiNG"时，获得一个火罐」 -- 行动阶段 13
        // 「移动终点」 (`SETTLE-STAGES.md` §4 M1): after [重叠], before any
        // settle, for every completed move including a 「不触发结算」 one.
        On::Action(Action::MoveAfter(
            bandori_std::guards::OWN_DEED,
            None,
            on_move_end)),
        // （3）「获得一个火罐（上限1）」 -- the cap has to hold from match start,
        // or `gain_charge` (cap 0 = no pots) drops the grant on the floor.
        On::Action(Action::TurnStartBefore("", None, declare_cap)),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)),
    ],
)
.silent(&[4, 5])
// 「（初始0，上限1）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 0),
    (bandori_std::fire::FIRE_CAP, 1),
]);

/// （3）「获得一个火罐（上限1）」 -- no 「初始N」, so initial is 0 and the cap is 1.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// 「当其他玩家移动[经过]您时」 -- another player's step onto **my** tile.
/// `actor != owner && tile.id == owner.pos && charge(owner) >= 1 &&
/// move.remaining > 0` is the pre.

/// （1）「初始获得"RiNG 4"格子，从"RiNG 4"格子开始游戏，首次经过CiRCLE不获得
/// 经过奖励」.
fn at_start(player_id: i32) -> card_sdk::Asked {
    let t = ctx::tile_named("RiNG 4");
    if t < 0 {
        return Ok(());
    }
    ctx::set_owner(t, player_id);
    ctx::teleport_to(player_id, t);
    // （1）「首次经过CiRCLE不获得经过奖励」 -- arm `prop::NO_REWARD` on this
    // instance (`docs/TILES.md`), and disarm it after the first pass (in
    // `on_pass`). Held as data on the source, never derived from prose.
    state::set(player_id, WAIVED, 0);
    ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("kokoro_practice_start")).tile("tile", t),
    );
    Ok(())
}

/// （2） 「如果移动时[经过]了RiNG，可以在[经过]的第一个RiNG停止移动」, plus
/// （1）'s waived first CiRCLE pass and （3）'s pot-on-RiNG / lose-off-RiNG.
fn on_pass(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    // （1） 「首次经过CiRCLE不获得经过奖励」 -- the flag was armed at game start;
    // this is the pass it waives, so disarm it now (after `circle_reward` has
    // already consulted and honoured it). Subsequent passes earn normally.
    if ctx::is_start(t) && state::get(player_id, WAIVED) == 0 {
        state::set(player_id, WAIVED, 1);
        ctx::set_prop(card_sdk::abi::prop::NO_REWARD, 0);
    }
    // （2） 「可以在[经过]的第一个RiNG停止移动」 -- offered on the first RiNG
    // the walk passes.
    if is_deed(t) && ctx::trigger::move_remaining() > 0 {
        if ctx::ask_yes(
            player_id,
            &Msg::new(key!("kokoro_practice_stop_title")),
            &Msg::new(key!("kokoro_practice_stop_ask")).tile("tile", t),
        )? {
            ctx::plan::set_stop_at(t);
            ctx::log(
                player_id,
                &Msg::new(key!("kokoro_practice_stopped")).tile("tile", t),
            );
        }
    }
    // （3） 「任何时刻当你不位于RiNG时，失去所有的火罐」.
    if !is_deed(t) {
        let have = bandori_std::fire::get(player_id);
        if have > 0 {
            bandori_std::fire::spend(player_id, have, &Msg::new(key!("kokoro_practice_lost")))?;
        }
    }
    Ok(())
}

/// （3）「当其他玩家移动[经过]您时，您可以选择使用一个[火罐]令该玩家强制停下
/// 并触发结算」.
///
/// `SETTLE-STAGES.md` §4 M4: this is a 经过 clause, so it rides the passer's
/// `passTile` step (`passed_by` names the shape). The `move_remaining() > 0`
/// guard keeps it to a **mid-route** pass -- 「强制停下」 is meaningless on the
/// destination tile, and E13's 「移动终点触发[经过]」 is the overlap half. It
/// used to sit on `passPlayer`, where `move_remaining()` is always 0 and the
/// force-stop could never fire (§4 M4's latent bug).
fn on_passed(player_id: i32) -> card_sdk::Asked {
    // `charge(owner) >= 1 && move.remaining > 0` is the pre.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("kokoro_practice_force_title")),
        &Msg::new(key!("kokoro_practice_force_ask")),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("kokoro_practice_spend")))? {
        return Ok(());
    }
    if !ctx::gate(ctx::trigger::player_id(), bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    ctx::plan::set_stop_at(ctx::player_pos(player_id));
    ctx::log(player_id, &Msg::new(key!("kokoro_practice_forced")));
    Ok(())
}

/// （3）「移动终点为任意"RiNG"时，获得一个火罐（上限1）」 -- the move's end
/// tile, 行动阶段 13 (`SETTLE-STAGES.md` §4 M1). `moveAfter` runs after [重叠]
/// and before any settle, for every completed move -- a 「不触发结算」 move
/// grants the pot too. `actor == owner && is_deed(tile.id)` is the pre.
fn on_move_end(player_id: i32) -> card_sdk::Asked {
    bandori_std::fire::gain(player_id, 1, &Msg::new(key!("kokoro_practice_gain")))?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_start_kani(player_id: i32) -> card_sdk::Asked {
    at_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_kani(player_id: i32) -> card_sdk::Asked {
    on_pass(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_passed_kani(player_id: i32) -> card_sdk::Asked {
    on_passed(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_move_end_kani(player_id: i32) -> card_sdk::Asked {
    on_move_end(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}
