//! `RAS:Repaint` -- C# `CardRepaint` (MatchHost.cs:9851-9903).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Repaint`）:
//! > Repaint：
//! > [反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const REPAINT: CardDef = CardDef {
    id: "RAS:Repaint",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// `CardRepaint.OnPath` -- tiles of `me` on `them`'s planned path.
///
/// 规则书: 「X为对方原本预计路径上你拥有的格子数」
///
/// The moveRoll trigger only fires from the engine's `MainMove`, which always
/// walks forward (`Move.dir = 1`), so the planned path is `1..=roll` steps ahead
/// of the mover's pre-move tile (C# `m.Dir * i`).
fn on_path(me: i32, them: i32, roll: i32) -> i32 {
    let steps = roll.abs();
    let mut count = 0;
    for i in 1..=steps {
        let t = ctx::tile_steps_ahead(them, i);
        if t >= 0 && ctx::tile_owner(t) == me {
            count += 1;
        }
    }
    count
}

/// 规则书: 「[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡」
fn can_react(seat: i32) -> bool {
    // 规则书: 「当任意其他玩家进行移动掷骰」 -- a move-roll window by someone else.
    if !matches!(trigger::kind(), TriggerKind::MoveRoll) {
        return false;
    }
    let them = trigger::seat();
    if them == seat {
        return false;
    }
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」 -- only worth playing when X > 0.
    match trigger::move_roll() {
        Some(r) if r > 0 => on_path(seat, them, r) > 0,
        _ => false,
    }
}

fn react(seat: i32) {
    let Some(roll) = trigger::move_roll() else { return };
    let them = trigger::seat();
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」
    let x = on_path(seat, them, roll);
    if x <= 0 {
        return;
    }
    // 规则书: 「使目标玩家的此次移动数-X」
    let now = (roll - x).max(0);
    trigger::set_move_roll(now);
    ctx::log(
        seat,
        &Msg::new(key!("repaint_cut")).seat("who", them).i("n", x as i64).i("roll", now as i64),
    );
    // TODO(规则书): 「对方此次结算的支付减半」 -- needs the MoveCtx.PayFactor hook
    // (C# `m.PayFactor *= 0.5`) so the settle of this move pays half.
    // TODO(规则书): C# declares `Targeting => true; SingleTarget => true` and
    // confirms with `H.Target(c, m.Seat, ...)` -- needs the H.Target targeting
    // prompt / card targeting flags (and EXIST's redirect depends on them).
}