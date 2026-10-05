//! `R:（亚子）黑暗大魔姬亚子` -- C# `CardAkoDark` (MatchHost.cs:10781-10814): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:（亚子）黑暗大魔姬亚子`）:
//! > （亚子）黑暗大魔姬亚子：
//! >
//! > （1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。
//!
//! just before you pay another player: take a [眩晕] layer instead of paying.

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const AKO_DARK: CardDef = CardDef {
    id: "R:（亚子）黑暗大魔姬亚子",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// 规则书（1）[反击]: 「[反击] 当你即将向其他玩家支付资金时可打出此卡」
fn can_react(seat: i32) -> bool {
    // 规则书（1）[反击]: 「当你即将向其他玩家支付资金时」 -- C# `t.Kind == "pay" &&
    // t.Pay.from == seat && t.Pay.PayToOther`.
    if trigger::kind() != TriggerKind::Pay || trigger::seat() != seat {
        return false;
    }
    // `t.Pay.PayToOther` -- the payee is another seat (`t.Pay.to` = `trigger::target()`).
    let to = trigger::target();
    to >= 0 && to != seat
    // TODO(规则书)（1）[反击]: the C# also refuses an already-cancelled pay
    //   (`!t.Pay.cancel`); the pay trigger carries no cancel flag.
}

fn react(seat: i32) {
    // 规则书（1）[反击]: 「使自己获得一层[眩晕]」 -- C# `H.GiveStun(i, 1, i, "黑暗大魔姬亚子")`.
    ctx::give_stun(seat, 1);
    ctx::log(seat, &Msg::new(key!("ako_dark_stun")).seat("who", seat));
    // TODO(规则书)（1）[反击]: the C# then cancels the in-flight pay
    //   (`c.Trigger.Pay.cancel = true`) now that the seat is stunned -- the
    //   pay-cancel is the card's real payoff (C# ReactHint: 「这笔就不用付了」) and
    //   needs a cancel-pay hook on the pay trigger. The stun layer alone does not
    //   reach the running payment.
    // 规则书（1）[反击]: 「若此卡从手牌以外的地方打出，你抽一张卡」
    // TODO(ABI): 「若此卡从手牌以外的地方打出，你抽一张卡」 -- needs `PlayCtx.FromDeck`
    //   (C# `if (c.FromDeck) H.DrawR(i, 1, ...)`); the reaction carries no origin
    //   flag.
}