//! `AG:无论是何种颜色的夕阳` -- C# `CardAnyColorSunset` (MatchHost.cs:1198-1285):
//! 1d6 picks one of five effects (6 = pick three of them).
//!
//! 规则书（docs/rulebook/cards.json, id `AG:无论是何种颜色的夕阳`）:
//! > 无论是何种颜色的夕阳：
//! >  投掷1d6并根据结果获得对应效果: 若为1则立刻获得1500资金，若为2则从弃牌堆中选择一张牌放置到抽牌堆顶，若为3则抽一张牌，若为4则可选择补满火罐或者获得2000资金，若为5则从弃牌堆中选择一张牌加入手牌，若为6则选择1-5中的3个效果触发（若结果严格大于6，则从1开始重新计数）
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const ANY_COLOR_SUNSET: CardDef = CardDef {
    id: "AG:无论是何种颜色的夕阳",
    play: Some(play),
    can_react: None,
    react: None,
    why_not: None,
};

fn play(seat: i32) {
    // 规则书: 「投掷1d6并根据结果获得对应效果」; 「若结果严格大于6，则从1开始重新计数」
    // -- C# `((H.Roll(i, 1, 6) - 1) % 6) + 1`.
    let raw = ctx::roll(seat, 1, 6);
    let k = (raw - 1).rem_euclid(6) + 1;
    if k <= 5 {
        // 规则书: 「若为1则…若为5则…」 (single-effect branch)
        effect(seat, k);
        return;
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
            seat,
            &Msg::new(key!("any_color_sunset_title")),
            &Msg::new(key!("any_color_sunset_pick")).i("n", n as i64 + 1),
            &options,
        );
        let k = ids[pick.min(ids.len() - 1)];
        left[(k - 1) as usize] = false;
        effect(seat, k);
    }
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

fn effect(seat: i32, k: i32) {
    match k {
        // 规则书: 「若为1则立刻获得1500资金」
        1 => {
            ctx::gain(seat, 1500, &Msg::new(key!("any_color_sunset_why")));
        }
        // 规则书: 「若为2则从弃牌堆中选择一张牌放置到抽牌堆顶」
        2 => {
            // TODO(ABI): the discard pile cannot be listed or taken from
            // (C# `h.discard.Distinct()` + `h.discard.Remove(text)`); the deck
            // half is now `ctx::add_to_deck_at(seat, id, DeckPos::Top)` but it
            // still needs the card id out of the discard first.
            ctx::log(seat, &Msg::new(key!("any_color_sunset_no_discard")));
        }
        // 规则书: 「若为3则抽一张牌」
        3 => {
            ctx::draw(seat, 1);
        }
        // 规则书: 「若为4则可选择补满火罐或者获得2000资金」
        4 => {
            // C# offers 「补满火罐」 only when `H.HasFireSkill(i) && H.Fire(i) < H.FireMax(i)`.
            let can_fire = ctx::fire_max(seat) > 0 && ctx::fire(seat) < ctx::fire_max(seat);
            let mut take_money = true;
            if can_fire {
                let pick = ctx::ask_pick(
                    seat,
                    &Msg::new(key!("any_color_sunset_title")),
                    &Msg::new(key!("any_color_sunset_fire_ask")),
                    &[
                        Msg::new(key!("any_color_sunset_fire_fill")),
                        Msg::new(key!("any_color_sunset_fire_money")),
                    ],
                );
                take_money = pick != 0;
            }
            if !take_money {
                // 规则书: 「补满火罐」 -- C# `H.GainFire(i, H.FireMax(i) - fire, ...)`.
                let need = ctx::fire_max(seat) - ctx::fire(seat);
                if need > 0 {
                    ctx::gain_fire(seat, need, &Msg::new(key!("any_color_sunset_why")));
                }
            } else {
                // 规则书: 「获得2000资金」
                ctx::gain(seat, 2000, &Msg::new(key!("any_color_sunset_why")));
            }
        }
        // 规则书: 「若为5则从弃牌堆中选择一张牌加入手牌」
        _ => {
            // TODO(ABI): same discard-pile query as effect 2 (C# `h.discard` +
            // `H.AddToHand(i, text)`).
            ctx::log(seat, &Msg::new(key!("any_color_sunset_no_discard")));
        }
    }
}