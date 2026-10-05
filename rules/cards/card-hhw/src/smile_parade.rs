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
use card_sdk::{key, CardDef, Msg};

pub const SMILE_PARADE: CardDef = CardDef {
    id: "HHW:笑容大游行",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `CardSmileParade.Group` -- `H.TsurumakiAgent` = tile 弦卷集团 (#29).
fn group_tile() -> i32 {
    ctx::tile_named("弦卷集团")
}

fn can_react(seat: i32) -> bool {
    // 规则书（1）[反击]: 「经过“弦卷集团”（#29格）时可将此卡放置在其上」
    let group = group_tile();
    if group < 0 {
        return false;
    }
    // C# `CanReact` also requires `t.Move != null` (the pass came from a move);
    // the trigger payload carries no move context beyond the roll.
    // TODO(ABI): `t.Move` presence on a Pass trigger.
    trigger::kind() == TriggerKind::Pass && trigger::seat() == seat && trigger::tile() == group
}

fn react(seat: i32) {
    let group = group_tile();
    // 规则书（1）[反击]: 「可将此卡放置在其上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // Group, 3)` places the card on the 弦卷集团 tile, charged with 3 crystals.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "HHW:笑容大游行", &Msg::new(key!("smile_parade_note")));
    ctx::log(seat, &Msg::new(key!("smile_parade_placed")).seat("who", seat).tile("tile", group));
    // TODO(规则书)（1）: 「当次移动的移动终点视为“弦卷集团”地产商」 -- needs
    // `t.Move.Tags["parade"]` and the Fx.SettleInstead hook (C#
    // `CardSmileParade.SettleInstead` -> `H.AgentLanding(m.Seat, Group)`).
    // TODO(规则书)（1）: 「为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时
    // 若此卡仍位于“弦卷集团”则将其放入弃牌堆」 -- needs per-card crystal counters
    // on a placed card (C# `Card.Crystals` / `H.PlaceFromPlay(c, ..., 3)`) plus the
    // Fx.TurnEnd hook (C# `CardSmileParade.TurnEnd`). Never faked with marks.
    // TODO(规则书)（2）: 「[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]
    // 格子上并移除其上全部奇迹水晶」 -- needs the Fx.SettleAfter hook (C#
    // `CardSmileParade.SettleAfter` -> `Move`, `H.AskYes` over the owner's tile).
    // TODO(规则书)（3）: 「[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，
    // 任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 -- needs the
    // Fx.SettleInstead tile-swap (C# `CardSmileParade.Swapped`) and unplace-to-
    // discard on that settle. Placing the card on a tile (not a seat's field)
    // is itself missing (`H.PlaceFromPlay(c, owner, tile)`).
}