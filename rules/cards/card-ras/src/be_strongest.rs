//! `RAS:成为最强` -- C# `CardBeStrongest` (MatchHost.cs:9532-9564): roll 1d10
//! and teleport to the matching Live House tile.
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:成为最强`）:
//! > 成为最强：
//! >  roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”），若你没有Livehouse格子，传送到“Live House”。视为你的主要移动。
//!

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const BE_STRONGEST: CardDef = CardDef::new("RAS:成为最强", &[
    On::Play(be_strongest),
]);

/// C# `H._tiles[t].group == 6` in tile-index order (board.json). Index 0 is the
/// destination for a 1d10 roll of 1.
const LIVEHOUSES: [&str; 9] = [
    "RiNG 1",
    "RiNG 2",
    "DUB MUSIC EXPERIMENT",
    "Live House",
    "RiNG 3",
    "武道馆",
    "Space",
    "Live House Galaxy",
    "RiNG 4",
];

fn be_strongest(player_id: i32) {
    // 规则书: 「若你没有Livehouse格子，传送到“Live House”」
    // -- C# `H.OwnedBy(i).Any((int t) => H.IsLiveHouse(i, t))`.
    // TODO(规则书): `H.IsLiveHouse` also counts ExtraColor tiles (e.g.
    // 「游击演出」's designated deed); no such hook here.
    let owns_livehouse = LIVEHOUSES.iter().any(|&n| {
        let t = ctx::tile_named(n);
        t >= 0 && ctx::tile_owner(t) == player_id
    });
    let live_house = ctx::tile_named("Live House");
    let to = if !owns_livehouse {
        // 规则书: 「若你没有Livehouse格子，传送到“Live House”」
        ctx::log(player_id, &Msg::new(key!("be_strongest_none")).player_id("who", player_id));
        live_house
    } else {
        // 规则书: 「roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”）」
        let r = ctx::roll(player_id, 1, 10);
        if r >= 10 || r as usize > LIVEHOUSES.len() {
            live_house
        } else {
            ctx::tile_named(LIVEHOUSES[(r - 1) as usize])
        }
    };
    if to >= 0 {
        ctx::teleport_to(player_id, to);
        ctx::log(player_id, &Msg::new(key!("be_strongest_to")).player_id("who", player_id).tile("tile", to));
    }
    // TODO(规则书): 「视为你的主要移动」 -- needs the H.CardMove / main-move
    // routine so this teleport consumes the turn's main move (C# `H.CardMove(c,
    // new MoveCtx { TeleportTo = teleportTo })`). Until then the player_id still gets
    // its normal main move after the teleport.
}