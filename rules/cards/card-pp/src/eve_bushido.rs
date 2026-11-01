//!
//! 规则书（docs/rulebook/cards.json, id `PP:[若宫伊芙]属于我的武士道！`）:
//! > [若宫伊芙]属于我的武士道！：
//! > [手]：
//! > 将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。
//! > [持续]：
//!
//! > （1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。
//!
//! > （2）[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家则使用1个[奇迹水晶]并依次进行以下操作：
//! > 1. 地契主人投掷1d6；
//! > 2. [拥有者]投掷3d4；
//! > 3. 投掷点数低的玩家[支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），如果[共鸣]则[支付]金额改为“投掷点差”×50。
//!
//! for money. The duel runs in the `SettleBefore` hook; the crystal growth
//! runs on the `Drew` hook.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::{roll_source, ActionKind, RollPurpose, TriggerKind};
use card_sdk::{it, it_sub, items, para, sec, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

/// The caster (1-based). The hooks key on the [使用者], which the rule names,
/// not on the field owner that happens to hold the card.
const USER_KEY: &str = "eve_bushido_user";

pub const EVE_BUSHIDO: CardDef = CardDef::new(
    "PP:[若宫伊芙]属于我的武士道！",
    &[
        On::Play("", None, eve_bushido),
        On::Action(Action::Drew("", Some(drew_guard), drew)),
        On::Action(Action::SettleBefore("", None, settle_before)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"属于我的武士道！"#)
        .raw_name(r#"[若宫伊芙]属于我的武士道！"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。"#),
            ]),
            sec(r#"[持续]："#, &[
                items(&[
                    it(r#"[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。"#),
                    it_sub(
                        r#"[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家则使用1个[奇迹水晶]并依次进行以下操作："#,
                        &[
                        r#"地契主人投掷1d6；"#,
                        r#"[拥有者]投掷3d4；"#,
                        r#"投掷点数低的玩家[支付]投掷点数高的玩家[拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），如果[共鸣]则[支付]金额改为“投掷点差”×50。"#,
                        ],
                    ),
                ]),
            ]),
        ]))
        // sheet cell D20
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", r#"若宫伊芙"#)]));

fn eve_bushido(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]场上」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PP:[若宫伊芙]属于我的武士道！",
        &Msg::new(key!("eve_bushido_note")),
    );
    ctx::set_prop(USER_KEY, player_id + 1);
    // 规则书[手]: 「并投掷12d4，获得投掷结果*60的资金」
    // `ctx::roll` honours a forced extreme (「以理论最大值或最小值结算」).
    let n = ctx::roll(player_id, 12, 4, RollPurpose::Effect, roll_source::CARD)?;
    ctx::gain(
        player_id,
        n * 60,
        &Msg::new(key!("eve_bushido_reason")).i("n", n as i64),
    )?;
    Ok(())
}

/// 规则书[持续]（1）: 「[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）」 Pure guard for [`drew`] -- the
/// activation gate. `false` means the card is not activated at all.
///
/// 规则书: 「[使用者]抽卡后」 -- the drawer must be the caster recorded at play,
/// not merely the field owner.
fn drew_guard(player_id: i32) -> bool {
    if !ctx::is_placed() {
        return false;
    }
    // A card put down without its play body (harness raw placement) recorded no
    // caster: placement is always on the caster's own field, so fall back to the
    // field owner rather than never firing.
    let user = ctx::prop(USER_KEY);
    let user = if user == 0 { player_id } else { user - 1 };
    trigger::player_id() == user
}

fn drew(_player_id: i32) -> card_sdk::Asked {
    if trigger::value() <= 0 {
        return Ok(());
    }
    // 规则书[持续]（1）: 「为此卡添加1个[奇迹水晶]（上限3）」
    ctx::add_crystals(1, 3)?;
    Ok(())
}

/// 规则书[持续]（2）: 「[结算]前如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家」
fn settle_before(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    // 规则书[持续]（2）: 「如果此卡上有[奇迹水晶]且[拥有者]所在格子的地契主人为其他玩家」
    if trigger::player_id() != player_id || ctx::crystals() <= 0 {
        return Ok(());
    }
    let at = trigger::tile();
    if at < 0 {
        return Ok(());
    }
    let owner = ctx::tile_owner(at);
    // 规则书[持续]（2）: 「[拥有者]所在格子的地契主人为其他玩家」
    if owner < 0 || owner == player_id || ctx::player_out(owner) {
        return Ok(());
    }
    // 规则书[持续]（2）: 「使用1个[奇迹水晶]」
    ctx::add_crystals(-1, 0)?;
    // 规则书[持续]（2）1: 「地契主人投掷1d6」
    let a = ctx::roll(owner, 1, 6, RollPurpose::Effect, roll_source::CARD)?;
    // 规则书[持续]（2）2: 「[拥有者]投掷3d4」
    let b = ctx::roll(player_id, 3, 4, RollPurpose::Effect, roll_source::CARD)?;
    // 规则书[持续]（2）3: 「如果[共鸣]则[支付]金额改为“投掷点差”×50」 -- the 共鸣
    // rider is printed after the tie parenthetical and paying the fuel resolves
    // its own effect, so the offer runs before the tie return too.
    let resonated = crate::resonance::try_resonance(player_id)?;
    // 规则书[持续]（2）3: 「如果平局则双方互不支付」
    if a == b {
        ctx::log(player_id, &Msg::new(key!("eve_bushido_tie")));
        return Ok(());
    }
    // 规则书[持续]（2）3: 「[支付]……房屋数量加1×100」
    let mut amount = (ctx::houses_of(at) + 1) * 100;
    if resonated {
        amount = (a - b).abs() * 50;
    }
    // 规则书[持续]（2）3: 「投掷点数低的玩家[支付]投掷点数高的玩家」
    let (from, to) = if a < b {
        (owner, player_id)
    } else {
        (player_id, owner)
    };
    ctx::transfer(
        from,
        to,
        amount,
        &Msg::new(key!("eve_bushido_reason")).i("n", amount as i64),
    )?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn eve_bushido_kani(player_id: i32) -> card_sdk::Asked {
    eve_bushido(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn drew_kani(player_id: i32) -> card_sdk::Asked {
    drew(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn settle_before_kani(player_id: i32) -> card_sdk::Asked {
    settle_before(player_id)
}
