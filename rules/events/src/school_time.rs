//! `event:上学时间` -- 事件卡「上学时间」（中立事件 A21）.
//!
//! 事件文本（data/events.json, id `上学时间`）:
//! > 全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const SCHOOL_TIME: CardDef = CardDef::new("event:上学时间", &[On::Play("", None, play)]);

// `data/schools.json` (whose own note is 「事件「上学时间」用」) -> `SCHOOLS` /
// `FALLBACK`, generated at build time by `build.rs`.
include!(concat!(env!("OUT_DIR"), "/schools.rs"));

/// 规则书: 「操作角色当前上学的学校的格子」 -- the school tile of `player_id`'s
/// character. 规则书: 「如果剧情没有明说则移动到周边学区」 -- the fallback.
fn school_tile(player_id: i32) -> i32 {
    for &(name, school) in SCHOOLS {
        if ctx::character_is(player_id, name) {
            return ctx::tile_named(school);
        }
    }
    ctx::tile_named(FALLBACK)
}

/// 规则书: 「全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]」 --
/// teleport, then one [除外] layer (no return tile named in the text).
fn play(_player_id: i32) -> card_sdk::Asked {
    for p in all_players() {
        let tile = school_tile(p);
        // 「移动到…的格子」 -- a jump, no settle clause in the text.
        ctx::teleport_to(p, tile);
        // 「然后获得1层[除外]」 -- `exile_to` left at -1 (no named return).
        ctx::give_exile(p, 1, -1);
        ctx::log(
            p,
            &Msg::new("log.event.school_time_to")
                .player_id("who", p)
                .tile("tile", tile),
        );
    }
    Ok(())
}