//! 火罐 -- a capped **技能指示物** (not a status; see `docs/STATUS-HOSTS.md`
//! taxonomy), owner-routed marker kernel (`docs/MARKERS.md` §3 / §7).
//!
//! 规则书（`data/rules.txt` 126）:
//! > · 火罐：技能指示物，在任意时刻，您持有的火罐数量不能超过持有上限
//!
//! MARKERS.md §3 / §7: 火罐 is **instance-owned** by the character skill that
//! prints 「（初始N，上限M）」. That face owns the numbers: it declares them on
//! its own [`CardDef`] as the [`FIRE_INITIAL`] / [`FIRE_CAP`] props.
//! Guests (hand cards, events, the Ave Mujica regime) write only through this
//! kernel, which applies the owner's policy and returns **actuals** so caller
//! clauses keep their own text (`Sumimi:一人两个甜甜圈` converts the pots that
//! did not fit into 500资金).
//!
//! ## Clause-by-clause
//!
//! * **「技能指示物」** -- 火罐 is a named counter **of the persona-skill
//!   instance**, held by the player, capped by that face's 「持有上限」.
//!   Storage is the seat's `tokens` row (`name = "charge"`, `instance =
//!   persona-skill uid`); the row dies with the skill instance
//!   (`destroy_instance_units`).
//! * **「在任意时刻，您持有的火罐数量不能超过持有上限」** -- I2. This kernel
//!   polices the cap (the engine stores the advertised `max` for display but
//!   does **not** clamp to it). Gain is partial (at most up to the cap); a cap
//!   change pulls the value down; the kernel never lets a write land above the
//!   declared max.
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

use crate::ctx;
use card_sdk::abi::TriggerKind;
use card_sdk::{Msg, Prompt};

// ------------------------------------------------------------- declaration keys

/// 「（初始N，上限M）」 -- the face's initial pot count, as a `CardDef` prop.
/// Read back with `ctx::card_prop(skill, FIRE_INITIAL)`.
pub const FIRE_INITIAL: &str = "fireInitial";

/// 「（初始N，上限M）」 -- the face's held cap, as a `CardDef` prop.
/// Read back with `ctx::card_prop(skill, FIRE_CAP)`.
pub const FIRE_CAP: &str = "fireCap";

// The numbers live on the skill that prints them (MARKERS.md §7: the face
// printing the numbers owns the pool), declared on its own `CardDef` as the
// two props above -- no central table, no engine field:
//
// ```
// pub const ANON_RESTART: CardDef = CardDef::new("skill:千早爱音:重新开始", &[…])
//     // 「（初始2，上限3）」
//     .props(&[(fire::FIRE_INITIAL, 2), (fire::FIRE_CAP, 3)]);
// ```
//
// Cap-only faces (「（上限7）」) pass `initial = 0`. [`declare_own`] /
// [`declare_from`] / [`overlay`] read them back through `ctx::card_prop`. A
// skill with no [`FIRE_INITIAL`] / [`FIRE_CAP`] props owns no pots (cap 0).

// ------------------------------------------------------------- storage keys

/// The held-token name this kernel owns (frozen webui wire).
pub const KEY: &str = "charge";

/// lisa's 「可超过上限的临时火罐」 -- a second declared counter of the same
/// instance (MARKERS.md §7). Storage is a second `tokens` row of the persona
/// skill; writes go through [`temp`] so no card invents a free key.
pub const TEMP_KEY: &str = "tempFire";

// ------------------------------------------------------------- ownership

/// The persona-skill field instance that owns this seat's fire pools.
/// Resolved through the existing guest lookup (`persona_skill` + `find_card`)
/// -- no dedicated host import. -1 when the seat has no persona skill.
fn skill_uid(player_id: i32) -> i32 {
    let Some(skill) = ctx::persona_skill(player_id) else {
        return -1;
    };
    ctx::find_card(player_id, &skill).unwrap_or(-1)
}

/// Does a live row already exist? A declared pool keeps its row even at
/// `value == 0` (the `max > 0` latch); an undeclared name has neither.
fn row_live(player_id: i32, name: &str) -> bool {
    ctx::tok(player_id, name) > 0 || ctx::tok_max(player_id, name) > 0
}

/// Can this seat hold fire? A persona-skill instance owns the pool; a row
/// that is already live (declared, or arranged by a test harness) is also
/// writable so guests do not have to re-bind it. A seat with neither owns no
/// pots -- every write is a no-op and every read is 0.
fn writable(player_id: i32) -> bool {
    skill_uid(player_id) >= 0 || row_live(player_id, KEY)
}

/// Write the named row of the persona-skill instance. `max` is the production
/// tri-state: `>= 0` writes that ceiling (0 clears it and drops the row),
/// `< 0` leaves any stored ceiling alone. Used by `adjust_cap` even before a
/// pool exists (it advertises the ceiling); `gain` / `spend` go through
/// [`writable`].
fn set_row(player_id: i32, name: &str, value: i32, max: i32) {
    let uid = skill_uid(player_id);
    ctx::set_tok(player_id, name, value, max, uid);
}

// ------------------------------------------------------------- pure policy

/// Gain `n`, at most up to the cap; returns how many landed. This is the
/// kernel's clause (「在任意时刻，您持有的火罐数量不能超过持有上限」). The
/// engine stores the advertised cap but does not police it -- this is the
/// arithmetic.
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
    if !writable(player_id) {
        return 0;
    }
    ctx::tok(player_id, KEY)
}

/// The declared cap (0 = none declared).
pub fn max(player_id: i32) -> i32 {
    if !writable(player_id) {
        return 0;
    }
    ctx::tok_max(player_id, KEY)
}

/// Total pots the seat can spend: the pool plus any live temp pots
/// (MARKERS.md §10(2) spend order).
pub fn total(player_id: i32) -> i32 {
    get(player_id) + temp::get(player_id)
}

// ------------------------------------------------------------- declaration

/// The face's 「（初始N，上限M）」 off its `CardDef` props. Both default to 0
/// when the face declares no [`FIRE_INITIAL`] / [`FIRE_CAP`] props (owns no pots).
fn face_bounds(skill: &str) -> (i32, i32) {
    (
        ctx::card_prop(skill, FIRE_INITIAL),
        ctx::card_prop(skill, FIRE_CAP),
    )
}

/// Write the face's 「（初始N，上限M）」.
///
/// The `Counter.max` write is the **advertisement** (the UI shows 「3/5」 from
/// it); the engine stores it and does not police it. The cap is written
/// **only when the token row does not yet exist**, so a later [`adjust_cap`]
/// (「火罐上限加1」) survives the per-turn re-hook -- the live row *is* the
/// "already declared" latch. Initial is created at `DeckAtGameStart` only
/// (where 「初始N」 belongs).
///
/// For a swap / overlay that must **replace** the previous face's numbers,
/// use [`redeclare`].
pub fn declare(player_id: i32, initial: i32, cap: i32) {
    if !writable(player_id) {
        return;
    }
    // Cap only on first declaration (row absent). A per-turn `TurnStartBefore`
    // re-hook finds the live row and leaves the ceiling alone -- `adjust_cap`'s
    // 「火罐上限加1」 must survive it.
    if !row_live(player_id, KEY) {
        set_row(player_id, KEY, 0, cap.max(0));
    } else {
        // Row already live (an `adjust_cap` ceiling, or a swap remnant): keep
        // its ceiling and value, but re-stamp the owner so it still dies with
        // the persona-skill instance.
        set_row(player_id, KEY, get(player_id), -1);
    }
    if ctx::trigger::kind() == TriggerKind::DeckAtGameStart {
        let live = max(player_id);
        set_row(player_id, KEY, initial.clamp(0, live.max(0)), live);
    }
}

/// Force-write the face's numbers (swap 「替换为…及其初始火罐数」, mutsumi's
/// 「获得其初始火罐」 overlay). Pulls the value under the new cap (I2) and
/// rebinds the row to the persona-skill instance.
pub fn redeclare(player_id: i32, initial: i32, cap: i32) -> i32 {
    let cap = cap.max(0);
    let want = pull_under_cap(initial, cap);
    // On a swap the face's 初始 is the new pool; on an overlay it is the
    // borrowed face's 初始. Either way the written value is the declaration.
    set_row(player_id, KEY, want, cap);
    want
}

/// Declare this seat's own character-skill numbers from its `CardDef` props
/// (`ctx::card_prop`). No-op when the face prints no fire pots.
pub fn declare_own(player_id: i32) -> bool {
    let Some(skill) = ctx::persona_skill(player_id) else {
        return false;
    };
    declare_from(player_id, &skill)
}

/// Declare from a named face's `CardDef` props (swap / overlay).
/// Returns whether the face declares any fire pots.
pub fn declare_from(player_id: i32, skill: &str) -> bool {
    let (initial, cap) = face_bounds(skill);
    if cap <= 0 && initial <= 0 {
        return false;
    }
    declare(player_id, initial, cap);
    true
}

/// Force-write a named face's `CardDef` props (swap 「替换为…及其初始火罐数」).
pub fn redeclare_from(player_id: i32, skill: &str) -> i32 {
    let (initial, cap) = face_bounds(skill);
    redeclare(player_id, initial, cap)
}

/// Overlay a borrowed face's numbers (mutsumi 「获得其初始火罐」): force-write
/// the borrowed 初始/上限. On 状态2 exit call [`end_overlay`].
pub fn overlay(player_id: i32, skill: &str) -> i32 {
    // No fire on the borrowed face: the overlay is an empty pool.
    redeclare_from(player_id, skill)
}

/// End an overlay and re-declare the seat's own face (MARKERS.md §7: her face
/// text is the ground fact). A face that prints no fire pots gets cap 0.
pub fn end_overlay(player_id: i32) -> i32 {
    match ctx::persona_skill(player_id) {
        Some(skill) => redeclare_from(player_id, &skill),
        None => redeclare(player_id, 0, 0),
    }
}

// ------------------------------------------------------------- kernel verbs

/// Gain `n` pots, at most up to the cap. Logs the gain. Returns how many
/// landed (actuals -- `Sumimi:一人两个甜甜圈` converts the rest to 500资金).
///
/// 「在任意时刻，您持有的火罐数量不能超过持有上限」 -- this kernel does the
/// arithmetic (`gain_clamped`); the engine stores the advertised cap but does
/// **not** police it. Opens `markerGain` before the pots move (host verb,
/// unchanged window). The window payload is the **actual** `want` (what will
/// land); a cancelled window moves nothing.
pub fn gain(player_id: i32, n: i32, why: &Msg) -> Result<i32, Prompt> {
    if n <= 0 {
        return Ok(0);
    }
    if !writable(player_id) {
        return Ok(0);
    }
    // I2: clamp here, not in the engine. `max == 0` (no declaration) refuses
    // (an undeclared resource cannot be gained).
    let have = get(player_id);
    let cap = max(player_id);
    let want = gain_clamped(have, cap, n);
    if want <= 0 {
        // Refuse before the write (and before the window): nothing would
        // move. Narrate -- 「不能超过持有上限」.
        ctx::log(
            player_id,
            &Msg::new("log.gain_charge_capped")
                .player_id("who", player_id)
                .msg("why", why)
                .i("charge", have as i64)
                .i("max", cap as i64),
        );
        return Ok(0);
    }
    // Ensure the row is bound to the persona-skill instance before the dumb
    // add (a fresh row would otherwise stamp the guest's uid).
    if !row_live(player_id, KEY) {
        set_row(player_id, KEY, 0, cap);
    }
    // Window payload is the actual `want` (the amount that will land);
    // a cancelled window writes nothing. The host verb is a dumb add and
    // logs `log.gain_charge` with the actual.
    ctx::gain_marker(player_id, KEY, want, why)
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
pub fn spend(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
    if n <= 0 {
        return Ok(true);
    }
    if !writable(player_id) {
        return Ok(false);
    }
    let from_temp = temp::get(player_id).min(n);
    let from_pool = n - from_temp;
    // Temp first: the over-cap allowance is spent before the declared pool.
    if from_temp > 0 {
        temp::spend(player_id, from_temp, why)?;
    }
    if from_pool > 0 {
        return spend_pool(player_id, from_pool, why);
    }
    Ok(true)
}

/// Pool-only spend (never temp): `ChargePaying` then `markerSpend`, then the
/// write. The substitution window opens **even when the pool is empty**.
fn spend_pool(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
    if n <= 0 {
        return Ok(true);
    }
    if !ctx::raise(player_id, crate::kind::ChargePaying, n)? {
        return Ok(true);
    }
    Ok(ctx::spend_marker(player_id, KEY, n, why)? != 0)
}

/// Move the cap by `n`; pulls the value down under the new ceiling (I2
/// 「在任意时刻」). Returns the new cap.
///
/// Allowed even when the seat has no fire skill (「此卡拥有者火罐上限加1」
/// creates the ceiling over a baseline of 0); [`gain`] still refuses until
/// there is a live pool.
///
/// One `set_row` does both jobs -- it binds the row to the persona-skill
/// instance and advertises the ceiling. Going through `add_marker_max` instead
/// would create an **unowned** row (it preserves an existing owner and stamps
/// `-1` on a fresh one), which `destroy_instance_units` then never drops.
pub fn adjust_cap(player_id: i32, n: i32) -> i32 {
    let cap = max(player_id).saturating_add(n).max(0);
    let want = pull_under_cap(get(player_id), cap);
    set_row(player_id, KEY, want, cap);
    cap
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
        if !spend_pool(player_id, take, why)? {
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
        if !writable(player_id) {
            return 0;
        }
        ctx::tok(player_id, TEMP_KEY)
    }

    /// The temp ceiling. lisa's face allows exceeding the pool cap, so this
    /// is unbounded by default; the kernel still owns the key.
    pub fn max(player_id: i32) -> i32 {
        if !writable(player_id) {
            return 0;
        }
        ctx::tok_max(player_id, TEMP_KEY)
    }

    /// Grant `n` temp pots (「获得一个可超过上限的临时火罐」). These are the
    /// over-cap allowance: they do **not** count against the pool cap (I2 is
    /// about the declared pool). Returns how many landed under the temp
    /// ceiling.
    pub fn grant(player_id: i32, n: i32) -> i32 {
        if n <= 0 || !writable(player_id) {
            return 0;
        }
        // First grant arms the over-cap allowance (unbounded by the pool cap).
        if !row_live(player_id, TEMP_KEY) {
            set_row(player_id, TEMP_KEY, 0, i32::MAX);
        }
        // The temp ceiling is its own bound (default i32::MAX); the pool cap
        // does not apply -- 「可超过上限」 is the whole point.
        let room = (max(player_id) - get(player_id)).max(0);
        let got = n.min(room);
        if got > 0 {
            // `-1` = leave the stored temp ceiling alone (its own bound).
            set_row(player_id, TEMP_KEY, get(player_id) + got, -1);
        }
        got
    }

    /// Spend `n` temp pots. Returns whether the full amount left.
    pub fn spend(player_id: i32, n: i32, why: &Msg) -> Result<bool, Prompt> {
        if n <= 0 {
            return Ok(true);
        }
        if !writable(player_id) {
            return Ok(false);
        }
        if get(player_id) < n {
            return Ok(false);
        }
        let left = get(player_id) - n;
        // `-1` = leave the stored temp ceiling alone.
        set_row(player_id, TEMP_KEY, left, -1);
        ctx::log(
            player_id,
            &Msg::new("log.spend_charge")
                .player_id("who", player_id)
                .msg("why", why)
                .i("n", n as i64)
                .i("charge", left as i64)
                .i("max", max(player_id) as i64),
        );
        Ok(true)
    }
}
