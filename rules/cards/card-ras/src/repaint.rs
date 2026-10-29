//! `RAS:Repaint` -- C# `CardRepaint` (MatchHost.cs:9851-9903).
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Repaint`）:
//! > Repaint：
//! > [反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。
//!

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const REPAINT: CardDef = CardDef::new(
    "RAS:Repaint",
    &[
        // 规则书: 「[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡」
        // -- the only play condition is another player's move roll; X = 0 is
        // still a legal play (the halving stands on its own).
        On::Counteract(
            &[ChainKind::DiceRollResult],
            "actor != owner && move.kind != Teleport && move.roll != null",
            None,
            counteract,
        )
        .counter_reason(key!("repaint_when")),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Repaint"#)
        .raw_name(r#"Repaint"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。"#))
        // sheet cell H11
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

/// `CardRepaint.OnPath` -- tiles of `me` on `them`'s planned path.
///
/// 规则书: 「X为对方原本预计路径上你拥有的格子数」
///
/// The roll window also opens from a card-driven move (`card_move`), and the
/// move is not necessarily forward: `move.dir` is -1 on a reversed move
/// (`plan::set_reverse`), so the path is `1..=roll` steps along the move's own
/// direction from the mover's pre-move tile (C# `m.Dir * i`).
fn on_path(me: i32, them: i32, roll: i32) -> i32 {
    let steps = roll.abs();
    let dir = trigger::move_dir();
    let mut count = 0;
    for i in 1..=steps {
        let t = ctx::tile_steps_ahead(them, dir * i);
        if t >= 0 && ctx::tile_owner(t) == me {
            count += 1;
        }
    }
    count
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // `actor != owner && move.kind != Teleport && move.roll != null` is the pre.
    // TODO(规则书)（当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡）: a
    //   counter link carries no move face (`move.roll` is null there), so we can
    //   only answer the root roll and never build on another counter's cut.
    let Some(roll) = trigger::move_roll() else {
        return Ok(());
    };
    let them = trigger::player_id();
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」
    let x = on_path(player_id, them, roll);
    // C# `H.Target(c, m.Seat, r)` then `if (r.yes && r.index == m.Seat)` -- the
    // mover must still be the hit (EXIST's redirect can move it) and the
    // `target` [反击] window must not have cancelled it.
    // TODO(规则书)（使目标玩家的此次移动数-X）: 「目标玩家」 is the mover and
    //   「对方」 the same player; a redirect splits the two -- C# then skips both
    //   effects and so do we, though the mover still settles.
    // 规则书: 「使目标玩家的此次移动数-X」
    let hit = match ctx::target(them) {
        Some(h) => h,
        None => return Ok(()),
    };
    if hit != them {
        return Ok(());
    }
    let now = (roll - x).max(0);
    // TODO(规则书)（使目标玩家的此次移动数-X）: the book cuts the move itself,
    //   but 「直至结果至少为10」 floors are applied again after this window, so a
    //   cut can be partly undone -- we do not re-cut after the floor.
    trigger::set_move_roll(now);
    ctx::log(
        player_id,
        &Msg::new(key!("repaint_cut"))
            .player_id("who", them)
            .i("n", x as i64)
            .i("roll", now as i64),
    );
    // 规则书: 「对方此次结算的支付减半」 -- the settle of this move pays half
    // (milli-units: 500 = x0.5).
    // TODO(规则书)（对方此次结算的支付减半）: C# folds with `m.PayFactor *= 0.5`
    //   -- we `set_pay_factor`, which replaces, so two halvings on one move do
    //   not compose (the plan factor has no read export).
    ctx::plan::set_pay_factor(500);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
