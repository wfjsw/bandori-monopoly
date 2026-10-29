//! `R:（ykn）louder` -- C# `CardLouder`: ring price base multiplier (「价格基础乘数」) +5.
//!
//! 规则书（docs/rulebook/cards.json, id `R:（ykn）louder`）:
//! > （ykn）louder：
//! >  每次打出此卡时使ring的价格基础乘数+5
//!
//! The book never defines 价格基础乘数; the engine slot is the ring rent unit
//! (`rentUnitMult`, OPEN-RING-MULT).

use bandori_std::ctx;
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

// 规则书: 「每次打出此卡时」 -- `On::Play` fires on every play, no gate.
pub const LOUDER: CardDef = CardDef::new("R:（ykn）louder", &[On::Play("", None, louder)])
    .props(&[(card_sdk::abi::prop::SONG, 1)])
    .meta(
        &card_sdk::meta::CardMeta::new()
            .name(r#"louder"#)
            .raw_name(r#"（ykn）louder"#)
            .tags(&[])
            .text(CardText::para(r#"每次打出此卡时使ring的价格基础乘数+5"#))
            // sheet cell E2
            .fields(&[("group", r#"Roselia"#), ("owner", r#"凑友希那"#)]),
    );

fn louder(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「每次打出此卡时使ring的价格基础乘数+5」 -- every play, no gate.
    // TODO(规则书)（使ring的价格基础乘数+5）: 价格基础乘数 is undefined in the book --
    //   we add to `rentUnitMult`, the ring rent unit the charge path reads
    //   (OPEN-RING-MULT).
    let now = ctx::add_ring_bonus(5);
    ctx::log(
        player_id,
        &Msg::new(key!("louder_up")).i("n", 5).i("mult", now as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn louder_kani(player_id: i32) -> card_sdk::Asked {
    louder(player_id)
}
