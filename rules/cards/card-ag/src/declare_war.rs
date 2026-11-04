//! `AG:宣战布告`
//!
//! 规则书（docs/rulebook/cards.json, id `AG:宣战布告`）:
//! > 宣战布告：
//! > [手]：
//! > [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。
//!
//! [反击] when another player's card affects you or a tile you own, they pay
//! you 500 and you draw.
//!
//! Reviewed 10/23/2026

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ChainKind;
use card_sdk::{key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const DECLARE_WAR: CardDef = CardDef::new(
    "AG:宣战布告",
    &[
        // > [反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。
        On::Counteract(&[ChainKind::Effect], "", Some(can_counteract), counteract).counter_reason(key!("declare_war_when"))],
).props(&[(card_sdk::abi::prop::SONG, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"宣战布告"#)
        .raw_name(r#"宣战布告"#)
        .tags(&[r#"手"#, r#"反击"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"[反击]当你或你拥有的格子被其他玩家的卡效果影响时：[指定]那名玩家。被[指定]的玩家[支付][使用者]500资金且[使用者]抽1张卡。"#),
            ]),
        ]))
        // sheet cell C2
        .fields(&[("group", r#"Afterglow"#), ("owner", "")]));

fn can_counteract(player_id: i32) -> bool {
    // 规则书[反击]: 「当你或你拥有的格子被其他玩家的卡效果影响时」
    //
    // One condition on the *effect*, and it now reads as one: any effect another
    // player's card declared at me. `effect::hits` covers both because a payment
    // touches its payer *and* its payee.
    //

    // TODO: Move some to CEL?
    // A house write (ownerSet / houseAdd / houseRemove) declares an `effect`
    // over the tile, so the tile half below reaches those too.
    let Some(by) = trigger::by_card().filter(|&by| by != player_id) else {
        return false;
    };
    // 规则书: 「其他玩家的卡效果」 -- a seat that has left the game has no live
    // effects (rulebook 6.1 L81 「所有其正在生效的卡，技能效果停止生效」).
    if ctx::player_out(by) {
        return false;
    }
    // TODO(规则书)（被其他玩家的卡效果影响）: the sheet says 卡效果, but a rival's
    //   skill body drives the same effect/pay host calls and also names this
    //   player, so a skill effect opens this card too -- the source is not
    //   tagged card vs skill.
    if ctx::effect::hits(player_id) {
        return true;
    }
    // 规则书: 「或你拥有的格子」 -- an effect aimed at a tile this player owns.
    (0..ctx::effect::count()).any(|i| {
        let t = ctx::effect::tile(i);
        t >= 0 && ctx::tile_owner(t) == player_id
    })
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let Some(by) = trigger::by_card() else {
        return Ok(());
    };

    // No by == player_id check needed: Handled by the guard

    // 规则书[反击]: 「[指定]那名玩家」 -- the designation is the card's player.
    let Some(hit) = ctx::target(by) else {
        return Ok(());
    };
    let reason = Msg::new(key!("declare_war_reason")).card("card", "AG:宣战布告");
    ctx::transfer(hit, player_id, 500, &reason)?;
    // 规则书[反击]: 「且[使用者]抽1张卡」
    ctx::draw(player_id, 1)?;
    Ok(())
}

//

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
