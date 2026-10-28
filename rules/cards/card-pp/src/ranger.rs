//! `PP:[衍生]魔法战队Pastel✽Ranger` -- C# `CardRanger` (MatchHost.cs:7766-7814):
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[衍生]魔法战队Pastel✽Ranger`）:
//! > [衍生]魔法战队Pastel✽Ranger：
//! > [特]：
//! > 仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用。
//! > [手]：
//! > 根据场上你拥有的卡数量并依次进行以下操作，如果[共鸣]则视为数量加1：
//! > 1. 数量至少为1则[获得]500资金；
//! > 2. 数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]；
//! > 3. 数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡；
//! > 4. 数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），盖房后减少1层”；
//! > 5.数量至少为6则抽1张卡。
//!
//! a ladder of rewards keyed on how many field cards the user has.
//! The ladder body runs off `cards_in(Field)` for the placed-card count.

use bandori_std::ctx;
use card_sdk::{key, Action, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};
use skill_groups::fans;

const ID: &str = "PP:[衍生]魔法战队Pastel✽Ranger";
/// Banked build-off layers 「下次盖房时减免2000（可溢出），盖房后减少1层」.
const BUILD_OFF: &str = "ranger.buildOff";

pub const RANGER: CardDef = CardDef::new(
    "PP:[衍生]魔法战队Pastel✽Ranger",
    &[
        On::Play("", Some(cant_play), ranger),
        // 规则书[手]4: 「下次盖房时减免2000（可溢出），盖房后减少1层」 -- the
        // status lives on the player, so its build hooks must outlive the body;
        // the card stands on the field as the listener (spec C-PP-ranger-01:
        // 「场上你拥有的卡」 counts the Ranger itself once played).
        On::Action(Action::BuildBefore(card_sdk::pre::MINE, None, before_build)),
        On::Action(Action::BuildAfter(card_sdk::pre::MINE, None, after_build)).silent(),
    ],
)
.props(&[(card_sdk::abi::prop::EST_COST, 2000)])
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"魔法战队Pastel✽Ranger"#)
        .raw_name(r#"[衍生]魔法战队Pastel✽Ranger"#)
        .derived(true)
        .tags(&[r#"手"#, r#"特"#, r#"衍生"#])
        .text(CardText::of(&[
            sec(r#"[特]："#, &[
                para(r#"仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用。"#),
            ]),
            sec(r#"[手]："#, &[
                para(r#"根据场上你拥有的卡数量并依次进行以下操作，如果[共鸣]则视为数量加1："#),
                steps(&[
                    r#"数量至少为1则[获得]500资金；"#,
                    r#"数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]；"#,
                    r#"数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡；"#,
                    r#"数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），盖房后减少1层”；"#,
                    r#"数量至少为6则抽1张卡。"#,
                ]),
            ]),
        ]))
        // sheet cell D14
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]),
);

/// 粉丝数量 for the [特] range: up + down fan tokens (`fans::total`, the
/// `ppFansUp`/`ppFansDown` counters; C# `H.FansUp` / `H.FansDown`).

fn cant_play(player_id: i32) -> Option<Msg> {
    // 规则书[特]: 「仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用」
    // C# `CardRanger.WhyNot`: `range = 30 - H.Fans(player_id)`, refuse unless some
    // owned tile has houses and `H.Dist(pos, t) <= range`.
    let range = 30 - (fans::total(player_id));
    let pos = ctx::player_pos(player_id);
    let ok = ctx::owned_tiles(player_id)
        .into_iter()
        .any(|t| ctx::houses_of(t) > 0 && ctx::dist(pos, t) <= range);
    if !ok {
        // Same (unclamped) figure as the gate above, so the message cannot
        // advertise a reach the comparison does not use.
        return Some(Msg::new(key!("ranger_why_not")).i("n", range as i64));
    }
    None
}

/// 规则书[手]: 「根据场上你拥有的卡数量」 -- C# `H.PlacedOf(i).Count`.
fn placed_count(player_id: i32) -> i32 {
    // TODO(规则书)（根据场上你拥有的卡数量）: 打出中的此卡自身也应计入（spec
    // C-PP-ranger-01）-- 现在只数结算时已经在场的卡，自身不算
    ctx::cards_in(player_id, ctx::CardPile::Field).len() as i32
}

fn ranger(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「根据场上你拥有的卡数量」
    let mut n = placed_count(player_id);
    // 规则书[手]: 「如果[共鸣]则视为数量加1」 -- the cost is the discard, and it
    // buys a fatter count rather than anything on its own.
    if crate::resonance::try_resonance(player_id)? {
        n += 1;
    }
    ctx::log(player_id, &Msg::new(key!("ranger_count")).i("n", n as i64));
    // 规则书[手]1: 「数量至少为1则[获得]500资金」
    if n >= 1 {
        ctx::gain(
            player_id,
            500,
            &Msg::new(key!("ranger_why")).i("n", n as i64),
        )?;
    }
    // 规则书[手]2: 「数量至少为3则为Pastel✽Palettes乐队卡添加3个[奇迹水晶]」
    if n >= 3 {
        ctx::add_group_crystals(player_id, 3, i32::MAX);
    }
    // 规则书[手]3: 「数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡」
    if n >= 4 {
        // TODO(规则书)（数量至少为4则移除Pastel✽Palettes乐队卡4个[奇迹水晶]并抽1张卡）:
        // 不足 4 个时仍按「移除4个」 -- 现在池底封 0，只移除现有的（OPEN-floor）
        ctx::add_group_crystals(player_id, -4, i32::MAX);
        ctx::draw(player_id, 1)?;
    }
    // 规则书[手]4: 「数量至少为5则获得1层状态“失去2000资金，下次盖房时减免2000（可溢出），
    // 盖房后减少1层”」 -- the 2,000 loss is `H.LoseR(i, 2000, CardName)`.
    if n >= 5 {
        ctx::pay(
            player_id,
            2000,
            &Msg::new(key!("ranger_why")).i("n", n as i64),
        )?;
        // 规则书[手]4: 「下次盖房时减免2000（可溢出），盖房后减少1层」 -- bank
        // one layer on the player (persistent across turns). `before_build`
        // re-applies the stack on the build's turn value and `after_build` pops
        // one layer; the engine refunds the 「可溢出」 excess past the cost.
        // TODO(规则书)（下次盖房时减免2000（可溢出），盖房后减少1层）: two copies of
        //   this card on the field both run the hooks, so one build pops two
        //   banked layers; the book's per-layer status knows one pop per build.
        ctx::state::add(player_id, BUILD_OFF, 1);
        // The hooks ride this instance, so the card stays in play (it also
        // counts for a later Ranger's 「场上你拥有的卡数量」).
        ctx::set_dest(ctx::Dest::Field);
        ctx::place_card(
            player_id,
            ID,
            &Msg::new(key!("ranger_why")).i("n", n as i64),
        );
    }
    // 规则书[手]5: 「数量至少为6则抽1张卡」
    if n >= 6 {
        ctx::draw(player_id, 1)?;
    }
    Ok(())
}

/// 规则书[手]4: 「下次盖房时减免2000（可溢出）」 -- re-apply every banked layer
/// to this build. The engine takes the discount off the cost and refunds the
/// part past it (「可溢出」).
fn before_build(player_id: i32) -> card_sdk::Asked {
    let n = ctx::state::get(player_id, BUILD_OFF);
    if n <= 0 {
        return Ok(());
    }
    ctx::set_build_discount(2000 * n, 1);
    Ok(())
}

/// 规则书[手]4: 「盖房后减少1层」 -- a build consumes one banked layer.
fn after_build(player_id: i32) -> card_sdk::Asked {
    if ctx::state::get(player_id, BUILD_OFF) > 0 {
        ctx::state::add(player_id, BUILD_OFF, -1);
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn ranger_kani(player_id: i32) -> card_sdk::Asked {
    ranger(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn before_build_kani(player_id: i32) -> card_sdk::Asked {
    before_build(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn after_build_kani(player_id: i32) -> card_sdk::Asked {
    after_build(player_id)
}
