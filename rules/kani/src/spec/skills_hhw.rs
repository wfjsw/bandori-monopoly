//! Unit `skills_hhw`: reference specs FROM the clause docs only.
//!
//! Sources: the roster character row `text` (band = Hello, Happy World!) and
//! the roster band row `text` (传播笑容). Never derived from engine behaviour.
//! Clause IDs are `C-HHW-*` (see `docs/rulebook/spec/cards/hhw.md`).

use crate::world::Effect;

// ===========================================================================
// C-HHW-kokoro-01 -- 「（1）开局时获得1000资金」
// (specs live in `spec::hhw`; re-exported here for the skills unit table)
// ===========================================================================
pub use crate::spec::hhw::{
    band_01_accepted_log_ok, band_01_amount, band_01_amount_mutant, band_01_declined_log_ok,
    kokoro_01_amount, kokoro_01_amount_mutant, kokoro_01_log_ok, kokoro_02_amount,
    kokoro_02_amount_mutant, kokoro_02_log_ok, misaki_skill_01_cap, misaki_skill_01_gain,
    misaki_skill_01_gain_mutant, misaki_skill_01_log_ok,
};

// ===========================================================================
// C-HHW-kaoru-01
// 「（1）每次[经过]或被[经过]时，若场上不存在[怪盗标记]，获得1火罐（上限7）；
//    若场上存在[怪盗标记]，则移除场上的一个[怪盗标记]」
// Exactly one of the two arms, never both.
// ===========================================================================

pub fn kaoru_01_cap() -> i32 {
    7
}

/// No mark on the field: +1 火罐, clamped at 7.
pub fn kaoru_01_pot_gain(pre_fire: i32) -> i32 {
    (pre_fire + 1).min(kaoru_01_cap())
}

/// A mark exists: one mark is removed, no pot is gained.
pub fn kaoru_01_removes_one_mark(pre_marks: i32) -> i32 {
    (pre_marks - 1).max(0)
}

/// The pot arm logs exactly one GainFire(+1).
pub fn kaoru_01_pot_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n } if p == player && n == 1)
}

/// The mark arm produces no GainFire. (The mark write is a mark/counter op
/// whose `Effect` variant is mark-specific; the distinguishing fact the book
/// pins is 「不获得火罐」.)
pub fn kaoru_01_mark_arm_no_fire(effects: &[Effect], player: i32) -> bool {
    !effects
        .iter()
        .any(|e| matches!(e, Effect::GainFire { player: p, .. } if *p == player))
}

/// MUTANT: the pot arm also removes a mark (both arms at once).
pub fn kaoru_01_pot_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.iter().any(|e| matches!(e, Effect::GainFire { player: p, n } if *p == player && *n == 1))
        && effects.len() >= 2
}

// ===========================================================================
// C-HHW-kaoru-02
// 「（2）主要阶段可消耗7火罐在绝对距离最近的一名玩家场上放置3个[怪盗标记]，
//    并使你的本次移动以微笑号为起点。」
// ===========================================================================

pub fn kaoru_02_cost() -> i32 {
    7
}

pub fn kaoru_02_marks() -> i32 {
    3
}

/// 微笑号 is board 序号 #27 → 0-based index 26.
pub const KAORU_SMILE_SHIP: i32 = 26;

/// Spend exactly 7 fire; place exactly 3 怪盗标记 on the nearest player's
/// field; the move starts from 微笑号.
/// `spend_fire` is not an `Effect` variant under the host (it returns a bool
/// and mutates `fire[]`), so the observable is the post fire count.
pub fn kaoru_02_fire_spent(pre_fire: i32) -> i32 {
    (pre_fire - kaoru_02_cost()).max(0)
}

/// MUTANT: cost 6.
pub fn kaoru_02_cost_mutant() -> i32 {
    6
}

// ===========================================================================
// C-HHW-hagumi-skill-05
// 「角色每拥有一个可乐饼，移动时多投掷1个1d2。」
// Extra movement dice: `n` extra `1d2` where `n` = croquette count.
// ===========================================================================

pub fn hagumi_skill_05_extra_dice(croquettes: i32) -> i32 {
    croquettes.max(0)
}

/// MUTANT: half as many extra dice.
pub fn hagumi_skill_05_extra_dice_mutant(croquettes: i32) -> i32 {
    croquettes.max(0) / 2
}

// ===========================================================================
// C-HHW-kanon-01 -- 「(1) 游戏开始起点为#30弦卷豪宅」
// 弦卷豪宅 = 序号 #30 → 0-based 29.
// BLOCKED-HOST on the whole skill row (`trig_move_roll`), but (01) is a
// start-position write that does not need the blocked op. Provable in
// isolation via the DeckBeforeGame entry.
// ===========================================================================

pub const KANON_START: i32 = 29; // #30 弦卷豪宅

pub fn kanon_01_start_ok(post_pos: i32) -> bool {
    post_pos == KANON_START
}

/// MUTANT: start on CiRCLE (index 0).
pub fn kanon_01_start_ok_mutant(post_pos: i32) -> bool {
    post_pos == 0
}

// ===========================================================================
// C-HHW-kanon-02 -- 「(2) 移动阶段进行两次移动掷骰并将它们的结果相减作为你的移动格数」
// Determined: two movement rolls, not one 1d20. The sign of `a - b` is
// OPEN-HHW-KANON-NEG (text does not say |a−b| vs signed).
// BLOCKED-HOST: `trig_move_roll` is unmodeled — the roll-reader cannot run.
// Spec keeps only the determined arithmetic shape.
// ===========================================================================

pub fn kanon_02_steps(a: i32, b: i32) -> i32 {
    a - b
}

/// MUTANT: sum instead of difference.
pub fn kanon_02_steps_mutant(a: i32, b: i32) -> i32 {
    a + b
}

// ===========================================================================
// C-HHW-band-03
// 「（3）你进行盖房时，可从盖房花费中减去所记录的数字，然后将记录数字归零。」
// Optional (「可」). When accepted: build cost is reduced by the recorded
// number (floor 0 is not printed — OPEN); the record resets to 0.
// ===========================================================================

pub fn band_03_cost_after(pre_cost: i32, saved: i32) -> i32 {
    (pre_cost - saved).max(0)
}

pub fn band_03_saved_after() -> i32 {
    0
}

/// MUTANT: the record is NOT reset.
pub fn band_03_saved_after_mutant(pre_saved: i32) -> i32 {
    pre_saved
}

// ===========================================================================
// Shared
// ===========================================================================

pub fn log_empty(effects: &[Effect]) -> bool {
    effects.is_empty()
}