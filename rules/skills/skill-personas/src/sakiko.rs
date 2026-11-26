//! `skill:丰川祥子:请把你们的人生交给我`
//!
//! 规则书（skill sheet, 丰川祥子）:
//! > 状态1： 经过其他玩家时，可将其所有层数的停留，眩晕转移至自己身上（仍正常完成
//! > 本次移动），若如此做，每获得一层停留，眩晕，你获得1500资金。每次受到停留，眩晕，
//! > 除外影响（并结算其影响），获得一个火罐（初始0，上限3），火罐数达到上限时可在
//! > 回合开始时选择进入状态2。状态2：保留至多一张手牌并将其余置入弃牌堆。每回合开始
//! > 时自动打出一张抽牌堆顶端的牌。
//!
//! 「状态1 / 状态2」 is the axis [`skill_groups::ave_mujica`] owns. 状态1's pass
//! clause is a `PassTile` transfer of the other player's `[停留]`/`[眩晕]`
//! layers onto this one, with `(X+Y)×500` money. The pot is granted on the
//! **natural wear** of this player's own `[停留]`/`[眩晕]`/`[除外]`, not on an
//! effect landing.
//!
//! 状态2's 「保留至多一张手牌并将其余置入弃牌堆」 is a one-shot on entry;
//! 「每回合开始时自动打出一张抽牌堆顶端的牌」 is a free `play_card` of the
//! deck's top card at each of this player's turn starts.

use bandori_std::ctx::{self, state, trigger, CardPile};
use bandori_std::state_key;
use card_sdk::abi::ActionKind;
use card_sdk::{key, Action, CardDef, Msg, On};

/// 「自己[获得](X+Y)×500资金」 -- per transferred layer.
const BOUNTY_PER_LAYER: i32 = 500;

pub const SAKIKO: CardDef = CardDef::new(
    "skill:丰川祥子:请把你们的人生交给我",
    &[
        On::Action(Action::TurnStartBefore("", None, declare_cap)).silent(),
        On::Action(Action::DeckAtGameStart("", None, declare_cap)).silent(),
        // 状态1's 「火罐数达到上限时可在回合开始时选择进入状态2」 -- the offer
        // exists only at the pot cap. 状态2's auto-play still has to run
        // whenever state is 2, so the residual is the disjunction.
        On::Action(Action::TurnStartBefore(
            card_sdk::condition::MINE,
            Some(state2_or_fire_capped),
            at_turn_start,
        )),
        // 状态1 「经过其他玩家时」 -- 行动阶段 12 [经过] (`SETTLE-STAGES.md` §4
        // M4): the step onto a tile another player stands on, not the end-tile
        // [重叠]. The "other player" is read off the tile, not `target`.
        On::Action(Action::PassTile(
            "actor == owner && move.is_main_move && slot('skillState') != 2 && players_on(tile.id, owner) > 0",
            Some(first_afflicted_on_tile),
            on_pass_player,
        )),
        // 「自己未处于[人偶模式]，自己的[停留]、[眩晕]、[除外]自然减少层数时：
        // 获得1个[火罐]」 -- the **natural wear**, not an effect landing. Each
        // kind that will drop a layer this turn end pays one pot; 停留/眩晕 tick
        // at this very hook, so the pre-decrement count is read. 除外 ticks at
        // the next turn start, so a live exile pays one pot per turn end here
        // (one layer late, but one-per-turn either way).
        On::Action(Action::TurnEndBefore(
            card_sdk::condition::MINE,
            Some(state1_wear),
            on_wear,
        )),
    ],
)
.rule()
// 「（初始0，上限3）」
.props(&[
    (bandori_std::fire::FIRE_INITIAL, 0),
    (bandori_std::fire::FIRE_CAP, 3),
]);

/// Guard for [`on_wear`]: 状态1 only (`Abnormal`'s old `im_hit`, now also
/// covering the 状态2 exclusion) and something is about to wear.
fn state1_wear(player_id: i32) -> bool {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        return false;
    }
    rules_status::stay::get(player_id) + rules_status::stun::get(player_id) > 0
        || state::get(player_id, state_key::EXILE) > 0
}

/// 「初始0，上限3」.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id);
    Ok(())
}

/// Residual guard for [`at_turn_start`]: 状态2 (auto-play must run) or the
/// pot is at cap (the 状态2 offer). `state::max` is the declared fire cap.
fn state2_or_fire_capped(player_id: i32) -> bool {
    state::get(player_id, state_key::SKILL_STATE) == 2
        || bandori_std::fire::get(player_id) >= bandori_std::fire::max(player_id)
}

/// 状态1's pass clause, and 状态2's entry offer.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, state_key::SKILL_STATE) == 2 {
        // 状态2: 「每回合开始时自动打出一张抽牌堆顶端的牌」
        play_deck_top(player_id)?;
        return Ok(());
    }
    // 「火罐数达到上限时可在回合开始时选择进入状态2」 -- the residual already
    // promised the cap.
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("sakiko_title")),
        &Msg::new(key!("sakiko_enter")),
    )? {
        return Ok(());
    }
    state::set(player_id, state_key::SKILL_STATE, 2);
    ctx::log(player_id, &Msg::new(key!("sakiko_two")));
    // 状态2: 「保留至多一张手牌并将其余置入弃牌堆」
    //
    // Discard from the end so the graveyard keeps its historical order (the
    // last card is discarded first). 「保留至多一张手牌并将其余置入弃牌堆」 does
    // not name an order, but a later 「弃牌堆顶」 reader sees whatever landed
    // last -- a `drain(1..)` here would silently reverse that for no gain.
    let mut hand = ctx::cards_in(player_id, CardPile::Hand);
    while hand.len() > 1 {
        if let Some(c) = hand.pop() {
            ctx::discard_from_hand(player_id, c.as_str());
        }
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
///
/// Residual guard for the pass entry: the player the body would read (the
/// first other player on the entered tile) carries a [停留] or [眩晕]. The
/// pass hook otherwise announces 「效果适用 …（无事发生）」 on every step.
fn first_afflicted_on_tile(player_id: i32) -> bool {
    let Some(&other) = ctx::players_on(ctx::trigger::tile(), player_id).first() else {
        return false;
    };
    rules_status::stay::get(other) + rules_status::stun::get(other) > 0
}

fn on_pass_player(player_id: i32) -> card_sdk::Asked {
    let at = ctx::trigger::tile();
    let Some(&other) = ctx::players_on(at, player_id).first() else {
        return Ok(());
    };
    let stay = rules_status::stay::get(other);
    let stun = rules_status::stun::get(other);
    if stay + stun <= 0 {
        return Ok(());
    }
    if !ctx::ask_yes(
        player_id,
        &Msg::new(key!("sakiko_title")),
        &Msg::new(key!("sakiko_take"))
            .player_id("who", other)
            .i("n", (stay + stun) as i64),
    )? {
        return Ok(());
    }
    // 「转移至自己身上（仍正常完成本次移动）」 -- the layers **move**; the
    // walk is untouched. Take them off the other player first, then land them
    // here (the self-side `give_*` raises `Abnormal` for the fire-pot clause).
    if stay > 0 {
        state::add(other, state_key::STAY, -stay);
        rules_status::stay::add(player_id, stay);
    }
    if stun > 0 {
        state::add(other, state_key::STUN, -stun);
        rules_status::stun::add(player_id, stun);
    }
    // 「自己[获得](X+Y)×500资金」
    ctx::gain(
        player_id,
        BOUNTY_PER_LAYER * (stay + stun),
        &Msg::new(key!("sakiko_bounty")),
    )?;
    ctx::log(
        player_id,
        &Msg::new(key!("sakiko_moved"))
            .player_id("who", other)
            .i("n", (stay + stun) as i64),
    );
    Ok(())
}

/// 「自己的[停留]、[眩晕]、[除外]自然减少层数时：获得1个[火罐]」 -- one pot per
/// kind about to lose a layer (the guard already pinned 状态1 and a live layer).
fn on_wear(player_id: i32) -> card_sdk::Asked {
    let stay = rules_status::stay::get(player_id);
    let stun = rules_status::stun::get(player_id);
    let exile = state::get(player_id, state_key::EXILE);
    let n = i32::from(stay > 0) + i32::from(stun > 0) + i32::from(exile > 0);
    if n > 0 {
        bandori_std::fire::gain(player_id, n, &Msg::new(key!("sakiko_gain")))?;
    }
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
        &Msg::new(key!("sakiko_auto")).card("card", top.as_str()),
    );
    // 规则书: 「每回合开始时自动打出一张抽牌堆顶端的牌」 -- the play takes the card
    // out of the pile (as the hand path does before resolving) and leaves it on its
    // own `Dest`; `ctx::play_card` only reports that `Dest` back to the caller.
    ctx::take_card(player_id, top.as_str(), CardPile::Deck);
    let dest = ctx::play_card(top.as_str(), player_id)?;
    match dest {
        ctx::Dest::Field | ctx::Dest::Banished => {}
        dest => ctx::add_to_dest(player_id, top.as_str(), dest, false),
    }
    Ok(())
}

//

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn declare_cap_kani(player_id: i32) -> card_sdk::Asked {
    declare_cap(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_pass_player_kani(player_id: i32) -> card_sdk::Asked {
    on_pass_player(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn state1_wear_kani(player_id: i32) -> bool {
    state1_wear(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_wear_kani(player_id: i32) -> card_sdk::Asked {
    on_wear(player_id)
}
