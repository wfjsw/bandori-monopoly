//! `MyGO:那天的雨` -- C# `CardThatDayRain` (MatchHost.cs:6855-6914): a
//! crystal-decay field card that, on play and every own turn start, rolls 1d10
//! and gives [停留] to everyone on (or next to) that agent's colour.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:那天的雨`）:
//! > 那天的雨：
//! >  将此卡放置于自己场上并为其放置5个奇迹水晶，打出此卡时及你的每回合开始时投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]，每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃牌堆（骰点大于10则固定为高级住宅区对应颜色的格子）
//!

use alloc::vec::Vec;

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const THAT_DAY_RAIN: CardDef = CardDef::new("MyGO:那天的雨", &[
    On::Play(that_day_rain),
    // C# `CardThatDayRain : DecayCard` (TurnEnd burn) and `TurnStart` -> `Rain`.
    On::Hook(&[TriggerKind::TurnStart, TriggerKind::TurnEnd], hook),
]);

const ID: &str = "MyGO:那天的雨";

/// C# `CardThatDayRain.Rain` -- the (地产商) agent tiles in board order with
/// 东京外 excluded, each paired with the buyable tiles of its colour group
/// (C# `H._tiles[t].kind == "agent"` / `.group` / `IsBuyable`).
const AGENTS: &[(&str, &[&str])] = &[
    ("主要街道", &["购物中心", "偶像经纪公司", "星空齿科", "江户川公园"]),
    ("中心学区", &["花咲川女子学院", "羽丘女子学院", "月之森女子学院"]),
    ("周边学区", &["白雪学园", "艺术学院高中", "瑟罗希亚国际学校", "加茂川中央中学"]),
    ("弦卷集团", &["微笑号", "米歇尔公园", "弦卷豪宅"]),
    (
        "Live House",
        &["RiNG 1", "RiNG 2", "RiNG 3", "RiNG 4", "DUB MUSIC EXPERIMENT", "武道馆", "Space", "Live House Galaxy"],
    ),
    ("周边精选", &["天文馆", "水族馆", "富士见坂", "成为人类桥", "便利店", "快餐店", "Bandori车站", "飞鸟山公园"]),
    ("大学路", &["庆鹏女子大学", "四叶女子大学"]),
    ("梦开始的地方", &["星星小巷", "星之鼓动山丘"]),
    ("商店街", &["羽泽咖啡厅", "山吹面包房", "银河拉面馆", "北泽精肉店", "旭汤澡堂"]),
    ("高级住宅区", &["六本木大厦", "CHUCHU的公寓", "旧古河庭园", "广町家画室"]),
];

/// C# `CardThatDayRain.TurnStart` (own turns -> `Rain`) and `DecayCard.TurnEnd`.
fn hook(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    match trigger::kind() {
        // 规则书: 「及你的每回合开始时投掷1d10并...」 -- C# `TurnStart(int turn)`
        // refuses other players' turns (`turn != Player`).
        TriggerKind::TurnStart => {
            if trigger::player_id() == player_id {
                rain(player_id);
            }
        }
        // 规则书: 「每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃牌堆」
        // -- C# `DecayCard.TurnEnd` / `Decay` (`AddCrystals(-1)`; empty ->
        // `H.Unplace(this, "discard", ...)`), on the owner's turn end.
        TriggerKind::TurnEnd => {
            if trigger::player_id() != player_id {
                return;
            }
            if ctx::decay(player_id, ID) == 0 {
                ctx::log(player_id, &Msg::new(key!("that_day_rain_decayed")).player_id("who", player_id));
            }
        }
        _ => {}
    }
}

fn that_day_rain(player_id: i32) {
    // 规则书: 「将此卡放置于自己场上并为其放置5个奇迹水晶」 -- C#
    // `H.PlaceFromPlay(c, -1, -1, 5)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("that_day_rain_note")));
    // 规则书: 「并为其放置5个奇迹水晶」 -- C# `PlaceFromPlay(..., crystals: 5)`.
    ctx::set_crystals(player_id, 5);
    ctx::log(player_id, &Msg::new(key!("that_day_rain_placed")).player_id("who", player_id));
    // 规则书: 「打出此卡时...投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商
    // 对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]」
    rain(player_id);
}

/// C# `CardThatDayRain.Rain` -- the 1d10 -> agent -> colour-and-neighbours stay.
fn rain(player_id: i32) {
    // 规则书: 「投掷1d10」 -- C# `H.Roll(Seat, 1, 10, CardName)`.
    let r = ctx::roll(player_id, 1, 10);
    // 规则书: 「按地产商格子顺序使（除“东京外”的）第n个地产商」
    // 规则书: 「（骰点大于10则固定为高级住宅区对应颜色的格子）」 -- a d10 cannot
    // exceed 10, so the fallback is only a safety net (C# `num > list.Count ||
    // num > 10` -> `H.TileNamed("高级住宅区")`, the last agent in board order).
    let idx = if r as usize > AGENTS.len() {
        AGENTS.len() - 1
    } else {
        (r - 1).max(0) as usize
    };
    let (agent, color) = AGENTS[idx];
    let n = ctx::tile_count();
    // 规则书: 「对应的颜色格子及这些格子相邻格子上的所有玩家」 -- C# builds `area`
    // from every buyable tile of the agent's group plus its two ring neighbours.
    let mut area: Vec<i32> = Vec::new();
    for &name in color {
        let t = ctx::tile_named(name);
        if t < 0 || n <= 0 {
            continue;
        }
        for d in [t, (t + 1) % n, (t - 1 + n) % n] {
            if !area.contains(&d) {
                area.push(d);
            }
        }
    }
    let agent_tile = ctx::tile_named(agent);
    ctx::log(
        player_id,
        &Msg::new(key!("that_day_rain_log"))
            .i("roll", r as i64)
            .tile("tile", agent_tile)
            .card("card", ID),
    );
    // 规则书: 「所有玩家获得一层[停留]」 -- C# `H.GiveStay(p, 1, Seat, CardName)`
    // for every present player standing in `area`.
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) {
            continue;
        }
        let pos = ctx::player_pos(p);
        if area.contains(&pos) {
            ctx::give_stay(p, 1);
            ctx::log(p, &Msg::new(key!("that_day_rain_stay")).player_id("who", p));
        }
    }
}