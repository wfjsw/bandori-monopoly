//! `PP:可爱又强壮的花朵` -- C# `CardStrongFlower` (MatchHost.cs:7629-7698):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:可爱又强壮的花朵`）:
//! > 可爱又强壮的花朵：
//! > [手]：
//! > [指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]。
//! > [持续]：
//! > [使用者][经过]此卡所在格子时依次进行以下操作：
//! > 1. [强制停下]，此次移动变为[结算]；
//! > 2. 如果[共鸣]则获得1500资金；
//! > 3. 此卡放入[使用者]弃卡区。
//!
//! place on one of your deeds; the user stops there and settles when passing.
//! `place_card_on` stamps the chosen tile on the instance and `self_tile` reads
//! the binding back; the [持续] stop is forced there with `plan::set_stop_at` +
//! `plan::set_resolve`.

use alloc::vec::Vec;
use bandori_std::ctx;
use bandori_std::ctx::{plan, trigger};
use card_sdk::abi::{ActionKind, TriggerKind};
use card_sdk::{key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};

const ID: &str = "PP:可爱又强壮的花朵";

pub const STRONG_FLOWER: CardDef = CardDef::new(
    "PP:可爱又强壮的花朵",
    &[
        On::Play("", Some(cant_play), strong_flower),
        // 规则书[持续]: 「[使用者][经过]此卡所在格子时」
        On::Action(Action::PassTile("actor == owner", None, pass_tile)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"可爱又强壮的花朵"#)
        .raw_name(r#"可爱又强壮的花朵"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]。"#),
            ]),
            sec(r#"[持续]："#, &[
                para(r#"[使用者][经过]此卡所在格子时依次进行以下操作："#),
                steps(&[
                    r#"[强制停下]，此次移动变为[结算]；"#,
                    r#"如果[共鸣]则获得1500资金；"#,
                    r#"此卡放入[使用者]弃卡区。"#,
                ]),
            ]),
        ]))
        // sheet cell D12
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」 -- nothing to point at without
    // a deed. C# `CardStrongFlower.WhyNot` refuses the play when the player owns
    // nothing ("你还没有地").
    if ctx::owned_count(player_id) == 0 {
        return Some(Msg::new(key!("strong_flower_why_not")));
    }
    None
}

fn strong_flower(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」
    let tiles: Vec<i32> = ctx::owned_tiles(player_id);
    let tile = if tiles.is_empty() {
        -1
    } else {
        ctx::ask_tile(
            player_id,
            &Msg::new(key!("strong_flower_title")),
            &Msg::new(key!("strong_flower_ask")),
            &tiles,
        )?
    };
    // 规则书[手]: 「[指定][使用者]拥有的一个格子」 -- the tile goes through the
    // [指定] pipeline, so a 「不能被指定」 mark refuses the play. (The owner-side
    // chain/immune half only runs for another player's tile, and this card can
    // only name the user's own.) Refusal = no placement.
    if tile >= 0 && !ctx::target_tile(tile) {
        return Ok(());
    }
    // 规则书[手]: 「将此卡放置在被[指定]格子上」 -- bound to the chosen tile, so
    // the `PassTile` hook below can ask 「此卡所在格子」.
    if tile >= 0 {
        ctx::log(
            player_id,
            &Msg::new(key!("strong_flower_at")).tile("tile", tile),
        );
    }
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(
        player_id,
        tile,
        "PP:可爱又强壮的花朵",
        &Msg::new(key!("strong_flower_note")),
    );
    // 规则书[手]: 「并为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]」
    ctx::add_group_crystals(player_id, 2, i32::MAX);
    Ok(())
}

/// 规则书[持续]: 「[使用者][经过]此卡所在格子时依次进行以下操作：1. [强制停下]，
/// 此次移动变为[结算]；2. 如果[共鸣]则获得1500资金；3. 此卡放入[使用者]弃卡区」
/// -- the user's own pass, onto the tile this card is bound to, and the three
/// steps in order.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // `actor == owner` is the pre. `self_tile` is a derived lookup (the tile
    // this card is bound to).
    let Some(tile) = ctx::self_tile() else {
        return Ok(());
    };
    if trigger::tile() != tile {
        return Ok(());
    }
    // 规则书[持续]1: 「[强制停下]，此次移动变为[结算]」 -- [强制停下] is an
    // [异常移动效果]: it passes `ctx::gate` so [反击] cards get their window, a
    // [不可阻挡] mover may refuse it, and a refusal voids the whole [持续] (the
    // card stays on its tile).
    if !ctx::gate(trigger::player_id(), bandori_std::kind::abnormal::Stop) {
        return Ok(());
    }
    plan::set_stop_at(tile);
    plan::set_resolve(true);
    // 2. 「如果[共鸣]则获得1500资金」
    if crate::resonance::try_resonance(player_id)? {
        ctx::gain(player_id, 1500, &Msg::new(key!("strong_flower_gain")))?;
    }
    // 3. 「此卡放入[使用者]弃卡区」
    ctx::set_dest(ctx::Dest::Graveyard);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn strong_flower_kani(player_id: i32) -> card_sdk::Asked {
    strong_flower(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}
