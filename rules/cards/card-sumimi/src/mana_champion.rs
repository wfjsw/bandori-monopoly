//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:（真奈）歌唱大赛5连冠`）:
//! > （真奈）歌唱大赛5连冠：
//! > [反击]当你或你的格子即将受到来自你以外的效果影响时打出此卡，此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。
//!
//! [反击] that lets the other players counteract as if they were the target.
//!
//! CH-05 join: the `SELF_REDIRECT` property *arms* the join -- the engine
//! opens the answering window on the answered link when this counter is
//! declared. This body opens the round (`ctx::join::open`), reports it, and
//! owns **both** draws of the clause: one optional draw per shielding joiner
//! (「若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡」)
//! and the fallback when nobody declared
//! (「若没有人在此卡的效果期间打出[反击]卡」).

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{prop, ChainKind};
use card_sdk::CardText;
use card_sdk::{key, CardDef, Msg, On};

pub const MANA_CHAMPION: CardDef = CardDef::new(
    "Sumimi:（真奈）歌唱大赛5连冠",
    &[
        // 规则书[反击]: 「当你或你的格子即将受到来自你以外的效果影响时」
        On::Counteract(
            &[ChainKind::Effect],
            // 「来自你以外」 is any causer but the holder: another player's card
            // (`by >= 0`), or a rent settlement -- another player's tile
            // charging us, which carries no card (`by_card` is -1) but is
            // `pay_is_rent`.
            // 规则书[反击]: 「即将受到来自你以外的效果影响时」
            "by != owner && (by >= 0 || pay_is_rent)",
            Some(can_counteract),
            counteract,
        )
        .counter_reason(key!("mana_champion_when")),
    ],
)
// 规则书[反击]: 「其反击卡中针对打出玩家自身的效果改为你」
.props(&[(prop::SELF_REDIRECT, 1)]).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"歌唱大赛5连冠"#)
        .raw_name(r#"（真奈）歌唱大赛5连冠"#)
        .tags(&[r#"反击"#])
        .text(CardText::para(r#"[反击]当你或你的格子即将受到来自你以外的效果影响时打出此卡，此时场上其他玩家可如同自身的对应目标被指定一般打出[反击]卡，且其反击卡中针对打出玩家自身的效果改为你。若以此种方式使你免于受到该影响，打出那张[反击]卡的玩家可抽一张卡。若没有人在此卡的效果期间打出[反击]卡，你抽一张卡。"#))
        // sheet cell L10
        .fields(&[("group", r#"Sumimi"#), ("owner", r#"纯田真奈"#)]));

/// The half of the trigger the CEL condition cannot express: an effect touches its
/// payer as well as its payee (`ctx::effect::hits` counts `from`), and a tile
/// of the holder is named by the effect (「或你的格子」).
fn can_counteract(owner: i32) -> bool {
    // 规则书[反击]: 「当你或你的格子即将受到来自你以外的效果影响时」
    // A causer that has left the game has no live effect (rulebook 6.1 L81).
    if trigger::by_card().is_some_and(ctx::player_out) {
        return false;
    }
    if ctx::effect::hits(owner) {
        return true;
    }
    // 规则书: 「或你的格子」 -- an effect aimed at a tile this player owns.
    // TODO(规则书)（或你的格子）: an effect that only touches a tile you own
    //   (add_house / set_houses) declares no effect at all, so it opens no
    //   window here; a mortgaged-tile force-buy (also 「你以外」) is a direct
    //   transfer outside the pay pipeline, so it raises no effect link either.
    (0..ctx::effect::count()).any(|i| {
        let t = ctx::effect::tile(i);
        t >= 0 && ctx::tile_owner(t) == owner
    })
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("mana_champion_log")).player_id("who", player_id),
    );
    // 规则书[反击]: 「此时场上其他玩家可如同自身的对应目标被指定一般打出
    // [反击]卡」 -- the nested round on the link this counter answers. A joiner
    // applies the redirect itself (`ctx::join::opener`, 「改为你」).
    let _ = ctx::join::open()?;
    // 规则书[反击]: 「若没有人在此卡的效果期间打出[反击]卡，你抽一张卡」 --
    // any declaration during the window bars the fallback, shielding or not.
    let count = ctx::join::joiner_count();
    if count == 0 {
        ctx::draw(player_id, 1)?;
        return Ok(());
    }
    // 规则书[反击]: 「若以此种方式使你免于受到该影响，打出那张[反击]卡的
    // 玩家可抽一张卡」 -- the draw is this card's clause, taken by its owner
    // for each joiner that shielded (`join::shield`), optionally per joiner.
    for i in 0..count {
        if !ctx::join::joiner_shielded(i) {
            continue;
        }
        let who = ctx::join::joiner(i);
        let take = ctx::ask_yes(
            player_id,
            &Msg::new(key!("net_error_draw_title")),
            &Msg::new(key!("mana_champion_draw")).player_id("who", who),
        )?;
        if take {
            ctx::draw(who, 1)?;
        }
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}
