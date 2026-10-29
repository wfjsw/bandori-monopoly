//! `PP:初次演出事故` -- C# `CardFirstLiveAccident`: stay in play, 「重叠的声音」
//!
//! 规则书（docs/rulebook/cards.json, id `PP:初次演出事故`）:
//! > 初次演出事故：
//! > [手]：
//! > 将此卡放置在[使用者]的[场地]并将1张“重叠的声音”加入抽卡区并洗切，然后抽1张卡。
//! > [持续]：
//! > [拥有者]不可使用任何Pastel✽Palettes角色的
//! > （2）技能。
//!
//! joins the draw pile, draw 1. The [持续] skill lock is a `skillBlock:Pastel✽Palettes`
//! token bound to the placed instance -- each P✽P (2) skill has to call
//! `skill_blocked` itself (there is no central dispatch gate), unlike the C# live scan
//! (`H.FanSkillBlocked`: `PlacedOf(player_id).Any(c => c.Live && c is
//! CardFirstLiveAccident)`).

use card_sdk::{ctx, key, CardDef, Msg, On};
use card_sdk::{para, sec, CardText};

pub const FIRST_LIVE_ACCIDENT: CardDef = CardDef::new(
    "PP:初次演出事故",
    &[On::Play("", None, first_live_accident)],
)
.meta(
    &card_sdk::meta::CardMeta::new()
        .name(r#"初次演出事故"#)
        .raw_name(r#"初次演出事故"#)
        .tags(&[r#"手"#, r#"持续"#])
        .text(CardText::of(&[
            sec(r#"[手]："#, &[
                para(r#"将此卡放置在[使用者]的[场地]并将1张“重叠的声音”加入抽卡区并洗切，然后抽1张卡。"#),
            ]),
            sec(r#"[持续]："#, &[
                para(r#"[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能。"#),
            ]),
        ]))
        // sheet cell D4
        .fields(&[("group", r#"Pastel✽Palettes"#), ("owner", "")]),
);

fn first_live_accident(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    let uid = ctx::place_card(
        player_id,
        "PP:初次演出事故",
        &Msg::new(key!("first_live_note")),
    );
    // 规则书[持续]: 「[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能。」 -- the
    // token is what the P✽P skills' press gates read. One concept, one name:
    // the band's real spelling (`skill_blocked` builds `skillBlock:<band>`).
    // Bound to the placed instance so it dies with the card.
    // TODO(规则书)（[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能。）: a guest
    //   write does not reach the host routine that raises this body's own draw
    //   hook (BACKLOG HOST-02), so a (2) window on that very draw still opens.
    ctx::set_tok(player_id, "skillBlock:Pastel✽Palettes", 1, 0, uid);
    // 规则书[手]: 「并将1张“重叠的声音”加入抽卡区并洗切」
    ctx::add_to_dest(player_id, "PP:[衍生]重叠的声音", ctx::Dest::DeckRandom);
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
