//! `R:（ykn）louder` -- C# `CardLouder`: the RiNG rent multiplier +5.
//!
//! 规则书（docs/rulebook/cards.json, id `R:（ykn）louder`）:
//! > （ykn）louder： 
//! >  每次打出此卡时使ring的价格基础乘数+5
//!

use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const LOUDER: CardDef = CardDef::new("R:（ykn）louder", &[On::Play("", None, louder)])
    .props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"louder"#)
        .raw_name(r#"（ykn）louder"#)
        .tags(&[])
        .text(r#"每次打出此卡时使ring的价格基础乘数+5"#)
        .cell(r#"E2"#)
        .fields(&[("band", r#"Roselia"#), ("owner", r#"凑友希那"#)]));

fn louder(player_id: i32) -> card_sdk::Asked {
    let now = ctx::add_ring_bonus(5);
    ctx::log(
        player_id,
        &Msg::new(key!("louder_up")).i("n", 5).i("mult", now as i64),
    );
    Ok(())
}
