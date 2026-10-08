//! `Mujica:黑色生日` -- C# `CardBlackBirthday`: every other player pays you
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:黑色生日`）:
//! > 黑色生日： 
//! >  每名你以外的资金不多于1000的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。
//!
//! 800 when they have at most 1,000, otherwise 200 twice (the second only
//! while they are still in).

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BLACK_BIRTHDAY: CardDef = CardDef::new(
    "Mujica:黑色生日",
    &[On::Play("", Some(cant_play), black_birthday)],
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
    // 「资金在1000以下的玩家」 is a single check at resolution. The card body
    // re-runs from the top on each prompt/answer (the host's replay model) and
    // `money_of` moves as each transfer lands, so the bracket is latched into a
    // slot on the first pass and every replay takes the same branch -- the two
    // 200s for a >1000 player each stay a separate [支付] (each answerable by a
    // counteraction).
    let latch = "latch:black_birthday.bracket";
    let others = ctx::others(player_id);
    if ctx::slot(player_id, latch) == 0 {
        // Bit i+1 = 1 means "others[i] was ≤1000 at resolution" (pays 800).
        let mut bits = 1i32;
        for (i, p) in others.iter().enumerate() {
            if ctx::money_of(*p) <= 1000 {
                bits |= 1 << (i + 1);
            }
        }
        ctx::set_slot(player_id, latch, bits);
    }
    let bits = ctx::slot(player_id, latch);
    for (i, p) in others.into_iter().enumerate() {
        if (bits >> (i + 1)) & 1 == 1 {
            ctx::transfer(p, player_id, 800, &why)?;
        } else {
            ctx::transfer(p, player_id, 200, &why)?;
            if !ctx::player_out(p) {
                ctx::transfer(p, player_id, 200, &why)?;
            }
        }
    }
    ctx::set_slot(player_id, latch, 0); // clear for a later play
    Ok(())
}
