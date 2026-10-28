//! `HHW:（kkr）前往笑容集结的地方！` -- C# `CardKokoroCircle` (MatchHost.cs:3900-3964).
//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（kkr）前往笑容集结的地方！`）:
//! > （kkr）前往笑容集结的地方！：
//! >  支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款
//!
//! The sheet prints no one-copy limit (C# `WhyNot` refused a second copy):
//! stacked copies each keep their own settle body and each charge the 6000.
//!

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ActionKind;
use card_sdk::CardText;
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "HHW:（kkr）前往笑容集结的地方！";

pub const KOKORO_CIRCLE: CardDef = CardDef::new(
    "HHW:（kkr）前往笑容集结的地方！",
    &[
        On::Play("", None, play),
        // 「若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款」
        // -- 行动阶段 15 (`SETTLE-STAGES.md` §4 M3): 「视为格子的收款」 is an
        // entry in the tile's settle effect list, not an after-hook. A field
        // card that replaces the body skips this entry. `docs/TILES.md` names
        // the long-term home as a collect rule instance on CiRCLE; the
        // `settleBody` hook is the same list entry for now.
        On::Action(Action::SettleBody("", None, settle_body)),
    ],
).props(&[(card_sdk::abi::prop::EST_COST, 10000)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"前往笑容集结的地方！"#)
        .raw_name(r#"（kkr）前往笑容集结的地方！"#)
        .tags(&[])
        .text(CardText::para(r#"支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款"#))
        // sheet cell F7
        .fields(&[("group", r#"Hello, Happy World!"#), ("owner", r#"弦卷心"#)]));

fn play(player_id: i32) -> card_sdk::Asked {
    // 规则书: 「支付10000资金（视为买地花费）」 -- C# `PayCtx { amount = 10000, kind = "buy" }`.
    // TODO(规则书)（视为买地花费）: the book treats the 10000 as a land purchase --
    //   we pay a plain card payment (`PayKind::Card`), so `pay_factor` reducers
    //   and 「买地」 riders do not see it; `ctx::pay_buy` would carry the kind but
    //   drops the mandatory shortfall/bankrupt path.
    let paid = ctx::pay(player_id, 10000, &Msg::new(key!("kokoro_circle_why")))?;
    if paid < 10000 {
        // TODO(规则书)（支付10000资金（视为买地花费））: the book does not say what
        // happens when the 10000 cannot be paid -- we keep the card spent and
        // place nothing. (The foreign phrasing 「视为此卡未生效」 is the C#
        // `PlayCtx.Effective = false` side channel; BACKLOG CH-07 owns it.)
        return Ok(());
    }
    // 规则书: 「并将此卡置于CiRCLE上」 -- C# `H.PlaceFromPlay(c, i, 0)`.
    ctx::set_dest(ctx::Dest::Field);
    ctx::log(
        player_id,
        &Msg::new(key!("kokoro_circle_placed")).player_id("who", player_id),
    );
    // 规则书: 「并将此卡置于CiRCLE上」 -- bound to the CiRCLE tile, not to the
    // player's field.
    let circle = ctx::tile_named("CiRCLE");
    ctx::place_card_on(player_id, circle, ID, &Msg::new(key!("kokoro_circle_note")));
    Ok(())
}

/// 规则书: 「若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款」
/// -- C# `CardKokoroCircle.SettleAfter`: a settle on the card's tile by anyone
/// but the owner pays the owner 6,000 as the tile's collection (rent). The
/// payment currently runs as a plain card payment, not `PayKind::Rent`, and
/// carries no tile (see the TODOs at the transfer). `SETTLE-STAGES.md` §4 M3:
/// an entry in the settle's effect list (行动阶段 15), so a body replace skips
/// it.
fn settle_body(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    // C# `H._placed.Contains(this)` / `Tile` -- this card must be the one in
    // play (`placed_tile` asks about *this* id; `is_placed` only asks about
    // "any card").
    let Some(tile) = ctx::self_tile() else {
        return Ok(());
    };
    // C# `num != Tile` -- the settle is on the card's tile. When the card is
    // not tile-bound (`Some(-1)`) fall back to CiRCLE, the tile the play names.
    let target = if tile >= 0 {
        tile
    } else {
        ctx::tile_named("CiRCLE")
    };
    if target < 0 || trigger::tile() != target {
        return Ok(());
    }
    // C# `m.Seat == Seat` -- the owner settling there does not pay themselves.
    let mover = trigger::player_id();
    if mover == player_id || mover < 0 {
        return Ok(());
    }
    // TODO(规则书)（若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款）:
    //   the book prints no carve-out for a normal CiRCLE settle (the reward that
    //   already ran), yet C# `H.CircleNormal(m, num)` skips the 6,000 there. That
    //   move-tag read is not on the trigger payload (BACKLOG ABI-02), so the
    //   6,000 is charged on every such settle by another player until it lands.
    // 规则书: 「向所有者支付6000资金，视为格子的收款」
    // TODO(ABI)（视为格子的收款）: the book makes this the tile's collection -- we
    //   pay a plain card payment with no tile, so `pay_is_rent` is false (the
    //   band skill and rent riders miss it), tile-keyed riders dead-end on
    //   tile -1 and `paid_in_settle` stays cold. Needs a fixed-amount
    //   `PayKind::Rent` payment carrying CiRCLE (`charge_rent` computes the
    //   tile's own rent and CiRCLE is unowned).
    let paid = ctx::transfer(
        mover,
        player_id,
        6000,
        &Msg::new(key!("kokoro_circle_rent")),
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("kokoro_circle_settled"))
            .player_id("who", mover)
            .tile("tile", target)
            .n("amount", paid as i64),
    );
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn play_kani(player_id: i32) -> card_sdk::Asked {
    play(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_body_kani(player_id: i32) -> card_sdk::Asked {
    settle_body(player_id)
}
