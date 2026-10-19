//! 火罐 -- a capped **技能指示物** (not a status; see `docs/STATUS-HOSTS.md`
//! taxonomy), owner-routed marker kernel (`docs/MARKERS.md` §3 / §7).
//!
//! 规则书（`data/rules.txt` 126）:
//! > · 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过持有上限
//!
//! MARKERS.md §3 / §7: 火罐 is **instance-owned** by the character skill that
//! prints 「（初始N，上限M）」. Declaration is structured ([`declare`] /
//! [`declare_from`]); guests (hand cards, events, the Ave Mujica regime) write
//! only through this kernel, which applies the owner's policy and returns
//! **actuals** so caller clauses keep their own text (`Sumimi:一人两个甜甜圈`
//! converts the pots that did not fit into 500资金).
//!
//! ## Clause-by-clause
//!
//! * **「技能指示物」** -- 火罐 is a named counter on the player, capped by
//!   the character skill's 「持有上限」.
//! * **「在任意时刻，您持有的火罐数量不能超过持有上限」** -- I2. Gain is partial
//!   (at most up to the cap); a cap change pulls the value down; the kernel
//!   never lets a write land above the declared max.
//! * **Gain** logs one 效果适用 entry (`log.gain_charge`): which effect, why,
//!   how many landed, the new count and cap.
//! * **Spend** opens the `markerSpend` [反击] window **before** the pots move
//!   (user ruling 2026-10-07); a cancelled window spends nothing. After a
//!   successful spend the engine raises `fireSpent` (「每当你消耗火罐时」).
//! * **`fire.temp`** (lisa's 「可超过上限的临时火罐」) is a **second declared
//!   counter** of the same instance, kernel-managed. Spend draws temp first
//!   (MARKERS.md §10(2): it is the over-cap allowance).
//!
//! 火罐 does not set any player gate -- it is a resource, not a status
//! marker. The cap is the only bound.

use card_sdk::abi::TriggerKind;
use card_sdk::{CardDef, Msg, Prompt};
use bandori_std::ctx::{self, state};

pub use crate::fire_decl::{decl_of, FireDecl, FIRE_DECLS};

/// The board-owned 火罐 rule. One instance on `BOARD_OWNER`, `tile = -1`.
/// No hooks of its own yet: the verbs below are the kernel, and the
/// `markerSpend` / `fireSpent` windows are generic engine points. A future
/// 「获得[火罐]时」 clause would hang off `markerGain`.
pub const STATUS_FIRE: CardDef = CardDef::new("status:fire", &[])
    .board_role("status");

// ------------------------------------------------------------- storage keys

/// The state key this host owns (frozen webui wire).
pub const KEY: &str = "charge";

/// lisa's 「可超过上限的临时火罐」 -- a second declared counter of the same
/// instance (MARKERS.md §7). Storage stays the named state key; writes go
/// through [`temp`] so no card invents a free key.
pub const TEMP_KEY: &str = "tempFire";

// ------------------------------------------------------------- pure policy

/// Gain `n`, at most up to the cap; returns how many landed. This is the
/// host's clause (「不能超过持有上限」).
pub fn gain_clamped(have: i32, cap: i32, n: i32) -> i32 {
    if n <= 0 || cap <= 0 {
        return 0;
    }
    n.clamp(0, (cap - have).max(0))
}

/// A cap change pulls the value down under the new ceiling
/// (「不能超过持有上限」, applied 「在任意时刻」).
pub fn pull_under_cap(have: i32, cap: i32) -> i32 {
    have.min(cap.max(0))
}

// ------------------------------------------------------------- reads

/// 火罐 held (the capped pool; does not include [`temp`]).
pub fn get(player_id: i32) -> i32 {
    bandori_std::ctx::charge(player_id)
}

/// The declared cap (0 = none declared).
pub fn max(player_id: i32) -> i32 {
    bandori_std::ctx::charge_max(player_id)
}

/// Total pots the seat can spend: the pool plus any live temp pots
/// (MARKERS.md §10(2) spend order).
pub fn total(player_id: i32) -> i32 {
    get(player_id) + temp::get(player_id)
}

// ------------------------------------------------------------- declaration

/// Write the face's 「（初始N，上限M）」.
///
/// The cap is written **only when unset** (0), so a later [`adjust_cap`]
/// (「火罐上限加1」) survives the per-turn re-hook. Initial is created at
/// `DeckAtGameStart` only (where 「初始N」 belongs).
///
/// For a swap / overlay that must **replace** the previous face's numbers,
/// use [`redeclare`].
pub fn declare(player_id: i32, initial: i32, cap: i32) {
    if state::max(player_id, KEY) == 0 {
        state::set_bounds(player_id, KEY, 0, cap.max(0));
    }
    if ctx::trigger::kind() == TriggerKind::DeckAtGameStart {
        state::set(player_id, KEY, initial.clamp(0, cap.max(0)));
    }
}

/// Force-write the face's numbers (swap 「替换为…及其初始火罐数」, mutsumi's
/// 「获得其初始火罐」 overlay). Pulls the value under the new cap (I2).
pub fn redeclare(player_id: i32, initial: i32, cap: i32) -> i32 {
    let cap = cap.max(0);
    state::set_bounds(player_id, KEY, 0, cap);
    let have = get(player_id);
    let want = pull_under_cap(initial, cap);
    // On a swap the face's 初始 is the new pool; on an overlay it is the
    // borrowed face's 初始. Either way the written value is the declaration.
    if have != want {
        state::set(player_id, KEY, want);
    }
    want
}

/// Declare this seat's own character-skill row from the registry
/// (`fire_pot`'s lookup). No-op when the face prints no fire pots.
pub fn declare_own(player_id: i32) -> bool {
    let Some(skill) = ctx::persona_skill(player_id) else {
        return false;
    };
    declare_from(player_id, &skill)
}

/// Declare from a named face's registry row (swap / overlay).
/// Returns whether a row existed.
pub fn declare_from(player_id: i32, skill: &str) -> bool {
    match decl_of(skill) {
        Some(d) => {
            declare(player_id, d.initial, d.cap);
            true
        }
        None => false,
    }
}

/// Force-write a named face's registry row (swap 「替换为…及其初始火罐数」).
pub fn redeclare_from(player_id: i32, skill: &str) -> i32 {
    match decl_of(skill) {
        Some(d) => redeclare(player_id, d.initial, d.cap),
        None => redeclare(player_id, 0, 0),
    }
}

/// Overlay a borrowed face's row (mutsumi 「获得其初始火罐」): force-write the
/// borrowed 初始/上限. On 状态2 exit call [`end_overlay`].
pub fn overlay(player_id: i32, skill: &str) -> i32 {
    match decl_of(skill) {
        Some(d) => redeclare(player_id, d.initial, d.cap),
        // No fire on the borrowed face: the overlay is an empty pool.
        None => redeclare(player_id, 0, 0),
    }
}

/// End an overlay and re-declare the seat's own face (MARKERS.md §7: her face
/// text is the ground fact). A face that prints no fire pots gets cap 0.
pub fn end_overlay(player_id: i32) -> i32 {
    match ctx::persona_skill(player_id).as_deref().and_then(decl_of) {
        Some(d) => redeclare(player_id, d.initial, d.cap),
        None => redeclare(player_id, 0, 0),
    }
}

// ------------------------------------------------------------- kernel verbs

/// Gain `n` pots, at most up to the cap. Logs the gain. Returns how many
/// landed (actuals -- `Sumimi:一人两个甜甜圈` converts the rest to 500资金).
///
/// Opens `markerGain` before the pots move (host verb, unchanged window).
/// The window payload is the **requested** `n` (today's shape); the host
/// clamps under the declared cap (I2) and returns the actual.
pub fn gain(
    player_id: i32,
    n: i32,
    why: &Msg,
) -> Result<i32, Prompt> {
    if n <= 0 {
        return Ok(0);
    }
    // I2 / 「不能超过持有上限」: the host verb opens `markerGain` with the
    // requested `n` (today's window payload), then applies the storage
    // bounds and returns the actual. `max == 0` (no declaration) refuses in
    // the host verb (an undeclared resource cannot be gained).
    bandori_std::ctx::gain_charge(player_id, n, why)
}

/// Spend `n` pots. `ChargePaying` runs first (spend substitution 「以代替此次
/// 技能的火罐消耗」), then `markerSpend` opens **before** the pots move; a
/// cancelled window spends nothing. After a successful spend the engine raises
/// `fireSpent`.
///
/// Spend draws [`temp`] pots first (§10(2): it is the over-cap allowance),
/// then the pool. Returns whether the full cost was paid.
///
/// The `ChargePaying` window rides the pool spend verb and opens **even when
/// the pool is empty** (the substitution path). A pre-check on the pool would
/// skip the window and break 「以代替此次技能的火罐消耗」.
pub fn spend(
    player_id: i32,
    n: i32,
    why: &Msg,
) -> Result<bool, Prompt> {
    if n <= 0 {
        return Ok(true);
    }
    let from_temp = temp::get(player_id).min(n);
    let from_pool = n - from_temp;
    // Temp first: the over-cap allowance is spent before the declared pool.
    if from_temp > 0 {
        temp::spend(player_id, from_temp, why)?;
    }
    if from_pool > 0 {
        // Opens ChargePaying + markerSpend for `from_pool`, then writes.
        // A substitution cancels the write and counts as paid.
        return bandori_std::ctx::spend_charge(player_id, from_pool, why);
    }
    Ok(true)
}

/// Move the cap by `n`; pulls the value down under the new ceiling (I2).
/// Returns the new cap.
pub fn adjust_cap(player_id: i32, n: i32) -> i32 {
    bandori_std::ctx::add_charge_max(player_id, n)
}

/// Backwards-compatible alias of [`adjust_cap`] (the old `add_max` name).
pub fn add_max(player_id: i32, n: i32) -> i32 {
    adjust_cap(player_id, n)
}

// ------------------------------------------------------------- regime protocol

/// Named regime guests (MARKERS.md §4.3) over the **owner's** pool -- never a
/// raw write. `skill:Ave Mujica:假面之下的真实` is the standing user:
/// 「进入状态2时将火罐补充至上限且每回合结束时失去1火罐…退出状态2时，失去所有
/// 剩余火罐」.
pub mod regime {
    use super::*;
    use card_sdk::Msg;

    /// Live pots in the regime's pool (the declared counter).
    pub fn count(player_id: i32) -> i32 {
        get(player_id)
    }

    /// 「将火罐补充至上限」 -- fill the pool to the declared cap. Returns how
    /// many landed.
    pub fn fill_to_cap(player_id: i32, why: &Msg) -> Result<i32, Prompt> {
        let room = (max(player_id) - get(player_id)).max(0);
        gain(player_id, room, why)
    }

    /// 「每回合结束时失去1火罐」 -- drain `n` from the pool (not temp).
    /// Returns how many actually left.
    pub fn drain(player_id: i32, n: i32, why: &Msg) -> Result<i32, Prompt> {
        let have = get(player_id);
        let take = n.clamp(0, have);
        if take <= 0 {
            return Ok(0);
        }
        if !bandori_std::ctx::spend_charge(player_id, take, why)? {
            return Ok(0);
        }
        Ok(take)
    }

    /// 「退出状态2时，失去所有剩余火罐」 -- burn the pool (not temp) down to 0.
    /// Returns how many left.
    pub fn burn_all(player_id: i32, why: &Msg) -> Result<i32, Prompt> {
        let have = get(player_id);
        drain(player_id, have, why)
    }
}

// ------------------------------------------------------------- temp pots

/// lisa's 「可超过上限的临时火罐」 -- a second declared counter of the same
/// instance (MARKERS.md §7). Kernel-managed, not a free key.
///
/// TODO(规则书)[judgement]: （3）「你提供的临时火罐将会在两回合后失去，使用你
///   提供的临时火罐需要支付你600资金」 -- the spend path has to know a pot's
///   *provenance* to charge the provider, and 「被冲榜类效果被动消耗无需支付」
///   carves out a class of spends the engine does not tag. The counter itself
///   is real; the 600-cost and the two-turn expiry stay open.
pub mod temp {
    use super::*;
    use card_sdk::Msg;

    /// Temp pots held.
    pub fn get(player_id: i32) -> i32 {
        state::get(player_id, TEMP_KEY)
    }

    /// The temp ceiling. lisa's face allows exceeding the pool cap, so this
    /// is unbounded by default; the kernel still owns the key.
    pub fn max(player_id: i32) -> i32 {
        state::max(player_id, TEMP_KEY)
    }

    /// Grant `n` temp pots (「获得一个可超过上限的临时火罐」). These are the
    /// over-cap allowance: they do **not** count against the pool cap (I2 is
    /// about the declared pool). Returns how many landed under the temp
    /// ceiling.
    pub fn grant(player_id: i32, n: i32) -> i32 {
        if n <= 0 {
            return 0;
        }
        // First grant arms the over-cap allowance (unbounded by the pool cap).
        if max(player_id) == 0 {
            state::set_max(player_id, TEMP_KEY, i32::MAX);
        }
        // The temp ceiling is its own bound (default i32::MAX); the pool cap
        // does not apply -- 「可超过上限」 is the whole point.
        let room = (max(player_id) - get(player_id)).max(0);
        let got = n.min(room);
        if got > 0 {
            state::add(player_id, TEMP_KEY, got);
        }
        got
    }

    /// Spend `n` temp pots. Returns whether the full amount left.
    pub fn spend(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
        if n <= 0 {
            return Ok(true);
        }
        if get(player_id) < n {
            return Ok(false);
        }
        state::add(player_id, TEMP_KEY, -n);
        ctx::log(
            player_id,
            &Msg::new("log.spend_charge")
                .player_id("who", player_id)
                .msg("why", why)
                .i("n", n as i64)
                .i("charge", get(player_id) as i64)
                .i("max", max(player_id) as i64),
        );
        Ok(true)
    }
}

