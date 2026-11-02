//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:即使迷茫着`）:
//! > 即使迷茫着：
//! >
//! > （1）[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。
//! > （2）[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。
//!
//! this card in play when another player's card hits you; a placed offer later
//! discards it to move as many tiles as your hand size.

use bandori_std::ctx;
use card_sdk::abi::{roll_source, ChainKind};
use card_sdk::{it, Action, Activation, CardText, Declared};
use card_sdk::{key, CardDef, Msg, On};

pub const EVEN_LOST: CardDef = CardDef::new(
    "MyGO:即使迷茫着",
    &[
        On::Counteract(
            &[ChainKind::Effect],
            // 规则书（1）: 「当你被其他人的卡的效果影响时」 -- another card's
            // effect, not your own (`by != owner`), and it must hit you.
            "by != owner && by >= 0 && effect.hits(owner)",
            None,
            counteract).counter_reason(key!("even_lost_when")),
        // 规则书（2）: 「主要阶段中」 -- the plan pass is exactly where the
        // movement phase opens, before the dice; the question is the declare,
        // so a 不发动 leaves no flash and no log.
        On::Action(Action::DiceRollPlan(
            "card.placed && actor == owner && move.is_main_move",
            Some(can_use),
            unused_body, // replaced by `declare_arg`
        ))
        .declare_arg(ask_use, use_it),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"即使迷茫着"#)
        .raw_name(r#"即使迷茫着"#)
        .tags(&[r#"反击"#, r#"持续"#])
        .text(CardText::items(&[
            it(r#"[反击] 当你被其他人的卡的效果影响时，你将此卡放置在自己场上。"#),
            it(r#"[持续] 主要阶段中，你可将此卡置入弃牌堆并进入移动阶段，使你的此次主要移动格数为你当前手牌张数。"#),
        ]))
        // sheet cell I2
        .fields(&[("group", r#"MyGO!!!!!"#), ("owner", "")]));

/// Constructor placeholder: `.declare_arg` replaces the body (docs/ACTIVATION.md §2).
const fn unused_body(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "MyGO:即使迷茫着",
        &Msg::new(key!("even_lost_note")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("even_lost_placed")).player_id("who", player_id),
    );
    Ok(())
}

fn can_use(player_id: i32) -> bool {
    // 「主要阶段中」 is the anchor; what can still shut the move is the movement
    // gate ([停留] / [晕眩] / [除外]), and another offer's claim. The gate is
    // spelled with the same three status hosts the engine's own clamp uses
    // (`stay.rs` / `stun.rs` / `exile.rs`; `unstoppable` exempts stay only,
    // rules.txt 51).
    if ctx::plan::is_claimed() || ctx::fixed_roll().is_some() {
        return false;
    }
    if ctx::hand_size(player_id) <= 0 {
        return false;
    }
    rules_status::stun::get(player_id) == 0
        && (rules_status::stay::get(player_id) == 0
            || rules_status::unstoppable::get(player_id) > 0)
        && rules_status::exile::get(player_id) == 0
}

/// 「你可将此卡置入弃牌堆并进入移动阶段」 -- the question is the declare, so a
/// 不发动 leaves no flash and no log. The face is taken here and carried as
/// the arg: nothing between the answer and the body changes the hand.
fn ask_use(player_id: i32) -> Declared<i32> {
    let n = ctx::hand_size(player_id);
    if ctx::ask_yes(
        player_id,
        &Msg::new(key!("even_lost_title")),
        &Msg::new(key!("even_lost_ask")).i("n", n as i64),
    )? {
        Ok(Activation::Arg(n))
    } else {
        Ok(Activation::Decline)
    }
}

fn use_it(player_id: i32, n: i32) -> card_sdk::Asked {
    ctx::plan::claim();
    // 「将此卡置入弃牌堆」 -- this instance, through the run's bound uid.
    ctx::set_dest(ctx::Dest::Graveyard);
    // 「使你的此次主要移动格数为你当前手牌张数」 -- the face *is* the hand size
    // (no die is thrown), and the step count is pinned to it so a 掷骰值
    // modifier cannot change the printed 格数.
    ctx::set_fixed_roll(n, roll_source::CARD);
    ctx::plan::set_steps(n);
    ctx::log(player_id, &Msg::new(key!("even_lost_go")).i("n", n as i64));
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn go_kani(player_id: i32) -> card_sdk::Asked {
    use_it(player_id, ctx::hand_size(player_id))
}

/// The declare's body arm. Its arg is whatever the declare answered, so the
/// caller supplies it.
#[cfg(kani)]
#[inline(always)]
pub fn use_it_kani(player_id: i32, n: i32) -> card_sdk::Asked {
    use_it(player_id, n)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
