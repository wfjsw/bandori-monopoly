//! `skill:宇田川亚子:对帅气的憧憬`
//!
//! 规则书（skill sheet, 宇田川亚子）:
//! > （1）每次经过CiRCLE时，若选择抽卡，则可以看抽牌堆顶至多两张卡，选择其中1张
//! > 加入手牌，剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己场上。
//! > （2）经过CiRCLE领取奖励时，若自己场上存在背面朝上的卡，则本次领取奖励被
//! > 替换为将自己场上背面朝上的卡拿取至手中。
//! > （3）若自己场上背面朝上放置的卡为[反击]卡，可在符合条件时将其直接打出。
//!
//! 「背面朝上放置在自己场上」 is a face-down field card, which is what
//! `card_face_down` reads and `set_dest(ctx::Dest::Field)` + `place_card` writes.
//! (2) and (3) are both keyed on that flag.
//!
//! (1) 「若选择抽卡」 is the CiRCLE reward being the card, which is the
//! `passRewardAffected` moment -- the reward has been chosen, and this skill
//! rewrites the draw it implies.
//!
//! (3) is not a body: a face-down card produces no effect of its own, so the
//! skill only grants the seat that ability (`COUNTERACT_FACE_DOWN`); the
//! engine offers the seat's face-down cards in a window their own condition
//! opens, and reveals the one declared.

use bandori_std::ctx;
use bandori_std::kind::Actions;
use card_sdk::abi::{CardPile, REWARD_CARD};
use card_sdk::ctx::NameId;
use card_sdk::{key, CardDef, Msg, On};

pub const AKO: CardDef = CardDef::new(
    "skill:宇田川亚子:对帅气的憧憬",
    &[On::Action(Actions::PassRewardAffected(
        card_sdk::pre::MINE,
        None,
        on_circle,
    ))],
)
.props(&[
    (card_sdk::abi::prop::EST_COST, 500),
    // （3）「可在符合条件时将其直接打出」 -- the seat may play a face-down
    // field card in its own counteraction window.
    (card_sdk::abi::prop::COUNTERACT_FACE_DOWN, 1),
]);

/// Face-down cards this player has on the field.
fn face_down(player_id: i32) -> alloc::vec::Vec<(i32, NameId)> {
    ctx::field_instances(player_id)
        .into_iter()
        .filter(|(uid, _)| ctx::is_face_down_at(*uid))
        .collect()
}

/// （1） and （2） -- the CiRCLE reward.
fn on_circle(player_id: i32) -> card_sdk::Asked {
    // （2）「若自己场上存在背面朝上的卡，则本次领取奖励被替换为将自己场上
    // 背面朝上的卡拿取至手中」 -- the replacement takes priority: it is what the
    // reward *is* once there is a face-down card to take, whichever half was
    // picked.
    let down = face_down(player_id);
    if !down.is_empty() {
        for (uid, c) in down {
            ctx::unplace_at(uid);
            ctx::add_to_dest(player_id, c.as_str(), ctx::Dest::Hand);
            ctx::log(
                player_id,
                &Msg::new(key!("ako_taken")).card("card", c.as_str()),
            );
        }
        // 「本次领取奖励被替换为」 -- the tile must not also pay the reward.
        ctx::trigger::set_cancelled();
        return Ok(());
    }
    // （1）「若选择抽卡，则可以看抽牌堆顶至多两张卡」 -- the money half is not
    // the draw and is left alone.
    if ctx::trigger::value() != REWARD_CARD {
        return Ok(());
    }
    let deck = ctx::cards_in(player_id, CardPile::Deck);
    if deck.is_empty() {
        return Ok(());
    }
    // 「至多两张」 -- the player chooses how many of the top to look at; with a
    // single card there is nothing to choose.
    let cap = deck.len().min(2);
    let n = if cap > 1 {
        let pick = ctx::ask_pick(
            player_id,
            &Msg::new(key!("ako_count_title")),
            &Msg::new(key!("ako_count_ask")),
            &[Msg::new(key!("ako_count_1")), Msg::new(key!("ako_count_2"))],
        )?;
        if pick == 0 {
            1
        } else {
            2
        }
    } else {
        cap
    };
    let look: alloc::vec::Vec<NameId> = deck[..n].to_vec();
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("ako_title")),
        &Msg::new(key!("ako_ask")),
        &look
            .iter()
            .map(|c| Msg::new(key!("ako_option")).card("card", c.as_str()))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(&keep) = look.get(pick) else {
        return Ok(());
    };
    if !ctx::take_card(player_id, keep.as_str(), CardPile::Deck) {
        return Ok(());
    }
    ctx::add_to_dest(player_id, keep.as_str(), ctx::Dest::Hand);
    // 「选择其中1张加入手牌」 *is* the reward now -- cancel the tile's own draw
    // so the peek replaces it instead of stacking on it.
    ctx::trigger::set_cancelled();
    // 「剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己场上」
    for c in look.iter().filter(|c| **c != keep) {
        // Take the card out of the pile first: `place_card` only creates a
        // field instance, so without this the deck copy would stay behind and
        // the card would exist twice.
        ctx::take_card(player_id, c.as_str(), CardPile::Deck);
        let paid = ctx::ask_yes(
            player_id,
            &Msg::new(key!("ako_place_title")),
            &Msg::new(key!("ako_place_ask")).card("card", c.as_str()),
        )?;
        if paid && ctx::pay(player_id, 500, &Msg::new(key!("ako_place_pay")))? > 0 {
            ctx::set_dest(ctx::Dest::Field);
            // Hold the uid `place_card` hands back: this is 「此卡」 -- the copy
            // just placed -- and re-resolving `c` by name would pick whichever
            // copy of that id is first on the field.
            let uid = ctx::place_card(player_id, c.as_str(), &Msg::new(key!("ako_note")));
            ctx::set_face_down_at(uid, true);
            ctx::log(
                player_id,
                &Msg::new(key!("ako_placed")).card("card", c.as_str()),
            );
        } else {
            ctx::to_discard(player_id, c.as_str());
        }
    }
    Ok(())
}

//

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn on_circle_kani(player_id: i32) -> card_sdk::Asked {
    on_circle(player_id)
}
