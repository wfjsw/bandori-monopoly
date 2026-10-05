//! `通用:[都筑诗船]Parking Space` -- C# `CardParkingSpace` (MatchHost.cs:2430-2501):
//! place on Space, hand its houses to your other tiles, settle becomes a
//! turn-end [停留].
//!
//! 规则书（docs/rulebook/cards.json, id `通用:[都筑诗船]Parking Space`）:
//! > [都筑诗船]Parking Space：
//! > [手]：
//! > 将此卡放置于“Space”格子上，将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）。
//! > [持续]：
//! >
//! > （1）此卡所在格子的[结算]改为回合结束后获得一层[停留]。
//! >
//! > （2）位于此卡所在格子上的玩家无法使用角色及乐队技能。
//!

use card_sdk::abi::{TriggerKind, HookKind};
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "通用:[都筑诗船]Parking Space";

/// Where the deferred [停留] layers are written down (C# `H.IncV(who, "parkingStay")`).
const SLOT_STAY: &str = "parking_space_stay";

pub const PARKING_SPACE: CardDef = CardDef::new("通用:[都筑诗船]Parking Space", &[
    On::Play(play),
    On::CantPlay(cant_play),
    // C# `CardParkingSpace.SettleInstead` / `TurnEndAfter` -- field hooks, not [反击].
    On::Hook(&[HookKind::SettleInstead, HookKind::TurnEndAfter], react),
]);

/// C# `CardParkingSpace.WhyNot`: refuses when the board has no "Space" tile
/// (`没有 Space`).
fn cant_play(_player: i32) -> Option<Msg> {
    if ctx::tile_named("Space") >= 0 {
        return None;
    }
    Some(Msg::new(key!("parking_space_no_space")))
}

fn play(player_id: i32) {
    // 规则书[手]: 「将此卡放置于“Space”格子上」 -- C# `H.PlaceFromPlay(c, i, s)`
    // with `s = H.TileNamed("Space")`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("parking_space_note")));
    let space = ctx::tile_named("Space");
    // TODO(规则书)[手]: 「将此卡放置于“Space”格子上」 -- the placement is bound to
    // the player's field, not the Space tile; needs field-card tile placement
    // (`H.PlaceFromPlay(c, owner, tile)`), the same gap as `kokoro_circle`.
    // The `SettleInstead` body below therefore keys off the Space tile id
    // rather than this card's `Tile`.
    // TODO(规则书)（2）[持续]: 「位于此卡所在格子上的玩家无法使用角色及乐队技能」
    // -- needs a skill-suppression hook (C# `CardParkingSpace.NoteText` /
    // `Fx` gate on skill use at this tile).
    // 规则书[手]: 「将其上的房屋转移至其他你拥有的格子上（每个格子因此效果最多获得1层）」
    // -- C# takes `H.State.houses[s]` houses off the Space tile (only when the
    // player owns it) and hands one to each of the player's other buildable deeds
    // (`H.State.houses[s]--` then `H.AddHouse(i, item, ...)`, at most one per
    // destination).
    let mut num = if space >= 0 && ctx::tile_owner(space) == player_id {
        ctx::houses_of(space)
    } else {
        0
    };
    for t in ctx::owned_tiles(player_id) {
        if num <= 0 {
            break;
        }
        if t == space {
            continue;
        }
        // TODO(规则书)[手]: the C# destination filter is `H.WhyNotBuildOn(i, t) == null`
        // (kind != "ring", rent capacity, house cap, `H._turnCtx.NoBuild`, the
        // "卡池BUG" event, `Fx.CanBuild`) -- only buyable / not-mortgaged /
        // can-pay are expressible here, and `ctx::add_house` does not clamp at
        // the C# house cap either. Same gap as `hey_kids`.
        if !ctx::is_buyable(t) || ctx::mortgaged_of(t) || !ctx::can_pay(player_id) {
            continue;
        }
        ctx::add_house(space, -1);
        ctx::add_house(t, 1);
        num -= 1;
    }
    // 规则书（1）[持续] runs in `react` at `settleInstead` / `turnEndAfter`.
}

/// C# `CardParkingSpace.SettleInstead` / `TurnEndAfter` (MatchHost.cs:2469-2495).
/// Runs through the Fx hook dispatch, so these are field effects, not [反击].
fn react(player_id: i32) {
    if !ctx::is_placed(player_id) {
        return;
    }
    match trigger::kind() {
        // 规则书（1）[持续]: 「此卡所在格子的[结算]改为回合结束后获得一层[停留]」 -- C#
        // `CardParkingSpace.SettleInstead`: when the settle lands on the card's
        // tile, replace the tile's effect (`H.IncV(who, "parkingStay")`) and
        // cancel the settle body. First card to claim the settle wins, so bail
        // out once another has (`trigger::cancelled()`).
        TriggerKind::SettleInstead => {
            if trigger::cancelled() {
                return;
            }
            let space = ctx::tile_named("Space");
            if space < 0 || trigger::tile() != space {
                return;
            }
            let who = trigger::player_id();
            // C# `((m.SettleTile >= 0) ? m.SettleTile : m.Seat.pos) != Tile` --
            // the host raises the trigger with the landing tile; the plan's
            // `set_settle_tile` override is not read back yet (engine-side).
            // C# `H.Log("text", who, ... 在「Parking Space」所在的格子结算：回合结束后获得 1 层 [停留])`.
            ctx::log(who, &Msg::new(key!("parking_space_park")).player_id("who", who));
            ctx::inc_slot(who, SLOT_STAY, 1);
            // C# returns a non-null enumerator, which the engine takes as
            // "the tile's own effect is replaced"; the ABI signals that by
            // cancelling the settle body.
            trigger::set_cancelled();
        }
        // 规则书（1）[持续]: the deferred layers land at the end of the mover's
        // own turn -- C# `CardParkingSpace.TurnEndAfter` (`H.V(turn, "parkingStay")`
        // -> `H.GiveStay(turn, layers, Seat, "Parking Space")`). Any player's turn
        // end counts: the counter sits on the mover, not on the card's owner.
        TriggerKind::TurnEndAfter => {
            let turn = trigger::player_id();
            let layers = ctx::slot(turn, SLOT_STAY);
            if layers <= 0 {
                return;
            }
            ctx::set_slot(turn, SLOT_STAY, 0);
            // C# `H.GiveStay(turn, layers, Seat, "Parking Space")` -> `AddStay`
            // logs 「… 获得 N 层 [停留]（Parking Space）」.
            ctx::give_stay(turn, layers);
            ctx::log(
                turn,
                &Msg::new(key!("parking_space_stay")).player_id("who", turn).i("n", layers as i64),
            );
        }
        _ => {}
    }
}