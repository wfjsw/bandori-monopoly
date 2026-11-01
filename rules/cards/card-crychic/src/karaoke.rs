//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:去唱卡拉ok吧`）:
//! > 去唱卡拉ok吧：
//! >  进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。
//!
//! turn's main move to roll up to 5 times and keep one face.

use alloc::vec::Vec;

use card_sdk::abi::{roll_source, RollPurpose};
use card_sdk::CardText;
use card_sdk::{ctx, key, CardDef, Msg, On};

pub const KARAOKE: CardDef = CardDef::new(
    "CRYCHIC:去唱卡拉ok吧",
    &[On::Play("", Some(cant_play), karaoke)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"去唱卡拉ok吧"#)
        .raw_name(r#"去唱卡拉ok吧"#)
        .tags(&[])
        .text(CardText::para(
            r#"进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数，视为正常掷骰移动。"#,
        ))
        // sheet cell M4
        .fields(&[("group", r#"CRYCHIC"#), ("owner", "")]),
);

fn cant_play(player_id: i32) -> Option<Msg> {
    rules_status::move_blocked(player_id)
}

fn karaoke(player_id: i32) -> card_sdk::Asked {
    // Shaping this turn's main move is the plan-flags path: roll the faces here and
    // pin the chosen one with `set_fixed_roll`, so the walk then runs as a normal
    // dice move.
    ctx::log(
        player_id,
        &Msg::new(key!("karaoke_note")).player_id("who", player_id),
    );
    let mut results: Vec<i32> = Vec::new();
    results.push(ctx::roll(player_id, 1, 20, RollPurpose::Move, roll_source::CARD)?.max(0));
    ctx::log(
        player_id,
        &Msg::new(key!("karaoke_face"))
            .player_id("who", player_id)
            .i("n", results[0] as i64),
    );
    // re-roll?)`.
    while results.len() < 5 {
        let again = ctx::ask_yes(
            player_id,
            &Msg::new(key!("karaoke_title")),
            &Msg::new(key!("karaoke_again")).i("n", results.len() as i64),
        )?;
        if !again {
            break;
        }
        // 规则书: 「视为正常掷骰移动」 -- a reroll is the movement roll too.
        let x = ctx::roll(player_id, 1, 20, RollPurpose::Move, roll_source::CARD)?.max(0);
        results.push(x);
        ctx::log(
            player_id,
            &Msg::new(key!("karaoke_face"))
                .player_id("who", player_id)
                .i("n", x as i64),
        );
    }
    let chosen = if results.len() == 1 {
        results[0]
    } else {
        // The options are in roll order, so the host's fallback (option 0) is
        // the first face rolled, not the highest.
        let options: Vec<Msg> = results
            .iter()
            .map(|&x| Msg::new(key!("karaoke_opt")).i("n", x as i64))
            .collect();
        let pick = ctx::ask_choice(
            player_id,
            &Msg::new(key!("karaoke_title")),
            &Msg::new(key!("karaoke_pick")),
            &options,
        )?;
        results[pick.min(results.len() - 1)]
    };
    // TODO(规则书)（视为正常掷骰移动）: the faces are the turn's movement dice (the plan's `base`
    // + `dice`) -- a dice effect that lands *after* this card is still bypassed,
    // because the pinned face wins in `run_planned_move`. `set_base_dice` clears it for its
    // own roll; other dice writers do not.
    ctx::set_fixed_roll(chosen);
    ctx::log(
        player_id,
        &Msg::new(key!("karaoke_chosen"))
            .player_id("who", player_id)
            .i("n", chosen as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn karaoke_kani(player_id: i32) -> card_sdk::Asked {
    karaoke(player_id)
}
