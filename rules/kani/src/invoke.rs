//! Invoke unmodified `CardDef` entries against the symbolic host.
//!
//! Card bodies are the shipped `rules/cards/*` functions. We do not copy or
//! rewrite them: we pull the `fn(i32) -> Asked` out of the `On` table and
//! call it.

use card_sdk::{On, Prompt, Asked};

/// `PP:[丸山彩]憧憬的前方` -- `On::Play` (clause C-PP-aya-haruka-01 body).
pub fn aya_longing_play(player_id: i32) -> Asked {
    let card = &card_pp::aya_longing::AYA_LONGING;
    match &card.on[0] {
        On::Play(_, _, f) => f(player_id),
        _ => unreachable!("aya_longing[0] is Play"),
    }
}

/// `PP:[丸山彩]憧憬的前方` -- `On::Hook(PayAdd)` (clause C-PP-aya-haruka-02 body).
pub fn aya_longing_pay_add(player_id: i32) -> Asked {
    let card = &card_pp::aya_longing::AYA_LONGING;
    match &card.on[1] {
        On::Hook(_, _, _, f) => f(player_id),
        _ => unreachable!("aya_longing[1] is Hook"),
    }
}

/// `PPP:（多惠）寻找更好的声音` -- `On::Play` (teleport + fire).
pub fn tae_sound_play(player_id: i32) -> Asked {
    let card = &card_ppp::tae_sound::TAE_SOUND;
    match &card.on[0] {
        On::Play(_, _, f) => f(player_id),
        _ => unreachable!("tae_sound[0] is Play"),
    }
}

/// `MyGO:轮符雨` -- `On::Play` (status: [停留]).
pub fn rinne_rain_play(player_id: i32) -> Asked {
    let card = &card_mygo::rinne_rain::RINNE_RAIN;
    match &card.on[0] {
        On::Play(_, _, f) => f(player_id),
        _ => unreachable!("rinne_rain[0] is Play"),
    }
}

/// Treat `Err(Prompt)` as a hard failure for proofs that must complete
/// inline (the pilot answers prompts in-host, so this should not fire).
pub fn unwrap_done(r: Asked) {
    match r {
        Ok(()) => {}
        Err(Prompt) => panic!("unexpected EXIT_NEED_INPUT"),
    }
}
