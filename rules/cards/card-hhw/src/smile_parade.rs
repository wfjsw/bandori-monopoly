//! `HHW:笑容大游行` -- C# `CardSmileParade` (MatchHost.cs:3597-3717): [反击] onto
//! 弦卷集团, the move settles as the agent tile; the card then swaps tiles around.
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:笑容大游行`）:
//! > 笑容大游行：
//! > （1）[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，当次移动的移动终点视为“弦卷集团”地产商；为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则将其放入弃牌堆。
//! > （2）[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上并移除其上全部奇迹水晶
//! > （3）[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "HHW:笑容大游行";

pub const SMILE_PARADE: CardDef = CardDef::new("HHW:笑容大游行", &[
    On::React(&[TriggerKind::Pass], can_react, react),
    On::Hook(&[TriggerKind::TurnEnd], turn_end),
]);

/// C# `CardSmileParade.Group` -- `H.TsurumakiAgent` = tile 弦卷集团 (#29).
fn group_tile() -> i32 {
    ctx::tile_named("弦卷集团")
}

fn can_react(player_id: i32) -> bool {
    // 规则书（1）[反击]: 「经过“弦卷集团”（#29格）时可将此卡放置在其上」
    let group = group_tile();
    if group < 0 {
        return false;
    }
    trigger::kind() == TriggerKind::Pass
        && trigger::player_id() == player_id
        && trigger::tile() == group
        && trigger::move_kind().is_some()
}

fn react(player_id: i32) {
    let group = group_tile();
    // 规则书（1）[反击]: 「可将此卡放置在其上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // Group, 3)` places the card on the 弦卷集团 tile, charged with 3 crystals.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("smile_parade_note")));
    // 规则书（1）: 「为此卡添加3个[奇迹水晶]」 -- the placement's crystal charge
    // (C# `H.PlaceFromPlay(c, ..., 3)`).
    ctx::set_crystals(player_id, 3);
    ctx::log(player_id, &Msg::new(key!("smile_parade_placed")).player_id("who", player_id).tile("tile", group));
    // 规则书（1）: 「当次移动的移动终点视为“弦卷集团”地产商」 -- the
    // `Fx.SettleInstead` hook kind is in (declare it and `trigger::set_cancelled()`
    // to replace the tile's effect); C# `CardSmileParade.SettleInstead` ->
    // `H.AgentLanding(m.Seat, Group)` is the replacement body.
    // TODO(规则书)（1）: `H.AgentLanding` (same-colour buy / build / half-rent
    // ladder at the agent tile) has no ctx counterpart, and the `m.Tags["parade"]`
    // latch the C# react writes has only a player-slot stand-in, so the settle
    // cannot be replaced yet.
}

/// 规则书（1）: 「你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则
/// 将其放入弃牌堆」 -- C# `CardSmileParade.TurnEnd` (`turn == Player && Tile ==
/// Group && Crystals > 0` -> `AddCrystals(-1)`; empty -> `H.Unplace(this,
/// "discard")`). `ctx::decay` is that body once the crystal runs out.
fn turn_end(player_id: i32) {
    if !ctx::is_placed(player_id) || trigger::player_id() != player_id {
        return;
    }
    // C# `Tile == Group` -- the decay only runs while the card still sits on
    // 弦卷集团. Tile-bound placement is the held TODO below, so the card is
    // placed on the owner's field and this gate cannot be tested yet.
    if ctx::decay(player_id, ID) == 0 {
        ctx::log(player_id, &Msg::new(key!("smile_parade_decayed")).player_id("who", player_id));
    }
}

// TODO(规则书)（2）: 「[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]
// 格子上并移除其上全部奇迹水晶」 -- needs the Fx.SettleAfter hook (C#
// `CardSmileParade.SettleAfter` -> `Move`, `H.AskYes` over the owner's tile)
// plus tile-bound placement so the card can sit on (and leave) 弦卷集团.
// TODO(规则书)（3）: 「[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，
// 任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 -- needs the
// Fx.SettleInstead tile-swap (C# `CardSmileParade.Swapped`) and unplace-to-
// discard on that settle. Placing the card on a tile (not a player's field)
// is itself missing (`H.PlaceFromPlay(c, owner, tile)`).
