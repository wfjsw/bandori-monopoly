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
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const DETOUR: CardDef = CardDef {
    id: "AG:回家的路上绕个道",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    why_not: Some(why_not),
};

/// C# `CardDetour.WhyNot` -> `H.MoveWhyNot`: refuses off the main move.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- C# `H.MoveWhyNot` refuses
    // when it is not the seat's turn.
    if ctx::turn_seat() != seat {
        return Some(Msg::new(key!("detour_not_your_turn")));
    }
    // TODO(规则书(2)): the rest of `H.MoveWhyNot` (`H._turnCtx.MainMoved` ->
    // 「这回合已经移动过了」, `State.skipMove` -> 「本回合不能移动」) is still
    // missing from the vocabulary, so those refusals cannot be expressed yet.
    None
}

fn can_react(seat: i32) -> bool {
    // 规则书(1): 「此卡可作为反击使用」 -- C# reacts on the seat's own non-teleport
    // move roll (`t.Kind == "moveRoll" && t.Seat == seat && !t.Move.Teleport`).
    trigger::kind() == TriggerKind::MoveRoll
        && trigger::seat() == seat
        && trigger::move_roll().is_some()
    // TODO(ABI): `t.Move.Teleport` is not in the trigger vocabulary, so a teleport
    // roll also opens this reaction (C# refuses those).
}

fn play(seat: i32) {
    // 规则书(2): 「进行一次 "afterglow"式的移动（视为使用一次初始Afterglow角色的（2）效果）」
    // -- the C# asks which of the four starter-Afterglow movement plans to arm:
    // Ran (reverse + no CiRCLE reward), Tomoe (start 10 tiles toward 银河拉面馆),
    // Himari (odd / even steps only).
    let pick = ctx::ask_pick(
        seat,
        &Msg::new(key!("detour_title")),
        &Msg::new(key!("detour_ask")),
        &[
            Msg::new(key!("detour_opt_reverse")),
            Msg::new(key!("detour_opt_tomoe")),
            Msg::new(key!("detour_opt_odd")),
            Msg::new(key!("detour_opt_even")),
        ],
    );
    // TODO(规则书(2)): the four plans mutate the turn's `MoveCtx`
    // (`plan.Reverse` / `plan.NoCircleReward` / `plan.Start` / `plan.Parity`) --
    // needs the movement-plan state and the H.CardMove main-move routine
    // (C# `CardDetour.Play`). Until then the choice is only logged.
    let _ = pick;
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- C# `plan.Base = { (1, 6) }`
    // replaces this turn's move dice, and the reaction path attaches a
    // `NextRollFx` for the next main roll.
    // TODO(规则书(2)): needs the move-dice plan (`MoveCtx.Base`) and the
    // `H.ExtraOf<NextRollFx>` attachment hook.
    ctx::log(seat, &Msg::new(key!("detour_planned")).seat("who", seat));
}

fn react(seat: i32) {
    let Some(_before) = trigger::move_roll() else { return };
    // 规则书(2): 「进行一次 "afterglow"式的移动」 -- the reaction path is the Ran
    // branch only: reverse the move and drop the CiRCLE reward (C#
    // `move.Reverse = !move.Reverse; move.NoCircleReward = true`).
    // TODO(规则书(2)): needs `MoveCtx.Reverse` / `MoveCtx.NoCircleReward` on the
    // trigger's move payload.
    // 规则书(2): 「并将下一次的移动掷骰变更为1d6」 -- C# `H.ExtraOf<NextRollFx>`
    // replaces the next main roll's dice plan with 1d6.
    // TODO(规则书(2)): needs the `H.ExtraOf` / NextRollFx attachment (a persistent
    // one-shot dice-plan override).
    ctx::log(seat, &Msg::new(key!("detour_react")).seat("who", seat));
}