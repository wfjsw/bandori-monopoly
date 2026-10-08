//! `RAS:成为最强` -- C# `CardBeStrongest` (MatchHost.cs:9532-9564): roll 1d10
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:成为最强`）:
//! > 成为最强：
//! >  roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到“Live House”），若你没有Livehouse格子，传送到“Live House”。视为你的主要移动。
//!
//! and teleport to the matching Live House tile.

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const BE_STRONGEST: CardDef =
    CardDef::new("RAS:成为最强", &[On::Play(Some(cant_play), be_strongest, "")]);

/// C# `CardBeStrongest.WhyNot` = `H.MoveWhyNot(seat)` -- the teleport is the
/// turn's main move.
fn cant_play(player_id: i32) -> Option<Msg> {
    ctx::cant_move(player_id)
}

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

fn be_strongest(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「若你没有Livehouse格子，传送到“Live House”」
    // -- C# `H.OwnedBy(i).Any((int t) => H.IsLiveHouse(i, t))`, which also
    // counts an `Fx.ExtraColor` deed (e.g. 「游击演出」's designated one).
    let owns_livehouse = ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| ctx::is_live_house_for(player_id, t));
    let live_house = ctx::tile_named("Live House");
    let to = if !owns_livehouse {
        // 规则书: 「若你没有Livehouse格子，传送到“Live House”」
        ctx::log(
            player_id,
            &Msg::new(key!("be_strongest_none")).player_id("who", player_id),
        );
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
    if to < 0 {
        return Ok(());
    }
    ctx::log(
        player_id,
        &Msg::new(key!("be_strongest_to"))
            .player_id("who", player_id)
            .tile("tile", to),
    );
    // 规则书: 「视为你的主要移动」 -- C# `H.CardMove(c, new MoveCtx { TeleportTo
    // = teleportTo })`: a teleport to the chosen tile that consumes the turn's
    // main move and settles where it lands (Resolve defaults to true).
    ctx::plan::set_teleport_to(to);
    ctx::card_move(player_id);
    Ok(())
}
