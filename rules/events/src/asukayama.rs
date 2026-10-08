//! `event:飞鸟山之战` -- 事件卡「飞鸟山之战」（中立事件 A12）.
//!
//! 事件文本（data/events.json, id `飞鸟山之战`）:
//! > 所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20，行动序列上在抽到此卡的玩家的上一位玩家获得2层回合开始时减少一层的[眩晕]，行动序列上在抽到此卡的玩家的下一位玩家在下个自己的回合结束前不可使用手牌，其余玩家获得1层在回合开始时移除的[除外]

use card_sdk::abi::{state_key, MoveKind};
use card_sdk::ctx::{self, plan};
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

// TODO(规则书): the sheet's B12 lists a derived 「迷子的追逐」; the text does not
// say drawing this event triggers it. The main clause only is implemented.

pub const ASUKAYAMA: CardDef = CardDef::new("event:飞鸟山之战", &[On::Play(None, play, "")]);

/// 规则书: 「所有玩家[传送]到飞鸟山公园」 -- everyone onto the park, no settle
/// on the jump itself.
/// 规则书: 「然后抽到此卡的玩家移动1d20」 -- the drawer walks the rolled face.
/// The text does not say 不触发场地效果 here, so the landing settles.
/// TODO(规则书): settle-on-land is the default reading; flip if the sheet means
/// the walk is also settle-free.
/// 规则书: 「上一位玩家获得2层回合开始时减少一层的[眩晕]」 -- `stunStart`, the
/// stun that loses a layer at turn start (not the end-of-turn `stun`).
/// 规则书: 「下一位玩家在下个自己的回合结束前不可使用手牌」 -- `noHand` = 2: the
/// engine ticks 2->1 at their turn start and 1->0 at their turn end, so the
/// veto lifts exactly at the end of their next own turn.
/// 规则书: 「其余玩家获得1层在回合开始时移除的[除外]」 -- `give_exile(p, 1, -1)`.
fn play(player_id: i32) -> card_sdk::Asked {
    let park = ctx::tile_named("飞鸟山公园");
    for p in all_players() {
        if p == player_id {
            // The drawer's jump is a `card_move` (a host request, replay-stable)
            // rather than `teleport_to`: the walk below is also a host request,
            // and a replay of this body would otherwise re-teleport them back to
            // the park after the walk has already run.
            plan::set_kind(MoveKind::Teleport);
            plan::set_teleport_to(park);
            plan::set_resolve(false);
            ctx::card_move(p);
        } else {
            ctx::teleport_to(p, park);
        }
    }
    let face = roll(player_id, 1, 20);
    ctx::log(
        player_id,
        &Msg::new("log.event.dice").player_id("who", player_id).i("n", face as i64),
    );
    // 「抽到此卡的玩家移动1d20」 -- `card_move` of the rolled length; the landing
    // settles (`MoveCtx.Resolve` defaults to true). Reset the plan the teleport
    // above shaped: this is a walk of `face` steps, not another jump.
    plan::set_kind(MoveKind::Walk);
    plan::set_teleport_to(-1);
    plan::set_resolve(true);
    plan::set_steps(face);
    ctx::card_move(player_id);

    // 「行动序列上…的上一位玩家」 / 「下一位玩家」 -- `neighbor` walks the
    // present-player turn order around the drawer.
    let prev = ctx::neighbor(player_id, -1);
    let next = ctx::neighbor(player_id, 1);
    if prev >= 0 {
        // 「获得2层回合开始时减少一层的[眩晕]」
        ctx::state::add(prev, state_key::STUN_START, 2);
        ctx::state::set_expires(prev, state_key::STUN_START, ctx::state::TURN_START);
        ctx::log(
            prev,
            &Msg::new("log.event.asukayama_stun").player_id("who", prev).i("n", 2),
        );
    }
    if next >= 0 {
        // 「在下个自己的回合结束前不可使用手牌」
        ctx::state::set(next, state_key::NO_HAND, 2);
        ctx::log(
            next,
            &Msg::new("log.event.asukayama_no_hand").player_id("who", next),
        );
    }
    for p in all_players() {
        if p == player_id || p == prev || p == next {
            continue;
        }
        // 「其余玩家获得1层在回合开始时移除的[除外]」
        ctx::give_exile(p, 1, -1);
        ctx::log(p, &Msg::new("log.event.asukayama_exile").player_id("who", p));
    }
    Ok(())
}