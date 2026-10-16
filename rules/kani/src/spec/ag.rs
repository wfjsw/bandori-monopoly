//! Unit `ag`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/ag.md` (verbatim quotes) and
//! `docs/rulebook/cards.json`. Never derived from engine behaviour.
//! OPEN items prove only the determined half and are listed in
//! `docs/rulebook/spec/kani/ag.md`.

use crate::world::{Effect, Snapshot};

// ===========================================================================
// C-AG-half-speed-01
// 「(1) 将此卡放置在场上并获得3个奇迹水晶」
// ===========================================================================

/// Exact effect log of the [手] play: place this card on the user's field.
/// 「获得3个奇迹水晶」 is the placement's crystal charge (post-state; the host's
/// `set_counter_self` writes the counter without a log entry).
pub fn half_speed_01_log_ok(effects: &[Effect], player: i32, this_card: u8) -> bool {
    // The body also stamps `set_crystals(3)` which pushes `AddCounter`
    // bookkeeping — ignore it.
    let mut place = 0;
    let mut other = 0;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card,
            } if *p == player && *card == this_card => place += 1,
            Effect::AddCounter { .. } => {}
            _ => other += 1,
        }
    }
    other == 0 && place == 1
}

/// Post: the card sits on the field with exactly 3 [奇迹水晶].
pub fn half_speed_01_post_ok(post_crystals: i32) -> bool {
    post_crystals == 3
}

/// MUTANT: the reference wants 4 crystals. The card does 3.
pub fn half_speed_01_post_ok_mutant(post_crystals: i32) -> bool {
    post_crystals == 4
}

// ===========================================================================
// C-AG-half-speed-02
// 「你的回合结束时移除一个奇迹水晶，奇迹水晶为0时此卡放入弃牌堆。」
// ===========================================================================

/// Turn-end tick: exactly one crystal leaves (`decay` = `add_crystals(-1, 0)`).
/// At 0 the card is moved to the discard (separate `CounterChanged` entry).
pub fn half_speed_02_decay_log_ok(effects: &[Effect], owner: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter { owner: o, key: k, n: -1 } if o == owner && k == key
        )
}

/// Post after a tick that started from `pre_crystals > 0`.
pub fn half_speed_02_decay_post_ok(pre_crystals: i32, post_crystals: i32) -> bool {
    post_crystals == pre_crystals - 1
}

/// Empty-at-zero entry: the card leaves the field to the discard.
/// Host `set_dest(Graveyard)` + place semantics produce `PlaceCard { dest: 0 }`
/// only when the engine re-places; under Kani `set_dest` alone writes `dest`
/// and the body only logs. The determined half is: the body's only effect is
/// none (log-only) and `dest` is set to Graveyard (0). We pin the dest write.
pub fn half_speed_02_empty_dest_is_graveyard(dest: i32) -> bool {
    dest == 0
}

// ===========================================================================
// C-AG-half-speed-04
// 「且你的所有资金支付与消耗减半」
// User ruling 2026-10-15 (OPEN-取整 family): the card is an add/mul modifier
// -- it writes the fold (exact integer half, no CeilTo) and the engine applies
// `final = round10(fold of add/mul)` once at settle.
// ===========================================================================

/// Round up to a multiple of 10 (the engine's end-of-calc money round).
pub fn round10(x: i32) -> i32 {
    if x <= 0 {
        return x;
    }
    ((x as i64 + 9) / 10 * 10).clamp(0, i32::MAX as i64) as i32
}

/// fold of the card's own 减半 -- what the body writes (integer ceil-half).
pub fn half_speed_04_fold(pre_amount: i32) -> i32 {
    if pre_amount <= 0 {
        return 0;
    }
    ((pre_amount as i64 + 1) / 2).clamp(0, i32::MAX as i64) as i32
}

/// Clause pin: `final = round10(fold of add/mul)` for a sole 减半
/// = `round10(amount / 2)` with the half kept exact (25 → 20).
pub fn half_speed_04_amount_ceil10(pre_amount: i32) -> i32 {
    round10(half_speed_04_fold(pre_amount))
}

/// Determined half: the pay amount is halved (integer half of an even amount).
pub fn half_speed_04_amount_even(pre_amount: i32) -> i32 {
    pre_amount / 2
}

/// Exact log of a positive-amount rewrite (the fold, not the end-of-calc round).
pub fn half_speed_04_log_ok(effects: &[Effect], expected_to: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == expected_to)
}

/// MUTANT: off-by-one high on the fold the body writes.
pub fn half_speed_04_amount_mutant(pre_amount: i32) -> i32 {
    half_speed_04_fold(pre_amount) + 1
}

// ===========================================================================
// C-AG-sky-02 / C-AG-sky-03
// 「如果你手牌数至少为3，获得手牌数*600的资金」
// 「如果你的手牌数小于3，抽一张卡」
// The card is 「立刻打出」 so it is already out of the hand when the body
// runs; 手牌数 is the remaining hand (clause doc C-AG-sky-02).
// ===========================================================================

/// Post-take hand size >= 3: gain exactly `n * 600`.
pub fn sky_02_log_ok(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: m } if p == player && m == n * 600
        )
}

/// Post-take hand size < 3: draw exactly 1.
pub fn sky_03_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n: 1 } if p == player)
}

/// Combined body (take_from_hand + branch + to_discard). The take and the
/// discard are pile moves; `take_card` logs `TakeFromPile` and `to_discard`
/// does not log. Frame: the money/draw leg plus the take.
pub fn sky_play_log_ok(
    effects: &[Effect],
    player: i32,
    this_card: u8,
    hand_after_take: i32,
) -> bool {
    if hand_after_take >= 3 {
        effects.len() == 2
            && matches!(
                effects[0],
                Effect::TakeFromPile {
                    player: p,
                    pile: 0,
                    card
                } if p == player && card == this_card
            )
            && matches!(
                effects[1],
                Effect::GainMoney { player: p, n } if p == player && n == hand_after_take * 600
            )
    } else {
        effects.len() == 2
            && matches!(
                effects[0],
                Effect::TakeFromPile {
                    player: p,
                    pile: 0,
                    card
                } if p == player && card == this_card
            )
            && matches!(
                effects[1],
                Effect::Draw { player: p, n: 1 } if p == player
            )
    }
}

/// MUTANT of the gain leg: demands `n * 601`.
pub fn sky_02_log_ok_mutant(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: m } if p == player && m == n * 601
        )
}

// ===========================================================================
// C-AG-colors-02 / 03 / 04 / 05 / 06
// 「投掷1d6并根据结果获得对应效果: 若为1则立刻获得1500资金，若为2则从弃牌堆中
//  选择一张牌放置到抽牌堆顶，若为3则抽一张牌，若为4则可选择补满火罐或者获得
//  2000资金，若为5则从弃牌堆中选择一张牌加入手牌，若为6则选择1-5中的3个效果触发
//  （若结果严格大于6，则从1开始重新计数）」
// ===========================================================================

/// Wrap of a raw 1d6 face: identity on 1..=6 (the >6 clause cannot fire).
pub fn colors_01_wrap(raw: i32) -> i32 {
    (raw - 1).rem_euclid(6) + 1
}

/// C-AG-colors-02: roll = 1 ⇒ gain exactly 1500.
pub fn colors_02_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 1500 } if p == player
        )
}

/// C-AG-colors-04: roll = 3 ⇒ draw exactly 1.
pub fn colors_04_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::Draw { player: p, n: 1 } if p == player)
}

/// C-AG-colors-05 money branch: gain exactly 2000.
pub fn colors_05_money_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 2000 } if p == player
        )
}

/// C-AG-colors-05 fire branch: refill the pot to the cap (`gain_charge(need)`).
pub fn colors_05_fire_log_ok(effects: &[Effect], player: i32, need: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == need
        )
}

/// C-AG-colors-06: roll = 5 ⇒ one discard card moves to the hand.
/// `pick_from_discard` takes the card (TakeFromPile) then `add_to_hand`.
pub fn colors_06_log_ok(effects: &[Effect], player: i32, picked: u8) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::TakeFromPile {
                player: p,
                pile: 1,
                card
            } if p == player && card == picked
        )
        && matches!(
            effects[1],
            Effect::AddToHand {
                player: p,
                card
            } if p == player && card == picked
        )
}

/// C-AG-colors-03: roll = 2 ⇒ one discard card moves to the draw-pile top.
/// The take is logged; `add_to_deck_at` is a pile write (no log entry).
pub fn colors_03_log_ok(effects: &[Effect], player: i32, picked: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::TakeFromPile {
                player: p,
                pile: 1,
                card
            } if p == player && card == picked
        )
}

/// Empty discard on branches 2 / 5: only the branch announcement (narration),
/// no pile effect.
pub fn colors_empty_discard_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// MUTANT of colors-02: demands 1501.
pub fn colors_02_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 1501 } if p == player
        )
}

// ===========================================================================
// C-AG-soul-01
// 「[手]：将此卡放置在[使用者]的[场地]，然后选择[消耗]1到5次500资金并在
//  这张卡上放置对应数量的[奇迹水晶]。」
// ===========================================================================

/// Play body when the chosen `k` is fully paid (`paid == k*500`):
/// place on field, pay `k*500`, `k` crystals on the card.
pub fn soul_01_log_ok(
    effects: &[Effect],
    player: i32,
    this_card: u8,
    k: i32,
) -> bool {
    if effects.len() != 2 {
        return false;
    }
    // Order in the body: pay first, then place.
    matches!(
        effects[0],
        Effect::PayMoney { player: p, n } if p == player && n == k * 500
    ) && matches!(
        effects[1],
        Effect::PlaceCard {
            player: p,
            dest: 2,
            card
        } if p == player && card == this_card
    )
}

/// Post crystals on the card = `k` when the pay landed.
pub fn soul_01_post_ok(post_crystals: i32, k: i32) -> bool {
    post_crystals == k
}

/// MUTANT: crystals = k+1.
pub fn soul_01_post_ok_mutant(post_crystals: i32, k: i32) -> bool {
    post_crystals == k + 1
}

// ===========================================================================
// C-AG-soul-02
// 「（1）[反击][拥有者]因导致的[消耗]或[支付]时可选择移除此卡的1个[奇迹水晶]，
//  此次[消耗]或[支付]金额减少1000（最少为0，若为[支付]则被[支付]玩家[获得]500资金）。」
// ===========================================================================

/// Accepted spend of one crystal on a [支付] to a live payee:
/// payee +500, crystal −1, amount = max(pre−1000, 0).
pub fn soul_02_pay_log_ok(
    effects: &[Effect],
    owner: i32,
    payee: i32,
    crystal_key: u16,
    expected_to: i32,
) -> bool {
    effects.len() == 3
        && matches!(
            effects[0],
            Effect::GainMoney { player: p, n: 500 } if p == payee
        )
        && matches!(
            effects[1],
            Effect::AddCounter {
                owner: o,
                key: k,
                n: -1
            } if o == owner && k == crystal_key
        )
        && matches!(
            effects[2],
            Effect::SetPayAmount { to } if to == expected_to
        )
}

/// Declined / no crystal / non-positive amount: no effects.
pub fn soul_02_noop_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Post amount the clause demands.
pub fn soul_02_amount(pre_amount: i32) -> i32 {
    (pre_amount - 1000).max(0)
}

/// MUTANT: amount off-by-one.
pub fn soul_02_amount_mutant(pre_amount: i32) -> i32 {
    soul_02_amount(pre_amount) + 1
}

// ===========================================================================
// C-AG-soul-03
// 「（2）[拥有者]使用自己原有的技能（2）时移除此卡的1个[奇迹水晶]。」
// ===========================================================================

pub fn soul_03_log_ok(effects: &[Effect], owner: i32, crystal_key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                key: k,
                n: -1
            } if o == owner && k == crystal_key
        )
}

// ===========================================================================
// C-AG-soul-04
// 「（3）此卡上不再拥有[奇迹水晶]时将此卡放入[使用者]弃卡区。」
// ===========================================================================

/// The empty-crystal entry only sets dest = Graveyard (0) and logs. Under the
/// Kani host `set_dest` writes `World::dest` and pushes no effect.
pub fn soul_04_dest_is_graveyard(dest: i32) -> bool {
    dest == 0
}

pub fn soul_04_log_empty(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// C-AG-glory-02 / C-AG-glory-03
// 「你从对方处获得等于对方最贵格子基础购买价格一半数额的资金」
// 「之后对方从你处获得等于你最贵格子基础购买价格一半数额的资金。」
// OPEN-半价取整: odd base prices unspecified; integer division (book prints
// even prices today).
// ===========================================================================

/// Floor half of the dearest deed's base purchase price.
pub fn glory_half(dearest_base: i32) -> i32 {
    dearest_base / 2
}

/// Both legs in order: other→user for their half, then user→other for ours.
/// A zero half contributes no leg.
pub fn glory_log_ok(
    effects: &[Effect],
    user: i32,
    other: i32,
    from_them: i32,
    from_us: i32,
) -> bool {
    let mut i = 0usize;
    if from_them > 0 {
        if i >= effects.len() {
            return false;
        }
        if !matches!(
            effects[i],
            Effect::PayTo {
                from,
                to,
                n
            } if from == other && to == user && n == from_them
        ) {
            return false;
        }
        i += 1;
    }
    if from_us > 0 {
        if i >= effects.len() {
            return false;
        }
        if !matches!(
            effects[i],
            Effect::PayTo {
                from,
                to,
                n
            } if from == user && to == other && n == from_us
        ) {
            return false;
        }
        i += 1;
    }
    i == effects.len()
}

/// MUTANT: demands an extra 1 on the first leg.
pub fn glory_log_ok_mutant(
    effects: &[Effect],
    user: i32,
    other: i32,
    from_them: i32,
    from_us: i32,
) -> bool {
    glory_log_ok(effects, user, other, from_them + 1, from_us)
}

// ===========================================================================
// C-AG-one-of-us-01
// 「将此卡放置在场上」
// ===========================================================================

pub fn one_of_us_01_log_ok(effects: &[Effect], player: i32, this_card: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card
            } if p == player && card == this_card
        )
}

// ===========================================================================
// Shared helpers
// ===========================================================================

/// Frame check: everything outside the listed money/card slots is unchanged.
pub fn frame_money_only(pre: &Snapshot, post: &Snapshot, touched: &[usize]) -> bool {
    for p in 0..4 {
        if touched.contains(&p) {
            continue;
        }
        if pre.money[p] != post.money[p] {
            return false;
        }
    }
    pre.pos == post.pos
        && pre.charge == post.charge
        && pre.stay == post.stay
        && crate::world::arr4b_eq(pre.out, post.out)
        && pre.pay_amount == post.pay_amount
}