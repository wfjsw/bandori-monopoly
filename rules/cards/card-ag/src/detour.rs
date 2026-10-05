//! `AG:回家的路上绕个道` -- C# `CardDetour` (MatchHost.cs:1286-1352):
//! an Afterglow-style move plan (or reverse a move) plus next roll 1d6.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:回家的路上绕个道`）:
//! > 回家的路上绕个道：
//! > (1)此卡可作为反击使用
//! > (2)进行一次 “afterglow”式的移动（视为使用一次初始Afterglow角色的
//! > （2）效果），并将下一次的移动掷骰变更为1d6
//!

use card_sdk::abi::TriggerKind;
use card_sdk::abi::MoveKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const DETOUR: CardDef = CardDef::new("AG:回家的路上绕个道", &[
    On::Play(play),
    On::CantPlay(cant_play),
    On::React(&[TriggerKind::MoveRoll], can_react, react),
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
            // TODO(规则书(2)): Tomoe -- `plan.Start = (pos + toward * 10) % n`
            // (10 tiles toward 银河拉面馆) + `plan.StartWhy`. `MoveCtx.start`
            // exists but `ctx::plan::*` has no `set_start` write, so this half
            // stays held.
        }
    }
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- C# `plan.Base = { (1, 6) }`
    // replaces this turn's move dice. The 1d6 face is pinned via `set_fixed_roll`
    // (`Plan.FixedRoll`, consumed at the roll) -- the kaoru_thief pattern for a
    // single-die base. The reaction path's `NextRollFx` half is separate (below).
    let n = ctx::roll(player_id, 1, 6);
    ctx::set_fixed_roll(n);
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
    // replaces the next main roll's dice plan with 1d6.
    // TODO(规则书(2)): needs the `H.ExtraOf` / NextRollFx attachment (a persistent
    // one-shot dice-plan override) -- unmapped half: `set_fixed_roll` is
    // per-turn and this turn's roll has already been consumed.
    ctx::log(player_id, &Msg::new(key!("detour_react")).player_id("who", player_id));
}