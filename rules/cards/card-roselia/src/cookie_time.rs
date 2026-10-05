//! `R:曲奇时间` -- C# `CardCookieTime` (MatchHost.cs:10423-10445): shuffle the
//!
//! 规则书（docs/rulebook/cards.json, id `R:曲奇时间`）:
//! > 曲奇时间：
//! >  将自己弃牌堆的卡全部返回抽牌堆并洗切，获得500*X资金，X为返回卡的总数。
//!
//! whole discard pile back into the deck and gain 500 per returned card.

use card_sdk::ctx;
use card_sdk::{key, CardDef, On, Msg};

pub const COOKIE_TIME: CardDef = CardDef::new("R:曲奇时间", &[
    On::Play(play),
    On::CantPlay(cant_play),
]);

/// C# `CardCookieTime.WhyNot`: 「弃卡区没有卡」.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「将自己弃牌堆的卡全部返回抽牌堆并洗切」 -- nothing to return when the
    // discard pile is empty (C# `H._hidden[player_id].discard.Count != 0`).
    if ctx::discard_size(player_id) == 0 {
        return Some(Msg::new(key!("cookie_time_no_discard")));
    }
    None
}

fn play(player_id: i32) {
    // 规则书: 「将自己弃牌堆的卡全部返回抽牌堆并洗切」 -- C#
    // `H.ShuffleAllIntoDeck(i, hand: false, discard: true)`.
    // 规则书: 「获得500*X资金，X为返回卡的总数」 -- X is `hidden.discard.Count`
    // before the shuffle (C# `CardCookieTime.Play`).
    let x = ctx::discard_size(player_id);
    // The hook sweeps hand + discard (there is no `hand: false` flag on
    // `sweep_to_deck`); X stays the discard size the rule names.
    ctx::sweep_to_deck(player_id);
    ctx::log(
        player_id,
        &Msg::new(key!("cookie_time_shuffle")).player_id("who", player_id).i("n", x as i64),
    );
    // 规则书: 「获得500*X资金，X为返回卡的总数」
    ctx::gain(player_id, 500 * x, &Msg::new(key!("cookie_time_why")));
    // 规则书: the C# also fires `H.Each((Fx f) => f.Reshuffled(i))` on the player.
    // v25: `sweep_to_deck` now raises `reshuffled` from the host (the C# card
    // body calls `H.Each(Reshuffled)` itself; the host folds that into the
    // sweep's commit). Listener cards declare `On::Hook(&[TriggerKind::Reshuffled], ...)`
    // and get the notification without any card-side raise.
}
