//! `Mujica:黑色生日` -- C# `CardBlackBirthday`: every other player pays you
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:黑色生日`）:
//! > 黑色生日：
//! >  每名你以外的资金在1000以下的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。
//!
//! 800 when they have at most 1,000, otherwise 200 twice (the second only
//! while they are still in).

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BLACK_BIRTHDAY: CardDef = CardDef::new(
    "Mujica:黑色生日",
    &[On::Play(Some(cant_play), black_birthday)],
);

/// C# `CardBlackBirthday.WhyNot` -- 「没有别的玩家」 when `H.Others` is empty.
fn cant_play(player_id: i32) -> Option<Msg> {
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_others")));
    }
    None // playable
}

fn black_birthday(player_id: i32) -> card_sdk::Asked {
    let why = Msg::new(key!("black_birthday_why"));
    for p in ctx::others(player_id) {
        if ctx::money_of(p) <= 1000 {
            ctx::transfer(p, player_id, 800, &why)?;
        } else {
            ctx::transfer(p, player_id, 200, &why)?;
            if !ctx::player_out(p) {
                ctx::transfer(p, player_id, 200, &why)?;
            }
        }
    }
    Ok(())
}
