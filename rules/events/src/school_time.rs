//! `event:上学时间` -- 事件卡「上学时间」（中立事件 A21）.
//!
//! 事件文本（data/events.json, id `上学时间`）:
//! > 全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。

use card_sdk::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const SCHOOL_TIME: CardDef = CardDef::new("event:上学时间", &[On::Play(None, play, "")]);

/// `data/schools.json` (whose own note is 「事件「上学时间」用」), inlined: `ctx`
/// has no school lookup, so the character -> school-tile map lives here.
/// TODO(规则书): no `ctx::school_of`; this table must stay in step with
/// `data/schools.json`.
const SCHOOLS: &[(&str, &[&str])] = &[
    (
        "花咲川女子学院",
        &[
            "户山香澄",
            "花园多惠",
            "牛込里美",
            "山吹沙绫",
            "市谷有咲",
            "弦卷心",
            "松原花音",
            "奥泽美咲",
            "北泽育美",
            "丸山彩",
            "白鹭千圣",
            "若宫伊芙",
            "冰川纱夜",
            "白金燐子",
            "要乐奈",
            "椎名立希",
        ],
    ),
    (
        "羽丘女子学院",
        &[
            "美竹兰",
            "青叶摩卡",
            "上原绯玛丽",
            "宇田川巴",
            "羽泽鸫",
            "濑田薰",
            "冰川日菜",
            "大和麻弥",
            "凑友希那",
            "今井莉莎",
            "宇田川亚子",
            "高松灯",
            "千早爱音",
            "丰川祥子",
        ],
    ),
    (
        "月之森女子学院",
        &[
            "仓田真白",
            "桐谷透子",
            "广町七深",
            "二叶筑紫",
            "八潮瑠唯",
            "长崎素世",
            "若叶睦",
            "长崎素世（CRYCHIC）",
            "若叶睦（CRYCHIC）",
            "丰川祥子（CRYCHIC）",
        ],
    ),
];

/// 规则书: 「操作角色当前上学的学校的格子」 -- the school tile of `player_id`'s
/// character. 规则书: 「如果剧情没有明说则移动到周边学区」 -- the fallback.
fn school_tile(player_id: i32) -> i32 {
    for (school, roster) in SCHOOLS {
        if roster.iter().any(|&name| ctx::character_is(player_id, name)) {
            return ctx::tile_named(school);
        }
    }
    ctx::tile_named("周边学区")
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