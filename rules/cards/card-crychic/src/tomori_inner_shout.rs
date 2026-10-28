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
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Filter, Msg, On};

use crate::want_human::SLOT_X;

pub const TOMORI_INNER_SHOUT: CardDef = CardDef::new(
    "CRYCHIC:（灯）内心的呐喊",
    &[
        // 规则书（3）: 「你位于此卡前后5格内时可在时机合适时消耗1火罐…」 -- the
        // offer exists only within 5 of this card, and the borrowed (2) skill
        // asks for the mover's 「[移动终点]」 within 5 of you
        // (`dist(pos(actor), pos(owner))`). The fire is left to
        // `fire::spend`, which opens its substitution window even on an empty
        // pool, so no pool gate here (「消耗1火罐」 may be covered by a crystal).
        // TODO(规则书)（使用一次高松灯（MyGO!!!!!）的（2）技能）: (2) 在其他玩家的
        //   任意[移动终点]都可发动 -- the pre narrows it to the main move.
        On::Action(Action::SettleBefore("actor != owner && move.main && card.placed && dist(pos(actor), pos(owner)) <= 5 && dist(pos(owner), card.tile) <= 5", Some(not_latched), pull)),
        // Once-per-move latch drop (the redirect re-raises `settleBefore` at the
        // destination; see `pull`).
        On::Action(Action::MoveResolved("", None, clear_latch)),
        // 高松灯（MyGO!!!!!）(2): 「在此技能影响下的支付价格变为原本的一半」 -- the
        // mover's payments on the pulled settle.
        On::Filter(Filter::PaySplitMul("", Some(half), on_pay)),
        // 规则书（2）: 「乐队技能的（2）效果发动时展示此卡并将其洗入抽牌堆」 lives in
        // the band skill's (2) body (`skill-groups/src/crychic.rs`), not here: the
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
    // re-declares its `Mem["x"]`. The player-scoped slot `want_human_x` is the
    // stand-in for that card's X, but it outlives the instance (nothing clears
    // it when the card leaves), so the live-instance check below is what stands
    // in for "the card is placed"; 0 means "no X was ever written down".
    // TODO(规则书)（你可重置一次“想要成为人类”所声明的X）: X belongs to the placed
    //   card instance (C# `Mem`) -- we keep it in a player slot, so it survives
    //   the card and two copies share it.
    let old = ctx::slot(player_id, SLOT_X);
    let mut reset = old > 0 && want_human_placed(player_id);
    if reset {
        // 规则书（1）: 「你可重置一次」 -- optional: ask first, then the new X
        // (`H.AskNumber(i, ..., 1, 20, ...)`).
        reset = ctx::ask_yes(
            player_id,
            &Msg::new(key!("tomori_inner_shout_title")),
            &Msg::new(key!("tomori_inner_shout_may")).card("card", "CRYCHIC:想要成为人类"),
        )?;
    }
    if reset {
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
        // TODO(规则书)（获得两个X差值*120的资金）: 「两个」读作 ×2 (OPEN-两个X差值) --
        //   现按 |ΔX| * 120（1×）支付。
        let diff = (x - old).abs();
        if diff > 0 {
            ctx::gain(
                player_id,
                diff * 120,
                &Msg::new(key!("tomori_inner_shout_gain")).i("n", diff as i64),
            )?;
        }
    }
    // 规则书（2）: 「此卡不会被你乐队技能的（2）效果移除」 -- see the band
    // skill's (2) sweep, which spares this card.
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
    // 「你的乐队技能为“MyGO!!!!!”」 is about the band skill held, not the persona
    // group: the only player of this card is 高松灯（CRYCHIC）, whose persona group
    // stays CRYCHIC even after the band (2) transform attaches a MyGO!!!!! skill.
    if ctx::group_skills(player_id)
        .iter()
        .any(|(_, id, _)| id.as_str().starts_with("skill:MyGO!!!!!:"))
    {
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
/// Keyed on this **instance** as well as the mover, so two copies on the field
/// each get their own use (the latch lives on the mover, one integer per key).
fn latch_key() -> alloc::string::String {
    alloc::format!("card.tomoriInnerShout.pulled.{}", ctx::self_uid())
}
/// The mover's payments on the pulled settle are halved (per instance).
fn half_key() -> alloc::string::String {
    alloc::format!("card.tomoriInnerShout.half.{}", ctx::self_uid())
}

fn not_latched(_player_id: i32) -> bool {
    ctx::slot(ctx::trigger::player_id(), &latch_key()) == 0
}

fn clear_latch(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        ctx::set_slot(mover, &latch_key(), 0);
        ctx::set_slot(mover, &half_key(), 0);
    }
    Ok(())
}

fn half(_player_id: i32) -> bool {
    ctx::is_placed() && ctx::slot(ctx::trigger::player_id(), &half_key()) > 0
}

/// Is a 「想要成为人类」 instance on this player's field? The X slot is
/// player-scoped and never cleared, so on its own it does not prove placement.
fn want_human_placed(player_id: i32) -> bool {
    ctx::field_instances(player_id)
        .iter()
        .any(|(_, c)| c.as_str() == "CRYCHIC:想要成为人类")
}

fn on_pay(_player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    (value + 1) / 2
}

/// （3）: 「你位于此卡前后5格内时可…消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能」.
/// The window is the **controller** ("你") within 5 of this card, plus the
/// borrowed (2) skill's own condition -- the mover's endpoint within 5 of you
/// (entry condition `dist(pos(actor), pos(owner)) <= 5 && dist(pos(owner),
/// card.tile) <= 5`). The body is 诗超绊 (2) with the cost swapped to 1 fire: another player (the
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
    ctx::set_slot(mover, &latch_key(), 1);
    ctx::set_slot(mover, &half_key(), 1);
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_inner_shout_pulled"))
            .player_id("who", mover)
            .tile("tile", card_tile),
    );
    // The sheet separates the two halves with a semicolon, so the half-price on
    // 「在此技能影响下的支付」 is unconditional (the persona body's `DUE` gate
    // is the different, still-open divergence).
    // TODO(规则书)（使用一次高松灯（MyGO!!!!!）的（2）技能）: (2) 的后半也须执行 --
    //   使其投掷 1d20，<=6 则获得一层[停留]并在其下回合结束时[触发结算]。现在
    //   既不投骰、也不加[停留]、也不排延后结算（无该词汇，见 TOMORI-BIND-01）。
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
