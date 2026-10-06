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
//! `card_face_down` reads and `set_dest(Dest::Field)` + `place_card` writes.
//! (2) and (3) are both keyed on that flag.
//!
//! (1) 「若选择抽卡」 is the CiRCLE reward being the card, which is the
//! `circleAffected` moment -- the reward has been chosen, and this skill
//! rewrites the draw it implies.

use alloc::string::String;

use card_sdk::abi::{CardPile, HookKind};
use card_sdk::ctx;
use card_sdk::{key, CardDef, Msg, On};

pub const AKAO_COOL: CardDef = CardDef::new("skill:宇田川亚子:对帅气的憧憬", &[
    On::Hook(&[HookKind::CircleAffected], mine, on_circle)]);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// Face-down cards this player has on the field.
fn face_down(player_id: i32) -> alloc::vec::Vec<(i32, String)> {
    ctx::field_instances(player_id)
        .into_iter()
        .filter(|(uid, _)| ctx::is_face_down_at(*uid))
        .collect()
}

/// （1） and （2） -- the CiRCLE reward.
fn on_circle(player_id: i32) -> card_sdk::Asked {
    // （2）「若自己场上存在背面朝上的卡，则本次领取奖励被替换为将自己场上
    // 背面朝上的卡拿取至手中」 -- the replacement takes priority: it is what the
    // reward *is* once there is a face-down card to take.
    let down = face_down(player_id);
    if !down.is_empty() {
        for (uid, c) in down {
            ctx::unplace_at(uid);
            ctx::add_to_hand(player_id, &c);
            ctx::log(player_id, &Msg::new(key!("akao_cool_taken")).card("card", &c));
        }
        return Ok(());
    }
    // （1）「若选择抽卡，则可以看抽牌堆顶至多两张卡」 -- the reward is the draw;
    // look at up to two and keep one.
    let deck = ctx::cards_in(player_id, CardPile::Deck);
    if deck.is_empty() {
        return Ok(());
    }
    let n = deck.len().min(2);
    let look: alloc::vec::Vec<String> = deck[..n].to_vec();
    let pick = ctx::ask_pick(
        player_id,
        &Msg::new(key!("akao_cool_title")),
        &Msg::new(key!("akao_cool_ask")),
        &look
            .iter()
            .map(|c| Msg::new(key!("akao_cool_option")).card("card", &c))
            .collect::<alloc::vec::Vec<_>>(),
    )?;
    let Some(keep) = look.get(pick).cloned() else { return Ok(()); };
    if !ctx::take_card(player_id, CardPile::Deck, &keep) {
        return Ok(());
    }
    ctx::add_to_hand(player_id, &keep);
    // 「剩余的卡放入弃牌堆或花费500资金将其背面朝上放置在自己场上」
    for c in look.iter().filter(|c| **c != keep) {
        let paid = ctx::ask_yes(
            player_id,
            &Msg::new(key!("akao_cool_place_title")),
            &Msg::new(key!("akao_cool_place_ask")).card("card", c),
        )?;
        if paid && ctx::pay(player_id, 500, &Msg::new(key!("akao_cool_place_pay")))? > 0 {
            ctx::set_dest(ctx::Dest::Field);
            // Hold the uid `place_card` hands back: this is 「此卡」 -- the copy
            // just placed -- and re-resolving `c` by name would pick whichever
            // copy of that id is first on the field.
            let uid = ctx::place_card(player_id, c, &Msg::new(key!("akao_cool_note")));
            ctx::set_face_down_at(uid, true);
            ctx::log(player_id, &Msg::new(key!("akao_cool_placed")).card("card", &c));
        } else {
            ctx::take_card(player_id, CardPile::Deck, c);
            ctx::to_discard(player_id, c);
        }
    }
    Ok(())
}