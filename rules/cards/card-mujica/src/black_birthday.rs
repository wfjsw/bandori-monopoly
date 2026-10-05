//! `Mujica:黑色生日` -- C# `CardBlackBirthday`: every other player pays you
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:黑色生日`）:
//! > 黑色生日：
//! >  每名你以外的资金在1000以下的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。
//!
//! 800 when they have at most 1,000, otherwise 200 twice (the second only
//! while they are still in).

use card_sdk::{ctx, key, CardDef, Msg};

pub const BLACK_BIRTHDAY: CardDef = CardDef {
    id: "Mujica:黑色生日",
    play: Some(black_birthday),
    can_react: None,
    react: None,
    why_not: Some(why_not),
};

/// C# `CardBlackBirthday.WhyNot` -- 「没有别的玩家」 when `H.Others` is empty.
fn why_not(seat: i32) -> Option<Msg> {
    if ctx::others(seat).is_empty() {
        return Some(Msg::new(key!("x_no_others")));
    }
    None // playable
}

fn black_birthday(seat: i32) {
    let why = Msg::new(key!("black_birthday_why"));
    for p in ctx::others(seat) {
        if ctx::money(p) <= 1000 {
            ctx::transfer(p, seat, 800, &why);
        } else {
            ctx::transfer(p, seat, 200, &why);
            if !ctx::seat_out(p) {
                ctx::transfer(p, seat, 200, &why);
            }
        }
    }
}