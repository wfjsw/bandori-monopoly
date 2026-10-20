//! `skill:珠手知由:天才制作人`
//!
//! 规则书（skill sheet, 珠手知由）:
//! > 加盖房屋时半价，如果你上回合进行过房屋加盖，本回合加盖房屋变为免费
//!
//! Two prices for the same action, and the second supersedes the first: a
//! standing half, and a whole free turn when the previous turn built. Both are
//! *prices*, not policies -- the engine stores the rate and `BuildRoutine`
//! applies it.
//!
//! 「上回合进行过房屋加盖」 is latched at `houseAdded` and read at the next
//! turn start; the free turn is announced when it is taken.

use card_sdk::abi::ActionKind;
use bandori_std::ctx::{self, state};
use card_sdk::{key, CardDef, Msg, On};

/// Did this player's own build commit last turn?
const BUILT_LAST: &str = "skill.tamade.builtLast";
/// Did one commit this turn? Latched at `houseAdded`.
const BUILT_THIS: &str = "skill.tamade.builtThis";

pub const TAMADE_PRODUCER: CardDef = CardDef::new(
    "skill:珠手知由:天才制作人",
    &[
        // 「加盖房屋时半价，如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」
        // -- one hook arms the whole turn's price. The free turn (builtLast != 0)
        // must still enter the body; gating the hook on `builtLast == 0` left
        // that arm dead (and skipped the builtThis reset), so the second build
        // paid full price.
        On::Action(&[ActionKind::TurnStartBefore], card_sdk::pre::MINE, "", None, at_turn_start),
        On::Action(&[card_sdk::abi::ActionKind::HouseAdded], card_sdk::pre::MINE, "", None, on_built),
        On::Action(&[ActionKind::TurnEnd], card_sdk::pre::MINE, "", None, at_turn_end),
    ],
)
    .silent(&[1, 2]);

/// 「加盖房屋时半价」, or 「本回合加盖房屋变为免费」 when last turn built.
fn at_turn_start(player_id: i32) -> card_sdk::Asked {
    if state::get(player_id, BUILT_LAST) != 0 {
        // The free turn supersedes the half.
        ctx::set_build_cost_pct(0);
        state::set(player_id, BUILT_LAST, 0);
        ctx::log(player_id, &Msg::new(key!("tamade_free")));
    } else {
        ctx::set_build_cost_pct(50);
    }
    state::set(player_id, BUILT_THIS, 0);
    Ok(())
}

fn on_built(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, BUILT_THIS, 1);
    Ok(())
}

/// Carry this turn's latch forward into 「上回合」.
fn at_turn_end(player_id: i32) -> card_sdk::Asked {
    state::set(player_id, BUILT_LAST, state::get(player_id, BUILT_THIS));
    state::set(player_id, BUILT_THIS, 0);
    Ok(())
}

// Kani-only direct-call exports (ppcost). Production builds omit these.
#[cfg(kani)]
#[inline(always)]
pub fn at_turn_start_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_start(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn on_built_kani(player_id: i32) -> card_sdk::Asked {
    on_built(player_id)
}

#[cfg(kani)]
#[inline(always)]
pub fn at_turn_end_kani(player_id: i32) -> card_sdk::Asked {
    at_turn_end(player_id)
}
