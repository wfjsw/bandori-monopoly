//! `CRYCHIC:（soyo）回到曾经` -- C# `CardSoyoBack`: play from hand for 500; the
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（soyo）回到曾经`）:
//! > （soyo）回到曾经：
//! > （1）[特] 抽到此卡时立刻从抽牌堆打出并执行以下操作之一 ：
//! > 1. 将你的所有手牌放入弃牌堆，然后获得弃牌数*500的资金，抽1张卡，为你的一个格子付费加盖一间房屋，然后[移除]此卡；
//! > 2. 获得1000资金并将此卡加入手牌 
//! > （2）[手] 获得500资金
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const SOYO_BACK: CardDef = CardDef::new("CRYCHIC:（soyo）回到曾经", &[
    On::Play(soyo_back),
    On::Hook(&[TriggerKind::Drawn], on_drawn),
]);

const ID: &str = "CRYCHIC:（soyo）回到曾经";

fn soyo_back(player_id: i32) {
    // 规则书（2）[手]: 「获得500资金」
    ctx::gain(player_id, 500, &Msg::new(key!("soyo_back_why")));
}

/// 规则书（1）[特]: 「抽到此卡时立刻从抽牌堆打出并执行以下操作之一」 -- C#
/// `CardSoyoBack.Drawn` -> `Special`.
fn on_drawn(player_id: i32) {
    if trigger::kind() != TriggerKind::Drawn || !trigger::card_is(ID) {
        return;
    }
    // The card was just drawn, so it is in hand (C# `hand.Contains(Id)`).
    ctx::log(player_id, &Msg::new(key!("soyo_back_special")).player_id("who", player_id));
    let options = [
        Msg::new(key!("soyo_back_opt1")),
        Msg::new(key!("soyo_back_opt2")),
    ];
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("soyo_back_title")),
        &Msg::new(key!("soyo_back_ask")),
        &options,
    );
    if pick == 0 {
        branch_discard(player_id);
    } else {
        branch_keep(player_id);
    }
}

/// 规则书（1）1: 「将你的所有手牌放入弃牌堆，然后获得弃牌数*500的资金，抽1张卡，
/// 为你的一个格子付费加盖一间房屋，然后[移除]此卡」
fn branch_discard(player_id: i32) {
    // C# `hand.Remove(Id)` first so this card is not among the discarded.
    if !ctx::take_from_hand(player_id, ID) {
        return;
    }
    // 规则书（1）1: 「将你的所有手牌放入弃牌堆」 -- the hand is listable now
    // (`ctx::cards_in`).
    let hand = ctx::cards_in(player_id, ctx::CardPile::Hand);
    for id in &hand {
        ctx::discard_from_hand(player_id, id);
    }
    // 规则书（1）1: 「获得弃牌数*500的资金」 -- C# `500 * list.Count` (the cards just
    // discarded, not the whole pile).
    let n = hand.len() as i32;
    if n > 0 {
        ctx::gain(player_id, 500 * n, &Msg::new(key!("soyo_back_discard_why")).i("n", n as i64));
    }
    // 规则书（1）1: 「抽1张卡」
    ctx::draw(player_id, 1);
    // 规则书（1）1: 「为你的一个格子付费加盖一间房屋」 -- C# `H.OfferBuildAmong(seat,
    // H.OwnedBy(player), CardName)`.
    // TODO(规则书)（1）1: 「为你的一个格子付费加盖一间房屋」 -- needs the build routine
    //   (`H.OfferBuildAmong` / `H.BuyRoutine`-family): a prompt over the owned
    //   tiles that pays `ctx::build_cost` and runs `ctx::add_house`. The rest of
    //   branch 1 is expressible.
    // 规则书（1）1: 「然后[移除]此卡」 -- already taken out of hand above and never
    // put back anywhere (C# just logs 「回到曾经」被 [移除]）。
    ctx::log(player_id, &Msg::new(key!("soyo_back_removed")).player_id("who", player_id));
}

/// 规则书（1）2: 「获得1000资金并将此卡加入手牌」
fn branch_keep(player_id: i32) {
    // 规则书（1）2: 「获得1000资金」
    ctx::gain(player_id, 1000, &Msg::new(key!("soyo_back_why")));
    // 规则书（1）2: 「并将此卡加入手牌」 -- the card is already in hand from the draw
    // (C# leaves it there); `add_to_hand` is a no-op-looking second copy guard
    // and is skipped so the draw's copy is the one kept.
}
