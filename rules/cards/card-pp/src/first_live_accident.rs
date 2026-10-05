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
//! joins the draw pile, draw 1. The [持续] skill lock is enforced by the skill
//! system (C# `H.FanSkillBlocked`), not by a card hook -- TODO below.

use card_sdk::{ctx, key, CardDef, Msg};

pub const FIRST_LIVE_ACCIDENT: CardDef = CardDef {
    id: "PP:初次演出事故",
    play: Some(first_live_accident),
    can_react: None,
    react: None,
    why_not: None,
};

fn first_live_accident(seat: i32) {
    // 规则书[手]: 「将此卡放置在[使用者]的[场地]」
    ctx::set_dest(ctx::Dest::Field);
    ctx::place_card(seat, "PP:初次演出事故", &Msg::new(key!("first_live_note")));
    // 规则书[手]: 「并将1张“重叠的声音”加入抽卡区并洗切」
    ctx::add_to_deck(seat, "PP:[衍生]重叠的声音", true);
    ctx::log(
        seat,
        &Msg::new(key!("first_live_added")).seat("who", seat).card("card", "PP:[衍生]重叠的声音"),
    );
    // 规则书[手]: 「然后抽1张卡」
    ctx::draw(seat, 1);
    // TODO(规则书): [持续] 「[拥有者]不可使用任何Pastel✽Palettes角色的（2）技能」 -- the
    // skill system must refuse the (2) skills while this card is live
    // (C# `H.FanSkillBlocked`: `PlacedOf(seat).Any(c => c.Live && c is CardFirstLiveAccident)`).
}