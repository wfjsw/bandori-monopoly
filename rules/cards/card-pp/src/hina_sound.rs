//! `PP:[冰川日菜]会发出怎样的声音呢？` -- C# `CardHinaSound`
//! (MatchHost.cs:8169-8215): play as one of the Pastel✽Palettes exclusive cards.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:[冰川日菜]会发出怎样的声音呢？`）:
//! > [冰川日菜]会发出怎样的声音呢？：
//! > [特]：
//! >
//! > （1）此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡。
//! >
//! > （2）[共鸣]本卡在[场地]时改变本卡代替的专属卡效果。
//!
//! The stand-in runs the substitute's [手] via `play_card`; the RealId marking
//! and the [共鸣] swap need hooks the ABI lacks (below).

use card_sdk::{ctx, key, CardDef, On, Msg};

pub const HINA_SOUND: CardDef = CardDef::new("PP:[冰川日菜]会发出怎样的声音呢？", &[
    On::Play(None, hina_sound)]);

/// C# `CardHinaSound.Subs` -- the exclusive cards this can stand in for when
/// played from hand (each has a hand-play [手] and places itself).
///
/// The rulebook also names 大和麻弥 (「可能性为∞」); the C# keeps it out of the
/// play list because that card is placed by its deck-reveal [特], and only
/// offers it in the [共鸣] swap (`HinaSwapFx`).
const SUBS: [&str; 3] = [
    "PP:[丸山彩]憧憬的前方",
    "PP:[白鹭千圣]微笑的铁假面",
    "PP:[若宫伊芙]属于我的武士道！"];

fn hina_sound(player_id: i32) {
    // 规则书[特]（1）: 「此卡在符合使用条件时可替代丸山彩，大和麻弥，白鹭千圣，或若宫伊芙的专属卡」
    // -- C# `WhyNot` offers the subs whose own `WhyNot` is clear
    // (`H.SubWhyNot(seat, id) == null`) and refuses when none are. `H.SubWhyNot`
    // is still missing, and all three subs' `WhyNot` are always clear (no
    // overrides), so the gate never refuses -- `cant_play: None` is faithful.
    let i = ctx::ask_card(
        player_id,
        &Msg::new(key!("hina_sound_title")),
        &Msg::new(key!("hina_sound_ask")),
        &SUBS,
    );
    let sub = SUBS[i];
    // C# `H.Log("text", i, "「" + CardName + "」当作「" + H.CardTitle(sub) + "」打出")`.
    ctx::log(
        player_id,
        &Msg::new(key!("hina_sound_as"))
            .card("card", "PP:[冰川日菜]会发出怎样的声音呢？")
            .card("sub", sub),
    );
    // 规则书[特]（1）: 「可替代……的专属卡」 -- run the chosen exclusive card's [手].
    ctx::play_card(sub, player_id);
    // C# when the substitute places itself (`inner.Dest == "placed"`) the stand-in
    // itself is "gone" (`c.Dest = "gone"`); all three subs above place themselves,
    // so this card leaves play rather than landing in the discard.
    ctx::set_dest(ctx::Dest::Banished);
    // TODO(规则书)[特]（1）: marking the placed substitute with RealId -- needs
    // per-card Mem state (C# `card.RealId = c.Id` on the just-placed `Card`) so
    // the [共鸣] swap can find it (`p.RealId != null && NewCard(p.RealId) is
    // CardHinaSound`).
    // TODO(规则书)[特]（2）: 「[共鸣]本卡在[场地]时改变本卡代替的专属卡效果」 -- needs
    // H.TryResonance (discard 「PP:[衍生]共鸣」 from hand) and the Fx.Actions hook
    // (C# `HinaSwapFx.Actions` offering 「[共鸣] 换成别的专属卡」) to swap the
    // substituted exclusive card among
    // 「[丸山彩]憧憬的前方」/「[大和麻弥]可能性为∞」/「[白鹭千圣]微笑的铁假面」/「[若宫伊芙]属于我的武士道！」
    // while keeping its crystals (`H.Unplace` + `H.PlaceCard(..., crystals, ...)`).
}