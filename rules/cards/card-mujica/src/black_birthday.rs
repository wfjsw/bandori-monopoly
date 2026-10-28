//! `Mujica:黑色生日` -- C# `CardBlackBirthday`: every other player pays you
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:黑色生日`）:
//! > 黑色生日：
//! >  每名你以外的资金不多于1000的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。
//!
//! 800 when they have at most 1,000, otherwise 200 twice (the second only
//! while they are still in).

use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BLACK_BIRTHDAY: CardDef = CardDef::new(
    "Mujica:黑色生日",
    &[On::Play("", Some(cant_play), black_birthday)],
)
.props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"黑色生日"#)
        .raw_name(r#"黑色生日"#)
        .tags(&[])
        .text(CardText::para(r#"每名你以外的资金不多于1000的玩家支付你800资金，每名你以外的资金严格在1000以上的玩家支付你两次200资金。"#))
        // sheet cell J14
        .fields(&[("group", r#"Ave Mujica"#), ("owner", "")]));

/// C# `CardBlackBirthday.WhyNot` -- 「没有别的玩家」 when `H.Others` is empty.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「每名你以外」 -- the cell states no playability condition; the
    // refusal when nobody else is in is C#-sourced.
    // TODO(规则书)（「每名你以外」）: the book has no "no other player -> unplayable"
    //   clause -- we refuse a play with no payer (C# `CardBlackBirthday.WhyNot`).
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_others")));
    }
    None // playable
}

fn black_birthday(player_id: i32) -> card_sdk::Asked {
    let why = Msg::new(key!("black_birthday_why"));
    // 规则书: 「每名你以外的资金不多于1000的玩家支付你800资金」 is a single check
    // at resolution. The card body
    // re-runs from the top on each prompt/answer (the host's replay model) and
    // `money_of` moves as each transfer lands, so the bracket is latched into a
    // slot on the first pass and every replay takes the same branch -- the two
    // 200s for a >1000 player each stay a separate [支付] (each answerable by a
    // counteraction). The `latch:` prefix is load-bearing: the body's world copy
    // is dropped at each pause and `Cx::adopt_turn_policy` carries only `latch:`
    // state keys across it (`engine/cx.rs`).
    let latch = "latch:black_birthday.bracket";
    // The payer set is latched by **seat** too (bit p+1), not re-read from
    // `ctx::others`: the body re-runs from the top as each payment lands, and
    // `ctx::others` drops a payer who just went bankrupt from the live world --
    // the list would shift under the logged answers and skip the payer after
    // the dead one.
    let payers = "latch:black_birthday.payers";
    if ctx::slot(player_id, latch) == 0 {
        // Bit p+1 of `latch` = "that payer was ≤1000 at resolution" (pays 800).
        let mut bits = 1i32;
        let mut mask = 1i32;
        for p in ctx::others(player_id) {
            mask |= 1 << (p + 1);
            if ctx::money_of(p) <= 1000 {
                bits |= 1 << (p + 1);
            }
        }
        ctx::set_slot(player_id, payers, mask);
        ctx::set_slot(player_id, latch, bits);
    }
    let bits = ctx::slot(player_id, latch);
    let mask = ctx::slot(player_id, payers);
    for p in 0..ctx::player_count() {
        if (mask >> (p + 1)) & 1 == 0 {
            continue; // not in the set (the user, or gone before the play)
        }
        if (bits >> (p + 1)) & 1 == 1 {
            // 规则书: 「每名你以外的资金不多于1000的玩家支付你800资金」
            ctx::transfer(p, player_id, 800, &why)?;
        } else {
            // 规则书: 「每名你以外的资金严格在1000以上的玩家支付你两次200资金。」
            ctx::transfer(p, player_id, 200, &why)?;
            // TODO(规则书)（「支付你两次200资金」）: the book says the two payments are
            //   unconditional -- we skip the second once the payer is out (a
            //   no-op today: `money_inner` early-returns for an out payer).
            if !ctx::player_out(p) {
                ctx::transfer(p, player_id, 200, &why)?;
            }
        }
    }
    ctx::set_slot(player_id, latch, 0); // clear for a later play
    ctx::set_slot(player_id, payers, 0);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn black_birthday_kani(player_id: i32) -> card_sdk::Asked {
    black_birthday(player_id)
}
