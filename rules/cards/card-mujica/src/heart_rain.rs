//! `Mujica:心の雨` -- C# `CardHeartRain`: everyone 1-20 tiles ahead rolls 1d20
//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:心の雨`）:
//! > 心の雨：
//! >  使你前方20格内的所有玩家各自骰1d20，骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]。若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金
//!
//! A roll ≤3 grants 2 stay layers (not 1+2), 4-12 grants 1. `give_stay` never
//! refuses a grant in this engine, so "granted stay" and "gained stay" coincide
//! and the fallback branch keys off the grants.

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const HEART_RAIN: CardDef = CardDef::new("Mujica:心の雨", &[
    On::Play(None, heart_rain)]);

fn heart_rain(player_id: i32) -> card_sdk::Asked {
    let me = ctx::player_pos(player_id);
    let mut any_stay = false;
    for p in ctx::others(player_id) {
        // 规则书: 「使你前方20格内的所有玩家」
        // C# `H.Forward(pos, seats[p].pos) in 1..=20`; a same-tile player is not
        // "ahead" (`H.Forward` returns 0 there).
        let ahead = ctx::tile_forward(me, ctx::player_pos(p));
        if !(1..=20).contains(&ahead) {
            continue;
        }
        // 规则书: 「各自骰1d20」
        let roll = ctx::roll(p, 1, 20);
        // 规则书: 「骰点小于等于12的玩家获得一层[停留]，小于等于3的玩家变为获得2层[停留]」
        let layers = if roll <= 3 {
            2
        } else if roll <= 12 {
            1
        } else {
            0
        };
        if layers > 0 {
            ctx::give_stay(p, layers);
            any_stay = true;
        }
    }
    // 规则书: 「若未能使任何玩家获得[停留]，自身获得一层[晕眩]并获得1000资金」
    if !any_stay {
        ctx::log(player_id, &Msg::new(key!("heart_rain_fallback")).player_id("who", player_id));
        ctx::give_stun(player_id, 1);
        ctx::gain(player_id, 1000, &Msg::new(key!("heart_rain_why")));
    }
    Ok(())
}