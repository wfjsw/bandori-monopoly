//! `CRYCHIC:（soyo）回到曾经` -- C# `CardSoyoBack`: play from hand for 500; the
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（soyo）回到曾经`）:
//! > （soyo）回到曾经：
//! > （1）[特] 抽到此卡时立刻从抽牌堆打出并执行以下操作之一 ：
//! > 1. 将你的所有手牌放入弃牌堆，然后获得弃牌数*500的资金，抽1张卡，为你的一个格子付费加盖一间房屋，然后[移除]此卡；
//! > 2. 获得1000资金并将此卡加入手牌 
//! > （2）[手] 获得500资金
//!
//! [特] draw-triggered choice is the Drawn hook (TODO).

use card_sdk::{ctx, key, CardDef, Msg};

pub const SOYO_BACK: CardDef = CardDef {
    id: "CRYCHIC:（soyo）回到曾经",
    play: Some(soyo_back),
    can_react: None,
    react: None,
    why_not: None,
};

fn soyo_back(seat: i32) {
    // 规则书（2）[手]: 「获得500资金」
    ctx::gain(seat, 500, &Msg::new(key!("soyo_back_why")));
    // TODO(规则书)（1）[特]: 「抽到此卡时立刻从抽牌堆打出并执行以下操作之一」-- needs the
    //   Fx.Drawn hook (auto-play on draw). The two branches also need hand
    //   enumeration (「将你的所有手牌放入弃牌堆」) and house counts
    //   (「付费加盖一间房屋」); branch 2 is otherwise expressible
    //   (gain 1000 + Dest::Hand).
}
