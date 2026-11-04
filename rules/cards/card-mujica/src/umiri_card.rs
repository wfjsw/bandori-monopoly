//!
//! 规则书（docs/rulebook/cards.json, id `Mujica:（海铃）`）:
//! > （海铃）
//! >
//! > （1）将此卡放置于在此卡使用者下一名行动的玩家场上，轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。
//! >
//! > （2）使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。
//!
//! > （3）当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。
//!
//! placed card that walks one player per turn and takes band skill cards.

use bandori_std::ctx::{self, trigger};
use card_sdk::abi::ActionKind;
use card_sdk::{it, CardText};
use card_sdk::{key, Action, CardDef, Msg, On};

const ID: &str = "Mujica:（海铃）";

/// The player that played the card (it may sit on another player's field).
/// An on-card counter of the instance, stored 1-based so 0 reads as "unset".
const USER_KEY: &str = "umiri_user";

pub const UMIRI_CARD: CardDef = CardDef::new(
    "Mujica:（海铃）",
    &[
        On::Play("", Some(cant_play), umiri_card),
        // 规则书（1）: 「轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上」
        // -- the hop at the user's turn start.
        On::Action(Action::TurnStart("card.placed && card.counter('umiri_user') - 1 == actor", None, turn_start)),
        // 规则书（3）: 「当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。」
        // -- the discard + draw when the card is back at the user's field

        On::Action(Action::TurnEnd("card.placed && card.counter('umiri_user') - 1 == actor && owner == card.counter('umiri_user') - 1", None, turn_end)),
    ],
).meta(&card_sdk::meta::CardMeta::new()
        .name("")
        .raw_name(r#"（海铃）"#)
        .tags(&[])
        .text(CardText::items(&[
            it(r#"将此卡放置于在此卡使用者下一名行动的玩家场上，轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。 "#),
            it(r#"使用者打出此卡时以及使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡（相同乐队技能卡的效果不可叠加），但不视为那个乐队的角色。"#),
            it(r#"当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。"#),
        ]))
        // sheet cell J16
        .fields(&[("group", r#"Ave Mujica"#), ("owner", r#"八幡海铃"#)]));

fn cant_play(player_id: i32) -> Option<Msg> {
    // TODO(规则书)（将此卡放置于在此卡使用者下一名行动的玩家场上）: 规则书（1）: 「将此卡放置于在此卡使用者下一名行动的玩家场上」 -- the
    // target is the implicit precondition of this gate.
    if ctx::others(player_id).is_empty() {
        return Some(Msg::new(key!("x_no_others")));
    }
    None // playable
}

fn umiri_card(player_id: i32) -> card_sdk::Asked {
    // `place_card` always places at the given player; the "next actor" is the next
    // live player in turn order (`NextOf` walks `player_id + 1 ..` skipping outed
    // players).
    let next = next_of(player_id);
    if next == player_id {
        return Ok(());
    }
    // 规则书（1）: 「将此卡放置于在此卡使用者下一名行动的玩家场上」
    ctx::set_dest(ctx::Dest::Field, card_sdk::abi::ZoneChangeCause::Play);
    ctx::place_card(next, ID, &Msg::new(key!("umiri_note")));
    // The user is an on-card counter of the instance just placed (1-based, so 0
    // reads as "unset"), so two copies keep separate users and the value dies
    // with its instance.
    ctx::set_counter(USER_KEY, player_id + 1);
    ctx::log(
        player_id,
        &Msg::new(key!("umiri_placed"))
            .player_id("who", player_id)
            .player_id("holder", next),
    );
    // 「相同乐队技能卡的效果不可叠加」 -- `add_group_skill` refuses an id already attached to the
    // user -- and 「不视为那个乐队的角色」 -- `in_group` reads only the character.
    take_bands(next, player_id);
    Ok(())
}

fn take_bands(holder: i32, user: i32) {
    for (_uid, id, _extra) in ctx::group_skills(holder) {
        // 规则书（2）: 「从卡堆拿取场上有此卡的玩家的所有乐队技能卡」 -- an
        // `extra` attachment is a band skill card on the holder's field too.
        let got = ctx::add_group_skill(user, id.as_str(), true);
        if got >= 0 {
            ctx::log(
                user,
                &Msg::new(key!("umiri_took_band"))
                    .player_id("who", user)
                    .card("card", id.as_str()),
            );
        }
    }
}

/// Runs through the hook dispatch at `turnStart`, so this is a field effect,
/// not a [反击].
fn turn_start(player_id: i32) -> card_sdk::Asked {
    // The `card.placed && card.counter('umiri_user') - 1 == actor` condition owns the gate.
    // `card.counter` answers for the running instance, so two copies read their
    // own users.
    let user = ctx::counter(USER_KEY) - 1;
    // 规则书（1）: 「轮到使用者的回合开始时，将其移动到其所在场的玩家行动序列后一名的玩家场上。」
    let dest = next_of(player_id);
    ctx::unplace_self();
    ctx::place_card(dest, ID, &Msg::new(key!("umiri_note")));
    // Re-set the user on the new instance (`place_card` made it current and
    // dropped the old instance's counters with it).
    ctx::set_counter(USER_KEY, user + 1);
    // 规则书（2）: 「使用者的回合开始时，从卡堆拿取场上有此卡的玩家的所有乐队技能卡」
    // -- `Take()` on whatever field the card now sits on, the user's own included.
    // The same-id refusal makes the return-to-user case a no-op unless the
    // user's own band skill was removed.
    take_bands(dest, user);
    if dest == user {
        // 「（海铃）」回到了 … 的场上：回合结束时置入弃牌堆.
        ctx::log(
            user,
            &Msg::new(key!("umiri_returned")).player_id("who", user),
        );
    } else {
        // 「（海铃）」移到了 … 的场上.
        ctx::log(
            dest,
            &Msg::new(key!("umiri_moved"))
                .player_id("who", dest)
                .player_id("user", user),
        );
    }
    Ok(())
}

/// When the card is back on the user's field at the user's turn end,
/// discard it and draw 1. Runs through the hook dispatch at `turnEnd`, so this
/// is a field effect, not a [反击].
fn turn_end(_player_id: i32) -> card_sdk::Asked {
    // The `card.placed && card.counter('umiri_user') - 1 == actor &&
    // owner == card.counter('umiri_user') - 1` condition owns the gate.
    let user = ctx::counter(USER_KEY) - 1;
    // 规则书（3）: 「当此卡回到使用者场上时，使用者回合结束时将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡。」
    // `unplace(this, "discard", "回到了使用者的场上")` + `DrawR(User, 1, ...)`;
    // `Detach` -> `Drop` of the taken band cards.
    // 「使用者拿取的所有乐队技能卡」 is every **extra** attachment on the user
    // (the 「拿取」 copies `take_bands` made); the user's own band skill is not
    // one of them and stays.
    for (uid, id, extra) in ctx::group_skills(user) {
        if extra == 0 {
            continue;
        }
        // TODO(规则书)（将此卡与使用者拿取的所有乐队技能卡置入弃牌堆）:
        ctx::unplace_at(
            uid,
            ctx::Dest::Graveyard,
            card_sdk::abi::ZoneChangeCause::Effect,
        );
        ctx::log(
            user,
            &Msg::new(key!("umiri_dropped_band"))
                .player_id("who", user)
                .card("card", id.as_str()),
        );
    }
    // 规则书（3）: 「将此卡与使用者拿取的所有乐队技能卡置入弃牌堆，抽一张卡」 -- the
    // discard is filed now, before the draw (a draw can reshuffle the discard).
    ctx::send_to_dest(ctx::Dest::Graveyard, card_sdk::abi::ZoneChangeCause::Effect);
    ctx::set_counter(USER_KEY, 0);
    ctx::draw(user, 1)?;
    ctx::log(user, &Msg::new(key!("umiri_ended")).player_id("who", user));
    Ok(())
}

fn next_of(holder: i32) -> i32 {
    let n = ctx::player_count();
    if n <= 0 {
        return holder;
    }
    for i in 1..=n {
        let s = (holder + i).rem_euclid(n);
        if !ctx::player_out(s) {
            return s;
        }
    }
    holder
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn umiri_card_kani(player_id: i32) -> card_sdk::Asked {
    umiri_card(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_start_kani(player_id: i32) -> card_sdk::Asked {
    turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn turn_end_kani(player_id: i32) -> card_sdk::Asked {
    turn_end(player_id)
}
