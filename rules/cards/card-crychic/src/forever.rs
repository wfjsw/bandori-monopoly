//! `CRYCHIC:如果能一直持续下去...` -- C# `CardForever`: drain the band skill
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:如果能一直持续下去...`）:
//! > 如果能一直持续下去...： 
//!
//! > （1）[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。
//!
//! > （2）[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。
//!
//! crystals for 2,000 + 500·X, then stays in play as [持续] until a draw
//! leaves the owner holding 7+ cards.

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const FOREVER: CardDef = CardDef::new("CRYCHIC:如果能一直持续下去...", &[
    On::Play(None, forever),
    On::Hook(&[HookKind::Drew], |_| true, on_drew)]);

const ID: &str = "CRYCHIC:如果能一直持续下去...";

fn forever(player_id: i32) {
    // 规则书（1）[手]: 「移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。」
    let x = ctx::band_crystals(player_id);
    if x > 0 {
        ctx::add_band_crystals(player_id, -x, 0); // 规则书（1）[手]: 「移除你乐队技能卡上的奇迹水晶」
    }
    ctx::gain(player_id, 2000 + 500 * x, &Msg::new(key!("forever_why")).i("x", x as i64));
    // 规则书（1）[手]: 「获得2000+500*X资金，X为移除的奇迹水晶数量」
    // 规则书（2）[持续]: the card lives in play until a draw leaves 7+ in hand
    // (`on_drew` below).
    ctx::set_dest(ctx::Dest::Field); // 规则书（2）[持续]: the card lives in play
    ctx::place_card(player_id, ID, &Msg::new(key!("forever_note")));
}

/// 规则书（2）[持续]: 「若你的手牌大于等于7，此卡立即置入弃牌堆。」 -- C#
/// `CardForever.Drew` (the `Fx.Drew` placed-card walk, once per draw batch):
/// when the owner draws and the hand is 7+, the card goes straight to the
/// discard pile.
fn on_drew(player_id: i32) {
    // The hook fires on the placed card; only the owner's own draws count
    // (C# `player_id != Player`).
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    // 规则书（2）[持续]: 「若你的手牌大于等于7」 -- C# `H._hidden[Player].hand.Count < 7`.
    if ctx::hand_size(player_id) < 7 {
        return;
    }
    // 规则书（2）[持续]: 「此卡立即置入弃牌堆」 -- C# `H.Unplace(this, "discard", "手牌有 7 张以上")`.
    ctx::unplace_card(player_id);
    ctx::to_discard(player_id, ID);
    ctx::log(player_id, &Msg::new(key!("forever_discard")).player_id("who", player_id));
}
