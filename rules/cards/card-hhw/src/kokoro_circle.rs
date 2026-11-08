//!
//! 规则书（docs/rulebook/cards.json, id `HHW:（kkr）前往笑容集结的地方！`）:
//! > （kkr）前往笑容集结的地方！：
//! >  支付10000资金（视为买地花费）并将此卡置于CiRCLE上，若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款
//!

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{ActionKind, PayKind};
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
    // 规则书: 「支付10000资金（视为买地花费）」 -- `kind(Buy)` is the 「买地」
    // stamp for the [支付] riders; mandatory is the builder default, so a short
    // payer still goes through the shortfall path.
    let receipt = ctx::PayCommand::new(
        player_id,
        -1,
        10000,
        &Msg::new(key!("kokoro_circle_reason")),
    )
    .kind(PayKind::Buy)
    .send()?;
    if !receipt.settled() {
        // 规则书: 「视为此卡未生效」 -- a shortfall on the 10000. The card is
        // spent anyway (C-Q5(a)); the waste bit carries the clause (CH-07).
        bandori_std::zones::set_ineffective();
        return Ok(());
    }
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
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
/// -- a settle on the card's tile by anyone but the owner pays the owner 6,000 as
/// the tile's collection (`PayKind::Rent` carrying the tile). `SETTLE-STAGES.md`
/// §4 M3: an entry in the settle's effect list (行动阶段 15), so a body replace
/// skips it.
fn settle_body(player_id: i32) -> card_sdk::Asked {
    if trigger::cancelled() {
        return Ok(());
    }
    let Some(tile) = ctx::self_tile() else {
        return Ok(());
    };
    // When the card is not tile-bound (`Some(-1)`) fall back to CiRCLE, the tile the
    // play names.
    let target = if tile >= 0 {
        tile
    } else {
        ctx::tile_named("CiRCLE")
    };
    if target < 0 || trigger::tile() != target {
        return Ok(());
    }
    // The owner settling there does not pay themselves, and the ordinary tile
    // reward runs instead of the 6,000 on a settle redirected here (2.19).
    let mover = trigger::player_id();
    if mover == player_id || mover < 0 || trigger::is_settle_redirect() {
        return Ok(());
    }
    // TODO(规则书)（若其他玩家在该格[触发结算]则向所有者支付6000资金，视为格子的收款）:
    //   the book prints no carve-out for a normal CiRCLE settle (the reward that
    //   already ran), yet the engine's settle-redirect tag skips the 6,000 there.
    //   The read lives on the trigger (BACKLOG ABI-02 closed by `is_settle_redirect`);
    //   a card redirect is the only shape that skips it.
    // 规则书: 「向所有者支付6000资金，视为格子的收款」 -- `kind(Rent)` is the
    // 收款 stamp and `tile(target)` carries CiRCLE, so `pay_is_rent`, the
    // tile-keyed riders and the *owner's* per-seat settle ledger all see it
    // (`charge_rent` cannot: CiRCLE is unowned and the 6,000 is fixed).
    let receipt = ctx::PayCommand::new(
        mover,
        player_id,
        6000,
        &Msg::new(key!("kokoro_circle_rent")),
    )
    .kind(PayKind::Rent)
    .tile(target)
    .send()?;
    ctx::log(
        player_id,
        &Msg::new(key!("kokoro_circle_settled"))
            .player_id("who", mover)
            .tile("tile", target)
            .n("amount", receipt.amount() as i64),
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
