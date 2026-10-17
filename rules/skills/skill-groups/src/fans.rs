//! PP fan counters in the **per-player state dict** (`ctx::state`).
//!
//! The markers are two keyed values on each seat (`ppFansUp` / `ppFansDown`),
//! owned by no card. Grants and flips go through these helpers so every site
//! shares one key spelling and one flip raise. Band clause （2） is an
//! `On::Action` listener on [`ruleset_core::kind::FansFlip`], not an inline
//! check at each spend site.
//!
//! `FansFlip` is raised **before** the dict writes, with `trigger::value()` =
//! the effect's intended X, so a listener still sees the pre-flip face-down
//! count. A cancelled / aborted raise leaves the counters untouched.

use ruleset_core::ctx;
use ruleset_core::kind;
use card_sdk::Prompt;

/// Face-up fan counter (C# `H.FansUp`).
pub const UP: &str = "ppFansUp";
/// Face-down fan counter (C# `H.FansDown`).
pub const DOWN: &str = "ppFansDown";

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