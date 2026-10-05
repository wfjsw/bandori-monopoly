//! `HHW:（kkr）前往笑容集结的地方！` -- C# `CardKokoroCircle` (MatchHost.cs:3900-3964).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（kkr）前往笑容集结的地方！`）:
//! > （kkr）前往笑容集结的地方！：
//! >  支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款
//!

use card_sdk::abi::HookKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, On, Msg};

const ID: &str = "HHW:（kkr）前往笑容集结的地方！";

pub const KOKORO_CIRCLE: CardDef = CardDef::new("HHW:（kkr）前往笑容集结的地方！", &[
    On::Play(Some(cant_play), play),
    On::Hook(&[HookKind::SettleAfter], |_| true, settle_after)]);

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
// TODO(规则书)[judgement]: 「视为此卡未生效」 -- the clause names a state without
        // saying what observes it. `PlayCtx.Effective = false` is the C#'s mutable
        // side channel and is not being ported (a routine should *return* whether
        // it took effect); but before that lands, what "not effective" changes has
        // to be ruled: does the card get spent (haneoka 「放入弃牌堆且视为此卡未生效」
        // says yes) or not (noble_blue / starry_night's "the card is spent anyway"
        // implies no)? And what counts a use that this would suppress?
        return;
    }
    // 规则书: 「并将此卡置于CiRCLE上」 -- C# `H.PlaceFromPlay(c, i, 0)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::log(player_id, &Msg::new(key!("kokoro_circle_placed")).player_id("who", player_id));
    // 规则书: 「并将此卡置于CiRCLE上」 -- bound to the CiRCLE tile, not to the
    // player's field.
    let circle = ctx::tile_named("CiRCLE");
    ctx::place_card_on(player_id, circle, ID, &Msg::new(key!("kokoro_circle_note")));
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
    // TODO(规则书)[judgement]: C# also skips when `H.CircleNormal(m, num)` (the move tag
    //   the clause under-specifies -- see the note above it
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
