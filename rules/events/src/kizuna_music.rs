//! `event:Kizuna Music` -- 事件卡「Kizuna Music」（中立事件 A23）.
//!
//! 事件文本（data/events.json, id `Kizuna Music`）:
//! > 触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的格子并获得1层[除外]，X为所有玩家卡组里卡名为歌名的卡数量的总数

use card_sdk::abi::CardPile;
use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::{all_players, roll};

pub const KIZUNA_MUSIC: CardDef = CardDef::new("event:Kizuna Music", &[On::Play("", None, play)]);

/// Song-title card rule ids (`data/cards.json` `id`, for every title in
/// `data/song_cards.json`).
/// TODO(规则书)/TODO(engine): `GameData.song_cards` (the title list in
/// `data/song_cards.json`) is not reachable from `ctx`, so the ids are
/// duplicated here. The data file says it is meant to stay editable; this
/// table has to be kept in step by hand.
const SONG_CARDS: &[&str] = &[
    "PPP:Returns",
    "PPP:STAR BEAT!",
    "AG:宣战布告",
    "AG:Y.O.L.O",
    "AG:ONE OF US",
    "PP:TITLE IDOL",
    "R:（ykn）louder",
    "R:Fire bird",
    "R:（燐子）Ringing Bloom",
    "R:Sprechchor",
    "R:轨迹",
    "RAS:R. I. O. T.",
    "RAS:狂乱Hey Kids!!",
    "RAS:UNSTOPPABLE",
    "MyGO:壱雫空",
    "MyGO:无路矢",
    "MyGO:轮符雨",
    "Mujica:黑色生日",
    "Mujica:骰子已经掷下",
    "CRYCHIC:春日影",
];

/// 「X为所有玩家卡组里卡名为歌名的卡数量的总数」 -- every song-title card in
/// every player's draw pile.
/// TODO(规则书): 「卡组」 is read as the draw pile (`CardPile::Deck`). If it
/// means the player's whole card pool, hand / discard / field are short here.
fn song_count() -> i32 {
    let mut n = 0;
    for p in all_players() {
        for c in ctx::cards_in(p, CardPile::Deck) {
            if SONG_CARDS.contains(&c.as_str()) {
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
        ctx::give_exile(p, 1, -1);
        ctx::log(
            player_id,
            &Msg::new("log.event.kizuna_tp")
                .player_id("who", p)
                .tile("tile", dest),
        );
    }
    Ok(())
}