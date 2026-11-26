//! joins the draw pile, draw 1.
//!
//! 规则书（docs/rulebook/cards.json, id `PP:初次演出事故`）:
//! > 初次演出事故：
//! > [手]：
//! > 依次进行以下操作：
//! > 1. 将此卡放置在[使用者]的[场地]。
//! > 2. 将1张“重叠的声音”加入抽卡区，并洗切。
//! > 3. 抽1张卡。
//! > [持续]：
//! > [拥有者]不可使用丸山彩、若宫伊芙、白鹭千圣或大和麻弥的[主动]
//! > （1）技能，也不可使用冰川日菜的[自动]
//! > （2）技能。
//!

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, steps, CardText};

pub const FIRST_LIVE_ACCIDENT: CardDef = CardDef::new(
    "PP:初次演出事故",
    &[On::Play("", None, first_live_accident)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"初次演出事故"#)
        .raw_name(r#"初次演出事故"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[sec(r#"[手]："#, &[para(r#"依次进行以下操作："#), steps(&[r#"将此卡放置在[使用者]的[场地]。"#, r#"将1张“重叠的声音”加入抽卡区，并洗切。"#, r#"抽1张卡。"#])]), sec(r#"[持续]："#, &[para(r#"[拥有者]不可使用丸山彩、若宫伊芙、白鹭千圣或大和麻弥的[主动]（1）技能，也不可使用冰川日菜的[自动]（2）技能。"#)])]))
        // sheet cell D4
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]),
);

fn first_live_accident(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    let uid = ctx::place_card(
        player_id,
        "PP:初次演出事故",
        &Msg::new(key!("first_live_note")),
    );
    // 规则书[持续]: 「[拥有者]不可使用丸山彩、若宫伊芙、白鹭千圣或大和麻弥的
    // [主动]（1）技能，也不可使用冰川日菜的[自动]（2）技能。」 -- one seal per
    // named *entry*, not the band: the four (1) actives and 日菜's one (2)
    // automatic. The tag is what each skill's own press/hook gate reads through
    // `skill_blocked` (`skillBlock:<tag>`); `:1` / `:2` names the numbered
    // entry so the same persona's other clause stays open (and every other PP
    // skill with it). Bound to the placed instance so they die with the card.
    // The body's own writes cross onto live before the draw routine runs, so
    // the hook the very next `ctx::draw` raises already sees the seal (HOST-02).
    for tag in [
        "skillBlock:skill:丸山彩:With~:1",
        "skillBlock:skill:若宫伊芙:天下统一:1",
        "skillBlock:skill:白鹭千圣:保持坦率的你:1",
        "skillBlock:skill:大和麻弥:朝阳照耀的片刻:1",
        "skillBlock:skill:冰川日菜:日菜抽中的大奖:2",
    ] {
        ctx::set_held(player_id, tag, 1, 0, uid);
    }
    // 规则书[手]: 「并将1张“重叠的声音”加入抽卡区并洗切」
    ctx::add_to_dest(
        player_id,
        "PP:[衍生]重叠的声音",
        ctx::Dest::DeckRandom,
        false,
    );
    ctx::log(
        player_id,
        &Msg::new(key!("first_live_added"))
            .player_id("who", player_id)
            .card("card", "PP:[衍生]重叠的声音"),
    );
    // 规则书[手]: 「然后抽1张卡」
    ctx::draw(player_id, 1)?;
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn first_live_accident_kani(player_id: i32) -> card_sdk::Asked {
    first_live_accident(player_id)
}
