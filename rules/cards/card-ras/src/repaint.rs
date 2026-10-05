//! `RAS:Repaint` -- C# `CardRepaint` (MatchHost.cs:9851-9903).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Repaint`）:
//! > Repaint：
//! > [反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。
//!

use card_sdk::abi::{TriggerKind, ChainKind, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const REPAINT: CardDef = CardDef::new("RAS:Repaint", &[
    On::React(&[ChainKind::MoveRoll], can_react, react),
]);

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
fn can_react(player_id: i32) -> bool {
    // 规则书: 「当任意其他玩家进行移动掷骰」 -- a move-roll window by someone else.
    if !matches!(trigger::kind(), TriggerKind::MoveRoll) {
        return false;
    }
    let them = trigger::player_id();
    if them == player_id {
        return false;
    }
    // C# `t.Move != null && !t.Move.Teleport && !t.Move.TeleportWalk` -- only a
    // walk has a planned path to count tiles on (the old TeleportWalk is just
    // `MoveKind::Teleport` now).
    if trigger::move_kind() == Some(MoveKind::Teleport) {
        return false;
    }
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」 -- only worth playing when X > 0.
    match trigger::move_roll() {
        Some(r) if r > 0 => on_path(player_id, them, r) > 0,
        _ => false,
    }
}

fn react(player_id: i32) {
    let Some(roll) = trigger::move_roll() else { return };
    let them = trigger::player_id();
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」
    let x = on_path(player_id, them, roll);
    if x <= 0 {
        return;
    }
    // C# `H.Target(c, m.Seat, r)` then `if (r.yes && r.index == m.Seat)` -- the
    // mover must still be the hit (EXIST's redirect can move it) and the
    // `target` [反击] window must not have cancelled it.
    let hit = match ctx::target(them) {
        Some(h) => h,
        None => return,
    };
    if hit != them {
        return;
    }
    // 规则书: 「使目标玩家的此次移动数-X」
    let now = (roll - x).max(0);
    trigger::set_move_roll(now);
    ctx::log(
        player_id,
        &Msg::new(key!("repaint_cut")).player_id("who", them).i("n", x as i64).i("roll", now as i64),
    );
    // 规则书: 「对方此次结算的支付减半」 -- C# `m.PayFactor *= 0.5`
    // (milli-units: 500 = x0.5); the settle of this move pays half.
    ctx::plan::set_pay_factor(500);
}