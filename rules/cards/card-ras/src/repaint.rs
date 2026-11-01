//!
//! 规则书（docs/rulebook/cards.json, id `RAS:Repaint`）:
//! > Repaint：
//! > [反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。
//!

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ChainKind, PayFactorScope};
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

///
/// 规则书: 「X为对方原本预计路径上你拥有的格子数」
///
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
    // `actor != owner && move.kind != Teleport && move.roll != null` is the condition.
    // TODO(规则书)（当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡）: a
    //   counter link carries no move face (`move.roll` is null there), so we can
    //   only answer the root roll and never build on another counter's cut.
    let Some(roll) = trigger::move_roll() else {
        return Ok(());
    };
    let them = trigger::player_id();
    // 规则书: 「X为对方原本预计路径上你拥有的格子数」
    let x = on_path(player_id, them, roll);
    // TODO(规则书)（使目标玩家的此次移动数-X）: 规则书: 「使目标玩家的此次移动数-X」
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
    // (milli-units: 500 = x0.5), multiplied in and scoped `ThisMove` so it
    // covers the move in flight only.
    ctx::plan::multiply_pay_factor(500, PayFactorScope::ThisMove);
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
