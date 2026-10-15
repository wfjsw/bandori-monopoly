//! `Sumimi:Sumimi是二人一体的` -- C# `CardTwoInOne` (MatchHost.cs:11249-11312):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi是二人一体的`）:
//! > Sumimi是二人一体的：
//! > （1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数
//! > （2）此卡可在你的移动掷骰后作为[反击]使用。
//!
//! swap to the other Sumimi character (play or [反击] after own move roll).

use card_sdk::abi::ChainKind;
use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const TWO_IN_ONE: CardDef = CardDef::new(
    "Sumimi:Sumimi是二人一体的",
    &[
        On::Play("", None, play),
        On::Counteract(
            &[ChainKind::MoveRoll],
            "actor == owner && (character_is(owner, \"三角初华（Sumimi）\") || character_is(owner, \"纯田真奈\"))",
            None,
            counteract,
        ),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Sumimi是二人一体的"#)
        .raw_name(r#"Sumimi是二人一体的"#)
        .tags(&[r#"反击"#])
        .text(r#"（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数（2）此卡可在你的移动掷骰后作为[反击]使用。"#)
        .cell(r#"L6"#)
        .fields(&[("band", r#"Sumimi"#), ("owner", "")]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」
    swap(player_id);
    Ok(())
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    // 规则书（2）: the [反击] is the same swap as the play effect (C# `Counteract` -> `Swap`).
    swap(player_id);
    Ok(())
}

/// The two the clause names, and their 「初始火罐数」.
const A: (&str, &str, i32, i32) = (
    "三角初华（Sumimi）",
    "skill:三角初华（Sumimi）:成为偶像",
    2,
    2,
);
const B: (&str, &str, i32, i32) = ("纯田真奈", "skill:纯田真奈:甜甜圈爱好者", 1, 2);

/// C# `CardTwoInOne.Swap` -- flip 三角初华（Sumimi） <-> 纯田真奈, reset fire.
fn swap(player_id: i32) {
    // 规则书（1）: 「将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」 --
    // `H.ReplaceSkill` is the old skill rule coming off the field and the new
    // one going on, and 「初始火罐数」 is the new character's cap being written.
    let (from, to) = if ctx::character_is(player_id, A.0) {
        (A, B)
    } else {
        (B, A)
    };
    if let Some(uid) = ctx::find_card(player_id, from.1) {
        ctx::unplace_at(uid);
    }
    ctx::place_card(player_id, to.1, &Msg::new(key!("two_in_one_note")));
    ruleset_core::ctx::state::set_bounds(player_id, card_sdk::abi::state_key::FIRE, 0, to.3);
    ruleset_core::ctx::state::set(player_id, card_sdk::abi::state_key::FIRE, to.2);
    ctx::log(
        player_id,
        &Msg::new(key!("two_in_one_swap"))
            .player_id("who", player_id)
            .card("card", to.1),
    );
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
