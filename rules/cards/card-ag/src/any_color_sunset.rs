//! `AG:无论是何种颜色的夕阳` 
//!
//! 规则书（docs/rulebook/cards.json, id `AG:无论是何种颜色的夕阳`）:
//! > 无论是何种颜色的夕阳：
//! >  投掷1d6并根据结果获得对应效果: 若为1则立刻获得1500资金，若为2则从弃牌堆中选择一张牌放置到抽牌堆顶，若为3则抽一张牌，若为4则可选择补满火罐或者获得2000资金，若为5则从弃牌堆中选择一张牌加入手牌，若为6则选择1-5中的3个效果触发（若结果严格大于6，则从1开始重新计数）
//!
//! 1d6 picks one of five effects (6 = pick three of them).
//!
//! The 6-branch is **one** multi-select of 3 distinct effects from {1..5}
//! (「3个效果」 names three effects, not one effect thrice) -- a bitmask
//! cannot name the same effect twice. The three trigger in ascending face
//! order (1 before 2 before …): the text specifies no order and a single
//! multiselect has none, so ascending is the deterministic reading.
//!
//! Reviewed 10/23/2026

use bandori_std::ctx;
use card_sdk::abi::{roll_source, RollPurpose};
use card_sdk::ctx::NameId;
use card_sdk::{key, CardDef, CardText, Msg, On};

pub const ANY_COLOR_SUNSET: CardDef =
    CardDef::new("AG:无论是何种颜色的夕阳", &[On::Play("", None, play)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"无论是何种颜色的夕阳"#)
        .raw_name(r#"无论是何种颜色的夕阳"#)
        .tags(&[r#"手"#])
        .text(CardText::para(r#"投掷1d6并根据结果获得对应效果: 若为1则立刻获得1500资金，若为2则从弃牌堆中选择一张牌放置到抽牌堆顶，若为3则抽一张牌，若为4则可选择补满火罐或者获得2000资金，若为5则从弃牌堆中选择一张牌加入手牌，若为6则选择1-5中的3个效果触发（若结果严格大于6，则从1开始重新计数）"#))
        // sheet cell C8
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「投掷1d6并根据结果获得对应效果」; 「若结果严格大于6，则从1开始重新计数」
    let raw = ctx::roll(player_id, 1, 6, RollPurpose::Effect, roll_source::CARD)?;
    let k = (raw - 1).rem_euclid(6) + 1;
    if k <= 5 {
        // 规则书: 「若为1则…若为5则…」 (single-effect branch)
        apply(player_id, k)?;
        return Ok(());
    }
    // 规则书: 「若为6则选择1-5中的3个效果触发」 -- one multi-select of 3 of the
    // five effects. Distinctness is the multiselect's own shape (a bitmask
    // cannot name the same effect twice).
    let options: [Msg; 5] = [
        Msg::new(opt_key(1)),
        Msg::new(opt_key(2)),
        Msg::new(opt_key(3)),
        Msg::new(opt_key(4)),
        Msg::new(opt_key(5)),
    ];
    let picks = ctx::ask_pick_multi(
        player_id,
        &Msg::new(key!("any_color_sunset_title")),
        &Msg::new(key!("any_color_sunset_pick")),
        &options,
        3,
    )?;
    // Ascending face order -- see the module doc. The decode already returns
    // ascending indices; `apply` takes the 1-based effect number.
    for k in picks {
        apply(player_id, k as i32 + 1)?;
    }
    Ok(())
}

fn opt_key(x: i32) -> &'static str {
    match x {
        1 => key!("any_color_sunset_opt1"),
        2 => key!("any_color_sunset_opt2"),
        3 => key!("any_color_sunset_opt3"),
        4 => key!("any_color_sunset_opt4"),
        _ => key!("any_color_sunset_opt5"),
    }
}

fn apply(player_id: i32, k: i32) -> card_sdk::Asked {
    // 「投掷1d6并根据结果获得对应效果」 -- the branch is announced as a popup as
    // well as a log line, so the player sees which of the five landed.
    ctx::effect(player_id, &Msg::new(opt_key(k)));
    match k {
        // 规则书: 「若为1则立刻获得1500资金」
        1 => {
            ctx::gain(player_id, 1500, &Msg::new(key!("any_color_sunset_why")))?;
        }
        // 规则书: 「若为2则从弃牌堆中选择一张牌放置到抽牌堆顶」
        2 => {
            // TODO: Rename discard => graveyard
            if let Some(id) = pick_from_graveyard(player_id, key!("any_color_sunset_deck_ask"))? {
                ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::DeckTop);
            }
        }
        // 规则书: 「若为3则抽一张牌」
        3 => {
            ctx::draw(player_id, 1)?;
        }
        // 规则书: 「若为4则可选择补满火罐或者获得2000资金」
        4 => {
            let fire_to_fill = bandori_std::fire::max(player_id) - bandori_std::fire::get(player_id);
            let take_money = if fire_to_fill > 0 {
                // Ask the player whether to take money instead of filling the fire charge.
                ctx::ask_pick(
                    player_id,
                    &Msg::new(key!("any_color_sunset_title")),
                    &Msg::new(key!("any_color_sunset_fire_ask")),
                    &[
                        Msg::new(key!("any_color_sunset_fire_fill")),
                        Msg::new(key!("any_color_sunset_fire_money")),
                    ],
                )? != 0
            } else {
                // If the fire cannot be filled, the player must take the money.
                true
            };

            if !take_money && fire_to_fill > 0 {
                // 规则书: 「补满火罐」 
                bandori_std::fire::gain(
                    player_id,
                    fire_to_fill,
                    &Msg::new(key!("any_color_sunset_why")),
                )?;
            } else {
                // 规则书: 「获得2000资金」
                ctx::gain(player_id, 2000, &Msg::new(key!("any_color_sunset_why")))?;
            }
        }
        // 规则书: 「若为5则从弃牌堆中选择一张牌加入手牌」
        5 => {
            if let Some(id) = pick_from_graveyard(player_id, key!("any_color_sunset_hand_ask"))? {
                ctx::add_to_dest(player_id, id.as_str(), ctx::Dest::Hand);
            }
        },
        _ => unreachable!(),
    }
    Ok(())
}

/// 规则书「从弃牌堆中选择一张牌」: the player picks one card out of their
/// discard pile, which is taken out of it. `None` (and a log line) when the
/// discard is empty. The pick/take mechanism is [`ctx::pick_from_pile`]; this
/// wrapper only binds this card's prompts.
fn pick_from_graveyard(
    player_id: i32,
    ask: &'static str,
) -> Result<Option<NameId>, card_sdk::Prompt> {
    ctx::pick_from_pile(
        player_id,
        ctx::CardPile::Graveyard,
        &Msg::new(key!("any_color_sunset_title")),
        &Msg::new(ask),
        Some(&Msg::new(key!("any_color_sunset_no_discard"))),
    )
}

//

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
