//!
//! 规则书（docs/rulebook/cards.json, id `Sumimi:Here the world`）:
//! > Here the world：
//! >
//! > （1）[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。
//! > （2）场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。
//!
//! [反击] onto the field of whoever played two cards this turn.

use bandori_std::ctx::{self, trigger};
use bandori_std::zones::ZoneChangeCause;
use card_sdk::abi::{ActionKind, ChainKind};
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "Sumimi:Here the world";

/// The card sits on another player's field; the draw-on-return goes to the user.
/// A **prop of this instance**, so two copies on one field keep their own user.
const USER_PROP: &str = "here_user";

/// The held card's instance uid (`+1`). The held card is a real attached
/// instance (「放置于此卡上」); the crystals that time the hold sit on **this**
/// host instance (「为其放置3个奇迹水晶」 -- 其 is 此卡), so they die with it.
const HELD: &str = "here_held";

/// The held instance, `-1` when there is none.
fn held_uid() -> i32 {
    ctx::prop(HELD) - 1
}

/// Is `uid` still this copy's held card?
fn holds(uid: i32) -> bool {
    uid >= 0 && ctx::attachment_host(uid) == ctx::self_uid()
}

pub const HERE_THE_WORLD: CardDef = CardDef::new(
    "Sumimi:Here the world",
    &[
        On::Counteract(
            &[ChainKind::TwoCards],
            "actor != owner",
            Some(can_counteract),
            counteract).counter_reason(key!("here_the_world_when")),
        // TODO(规则书)（当有人同一回合内打出两张卡时）: 打出的反击也是「打出」的一张 -- `turn.cards_played` 只在
        // `play_from_hand` 里加，而反击宣言走 `hand_counteractions`/`declare_one`，一对里含反击就永远凑不到两张。
        // The hold gate is re-checked in the body against **this** instance:
        // `card.counter(…)` in a `condition` resolves by card id, which two copies on one
        // field would share.
        On::Action(Action::Drew("actor == owner && card.placed", None, drew)),
        On::Action(Action::TurnStart("actor == owner && card.placed", None, turn_start)),
        // 规则书（2）: 「那名玩家将那张卡加入手牌」 -- the host leaving before the
        // crystals run out still returns the held card. Its own leave also sets
        // the destination (Q49).
        On::Action(Action::ZoneLeft("card.placed", None, on_leave)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name(r#"Here the world"#)
        .raw_name(r#"Here the world"#)
        .tags(&[r#"反击"#])
        .text(CardText::items(&[
            it(r#"[反击] 当有人同一回合内打出两张卡时，将此卡放置于对方场上。"#),
            it(r#"场上有此卡的玩家下次抽卡时，将那张卡背面朝上放置于此卡上并为其放置3个奇迹水晶，那名玩家的每个回合开始时移除一个，当奇迹水晶数为0时，那名玩家将那张卡加入手牌，并使此卡使用者抽一张卡。"#),
        ]))
        // sheet cell L7
        .fields(&[("group", r#"Sumimi"#), ("owner", "")]));

/// Residual the condition cannot express: the actor is still in the game
/// (`actor != owner` is the `condition`).
fn can_counteract(player_id: i32) -> bool {
    let them = trigger::player_id();
    let _ = player_id;
    !ctx::player_out(them)
}

fn counteract(player_id: i32) -> card_sdk::Asked {
    let them = trigger::player_id();
    ctx::set_dest(ctx::Dest::Field, ZoneChangeCause::Effect);
    ctx::place_card(them, ID, &Msg::new(key!("here_the_world_note")));
    // Written on the instance `place_card` just bound (not on the player).
    ctx::set_prop(USER_PROP, player_id + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("here_the_world_placed"))
            .player_id("who", them)
            .player_id("by", player_id),
    );
    Ok(())
}

fn drew(player_id: i32) -> card_sdk::Asked {
    // Only this instance's own hold latches (`card.placed` is name-keyed).
    if ctx::prop(HELD) > 0 {
        return Ok(());
    }
    let got = trigger::value();
    if got <= 0 {
        return Ok(());
    }
    let cards = trigger::cards();
    let Some(&id) = cards.first() else {
        return Ok(());
    };
    // 规则书（2）: 「将那张卡背面朝上放置于此卡上」 -- a real attached
    // instance, face-down, taken out of the hand.
    let held = ctx::attach_card(ctx::self_uid(), id.as_str(), ZoneChangeCause::Effect, true);
    if held < 0 {
        return Ok(());
    }
    ctx::set_prop(HELD, held + 1);
    // 规则书（2）: 「那名玩家的每个回合开始时移除一个」 -- 3 own turn starts
    // before the return.
    ctx::set_turns_left(3);
    // 规则书（2）: 「并为其放置3个奇迹水晶」 -- the hold's own timer, on this
    // host instance.
    ctx::add_counter_at(ctx::self_uid(), bandori_std::counter::CRYSTALS, 3, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("here_the_world_held")).player_id("who", player_id),
    );
    Ok(())
}

fn turn_start(player_id: i32) -> card_sdk::Asked {
    // Only this instance's own hold ticks (`card.placed` is name-keyed, so two
    // copies would read each other's latch).
    // The latch is the prop, not the attachment: the commit re-run starts from
    // the world the learn pass left, where a completed return is already
    // detached.
    let held = held_uid();
    if held < 0 {
        return Ok(());
    }
    // 「那名玩家的每个回合开始时移除一个」 -- the tick, on the host instance.
    ctx::dec_turns_left();
    let left = ctx::add_counter_at(ctx::self_uid(), bandori_std::counter::CRYSTALS, -1, 0);
    if left > 0 {
        return Ok(());
    }
    let user = ctx::prop(USER_PROP) - 1;
    // 规则书（2）: 「那名玩家将那张卡加入手牌」 -- the held card returns as a
    // real hand card.
    ctx::detach_card(held, ctx::Dest::Hand, ZoneChangeCause::Effect);
    ctx::set_prop(HELD, 0);
    ctx::log(
        player_id,
        &Msg::new(key!("here_the_world_returned")).player_id("who", player_id),
    );
    // 规则书（2）: the hold is done -- the host leaves (the book does not say
    // when; Q49).
    ctx::set_dest(ctx::Dest::Graveyard, ZoneChangeCause::Effect);
    ctx::set_prop(USER_PROP, 0);
    // 规则书（2）: 「并使此卡使用者抽一张卡」.
    if user >= 0 && !ctx::player_out(user) {
        ctx::draw(user, 1)?;
    }
    Ok(())
}

/// The host leaves: return the held card (「那名玩家将那张卡加入手牌」) and file
/// the host to the Graveyard -- unless the hold already completed.
fn on_leave(player_id: i32) -> card_sdk::Asked {
    // Instance kind: another copy sees this raise too.
    if trigger::instance() != ctx::self_uid() {
        return Ok(());
    }
    let held = held_uid();
    if held >= 0 {
        // 规则书（2）: 「那名玩家将那张卡加入手牌」 (a no-op when it is already
        // back in hand).
        ctx::detach_card(held, ctx::Dest::Hand, ZoneChangeCause::Effect);
        ctx::set_prop(HELD, 0);
        ctx::log(
            player_id,
            &Msg::new(key!("here_the_world_returned")).player_id("who", player_id),
        );
    }
    // 规则书（2）: the host sits in its owner's discard.
    bandori_std::zones::set_leave_destination(ctx::Dest::Graveyard);
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn counteract_kani(player_id: i32) -> card_sdk::Asked {
    counteract(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn drew_kani(player_id: i32) -> card_sdk::Asked {
    drew(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_leave_kani(player_id: i32) -> card_sdk::Asked {
    on_leave(player_id)
}
