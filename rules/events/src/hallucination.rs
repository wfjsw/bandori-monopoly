//! `event:幻觉来了` -- 事件卡「幻觉来了」（中立事件 A13）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `幻觉来了`）:
//! > 将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃牌。任何玩家进行投掷前在行动顺序的上一名玩家代替进行此次投掷（所有影响投掷的效果服从于原本进行投掷的玩家所收影响）。

use card_sdk::abi::{ActionKind, FilterKind, Seat};
use bandori_std::ctx::{self, trigger};
use card_sdk::{Action, CardDef, Filter, Msg, On, PlanValue};

use crate::util::{expire, keep};

const ID: &str = "幻觉来了";

/// The player who drew the event, on the instance's props.
const DRAWER: &str = "drawer";
/// How many of the drawer's own turn starts have passed.
const TURNS: &str = "turns";

pub const HALLUCINATION: CardDef = CardDef::new(
    "event:幻觉来了",
    &[
        On::Play("", None, None, play),
        // 「任何玩家进行投掷前」 -- every plan, so the category filter is the
        // whole applicability. `always` deleted (a no-op prefetch).
        On::Filter(Filter::DiceRollPlan("", None, on_plan)),
        On::Action(Action::TurnStartBefore("", None, None, on_turn_start)),
    ],
)
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"幻觉来了"#)
            .text(r#"将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃牌。任何玩家进行投掷前在行动顺序的上一名玩家代替进行此次投掷（所有影响投掷的效果服从于原本进行投掷的玩家所收影响）。"#)
            // sheet cell A13
            .derived(false));

/// 规则书: 「将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃牌」
/// -- keep the event and remember who drew it; the turn-start hook below files
/// it away on the drawer's next turn.
fn play(player_id: i32) -> card_sdk::Asked {
    keep();
    ctx::set_prop(DRAWER, player_id);
    ctx::set_prop(TURNS, 0);
    ctx::log(
        player_id,
        &Msg::new("log.event.hallucination_on").player_id("who", player_id),
    );
    Ok(())
}

/// The previous living seat in turn order (`行动顺序的上一名玩家`), wrapping.
fn prev_player(p: i32) -> i32 {
    let n = ctx::player_count();
    if n <= 0 {
        return p;
    }
    for i in 1..=n {
        let q = (p - i).rem_euclid(n);
        if q != p && !ctx::player_out(q) {
            return q;
        }
    }
    p
}

/// 规则书: 「任何玩家进行投掷前在行动顺序的上一名玩家代替进行此次投掷（所有
/// 影响投掷的效果服从于原本进行投掷的玩家所收影响）」 -- the roller is the
/// previous player; the move still belongs to the original one (and its
/// modifiers key off them), which is what `v.roller` does: the log line
/// shows 「by」, `t.Move.Main` / the mover are unchanged.
fn on_plan(_owner: i32, mut v: PlanValue) -> PlanValue {
    let who = trigger::player_id();
    if who < 0 {
        return v;
    }
    let by = prev_player(who);
    if by == who {
        return v;
    }
    // 「上一名玩家代替进行此次投掷」
    v.roller = Some(Seat(by));
    ctx::log(
        who,
        &Msg::new("log.event.hallucination_sub")
            .player_id("who", who)
            .player_id("by", by),
    );
    v
}

/// 规则书: 「抽到的玩家的下回合开始时放入事件弃牌」 -- count the drawer's own
/// turn starts and expire on the first (their next turn).
fn on_turn_start(_player_id: i32) -> card_sdk::Asked {
    if trigger::player_id() != ctx::prop(DRAWER) {
        return Ok(());
    }
    let n = ctx::prop(TURNS) + 1;
    ctx::set_prop(TURNS, n);
    if n >= 1 {
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
