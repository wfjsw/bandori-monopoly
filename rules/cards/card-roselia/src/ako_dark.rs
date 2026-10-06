//! `R:（亚子）黑暗大魔姬亚子` -- C# `CardAkoDark` (MatchHost.cs:10781-10814): [反击]
//!
//! 规则书（docs/rulebook/cards.json, id `R:（亚子）黑暗大魔姬亚子`）:
//! > （亚子）黑暗大魔姬亚子： 
//! >  
//! > （1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。若此卡从手牌以外的地方打出，你抽一张卡。
//!
//! just before you pay another player: take a [眩晕] layer instead of paying.

use card_sdk::abi::{ChainKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const AKO_DARK: CardDef = CardDef::new(
    "R:（亚子）黑暗大魔姬亚子",
    &[On::Counteract(&[ChainKind::Effect], can_counteract, counteract)],
);

/// 规则书（1）[反击]: 「[反击] 当你即将向其他玩家支付资金时可打出此卡」
fn can_counteract(player_id: i32) -> bool {
    // 规则书（1）[反击]: 「当你即将向其他玩家支付资金时」 -- C# `t.Kind == "pay" &&
    // t.Pay.from == player && t.Pay.PayToOther`.
    if trigger::kind() != ChainKind::Effect || trigger::player_id() != player_id {
        return false;
    }
    // `t.Pay.PayToOther` -- the payee is another player (`t.Pay.to` = `trigger::target()`).
    let to = trigger::target();
    if to < 0 || to == player_id {
        return false;
    }
    // C# `!t.Pay.cancel` -- a payment an earlier counteraction already zeroed reads as
    // `value() == 0`, so the >0 guard covers it (cf. `set_pay_amount(0)` = cancel).
    trigger::value() > 0
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）[反击]: 「使自己获得一层[眩晕]」 -- C# `H.GiveStun(i, 1, i, "黑暗大魔姬亚子")`.
    ctx::give_stun(player_id, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("ako_dark_stun")).player_id("who", player_id),
    );
    // 规则书（1）[反击]: the pay is waived once the player is stunned (C# CounteractHint:
    // 「这笔就不用付了」) -- C# `c.Trigger.Pay.cancel = true` when `H.State.seats[i].Stunned`.
    if ctx::stun_of(player_id) > 0 {
        trigger::set_pay_amount(0);
    }
    // 规则书（1）[反击]: 「若此卡从手牌以外的地方打出，你抽一张卡」 -- the
    // play's origin: a card run through `ctx::play_card` did not come from a hand.
    if !ctx::play_from_hand() {
        ctx::draw(player_id, 1);
        ctx::log(
            player_id,
            &Msg::new(key!("ako_dark_drawn")).player_id("who", player_id),
        );
    }
    Ok(())
}
