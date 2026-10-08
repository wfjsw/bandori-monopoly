//! `AG:回家的路上绕个道` -- C# `CardDetour` (MatchHost.cs:1286-1352):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:回家的路上绕个道`）:
//! > 回家的路上绕个道：
//! > (1)此卡可作为反击使用
//! > (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的
//! > （2）效果），并将下一次的移动掷骰变更为1d6
//!
//! an Afterglow-style move plan (or reverse a move) plus next roll 1d6.

use card_sdk::abi::{prop, ChainKind, MoveKind, TriggerKind};
use card_sdk::ctx::plan;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const DETOUR: CardDef = CardDef::new(
    "AG:回家的路上绕个道",
    &[
        On::Play(Some(cant_play), play, ""),
        // G4: kind (MoveRoll) is the category; `mine` + move fields are the
        // condition. Residual guard deleted -- nothing left.
        On::Counteract(&[ChainKind::MoveRoll], None, counteract, "actor == owner && move.roll != null && move.kind != Teleport"),
        On::RollPlan(next_roll),
        On::AtEnd(clear_no_reward),
    ],
)
    .legacy(&[(1, legacy_can_counteract)]);

/// G3 audit (GUARDS.md §5.1): the pre-migration guard.
fn legacy_can_counteract(player_id: i32) -> bool {
    trigger::kind() == TriggerKind::MoveRoll
        && trigger::player_id() == player_id
        && trigger::move_roll().is_some()
        && trigger::move_kind() != Some(MoveKind::Teleport)
}

/// C# `CardDetour.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- the move is the turn's main
    // move, so the C# `H.MoveWhyNot` gate applies (own turn, main move still
    // available, turn's move not skipped).
    ctx::cant_move(player_id)
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书(2): 「进行一次 "afterglow"式的移动（视为使用一次初始Afterglow角色的（2）效果）」
    // -- the C# asks which of the four starter-Afterglow movement plans to arm:
    // Ran (reverse + no CiRCLE reward), Tomoe (start 10 tiles toward 银河拉面馆),
    // Himari (odd / even steps only).
    // TODO(规则书)[judgement]: the whole of 「"afterglow"式的移动」 resolves through
    //   「初始Afterglow角色」, which the book never names. The four-way pick below is
    //   the C# reading, not the text's -- see the judgement block at the foot of
    //   this file.
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("detour_title")),
        &Msg::new(key!("detour_ask")),
        &[
            Msg::new(key!("detour_opt_reverse")),
            Msg::new(key!("detour_opt_tomoe")),
            Msg::new(key!("detour_opt_odd")),
            Msg::new(key!("detour_opt_even")),
        ],
    )?;
    // 规则书(2): the four plans mutate the turn's `MoveCtx`
    // (C# `H._turnCtx.Plan`: `Reverse` / `NoCircleReward` / `Start` / `Parity`).
    match pick {
        0 => {
            // Ran: `plan.Reverse = !plan.Reverse; plan.NoCircleReward = true`.
            ctx::plan::set_reverse(ctx::plan::dir() >= 0);
            arm_no_reward(player_id);
        }
        2 | 3 => {
            // Himari: `plan.Parity = (pick == 2) ? 1 : 0` (odd tiles / even tiles).
            ctx::plan::set_parity(if pick == 2 { 1 } else { 0 });
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

fn counteract(player_id: i32) -> card_sdk::Asked {
    let Some(_before) = trigger::move_roll() else {
        return Ok(());
    };
    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- the counteraction path only reverses
    // the in-flight move and drops the CiRCLE reward (C#
    // `move.Reverse = !move.Reverse; move.NoCircleReward = true` on the
    // trigger's in-flight move). `moveRoll` runs before `walk`, so the plan is
    // still live and `ctx::plan::*` is the write.
    // TODO(规则书)[judgement]: which 「afterglow」 plan the counter is. The C# logs
    //   this one 「羽泽鸫式」 while implementing 美竹兰's reverse; 羽泽鸫's （2） also
    //   grants 「你的下回合开始时，进行一次双倍掷骰的移动」, which neither does. See the
    //   judgement block at the foot of this file.
    ctx::plan::set_reverse(true);
    arm_no_reward(player_id);
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- armed here, consumed by the
    // `RollPlan` hook below when the next plan is being built. One-shot: the hook
    // clears it so it does not rewrite the roll after that.
    ctx::state::set(player_id, NEXT_ROLL, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("detour_counteract")).player_id("who", player_id),
    );
    Ok(())
}

/// Armed when the counteraction fired; the next plan is the one it rewrites.
const NEXT_ROLL: &str = "detour_next_roll";

/// 规则书(2): 「下一次的移动掷骰变更为1d6」 -- `RollPlan` is the moment the dice
/// plan is being built, so this is where the override belongs. `set_base_dice`
/// replaces the table; 1d6 is the clause's.
fn next_roll(player_id: i32) -> card_sdk::Asked {
    if ctx::state::get(player_id, NEXT_ROLL) == 0 {
        return Ok(());
    }
    ctx::state::set(player_id, NEXT_ROLL, 0);
    plan::set_base_dice(1, 6, "detour");
    Ok(())
}

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

/// The clause ends at the turn end -- disarm the veto this card armed.
fn clear_no_reward(player_id: i32) -> card_sdk::Asked {
    let circle = ctx::tile_named("CiRCLE");
    if circle >= 0 {
        ctx::set_tile_prop(circle, prop::NO_REWARD, 0);
    }
    Ok(())
}
