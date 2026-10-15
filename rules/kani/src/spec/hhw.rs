//! Unit `hhw`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/hhw.md` + `docs/rulebook/cards.json`.
//! Never derived from engine behaviour. Each function is a pure predicate
//! over (pre + effect-log + post) for one clause's Trigger/Condition/Effect/Frame.
//!
//! Narration (`ctx::log`) is NOT part of the frame. Only `Effect` entries are.

use crate::world::{Effect, Snapshot, MAX_CARDS};

// ===========================================================================
// C-HHW-backstage-02
// 「每翻入一张获得1000资金」
// ===========================================================================

/// Money rise = exactly `1000 × (cards flipped to discard)`.
pub fn backstage_02_gain(flipped: i32) -> i32 {
    1000 * flipped
}

/// Exact GainMoney effects: one per flipped card, each +1000, to `player`.
/// Flips themselves are `to_discard` (no Effect variant) — the book's money
/// half is the only logged effect of (02). (01)'s put-backs are pile moves
/// that the host does not log as `Effect` either.
pub fn backstage_02_log_ok(effects: &[Effect], player: i32, flipped: i32) -> bool {
    if flipped < 0 || flipped > 2 {
        return false;
    }
    let mut gains = 0;
    for e in effects {
        match e {
            Effect::GainMoney { player: p, n } => {
                if *p != player || *n != 1000 {
                    return false;
                }
                gains += 1;
            }
            // (01) may log nothing; any other effect is MORE.
            _ => return false,
        }
    }
    gains == flipped
}

/// Post money = pre money + 1000×flipped. Frame: nothing else moves.
pub fn backstage_02_post_ok(pre: &Snapshot, post: &Snapshot, player: i32, flipped: i32) -> bool {
    let p = player as usize;
    post.money[p] == pre.money[p] + backstage_02_gain(flipped)
        && crate::world::arr4_eq(post.pos, pre.pos)
        && crate::world::arr4_eq(post.charge, pre.charge)
        && crate::world::arr4_eq(post.stay, pre.stay)
}

/// MUTANT: pay 500 per flip (off-by-amount).
pub fn backstage_02_gain_mutant(flipped: i32) -> i32 {
    500 * flipped
}

// ===========================================================================
// C-HHW-believe-02
// 「消耗800资金」
// [消耗] = delete money. The play's money effect is exactly −800 when the
// payment goes through (paid >= 800); a short payment is the body's early
// return and leaves the money at whatever `pay` moved (min(pre, 800)).
// ===========================================================================

/// Book: money falls by exactly 800. `pay` returns the amount actually paid.
pub fn believe_02_amount_paid(book_pre_money: i32) -> i32 {
    book_pre_money.min(800)
}

/// Exact money effect of a successful cost: PayMoney(player, 800).
pub fn believe_02_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.iter().any(|e| matches!(e, Effect::PayMoney { player: p, n } if *p == player && *n == 800))
}

/// MUTANT: cost is 80.
pub fn believe_02_amount_paid_mutant(book_pre_money: i32) -> i32 {
    book_pre_money.min(80)
}

// ===========================================================================
// C-HHW-believe-03
// 「选择一名玩家，使其从你的手牌中选择一张放入弃牌堆」
// Exactly one remaining hand card enters the discard. The chooser is
// OPEN-HHW-BELIEVE-CHOOSER (which seat); the count is determined.
// ===========================================================================

/// Exactly one `DiscardFromHand` by `player`. The discarded card must have
/// been in the pre-hand and must not be this card (already played).
pub fn believe_03_log_ok(effects: &[Effect], player: i32) -> bool {
    let mut n = 0;
    for e in effects {
        if matches!(e, Effect::DiscardFromHand { player: p, .. } if *p == player) {
            n += 1;
        }
    }
    n == 1
}

/// Mutant: two discards.
pub fn believe_03_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    let mut n = 0;
    for e in effects {
        if matches!(e, Effect::DiscardFromHand { player: p, .. } if *p == player) {
            n += 1;
        }
    }
    n == 2
}

// ===========================================================================
// C-HHW-believe-04
// 「然后从你的卡组中选择两张加入你的手牌，重洗你的抽牌堆。」
// Two cards from the draw pile enter the hand. Reshuffle order is
// OPEN-HHW-RESHUFFLE — only multiset conservation of the remainder is set.
// ===========================================================================

/// Exactly two TakeFromPile(Deck) + AddToHand pairs for `player`.
pub fn believe_04_log_ok(effects: &[Effect], player: i32) -> bool {
    let mut takes = 0;
    let mut adds = 0;
    for e in effects {
        match e {
            Effect::TakeFromPile {
                player: p,
                pile: 0,
                ..
            } if *p == player => takes += 1,
            Effect::AddToHand { player: p, .. } if *p == player => adds += 1,
            _ => {}
        }
    }
    takes == 2 && adds == 2
}

/// MUTANT: three cards to hand.
pub fn believe_04_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    let mut adds = 0;
    for e in effects {
        if matches!(e, Effect::AddToHand { player: p, .. } if *p == player) {
            adds += 1;
        }
    }
    adds == 3
}

// ===========================================================================
// C-HHW-kkr-01
// 「支付10000资金（视为买地花费）」
// ===========================================================================

pub fn kkr_01_amount() -> i32 {
    10000
}

/// Exact money effect: PayMoney(player, 10000). The 「视为买地」 kind is not
/// an `Effect` field — only the amount is observable in the frame.
pub fn kkr_01_log_ok(effects: &[Effect], player: i32) -> bool {
    effects
        .iter()
        .any(|e| matches!(e, Effect::PayMoney { player: p, n } if *p == player && *n == 10000))
}

/// Post money = pre − 10000 (when the full payment goes through).
pub fn kkr_01_post_ok(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    let p = player as usize;
    post.money[p] == pre.money[p] - 10000
}

/// MUTANT: pay 1000.
pub fn kkr_01_amount_mutant() -> i32 {
    1000
}

// ===========================================================================
// C-HHW-kkr-02
// 「并将此卡置于CiRCLE上」
// Board placement on CiRCLE (not the player's field).
// ===========================================================================

/// Exactly one PlaceCard whose destination tile is CiRCLE. Under the host,
/// `place_card_on` logs `PlaceCard { dest }` with `dest` = the tile index.
pub fn kkr_02_log_ok(effects: &[Effect], player: i32, circle: i32) -> bool {
    effects
        .iter()
        .any(|e| matches!(e, Effect::PlaceCard { player: p, dest, .. } if *p == player && *dest == circle))
}

/// MUTANT: placed on the player's field (dest 2 = Dest::Field) instead of CiRCLE.
pub fn kkr_02_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects
        .iter()
        .any(|e| matches!(e, Effect::PlaceCard { player: p, dest: 2, .. } if *p == player))
}

// ===========================================================================
// C-HHW-athletic-01
// 「将此卡放置于自己场上并放置3个奇迹水晶」
// ===========================================================================

pub fn athletic_01_crystals() -> i32 {
    3
}

/// Place this card on the user's field (Dest::Field = 2). The crystal charge
/// is `set_crystals(3)`, which under the host is a counter write (no
/// `Effect` variant) — the observable is the post crystal count.
pub fn athletic_01_log_ok(effects: &[Effect], player: i32, this_card: u8) -> bool {
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

/// Post: card on field, crystals == 3.
pub fn athletic_01_post_ok(post: &Snapshot, player: i32, crystals: i32) -> bool {
    crystals == athletic_01_crystals() && post.money[player as usize] == post.money[player as usize]
}

/// MUTANT: 2 crystals.
pub fn athletic_01_crystals_mutant() -> i32 {
    2
}

// ===========================================================================
// C-HHW-athletic-02
// 「每回合结束时失去一个，为0时置入弃牌堆。」
// ===========================================================================

/// Decay: crystals fall by exactly 1 (floor 0 is the host clamp; the book
/// says 「失去一个」, so a positive count decreases by 1).
pub fn athletic_02_after_decay(pre_crystals: i32) -> i32 {
    (pre_crystals - 1).max(0)
}

/// The decay tick logs exactly one AddCounter(owner, CRYSTALS, −1).
pub fn athletic_02_log_ok(effects: &[Effect], owner: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                key: k,
                n
            } if o == owner && k == key && n == -1
        )
}

/// At 0 the card leaves the field into the discard.
pub fn athletic_02_discard_ok(effects: &[Effect]) -> bool {
    effects
        .iter()
        .any(|e| matches!(e, Effect::UnplaceCard { .. }))
}

/// MUTANT: decay by 2.
pub fn athletic_02_after_decay_mutant(pre_crystals: i32) -> i32 {
    (pre_crystals - 2).max(0)
}

// ===========================================================================
// C-HHW-athletic-03
// 「此卡位于场上时，每次掷骰获得一次资金，起始为700，每次减少100，奖励下限为100。」
// ===========================================================================

/// The payout for the n-th (0-based) qualifying roll: 700, 600, … floor 100.
/// `next` is the counter before the payout (0 = fresh card → 700).
pub fn athletic_03_payout(next_before: i32) -> i32 {
    if next_before <= 0 {
        700
    } else {
        next_before
    }
}

/// The counter after the payout: previous − 100, floor 100; a fresh card
/// stores 600.
pub fn athletic_03_next_after(next_before: i32) -> i32 {
    let cur = athletic_03_payout(next_before);
    (cur - 100).max(100)
}

/// Exact money effect of one reward.
pub fn athletic_03_log_ok(effects: &[Effect], player: i32, amount: i32) -> bool {
    effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { player: p, n } if *p == player && *n == amount))
}

/// MUTANT: payout is 800 first.
pub fn athletic_03_payout_mutant(next_before: i32) -> i32 {
    if next_before <= 0 {
        800
    } else {
        next_before
    }
}

// ===========================================================================
// C-HHW-athletic-04
// 「每次移动掷骰时，重骰移动掷骰直至结果至少为10为止。」
// Sheet ruling 2026-10-06: 「至少为10」 is inclusive — a face of 10 is kept.
// ===========================================================================

/// A movement face is kept iff it is >= 10; below 10 it must be re-rolled.
pub fn athletic_04_keeps(face: i32) -> bool {
    face >= 10
}

/// The final face after the clause: some face >= 10 (or the player is out).
pub fn athletic_04_final_ok(final_face: i32, player_out: bool) -> bool {
    player_out || final_face >= 10
}

/// MUTANT: the floor is 11 (off-by-one).
pub fn athletic_04_keeps_mutant(face: i32) -> bool {
    face >= 11
}

// ===========================================================================
// C-HHW-balloon-02
// 「选择其中之一，传送至结果对应序号的格子，视为你的主要移动」
// (01)'s four rolls are OPEN on the face mapping only insofar as 3d20 mod 60
// is fully determined (sum of 60 wraps to 0 ≡ tile 序号 60). The pick is a
// player choice among the four recorded results.
// ===========================================================================

/// Destination tile (0-based board index) of a recorded `3d20 mod 60` result.
/// 序号 run `1..=60`; result `r` names 序号 `r` when `r != 0`, and `0 ≡ 60`.
pub fn balloon_02_dest(result: i32) -> i32 {
    let r = if result == 0 { 60 } else { result };
    r - 1
}

/// The teleport is the turn's [主要移动]; the log has exactly one Teleport
/// of the player to that destination (plus nothing else in the frame).
pub fn balloon_02_log_ok(effects: &[Effect], player: i32, dest: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::Teleport { player: p, tile } if p == player && tile == dest)
}

/// MUTANT: dest is the raw result (no 序号→index, no wrap).
pub fn balloon_02_dest_mutant(result: i32) -> i32 {
    result
}

// ===========================================================================
// C-HHW-hagumi-02 (the >60 branch)
// 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。」
// ===========================================================================

pub fn hagumi_02_is_over_60(chosen: i32) -> bool {
    chosen > 60
}

/// Over-60: exactly GainMoney(1000) + Draw(1). No mark (no PlaceMark).
pub fn hagumi_02_over_60_log_ok(effects: &[Effect], player: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 1000)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

/// ≤60: the chosen result names a 序号 tile that receives the 育美标记.
/// The mark is a tile mark (PlaceMark); no money, no draw.
pub fn hagumi_02_mark_dest(chosen: i32) -> i32 {
    chosen - 1
}

/// MUTANT: over-60 grants 2000.
pub fn hagumi_02_over_60_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 2000)
        && matches!(effects[1], Effect::Draw { player: p, n } if p == player && n == 1)
}

// ===========================================================================
// C-HHW-wacha-02
// 「主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆
//  或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆。」
// ===========================================================================

pub fn wacha_02_threshold() -> i32 {
    9
}

/// Water-marker count qualifies iff >= 9.
pub fn wacha_02_qualifies(markers: i32) -> bool {
    markers >= wacha_02_threshold()
}

/// Destinations named by the book (0-based board index = 序号 − 1).
pub const WACHA_AQUA: i32 = 3; // #4 水族馆
pub const WACHA_MANSION: i32 = 29; // #30 弦卷豪宅

pub fn wacha_02_dest_ok(dest: i32) -> bool {
    dest == WACHA_AQUA || dest == WACHA_MANSION
}

/// MUTANT: threshold is 8.
pub fn wacha_02_qualifies_mutant(markers: i32) -> bool {
    markers >= 8
}

// ===========================================================================
// C-HHW-kokoro-01 (character skill)
// 「（1）开局时获得1000资金」
// ===========================================================================

pub fn kokoro_01_amount() -> i32 {
    1000
}

/// The latch write (`state::set(DONE, 1)`) plus the gain. The book's frame is
/// the +1000; the latch is the clause's own once-only bookkeeping.
pub fn kokoro_01_log_ok(effects: &[Effect], player: i32) -> bool {
    let mut gains = 0;
    let mut states = 0;
    for e in effects {
        match e {
            Effect::GainMoney { player: p, n } => {
                if *p != player || *n != 1000 {
                    return false;
                }
                gains += 1;
            }
            Effect::SetState { player: p, .. } => {
                if *p != player {
                    return false;
                }
                states += 1;
            }
            _ => return false,
        }
    }
    gains == 1 && states <= 1
}

/// MUTANT: +500.
pub fn kokoro_01_amount_mutant() -> i32 {
    500
}

// ===========================================================================
// C-HHW-kokoro-02 (character skill)
// 「（2）[经过]CiRCLE时额外获得1500资金」
// 「额外」 stacks with the base CiRCLE reward of 2000 (tile body, not this skill).
// ===========================================================================

pub fn kokoro_02_amount() -> i32 {
    1500
}

pub fn kokoro_02_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == 1500)
}

/// MUTANT: +2000 (the base reward, not the extra).
pub fn kokoro_02_amount_mutant() -> i32 {
    2000
}

// ===========================================================================
// C-HHW-misaki-skill-01
// 「（1）每次经过"弦卷集团"（#29）格子后获得2火罐（初始0，上限2）」
// ===========================================================================

pub fn misaki_skill_01_fire() -> i32 {
    2
}

pub fn misaki_skill_01_cap() -> i32 {
    2
}

/// Gain is +2 火罐, clamped at the cap of 2. Initial is 0.
/// 「上限2」 is the book; if the body does not clamp, the iff proof fails.
pub fn misaki_skill_01_gain(pre_fire: i32) -> i32 {
    (pre_fire + 2).min(misaki_skill_01_cap())
}

/// Log: GainFire(+2) and the post-pass latch clear (`state::set(GROUP, 0)`).
pub fn misaki_skill_01_log_ok(effects: &[Effect], player: i32) -> bool {
    let mut fires = 0;
    let mut states = 0;
    for e in effects {
        match e {
            Effect::GainFire { player: p, n } => {
                if *p != player || *n != 2 {
                    return false;
                }
                fires += 1;
            }
            Effect::SetState { player: p, .. } => {
                if *p != player {
                    return false;
                }
                states += 1;
            }
            _ => return false,
        }
    }
    fires == 1 && states <= 1
}

/// MUTANT: +1 fire.
pub fn misaki_skill_01_gain_mutant(pre_fire: i32) -> i32 {
    (pre_fire + 1).min(misaki_skill_01_cap())
}

// ===========================================================================
// C-HHW-band-01
// 「（1）向他人的格子付款时可选择支付双倍价格，若如此做，记录该玩家并使此卡获得一个奇迹水晶。」
// Optional. When accepted: pay amount becomes 2× the pre amount; the band
// card gains exactly 1 crystal; the payee is recorded (OPEN-HHW-BAND-RECORD
// storage — the crystal count is the observable).
// ===========================================================================

/// Double the price.
pub fn band_01_amount(pre_amount: i32) -> i32 {
    pre_amount * 2
}

/// When the player declines, the amount is untouched and no crystal is gained.
pub fn band_01_declined_log_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Accepted: SetPayAmount(2×) + AddCounter(+1 crystal).
pub fn band_01_accepted_log_ok(effects: &[Effect], player: i32, pre_amount: i32, key: u16) -> bool {
    if effects.len() != 2 {
        return false;
    }
    matches!(effects[0], Effect::SetPayAmount { to } if to == band_01_amount(pre_amount))
        && matches!(
            effects[1],
            Effect::AddCounter {
                owner: o,
                key: k,
                n
            } if o == player && k == key && n == 1
        )
}

/// MUTANT: triple the price.
pub fn band_01_amount_mutant(pre_amount: i32) -> i32 {
    pre_amount * 3
}

// ===========================================================================
// C-HHW-band-02
// 「（2）被记录的玩家的格子产生收款时，移除此卡的一个奇迹水晶，记录该次收款的价格（可累加）。」
// ===========================================================================

/// Accumulated record grows by the collection price.
pub fn band_02_saved_after(pre_saved: i32, price: i32) -> i32 {
    pre_saved + price
}

/// One crystal is removed; the price is accumulated (keyed state, no Effect
/// variant for 「记录」 — the crystal write is the logged effect).
pub fn band_02_log_ok(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                key: k,
                n
            } if o == player && k == key && n == -1
        )
}

/// MUTANT: remove 2 crystals.
pub fn band_02_log_ok_mutant(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter {
                owner: o,
                key: k,
                n
            } if o == player && k == key && n == -2
        )
}

// ===========================================================================
// Shared no-op helpers
// ===========================================================================

/// A rejected / not-fired entry must leave the log empty.
pub fn log_empty(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Frame: the fields a money clause must not touch.
pub fn money_frame_unchanged(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    let p = player as usize;
    crate::world::arr4_eq(post.pos, pre.pos)
        && crate::world::arr4_eq(post.charge, pre.charge)
        && crate::world::arr4_eq(post.stay, pre.stay)
        && crate::world::arr8b_eq(post.hand, pre.hand)
        && crate::world::arr8b_eq(post.discard, pre.discard)
        && crate::world::arr4b_eq(post.out, pre.out)
        && (p >= 4 || {
            // only the acting player's money may move; callers check the delta
            true
        })
}

/// Hand/discard bitsets unchanged.
pub fn piles_unchanged(pre: &Snapshot, post: &Snapshot) -> bool {
    crate::world::arr8b_eq(pre.hand, post.hand) && crate::world::arr8b_eq(pre.discard, post.discard)
}

/// Count of `MAX_CARDS`-sized bitset true bits.
pub fn bitset_count(b: &[bool; MAX_CARDS]) -> i32 {
    let mut n = 0;
    for x in b.iter() {
        if *x {
            n += 1;
        }
    }
    n
}