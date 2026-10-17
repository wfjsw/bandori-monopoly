//! Cross-card fan-marker channel for `P✽P粉丝` (Pastel✽Palettes band skill).
//!
//! The markers are units of the band-skill card's named counter. **Only**
//! [`crate::pastel`] creates or moves them; every other card / persona skill
//! sends these messages (see `docs/CARDS.md` → 「Pastel fans owned by the band
//! skill」). No band-skill instance on the field → reply 0 → caller does
//! nothing.

use ruleset_core::ctx;

/// Face-up fan counter (C# `H.FansUp`).
pub const UP: &str = "P✽P粉丝(正)";
/// Face-down fan counter (C# `H.FansDown`).
pub const DOWN: &str = "P✽P粉丝(反)";
/// The owning band-skill rule id.
pub const BAND: &str = "skill:Pastel✽Palettes:与偶像一起";

/// `Message.a` / `b` for [`grant`]: face-up / face-down counts to add.
pub const MSG_GRANT: &str = "fansGrant";
/// `Message.seat` = holder, `a` = effect X (「将X个…」), `b` = direction
/// (0 = down→up 「变正」, 1 = up→down 「变反」).
///
/// Wire reply is `1 + units moved` so a handled-but-empty reply (1) is
/// distinct from "no receiver" (0) — `ctx::send` returns the raw reply and
/// [`send_fans`] subtracts the 1. Without that, a 0-flip handler would look
/// absent and the fallback probe would run `on_fans` twice (double overflow).
pub const MSG_FLIP: &str = "fansFlip";
/// Direction bit: face-down → face-up (「变正」).
pub const DIR_UP: i32 = 0;
/// Direction bit: face-up → face-down (「变反」).
pub const DIR_DOWN: i32 = 1;

/// Grant `up` face-up / `down` face-down fans to `seat`. Reply = units granted.
pub fn grant(seat: i32, up: i32, down: i32) -> Result<i32, card_sdk::Prompt> {
    send_fans(MSG_GRANT, seat, up, down)
}

/// Flip `x` of `seat`'s fans in `dir` (effect X, not the clamped count).
/// Reply = units actually flipped.
pub fn flip(seat: i32, x: i32, dir: i32) -> Result<i32, card_sdk::Prompt> {
    send_fans(MSG_FLIP, seat, x, dir)
}

/// Flip `x` face-down → face-up (「将X个反面[P✽P粉丝]变正」).
pub fn flip_up(seat: i32, x: i32) -> Result<i32, card_sdk::Prompt> {
    flip(seat, x, DIR_UP)
}

/// Flip `x` face-up → face-down (「将X个正面[P✽P粉丝]变反」).
pub fn flip_down(seat: i32, x: i32) -> Result<i32, card_sdk::Prompt> {
    flip(seat, x, DIR_DOWN)
}

/// Address the seat's own band card first (it owns clause （2） for that seat);
/// fall back to the first band card on any seat so non-PP holders' markers
/// still move. No band card anywhere → 0.
///
/// Goes through `ctx::send_card_msg` (typed ints + interned name — no
/// `String`/postcard under Kani). The wire reply is `1 + units` (see
/// [`MSG_FLIP`]); this maps it back to units moved and stops at the first
/// receiver (0 = nobody answered).
fn send_fans(name: &str, seat: i32, a: i32, b: i32) -> Result<i32, card_sdk::Prompt> {
    let r = card_sdk::ctx::send_card_msg(BAND, seat, name, seat, a, b)?;
    if r != 0 {
        return Ok(r - 1);
    }
    for p in 0..ctx::player_count() {
        if p == seat {
            continue;
        }
        let r = card_sdk::ctx::send_card_msg(BAND, p, name, seat, a, b)?;
        if r != 0 {
            return Ok(r - 1);
        }
    }
    Ok(0)
}