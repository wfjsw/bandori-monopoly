//! `MyGO:那天的雨` -- C# `CardThatDayRain` (MatchHost.cs:6855-6914): a
//! crystal-decay field card that, on play and every own turn start, rolls 1d10
//! and gives [停留] to everyone on (or next to) that agent's colour.
//!
//! 规则书（docs/rulebook/cards.json, id `MyGO:那天的雨`）:
//! > 那天的雨：
//! >  将此卡放置于自己场上并为其放置5个奇迹水晶，打出此卡时及你的每回合开始时投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]，每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃牌堆（骰点大于10则固定为高级住宅区对应颜色的格子）
//!

use alloc::vec::Vec;

use card_sdk::{ctx, key, CardDef, Msg};

pub const THAT_DAY_RAIN: CardDef = CardDef {
    id: "MyGO:那天的雨",
    play: Some(that_day_rain),
    can_react: None,
    react: None,
    why_not: None,
};

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

fn that_day_rain(seat: i32) {
    // 规则书: 「将此卡放置于自己场上并为其放置5个奇迹水晶」 -- C#
    // `H.PlaceFromPlay(c, -1, -1, 5)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "MyGO:那天的雨", &Msg::new(key!("that_day_rain_note")));
    ctx::log(seat, &Msg::new(key!("that_day_rain_placed")).seat("who", seat));
    // TODO(规则书): 「并为其放置5个奇迹水晶」 -- needs a per-card crystal counter
    // (`PlaceFromPlay(..., crystals: 5)`).
    // 规则书: 「打出此卡时...投掷1d10并按地产商格子顺序使（除“东京外”的）第n个地产商
    // 对应的颜色格子及这些格子相邻格子上的所有玩家获得一层[停留]」
    rain(seat);
    // TODO(规则书): 「及你的每回合开始时投掷1d10并...」 -- needs the Fx.TurnStart
    // hook (C# `CardThatDayRain.TurnStart` -> `Rain`).
    // TODO(规则书): 「每回合结束时移除一个奇迹水晶，移除所有奇迹水晶后将其放入弃牌堆」
    // -- needs the Fx.TurnEnd decay (C# `DecayCard.TurnEnd` / `Decay`) plus the
    // card crystal counter.
}

/// C# `CardThatDayRain.Rain` -- the 1d10 -> agent -> colour-and-neighbours stay.
fn rain(seat: i32) {
    // 规则书: 「投掷1d10」 -- C# `H.Roll(Seat, 1, 10, CardName)`.
    let r = ctx::roll(seat, 1, 10);
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
        seat,
        &Msg::new(key!("that_day_rain_log"))
            .i("roll", r as i64)
            .tile("tile", agent_tile)
            .card("card", "MyGO:那天的雨"),
    );
    // 规则书: 「所有玩家获得一层[停留]」 -- C# `H.GiveStay(p, 1, Seat, CardName)`
    // for every present seat standing in `area`.
    for p in 0..ctx::seat_count() {
        if ctx::seat_out(p) {
            continue;
        }
        let pos = ctx::seat_pos(p);
        if area.contains(&pos) {
            ctx::give_stay(p, 1);
            ctx::log(p, &Msg::new(key!("that_day_rain_stay")).seat("who", p));
        }
    }
}