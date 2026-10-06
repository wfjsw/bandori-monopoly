//! `AG:（鸫）微小的『能做到』的事` -- C# `CardTsugumiCan` (MatchHost.cs:1818-1901):
//! play as @Tsugu ycm, or react to force a range-card's dice to max/min.
//!
//! 规则书（docs/rulebook/cards.json, id `AG:（鸫）微小的『能做到』的事`）:
//! > （鸫）微小的『能做到』的事：打出此卡时，选择其中一个效果发动：
//! > （1）【反击】时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算，除@Tsugu ycm以外无法直接改变骰子点数（ex：可以对YOLO生效但无法对无论是何种颜色的夕阳生效）
//! > （2）视为打出一张@Tsugu ycm
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const TSUGUMI_CAN: CardDef = CardDef::new("AG:（鸫）微小的『能做到』的事", &[
    On::Play(Some(cant_play), play),
    On::CounterAct(&[ChainKind::Card], can_react, react)]);

/// 规则书（1）: the gate is 「这回合已经移动过了」 -- the same main-move latch the
/// movement rules use (`H.MoveWhyNot`), so the card is refused once the turn's
/// main move is spent.
fn cant_play(player_id: i32) -> Option<Msg> {
    ctx::cant_move(player_id)
}

fn can_react(player_id: i32) -> bool {
    // 规则书（1）【反击】: 「时机合适时打出此卡，使任意结果只有数字区间的效果以理论最大值或最小值结算」
    // -- C# reacts while another `RangeCard` play is resolving
    // (`t.Kind == "card" && t.Play.Def.RangeCard && t.Play.Extreme == 0`).
    if trigger::kind() != TriggerKind::Card {
        return false;
    }
    let _ = player_id;
    // 规则书（1）: 「除@Tsugu ycm以外」 -- the exclusion reads the play's card id
    // off the trigger (C# `t.Play.Id` / `t.Card`).
    if trigger::card_is("通用:@Tsugu ycm") {
        return false;
    }
    // TODO(规则书)[judgement](ABI): `t.Play.Def.RangeCard` / `t.Play.Extreme` are not in the
    //   the clause under-specifies -- see the note above it
    // trigger vocabulary. Matching every `card` trigger over-fires (the
    // passage's example says YOLO yes, 「无论是何种颜色的夕阳」 no); the RangeCard
    // filter is what keeps those apart.
    true
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: 「视为打出一张@Tsugu ycm」 -- C# `Ycm.Play(c)` on a shared
    // `CardTsuguYcm` instance (id `通用:@Tsugu ycm`).
    ctx::log(player_id, &Msg::new(key!("tsugumi_can_ycm")).player_id("who", player_id));
    ctx::play_card("通用:@Tsugu ycm", player_id)?;
    Ok(())
}

fn react(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）【反击】: 「使任意结果只有数字区间的效果以理论最大值或最小值结算」
    // -- C# asks max/min and writes `target.Extreme = 1 / -1`.
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("tsugumi_can_title")),
        &Msg::new(key!("tsugumi_can_ask")),
        &[
            Msg::new(key!("tsugumi_can_max")),
            Msg::new(key!("tsugumi_can_min"))],
    )?;
    // 规则书（1）: 「以理论最大值或最小值结算」 -- C# `target.Extreme = 1 / -1`,
    // so the play being reacted to settles its number ranges at the theoretical
    // extreme the picker named.
    ctx::set_extreme(if pick == 0 { 1 } else { -1 });
    ctx::log(player_id, &Msg::new(key!("tsugumi_can_forced")).player_id("who", player_id));
    Ok(())
}