//! `R:曲奇时间` -- C# `CardCookieTime` (MatchHost.cs:10423-10445): shuffle the
//!
//! 规则书（docs/rulebook/cards.json, id `R:曲奇时间`）:
//! > 曲奇时间：
//! >  将自己弃牌堆的卡全部返回抽牌堆并洗切，获得500*X资金，X为返回卡的总数。
//!
//! whole discard pile back into the deck and gain 500 per returned card.

use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg};

pub const COOKIE_TIME: CardDef = CardDef {
    id: "R:曲奇时间",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardCookieTime.WhyNot`: 「弃卡区没有卡」.
fn why_not(seat: i32) -> Option<Msg> {
    // 规则书: 「将自己弃牌堆的卡全部返回抽牌堆并洗切」 -- nothing to return when the
    // discard pile is empty (C# `H._hidden[seat].discard.Count != 0`).
    if ctx::discard_size(seat) == 0 {
        return Some(Msg::new(key!("cookie_time_no_discard")));
    }
    None
}

fn play(seat: i32) {
    // 规则书: 「将自己弃牌堆的卡全部返回抽牌堆并洗切」 -- C#
    // `H.ShuffleAllIntoDeck(i, hand: false, discard: true)`.
    // 规则书: 「获得500*X资金，X为返回卡的总数」 -- X is `hidden.discard.Count`
    // before the shuffle (C# `CardCookieTime.Play`).
    let x = ctx::discard_size(seat);
    // The hook sweeps hand + discard (there is no `hand: false` flag on
    // `sweep_to_deck`); X stays the discard size the rule names.
    ctx::sweep_to_deck(seat);
    ctx::log(
        seat,
        &Msg::new(key!("cookie_time_shuffle")).seat("who", seat).i("n", x as i64),
    );
    // 规则书: 「获得500*X资金，X为返回卡的总数」
    ctx::gain(seat, 500 * x, &Msg::new(key!("cookie_time_why")));
    // TODO(规则书): the C# also fires `H.Each((Fx f) => f.Reshuffled(i))` on the
    //   seat -- needs the persistent Fx.Reshuffled hook.
}