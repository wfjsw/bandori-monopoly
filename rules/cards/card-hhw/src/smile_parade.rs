//! `HHW:笑容大游行` -- C# `CardSmileParade` (MatchHost.cs:3597-3717): [反击] onto
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:笑容大游行`）:
//! > 笑容大游行：
//! > （1）[反击]经过“弦卷集团”（#29格）时可将此卡放置在其上，当次移动的移动终点视为“弦卷集团”地产商；为此卡添加3个[奇迹水晶]，你的每回合结束时移除一个，奇迹水晶为0时若此卡仍位于“弦卷集团”则将其放入弃牌堆。
//! > （2）[持续] [触发结算]后可将“弦卷集团”格子上的此卡放置于[移动终点]格子上并移除其上全部奇迹水晶
//! > （3）[持续] 当此卡位于格子上时，那格视为与“弦卷集团”格子交换位置，任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆。
//!
//! 弦卷集团, the move settles as the agent tile; the card then swaps tiles around.

use card_sdk::abi::{ChainKind, HookKind, TriggerKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "HHW:笑容大游行";

pub const SMILE_PARADE: CardDef = CardDef::new(
    "HHW:笑容大游行",
    &[
        On::Hook(&[card_sdk::abi::HookKind::SettleBody], card_sdk::pre::MINE, None, settle_instead),
        On::Hook(&[card_sdk::abi::HookKind::SettleAfter], "actor == owner && card.placed", None, move_after),
        On::Counteract(&[ChainKind::Pass], "", Some(can_counteract), counteract),
        On::Hook(&[HookKind::TurnEnd], "actor == owner && card.placed", None, turn_end),
        On::Hook(&[HookKind::CrystalsChanged], "actor == owner && card.placed && card.cp == 0 && value <= 0", Some(crystals_changed_guard), on_crystals_changed),
    ],
)
    .legacy(&[(0, legacy_mine), (1, legacy_mine)]);

/// C# `CardSmileParade.Group` -- `H.TsurumakiAgent` = tile 弦卷集团 (#29).
fn group_tile() -> i32 {
    ctx::tile_named("弦卷集团")
}

fn can_counteract(player_id: i32) -> bool {
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

fn counteract(player_id: i32) -> card_sdk::Asked {
    let group = group_tile();
    // 规则书（1）[反击]: 「可将此卡放置在其上」 -- C# `H.PlaceFromPlay(c, c.Seat,
    // Group, 3)` places the card on the 弦卷集团 tile, charged with 3 crystals.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("smile_parade_note")));
    // 规则书（1）: 「为此卡添加3个[奇迹水晶]」 -- the placement's crystal charge
    // (C# `H.PlaceFromPlay(c, ..., 3)`).
    ctx::set_crystals(3);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_placed"))
            .player_id("who", player_id)
            .tile("tile", group),
    );
    // 规则书（1）: 「当次移动的移动终点视为“弦卷集团”地产商」 -- the
    // `Fx.SettleBody` hook kind is in (declare it and `trigger::set_cancelled()`
    // to replace the tile's effect); C# `CardSmileParade.SettleBody` ->
    // `H.AgentLanding(m.Seat, Group)` is the replacement body.
    // （1）「当次移动的移动终点视为"弦卷集团"地产商」 -- `settle_instead`
    // below is the replacement: `plan::set_settle_as_agent` is
    // `H.AgentLanding`'s shape.
    Ok(())
}

/// 规则书（1）: 「你的每回合结束时移除一个」 -- C# `CardSmileParade.TurnEnd`
/// (`turn == Player && Tile == Group && Crystals > 0` -> `AddCrystals(-1)`).
/// The 「奇迹水晶为0时若此卡仍位于"弦卷集团"则将其放入弃牌堆」 half is
/// [`on_crystals_changed`].
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    // C# `Tile == Group` -- the decay only runs while the card still sits on
    // 弦卷集团.
    if !on_group() {
        return Ok(());
    }
    ctx::decay()?;
    Ok(())
}

/// Is the card still sitting on 「弦卷集团」?
fn on_group() -> bool {
    let group = ctx::tile_named("弦卷集团");
    match ctx::self_tile() {
        Some(t) => group >= 0 && t == group,
        None => false,
    }
}

/// 规则书（1）: 「奇迹水晶为0时若此卡仍位于"弦卷集团"则将其放入弃牌堆」 -- C#
/// `H.Unplace(this, "discard")`.
///
/// 「若此卡仍位于"弦卷集团"」 is a real condition, not a restatement of the
/// tick's own gate: （2） moves the card to the [移动终点] and strips its
/// crystals, and that emptying is *not* this discard.
/// Residual guard for [`on_crystals_changed`] -- `card_is` and the group
/// check stay here (not yet in the condition vocabulary).
fn crystals_changed_guard(_player_id: i32) -> bool {
    trigger::card_is(ID) && on_group()
}

fn on_crystals_changed(player_id: i32) -> card_sdk::Asked {
    ctx::set_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_decayed")).player_id("who", player_id),
    );
    Ok(())
}

/// （1）「当次移动的移动终点视为"弦卷集团"地产商」 -- `H.AgentLanding(m.Seat, Group)`.
fn settle_instead(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    if group < 0 {
        return Ok(());
    }
    ctx::trigger::set_cancelled();
    ctx::plan::set_settle_as_agent(true);
    ctx::card_settle_at(player_id, group, true);
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_agent")).tile("tile", group),
    );
    Ok(())
}

/// （2）「[触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上并移除其上
/// 全部奇迹水晶」.
fn move_after(player_id: i32) -> card_sdk::Asked {
    let group = ctx::tile_named("弦卷集团");
    let here = ctx::self_tile().unwrap_or(-1);
    if group < 0 || here != group {
        return Ok(());
    }
    let to = ctx::trigger::tile();
    if to < 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("smile_parade_title")),
        &Msg::new(key!("smile_parade_move")).tile("tile", to),
    )? {
        return Ok(());
    }
    ctx::set_self_tile(to);
    let n = ctx::crystals();
    if n > 0 {
        ctx::add_crystals(-n, i32::MAX)?;
    }
    ctx::log(
        player_id,
        &Msg::new(key!("smile_parade_moved")).tile("tile", to),
    );
    Ok(())
}

// （2）「[持续] [触发结算]后可将"弦卷集团"格子上的此卡放置于[移动终点]格子上
// 并移除其上全部奇迹水晶」 -- a `settleAfter` press; `set_card_tile` is the
// move, and `place_card_on` is what put it on 弦卷集团 in the first place.
// TODO(规则书)（3）: 「[持续] 当此卡位于格子上时，那格视为与"弦卷集团"格子交换位置，
//   任何玩家在此卡放置的格子上[触发结算]后此卡放入弃牌堆」 -- the swap is a board
//   *position* exchange (「交换位置」), not a colour or an effect copy: settling on
//   the card's tile has to resolve as if the mover landed on 弦卷集团, with the two
//   tiles' rents and ownership reads exchanged for that settle. A colour
//   override and `SettleBody` are the wrong axes -- one re-colours, the other replaces
//   one landing with another. The discard half (`unplace` + `to_discard` on that
//   settle) is expressible and waits on the swap.

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}
