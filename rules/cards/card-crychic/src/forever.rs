//! `CRYCHIC:如果能一直持续下去...` -- C# `CardForever`: drain the band skill
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:如果能一直持续下去...`）:
//! > 如果能一直持续下去...： 
//!
//! > （1）[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。
//!
//! > （2）[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。
//!
//! crystals for 2,000 + 500·X, then stays in play as [持续].

use card_sdk::{ctx, key, CardDef, Msg};

pub const FOREVER: CardDef = CardDef {
    id: "CRYCHIC:如果能一直持续下去...",
    play: Some(forever),
    can_react: None,
    react: None,
    why_not: None,
};

fn forever(seat: i32) {
    // 规则书（1）[手]: 「移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。」
    let x = ctx::band_crystals(seat);
    if x > 0 {
        ctx::add_band_crystals(seat, -x, 0); // 规则书（1）[手]: 「移除你乐队技能卡上的奇迹水晶」
    }
    ctx::gain(seat, 2000 + 500 * x, &Msg::new(key!("forever_why")).i("x", x as i64));
    // 规则书（1）[手]: 「获得2000+500*X资金，X为移除的奇迹水晶数量」
    // 规则书（2）[持续]: 「若你的手牌大于等于7，此卡立即置入弃牌堆。」
    // TODO(规则书)（2）[持续]: the discard check runs on every draw (C#
    // `CardForever.Drew`) -- needs the Fx.Drew persistent hook and a hand-count
    // query. Until then the card stays in play as [持续].
    ctx::set_dest(ctx::Dest::Field); // 规则书（2）[持续]: the card lives in play
    ctx::place_card(seat, "CRYCHIC:一起演奏音乐的命运共同体", &Msg::new(key!("forever_note")));
}
