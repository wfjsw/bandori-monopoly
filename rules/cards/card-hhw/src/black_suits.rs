//! `HHW:黑衣人的补给` -- C# `CardBlackSuits` (MatchHost.cs:3966-3993): [反击] past
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:黑衣人的补给`）:
//! > 黑衣人的补给：
//! > [反击] 经过“CiRCLE”格子（#1）时可打出此卡，在“弦卷集团”（#29格）格子上放置一个奇迹水晶，该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。
//!
//! CiRCLE, drop a crystal on 弦卷集团 so that tile borrows CiRCLE's effect.
//!
//! `SETTLE-STAGES.md` §4 M3 / `docs/TILES.md`: 「该格获得"CiRCLE"格子的全部
//! 效果」 is the tile **gaining CiRCLE's effect-list entry** -- a card attaching
//! a `tile:circle` rule instance to 弦卷集团, additive alongside its own
//! `tile:agent` body. It is not an after-hook: a body replace must not leave a
//! borrowed draw running. The instance lives exactly while the crystal does.

use card_sdk::abi::{ChainKind, HookKind, MarkFilter};
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

/// The mark kind standing in for the 「奇迹水晶」 this card parks on 弦卷集团
/// (C# `H.AddMark(..., "黑衣人的补给", ...)`).
const MARK: &str = "黑衣人的补给";

pub const BLACK_SUITS: CardDef = CardDef::new(
    "HHW:黑衣人的补给",
    &[
        // 规则书: 「你经过"弦卷集团"格子后，移除那格的一个奇迹水晶」 -- the tile
        // and the owner's own pass are the condition; the crystal mark count is
        // the residual guard.
        On::Hook(
            &[HookKind::PassTile],
            "actor == owner && tile.id == tile_named('弦卷集团')",
            Some(pass_tile_guard),
            pass_tile,
        ),
        // 规则书[反击]: 「经过“CiRCLE”格子（#1）时可打出此卡」 -- C# `CanCounteract`:
        // `t.Kind == "pass" && t.Seat == seat && H.Tile(t.Tile)?.kind == "circle"`.
        // `PassBefore` is the category; the owner's own pass of CiRCLE is the
        // condition. The `circle < 0` defensive check is dropped (`tile_named`
        // missing = -1 never equals a live `tile.id`).
        On::Counteract(
            &[ChainKind::PassBefore],
            "actor == owner && tile.id == tile_named('CiRCLE')",
            None,
            counteract,
        ),
    ],
);

fn counteract(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    // 规则书[反击]: 「在“弦卷集团”（#29格）格子上放置一个奇迹水晶」 -- C#
    // `H.AddMark(tsurumakiAgent, "黑衣人的补给", c.Seat, 1, ...)`: a tile mark
    // owned by the player of this card. `place_mark_new`: one row per crystal.
    if group >= 0 {
        ctx::place_mark_new(
            group,
            MARK,
            "",
            player_id,
            ctx::self_uid(),
            1,
            &Msg::new(key!("black_suits_crystal_note")),
        );
        ctx::log(
            player_id,
            &Msg::new(key!("black_suits_placed"))
                .player_id("who", player_id)
                .tile("tile", group),
        );
        // 「该格上拥有奇迹水晶时，该格获得"CiRCLE"格子的全部效果」 --
        // `docs/TILES.md` / `SETTLE-STAGES.md` §4 M3: attach a `tile:circle`
        // rule instance to 弦卷集团, additive next to its `tile:agent` body.
        // The instance's own `On::Settle` (landing draw) and Pass entry (the
        // [经过] reward) are what 「全部效果」 means; no card hook draws.
        sync_circle_instance(group);
    }
    Ok(())
}

/// Attach or drop the borrowed `tile:circle` instance so it exists exactly
/// while 弦卷集团 carries a crystal.
fn sync_circle_instance(group: i32) {
    let has = ctx::count_marks(group, &MarkFilter::any().kind(MARK)) > 0;
    // `place_card_on(BOARD_OWNER, tile, …)` is the "attach a rule to a tile"
    // gesture (`game_core::state::BOARD_OWNER`, `docs/TILES.md`). The board
    // field is `field_instances(-1)`; a borrowed `tile:circle` on *this* tile
    // is one whose `tile_at` is the group (the real CiRCLE's own instance
    // governs the CiRCLE square and is left alone).
    let borrowed = ctx::field_instances(-1)
        .into_iter()
        .find(|(uid, id)| id == "tile:circle" && ctx::tile_at(*uid) == Some(group))
        .map(|(uid, _)| uid);
    if has && borrowed.is_none() {
        ctx::place_card_on(-1, group, "tile:circle", &Msg::new(key!("black_suits_attach")));
    } else if !has {
        if let Some(uid) = borrowed {
            ctx::unplace_at(uid);
        }
    }
}

/// Residual guard for [`pass_tile`] -- the crystal mark count on the tile
/// (`count_marks`) stays here (not yet in the condition vocabulary).
fn pass_tile_guard(_player_id: i32) -> bool {
    ctx::count_marks(ctx::trigger::tile(), &MarkFilter::any().kind(MARK)) > 0
}

/// 「你经过"弦卷集团"格子后，移除那格的一个奇迹水晶」 -- `BlackSuitFx.PassTile`'s
/// `mark.count--`, which is `bump_mark`'s single-tick form. Dropping the last
/// crystal takes the borrowed `tile:circle` instance with it.
fn pass_tile(_player_id: i32) -> card_sdk::Asked {
    // `actor == owner && tile.id == tile_named('弦卷集团')` is the pre; the
    // crystal mark count is the residual guard.
    let t = ctx::trigger::tile();
    // `MarkFilter::any()` keeps the old `owner: -2` "any owner" match.
    ctx::bump_mark(t, &MarkFilter::any().kind(MARK), -1);
    sync_circle_instance(t);
    Ok(())
}
