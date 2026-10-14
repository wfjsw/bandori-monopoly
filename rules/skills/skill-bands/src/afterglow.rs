//! `skill:Afterglow:商店街的宠儿`
//!
//! 规则书（band sheet, Afterglow）:
//! > 你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子
//!
//! 「商店街的」 is the shop tile group; 「价值小于等于1200」 is a price filter.
//! 「自动免费在上面加盖一栋房子」 is `card_build` with the build cost waived.

use card_sdk::abi::HookKind;
use ruleset_core::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const AFTERGLOW: CardDef = CardDef::new(
    "skill:Afterglow:商店街的宠儿",
    &[On::Hook(&[HookKind::Bought], card_sdk::pre::MINE, None, on_bought)],
);

/// 「你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子」.
fn on_bought(player_id: i32) -> card_sdk::Asked {
    let t = ctx::trigger::tile();
    if t < 0 {
        return Ok(());
    }
    if !ctx::is_shop(t) && ctx::tile_price(t) > 1200 {
        return Ok(());
    }
    // 「自动免费」 -- the cost is waived, so the build goes through regardless.
    ctx::set_build_discount(ctx::build_cost(t), 1);
    if ctx::card_build(player_id, t) {
        ctx::log(
            player_id,
            &Msg::new(key!("afterglow_free_house")).tile("tile", t),
        );
    }
    Ok(())
}

/// `bands.json` row for this band skill (ABI v63).
pub const BAND: ruleset_core::roster::BandDef =
ruleset_core::roster::BandDef::new(r#"Afterglow"#)
    .short_name(r#"AG"#)
    .boot_art(r#"002"#)
    .color(r#"#EE3344"#)
    .skill(r#"商店街的宠儿"#)
    .text(r#"你购买商店街的或价值小于等于1200的格子时自动免费在上面加盖一栋房子"#)
    .logo(r#"002"#);
