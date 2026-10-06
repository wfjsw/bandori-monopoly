//! `skill:CRYCHIC:美好的往日幻影`
//!
//! 规则书（band sheet, CRYCHIC）:
//! > （1）你的手牌数没有上限，任何时刻拥有手牌数大于等于6时，你无法获得或失去资金，
//! > 无法从手中打出任何牌且领取CiRCLE奖励时必须选择抽一张卡。（2）你的回合结束时，
//! > 若你的抽牌堆与弃牌堆中都没有卡，移除此卡与你所有区域的所有"CRYCHIC"卡，将你
//! > 剩余的所有手牌放入抽牌堆，并向抽牌堆中加入角色对应的自选"MyGO"或"Ave Mujica"
//! > 卡至抽牌堆中总共有10张卡并洗切；获得角色对应的"MyGO"或"Ave Mujica"乐队技能卡，
//! > 然后抽2张卡。（3）每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶（上限10）；
//! > 当此卡移除时，你获得X*500资金，X为此卡上的奇迹水晶数。
//!
//! （1）'s 「手牌数大于等于6」 is a standing claim: money cannot move and no card
//! can be played from hand. （2） is the hand-emptying reshuffle that turns the
//! player into a MyGO or Ave Mujica player. （3） is a crystal on a 4→5 crossing.

use card_sdk::abi::{state_key, CardPile, HookKind};
use card_sdk::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

const ID: &str = "skill:CRYCHIC:美好的往日幻影";
/// Hand size last seen, so （3） can see a 4→5 crossing.
const SEEN: &str = "skill.crychic.seen";

pub const CRYCHIC: CardDef = CardDef::new(
    "skill:CRYCHIC:美好的往日幻影",
    &[
        On::Hook(&[HookKind::TurnStartBefore], mine, at_turn_start),
        On::Hook(&[HookKind::Drawn], mine, on_drawn),
        On::Hook(&[HookKind::TurnEndBefore], mine, at_turn_end),
        On::Hook(&[HookKind::PayChoose], in_lock, lock_pay),
        On::Hook(&[HookKind::CircleAffected], in_lock, force_card),
        On::Play(Some(can_transform), transform_now),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// （1）「任何时刻拥有手牌数大于等于6时」.
fn in_lock(player_id: i32) -> bool {
    ctx::hand_size(player_id) >= 6
}

fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    // （1）「你的手牌数没有上限」 -- the engine's hand cap is the `handLimit` key.
    state::set(player_id, state_key::HAND_LIMIT, i32::MAX);
    state::set(player_id, SEEN, ctx::hand_size(player_id));
    Ok(())
}

/// （1）「你无法获得或失去资金」 -- a payment in either direction is voided.
fn lock_pay(player_id: i32) -> card_sdk::Asked {
    ctx::trigger::set_pay_amount(0);
    ctx::log(player_id, &Msg::new(key!("crychic_locked")));
    Ok(())
}

/// （3）「每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶（上限10）」.
fn on_drawn(player_id: i32) -> card_sdk::Asked {
    let before = state::get(player_id, SEEN);
    let now = ctx::hand_size(player_id);
    state::set(player_id, SEEN, now);
    if before == 4 && now >= 5 {
        ctx::add_crystals(1, 10);
        ctx::log(player_id, &Msg::new(key!("crychic_crystal")));
    }
    Ok(())
}

/// （2）「你的回合结束时，若你的抽牌堆与弃牌堆中都没有卡…」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    if ctx::deck_count(player_id) > 0 || ctx::discard_size(player_id) > 0 {
        return Ok(());
    }
    ctx::log(player_id, &Msg::new(key!("crychic_empty")));
    // 「当此卡移除时，你获得X*500资金」
    let x = ctx::crystals();
    if x > 0 {
        ctx::gain(
            player_id,
            x * 500,
            &Msg::new(key!("crychic_cash_out")).i("n", (x * 500) as i64),
        );
        ctx::add_crystals(-x, 10);
    }
    // 「移除此卡与你所有区域的所有"CRYCHIC"卡」 -- every zone.
    for (uid, c) in ctx::field_instances(player_id) {
        if c.starts_with("CRYCHIC:") {
            ctx::unplace_at(uid);
            ctx::to_discard(player_id, &c);
        }
    }
    for c in ctx::cards_in(player_id, CardPile::Hand) {
        // 「此卡不会被你乐队技能的（2）效果移除」 -- 「内心的呐喊」 stays.
        if c == "CRYCHIC:（灯）内心的呐喊" {
            continue;
        }
        if c.starts_with("CRYCHIC:") {
            ctx::discard_from_hand(player_id, &c);
        }
    }
    for c in ctx::cards_in(player_id, CardPile::Deck) {
        if c.starts_with("CRYCHIC:") {
            ctx::take_card(player_id, CardPile::Deck, &c);
            ctx::to_discard(player_id, &c);
        }
    }
    // 「将你剩余的所有手牌放入抽牌堆」
    for c in ctx::cards_in(player_id, CardPile::Hand) {
        ctx::discard_from_hand(player_id, &c);
    }
    ctx::sweep_to_deck(player_id);
    // 「并向抽牌堆中加入角色对应的自选"MyGO"或"Ave Mujica"卡至抽牌堆中总共有
    // 10张卡并洗切」, and 「获得角色对应的"MyGO"或"Ave Mujica"乐队技能卡」 --
    // one pick names both.
    let mygo = ctx::ask_yes(
        player_id,
        &Msg::new(key!("crychic_title")),
        &Msg::new(key!("crychic_which")),
    )?;
    while ctx::deck_count(player_id) < 10 {
        ctx::add_to_deck(player_id, if mygo { "MyGO" } else { "Ave Mujica" }, false);
    }
    ctx::shuffle_into_deck(player_id, false, true);
    // 「获得角色对应的"MyGO"或"Ave Mujica"乐队技能卡，然后抽2张卡」
    let band_skill = if mygo {
        "skill:MyGO!!!!!:迷途之星"
    } else {
        "skill:Ave Mujica:假面之下的真实"
    };
    ctx::place_card(player_id, band_skill, &Msg::new(key!("crychic_gained")));
    ctx::draw(player_id, 2);
    Ok(())
}

// （1）「无法从手中打出任何牌且领取CiRCLE奖励时必须选择抽一张卡」 -- the play
// block and the forced draw are two more sides of the same standing claim.
// TODO(规则书)（1）: 「无法从手中打出任何牌」 -- a play gate keyed on *this*
//   player's hand size, which is a global rule rather than one card's `WhyNot`.
//   The engine's play path consults the played card's own gate, not every placed
//   card's.
// （1）「领取CiRCLE奖励时必须选择抽一张卡」 -- `circleAffected` fires between the
// pick and the payout, so the money half can be cancelled and replaced with the
// draw the clause mandates.
// （2）「移除此卡与你所有区域的所有"CRYCHIC"卡」 -- a zone-by-zone sweep:
// `placed_cards` + `unplace_card_named` for the field, `cards_in(Hand)` +
// `discard_from_hand` for the hand, `cards_in(Deck)` + `take_card` for the deck.
// 「获得角色对应的"MyGO"或"Ave Mujica"乐队技能卡」 is the same pick the reshuffle
// already offers -- the two band-skill ids are fixed, and the choice names which.

/// （1）「领取CiRCLE奖励时必须选择抽一张卡」 -- the money option is replaced.
fn force_card(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::value() != card_sdk::abi::REWARD_MONEY {
        return Ok(());
    }
    ctx::trigger::set_cancelled();
    ctx::draw(player_id, 1);
    ctx::log(player_id, &Msg::new(key!("crychic_forced_draw")));
    Ok(())
}

/// 「立即执行乐队技能的（2）效果」 -- the (2) reshuffle is also a press, so a card
/// can run it out of turn (`mutsumi_never` calls `play_card` on this id).
fn can_transform(player_id: i32) -> Option<Msg> {
    if ctx::deck_count(player_id) > 0 || ctx::discard_size(player_id) > 0 {
        return Some(Msg::new(key!("crychic_not_empty")));
    }
    None
}

fn transform_now(player_id: i32) -> card_sdk::Asked {
    crate::spend_copy_sticker(player_id);
    if ctx::deck_count(player_id) > 0 || ctx::discard_size(player_id) > 0 {
        return Ok(());
    }
    at_turn_end(player_id)?;
    Ok(())
}
