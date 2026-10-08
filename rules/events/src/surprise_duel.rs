//! `event:意外的对邦` -- 事件卡「意外的对邦」（中立事件 A19）.
//!
//! 事件文本（data/events.json, id `意外的对邦`）:
//! > 将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌。所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动（如果距离一样则向正常方向移动）。

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, plan, trigger};
use card_sdk::{CardDef, Msg, On};

use crate::util::{expire, keep};

const ID: &str = "意外的对邦";

/// The player who drew the event, on the instance's props.
const DRAWER: &str = "drawer";
/// How many of the drawer's own turn starts have passed.
const TURNS: &str = "turns";

pub const SURPRISE_DUEL: CardDef = CardDef::new(
    "event:意外的对邦",
    &[
        On::Play("", None, play),
        On::Hook(&[HookKind::RollPlan], "", Some(always), on_plan),
        On::Hook(&[HookKind::TurnStartBefore], "", Some(always), on_turn_start),
    ],
);

fn always(_player_id: i32) -> bool {
    true
}

/// 规则书: 「将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌」
/// -- keep the event and remember who drew it; the turn-start hook below counts
/// their turns and files it away on the second.
/// TODO(规则书): 「下2回合」 -- the start of their 2nd turn from now (implemented)
/// versus a two-turn window that expires at the first start? Reading as
/// 「下2回合」 = the turn after next.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_prop(DRAWER, player_id);
    ctx::set_prop(TURNS, 0);
    ctx::log(
        player_id,
        &Msg::new("log.event.surprise_duel_on").player_id("who", player_id),
    );
    Ok(())
}

/// 规则书: 「所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动
/// （如果距离一样则向正常方向移动）」 -- at `RollPlan`, a non-drawer mover whose
/// reverse path is strictly closer to the drawer's seat walks backwards; a tie
/// keeps the normal direction. Only the direction is rewritten, not the length.
fn on_plan(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::turn_player();
    let drawer = ctx::prop(DRAWER);
    if mover < 0 || mover == drawer {
        return Ok(());
    }
    let here = ctx::player_pos(mover);
    let there = ctx::player_pos(drawer);
    let n = ctx::tile_count();
    if here < 0 || there < 0 || n <= 0 {
        return Ok(());
    }
    // Forward steps from `here` to `there`; the wrap-around the other way is
    // `n - f`. `f == 0` (same tile) leaves the normal direction alone.
    let f = ctx::tile_forward(here, there);
    let b = n - f;
    if f > 0 && b < f {
        plan::set_reverse(true);
        ctx::log(
            mover,
            &Msg::new("log.event.surprise_duel_dir").player_id("who", mover),
        );
    }
    Ok(())
}

/// 规则书: 「抽到的玩家的下2回合开始时放入事件弃牌」 -- count the drawer's own
/// turn starts and expire on the second.
fn on_turn_start(_player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != ctx::prop(DRAWER) {
        return Ok(());
    }
    let n = ctx::prop(TURNS) + 1;
    ctx::set_prop(TURNS, n);
    if n >= 2 {
        expire(ID);
    }
    Ok(())
}