//! `AG:回家的路上绕个道` -- C# `CardDetour` (MatchHost.cs:1286-1352):
//! an Afterglow-style move plan (or reverse a move) plus next roll 1d6.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:回家的路上绕个道`）:
//! > 回家的路上绕个道：
//! > (1)此卡可作为反击使用
//! > (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的
//! > （2）效果），并将下一次的移动掷骰变更为1d6
//!

use card_sdk::abi::{TriggerKind, ChainKind, MoveKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const DETOUR: CardDef = CardDef::new("AG:回家的路上绕个道", &[
    On::Play(play),
    On::CantPlay(cant_play),
    On::React(&[ChainKind::MoveRoll], can_react, react),
]);

/// C# `CardDetour.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- the move is the turn's main
    // move, so the C# `H.MoveWhyNot` gate applies (own turn, main move still
    // available, turn's move not skipped).
    ctx::cant_move(player_id)
}

fn can_react(player_id: i32) -> bool {
    // 规则书(1): 「此卡可作为反击使用」 -- C# reacts on the player's own non-teleport
    // move roll (`t.Kind == "moveRoll" && t.Seat == seat && !t.Move.Teleport`).
    trigger::kind() == TriggerKind::MoveRoll
        && trigger::player_id() == player_id
        && trigger::move_roll().is_some()
        && trigger::move_kind() != Some(MoveKind::Teleport)
}

fn play(player_id: i32) {
    // 规则书(2): 「进行一次 "afterglow"式的移动（视为使用一次初始Afterglow角色的（2）效果）」
    // -- the C# asks which of the four starter-Afterglow movement plans to arm:
    // Ran (reverse + no CiRCLE reward), Tomoe (start 10 tiles toward 银河拉面馆),
    // Himari (odd / even steps only).
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
    );
    // 规则书(2): the four plans mutate the turn's `MoveCtx`
    // (C# `H._turnCtx.Plan`: `Reverse` / `NoCircleReward` / `Start` / `Parity`).
    match pick {
        0 => {
            // Ran: `plan.Reverse = !plan.Reverse; plan.NoCircleReward = true`.
            ctx::plan::set_reverse(ctx::plan::dir() >= 0);
            ctx::plan::set_no_circle_reward(true);
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
    // The reaction path's `NextRollFx` half is separate (below).
    ctx::plan::set_base_dice(1, 6, "（回家的路上绕个道）");
    ctx::log(player_id, &Msg::new(key!("detour_planned")).player_id("who", player_id));
}

fn react(player_id: i32) {
    let Some(_before) = trigger::move_roll() else { return };
    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- the reaction path is the Ran
    // branch only: reverse the move and drop the CiRCLE reward (C#
    // `move.Reverse = !move.Reverse; move.NoCircleReward = true` on the
    // trigger's in-flight move). `ctx::plan::*` shapes the plan, not this move,
    // and the trigger payload has no reverse / no-circle writeback.
    // TODO(规则书(2)): `MoveCtx.Reverse` / `MoveCtx.NoCircleReward` on the
    // trigger's move payload -- unmapped half: the walk has not started yet
    // (moveRoll runs before `walk`) but nothing flips those fields on the live
    // move.
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- C# `H.ExtraOf<NextRollFx>`
    // replaces the next main roll's dice plan with 1d6 (its `RollPlan` runs
    // `m.Base.Clear(); m.Base.Add(Dice)` on that move).
    // TODO(规则书(2)): needs the `H.ExtraOf` / NextRollFx attachment (a persistent
    // one-shot dice-plan override) -- unmapped half: `set_base_dice` shapes the
    // plan being built, and this reaction runs on an in-flight roll (the plan's
    // dice have already been consumed).
    ctx::log(player_id, &Msg::new(key!("detour_react")).player_id("who", player_id));
}