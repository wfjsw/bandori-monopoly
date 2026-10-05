//! `CRYCHIC:去唱卡拉ok吧` -- C# `CardKaraoke` (MatchHost.cs:2734-2748): arm this
//! turn's main move to roll up to 5 times and keep one face.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:去唱卡拉ok吧`）:
//! > 去唱卡拉ok吧：
//! >  进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const KARAOKE: CardDef = CardDef {
    id: "CRYCHIC:去唱卡拉ok吧",
    play: Some(karaoke),
    can_react: None,
    react: None,
    why_not: None,
};

fn karaoke(seat: i32) {
    // 规则书: 「进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数」
    // -- the C# arms `KaraokeFx` here (`H.ExtraOf<KaraokeFx>(c.Seat).Turn =
    // H.TurnKey`) and the rolls happen when the main move rolls.
    ctx::log(seat, &Msg::new(key!("karaoke_note")).seat("who", seat));
    // TODO(ABI): 「进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。」
    //   -- needs the Fx.RollAfter persistent hook (C# `KaraokeFx.RollAfter` ->
    //   `Sing`) on the main move of this turn: keep the move's first face, then
    //   `H.AskYes` to reroll (`H.DoMoveRoll`) up to 5 results, `H.AskPick` one of
    //   them into `m.Roll`, and let the normal move proceed with it. `ctx::roll`
    //   + `ctx::ask_yes` + `ctx::ask_pick` cover the prompts; `H.DoMoveRoll` and
    //   the `m.Roll` write are the missing pieces.
    // TODO(规则书): the C# `WhyNot` refuses the card outside the move phase
    // (`H.MoveWhyNot`) -- needs the `WhyNot` hook on `CardDef`.
}