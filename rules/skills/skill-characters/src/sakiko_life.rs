//! `skill:丰川祥子:请把你们的人生交给我`
//!
//! 规则书（skill sheet, 丰川祥子）:
//! > 状态1： 经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上（仍正常完成
//! > 本次移动），若如此做，每获得一层停留，眩晕，你获得1500资金。每次受到停留，眩晕，
//! > 除外影响（并结算其影响），获得一个火罐（初始0，上限3），火罐数达到上限时可在
//! > 回合开始时选择进入状态2。状态2：保留至多一张手牌并将其余置入弃牌堆。每回合开始
//! > 时自动打出一张抽牌堆顶端的牌。
//!
//! 「状态1 / 状态2」 is the axis [`skill_bands::ave_mujica`] owns. 状态1's pass
//! clause is a `PassPlayer` transfer of the other player's `[停留]`/`[眩晕]`
//! layers onto this one, with a bounty per layer. The pot is granted at the
//! *outcome* of an abnormal effect (`stay` / `stun` / `exile`), after it lands.
//!
//! 状态2's 「保留至多一张手牌并将其余置入弃牌堆」 is a one-shot on entry;
//! 「每回合开始时自动打出一张抽牌堆顶端的牌」 is a free `play_card` of the
//! deck's top card at each of this player's turn starts.

use card_sdk::abi::{state_key, HookKind};
use card_sdk::ctx::{self, state, CardPile};
use card_sdk::{key, CardDef, Msg, On};

/// 「每获得一层停留，眩晕，你获得1500资金」.
const BOUNTY: i32 = 1500;

pub const SAKIKO_LIFE: CardDef = CardDef::new(
    "skill:丰川祥子:请把你们的人生交给我",
    &[
        On::Hook(&[HookKind::TurnStartBefore], |_| true, declare_cap),
        On::Hook(&[HookKind::TurnStartBefore], mine, at_turn_start),
        On::Hook(&[HookKind::PassPlayer], in_one, on_pass_player),
        On::Hook(
            &[HookKind::Stay, HookKind::Stun, HookKind::Exile],
            in_one,
            on_abnormal,
        ),
    ],
);

fn mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

fn in_one(player_id: i32) -> bool {
    mine(player_id) && state::get(player_id, state_key::SKILL_STATE) != 2
}

/// 「初始0，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    state::set_bounds(player_id, state_key::FIRE, 0, 3);
    Ok(())
}

/// 状态1's pass clause, and 状态2's entry offer.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        // 状态2: 「每回合开始时自动打出一张抽牌堆顶端的牌」
        play_deck_top(player_id);
        return Ok(());
    }
    // 「火罐数达到上限时可在回合开始时选择进入状态2」
    if state::get(player_id, state_key::FIRE) < state::max(player_id, state_key::FIRE) {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("sakiko_life_title")),
        &Msg::new(key!("sakiko_life_enter")),
    )? {
        return Ok(());
    }
    state::set(player_id, state_key::SKILL_STATE, 2);
    ctx::log(player_id, &Msg::new(key!("sakiko_life_two")));
    // 状态2: 「保留至多一张手牌并将其余置入弃牌堆」
    let mut hand = ctx::cards_in(player_id, CardPile::Hand);
    while hand.len() > 1 {
        let c = hand.pop().unwrap();
        ctx::discard_from_hand(player_id, &c);
    }
    play_deck_top(player_id);
    Ok(())
}

/// 「经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上（仍正常完成本次
/// 移动），若如此做，每获得一层停留，眩晕，你获得1500资金」.
fn on_pass_player(player_id: i32) -> card_sdk::Asked {
    if !ctx::trigger::move_is_main() {
        return Ok(());
    }
    let other = ctx::trigger::player_id();
    if other == player_id {
        return Ok(());
    }
    let stay = ctx::stay_of(other);
    let stun = ctx::stun_of(other);
    if stay + stun <= 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("sakiko_life_title")),
        &Msg::new(key!("sakiko_life_take"))
            .player_id("who", other)
            .i("n", (stay + stun) as i64),
    )? {
        return Ok(());
    }
    // 「转移至自己身上（仍正常完成本次移动）」 -- the layers move; the walk
    // is untouched.
    if stay > 0 {
        ctx::give_stay(player_id, stay);
    }
    if stun > 0 {
        ctx::give_stun(player_id, stun);
    }
    // 「每获得一层停留，眩晕，你获得1500资金」
    ctx::gain(
        player_id,
        BOUNTY * (stay + stun),
        &Msg::new(key!("sakiko_life_bounty")),
    );
    ctx::log(
        player_id,
        &Msg::new(key!("sakiko_life_moved"))
            .player_id("who", other)
            .i("n", (stay + stun) as i64),
    );
    Ok(())
}

/// 「每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐」.
fn on_abnormal(player_id: i32) -> card_sdk::Asked {
    ctx::gain_fire(player_id, 1, &Msg::new(key!("sakiko_life_gain")));
    Ok(())
}

/// 状态2's 「每回合开始时自动打出一张抽牌堆顶端的牌」.
fn play_deck_top(player_id: i32) -> card_sdk::Asked {
    let deck = ctx::cards_in(player_id, CardPile::Deck);
    let Some(top) = deck.into_iter().next() else {
        return Ok(());
    };
    ctx::log(
        player_id,
        &Msg::new(key!("sakiko_life_auto")).card("card", &top),
    );
    ctx::play_card(&top, player_id)?;
    Ok(())
}
