//! `mark:pp_fans` -- the P✽P粉丝 **kind-owned host** (MARKERS.md §7).
//!
//! 规则书 (Pastel✽Palettes band sheet):
//! > （1）游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
//! > Pastel✽Palettes乐队卡时此效果不重复发动）。
//!
//! and the glossary (`data/rules.txt` 「标志物解释」):
//! > · P✽P粉丝：Pastel✽Palettes乐队标
//!
//! Holder-bound counters (`ppFansUp` / `ppFansDown` on each seat) -- storage
//! stays the frozen webui wire keys; ownership, lifetime and protocol are the
//! semantics layered on top (MARKERS.md §5). This host owns:
//!
//! * the two rows and the flip protocol (pre-raise, so listeners see the
//!   pre-flip face-down count);
//! * the once-per-match initial grant (「多张…乐队卡时此效果不重复发动」).
//!
//! The Pastel band skill keeps only what its face binds to it: clause （1） as
//! a creator call to [`initial_grant_once`], clause （2） as its listener (per
//! its **own owner seat** -- 「你…受到」), and the overflow crystals on itself
//! (「为此卡」). Persona skills and `card-pp/**` cards keep calling
//! [`grant`] / [`flip_up`] / [`flip_down`] -- the facade signatures do not
//! change (`rules/skills/skill-groups/src/fans.rs` re-exports them).
//!
//! **Two affiliations are not the same thing** (the faces distinguish them):
//!
//! * 「非Pastel✽Palettes**角色**」 -- a test on the **character's** group
//!   ([`ctx::in_group`]). That is the exclusion in [`initial_grant_once`].
//! * 「多张Pastel✽Palettes**乐队卡**时此效果不重复发动」 -- about **band-skill
//!   instances**. That is the once-flag (creation is once-per-match, not
//!   once-per-instance).
//!
//! A seat may be either, both, or neither. The initial grant tests the first;
//! the dedupe tests the second; band clause （2） is per band instance.
//!
//! `FansFlip` is raised **before** the dict writes, with `trigger::value()` =
//! the effect's intended X, so a listener still sees the pre-flip face-down
//! count. A cancelled / aborted raise leaves the counters untouched.

use bandori_std::ctx;
use bandori_std::kind;
use card_sdk::{CardDef, Prompt};

/// The board-owned P✽P粉丝 rule. One instance on `BOARD_OWNER`, `tile = -1`.
/// No hooks of its own: the once-per-match deal is a creator call from the
/// Pastel band skill's clause （1）, and the flip protocol is the [`flip_up`]
/// verb. Present so `mark_rule_ids` stamps the owner (MARKERS.md §3).
pub const MARK_PP_FANS: CardDef = CardDef::new("mark:pp_fans", &[]).board_role("mark");

/// Face-up fan counter (C# `H.FansUp`).
pub const UP: &str = "ppFansUp";
/// Face-down fan counter (C# `H.FansDown`).
pub const DOWN: &str = "ppFansDown";

/// Once-per-match initial-grant flag (「此效果不重复发动」). Held on seat 0 as
/// the canonical flag seat -- the host is board-wide, and the flag is creation
/// idempotence (MARKERS.md §4.5), not a per-seat latch.
const ONCE: &str = "mark.pp_fans.initial";

// ------------------------------------------------------------- initial grant

/// 「游戏开始时非Pastel✽Palettes角色获得1个反面[P✽P粉丝]（如有场上有多张
/// Pastel✽Palettes乐队卡时此效果不重复发动）」 -- the host's once-per-match
/// creation protocol (MARKERS.md §4.5).
///
/// * `exclude_group` is the **character** group the face names
///   (「非Pastel✽Palettes角色」). It stays with the face text; the host only
///   runs the loop.
/// * The 「多张…乐队卡」 dedupe is this once-flag: any number of band-card
///   copies call this; the first deal wins and the rest are no-ops.
///
/// Returns how many seats received a fan.
pub fn initial_grant_once(exclude_group: &str) -> Result<i32, Prompt> {
    if already_granted() {
        return Ok(0);
    }
    // Stamp before the loop so a re-entrant second copy never double-deals.
    ctx::state::set(0, ONCE, 1);
    let mut n = 0;
    for p in 0..ctx::player_count() {
        if ctx::player_out(p) || ctx::in_group(p, exclude_group) {
            continue;
        }
        n += grant(p, 0, 1)?;
    }
    Ok(n)
}

/// Has the once-per-match initial grant already run?
pub fn already_granted() -> bool {
    // Seat 0 is the canonical flag seat; also accept any seat holding the
    // stamp so a world that lost seat 0 still dedupes.
    ctx::state::get(0, ONCE) > 0 || (0..ctx::player_count()).any(|p| ctx::state::get(p, ONCE) > 0)
}

// ------------------------------------------------------------- counters

/// Grant `up` face-up / `down` face-down fans to `seat`. No raise.
/// Returns the number of units added.
pub fn grant(seat: i32, up: i32, down: i32) -> Result<i32, Prompt> {
    let up = up.max(0);
    let down = down.max(0);
    if up > 0 {
        ctx::state::add(seat, UP, up);
    }
    if down > 0 {
        ctx::state::add(seat, DOWN, down);
    }
    Ok(up + down)
}

/// Flip `x` face-down → face-up (「将X个反面[P✽P粉丝]变正」).
/// Returns the number actually flipped.
pub fn flip_up(seat: i32, x: i32) -> Result<i32, Prompt> {
    if x <= 0 {
        return Ok(0);
    }
    let down = ctx::state::get(seat, DOWN);
    let flip = x.min(down).max(0);
    // Raise first so a clause-（2） listener sees `down` pre-flip. `value` is
    // the effect's intended X (even when X > down). Cancelled / aborted
    // (`Ok(false)` / `Err`) leaves both counters untouched -- never half-move.
    if !ctx::raise(seat, kind::FansFlip, x)? {
        return Ok(0);
    }
    if flip > 0 {
        ctx::state::add(seat, DOWN, -flip);
        ctx::state::add(seat, UP, flip);
    }
    Ok(flip)
}

/// Flip `x` face-up → face-down (「将X个正面[P✽P粉丝]变反」). No `FansFlip`
/// raise (clause （2） is only 「变正」).
pub fn flip_down(seat: i32, x: i32) -> Result<i32, Prompt> {
    if x <= 0 {
        return Ok(0);
    }
    let up = ctx::state::get(seat, UP);
    let flip = x.min(up).max(0);
    if flip > 0 {
        ctx::state::add(seat, UP, -flip);
        ctx::state::add(seat, DOWN, flip);
    }
    Ok(flip)
}

/// Face-up count on `seat`.
pub fn up(seat: i32) -> i32 {
    ctx::state::get(seat, UP)
}

/// Face-down count on `seat`.
pub fn down(seat: i32) -> i32 {
    ctx::state::get(seat, DOWN)
}

/// Total fans on `seat`.
pub fn total(seat: i32) -> i32 {
    up(seat) + down(seat)
}
