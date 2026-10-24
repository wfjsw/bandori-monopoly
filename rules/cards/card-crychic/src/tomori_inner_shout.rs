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

use bandori_std::ctx;
use card_sdk::{key, Action, CardDef, Filter, Msg, On};
use card_sdk::{CardText, it};

use crate::want_human::SLOT_X;

pub const TOMORI_INNER_SHOUT: CardDef = CardDef::new(
    "CRYCHIC:（灯）内心的呐喊",
    &[
        // 规则书（3）: 「你位于此卡前后5格内时可在时机合适时消耗1火罐…」 -- the
        // offer exists only within 5 of this card and with a fire to spend.
        // Geometry + fire are CEL (`dist` / `card.tile` / `pos` / `fire`).
        On::Action(Action::SettleBefore("actor != owner && move.main && card.placed && charge(owner) >= 1 && dist(pos(owner), card.tile) <= 5", Some(not_latched), pull)),
        // Once-per-move latch drop (the redirect re-raises `settleBefore` at the
        // destination; see `pull`).
        On::Action(Action::MoveResolved("", None, clear_latch)),
        // 高松灯（MyGO!!!!!）(2): 「在此技能影响下的支付价格变为原本的一半」 -- the
        // mover's payments on the pulled settle.
        On::Filter(Filter::PaySplitMul("", Some(half), on_pay)),
        // （2）「乐队技能的（2）效果发动时展示此卡并将其洗入抽牌堆」 lives in the
        // band skill's (2) body (`skill-groups/src/crychic.rs`), not here: the
        // band effect is what moves the card, and by a separate empty-piles hook
        // the deck has already been refilled so the proxy never fires.
        On::Play("", None, tomori_inner_shout),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"内心的呐喊"#)
        .raw_name(r#"（灯）内心的呐喊"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"打出此卡时，你可重置一次“想要成为人类”所声明的X（不影响水晶）并获得两个X差值*120的资金。"#),
            it(r#"此卡不会被你乐队技能的（2）效果移除，乐队技能的（2）效果发动时展示此卡并将其洗入抽牌堆。"#),
            it(r#"若你的乐队技能为“MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上，你位于此卡前后5格内时可在时机合适时消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能。"#),
        ]))
        // sheet cell M13
        .fields(&[("group", r#"CRYCHIC"#), ("owner", r#"高松灯（CRYCHIC）"#)]));

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
            &Msg::new(key!("tomori_inner_shout_ask"))
                .i("n", old as i64)
                .card("card", "CRYCHIC:想要成为人类"),
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
            )?;
        }
    }
    // （2）「此卡不会被你乐队技能的（2）效果移除」 -- see the band skill's (2)
    // band skill's sweep, which spares this card.
    // 规则书（3）: 「若你的乐队技能为“MyGO!!!!!”，打出此卡时将其放置于你最早拥有的格子上」
    // -- C# `H.InBand(i, "MyGO!!!!!")` then `H.PlaceFromPlay(c, i, tile)` at the
    // first owned tile (`H.V(i, "firstTile") - 1` falling back to `list.Min()`).
    //
    // TODO(规则书)（3）「最早拥有的格子」 (LOY-TOMORI-03) -- **class B**. The engine
    //   has no tile-acquisition order: `owners` is tile→owner only,
    //   `ctx::owned_tiles` walks board indices (so `.min()` is the lowest
    //   board index, not the earliest acquisition), and `ctx::slot` is a single
    //   integer per key -- there is no ordered accumulator to push onto. A
    //   correct 「最早拥有」 needs a per-player list written by a deed-acquisition
    //   hook (`bought` / `buyAfter`), running from game start so tiles taken
    //   before this card was played are already ordered. Until that lands, the
    //   lowest board index stands in for "earliest owned", and `firstTile` is
    //   honoured if anything ever writes it.
    if ctx::in_group(player_id, "MyGO!!!!!") {
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

/// Once per move: the teleport re-raises `settleBefore` at the destination.
const LATCH: &str = "card.tomoriInnerShout.pulled";
/// The mover's payments on the pulled settle are halved.
const HALF: &str = "card.tomoriInnerShout.half";

fn not_latched(_player_id: i32) -> bool {
    ctx::slot(ctx::trigger::player_id(), LATCH) == 0
}

fn clear_latch(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        ctx::set_slot(mover, LATCH, 0);
        ctx::set_slot(mover, HALF, 0);
    }
    Ok(())
}

fn half(_player_id: i32) -> bool {
    ctx::is_placed() && ctx::slot(ctx::trigger::player_id(), HALF) > 0
}

fn on_pay(_player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    (value + 1) / 2
}

/// （3）: 「你位于此卡前后5格内时可…消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能」.
/// The window is the **controller** ("你") within 5 of this card, not the
/// mover's endpoint (entry condition `dist(pos(owner), card.tile) <= 5`). The
/// body is 诗超绊 (2) with the cost swapped to 1 fire: another player (the
/// mover, before their settle) is teleported to 「自己所在的格子」 -- the
/// controller's own tile -- and the redirected settle runs there (`teleport_to`
/// inside `SettleBefore`: `after_walk` settles once at the destination), with
/// the mover's payments halved on it.
fn pull(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    let card_tile = ctx::player_pos(player_id);
    if card_tile < 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("tomori_inner_shout_title")),
        &Msg::new(key!("tomori_inner_shout_pull")).player_id("who", mover),
    )? {
        return Ok(());
    }
    if !bandori_std::fire::spend(player_id, 1, &Msg::new(key!("tomori_inner_shout_spend")))? {
        return Ok(());
    }
    ctx::teleport_to(mover, card_tile);
    ctx::set_slot(mover, LATCH, 1);
    ctx::set_slot(mover, HALF, 1);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_inner_shout_pulled"))
            .player_id("who", mover)
            .tile("tile", card_tile),
    );
    // TODO(规则书)（3）: half-pay is applied unconditionally to the pulled
    //   settle; 诗超绊 (2) itself only halves after a 1d20 <= 6 (its `DUE`
    //   state), so the exact scope of 「在此技能影响下」 is unsettled (the
    //   persona body may itself diverge from the book).
    // TODO(规则书)（3）: the second half -- a 1d20 (<=6: `H.TomoriBind` +
    //   `TomoriLaterFx` delayed settles at half pay). The bind and the
    //   delayed-settle queue have no vocabulary: nothing schedules "this player
    //   settles these tiles later at half pay".
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn pull_kani(player_id: i32) -> card_sdk::Asked {
    pull(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_latch_kani(player_id: i32) -> card_sdk::Asked {
    clear_latch(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32, value: i32) -> i32 {
    on_pay(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn tomori_inner_shout_kani(player_id: i32) -> card_sdk::Asked {
    tomori_inner_shout(player_id)
}
