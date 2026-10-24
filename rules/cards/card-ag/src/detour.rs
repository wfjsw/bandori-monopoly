//! `AG:回家的路上绕个道` 
//!
//! 规则书（docs/rulebook/cards.json, id `AG:回家的路上绕个道`）:
//! > 回家的路上绕个道：
//! > (1)此卡可作为反击使用
//! > (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的
//! > （2）效果），并将下一次的移动掷骰变更为1d6
//!
//! an Afterglow-style move plan (or reverse a move) plus next roll 1d6.
//! 
//! Reviewed 10/23/2026

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, ChainKind, Dice};
use card_sdk::serde;
use card_sdk::{Activation, CardText, it};
use card_sdk::{key, Action, CardDef, Declared, Filter, Msg, On, PlanValue};

/// The four 「"afterglow"式的移动」 plans the C# offers, as the skill ids whose
/// press entry (`On::Play`, the character's （2）) each one is.
///
/// TODO(规则书)[judgement] OPEN-afterglow式移动
/// (`docs/rulebook/spec/OPEN-QUESTIONS.md:224`): 「初始Afterglow角色」 is never
/// named in the book, so which (2) is 「"afterglow"式的移动」 is still open.
/// These four stay the C# reading (Ran reverse / Tomoe start / Himari odd /
/// Himari even). The two Himari rows are the same skill -- the pick is the
/// parity, not a second character.
const PLANS: [&str; 4] = [
    "skill:美竹兰:叛逆的红挑染",
    "skill:宇田川巴:豚骨酱油拉面大姐",
    "skill:上原绯玛丽:大家一起迈出新的一步",
    "skill:上原绯玛丽:大家一起迈出新的一步",
];

pub const DETOUR: CardDef = CardDef::new(
    "AG:回家的路上绕个道",
    &[
        On::Play(
            "",
            None,
            // Gate: only allow play if the move is not blocked, and some
            // candidate (2) is activatable (docs/ACTIVATION.md §3 -- the
            // card's own activation depends on the skill's gate).
            Some(|player_id| {
                if let Some(why) = rules_status::move_blocked(player_id) {
                    return Some(why);
                }
                if !some_plan_fires(player_id) {
                    return Some(Msg::new(key!("detour_no_skill")));
                }
                None
            }),
            unused_body,
        )
        // The four-way pick is part of *declaring* the play, not of running it
        // -- the activation cannot be cancelled once `hand.remove` commits
        // (docs/ACTIVATION.md §2). Decline = the card stays in hand.
        .declare_arg(pick_plan, play_with_plan),
        // G4: kind (DiceRollValue) is the category; `mine` + move fields are the
        // condition. Residual guard deleted -- nothing left.
        // > (1)此卡可作为反击使用
        On::Counteract(
            &[ChainKind::DiceRollResult], "actor == owner && move.roll != null && move.kind != Teleport",
            None,
            // TODO: Only handle the `blocked by status` case, not the `not_my_turn` and `already` case.
            Some(|player_id| {
                rules_status::move_blocked(player_id).is_none() && some_plan_fires(player_id)
            }),
            unused_body,
        )
        .declare_arg(pick_plan, counteract_with_plan),
        // The body-top applicability check (`state::get(NEXT_ROLL) == 0`) is
        // the condition now: the routine runs -- and flashes -- only when the
        // counteraction armed it.
        // > (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的
        // > （2）效果），并将下一次的移动掷骰变更为1d6
        On::Filter(Filter::DiceRollPlan("slot('detour_next_roll') != 0", None, next_roll)),
        // One-shot consume of the armed next-roll rewrite, on the roll that the
        // plan just built (DiceRollAfter is the next event after DiceRollPlan).
        // > (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的
        // > （2）效果），并将下一次的移动掷骰变更为1d6
        On::Action(Action::DiceRollAfter("slot('detour_next_roll') != 0", None, None, consume_next_roll)),

    ],
)
// TODO: Move this .silent to the actual onlist
    .silent(&[3]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"回家的路上绕个道"#)
        .raw_name(r#"回家的路上绕个道"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::items(&[
            it(r#"此卡可作为反击使用"#),
            it(r#"进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的（2）效果），并将下一次的移动掷骰变更为1d6"#),
        ]))
        // sheet cell C9
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

/// Does some candidate 「(2)」 press entry admit right now? This is the
/// guard dependency the card's activation has on the skill's gate
/// (docs/ACTIVATION.md §3): 「视为使用一次初始Afterglow角色的（2）效果」 can only
/// fire when that (2) can fire.
///
/// Note this makes detour require a fire pot -- the Afterglow (2) gates read
/// `state_key::CHARGE`. That is the dependency asked for, not an accident.
fn some_plan_fires(player_id: i32) -> bool {
    PLANS
        .iter()
        .any(|id| ctx::query_guard(player_id, id, ctx::entry::PRESS))
}

/// The card's own activation argument -- private to this file
/// (docs/ACTIVATION.md §2). Not shared with any other card: only `postcard`
/// crosses, so the shape is ours to choose.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
struct DetourPick {
    /// Index into [`PLANS`].
    plan: i32,
    /// The tile the plan aims at, or -1.
    tile: i32,
    /// The seat it aims at, or -1.
    seat: i32,
}

/// Activation-argument collector (docs/ACTIVATION.md §2): which 「(2)」 to arm.
/// Options are exactly the candidates whose press entry admits -- the pick is
/// made before the activation commits, so a decline is still real.
fn pick_plan(player_id: i32) -> Declared<DetourPick> {
    // The label set is fixed and matches `PLANS` order; a candidate whose (2)
    // is not activatable is simply not offered. If none is, decline rather
    // than offering a pick that cannot work.
    let mut live: [bool; 4] = [false; 4];
    let mut any = false;
    for (i, id) in PLANS.iter().enumerate() {
        live[i] = ctx::query_guard(player_id, id, ctx::entry::PRESS);
        any |= live[i];
    }
    if !any {
        return Ok(Activation::Decline);
    }
    // Only offer the live plans, then map the pick back to a `PLANS` index.
    let keys = [
        key!("detour_opt_reverse"),
        key!("detour_opt_tomoe"),
        key!("detour_opt_odd"),
        key!("detour_opt_even"),
    ];
    let mut opts = alloc::vec::Vec::new();
    let mut map = alloc::vec::Vec::new();
    for i in 0..4 {
        if live[i] {
            opts.push(Msg::new(keys[i]));
            map.push(i);
        }
    }
    let k = ctx::ask_pick(
        player_id,
        &Msg::new(key!("detour_title")),
        &Msg::new(key!("detour_ask")),
        &opts,
    )?;
    Ok(Activation::Arg(DetourPick {
        plan: map[k.min(map.len() - 1)] as i32,
        tile: -1,
        seat: -1,
    }))
}

/// Constructor placeholder: `.declare_arg` replaces the body (docs/ACTIVATION.md §2).
const fn unused_body(_player_id: i32) -> card_sdk::Asked {
    Ok(())
}

fn play_with_plan(player_id: i32, pick: DetourPick) -> card_sdk::Asked {
    // 规则书(2): 「进行一次 "afterglow"式的移动（视为使用一次初始Afterglow角色的（2）效果）」
    // The pick was made at declaration time (`pick_plan`); `pick.plan` is a
    // [`PLANS`] index.
    // TODO(规则书)[judgement] OPEN-afterglow式移动: the four plans below are
    //   the C# reading, not the text's.
    // TODO: Call the skill card to apply the 2nd effect there. Switching to
    //   `ctx::invoke_skill` would make the skill body spend a fire pot and
    //   reset `REST_TURNS`, which the C# detour did not -- whether 「视为使用」
    //   pays the cost is an open ruling, so the plan mutations stay inlined.
    apply_plan(player_id, pick);
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- C# `plan.Base.Clear();
    // plan.Base.Add((1, 6, "（回家的路上绕个道）"))` (MatchHost.cs:1337-1338)
    // replaces this turn's move dice with 1d6 (`plan::set_base_dice` is that).
    // The counteraction path's `NextRollFx` half is separate (below).
    ctx::plan::set_base_dice(1, 6, "（回家的路上绕个道）");
    ctx::log(
        player_id,
        &Msg::new(key!("detour_planned")).player_id("who", player_id),
    );
    Ok(())
}

/// The four plans mutate the turn's `MoveCtx` (C# `H._turnCtx.Plan`).
/// `pick` is a [`PLANS`] index.
fn apply_plan(player_id: i32, pick: DetourPick) {
    match pick.plan {
        0 => {
            // Ran: `plan.Reverse = !plan.Reverse; plan.NoCircleReward = true`.
            ctx::plan::set_reverse(ctx::plan::dir() >= 0);
            arm_no_reward(player_id);
        }
        2 => {
            // Himari odd: `plan.Parity = 1`.
            ctx::plan::set_parity(1);
        }
        3 => {
            // Himari even: `plan.Parity = 0`.
            ctx::plan::set_parity(0);
        }
        _ => {
            // Tomoe: `plan.Start = (pos + toward * 10) % n` -- 10 tiles toward
            // 银河拉面馆 (C# `num3 = (Forward(pos, b) <= n - Forward(pos, b)) ? 1
            // : -1`), `plan.StartWhy = "回家的路上绕个道"` =
            // `plan::set_start(start, why)`.
            let n = ctx::tile_count();
            let pos = ctx::player_pos(player_id);
            let b = ctx::tile_named("银河拉面馆");
            if n > 0 && pos >= 0 && b >= 0 {
                let forward = ctx::tile_forward(pos, b);
                let toward = if forward <= n - forward { 1 } else { -1 };
                let start = (pos + toward * 10).rem_euclid(n);
                ctx::plan::set_start(start, "回家的路上绕个道");
            }
        }
    }
}

fn counteract_with_plan(player_id: i32, pick: DetourPick) -> card_sdk::Asked {
    let Some(_before) = trigger::move_roll() else {
        return Ok(());
    };

    // TODO: Call the skill card to apply the 2nd effect there.
    // This entire thing should splitted to a separate function to keep uniform to the
    // play handler.

    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- the pick was made at
    // declaration time (`pick_plan`); `pick` is a [`PLANS`] index. The counter
    // path writes the same `ctx::plan::*` the play path does -- `diceRollResult`
    // runs before `walk`, so the plan is still live.
    // TODO(规则书)[judgement]: which 「afterglow」 plan the counter is. The C# logs
    //   this one 「羽泽鸫式」 while implementing 美竹兰's reverse; 羽泽鸫's （2） also
    //   grants 「你的下回合开始时，进行一次双倍掷骰的移动」, which neither does. See the
    //   judgement block at the foot of this file.
    // TODO: This used to hard-code the reverse plan and diverged from `play`;
    //   it now shares `apply_plan`, which is a behaviour change on the
    //   counter path.
    apply_plan(player_id, pick);
    arm_no_reward(player_id);
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- armed here, consumed by the
    // `DiceRollPlan` hook below when the next plan is being built. One-shot: the hook
    // clears it so it does not rewrite the roll after that.

    // TODO: Such key we need to keep it scoped to this card.
    // TODO: This diverged from what `play` handles.
    ctx::state::set(player_id, NEXT_ROLL, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("detour_counteract")).player_id("who", player_id),
    );
    Ok(())
}

/// Armed when the counteraction fired; the next plan is the one it rewrites.
const NEXT_ROLL: &str = "detour_next_roll";

/// 规则书(2): 「下一次的移动掷骰变更为1d6」 -- `DiceRollPlan` is the moment the dice
/// plan is being built, so this is where the override belongs. Pure: only the
/// dice reshape. The one-shot latch is consumed by [`consume_next_roll`] on
/// `DiceRollAfter`.
fn next_roll(_player_id: i32, v: PlanValue) -> PlanValue {
    PlanValue {
        base: Some(Dice { count: 1, sides: 6 }),
        ..v
    }
}

/// Drop the one-shot [`NEXT_ROLL`] latch once the rewritten roll has landed.
fn consume_next_roll(player_id: i32) -> card_sdk::Asked {
    ctx::state::set(player_id, NEXT_ROLL, 0);
    Ok(())
}

// TODO: Remove
/// 「向后移动经过CiRCLE时不获得CiRCLE奖励」 -- the veto is `prop::NO_REWARD` on
/// the CiRCLE tile's `tile:circle` instance (`docs/TILES.md`), not a walk-plan
/// flag. This card is played from hand and leaves the field, so it cannot arm
/// the prop in a per-pass hook: it arms it here and [`clear_no_reward`]
/// disarms it at the turn end, when the clause 「进行一次 "afterglow"式的移动」
/// is over. The reward step consumes the prop, so a walk that does pass CiRCLE
/// disarms it early.
fn arm_no_reward(player_id: i32) {
    let circle = ctx::tile_named("CiRCLE");
    if circle < 0 {
        return;
    }
    ctx::set_tile_prop(circle, prop::NO_REWARD, 1);
    ctx::at_turn_end(player_id);
}

// TODO: Remove
/// The clause ends at the turn end -- disarm the veto this card armed.
fn clear_no_reward(player_id: i32) -> card_sdk::Asked {
    let circle = ctx::tile_named("CiRCLE");
    if circle >= 0 {
        ctx::set_tile_prop(circle, prop::NO_REWARD, 0);
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
// The arg-taking bodies' wrappers take the activation argument so a proof
// drives the real shape (docs/ACTIVATION.md §2; rules/kani BodyShape::ActionArg).
#[cfg(kani)]
#[inline(always)]
pub fn play_with_plan_kani(player_id: i32, pick: DetourPick) -> card_sdk::Asked {
    play_with_plan(player_id, pick)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_with_plan_kani(player_id: i32, pick: DetourPick) -> card_sdk::Asked {
    counteract_with_plan(player_id, pick)
}

#[cfg(kani)]
#[inline(always)]
pub fn pick_plan_kani(player_id: i32) -> card_sdk::Declared<DetourPick> {
    pick_plan(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn next_roll_kani(player_id: i32, v: card_sdk::PlanValue) -> card_sdk::PlanValue {
    next_roll(player_id, v)
}

#[cfg(kani)]
#[inline(always)]
pub fn consume_next_roll_kani(player_id: i32) -> card_sdk::Asked {
    consume_next_roll(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_no_reward_kani(player_id: i32) -> card_sdk::Asked {
    clear_no_reward(player_id)
}
