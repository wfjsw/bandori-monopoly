//! `event:上学时间` -- 事件卡「上学时间」（中立事件 A21）.
//!
//! 事件文本（`CardMeta` beside the rule body, id `上学时间`）:
//! > 全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。

use bandori_std::ctx;
use card_sdk::{CardDef, Msg, On};

use crate::util::all_players;

pub const SCHOOL_TIME: CardDef = CardDef::new("event:上学时间", &[On::Play("", "", None, play)])
        .meta(&card_sdk::meta::CardMeta::new()
            .name(r#"上学时间"#)
            .text(r#"全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情没有明说则移动到周边学区）。"#)
            .cell(r#"A21"#)
            .derived(false));

/// 规则书: 「如果剧情没有明说则移动到周边学区」 -- the fallback school tile.
const FALLBACK: &str = "周边学区";

/// Persona -> school tile name (「事件「上学时间」用」). Story-unlisted
/// personas fall back to [`FALLBACK`]. Data lives with the rule; the retired
/// `data/schools.json` is gone.
const SCHOOLS: &[(&str, &str)] = &[
    ("户山香澄", "花咲川女子学院"),
    ("花园多惠", "花咲川女子学院"),
    ("牛込里美", "花咲川女子学院"),
    ("山吹沙绫", "花咲川女子学院"),
    ("市谷有咲", "花咲川女子学院"),
    ("弦卷心", "花咲川女子学院"),
    ("松原花音", "花咲川女子学院"),
    ("奥泽美咲", "花咲川女子学院"),
    ("北泽育美", "花咲川女子学院"),
    ("丸山彩", "花咲川女子学院"),
    ("白鹭千圣", "花咲川女子学院"),
    ("若宫伊芙", "花咲川女子学院"),
    ("冰川纱夜", "花咲川女子学院"),
    ("白金燐子", "花咲川女子学院"),
    ("要乐奈", "花咲川女子学院"),
    ("椎名立希", "花咲川女子学院"),
    ("美竹兰", "羽丘女子学院"),
    ("青叶摩卡", "羽丘女子学院"),
    ("上原绯玛丽", "羽丘女子学院"),
    ("宇田川巴", "羽丘女子学院"),
    ("羽泽鸫", "羽丘女子学院"),
    ("濑田薰", "羽丘女子学院"),
    ("冰川日菜", "羽丘女子学院"),
    ("大和麻弥", "羽丘女子学院"),
    ("凑友希那", "羽丘女子学院"),
    ("今井莉莎", "羽丘女子学院"),
    ("宇田川亚子", "羽丘女子学院"),
    ("高松灯", "羽丘女子学院"),
    ("千早爱音", "羽丘女子学院"),
    ("丰川祥子", "羽丘女子学院"),
    ("仓田真白", "月之森女子学院"),
    ("桐谷透子", "月之森女子学院"),
    ("广町七深", "月之森女子学院"),
    ("二叶筑紫", "月之森女子学院"),
    ("八潮瑠唯", "月之森女子学院"),
    ("长崎素世", "月之森女子学院"),
    ("若叶睦", "月之森女子学院"),
    ("长崎素世（CRYCHIC）", "月之森女子学院"),
    ("若叶睦（CRYCHIC）", "月之森女子学院"),
    ("丰川祥子（CRYCHIC）", "月之森女子学院"),
];

/// 规则书: 「操作角色当前上学的学校的格子」 -- the school tile of `player_id`'s
/// character. 规则书: 「如果剧情没有明说则移动到周边学区」 -- the fallback.
fn school_tile(player_id: i32) -> i32 {
    for &(name, school) in SCHOOLS {
        if ctx::persona_is(player_id, name) {
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
        rules_status::exile::send(p, 1, -1);
        ctx::log(
            p,
            &Msg::new("log.event.school_time_to")
                .player_id("who", p)
                .tile("tile", tile),
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
