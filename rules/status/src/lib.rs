//! **Status rule hosts** -- the `status:*` category: one standing pseudo card
//! per rulebook status marker, bound **once** to the neutral board owner
//! (`BOARD_OWNER`, `tile = -1`) via `game_core::data::status_rule_ids` ->
//! `bind_tiles`. See [`docs/STATUS-HOSTS.md`].
//!
//! | id | file | rulebook (`data/rules.txt`) |
//! |---|---|---|
//! | `status:stay` | `stay.rs` | 48 「[停留]」 + 47 「[无法移动]」 |
//! | `status:stun` | `stun.rs` | 49 「[晕眩]」 |
//! | `status:exile` | `exile.rs` | 50 「[除外]」 |
//! | `status:fire` | `fire.rs` | 126 「火罐」 |
//! | `status:unstoppable` | `unstoppable.rs` | 51 「[不可阻挡]」 |
//! | `status:no_hand` | `no_hand.rs` | `noHand` companion (card text) |
//!
//! The `mark:cp` precedent (`rules/tile_marks/src/cp.rs`, `docs/TILES.md`):
//! cards never special-case a status. They call this crate's canonical API
//! (`status::stay::add`, `status::fire::spend`, …) or message the host. The
//! engine keeps only generic primitives (keyed state, turn-flow verbs, player
//! gates) and stops naming stay / stun / exile / fire at policy sites.
//!
//! ## Storage (recorded in `docs/STATUS-HOSTS.md` §3.1)
//!
//! The named state keys `stay` / `stun` / `stunStart` / `exile` / `exileTo` /
//! `exileMain` / `fire` / `noHand` / `unstoppable` remain the player-bound
//! counters **and** the frozen webui wire (`MatchPlayer.state`). Each host is
//! the creating rule of its key(s): it owns wear-off, clamping, gates and
//! clauses. Wear-off is item-carried (`ctx::state::set_expires`,
//! `Tick::TurnEnd` / `Tick::TurnStart`).
//!
//! ## Gates
//!
//! `[无法移动]` (rules.txt 47) is a **generic move gate**, not a marker.
//! Stay / stun / exile all set it while their layers > 0. The engine reads
//! gates (`act` / `move` / `pay` / `on_board` / `targetable`); the hosts'
//! clauses say what the counters mean to each gate. Because `tick_state` and
//! the turn flow run outside a card run, the gate **derivation** lives
//! engine-side in one function that only the status keys touch
//! (`game_core::engine::ops::status_gates`). The hosts own the meaning; the
//! engine owns the timing.

#![cfg_attr(target_arch = "wasm32", no_std)]

extern crate alloc;

pub mod exile;
pub mod fire;
pub mod no_hand;
pub mod stay;
pub mod stun;
pub mod unstoppable;

pub use exile::STATUS_EXILE;
pub use fire::STATUS_FIRE;
pub use no_hand::STATUS_NO_HAND;
pub use stay::STATUS_STAY;
pub use stun::STATUS_STUN;
pub use unstoppable::STATUS_UNSTOPPABLE;

/// Every status rule, in registration order. The shipped module
/// (`card-all`) concatenates these with the card, skill, tile, mark and event
/// tables.
pub static CARDS: &[card_sdk::CardDef] = &[
    STATUS_STAY,
    STATUS_STUN,
    STATUS_EXILE,
    STATUS_FIRE,
    STATUS_UNSTOPPABLE,
    STATUS_NO_HAND,
];

// --------------------------------------------------------------- canonical API
//
// One shape for every status. Thin wrappers over the generic `ctx::state`
// surface and the still-shipped engine verbs (Phase B shims; Phase C drops
// the old `ctx::{give_stay, …}` names once every card has moved).

pub mod stay_api {
    //! `status:stay` -- [停留] layers. `docs/STATUS-HOSTS.md` §1.
    use card_sdk::ctx;

    /// The state key this host owns (frozen webui wire).
    pub const KEY: &str = "stay";

    /// Add `n` [停留] layers (negative removes; floors at 0). Wear-off is the
    /// item's own `Tick::TurnEnd`. Un-latching the turn's move gate on the
    /// last layer is the host's count-changed clause, not this call site.
    pub fn add(player_id: i32, n: i32) {
        // Phase B shim: the engine verb still owns the un-latch side effect.
        // Phase C routes this through `ctx::state` + the stay host's hook.
        ctx::give_stay(player_id, n);
    }

    /// [停留] layers on the player.
    pub fn get(player_id: i32) -> i32 {
        ctx::stay_of(player_id)
    }

    /// Drop every [停留] layer.
    pub fn clear(player_id: i32) {
        let n = get(player_id);
        if n > 0 {
            add(player_id, -n);
        }
    }
}

pub mod stun_api {
    //! `status:stun` -- [晕眩] layers + `stunStart`. `docs/STATUS-HOSTS.md` §1.
    use card_sdk::ctx;

    pub const KEY: &str = "stun";
    /// The layer that only starts counting next turn (`Tick::TurnStart`).
    pub const KEY_START: &str = "stunStart";

    /// Add `n` [晕眩] layers (floors at 0). Wear-off `Tick::TurnEnd`.
    pub fn add(player_id: i32, n: i32) {
        ctx::give_stun(player_id, n);
    }

    /// [晕眩] layers (`stun` + `stunStart`, what the view shows as 眩晕).
    pub fn get(player_id: i32) -> i32 {
        ctx::stun_of(player_id)
    }

    /// Drop every [晕眩] layer (both counters).
    pub fn clear(player_id: i32) {
        let n = ctx::state::get(player_id, KEY);
        if n > 0 {
            ctx::state::add(player_id, KEY, -n);
        }
        let s = ctx::state::get(player_id, KEY_START);
        if s > 0 {
            ctx::state::add(player_id, KEY_START, -s);
        }
    }
}

pub mod exile_api {
    //! `status:exile` -- [除外] layers + return tile + `exileMain`.
    //! `docs/STATUS-HOSTS.md` §1.
    use card_sdk::ctx;

    pub const KEY: &str = "exile";
    /// Tile the return [传送] lands on, or -1 for none.
    pub const KEY_TO: &str = "exileTo";
    /// 「视为当回合的主要移动」 -- the return teleport **is** that turn's
    /// [主要移动] (MyGO:无路矢).
    pub const KEY_MAIN: &str = "exileMain";

    /// Give `n` [除外] layers; `to` is the return tile (-1 = none).
    ///
    /// TODO(规则书)（1）: 「该效果生效时无法重复获得该效果」 (`data/rules.txt`
    /// 50) -- today layers stack. Conservative reading: an already-exiled
    /// player gains no new layer. Phase C flips this in the host.
    pub fn send(player_id: i32, n: i32, to: i32) {
        ctx::give_exile(player_id, n, to);
    }

    /// [除外] layers on the player.
    pub fn get(player_id: i32) -> i32 {
        ctx::exile_of(player_id)
    }

    /// Drop every [除外] layer and forget the return tile.
    pub fn clear(player_id: i32) {
        let n = get(player_id);
        if n > 0 {
            ctx::state::add(player_id, KEY, -n);
        }
        ctx::state::set(player_id, KEY_TO, -1);
        ctx::state::set(player_id, KEY_MAIN, 0);
    }
}

pub mod fire_api {
    //! `status:fire` -- 火罐 counter + cap. `docs/STATUS-HOSTS.md` §1.
    use card_sdk::{ctx, Msg, Prompt};

    pub const KEY: &str = "fire";

    /// 火罐 held.
    pub fn get(player_id: i32) -> i32 {
        ctx::fire(player_id)
    }

    /// The mandated cap (0 = none declared).
    pub fn max(player_id: i32) -> i32 {
        ctx::fire_max(player_id)
    }

    /// Gain `n` pots, at most up to the cap; 0 if nothing. Logs the gain.
    pub fn gain(player_id: i32, n: i32, why: &Msg) -> Result<i32, Prompt> {
        ctx::gain_fire(player_id, n, why)
    }

    /// Spend `n` pots. `markerSpend` opens **before** the pots move; a
    /// cancelled window spends nothing.
    pub fn spend(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
        ctx::spend_fire(player_id, n, why)
    }

    /// Move the cap by `n`; pulls the value down under the new ceiling.
    pub fn add_max(player_id: i32, n: i32) -> i32 {
        ctx::add_fire_max(player_id, n)
    }
}

pub mod unstoppable_api {
    //! `status:unstoppable` -- [不可阻挡]. `docs/STATUS-HOSTS.md` §1.
    use card_sdk::ctx;

    pub const KEY: &str = "unstoppable";

    /// Add `n` [不可阻挡] layers.
    pub fn add(player_id: i32, n: i32) {
        ctx::state::add(player_id, KEY, n);
    }

    pub fn get(player_id: i32) -> i32 {
        ctx::state::get(player_id, KEY)
    }
}

pub mod no_hand_api {
    //! `status:no_hand` -- `noHand` companion. `docs/STATUS-HOSTS.md` §1.
    use card_sdk::ctx;

    pub const KEY: &str = "noHand";

    pub fn add(player_id: i32, n: i32) {
        ctx::state::add(player_id, KEY, n);
    }

    pub fn get(player_id: i32) -> i32 {
        ctx::state::get(player_id, KEY)
    }
}

// -------------------------------------------------------- generic gate readers
//
// Replacements for `ctx::can_pay` / `ctx::cant_move` (Phase C deletes those
// names). These are **generic** -- no status vocabulary -- and read the same
// thing the engine's policy sites read.

pub use card_sdk::ctx::{can_pay, cant_move};

/// `true` when the player may use [主动] skills and [手] effects.
/// Not out, not [晕眩], not [除外].
pub fn can_act(player_id: i32) -> bool {
    can_pay(player_id)
}

/// `None` when the player may still make this turn's [主要移动], else the
/// shared refusal (`status.move_*` messages).
pub fn move_blocked(player_id: i32) -> Option<card_sdk::Msg> {
    cant_move(player_id)
}