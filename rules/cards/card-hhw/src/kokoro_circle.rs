//! `HHW:（kkr）前往笑容集结的地方！` -- C# `CardKokoroCircle` (MatchHost.cs:3900-3964).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（kkr）前往笑容集结的地方！`）:
//! > （kkr）前往笑容集结的地方！：
//! >  支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "HHW:（kkr）前往笑容集结的地方！";

pub const KOKORO_CIRCLE: CardDef = CardDef::new("HHW:（kkr）前往笑容集结的地方！", &[
    On::Play(play),
    On::CantPlay(cant_play),
    On::Hook(&[TriggerKind::SettleAfter], settle_after),
]);

fn cant_play(player_id: i32) -> Option<Msg> {
    // C# `CardKokoroCircle.WhyNot`: refuses without 10,000 money, or when a
    // `CardKokoroCircle` is already placed on CiRCLE.
    if ctx::money(player_id) < 10000 {
        return Some(Msg::new(key!("x_no_money_10000")));
    }
    // 规则书: 「并将此卡置于CiRCLE上」 -- C# `H._placed.Any(p => p is CardKokoroCircle)`
    // (`ctx::placed_tile` answers "is this id in play on that player").
    let n = ctx::player_count();
    for s in 0..n {
        if ctx::placed_tile(s, ID).is_some() {
            return Some(Msg::new(key!("kokoro_circle_already")));
        }
    }
    None
}

fn play(player_id: i32) {
    // 规则书: 「支付10000资金（视为买地花费）」 -- C# `PayCtx { amount = 10000, kind = "buy" }`.
    let paid = ctx::pay(player_id, 10000, &Msg::new(key!("kokoro_circle_why")));
    if paid < 10000 {
        // TODO(规则书): the C# sets `c.Effective = false` when the payment does not
        // go through (the play is wasted, `H.ToDiscard(..., wasted: true)`); the ABI
        // has no set_effective hook.
        return;
    }
    // 规则书: 「并将此卡置于CiRCLE上」 -- C# `H.PlaceFromPlay(c, i, 0)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(player_id, ID, &Msg::new(key!("kokoro_circle_note")));
    ctx::log(player_id, &Msg::new(key!("kokoro_circle_placed")).player_id("who", player_id));
    // TODO(规则书): 「并将此卡置于CiRCLE上」 -- the placement is bound to tile #0
    // (CiRCLE) rather than the player's field; `place_card` does not take a tile
    // (`f.tile` stays -1), so `placed_tile` reports `Some(-1)`. The settle hook
    // below falls back to `tile_named("CiRCLE")` for the same reason.
}

/// 规则书: 「若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款」
/// -- C# `CardKokoroCircle.SettleAfter`: a settle on the card's tile by anyone
/// but the owner pays the owner 6,000 as if it were the tile's rent (`kind =
/// "rent"`).
fn settle_after(player_id: i32) {
    // C# `H._placed.Contains(this)` / `Tile` -- this card must be the one in
    // play (`placed_tile` asks about *this* id; `is_placed` only asks about
    // "any card").
    let Some(tile) = ctx::placed_tile(player_id, ID) else {
        return;
    };
    // C# `num != Tile` -- the settle is on the card's tile. When the card is
    // not tile-bound (`Some(-1)`) fall back to CiRCLE, the tile the play names.
    let target = if tile >= 0 { tile } else { ctx::tile_named("CiRCLE") };
    if target < 0 || trigger::tile() != target {
        return;
    }
    // C# `m.Seat == Seat` -- the owner settling there does not pay themselves.
    let mover = trigger::player_id();
    if mover == player_id || mover < 0 {
        return;
    }
    // TODO(规则书): C# also skips when `H.CircleNormal(m, num)` (the move tag
    // `circleNormal` on a real CiRCLE settle -- the normal CiRCLE reward already
    // ran). That move-tag read is not on the trigger payload, so the 6,000 is
    // charged on every CiRCLE settle by another player until it lands.
    // 规则书: 「向所有者支付6000资金，视为格子的收款」
    ctx::transfer(mover, player_id, 6000, &Msg::new(key!("kokoro_circle_rent")));
    ctx::log(
        player_id,
        &Msg::new(key!("kokoro_circle_settled")).player_id("who", mover).tile("tile", target),
    );
}
