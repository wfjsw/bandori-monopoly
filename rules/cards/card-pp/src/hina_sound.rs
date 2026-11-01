//!
//! 规则书（docs/rulebook/cards.json, id `PP:[冰川日菜]会发出怎样的声音呢？`）:
//! > [冰川日菜]会发出怎样的声音呢？：
//! > [特]：
//!
//! > （1）此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡。
//!
//! > （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。
//!
//! The stand-in runs the substitute's [手] via `play_card` (大和麻弥 has none and is
//! placed directly), then banishes itself. (2) is passive: on a [共鸣] the
//! substitute's instance is marked with the effective card [`ID`] and the
//! substitute's effect is re-picked, so later dispatch runs this card's surface.

use alloc::vec::Vec;

use bandori_std::counter;
use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{it, items, sec, CardText};

const ID: &str = "PP:[冰川日菜]会发出怎样的声音呢？";

pub const HINA_SOUND: CardDef = CardDef::new(ID, &[On::Play("", None, hina_sound)]).meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"会发出怎样的声音呢？"#)
        .raw_name(r#"[冰川日菜]会发出怎样的声音呢？"#)
        .tags(&[r#"特"#])
        .text(CardText::of(&[sec(
            r#"[特]："#,
            &[items(&[
                it(r#"此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡。"#),
                it(r#"[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。"#),
            ])],
        )]))
        // sheet cell D19
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", r#"冰川日菜"#)]),
);

// 规则书[特]（1）: 「此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡」
///
/// 大和麻弥 (「可能性为∞」) has no hand-play [手] -- its `On::Play` is the [共鸣]
/// pile-swap press and places nothing -- so both the play here and the (2) swap
/// must place it explicitly rather than run that entry.
const MAMI: &str = "PP:[大和麻弥]可能性为∞";
const SUBS: [&str; 4] = [
    "PP:[丸山彩]憧憬的前方",
    MAMI,
    "PP:[白鹭千圣]微笑的铁假面",
    "PP:[若宫伊芙]属于我的武士道！",
];

/// The player's current field instance ids. Taken **before** the placement, so
/// the stand-in's own instance -- still on the field while its body runs -- is
/// excluded from the "new instance" search.
fn instance_floor(player_id: i32) -> Vec<i32> {
    ctx::field_instances(player_id)
        .into_iter()
        .map(|(uid, _)| uid)
        .collect()
}

/// The first instance of `card` that is not in `floor`; the play/place imports
/// answer `Dest`, not an instance id, so the body finds its own placement here.
fn new_instance_since(player_id: i32, floor: &[i32], card: &str) -> Option<i32> {
    ctx::field_instances(player_id)
        .into_iter()
        .find(|(uid, id)| !floor.contains(uid) && id.as_str() == card)
        .map(|(uid, _)| uid)
}

/// The instance's **own** card. `effective_card_at` answers the stand-in once
/// (2) has marked it, so it cannot name the current substitute.
fn own_card_at(player_id: i32, instance: i32) -> Option<&'static str> {
    ctx::field_instances(player_id)
        .into_iter()
        .find(|(uid, _)| *uid == instance)
        .map(|(_, card)| card.as_str())
}

fn hina_sound(player_id: i32) -> card_sdk::Asked {
    // TODO(规则书)（此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡）: the ask offers every sub
    // unconditionally -- 「符合使用条件时」 wants a per-substitute `guard` filter (none is
    // refused today).
    let i = ctx::ask_card(
        player_id,
        &Msg::new(key!("hina_sound_title")),
        &Msg::new(key!("hina_sound_ask")),
        &SUBS,
    )?;
    let sub = SUBS[i];
    let as_msg = Msg::new(key!("hina_sound_as"))
        .card("card", ID)
        .card("sub", sub);
    ctx::log(player_id, &as_msg);
    // The floor is taken before the placement, so the stand-in's own instance
    // is not mistaken for the substitute it is about to place.
    let before = instance_floor(player_id);
    // 规则书[特]（1）: 「可替代……的专属卡」 -- run the chosen exclusive card's [手]
    // (大和麻弥 has no [手]; it is placed instead).
    if sub == MAMI {
        // 大和麻弥 has no [手]: its only play entry is the pile-swap press, which
        // needs the card placed first. The [共鸣] it prints runs inside `swap`.
        ctx::place_card(player_id, sub, &as_msg);
        crate::infinite_possibility::swap(player_id)?;
    } else {
        ctx::play_card(sub, player_id)?;
    }
    let Some(instance) = new_instance_since(player_id, &before, sub) else {
        return Ok(());
    };
    // 规则书（2）「[共鸣]」: the change costs the fuel, and is passive -- asked
    // here, at the only moment 「本卡在[场地]」 can hold, and applied on yes.
    // A decline leaves the substitute running its own effect.
    if crate::resonance::try_resonance(player_id)? {
        ctx::set_effective_card_at(instance, ID);
        // TODO(G08)（`created_by`）: the substitute is no longer the stand-in's
        // child -- `ctx::set_created_by_at` is not in yet, so the mark is the
        // only record and the edge keeps pointing at the banished stand-in.
        // ctx::set_created_by_at(instance, -1);
        // The changed effect itself (OPEN-substitution-effect): re-pick the
        // substitute, carrying the crystals across.
        hina_change(player_id, instance)?;
    }
    // The run's bound uid is the stand-in; `hina_change` already unplaced it, so
    // the substitute it placed stays.
    ctx::set_dest(ctx::Dest::Banished);
    Ok(())
}

/// The changed effect. The marked instance's own card is the current
/// substitute; `effective_card_at` is the stand-in, so it cannot answer.
/// What 「改变…效果」 changes it *to* is unprinted (pp.md
/// `OPEN-substitution-effect`); the re-pick below is the interim reading.
fn hina_change(player_id: i32, instance: i32) -> card_sdk::Asked {
    let crystals = ctx::counter_at(instance, counter::CRYSTALS);
    let Some(cur) = own_card_at(player_id, instance) else {
        return Ok(());
    };
    let others: Vec<&str> = SUBS.iter().copied().filter(|s| *s != cur).collect();
    let pick = ctx::ask_card(
        player_id,
        &Msg::new(key!("hina_sound_title")),
        &Msg::new(key!("hina_sound_swap_ask")),
        &others,
    )?;
    let sub = others[pick];
    let as_msg = Msg::new(key!("hina_sound_as"))
        .card("card", ID)
        .card("sub", sub);
    ctx::log(player_id, &as_msg);
    ctx::unplace_at(instance);
    let before = instance_floor(player_id);
    if sub == MAMI {
        ctx::place_card(player_id, sub, &as_msg); // 大和麻弥 has no [手]
    } else {
        ctx::play_card(sub, player_id)?;
    }
    let Some(next) = new_instance_since(player_id, &before, sub) else {
        return Ok(());
    };
    ctx::set_effective_card_at(next, ID);
    // The crystals are the substitute's own ([手] filler), so they travel with
    // the change -- the same read/write pair `crystal_swap.rs` uses.
    ctx::add_counter_at(next, counter::CRYSTALS, crystals, 0);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn hina_sound_kani(player_id: i32) -> card_sdk::Asked {
    hina_sound(player_id)
}
