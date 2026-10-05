//! `R:（ykn）louder` -- C# `CardLouder`: the RiNG rent multiplier +5.
//!
//! 规则书（docs/rulebook/cards.json, id `R:（ykn）louder`）:
//! > （ykn）louder： 
//! >  每次打出此卡时使ring的价格基础乘数+5
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const LOUDER: CardDef = CardDef { id: "R:（ykn）louder", play: Some(louder), can_react: None, react: None, why_not: None };

fn louder(seat: i32) {
    let now = ctx::add_ring_bonus(5);
    ctx::log(seat, &Msg::new(key!("louder_up")).i("n", 5).i("mult", now as i64));
}
