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
//! token the shared skill-press gate reads (C# `H.FanSkillBlocked`:
//! `PlacedOf(player_id).Any(c => c.Live && c is CardFirstLiveAccident)`).

use card_sdk::{ctx, key, CardDef, Msg, On};

pub const FIRST_LIVE_ACCIDENT: CardDef =
    CardDef::new("PP:初次演出事故", &[On::Play("", None, first_live_accident)]);

fn first_live_accident(player_id: i32) -> card_sdk::Asked {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(
        player_id,
        "PP:初次演出事故",
        &Msg::new(key!("first_live_note")),
    );
    // [持续] 「[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能」 -- the
    // token is what the P✽P skills' press gates read. One concept, one name:
    // the band's real spelling (`skill_blocked` builds `skillBlock:<band>`).
    ctx::set_tok(player_id, "skillBlock:Pastel✽Palettes", 1);
    // 规则书[手]: 「并将1张“重叠的声音”加入抽卡区并洗切」
    ctx::add_to_deck(player_id, "PP:[衍生]重叠的声音", true);
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
