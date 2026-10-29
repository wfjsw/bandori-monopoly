//! `PP:[冰川日菜]会发出怎样的声音呢？` -- C# `CardHinaSound`
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[冰川日菜]会发出怎样的声音呢？`）:
//! > [冰川日菜]会发出怎样的声音呢？：
//! > [特]：
//!
//! > （1）此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡。
//!
//! > （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。
//!
//! (MatchHost.cs:8169-8215): play as one of the Pastel✽Palettes exclusive cards.
//! The stand-in runs the substitute's [手] via `play_card` (大和麻弥 has none and
//! is placed directly); the RealId marking and the [共鸣] swap need hooks the
//! ABI lacks (below).

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{it, items, sec, CardText};

pub const HINA_SOUND: CardDef = CardDef::new(
    "PP:[冰川日菜]会发出怎样的声音呢？",
    &[On::Play("", None, hina_sound)],
)
.meta(
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

/// C# `CardHinaSound.Subs` -- the exclusive cards this can stand in for.
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

fn hina_sound(player_id: i32) -> card_sdk::Asked {
    // 规则书[特]（1）: 「此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡」
    // -- C# `WhyNot` offers the subs whose own `WhyNot` is clear
    // (`H.SubWhyNot(seat, id) == null`) and refuses when none are. `H.SubWhyNot`
    // is still missing, and every sub's `WhyNot` is always clear (no
    // overrides), so the gate never refuses -- `cant_play: None` is faithful.
    // TODO(规则书)（此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡）:
    //   the ask offers every sub unconditionally -- 「符合使用条件时」 wants a
    //   per-substitute `why_not` filter (none is refused today).
    let i = ctx::ask_card(
        player_id,
        &Msg::new(key!("hina_sound_title")),
        &Msg::new(key!("hina_sound_ask")),
        &SUBS,
    )?;
    let sub = SUBS[i];
    // C# `H.Log("text", i, "「" + CardName + "」当作「" + H.CardTitle(sub) + "」打出")`.
    let as_msg = Msg::new(key!("hina_sound_as"))
        .card("card", "PP:[冰川日菜]会发出怎样的声音呢？")
        .card("sub", sub);
    ctx::log(player_id, &as_msg);
    // 规则书[特]（1）: 「可替代……的专属卡」 -- run the chosen exclusive card's [手]
    // (大和麻弥 has no [手]; it is placed instead).
    if sub == MAMI {
        // 大和麻弥 has no [手]: its only play entry is the pile-swap press.
        ctx::place_card(player_id, sub, &as_msg);
    } else {
        ctx::play_card(sub, player_id)?;
    }
    // C# when the substitute places itself (`inner.Dest == "placed"`) the stand-in
    // itself is "gone" (`c.Dest = "gone"`); every sub above places itself, so
    // this card leaves play rather than landing in the discard.
    ctx::set_dest(ctx::Dest::Banished);
    // TODO(规则书)[特]（2）: marking the placed substitute with RealId -- needs
    // per-card Mem state (C# `card.RealId = c.Id` on the just-placed `Card`) so
    // the [共鸣] swap can find it (`p.RealId != null && NewCard(p.RealId) is
    // CardHinaSound`).
    // TODO(规则书)[特]（2）: 「[共鸣]本卡在[场地]时改变本卡代替的专属卡效果」 -- needs
    // the RealId marking above and the Fx.Actions hook
    // (C# `HinaSwapFx.Actions` offering 「[共鸣] 换成别的专属卡」) to swap the
    // substituted exclusive card among
    // 「[丸山彩]憧憬的前方」/「[大和麻弥]可能性为∞」/「[白鹭千圣]微笑的铁假面」/「[若宫伊芙]属于我的武士道！」
    // while keeping its crystals (`H.Unplace` + `H.PlaceCard(..., crystals, ...)`).
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
