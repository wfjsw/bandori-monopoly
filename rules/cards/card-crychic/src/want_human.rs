//! `CRYCHIC:想要成为人类` -- C# `CardWantHuman` (MatchHost.cs:2502-2583): place the
//! card, declare X in 1-20, bank crystals when your move roll is under X.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:想要成为人类`）:
//! > 想要成为人类
//! > ：
//! > （1）将此卡置于场上并从1-20间选择并声明X，将其写下。每当你的移动掷骰小于X，为此卡添加一个奇迹水晶。
//! > （2）[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部奇迹水晶并使你下次的移动掷骰结果额外增加20-X；若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const WANT_HUMAN: CardDef = CardDef {
    id: "CRYCHIC:想要成为人类",
    play: Some(want_human),
    can_react: None,
    react: None,
    why_not: None,
};

/// Where the declared X is written down (C# `CardWantHuman.Mem["x"]`).
/// A per-seat slot stands in for the per-field-card `Mem` map; 0 means
/// "never declared" (the C# default X is 10).
pub(crate) const SLOT_X: &str = "want_human_x";

fn want_human(seat: i32) {
    // 规则书（1）: 「将此卡置于场上」 -- C# `H.PlaceFromPlay(c)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "CRYCHIC:想要成为人类", &Msg::new(key!("want_human_note")));
    // 规则书（1）: 「从1-20间选择并声明X，将其写下」 -- C# `H.AskNumber(..., 1, 20, ...)`.
    let x = ctx::ask_number(
        seat,
        &Msg::new(key!("want_human_title")),
        &Msg::new(key!("want_human_ask")),
        1,
        20,
    );
    // 规则书（1）: 「将其写下」 -- `Mem["x"]`; the seat slot is the stand-in.
    ctx::set_slot(seat, SLOT_X, x);
    ctx::log(seat, &Msg::new(key!("want_human_declared")).seat("who", seat).i("n", x as i64));
    // TODO(ABI): （1） 「每当你的移动掷骰小于X，为此卡添加一个奇迹水晶。」
    //   -- needs the Fx.RollAfter persistent hook (C# `CardWantHuman.RollAfter`:
    //   `m.Roll < X` -> `AddCrystals(1)`) and a per-field-card crystal counter
    //   (C# `Card.Crystals` / `card_crystals`; the ABI only has `band_crystals`
    //   and seat tokens, not per-card crystals).
    // TODO(ABI): （2） 「[自动]若你的回合开始时此卡上拥有两个或以上的奇迹水晶，移除此卡上全部
    //   奇迹水晶并使你下次的移动掷骰结果额外增加20-X」 -- needs the Fx.TurnStart
    //   hook (C# `CardWantHuman.TurnStart`: drain the crystals, set `Mem["boost"]`)
    //   and the same Fx.RollAfter hook to add `20 - X` to the boosted move roll.
    // TODO(ABI): （2） 「若该次移动过程中受到异常移动效果影响，为此卡添加两个奇迹水晶。」
    //   -- needs the Fx.TurnEnd hook plus the abnormal-move counter
    //   (C# `CardWantHuman.TurnEnd` vs `H._abnormalTurn[Seat]`) and the
    //   per-card crystal counter.
}