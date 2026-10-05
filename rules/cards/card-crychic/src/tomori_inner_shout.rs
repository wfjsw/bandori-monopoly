//! `CRYCHIC:（灯）内心的呐喊` -- C# `CardTomoriInnerShout` (MatchHost.cs:3217-3294):
//! re-declare 「想要成为人类」's X for money, then a MyGO placement + pull skill.
//!
//! 规则书（docs/rulebook/cards.json, id `CRYCHIC:（灯）内心的呐喊`）:
//! > （灯）内心的呐喊：
//! > （1）打出此卡时，你可重置一次“想要成为人类”所声明的X（不影响水晶）并获得两个X差值*120的资金。
//! > （2）此卡不会被你乐队技能的
//! > （2）效果移除，乐队技能的
//! > （2）效果发动时展示此卡并将其洗入抽牌堆。
//! > （3）若你的乐队技能为“MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上，你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯（MyGO!!!!!）的
//! > （2）技能。
//!

use card_sdk::{ctx, key, CardDef, Msg};

use crate::want_human::SLOT_X;

pub const TOMORI_INNER_SHOUT: CardDef = CardDef {
    id: "CRYCHIC:（灯）内心的呐喊",
    play: Some(tomori_inner_shout),
    can_react: None,
    react: None,
    why_not: None,
};

fn tomori_inner_shout(seat: i32) {
    // 规则书（1）: 「打出此卡时，你可重置一次“想要成为人类”所声明的X（不影响水晶）并获得
    // 两个X差值*120的资金。」 -- C# finds this seat's placed `CardWantHuman` and
    // re-declares its `Mem["x"]`. The slot `want_human_x` is the stand-in for
    // "that card is placed here and its X"; 0 means "no X written down", so (1)
    // is skipped exactly like the C# does without the card.
    let old = ctx::slot(seat, SLOT_X);
    if old > 0 {
        // 规则书（1）: 「重置一次“想要成为人类”所声明的X」 -- C# `H.AskNumber(i, ..., 1, 20, ...)`.
        let x = ctx::ask_number(
            seat,
            &Msg::new(key!("tomori_inner_shout_title")),
            &Msg::new(key!("tomori_inner_shout_ask")).i("n", old as i64),
            1,
            20,
        );
        // 规则书（1）: 「重置一次“想要成为人类”所声明的X（不影响水晶）」 -- only the
        // written X moves; the card's crystal counter is left alone.
        ctx::set_slot(seat, SLOT_X, x);
        ctx::log(
            seat,
            &Msg::new(key!("tomori_inner_shout_changed")).seat("who", seat).i("n", x as i64),
        );
        // 规则书（1）: 「并获得两个X差值*120的资金」 -- C# `H.GainR(i, |new-old| * 120, ...)`.
        let diff = (x - old).abs();
        if diff > 0 {
            ctx::gain(
                seat,
                diff * 120,
                &Msg::new(key!("tomori_inner_shout_gain")).i("n", diff as i64),
            );
        }
    }
    // TODO(规则书)（2）: 「此卡不会被你乐队技能的（2）效果移除，乐队技能的（2）效果发动时展示
    //   此卡并将其洗入抽牌堆。」 -- needs the band-skill (2) surface (C#
    //   `BandCrychic.TransformNow` / the `ICardHook.CardRemoved` guard that skips
    //   this card) and a "show + shuffle into the deck" op
    //   (`H.ShuffleAllIntoDeck`-style for one card).
    // 规则书（3）: 「若你的乐队技能为“MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上」
    // -- C# `H.InBand(i, "MyGO!!!!!")` then `H.PlaceFromPlay(c, i, tile)` at the
    // first owned tile (`H.V(i, "firstTile") - 1` falling back to `list.Min()`).
    if ctx::in_band(seat, "MyGO!!!!!") {
        let owned = ctx::owned_tiles(seat);
        if let Some(&first) = owned.iter().min() {
            let remembered = ctx::slot(seat, "firstTile") - 1;
            let tile = if owned.contains(&remembered) { remembered } else { first };
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card(seat, "CRYCHIC:（灯）内心的呐喊", &Msg::new(key!("tomori_inner_shout_note")));
            ctx::log(seat, &Msg::new(key!("tomori_inner_shout_placed")).seat("who", seat).tile("tile", tile));
            // TODO(规则书)（3）: field-card tile placement -- C# `H.PlaceFromPlay(c, i, tile)`
            //   puts the card on `tile` rather than at the seat; the ABI's
            //   `place_card_at` has no tile parameter yet. The card is placed at
            //   the seat as a stand-in; the computed `tile` is logged above.
        }
    }
    // TODO(规则书)（3）: 「你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯
    //   （MyGO!!!!!）的（2）技能。」 -- needs the Fx.SettleBefore persistent hook
    //   (C# `CardTomoriInnerShout.SettleBefore` -> `Pull`): when another seat's
    //   main-move endpoint is within 5 of this card's tile and the owner has 1
    //   fire, `H.AskYes`, `H.SpendFire`, `H.ForceTeleport(other, tile,
    //   resolve: false)`, then a 1d20 (<=6: `H.TomoriBind` + `TomoriLaterFx`
    //   delayed settles at half pay). `ctx::spend_fire` is now host-bound; the
    //   bind and the delayed-settle queue are still missing.
}