//! 「无视【距离最近】」 -- the waiver a placed card grants its placer on the
//! holder (`R:必然的联系（莉莎）`: 「你对对方使用技能将可以无视【距离最近】这一
//! 限制」).
//!
//! The generic nearest selection is `card_sdk::ctx::nearest` /
//! `nearest_with_prop`; this module owns the game's words for the limit
//! itself, so no key or skill policy lives in the SDK or the engine.

use alloc::vec::Vec;

use crate::ctx;

/// On a **placed instance's** props, the value is the **grantor** seat whose
/// skills may ignore the nearest limit when targeting this instance's holder.
/// Written at runtime and gone with the instance; presence is the fact, so the
/// value `0` (grantor seat 0) is a real grantor.
pub const EXEMPTION: &str = "nearestExemption";

/// Mark instance `uid` as granting `grantor` the waiver on its holder.
pub fn grant(uid: i32, grantor: i32) -> i32 {
    ctx::set_prop_at(uid, EXEMPTION, grantor)
}

/// Does `candidate` hold an instance granting `grantor` the waiver?
///
/// Per-pair form, for a skill that already has its candidate. `prop_at` answers
/// `0` for an absent key, so the scan also gates on the card id: only a placed
/// bond instance carries the key, and every one is granted right after its
/// placement (`lisa_bond`). (The card "PP:[若宫伊芙]属于我的武士道！" stores
/// `player_id + 1` for the same seat-0 reason; here the card-id gate makes the
/// plain seat unambiguous.)
pub fn exemption(grantor: i32, candidate: i32) -> bool {
    ctx::field_instances(candidate)
        .iter()
        .any(|&(uid, card)| card == "R:必然的联系（莉莎）" && ctx::prop_at(uid, EXEMPTION) == grantor)
}

/// The candidate list of a 「距离最近」 **skill**: the nearest set plus the
/// exempt seats, distance-then-seat ordered, deduplicated, raw set first.
pub fn skill_candidates(grantor: i32, kind: ctx::DistanceKind) -> Vec<i32> {
    ctx::nearest_with_prop(grantor, kind, EXEMPTION)
}
