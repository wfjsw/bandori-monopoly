//! `R:Sprechchor` -- C# `CardSprechchor`: roll 1d20, gain 1,000 + roll x 120.
//!
//! 规则书（docs/rulebook/cards.json, id `R:Sprechchor`）:
//! > Sprechchor：
//! >  在Livehouse地块开始回合时，可打出此卡并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const SPRECHCHOR: CardDef = CardDef::new("R:Sprechchor", &[On::Play(sprechchor)]);

fn sprechchor(player_id: i32) {
    // TODO: H.CardRoll honours PlayCtx.Extreme (forced max/min dice).
    let n = ctx::roll(player_id, 1, 20);
    ctx::gain(player_id, 1000 + n * 120, &Msg::new(key!("sprechchor_why")));
    // TODO(规则书): 「在Livehouse地块开始回合时」 -- the C# `CardSprechchor.WhyNot`
    //   refuses the play unless the **turn-start** tile (`H._turnSnap[player_id].pos`)
    //   is a buyable Livehouse; the vocabulary has no turn-snapshot position
    //   (`cant_play` stays `None`) and `H.IsLiveHouse`'s `Fx.ExtraColor` side is
    //   missing too. A mid-turn walk onto a Livehouse must not open the play.
}
