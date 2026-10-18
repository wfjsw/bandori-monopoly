//! `event:Kizuna Music` -- 事件卡「Kizuna Music」（中立事件 A23）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `Kizuna Music`）:
//! > 触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的格子并获得1层[除外]，X为所有玩家卡组里卡名为歌名的卡数量的总数

use card_sdk::abi::{prop, CardPile};
use ruleset_core::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const KIZUNA_MUSIC: CardDef = CardDef::new("event:Kizuna Music", &[On::Play("", "", None, play)])
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"Kizuna Music"#)
            .text(r#"触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的格子并获得1层[除外]，X为所有玩家卡组里卡名为歌名的卡数量的总数"#)
            .cell(r#"A23"#)
            .derived(false));

/// 「X为所有玩家卡组里卡名为歌名的卡数量的总数」 -- every song-title card in
/// every player's draw pile. The song marker is the static card property
/// [`prop::SONG`] on the twenty song-title cards.
/// TODO(规则书): 「卡组」 is read as the draw pile (`CardPile::Deck`). If it
/// means the player's whole card pool, hand / discard / field are short here.
fn song_count() -> i32 {
    let mut n = 0;
    for p in all_players() {
        for c in ctx::cards_in(p, CardPile::Deck) {
            if ctx::card_prop(&c, prop::SONG) != 0 {
                n += 1;
            }
        }
    }
    n
}

/// 规则书: 「触发此卡的玩家投掷X+1d20」 -- the drawer rolls 1d20 and adds X
/// (a bare roll, no [反击] window). 「并所有玩家依次[传送]到投掷结果mod60
/// 所对应的格子并获得1层[除外]」 -- everyone teleports to that tile (a
/// [传送], so no settle) and gains one exile layer.
fn play(player_id: i32) -> card_sdk::Asked {
    let x = song_count();
    ctx::log(
        player_id,
        &Msg::new("log.event.kizuna_x").player_id("who", player_id).i("n", x as i64),
    );
    let face = x + roll(player_id, 1, 20);
    ctx::log(
        player_id,
        &Msg::new("log.event.dice").player_id("who", player_id).i("n", face as i64),
    );
    // 「投掷结果mod60所对应的格子」 -- the board is 60 tiles (0..=59).
    let dest = face.rem_euclid(60);
    for &p in &all_players() {
        // 「[传送]」 -- a position write, not a move; no landing settle.
        ctx::teleport_to(p, dest);
        // 「并获得1层[除外]」 -- one plain exile layer; the engine ticks it off
        // at the player's next turn start (`game-core` exile tick).
        rules_status::exile::send(p, 1, -1);
        ctx::log(
            player_id,
            &Msg::new("log.event.kizuna_tp")
                .player_id("who", p)
                .tile("tile", dest),
        );
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}
