//! `Sumimi:Sumimi是二人一体的` -- C# `CardTwoInOne` (MatchHost.cs:11249-11312):
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Sumimi是二人一体的`）:
//! > Sumimi是二人一体的：
//! > （1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数
//! > （2）此卡可在你的移动掷骰后作为[反击]使用。
//!
//! swap to the other Sumimi character (play or [反击] after own move roll).

use card_sdk::abi::ChainKind;
use bandori_std::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const TWO_IN_ONE: CardDef = CardDef::new(
    "Sumimi:Sumimi是二人一体的",
    &[
        On::Play("", "", None, play),
        On::Counteract(
            &[ChainKind::DiceRollResult],
            "actor == owner && (persona_is(owner, \"三角初华（Sumimi）\") || persona_is(owner, \"纯田真奈\"))",
            r#"你的移动掷骰后"#,
            None,
            counteract,
        ),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Sumimi是二人一体的"#)
        .raw_name(r#"Sumimi是二人一体的"#)
        .tags(&[r#"反击"#])
        .text(r#"（1）打出此卡，将自己的角色卡替换为sumimi的另一名角色及其初始火罐数（2）此卡可在你的移动掷骰后作为[反击]使用。"#)
        // sheet cell L6
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

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

/// The two the clause names. Fire pots come from the **new face's** `CardDef`
/// props (`bandori_std::fire::FIRE_INITIAL` / `FIRE_CAP`) -- no hardcoded
/// 初始/上限 pairs here.
const A: (&str, &str) = ("三角初华（Sumimi）", "skill:三角初华（Sumimi）:成为偶像");
const B: (&str, &str) = ("纯田真奈", "skill:纯田真奈:甜甜圈爱好者");

/// C# `CardTwoInOne.Swap` -- flip 三角初华（Sumimi） <-> 纯田真奈, re-declare
/// fire from the new face's `CardDef`.
fn swap(player_id: i32) {
    // 规则书（1）: 「将自己的角色卡替换为sumimi的另一名角色及其初始火罐数」 --
    // `H.ReplaceSkill` is the old skill rule coming off the field and the new
    // one going on, and 「初始火罐数」 is the new face's `CardDef` declaration
    // being written (swap **re-declares**; never carry-over -- MARKERS.md §4.6).
    let (from, to) = if ctx::persona_is(player_id, A.0) {
        (A, B)
    } else {
        (B, A)
    };
    // 规则书（1）: 「将自己的角色卡替换为sumimi的另一名角色」 -- the identity
    // itself, not just the bound skill. Exclusive play and `persona_is` read
    // this name.
    ctx::set_persona(player_id, to.0);
    if let Some(uid) = ctx::find_card(player_id, from.1) {
        ctx::unplace_at(uid);
    }
    ctx::place_card(player_id, to.1, &Msg::new(key!("two_in_one_note")));
    // Re-declare the new face's 「初始火罐数」 from its `CardDef` props.
    bandori_std::fire::redeclare_from(player_id, to.1);
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
