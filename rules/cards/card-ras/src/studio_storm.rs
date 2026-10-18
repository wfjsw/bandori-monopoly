//! `RAS:练习室里的风暴` -- C# `CardStudioStorm` (MatchHost.cs:9637-9709):
//!
//! 规则书（docs/rulebook/cards.json, id `RAS:练习室里的风暴`）:
//! > 练习室里的风暴：
//!
//! > （1）当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上，每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）
//!
//! > （2）[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动
//!
//! placed on your Live House; CiRCLE passes charge it, a nearby settle spends it.

use card_sdk::abi::ActionKind;
use ruleset_core::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg, On};

pub const STUDIO_STORM: CardDef = CardDef::new(
    "RAS:练习室里的风暴",
    &[
        On::Play("", Some(cant_play), play),
        On::Action(&[ActionKind::PassTile], "actor == owner && card.placed && tile_is(tile.id, \"circle\")", None, pass_tile),
        // （2）「在距此卡所在格子X个格子处[结算]时」 is 行动阶段 15
        // (`SETTLE-STAGES.md` §4 M2) -- an entry in the settle's effect list.
        // The body is a one-shot 「收费」 (sheet 2026-10-06), not a re-settle;
        // `settleBody` is still the raise point for 「[结算]时」.
        On::Action(&[ActionKind::SettleBody], "actor != owner && card.placed", None, settle_body),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"练习室里的风暴"#)
        .raw_name(r#"练习室里的风暴"#)
        .tags(&[])
        .text(r#"（1）当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上，每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）
（2）[使用者]以外的玩家在距此卡所在格子X个格子处[结算]时将此卡放入弃牌堆且对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍价格的收费，此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动"#)
        .cell(r#"H8"#)
        .fields(&[("group", r#"RAISE A SUILEN"#), ("owner", "")]));

const ID: &str = "RAS:练习室里的风暴";

/// C# `H.IsLiveHouse` (color group 6) tiles that can hold houses.
const LIVEHOUSE_PROPS: [&str; 4] = [
    "DUB MUSIC EXPERIMENT",
    "武道馆",
    "Space",
    "Live House Galaxy",
];

/// The tile the card sits on (C# `Mem` on a tile-bound field card).

fn on_own_lh(player_id: i32) -> bool {
    let pos = ctx::player_pos(player_id);
    pos >= 0
        && ctx::tile_owner(pos) == player_id
        && LIVEHOUSE_PROPS.iter().any(|&n| ctx::tile_named(n) == pos)
}

/// C# `CardStudioStorm.WhyNot`: 「只有站在自己的 Live House 格子上才能打出」
fn cant_play(player_id: i32) -> Option<Msg> {
    if !on_own_lh(player_id) {
        return Some(Msg::new(key!("studio_storm_not_on_lh")));
    }
    None
}

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上」
    // -- C# `WhyNot` requires `owners[pos] == player_id && H.IsLiveHouse(player_id, pos)`;
    // the gate is now `cant_play` above.
    let pos = ctx::player_pos(player_id);
    if !on_own_lh(player_id) {
        return Ok(());
    }
    // 规则书（1）: 「将此卡放置在当前格子上」 -- C# `H.PlaceFromPlay(c, seat, pos)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card_on(player_id, pos, ID, &Msg::new(key!("studio_storm_note")));
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_placed"))
            .player_id("who", player_id)
            .tile("tile", pos),
    );
    // 规则书（1）: 「每次[使用者]经过CiRCLE时为此卡放置一个[奇迹水晶]（初始0，上限3）」
    // -- crystals start at 0 (the default) and cap at 3 via `add_crystals(.., 3)`.
    Ok(())
}

/// C# `CardStudioStorm.PassTile` -- the user passing a CiRCLE tile adds a crystal.
/// Pure guard for [`pass_tile`] -- the activation gate. `false`
/// means the card is not activated at all.
fn pass_tile(player_id: i32) -> card_sdk::Asked {
    // C# `m.Seat != User || H.Tile(t)?.kind != "circle"`.
    let t = trigger::tile();
    ctx::add_crystals(1, 3)?;
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_crystal"))
            .player_id("who", player_id)
            .tile("tile", t),
    );
    Ok(())
}

/// C# `CardStudioStorm.SettleAfter` -- a non-user settle within `Crystals` ring
/// distance of the card's tile triggers the storm.
/// 规则书（2）: 「…[结算]时将此卡放入弃牌堆且对那个玩家进行一次…收费」 --
/// 行动阶段 15 (`SETTLE-STAGES.md` §4 M2), so a body replace skips it.
fn settle_body(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    let Some(tile) = ctx::self_tile() else {
        return Ok(());
    };
    if tile < 0 {
        return Ok(());
    }
    let a = trigger::tile();
    let a = if a >= 0 {
        a
    } else {
        ctx::player_pos(trigger::player_id())
    };
    // 规则书（2）: 「此效果只有在X至少为1且小等于此卡[奇迹水晶]数量时可发动」
    let x = ctx::dist(a, tile);
    let crystals = ctx::crystals();
    if x < 1 || x > crystals {
        return Ok(());
    }
    // 规则书（2）: 「将此卡放入弃牌堆」 -- C# `Storm`: unplace to discard. The text
    // order puts the discard before the 收费, so the card is gone first either way.
    ctx::send_to_dest(ctx::Dest::Graveyard);
    ctx::log(
        player_id,
        &Msg::new(key!("studio_storm_fired"))
            .player_id("who", trigger::player_id())
            .tile("tile", tile)
            .i("x", x as i64),
    );
    // 规则书（2）: 「对那个玩家进行一次相当于此卡所在格子普通[结算]的(4-X)/4倍
    // 价格的收费」 -- sheet 2026-10-06 新卡组卡 H8 rewrote this from the older
    // 「那个玩家进行一次此卡所在格子的[结算]」 (a forced re-settle) into a direct
    // charge. A one-shot 「收费」 to the tile owner, **not** a `card_settle_at`:
    // a full settle also fires settle hooks / counteractions and offers
    // build/buy, none of which the printed wording asks for.
    //
    // 「普通[结算]的价格」 is `rent_of` (the table rent at the tile's current
    // house count); the scale is integer (4-X)/4, the same figure the old
    // `set_rent_factor(250 * (4-X))` produced (`3340 * 2 / 4 = 1670` in
    // `ras.rs` `storm_part2_rent_factor`). `scale_settle_payment` truncates
    // (`spec/ROUNDING.md`), so the multiply-then-divide keeps that.
    //
    // TODO(规则书)[ruling]: when the tile is **mortgaged**, 「普通[结算]」 is a
    // forced-buy offer, not a rent, so there is no obvious 收费 recipient.
    // Class C -- charge nothing for now rather than guess.
    if !ctx::mortgaged_of(tile) {
        let owner = ctx::tile_owner(tile);
        let amount = ctx::rent_of(tile) * (4 - x) / 4;
        if owner >= 0 && amount > 0 {
            ctx::transfer(
                trigger::player_id(),
                owner,
                amount,
                &Msg::new(key!("studio_storm_charge"))
                    .player_id("to", owner)
                    .tile("tile", tile)
                    .i("amount", amount as i64),
            )?;
        }
    }
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn pass_tile_kani(player_id: i32) -> card_sdk::Asked {
    pass_tile(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_body_kani(player_id: i32) -> card_sdk::Asked {
    settle_body(player_id)
}
