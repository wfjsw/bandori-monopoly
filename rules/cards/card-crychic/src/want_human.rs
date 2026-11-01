//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:想要成为人类`）:
//! > 想要成为人类
//! >  ：
//! > （1）将此卡置于场上并从1-20间选择并声明X，将其写下。每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶。
//! > （2）[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。
//!
//! Sheet 2026-10-06 新卡组卡 M2: 「小于等于X」 (was 「小于X」) -- roll == X
//! also banks a crystal.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, TriggerKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On, RollValue};

pub const WANT_HUMAN: CardDef = CardDef::new(
    "CRYCHIC:想要成为人类",
    &[
        On::Play("", None, want_human),
        On::Action(Action::TurnStart("actor == owner && card.placed", None, counteract)),
        On::Action(Action::TurnEnd("actor == owner && card.placed", None, counteract)),
        // 规则书（1）: every one of your move casts is checked, rerolls included.
        On::Action(Action::DiceRollCast(
            "actor == owner && card.placed && roll_purpose == 1",
            None,
            on_cast,
        )),
        // 规则书（2）: the armed boost rides the Add stage, so a reroll re-folds
        // it instead of losing the already-applied `set_move_roll`.
        On::Filter(Filter::DiceRollAdd(
            "actor == owner && card.placed && roll_purpose == 1",
            Some(boost_armed),
            boost_add,
        )),
        // 规则书（2）: consume the one-shot boost and snapshot the abnormal
        // counter once the result is final (still before the movement itself).
        On::Action(Action::DiceRollSettled(
            "actor == owner && card.placed && roll_purpose == 1",
            None,
            on_settled,
        )),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"想要成为人类"#)
        .raw_name(r#"想要成为人类"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"将此卡置于场上并从1-20间选择并声明X，将其写下。每当你的移动掷骰小于等于X，为此卡添加一个奇迹水晶。"#),
            it(r#"[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。"#),
        ]))
        // sheet cell M2
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]));

/// Where the declared X is written down (`Mem["x"]`). A prop of the placed
/// instance stands in for the per-field-card `Mem` map; 0 means "never declared"
/// (the default X is 10).
pub(crate) const SLOT_X: &str = "want_human_x";

fn want_human(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "CRYCHIC:想要成为人类",
        &Msg::new(key!("want_human_note")),
    );
    let x = ctx::ask_number(
        player_id,
        &Msg::new(key!("want_human_title")),
        &Msg::new(key!("want_human_ask")).card("card", "CRYCHIC:想要成为人类"),
        1,
        20,
    )?;
    // 规则书（1）: 「将其写下」 -- `Mem["x"]`; an on-instance prop is the stand-in.
    // The props ride the instance just placed (`current_uid`), so two copies
    // keep their own X and the value dies with the card.
    ctx::set_prop(SLOT_X, x);
    // Fresh placement: no boost snapshot from a previous instance.
    ctx::set_prop(SLOT_AB_BEFORE, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("want_human_declared"))
            .player_id("who", player_id)
            .i("n", x as i64),
    );
    // (1) runs per cast (`on_cast`); (2)'s boost rides `boost_add` and is
    // consumed at settlement (`on_settled`); the arming and the abnormal
    // payoff run in `counteract` at `turnStart` / `turnEnd`.
    Ok(())
}

const SLOT_BOOST: &str = "want_human_boost";

const SLOT_AB_BEFORE: &str = "want_human_ab_before";

/// The `DiceRollCast` window. Every cast of one of your move rolls is checked
/// -- a reroll lands as another Cast, not a later `DiceRollAfter`.
///
/// 规则书（1）(sheet 2026-10-06 新卡组卡 M2): 「每当你的移动掷骰小于等于X，
/// 为此卡添加一个奇迹水晶。」 (was 「小于X」).
fn on_cast(player_id: i32) -> card_sdk::Asked {
    let roll = trigger::value();
    let x = ctx::prop(SLOT_X);
    if x > 0 && roll <= x {
        ctx::add_crystals(1, 0)?;
        ctx::log(
            player_id,
            &Msg::new(key!("want_human_crystal"))
                .player_id("who", player_id)
                .i("n", roll as i64),
        );
    }
    Ok(())
}

/// Residual guard for [`boost_add`]: a boost is armed (the prop holds the
/// boost + 1, so `armed - 1` may be 0).
fn boost_armed(_player_id: i32) -> bool {
    ctx::prop(SLOT_BOOST) > 0
}

/// 规则书（2）: the boost armed last turn start lands on this roll. A pure Add
/// body so every settle re-folds it after a reroll.
fn boost_add(player_id: i32, v: RollValue) -> RollValue {
    let boost = (ctx::prop(SLOT_BOOST) - 1).max(0);
    RollValue { value: v.value + boost, ..v }
}

/// 规则书（2）: consume the armed boost and snapshot the abnormal counter so
/// 「若该次移动过程中受到异常移动效果影响」 can be checked at turn end. The
/// result is final here, so a reroll can no longer replace the face the boost
/// was logged against.
fn on_settled(player_id: i32) -> card_sdk::Asked {
    let armed = ctx::prop(SLOT_BOOST);
    if armed <= 0 {
        return Ok(());
    }
    let boost = armed - 1;
    ctx::set_prop(SLOT_BOOST, 0);
    ctx::set_prop(SLOT_AB_BEFORE, ctx::abnormal_count(player_id) + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("want_human_boost"))
            .player_id("who", player_id)
            .i("n", boost as i64),
    );
    Ok(())
}

/// `TurnStart` / `TurnEnd`. Runs through the hook dispatch, so this is a field
/// effect, not a [反击]. `actor == owner && card.placed` is the pre; the arms
/// branch on kind.
fn counteract(player_id: i32) -> card_sdk::Asked {
    match trigger::kind() {
        // 规则书（2）: 「若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部
        // 奇迹水晶并使你下次的移动掷骰结果额外增加20-X。」
        TriggerKind::TurnStart => {
            if ctx::crystals() < 2 {
                return Ok(());
            }
            let x = ctx::prop(SLOT_X);
            let boost = (20 - x).max(0);
            ctx::set_crystals(0);
            // An earlier arming that never met its move targets the same
            // 「下次的移动掷骰」, so add to it; only an unarmed slot gains the
            // "+1 = armed" bit.
            let armed = ctx::prop(SLOT_BOOST);
            ctx::set_prop(
                SLOT_BOOST,
                if armed > 0 { armed + boost } else { boost + 1 },
            );
            ctx::log(
                player_id,
                &Msg::new(key!("want_human_drained"))
                    .player_id("who", player_id)
                    .i("n", boost as i64),
            );
        }
        // TODO(规则书)（若该次移动过程中受到异常移动效果影响）: 只该看该次移动 -- 这里是整回合的 `abnormal_count`
        // 在回合结束时比，移动之后、回合结束 之前落下的异常也会算数。
        TriggerKind::TurnEnd => {
            let ab_before = ctx::prop(SLOT_AB_BEFORE);
            if ab_before <= 0 {
                return Ok(());
            }
            ctx::set_prop(SLOT_AB_BEFORE, 0);
            if ctx::abnormal_count(player_id) > ab_before - 1 {
                ctx::add_crystals(2, 0)?;
                ctx::log(
                    player_id,
                    &Msg::new(key!("want_human_abnormal")).player_id("who", player_id),
                );
            }
        }
        _ => {}
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn want_human_kani(player_id: i32) -> card_sdk::Asked {
    want_human(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_cast_kani(player_id: i32) -> card_sdk::Asked {
    on_cast(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn boost_add_kani(player_id: i32, v: card_sdk::RollValue) -> card_sdk::RollValue {
    boost_add(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_settled_kani(player_id: i32) -> card_sdk::Asked {
    on_settled(player_id)
}
