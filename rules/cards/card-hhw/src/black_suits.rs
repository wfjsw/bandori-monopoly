//! `HHW:黑衣人的补给` -- C# `CardBlackSuits` (MatchHost.cs:3966-3993): [反击] past
//! CiRCLE, drop a crystal on 弦卷集团 so that tile borrows CiRCLE's effect.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:黑衣人的补给`）:
//! > 黑衣人的补给：
//! > [反击] 经过“CiRCLE”格子（#1）时可打出此卡，在“弦卷集团”（#29格）格子上放置一个奇迹水晶，该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果。你经过“弦卷集团”格子后，移除那格的一个奇迹水晶。
//!

use card_sdk::abi::{TriggerKind, ChainKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

pub const BLACK_SUITS: CardDef = CardDef::new("HHW:黑衣人的补给", &[
    On::React(&[ChainKind::Pass], can_react, react),
]);

fn can_react(player_id: i32) -> bool {
    // 规则书[反击]: 「经过“CiRCLE”格子（#1）时可打出此卡」 -- C# `CanReact`:
    // `t.Kind == "pass" && t.Seat == seat && H.Tile(t.Tile)?.kind == "circle"`.
    let circle = ctx::tile_named("CiRCLE");
    if circle < 0 {
        return false;
    }
    trigger::kind() == TriggerKind::Pass && trigger::player_id() == player_id && trigger::tile() == circle
}

fn react(player_id: i32) {
    let group = ctx::tile_named("弦卷集团");
    // 规则书[反击]: 「在“弦卷集团”（#29格）格子上放置一个奇迹水晶」 -- C#
    // `H.AddMark(tsurumakiAgent, "黑衣人的补给", c.Seat, 1, ...)`: a tile mark
    // owned by the player of this card.
    if group >= 0 {
        ctx::add_mark(
            group,
            player_id,
            key!("black_suits_crystal"),
            &Msg::new(key!("black_suits_crystal_note")),
        );
        ctx::log(
            player_id,
            &Msg::new(key!("black_suits_placed")).player_id("who", player_id).tile("tile", group),
        );
    }
    // 规则书[反击]: 「该格上拥有奇迹水晶时，该格获得“CiRCLE”格子的全部效果」 -- C#
    // landing on a tile with a 「黑衣人的补给」 mark draws 1 card like CiRCLE
    // (`MatchHost` landing, `CountMarks(at, "黑衣人的补给") > 0` -> `H.DrawR(i, 1)`),
    // and `CircleLike` folds it into the CiRCLE pass-reward path.
    // TODO(规则书): that effect copy needs the landing / pass hooks
    // (`Fx.SettleAfter`-ish `MatchHost` landing + `PassTileBuiltin`/`CircleLike`).
    // 规则书[反击]: 「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶」 -- C#
    // `BlackSuitFx.PassTile` decrements the owner's mark and drops it at 0.
    // 规则书[反击]: 「你经过“弦卷集团”格子后，移除那格的一个奇迹水晶」 -- C#
    // `BlackSuitFx.PassTile` is an `H.ExtraOf` player attachment (the hook surface
    // only dispatches to *placed* cards) and its `mark.count--` removes one
    // crystal; `ctx::remove_marks` clears every matching mark at once.
    // TODO(规则书): the `H.ExtraOf<BlackSuitFx>` attachment and the remove-one
    // mark tick are the unmapped halves.
}