//! `skill:户山香澄:非凡之星`
//!
//! 规则书（skill sheet, 户山香澄）:
//! > （1）其他玩家"星之鼓动山丘"上[结算]时获得一个[火罐]（初始0，上限1）
//! > （2）运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]
//! >      改为[传送]到任意自己拥有的格子且可选择盖房
//!
//! Both halves are ordinary entries on one [`CardDef`] -- a field event and a
//! user press -- so no new hook is needed. See the crate docs for why.
//!
//! The parenthetical 「初始0，上限1」 is this skill's to state, and it is the
//! whole reason the keyed state carries bounds: it writes `fire`'s `max` and
//! every card reads it back with `state::max(player_id, state_key::FIRE)`. The
//! engine holds the number and enforces nothing -- see `game_core::state::StateVar`.
//!
//! The cap is restated at the top of every turn rather than bound once when the
//! character is picked, because there is no "character assigned" occurrence.
//! This is not a divergence from 「初始」: the sentence is a standing claim that
//! the cap *is* 1, and restating it is the same claim. It does imply nothing
//! else may write `fire`'s `max` mid-turn -- a rule that raised the cap would be
//! disagreeing with this skill, which is its own divergence to note rather than
//! something for this body to accommodate.

use alloc::vec::Vec;

use card_sdk::abi::{state_key, HookKind, MoveKind};
use card_sdk::ctx::{self, plan, state};
use card_sdk::{key, CardDef, Msg, On};

/// The tile the clause names.
fn hill() -> i32 {
    ctx::tile_named("星之鼓动山丘")
}

pub const EXTRAORDINARY_STAR: CardDef = CardDef::new(
    "skill:户山香澄:非凡之星",
    &[
        On::Play(Some(can_use), use_skill, ""),
        On::Hook(&[HookKind::TurnStartBefore, HookKind::DeckAtGameStart], None, declare_cap, ""),
        On::Hook(&[HookKind::Settle], None, on_settle, ""),
    ],
);

/// 「初始0，上限1」 -- this skill states the fire-pot cap. It is a *consumer*
/// deciding the bound; the engine only holds it.
fn declare_cap(player_id: i32) -> card_sdk::Asked {
    crate::fire_pot(player_id, 0, 1);
    Ok(())
}

/// （1）「其他玩家"星之鼓动山丘"上[结算]时获得一个[火罐]」 -- a field event, so
/// an [`On::Hook`] at `Settle`. The settler is the trigger's player and the tile
/// is the trigger's tile; the clause says *another* player, so this player's own
/// landing does not pay out.
fn on_settle(player_id: i32) -> card_sdk::Asked {
    if ctx::trigger::player_id() == player_id {
        return Ok(());
    }
    if ctx::trigger::tile() != hill() {
        return Ok(());
    }
    ctx::gain_fire(player_id, 1, &Msg::new(key!("extraordinary_star.gain")))?;
    Ok(())
}

/// （2）「运营阶段可使用1个[火罐]」 -- the gate on the press. The engine already
/// refuses an action outside the operations phase, as it does for every `act`,
/// so this only has to check the cost.
fn can_use(player_id: i32) -> Option<Msg> {
    if card_sdk::ctx::skill_blocked(player_id, "") {
        return Some(Msg::new(key!("skill_blocked")));
    }
    if state::get(player_id, state_key::FIRE) < 1 {
        return Some(Msg::new(key!("extraordinary_star.no_fire")));
    }
    None
}

/// （2）「立刻进入移动阶段且本回合的[主要移动]改为[传送]到任意自己拥有的格子
/// 且可选择盖房」 -- a press, so an [`On::Play`]. Spending the pot is the cost and
/// is all-or-nothing; the destination is a choice among the tiles this player
/// owns; 「可选择盖房」 is the move being allowed to build where it lands.
fn use_skill(player_id: i32) -> card_sdk::Asked {
    if !ctx::spend_fire(player_id, 1, &Msg::new(key!("extraordinary_star.spend")))? {
        return Ok(());
    }
    let owned = ctx::owned_tiles(player_id);
    if owned.is_empty() {
        return Ok(());
    }
    let options: Vec<Msg> = owned
        .iter()
        .map(|&t| Msg::new(key!("extraordinary_star_option")).tile("tile", t))
        .collect();
    let k = ctx::ask_pick(
        player_id,
        &Msg::new(key!("extraordinary_star_pick_title")),
        &Msg::new(key!("extraordinary_star_pick_text")),
        &options,
    )?;
    let to = *owned.get(k).unwrap_or(&owned[0]);

    // 「本回合的[主要移动]改为[传送]」 -- the walk is replaced, not added to.
    // 「且可选择盖房」 needs no flag: building where the move lands is the
    // normal rule (`can_build` defaults to true, and `false` is the exception).
    plan::set_kind(MoveKind::Teleport);
    plan::set_teleport_to(to);
    ctx::card_move(player_id);
    Ok(())
}
