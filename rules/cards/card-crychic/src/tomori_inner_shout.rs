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
        //   任意[移动终点]都可发动 -- the condition narrows it to the main move.
        On::Action(Action::SettleBefore("actor != owner && move.is_main_move && card.placed && dist(pos(actor), pos(owner)) <= 5 && dist(pos(owner), card.tile) <= 5", Some(offer_open), pull)),
        // Once-per-move latch drop (the redirect re-raises `settleBefore` at the
        // destination; see `pull`).
        On::Action(Action::MoveResolved("", None, clear_latches)),
        // 高松灯（MyGO!!!!!）(2): 「在此技能影响下的支付价格变为原本的一半」 -- the
        // mover's payments on the pulled settle.
        On::Filter(Filter::PaySplitMul("", Some(half_price_applies), on_pay)),
        // The owed settle's payment is adjudicated after its body returns;
        // drop the half-price and the borrow latch once it is done.
        On::Action(Action::SettleAfter("", None, clear_owed_paying)),
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
    // 两个X差值*120的资金。」 X is a prop of the target card's instance, so the
    // instance is resolved by id and read cross-instance; 0 means "no X was ever
    // written down". The first (lowest-uid) copy stands in when two are placed.
    let target = ctx::field_instances(player_id)
        .into_iter()
        .find(|(_, c)| c.as_str() == "CRYCHIC:想要成为人类")
        .map(|(u, _)| u);
    let old = target.map_or(0, |u| ctx::prop_at(u, SLOT_X));
    let mut reset = old > 0 && target.is_some();
    if reset {
        reset = ctx::ask_yes(
            player_id,
            &Msg::new(key!("tomori_inner_shout_title")),
            &Msg::new(key!("tomori_inner_shout_may")).card("card", "CRYCHIC:想要成为人类"),
        )?;
    }
    if reset {
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
        ctx::set_prop_at(target.unwrap(), SLOT_X, x);
        ctx::log(
            player_id,
            &Msg::new(key!("tomori_inner_shout_changed"))
                .player_id("who", player_id)
                .i("n", x as i64),
        );
        // TODO(规则书)（获得两个X差值*120的资金）: 「两个」读作 ×2 (OPEN-两个X差值) -- 现按 |ΔX| * 120（1×）支付。
        let diff = (x - old).abs();
        if diff > 0 {
            ctx::gain(
                player_id,
                diff * 120,
                &Msg::new(key!("tomori_inner_shout_gain")).i("n", diff as i64),
            )?;
        }
    }
    // 规则书（2）: 「此卡不会被你乐队技能的（2）效果移除」 -- see the band skill's (2) sweep, which spares
    // this card.
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
    Ok(())
}

/// 「高松灯（MyGO!!!!!）的（2）技能」 -- the *base* 高松灯's persona skill. The
/// controller is 高松灯（CRYCHIC）, whose own persona skill is 跌跌撞撞, so the
/// base skill is named literally; nothing places it for a variant, and the body
/// runs with its instance unbound (`-1`), which only the half filter would read.
const POEM: &str = "skill:高松灯:诗超绊";
/// The borrowed skill's own once-per-move key -- must match `tomori.rs`. A
/// borrowed use claims the same latch the skill's own offer reads, so the two
/// cannot both run on one landing.
const POEM_USED_THIS_MOVE: &str = "skill.tomoriPoem.usedThisMove";
/// The borrowed skill's half-price latch -- must match `tomori.rs`. It stays up
/// from the pull until the owed settle pays (the move-resolved drop is
/// conditional on nothing being owed).
const POEM_HALF_PRICE: &str = "skill.tomoriPoem.halfPrice";
/// Set while the borrow runs so the skill's own filter stands down: the skill
/// instance is not on a variant's field, so its `Filter` hook does not dispatch
/// at all -- the one filter here is the whole bend.
const POEM_BORROWED: &str = "skill.tomoriPoem.borrowed";
/// The skill's owed-settle latch (must match `tomori.rs`); the skill is
/// unplaced here, so this card clears the half-price after the owed settle.
const POEM_OWED_SETTLE: &str = "skill.tomoriPoem.owedSettle";

/// Once per move: the teleport re-raises `settleBefore` at the destination.
/// Keyed on this **instance** as well as the mover, so two copies on the field
/// each get their own use (the latch lives on the mover, one integer per key).
fn used_this_move_key() -> alloc::string::String {
    alloc::format!("card.tomoriInnerShout.pulled.{}", ctx::self_uid())
}

/// 「在此技能影响下的支付价格变为原本的一半」 -- only the payments of the
/// mover this copy pulled (the guard argument is this card's owner; the payer
/// is `trigger.player_id`). A placed poem answers every settle itself; stand
/// down then so the two filters do not compose (the forced settle has
/// `POEM_BORROWED` for the same reason).
fn half_price_applies(player_id: i32) -> bool {
    if !ctx::is_placed()
        || ctx::trigger::settle_tile() < 0
        || ctx::slot(ctx::trigger::player_id(), POEM_HALF_PRICE) == 0
    {
        return false;
    }
    // This copy is the active bend while its borrow runs; afterwards only when
    // the poem is not on the field to answer for itself -- and at the owed
    // settle the poem's own filter is skipped anyway: its body is the resolving
    // one, and a money hook cannot re-trigger on its own payment.
    ctx::slot(ctx::trigger::player_id(), POEM_BORROWED) > 0
        || ctx::trigger::is_settle_redirect()
        || !ctx::field_instances(player_id)
            .iter()
            .any(|(_, c)| c.as_str() == POEM)
}

fn on_pay(_player_id: i32, value: i32) -> i32 {
    if value <= 0 {
        return value;
    }
    (value + 1) / 2
}

/// `card.placed && dist(...)` is the condition; this is the residual: 「若你的
/// 乐队技能为“MyGO!!!!!”」 (the sentence gates both the placement and the use)
/// and neither latch is set -- not offered twice on one move, and not offered at
/// all when the skill's own (2) already claimed the landing (it dispatches
/// first; §7.3). The band check is the same scan the placement half uses.
fn offer_open(player_id: i32) -> bool {
    if !ctx::group_skills(player_id)
        .iter()
        .any(|(_, id, _)| id.as_str().starts_with("skill:MyGO!!!!!:"))
    {
        return false;
    }
    let mover = ctx::trigger::player_id();
    ctx::slot(mover, &used_this_move_key()) == 0 && ctx::slot(mover, POEM_USED_THIS_MOVE) == 0
}

/// The owed settle is a **card-forced** settle (`settle_redirect`, the skill's
/// `settle_tile_at`); the mover's own settle is the walk's and is not. Clear the
/// half-price only at the owed one. `settleAfter` fires for every settle, so
/// this re-checks inside the body.
fn clear_owed_paying(_player_id: i32) -> card_sdk::Asked {
    if !ctx::trigger::is_settle_redirect() {
        return Ok(());
    }
    let mover = ctx::trigger::player_id();
    if mover >= 0 && ctx::slot(mover, POEM_HALF_PRICE) > 0 {
        ctx::set_slot(mover, POEM_HALF_PRICE, 0);
        ctx::set_slot(mover, POEM_BORROWED, 0);
    }
    Ok(())
}

fn clear_latches(_player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if mover >= 0 {
        ctx::set_slot(mover, &used_this_move_key(), 0);
        ctx::set_slot(mover, POEM_USED_THIS_MOVE, 0);
        // An owed settle pays later and must stay halved.
        if ctx::slot(mover, POEM_OWED_SETTLE) == 0 {
            ctx::set_slot(mover, POEM_HALF_PRICE, 0);
            ctx::set_slot(mover, POEM_BORROWED, 0);
        }
    }
    Ok(())
}

/// （3）: 「你位于此卡前后5格内时可…消耗1火罐使用一次高松灯（MyGO!!!!!）的（2）技能」.
/// The window is the **controller** ("你") within 5 of this card, plus the
/// borrowed (2) skill's own condition -- the mover's endpoint within 5 of you
/// (entry condition `dist(pos(actor), pos(owner)) <= 5 && dist(pos(owner),
/// card.tile) <= 5`). The cost is one pot, not the skill's four: the body is
/// entered paid (`invoke_entry_paid`) on the declared entry `2`, and the body
/// owns the teleport, the roll, the [停留], the owed settle and the half-price.
fn pull(player_id: i32) -> card_sdk::Asked {
    let mover = ctx::trigger::player_id();
    if ctx::player_pos(player_id) < 0 {
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
    // Claim the card's own once-per-move before the body teleports, so the
    // redirect re-raise does not offer again. The half-price rides on this
    // copy; `POEM_BORROWED` makes the skill's own filter stand down.
    ctx::set_slot(mover, &used_this_move_key(), 1);
    ctx::set_slot(mover, POEM_HALF_PRICE, 1);
    ctx::set_slot(mover, POEM_BORROWED, 1);
    // 「使用一次高松灯（MyGO!!!!!）的（2）技能」 -- the body, paid: one pot,
    // not four. The body owns the teleport, the roll and the delay.
    ctx::invoke_entry_paid(player_id, POEM, "2")?;
    ctx::log(
        player_id,
        &Msg::new(key!("tomori_inner_shout_pulled"))
            .player_id("who", mover)
            .tile("tile", ctx::player_pos(player_id)),
    );
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
pub fn clear_latches_kani(player_id: i32) -> card_sdk::Asked {
    clear_latches(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn offer_open_kani(player_id: i32) -> bool {
    offer_open(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn half_price_applies_kani(player_id: i32) -> bool {
    half_price_applies(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn tomori_inner_shout_kani(player_id: i32) -> card_sdk::Asked {
    tomori_inner_shout(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pay_kani(player_id: i32, value: i32) -> i32 {
    on_pay(player_id, value)
}

#[cfg(kani)]
#[inline(always)]
pub fn clear_owed_paying_kani(player_id: i32) -> card_sdk::Asked {
    clear_owed_paying(player_id)
}
