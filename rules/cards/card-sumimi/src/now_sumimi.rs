//! `Sumimi:现在她是Sumimi的小初啦` -- C# `CardNowSumimi` (MatchHost.cs:11220-11248):
//! [反击] on a pay, gain the tile's price tag.
//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:现在她是Sumimi的小初啦`）:
//! > 现在她是Sumimi的小初啦：
//! > [反击] 当你在你的本回合开始后到下回合开始前之间失去资金的总额即将超过你所在格子的[收费标价]时打出此卡，获得相当于你所在格子[收费标价]数额的资金（RiNG则为其基础乘数）
//!

use card_sdk::abi::TriggerKind;
use card_sdk::ctx::{self, trigger};
use card_sdk::{key, CardDef, Msg};

pub const NOW_SUMIMI: CardDef = CardDef {
    id: "Sumimi:现在她是Sumimi的小初啦",
    play: None,
    can_react: Some(can_react),
    react: Some(react),
    why_not: None,
};

/// C# `TileData.kind == "ring"` -- the ABI has no `tile_kind`, but the board's
/// tile-kind surface is `is_buyable` / `is_shop` / `tile_group`, and the RiNG
/// deeds are exactly the buyable tiles whose rent table is empty (`rent_of`
/// falls back to the land price). They never hold houses (`H.WhyNotBuildOn`
/// rejects `kind == "ring"`).
fn is_ring(t: i32) -> bool {
    ctx::is_buyable(t)
        && ctx::tile_price(t) > 0
        && ctx::houses_of(t) == 0
        && ctx::rent_of(t) == ctx::tile_price(t)
}

/// C# `CardNowSumimi.Tag` = `H.PriceTag(H.State.seats[seat].pos)`.
fn price_tag(seat: i32) -> i32 {
    let t = ctx::seat_pos(seat);
    if t < 0 || !ctx::is_buyable(t) {
        return 0;
    }
    // 规则书: 「（RiNG则为其基础乘数）」 -- C# `H.PriceTag` returns
    // `H.RingMultiplier` for `kind == "ring"` tiles and the rent table otherwise.
    if is_ring(t) {
        return ctx::ring_multiplier();
    }
    // `rent_of` is the rent table at the current house count (0 for empty tables).
    ctx::rent_of(t)
}

fn can_react(seat: i32) -> bool {
    // 规则书[反击]: 「当…失去资金的总额即将超过你所在格子的[收费标价]时打出此卡」
    // C# `t.Kind == "pay" && t.Pay.from == seat && t.Pay.amount > 0 && !t.Pay.cancel`
    if trigger::kind() != TriggerKind::Pay {
        return false;
    }
    if trigger::seat() != seat || trigger::value() <= 0 {
        return false;
    }
    let tag = price_tag(seat);
    if tag <= 0 {
        return false;
    }
    // TODO(规则书)[反击]: 「失去资金的总额即将超过」 -- C# compares
    // `H._lostSinceTurn[seat]` (money lost since this seat's turn started) plus
    // `t.Pay.amount` against `Tag`, firing when `lost <= Tag && lost + amount >
    // Tag`. The ABI has no lost-since-turn counter (needs a Pay/Paid Fx hook to
    // feed a slot), so the guard here is the conservative subset `amount > Tag`
    // (this payment alone would exceed the tag).
    // TODO(ABI): `t.Pay.cancel` is not on the wire trigger.
    trigger::value() > tag
}

fn react(seat: i32) {
    // 规则书[反击]: 「获得相当于你所在格子[收费标价]数额的资金」 -- C# `H.GainR(c.Seat, Tag(c.Seat), CardName)`.
    let tag = price_tag(seat);
    if tag > 0 {
        ctx::gain(seat, tag, &Msg::new(key!("now_sumimi_why")).n("n", tag as i64));
    }
}