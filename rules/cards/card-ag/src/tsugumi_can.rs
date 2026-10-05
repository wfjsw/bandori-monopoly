//! `AG:（鸫）微小的『能做到』的事` -- C# `CardTsugumiCan` (MatchHost.cs:1818-1901):
//! play as @Tsugu ycm, or react to force a range-card's dice to max/min.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（鸫）微小的『能做到』的事`）:
//! > （鸫）微小的『能做到』的事：打出此卡时，选择其中一个效果发动：
//! > （1）【反击】时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳生效）
//! > （2）视为打出一张@Tsugu ycm
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const TSUGUMI_CAN: CardDef = CardDef {
    id: "AG:（鸫）微小的『能做到』的事",
    play: Some(play),
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

fn can_react(seat: i32) -> bool {
    // 规则书（1）【反击】: 「时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算」
    // -- C# reacts while another `RangeCard` play is resolving
    // (`t.Kind == "card" && t.Play.Def.RangeCard && t.Play.Extreme == 0`).
    if trigger::kind() != TriggerKind::Card {
        return false;
    }
    let _ = seat;
    // 规则书（1）: 「除@Tsugu ycm以外」 -- the exclusion reads the play's card id
    // off the trigger (C# `t.Play.Id` / `t.Card`).
    if trigger::card_is("通用:@Tsugu ycm") {
        return false;
    }
    // TODO(ABI): `t.Play.Def.RangeCard` / `t.Play.Extreme` are not in the
    // trigger vocabulary. Matching every `card` trigger over-fires (the
    // passage's example says YOLO yes, 「无论是何种颜色的夕阳」 no); the RangeCard
    // filter is what keeps those apart.
    true
}

fn play(seat: i32) {
    // 规则书（2）: 「视为打出一张@Tsugu ycm」 -- C# `Ycm.Play(c)` on a shared
    // `CardTsuguYcm` instance (id `通用:@Tsugu ycm`).
    ctx::log(seat, &Msg::new(key!("tsugumi_can_ycm")).seat("who", seat));
    ctx::play_card("通用:@Tsugu ycm", seat);
}

fn react(seat: i32) {
    // 规则书（1）【反击】: 「使任意结果只有数字区间的效果以理论最大值或最小值结算」
    // -- C# asks max/min and writes `target.Extreme = 1 / -1`.
    let pick = ctx::ask_pick(
        seat,
        &Msg::new(key!("tsugumi_can_title")),
        &Msg::new(key!("tsugumi_can_ask")),
        &[
            Msg::new(key!("tsugumi_can_max")),
            Msg::new(key!("tsugumi_can_min")),
        ],
    );
    // TODO(规则书（1）): 「以理论最大值或最小值结算」 -- needs `PlayCtx.Extreme`
    // (C# `target.Extreme = 1 / -1`) so the target play's dice resolve at their
    // theoretical extremes (`H.CardRoll`). Until then the choice is only logged.
    let _ = pick;
    ctx::log(seat, &Msg::new(key!("tsugumi_can_forced")).seat("who", seat));
    // TODO(规则书): the C# `WhyNot` delegates to `@Tsugu ycm` (C#
    // `CardTsuguYcm.WhyNot` -> `H._turnCtx.MainMoved` -> 「这回合已经移动过了」)
    // -- the main-move latch is still missing from the vocabulary, so the gate
    // stays unimplemented (`why_not: None`).
}