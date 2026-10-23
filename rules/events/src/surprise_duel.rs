//! `event:意外的对邦` -- 事件卡「意外的对邦」（中立事件 A19）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `意外的对邦`）:
//! > 将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌。所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动（如果距离一样则向正常方向移动）。

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, FilterKind};
use card_sdk::{Action, CardDef, Filter, Msg, On, PlanValue};

use crate::util::{expire, keep};

const ID: &str = "意外的对邦";

/// The player who drew the event, on the instance's props.
const DRAWER: &str = "drawer";
/// How many of the drawer's own turn starts have passed.
const TURNS: &str = "turns";

pub const SURPRISE_DUEL: CardDef = CardDef::new(
    "event:意外的对邦",
    &[
        On::Play("", None, None, play),
        // 「所有其他玩家的移动方向」 -- a non-drawer mover. `always` deleted
        // (a no-op prefetch); the drawer relation is the residual guard (the
        // instance prop is outside the CEL schema).
        On::Filter(Filter::DiceRollPlan("", Some(on_plan_others), on_plan)),
        On::Action(Action::TurnStartBefore("", None, None, on_turn_start)),
    ],
)
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"意外的对邦"#)
            .text(r#"将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌。所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动（如果距离一样则向正常方向移动）。"#)
            // sheet cell A19
            .derived(false));

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

/// 「所有其他玩家」 -- the plan's mover is not the drawer. The drawer lives on
/// the instance's props (outside the CEL schema, docs/GUARDS.md §4.2c), so this
/// is the residual guard of [`on_plan`].
fn on_plan_others(_player_id: i32) -> bool {
    let mover = ctx::turn_player();
    let drawer = ctx::prop(DRAWER);
    mover >= 0 && mover != drawer
}

/// 规则书: 「所有其他玩家的移动方向改为向抽到的玩家绝对距离最近的方向移动
/// （如果距离一样则向正常方向移动）」 -- at `DiceRollPlan`, a non-drawer mover whose
/// reverse path is strictly closer to the drawer's seat walks backwards; a tie
/// keeps the normal direction. Only the direction is rewritten, not the length.
/// Applicability is [`on_plan_others`].
fn on_plan(_player_id: i32, mut v: PlanValue) -> PlanValue {
    let mover = ctx::turn_player();
    let drawer = ctx::prop(DRAWER);
    let here = ctx::player_pos(mover);
    let there = ctx::player_pos(drawer);
    let n = ctx::tile_count();
    if here < 0 || there < 0 || n <= 0 {
        return v;
    }
    // Forward steps from `here` to `there`; the wrap-around the other way is
    // `n - f`. `f == 0` (same tile) leaves the normal direction alone.
    let f = ctx::tile_forward(here, there);
    let b = n - f;
    if f > 0 && b < f {
        v.reverse = Some(true);
        ctx::log(
            mover,
            &Msg::new("log.event.surprise_duel_dir").player_id("who", mover),
        );
    }
    v
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

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_plan_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    on_plan(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    on_turn_start(player_id)
}
