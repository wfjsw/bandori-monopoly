//! `CRYCHIC:去唱卡拉ok吧` -- C# `CardKaraoke` (MatchHost.cs:2734-2748): arm this
//! turn's main move to roll up to 5 times and keep one face.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:去唱卡拉ok吧`）:
//! > 去唱卡拉ok吧：
//! >  进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const KARAOKE: CardDef = CardDef::new(
    "CRYCHIC:去唱卡拉ok吧",
    &[On::Play(Some(cant_play), karaoke)],
);

/// C# `CardKaraoke.WhyNot` = `H.MoveWhyNot(seat)`.
fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书: 「作为你本回合的移动掷骰数」 -- the card arms the turn's main move,
    // so the C# `H.MoveWhyNot` gate applies (own turn, main move still available,
    // turn's move not skipped).
    ctx::cant_move(player_id)
}

fn karaoke(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。」
    // -- C# arms `KaraokeFx` and the rolls happen when the main move rolls
    // (`KaraokeFx.RollAfter` -> `Sing`). Shaping this turn's main move is the
    // plan-flags path: roll the faces here and pin the chosen one with
    // `set_fixed_roll`, so the walk then runs as a normal dice move.
    ctx::log(
        player_id,
        &Msg::new(key!("karaoke_note")).player_id("who", player_id),
    );
    let mut results: Vec<i32> = Vec::new();
    // The first face is this turn's natural d20 (C# `m.Roll` before `Sing`).
    results.push(ctx::roll(player_id, 1, 20).max(0));
    ctx::log(
        player_id,
        &Msg::new(key!("karaoke_face"))
            .player_id("who", player_id)
            .i("n", results[0] as i64),
    );
    // 规则书: 「进行至多5次掷骰」 -- C# `while (results.Count < 5) H.AskYes(... re-roll?)`.
    while results.len() < 5 {
        let again = ctx::ask_yes(
            player_id,
            &Msg::new(key!("karaoke_title")),
            &Msg::new(key!("karaoke_again")).i("n", results.len() as i64),
        )?;
        if !again {
            break;
        }
        let x = ctx::roll(player_id, 1, 20).max(0);
        results.push(x);
        ctx::log(
            player_id,
            &Msg::new(key!("karaoke_face"))
                .player_id("who", player_id)
                .i("n", x as i64),
        );
    }
    // 规则书: 「并选择其中一个结果作为你本回合的移动掷骰数」 -- C# `H.AskPick` over the
    // faces (default the highest).
    let chosen = if results.len() == 1 {
        results[0]
    } else {
        // C# defaults the pick to the highest face.
        let options: Vec<Msg> = results
            .iter()
            .map(|&x| Msg::new(key!("karaoke_opt")).i("n", x as i64))
            .collect();
        let pick = ctx::ask_pick(
            player_id,
            &Msg::new(key!("karaoke_title")),
            &Msg::new(key!("karaoke_pick")),
            &options,
        )?;
        results[pick.min(results.len() - 1)]
    };
    // 规则书: 「视为正常掷骰移动」 -- `H._turnCtx.Plan.FixedRoll`; the main move then
    // rolls this face and walks normally.
    ctx::set_fixed_roll(chosen);
    ctx::log(
        player_id,
        &Msg::new(key!("karaoke_chosen"))
            .player_id("who", player_id)
            .i("n", chosen as i64),
    );
    Ok(())
}
