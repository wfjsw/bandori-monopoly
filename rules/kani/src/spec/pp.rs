//! Unit `pp`: reference specs FROM the clause docs / cards.json only.
//!
//! Sources: `docs/rulebook/cards.json` keys `PP:*` (verbatim quotes in
//! `docs/rulebook/spec/cards/pp.md`). Never derived from engine behaviour.
//!
//! OPEN items (text undefined) get the narrowest predicate the quote pins;
//! the proof tables record `OPEN-<id>`.
//!
//! Fan tokens: `P✽P粉丝(正)` / `P✽P粉丝(反)`. `owned = up + down`.
//! 「Pastel✽Palettes乐队卡」 crystal pool = the band-skill field card's
//! crystals (OPEN-乐队卡).

use crate::world::{Effect, Snapshot, MAX_CARDS};

// ===========================================================================
// Shared formulas
// ===========================================================================

/// `owned` = positive + reverse [P✽P粉丝].
pub fn fans_owned(up: i32, down: i32) -> i32 {
    up + down
}

/// Ceil of `n / 2` for non-negative `n` (「差的一半（向上取整）」).
pub fn ceil_half(n: i32) -> i32 {
    if n <= 0 {
        0
    } else {
        (n + 1) / 2
    }
}

// ===========================================================================
// C-PP-title-idol-01
// 「1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；」
// ===========================================================================

/// Band pool rises by exactly 2 from step 1.
pub fn title_idol_01_band_after(pre_band: i32) -> i32 {
    pre_band + 2
}

// ===========================================================================
// C-PP-title-idol-02
// 「2. 为[使用者]所有效果包含[奇迹水晶]的卡添加1个[奇迹水晶]，如果[共鸣]则改为添加2个。」
// OPEN-乐队卡, OPEN-共鸣.
// ===========================================================================

/// Per qualifying card the crystal delta is 1, or 2 when [共鸣] is accepted.
pub fn title_idol_02_per_card(echo: bool) -> i32 {
    if echo {
        2
    } else {
        1
    }
}

// ===========================================================================
// C-PP-aya-haruka-02 / 01 live in `spec/pilot.rs` (foundation-owned).
// Do not duplicate them here.
// ===========================================================================

// ===========================================================================
// C-PP-aya-haruka-03
// 「（2）[共鸣][反击][消耗]或[支付]时将金额降低1500（最低0）。」
// OPEN-共鸣 (the accepted branch is the only one the text pins).
// ===========================================================================

/// Post amount after the 1500 shave, floored at 0.
pub fn aya_haruka_03_amount(pre_amount: i32) -> i32 {
    (pre_amount - 1500).max(0)
}

/// Exactly one effect: the pay-amount rewrite.
pub fn aya_haruka_03_log_ok(effects: &[Effect], expected_to: i32) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::SetPayAmount { to } if to == expected_to)
}

// ===========================================================================
// C-PP-hina-sound-01 / 02 — OPEN-substitution / OPEN-substitution-effect.
// ===========================================================================

// ===========================================================================
// C-PP-maya-inf-02
// 「（1）[拥有者]进行抽卡动作后如果此卡的[奇迹水晶]小于4则为此卡添加1个[奇迹水晶]，
//  否则移除此卡的[奇迹水晶]并为[拥有者]的Pastel✽Palettes乐队卡添加1个[奇迹水晶]。」
// ===========================================================================

/// Post crystals on this card after one owner-draw tick.
pub fn maya_inf_02_card_crystals(pre_crystals: i32) -> i32 {
    if pre_crystals < 4 {
        pre_crystals + 1
    } else {
        0
    }
}

/// Whether the band pool is bumped (the `>= 4` leg).
pub fn maya_inf_02_band_bump(pre_crystals: i32) -> i32 {
    if pre_crystals < 4 {
        0
    } else {
        1
    }
}

// ===========================================================================
// C-PP-chisato-mask-01
// 「[手]：将此卡放置在[使用者]的[场地]，其他玩家[分摊][支付][使用者]2000资金。」
// OPEN-分摊: only the 2000 total is determined.
// ===========================================================================

/// The collective total others pay the user is exactly 2000.
pub fn chisato_mask_01_total() -> i32 {
    2000
}

/// User field placement + money legs summing to `total` toward `user`.
/// Split among payers is OPEN-分摊; only the sum and the direction are pinned.
pub fn chisato_mask_01_money_ok(effects: &[Effect], user: i32, total: i32) -> bool {
    let mut sum = 0;
    let mut saw_place = false;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                ..
            } if *p == user => saw_place = true,
            Effect::PayTo { from, to, n } if *to == user && *from != user => sum += *n,
            Effect::PayMoney { player: p, n } if *p != user => sum += *n,
            Effect::GainMoney { player: p, n } if *p == user => sum += *n,
            _ => return false,
        }
    }
    saw_place && sum == total
}

// ===========================================================================
// C-PP-eve-bushido-01
// 「[手]：将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。」
// ===========================================================================

/// Money gained = (sum of twelve d4) * 60. Sum is in `12..=48`.
pub fn eve_bushido_01_gain(dice_sum: i32) -> i32 {
    dice_sum * 60
}

/// Legal 12d4 total.
pub fn eve_bushido_01_sum_ok(sum: i32) -> bool {
    (12..=48).contains(&sum)
}

// ===========================================================================
// C-PP-eve-bushido-04
// 「（1）[使用者]抽卡后为此卡添加1个[奇迹水晶]（上限3）。」
// ===========================================================================

/// Post crystals after +1, capped at 3.
pub fn eve_bushido_04_crystals(pre: i32) -> i32 {
    (pre + 1).min(3)
}

// ===========================================================================
// C-PP-eve-bushido-05
// 「（2）[结算]前…投掷点数低的玩家[支付]投掷点数高的玩家
//  [拥有者]所在格子的房屋数量加1×100（如果平局则双方互不支付），
//  如果[共鸣]则[支付]金额改为“投掷点差”×50。」
// OPEN-共鸣 (accepted branch).
// ===========================================================================

/// Loser pays winner `(houses + 1) * 100`, or 0 on a tie; with [共鸣]
/// `|a - b| * 50` instead (still 0 on a tie, since the difference is 0).
pub fn eve_bushido_05_payment(roll_a: i32, roll_b: i32, houses: i32, echo: bool) -> i32 {
    if roll_a == roll_b {
        return 0;
    }
    if echo {
        (roll_a - roll_b).abs() * 50
    } else {
        (houses + 1) * 100
    }
}

/// 1d6 vs 3d4 ranges.
pub fn eve_bushido_05_rolls_ok(a: i32, b: i32) -> bool {
    (1..=6).contains(&a) && (3..=12).contains(&b)
}

// ===========================================================================
// C-PP-der-echo-01
// 「[特]：…将此卡放入弃卡区，发动那张卡的[共鸣]效果并为拥有此卡的玩家的
//  Pastel✽Palettes乐队卡添加2[奇迹水晶]。」
// OPEN-共鸣.
// ===========================================================================

/// Band pool +2 when this card is paid as the [共鸣] cost by a PP player.
pub fn der_echo_01_band_after(pre_band: i32) -> i32 {
    pre_band + 2
}

// ===========================================================================
// C-PP-seeyou-01
// 「（1）只有[使用者]的正面[P✽P粉丝]数量少等于反面[P✽P粉丝]数量才可使用。」
// ===========================================================================

/// Play is allowed iff `up <= down`.
pub fn seeyou_01_gate_ok(up: i32, down: i32) -> bool {
    up <= down
}

// ===========================================================================
// C-PP-seeyou-03
// 「[手]：将自己拥有的[P✽P粉丝]数量个反面[P✽P粉丝]变正。」
// ===========================================================================

/// Flip `min(owned, down)` reverse fans positive.
pub fn seeyou_03_flip(up: i32, down: i32) -> i32 {
    fans_owned(up, down).min(down).max(0)
}

/// Post face counts after the flip.
pub fn seeyou_03_post(up: i32, down: i32) -> (i32, i32) {
    let n = seeyou_03_flip(up, down);
    (up + n, down - n)
}

// ===========================================================================
// C-PP-overlap-01
// 「[手]：…1. [移除]此卡和[使用者][场地]上的“初次演出事故”（如果有）；」
// ===========================================================================

/// The play body must unplace this card (and any accident). Exact set of
/// unplace effects: this card always; accident iff it was on the field.
pub fn overlap_01_unplaces_ok(
    effects: &[Effect],
    this_uid: i32,
    accident_uid: Option<i32>,
) -> bool {
    let mut n = 0;
    let mut saw_this = false;
    let mut saw_acc = false;
    for e in effects {
        if let Effect::UnplaceCard { uid } = e {
            n += 1;
            if *uid == this_uid {
                saw_this = true;
            }
            if Some(*uid) == accident_uid {
                saw_acc = true;
            }
        }
    }
    saw_this && (accident_uid.is_none() || saw_acc) && n == 1 + i32::from(accident_uid.is_some())
}

// ===========================================================================
// C-PP-ranger-00
// 「[特]：仅在你的绝对距离30-“粉丝数量”格内有你拥有房的格子时可使用。」
// ===========================================================================

/// Gate: some owned-with-house tile lies within absolute board distance
/// `30 - owned` (OPEN: 「绝对距离」 wrapping is the board's cyclic distance).
pub fn ranger_00_gate_ok(distances: &[i32], owned_with_house: &[bool], owned_fans: i32) -> bool {
    let reach = 30 - owned_fans;
    let mut i = 0;
    while i < distances.len() && i < owned_with_house.len() {
        if owned_with_house[i] && distances[i].abs() <= reach {
            return true;
        }
        i += 1;
    }
    false
}

// ===========================================================================
// C-PP-ranger-01..05
// 「根据场上你拥有的卡数量并依次进行以下操作，如果[共鸣]则视为数量加1：
//  1. 数量至少为1则[获得]500资金； 2. …3则…3个[奇迹水晶]；
//  3. …4则移除…4个[奇迹水晶]并抽1张卡； 4. …5则获得1层状态…； 5.…6则抽1张卡。」
// OPEN-共鸣 (count +1); OPEN-floor (removing 4 crystals when fewer exist).
// ===========================================================================

/// Effective count = field cards the user owns (the Ranger itself once
/// played), +1 with [共鸣].
pub fn ranger_count(field_owned: i32, echo: bool) -> i32 {
    if echo {
        field_owned + 1
    } else {
        field_owned
    }
}

pub fn ranger_01_money(count: i32) -> i32 {
    if count >= 1 {
        500
    } else {
        0
    }
}

pub fn ranger_02_band(count: i32) -> i32 {
    if count >= 3 {
        3
    } else {
        0
    }
}

/// Step 3: band pool change and draw. OPEN-floor on the removal.
pub fn ranger_03_band_delta(count: i32) -> i32 {
    if count >= 4 {
        -4
    } else {
        0
    }
}

pub fn ranger_03_draw(count: i32) -> i32 {
    if count >= 4 {
        1
    } else {
        0
    }
}

pub fn ranger_04_status_layers(count: i32) -> i32 {
    if count >= 5 {
        1
    } else {
        0
    }
}

pub fn ranger_05_draw(count: i32) -> i32 {
    if count >= 6 {
        1
    } else {
        0
    }
}

// ===========================================================================
// C-PP-no-expect-02
// 「[手]：将此卡放置在[使用者]的[场地]并将1张“共鸣”加入卡组，然后抽1张牌。」
// ===========================================================================

/// Effects: PlaceCard(Field) + Draw(1). `add_to_deck` is silent in the
/// effect log (host does not record it); post-state covers that half.
pub fn no_expect_02_effects_ok(effects: &[Effect], player: i32) -> bool {
    let mut place = 0;
    let mut draw = 0;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                ..
            } if *p == player => place += 1,
            Effect::Draw {
                player: p, n: 1, ..
            } if *p == player => draw += 1,
            _ => return false,
        }
    }
    place == 1 && draw == 1
}

// ===========================================================================
// C-PP-shine-again-01
// 「（1）[反击][拥有者][消耗]或[支付]并当前资金不够时：记录一个此卡未记录且
//  [拥有者]拥有同色地契的颜色，[拥有者][获得]“…购买价格”÷5的资金和1个正面[P✽P粉丝]。」
// [反击] window — PROTOCOL-LEVEL for the window; the formula is pinned.
// ===========================================================================

/// Money leg = `purchase_price / 5` (integer division).
pub fn shine_again_01_money(purchase_price: i32) -> i32 {
    purchase_price / 5
}

/// Fan leg = exactly 1 positive fan.
pub fn shine_again_01_fan() -> i32 {
    1
}

// ===========================================================================
// C-PP-first-accident-01
// 「[手]：将此卡放置在[使用者]的[场地]并将1张“重叠的声音”加入抽卡区并洗切，然后抽1张卡。」
// ===========================================================================

/// Place + Draw(1). Deck insert is silent in the effect log.
pub fn first_accident_01_effects_ok(effects: &[Effect], player: i32) -> bool {
    let mut place = 0;
    let mut draw = 0;
    for e in effects {
        match e {
            Effect::PlaceCard {
                player: p,
                dest: 2,
                ..
            } if *p == player => place += 1,
            Effect::Draw {
                player: p, n: 1, ..
            } if *p == player => draw += 1,
            _ => return false,
        }
    }
    place == 1 && draw == 1
}

// ===========================================================================
// C-PP-flower-01
// 「[手]：[指定][使用者]拥有的一个格子，将此卡放置在被[指定]格子上并为[使用者]的
//  Pastel✽Palettes乐队卡添加2个[奇迹水晶]。」
// ===========================================================================

/// Band pool +2.
pub fn flower_01_band_after(pre_band: i32) -> i32 {
    pre_band + 2
}

/// The designated tile must be owned by the user.
pub fn flower_01_tile_ok(tile: i32, owner: i32, user: i32) -> bool {
    tile >= 0 && owner == user
}

// ===========================================================================
// C-PP-same-dream-01
// 「1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；」
// ===========================================================================

pub fn same_dream_01_band_after(pre_band: i32) -> i32 {
    pre_band + 3
}

// ===========================================================================
// C-PP-same-dream-02
// 「2. 将所有正面[P✽P粉丝]变反，[获得]变反数量乘100的资金；」
// ===========================================================================

/// Every positive fan flips reverse; money += `up * 100`.
pub fn same_dream_02_money(up: i32) -> i32 {
    up * 100
}

/// Post faces: all `up` moved to reverse.
pub fn same_dream_02_post(up: i32, down: i32) -> (i32, i32) {
    (0, down + up)
}

// ===========================================================================
// C-PP-same-dream-03
// 「3. 将自己的所有反面[P✽P粉丝]变正，如果[共鸣]则此效果对所有Pastel✽Palettes角色生效。」
// OPEN-共鸣 (the widen branch).
// ===========================================================================

/// User's reverse fans all flip positive.
pub fn same_dream_03_user_post(up: i32, down: i32) -> (i32, i32) {
    (up + down, 0)
}

// ===========================================================================
// C-PP-jennifer-01
// 「[手]：将此卡放置在除[使用者]以外的一名玩家的[场地]。」
// ===========================================================================

pub fn jennifer_01_place_ok(owner: i32, user: i32) -> bool {
    owner != user && owner >= 0
}

// ===========================================================================
// C-PP-jennifer-02 / 03
// 「…1. [使用者][支付][拥有者]400资金； 2. [使用者]获得1个正面的[P✽P粉丝]…
//  3. 此卡[移除]，然后将1张“魔法战队Pastel✽Ranger”加入[使用者]手卡。」
// OPEN-fan-face (which face is flipped on the owner).
// ===========================================================================

pub fn jennifer_02_pay() -> i32 {
    400
}

pub fn jennifer_02_user_fan() -> i32 {
    1
}

// ===========================================================================
// C-PP-together-01
// 「[手]：将X个反面[P✽P粉丝]变正；X为5，如果[使用者]拥有的正面[P✽P粉丝]数量少于
//  反面[P✽P粉丝]数量则X额外添加反面和正面[P✽P粉丝]数量的差的一半（向上取整），」
// ===========================================================================

/// X before any [共鸣] cut.
pub fn together_01_x(up: i32, down: i32) -> i32 {
    let mut x = 5;
    if up < down {
        x += ceil_half(down - up);
    }
    x
}

/// Flip count = `min(X, down)` (floored at 0).
pub fn together_01_flip(up: i32, down: i32) -> i32 {
    together_01_x(up, down).min(down).max(0)
}

// ===========================================================================
// C-PP-together-02
// 「如果[共鸣]则X减少5并抽1张卡。」
// OPEN-共鸣.
// ===========================================================================

/// X after the [共鸣] cut (may go negative; the flip floors at 0).
pub fn together_02_x(up: i32, down: i32) -> i32 {
    together_01_x(up, down) - 5
}

pub fn together_02_draw() -> i32 {
    1
}

// ===========================================================================
// C-PP-dream-ahead-01
// 「（1）[拥有者]每次抽牌时为此卡添加1个[奇迹水晶]（上限5），此卡每获得一个超出上限的
//  [奇迹水晶]就为此卡的X加1（X初始0）。」
// ===========================================================================

/// Post crystals after one draw tick.
pub fn dream_ahead_01_crystals(pre: i32) -> i32 {
    (pre + 1).min(5)
}

/// Overflow that increments X (0 when still under the cap).
pub fn dream_ahead_01_overflow(pre: i32) -> i32 {
    if pre >= 5 {
        1
    } else {
        0
    }
}

// ===========================================================================
// C-PP-dream-ahead-03
// 「（3）…额外[支付][拥有者]“[拥有者]拥有的[P✽P粉丝]数量”×X+MIN(X×30, 300)。」
// ===========================================================================

/// Extra payment = `owned_fans * X + min(X * 30, 300)`.
pub fn dream_ahead_03_extra(owned_fans: i32, x: i32) -> i32 {
    owned_fans * x + (x * 30).min(300)
}

// ===========================================================================
// C-PP-guide-01
// 「[手]：为[使用者]的Pastel✽Palettes乐队卡添加3个[奇迹水晶]并将此卡放置在[使用者]的
//  [场地]，在此卡上放置“粉丝数量”÷3个[奇迹水晶]，」
// OPEN-div (integer division is the live reading of 「÷」).
// ===========================================================================

/// Crystals placed on the card = `owned / 3` (integer division).
pub fn guide_01_crystals(up: i32, down: i32) -> i32 {
    fans_owned(up, down) / 3
}

pub fn guide_01_band_after(pre_band: i32) -> i32 {
    pre_band + 3
}

// ===========================================================================
// C-PP-guide-04
// 「（2）回合结束时添加1个[奇迹水晶]。」
// ===========================================================================

pub fn guide_04_crystals(pre: i32) -> i32 {
    pre + 1
}

// ===========================================================================
// Mutants — deliberately wrong specs. A correct card must FAIL these.
// ===========================================================================

pub fn title_idol_01_band_after_mutant(pre_band: i32) -> i32 {
    pre_band + 3
}

pub fn eve_bushido_04_crystals_mutant(pre: i32) -> i32 {
    // forgot the cap
    pre + 1
}

pub fn seeyou_03_flip_mutant(_up: i32, down: i32) -> i32 {
    // flip every reverse, ignoring `owned`
    down
}

pub fn together_01_x_mutant(up: i32, down: i32) -> i32 {
    // wrong bonus
    5 + if up < down { 1 } else { 0 }
}

pub fn same_dream_02_money_mutant(up: i32) -> i32 {
    up * 101
}

pub fn dream_ahead_03_extra_mutant(owned_fans: i32, x: i32) -> i32 {
    owned_fans * x + (x * 30).min(300) + 1
}

pub fn guide_01_crystals_mutant(up: i32, down: i32) -> i32 {
    // round-up instead of truncating
    (up + down + 2) / 3
}

pub fn maya_inf_02_card_crystals_mutant(pre: i32) -> i32 {
    if pre < 4 {
        pre + 1
    } else {
        // left one behind
        1
    }
}

// ===========================================================================
// Frame helpers shared by proofs (not clause specs).
// ===========================================================================

/// Money of every player unchanged.
pub fn frame_money(pre: &Snapshot, post: &Snapshot) -> bool {
    crate::world::arr4_eq(pre.money, post.money)
}

/// Fan views unchanged.
pub fn frame_fans(pre: &Snapshot, post: &Snapshot) -> bool {
    crate::world::arr4_eq(pre.fans_pos, post.fans_pos) && crate::world::arr4_eq(pre.fans_neg, post.fans_neg)
}

/// Hand / discard bitsets unchanged.
pub fn frame_piles(pre: &Snapshot, post: &Snapshot) -> bool {
    crate::world::arr8b_eq(pre.hand, post.hand) && crate::world::arr8b_eq(pre.discard, post.discard)
}

/// Quiet: no effects at all.
pub fn log_empty(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// Silence unused-MAX_CARDS when a clause does not need pile bits.
#[allow(dead_code)]
fn _keep_max_cards() -> usize {
    MAX_CARDS
}