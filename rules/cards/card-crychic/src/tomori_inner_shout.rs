//! `CRYCHIC:（灯）内心的呐喊` -- C# `CardTomoriInnerShout` (MatchHost.cs:3217-3294):
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
//! re-declare 「想要成为人类」's X for money, then a MyGO placement + pull skill.

use card_sdk::{ctx, key, CardDef, Msg, On};

use crate::want_human::SLOT_X;

pub const TOMORI_INNER_SHOUT: CardDef = CardDef::new(
    "CRYCHIC:（灯）内心的呐喊",
    &[
        On::Hook(&[card_sdk::abi::HookKind::SettleBefore], mine, pull),
        On::Hook(
            &[card_sdk::abi::HookKind::TurnEndBefore],
            empty_piles,
            shuffle_in,
        ),
        On::Play(None, tomori_inner_shout),
    ],
);

fn tomori_inner_shout(player_id: i32) -> card_sdk::Asked {
    // 规则书（1）: 「打出此卡时，你可重置一次“想要成为人类”所声明的X（不影响水晶）并获得
    // 两个X差值*120的资金。」 -- C# finds this player's placed `CardWantHuman` and
    // re-declares its `Mem["x"]`. The slot `want_human_x` is the stand-in for
    // "that card is placed here and its X"; 0 means "no X written down", so (1)
    // is skipped exactly like the C# does without the card.
    let old = ctx::slot(player_id, SLOT_X);
    if old > 0 {
        // 规则书（1）: 「重置一次“想要成为人类”所声明的X」 -- C# `H.AskNumber(i, ..., 1, 20, ...)`.
        let x = ctx::ask_number(
            player_id,
            &Msg::new(key!("tomori_inner_shout_title")),
            &Msg::new(key!("tomori_inner_shout_ask")).i("n", old as i64),
            1,
            20,
        )?;
        // 规则书（1）: 「重置一次“想要成为人类”所声明的X（不影响水晶）」 -- only the
        // written X moves; the card's crystal counter is left alone.
        ctx::set_slot(player_id, SLOT_X, x);
        ctx::log(
            player_id,
            &Msg::new(key!("tomori_inner_shout_changed"))
                .player_id("who", player_id)
                .i("n", x as i64),
        );
        // 规则书（1）: 「并获得两个X差值*120的资金」 -- C# `H.GainR(i, |new-old| * 120, ...)`.
        let diff = (x - old).abs();
        if diff > 0 {
            ctx::gain(
                player_id,
                diff * 120,
                &Msg::new(key!("tomori_inner_shout_gain")).i("n", diff as i64),
            );
        }
    }
    // （2）「此卡不会被你乐队技能的（2）效果移除」 -- see `shuffle_in` and the
    // band skill's sweep, which spares this card.
    // 规则书（3）: 「若你的乐队技能为“MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上」
    // -- C# `H.InBand(i, "MyGO!!!!!")` then `H.PlaceFromPlay(c, i, tile)` at the
    // first owned tile (`H.V(i, "firstTile") - 1` falling back to `list.Min()`).
    if ctx::in_band(player_id, "MyGO!!!!!") {
        let owned = ctx::owned_tiles(player_id);
        if let Some(&first) = owned.iter().min() {
            let remembered = ctx::slot(player_id, "firstTile") - 1;
            let tile = if owned.contains(&remembered) {
                remembered
            } else {
                first
            };
            ctx::set_dest(ctx::Dest::Field);
            ctx::place_card_on(
                player_id,
                tile,
                "CRYCHIC:（灯）内心的呐喊",
                &Msg::new(key!("tomori_inner_shout_note")),
            );
            ctx::log(
                player_id,
                &Msg::new(key!("tomori_inner_shout_placed"))
                    .player_id("who", player_id)
                    .tile("tile", tile),
            );
        }
    }
    // （3）「你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯
    // （MyGO!!!!!）的（2）技能」 -- `pull` below is the Fx.SettleBefore half.
    Ok(())
}

/// （3）: when another player's main-move endpoint is within 5 of this card's
/// tile and the owner has 1 fire, pull them onto the card's tile with no settle.
fn pull(player_id: i32) -> card_sdk::Asked {
    if !ctx::is_placed() {
        return Ok(());
    }
    let mover = ctx::trigger::player_id();
    if mover == player_id || !ctx::trigger::move_is_main() {
        return Ok(());
    }
    let Some(card_tile) = ctx::self_tile() else {
        return Ok(());
    };
    if card_tile < 0 || ctx::dist(ctx::player_pos(mover), card_tile) > 5 {
        return Ok(());
    }
    if ctx::fire(player_id) < 1 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_inner_shout_title")),
        &Msg::new(key!("tomori_inner_shout_pull")).player_id("who", mover),
    )? {
        return Ok(());
    }
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("tomori_inner_shout_spend"))) {
        return Ok(());
    }
    // `H.ForceTeleport(other, tile, resolve: false)` -- a plain position write,
    // so nothing settles on the way in.
    ctx::teleport_to(mover, card_tile);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_inner_shout_pulled"))
            .player_id("who", mover)
            .tile("tile", card_tile),
    );
    // TODO(规则书)（3）: the second half -- a 1d20 (<=6: `H.TomoriBind` +
    //   `TomoriLaterFx` delayed settles at half pay). The bind and the
    //   delayed-settle queue have no vocabulary: nothing schedules "this player
    //   settles these tiles later at half pay".
    Ok(())
}

/// The band skill's (2) fires when both piles are empty; this card shuffles
/// itself in at the same moment.
fn empty_piles(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
        && ctx::deck_count(player_id) == 0
        && ctx::discard_size(player_id) == 0
}

fn shuffle_in(player_id: i32) -> card_sdk::Asked {
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_inner_shout_shown")).card("card", "CRYCHIC:（灯）内心的呐喊"),
    );
    ctx::shuffle_into_deck(player_id, true, true);
    Ok(())
}

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() != player_id
}
