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
use card_sdk::ctx::{self, state, trigger, CardPile};
use card_sdk::{key, CardDef, Msg, On};

/// 「每获得一层停留，眩晕，你获得1500资金」.
const BOUNTY: i32 = 1500;

pub const SAKIKO_LIFE: CardDef = CardDef::new(
    "skill:丰川祥子:请把你们的人生交给我",
    &[
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], None, declare_cap, ""),
        On::Hook(&[HookKind::TurnStartBefore], None, at_turn_start, card_sdk::pre::MINE),
        // 状态1 「经过其他玩家时」 -- 行动阶段 12 [经过] (`SETTLE-STAGES.md` §4
        // M4): the step onto a tile another player stands on, not the end-tile
        // [重叠]. The "other player" is read off the tile, not `target`.
        On::Hook(&[HookKind::PassTile], Some(in_one), on_pass_player, ""),
        // 「每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐」 -- the
        // outcome of an abnormal effect landing on this player. `Abnormal` is
        // the settlement hook; the legacy `Stay`/`Stun`/`Exile` kinds are never
        // raised. The guard keeps 状态1 and this player as the *recipient*
        // (`trigger::target()`), not the causer.
        On::Hook(&[HookKind::Abnormal], Some(im_hit), on_abnormal, ""),
    ],
)
    .legacy(&[(1, legacy_mine)]);

fn legacy_mine(player_id: i32) -> bool {
    ctx::trigger::player_id() == player_id
}

/// G4: kept as a callable alias for in-body uses of the old guard.
fn mine(player_id: i32) -> bool {
    legacy_mine(player_id)
}

fn in_one(player_id: i32) -> bool {
    mine(player_id) && state::get(player_id, state_key::SKILL_STATE) != 2
}

/// The `Abnormal` hook's guard: this player is the *recipient* of the effect
/// (`trigger::target()`), still in 状态1.
fn im_hit(player_id: i32) -> bool {
    trigger::target() == player_id && state::get(player_id, state_key::SKILL_STATE) != 2
}

/// 「初始0，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 3);
    Ok(())
}

/// 状态1's pass clause, and 状态2's entry offer.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        // 状态2: 「每回合开始时自动打出一张抽牌堆顶端的牌」
        play_deck_top(player_id)?;
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
    play_deck_top(player_id)?;
    Ok(())
}

/// 「经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上（仍正常完成本次
/// 移动），若如此做，每获得一层停留，眩晕，你获得1500资金」.
///
/// `SETTLE-STAGES.md` §4 M4: 「经过其他玩家」 is the passer's step onto a tile
/// another player stands on (行动阶段 12 [经过]). The other player is read off
/// the tile -- a `passTile` payload has no `target`. (It used to sit on
/// `passPlayer` and read `trigger::player_id()` as "the other", which is the
/// mover -- the guard already pinned that to `player_id`, so the body bailed at
/// its own first line and the clause never fired.)
fn on_pass_player(player_id: i32) -> card_sdk::Asked {
    if !ctx::trigger::move_is_main() {
        return Ok(());
    }
    let at = ctx::trigger::tile();
    let Some(&other) = ctx::players_on(at, player_id).first() else {
        return Ok(());
    };
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
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("sakiko_life_moved"))
            .player_id("who", other)
            .i("n", (stay + stun) as i64),
    );
    Ok(())
}

/// 「每次受到停留，眩晕，除外影响（并结算其影响），获得一个火罐」 -- the
/// *recipient* of the abnormal gains the pot. `in_one` (the hook guard) has
/// already checked 状态1; here we only need the recipient match.
fn on_abnormal(player_id: i32) -> card_sdk::Asked {
    // The `abnormal` hook carries `t.target` = the recipient and `t.player_id`
    // = the causer. 「受到…影响」 names the recipient.
    if trigger::target() != player_id {
        return Ok(());
    }
    if !matches!(
        trigger::abnormal_kind(),
        Some(card_sdk::abi::AbKind::Stay | card_sdk::abi::AbKind::Stun | card_sdk::abi::AbKind::Exile)
    ) {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("sakiko_life_gain")))?;
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
