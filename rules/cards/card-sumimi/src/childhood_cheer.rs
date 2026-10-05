//! `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励` -- C# `CardChildhoodCheer`
//! (MatchHost.cs:11481-11498): move starts from 小豆岛, +1 fire afterwards.
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励`）:
//! > (初华（Sumimi）)儿时玩伴的鼓励：
//! >  可在移动掷骰前打出此卡，使本次移动以“小豆岛”为起点并在移动后获得一个火罐。
//!

use card_sdk::{ctx, key, CardDef, Msg};

pub const CHILDHOOD_CHEER: CardDef = CardDef {
    id: "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励",
    play: Some(childhood_cheer),
    can_react: None,
    react: None,
    why_not: None,
};

fn childhood_cheer(seat: i32) {
    // TODO(规则书): 「可在移动掷骰前打出此卡」 -- the C# `CardChildhoodCheer.WhyNot`
    // is just `H.MoveWhyNot` (own turn / not yet main-moved); that hook is still
    // missing, so there is nothing for `why_not` to express here yet.
    // 规则书: 「使本次移动以“小豆岛”为起点」 -- C# `H._turnCtx.Plan.Start =
    // H.TileNamed("小豆岛")`. The move plan hook is missing, so approximate by
    // putting the seat on 小豆岛 without settling; the dice move then starts there.
    let start = ctx::tile_named("小豆岛");
    if start >= 0 {
        ctx::teleport_to(seat, start);
        ctx::log(seat, &Msg::new(key!("childhood_cheer_start")).seat("who", seat).tile("tile", start));
    }
    // TODO(规则书): 「并在移动后获得一个火罐」 -- needs the Fx.Arrive hook after
    // the main move (C# `AfterMoveFireFx.Arrive` -> `H.GainFire(Seat, 1, ...)`;
    // also `TurnEndAfter` to drop the attachment). The vocabulary only has
    // `gain_fire` now, which would pay the fire out before the move.
    // TODO(规则书): the C# `Plan.Start` form of 「以“小豆岛”为起点」 keeps the seat
    // put until the move runs (and rolls back cleanly if the move never happens);
    // the `teleport_to` stand-in above cannot.
}