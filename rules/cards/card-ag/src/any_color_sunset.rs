//! `AG:无论是何种颜色的夕阳` -- C# `CardAnyColorSunset` (MatchHost.cs:1198-1285):
//!
//! 规则书（docs/rulebook/cards.json, id `AG:无论是何种颜色的夕阳`）:
//! > 无论是何种颜色的夕阳：
//! >  投掷1d6并根据结果获得对应效果: 若为1则立刻获得1500资金，若为2则从弃牌堆中选择一张牌放置到抽牌堆顶，若为3则抽一张牌，若为4则可选择补满火罐或者获得2000资金，若为5则从弃牌堆中选择一张牌加入手牌，若为6则选择1-5中的3个效果触发（若结果严格大于6，则从1开始重新计数）
//!
//! 1d6 picks one of five effects (6 = pick three of them).

use alloc::string::String;
use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const ANY_COLOR_SUNSET: CardDef =
    CardDef::new("AG:无论是何种颜色的夕阳", &[On::Play(None, play)]);

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「投掷1d6并根据结果获得对应效果」; 「若结果严格大于6，则从1开始重新计数」
    // -- C# `((H.Roll(i, 1, 6) - 1) % 6) + 1`.
    let raw = ctx::roll(player_id, 1, 6);
    let k = (raw - 1).rem_euclid(6) + 1;
    if k <= 5 {
        // 规则书: 「若为1则…若为5则…」 (single-effect branch)
        apply(player_id, k)?;
        return Ok(());
    }
    // 规则书: 「若为6则选择1-5中的3个效果触发」
    let mut left: [bool; 5] = [true; 5];
    for n in 0..3 {
        let mut options: Vec<Msg> = Vec::new();
        let mut ids: Vec<i32> = Vec::new();
        for x in 1..=5 {
            if left[(x - 1) as usize] {
                options.push(Msg::new(opt_key(x)));
                ids.push(x);
            }
        }
        if ids.is_empty() {
            break;
        }
        let pick = ctx::ask_pick(
            player_id,
            &Msg::new(key!("any_color_sunset_title")),
            &Msg::new(key!("any_color_sunset_pick")).i("n", n as i64 + 1),
            &options,
        )?;
        let k = ids[pick.min(ids.len() - 1)];
        left[(k - 1) as usize] = false;
        apply(player_id, k)?;
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
            ctx::gain(player_id, 1500, &Msg::new(key!("any_color_sunset_why")));
        }
        // 规则书: 「若为2则从弃牌堆中选择一张牌放置到抽牌堆顶」
        2 => {
            if let Some(id) = pick_from_discard(player_id, key!("any_color_sunset_deck_ask"))? {
                ctx::add_to_deck_at(player_id, &id, ctx::DeckPos::Top);
            }
        }
        // 规则书: 「若为3则抽一张牌」
        3 => {
            ctx::draw(player_id, 1);
        }
        // 规则书: 「若为4则可选择补满火罐或者获得2000资金」
        4 => {
            // C# offers 「补满火罐」 only when `H.HasFireSkill(i) && H.Fire(i) < H.FireMax(i)`.
            let can_fire =
                ctx::fire_max(player_id) > 0 && ctx::fire(player_id) < ctx::fire_max(player_id);
            let mut take_money = true;
            if can_fire {
                let pick = ctx::ask_pick(
                    player_id,
                    &Msg::new(key!("any_color_sunset_title")),
                    &Msg::new(key!("any_color_sunset_fire_ask")),
                    &[
                        Msg::new(key!("any_color_sunset_fire_fill")),
                        Msg::new(key!("any_color_sunset_fire_money")),
                    ],
                )?;
                take_money = pick != 0;
            }
            if !take_money {
                // 规则书: 「补满火罐」 -- C# `H.GainFire(i, H.FireMax(i) - fire, ...)`.
                let need = ctx::fire_max(player_id) - ctx::fire(player_id);
                if need > 0 {
                    ctx::gain_fire(player_id, need, &Msg::new(key!("any_color_sunset_why")));
                }
            } else {
                // 规则书: 「获得2000资金」
                ctx::gain(player_id, 2000, &Msg::new(key!("any_color_sunset_why")));
            }
        }
        // 规则书: 「若为5则从弃牌堆中选择一张牌加入手牌」
        _ => {
            if let Some(id) = pick_from_discard(player_id, key!("any_color_sunset_hand_ask"))? {
                ctx::add_to_hand(player_id, &id);
            }
        }
    }
    Ok(())
}

/// C# `h.discard.Distinct()` -> `H.AskCard` -> `h.discard.Remove(text)`: the
/// player picks one card out of their discard pile, which is taken out of it.
/// `None` (and a log line) when the discard is empty.
fn pick_from_discard(
    player_id: i32,
    ask: &'static str,
) -> Result<Option<String>, card_sdk::Prompt> {
    let mut ids: Vec<String> = Vec::new();
    for c in ctx::cards_in(player_id, ctx::CardPile::Discard) {
        if !ids.contains(&c) {
            ids.push(c);
        }
    }
    if ids.is_empty() {
        ctx::log(player_id, &Msg::new(key!("any_color_sunset_no_discard")));
        return Ok(None);
    }
    let refs: Vec<&str> = ids.iter().map(|c| c.as_str()).collect();
    let pick = ctx::ask_card(
        player_id,
        &Msg::new(key!("any_color_sunset_title")),
        &Msg::new(ask),
        &refs,
    )?;
    let id = ids.swap_remove(pick.min(ids.len() - 1));
    Ok(ctx::take_card(player_id, ctx::CardPile::Discard, &id).then_some(id))
}
