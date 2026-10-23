//! Unit `r`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/cards/r.md`, `docs/rulebook/cards.json`,
//! the roster character / band rows. Never derive a spec from engine
//! behaviour. OPEN items stay unasserted; see the unit table in
//! `docs/rulebook/spec/kani/r.md`.
//!
//! Livehouse = colour group 6 buyable deeds (`r.md` header). Bare 「RiNG」 is
//! only the four `kind == "ring"` tiles.

use crate::world::{Effect, Snapshot, MAX_CARDS, MAX_PLAYERS};

// ===========================================================================
// Shared helpers
// ===========================================================================

/// Only the user's money may move among the money fields.
pub fn only_player_money_delta(
    pre: &Snapshot,
    post: &Snapshot,
    player: i32,
    delta: i32,
    n: i32,
) -> bool {
    let me = player as usize;
    if me >= MAX_PLAYERS {
        return false;
    }
    if post.money[me] != pre.money[me].saturating_add(delta) {
        return false;
    }
    for p in 0..MAX_PLAYERS {
        if p == me {
            continue;
        }
        if post.money[p] != pre.money[p] {
            return false;
        }
    }
    let _ = n;
    true
}

/// Exactly one `GainMoney` for `player` of `n`, nothing else.
pub fn only_gain_money(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainMoney { player: p, n: m } if p == player && m == n)
}

/// Exactly one `PayMoney` for `player` of `n`, nothing else.
pub fn only_pay_money(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::PayMoney { player: p, n: m } if p == player && m == n)
}

// ===========================================================================
// C-R-觉悟-01 / C-R-觉悟-02 -- `R:[衍生] 觉悟`
// 「[特]：此卡[移除]时获得1000资金。」 / 「[手]：[移除]此卡。」
// The [手] Play body is the activation that [移除]s the card (engine play
// pipeline) and pays the [特]. Body-local effect the book determines: +1000.
// ===========================================================================

/// The [特] payout is exactly 1000 to the card's user.
pub fn resolve_gain_amount() -> i32 {
    1000
}

/// Play body: exactly `GainMoney { player, 1000 }`. Zone motion of the played
/// copy is the engine play pipeline (`Dest::Banished` is the body's return).
pub fn resolve_play_log_ok(effects: &[Effect], player: i32) -> bool {
    only_gain_money(effects, player, resolve_gain_amount())
}

/// Post: the user's money rose by exactly 1000; nobody else moved.
pub fn resolve_play_post_ok(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    only_player_money_delta(pre, post, player, resolve_gain_amount(), 1)
        && crate::world::arr4_eq(post.pos, pre.pos)
        && crate::world::arr4_eq(post.charge, pre.charge)
        && crate::world::arr4_eq(post.stay, pre.stay)
        && crate::world::arr8b_eq(post.hand, pre.hand)
        && crate::world::arr8b_eq(post.discard, pre.discard)
}

/// MUTANT: wants 2000.
pub fn resolve_play_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    only_gain_money(effects, player, 2000)
}

// ===========================================================================
// C-R-曲奇时间-01 / C-R-曲奇时间-02 -- `R:曲奇时间`
// 「将自己弃牌堆的卡全部返回抽牌堆并洗切，获得500*X资金，X为返回卡的总数。」
// X is the discard size at play time (the played copy is not in it).
// Clause 01's pile multiset move is host-stubbed (`shuffle_into_deck`).
// ===========================================================================

/// X = number of cards returned = pre-discard size.
pub fn cookie_x(pre_discard_n: i32) -> i32 {
    pre_discard_n.max(0)
}

/// Payout is exactly `500 * X`.
pub fn cookie_pay_amount(x: i32) -> i32 {
    500 * x
}

/// Body-local money effect: exactly one `GainMoney` of `500*X`.
/// (Shuffle is clause 01 and is not an effect-log entry.)
pub fn cookie_pay_log_ok(effects: &[Effect], player: i32, x: i32) -> bool {
    only_gain_money(effects, player, cookie_pay_amount(x))
}

/// Post money rose by `500*X`.
pub fn cookie_pay_post_ok(pre: &Snapshot, post: &Snapshot, player: i32, x: i32) -> bool {
    only_player_money_delta(pre, post, player, cookie_pay_amount(x), 1)
}

/// MUTANT: off-by-one X (wants 500*(X+1)).
pub fn cookie_pay_amount_mutant(x: i32) -> i32 {
    500 * (x + 1)
}

// ===========================================================================
// C-R-Sprechchor-02 -- `R:Sprechchor`
// 「并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目」
// C-R-Sprechchor-01 (Livehouse turn-start gate) is the `cant_play` gate;
// the body is only the payout.
// ===========================================================================

/// Gain = 1000 + X*120 for the rolled face X ∈ 1..=20.
pub fn sprechchor_gain(x: i32) -> i32 {
    1000 + x * 120
}

/// Exactly one `GainMoney` of `1000 + X*120`.
pub fn sprechchor_log_ok(effects: &[Effect], player: i32, x: i32) -> bool {
    only_gain_money(effects, player, sprechchor_gain(x))
}

/// Post: only the user's money moved, by `1000 + X*120`.
pub fn sprechchor_post_ok(pre: &Snapshot, post: &Snapshot, player: i32, x: i32) -> bool {
    only_player_money_delta(pre, post, player, sprechchor_gain(x), 1)
        && crate::world::arr4_eq(post.pos, pre.pos)
        && crate::world::arr4_eq(post.charge, pre.charge)
        && crate::world::arr4_eq(post.stay, pre.stay)
}

/// MUTANT: off-by-one on the X term (1000 + (X+1)*120).
pub fn sprechchor_gain_mutant(x: i32) -> i32 {
    1000 + (x + 1) * 120
}

/// Face range the book's 1d20 determines.
pub fn sprechchor_face_ok(x: i32) -> bool {
    (1..=20).contains(&x)
}

// ===========================================================================
// C-R-黑暗大魔姬亚子-01/02/03 -- `R:（亚子）黑暗大魔姬亚子`
// 「（1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。
//  若此卡从手牌以外的地方打出，你抽一张卡。」
//
// Determined body-local effects (activation):
//   * +1 layer of [眩晕]  (clause 02)
//   * the pending pay is zeroed once stunned (rulebook 49 「无法收付款」; the
//     body's `set_pay_amount(0)` is that consequence)
//   * +1 draw when the card did not come from hand (clause 03)
// Clause 01's window is the [反击] protocol (PROTOCOL-LEVEL).
// `play_from_hand` is a host protocol stub (always 0), so the off-hand branch
// is the only one the Kani host can admit -- see the unit table.
// ===========================================================================

/// Stun delta the book pins.
pub fn ako_dark_stun_n() -> i32 {
    1
}

/// Draw count for an off-hand activation.
pub fn ako_dark_offhand_draw_n() -> i32 {
    1
}

/// Activation effects when the card is treated as off-hand (the only origin the
/// Kani host can report). Exactly: GiveStun{1}, SetPayAmount{0}, Draw{1}.
pub fn ako_dark_offhand_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::GiveStun { player: p, n } if p == player && n == ako_dark_stun_n())
        && matches!(effects[1], Effect::SetPayAmount { to: 0 })
        && matches!(effects[2], Effect::Draw { player: p, n } if p == player && n == ako_dark_offhand_draw_n())
}

/// Post: stun layer +1 ([眩晕] is `stun`, distinct from [停留]), money
/// unchanged at activation, pay_amount zeroed.
pub fn ako_dark_post_ok(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    let me = player as usize;
    me < MAX_PLAYERS
        && post.stun[me] == pre.stun[me].saturating_add(1)
        && crate::world::arr4_eq(post.money, pre.money)
        && post.pay_amount == 0
}

/// MUTANT: wants 2 stun layers.
pub fn ako_dark_stun_n_mutant() -> i32 {
    2
}

// ===========================================================================
// C-R-louder-01 -- `R:（ykn）louder`
// 「每次打出此卡时使ring的价格基础乘数+5」
// OPEN-RING-MULT: the book never defines 「价格基础乘数」. The only observable
// the host offers is `add_rent_unit_bonus` / `rent_unit_mult`, both protocol stubs
// with no world slot -- classified BLOCKED-HOST in the unit table. The pure
// spec below is the determined arithmetic (+5) the stub *should* observe.
// ===========================================================================

/// Determined delta: +5.
pub fn louder_mult_delta() -> i32 {
    5
}

/// If a ring-multiplier slot existed, post = pre + 5.
pub fn louder_mult_ok(pre_mult: i32, post_mult: i32) -> bool {
    post_mult == pre_mult + louder_mult_delta()
}

// ===========================================================================
// C-R-Fire bird-01 -- `R:Fire bird`
// 「支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶」
// OPEN-CHARGE-X: the book gives no range; the executable contract asks 1..=10
// and the body pins `X.max(1)`. Spec uses the body's offered range as the
// 'chooses some X' the text does determine (a positive count).
// ===========================================================================

/// Fixed activation cost.
pub fn fire_bird_cost() -> i32 {
    1600
}

/// Crystal count after the body's `x.max(1)` on an offered 1..=10 pick.
pub fn fire_bird_crystals(x: i32) -> i32 {
    if x < 1 {
        1
    } else {
        x
    }
}

/// Pay + place effects. The crystal write (`set_crystals` → `set_counter_self`)
/// lands as a trailing `AddCounter` under the current host.
pub fn fire_bird_place_log_ok(effects: &[Effect], player: i32, card: u8) -> bool {
    if effects.len() < 2 || effects.len() > 3 {
        return false;
    }
    matches!(effects[0], Effect::PayMoney { player: p, n } if p == player && n == fire_bird_cost())
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card: c
            } if p == player && c == card
        )
        && (effects.len() == 2 || matches!(effects[2], Effect::AddCounter { .. }))
}

/// Post: money -1600, card on the user's field.
pub fn fire_bird_place_post_ok(pre: &Snapshot, post: &Snapshot, player: i32) -> bool {
    only_player_money_delta(pre, post, player, -fire_bird_cost(), 1)
}

/// MUTANT: wants a 1600 gain instead of a pay.
pub fn fire_bird_place_log_ok_mutant(effects: &[Effect], player: i32, card: u8) -> bool {
    if effects.len() < 2 || effects.len() > 3 {
        return false;
    }
    matches!(effects[0], Effect::GainMoney { player: p, n } if p == player && n == fire_bird_cost())
        && matches!(
            effects[1],
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card: c
            } if p == player && c == card
        )
}

// ===========================================================================
// C-R-Fire bird-02 -- turn-end burn
// 「自己场地上存在此卡时，每回合结束时失去400资金并移除1个奇迹水晶」
// ===========================================================================

pub fn fire_bird_burn_pay() -> i32 {
    400
}

pub fn fire_bird_burn_crystals() -> i32 {
    1
}

/// PayMoney of 400; the crystal decay (`add_counter_self`) rides along as
/// `AddCounter` under the current host.
pub fn fire_bird_burn_log_ok(effects: &[Effect], player: i32) -> bool {
    let pay = fire_bird_burn_pay();
    effects
        .iter()
        .any(|e| matches!(e, Effect::PayMoney { player: p, n } if *p == player && *n == pay))
        && effects
            .iter()
            .all(|e| matches!(e, Effect::PayMoney { .. } | Effect::AddCounter { .. }))
}

/// MUTANT: wants a 400 gain.
pub fn fire_bird_burn_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    only_gain_money(effects, player, fire_bird_burn_pay())
}

// ===========================================================================
// C-R-Ringing Bloom-01 -- place on own field
// 「（1）将此卡放置于自身场上」
// ===========================================================================

/// Exactly one PlaceCard on the user's field (Dest::Field = 2).
pub fn ringing_bloom_place_log_ok(effects: &[Effect], player: i32, card: u8) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PlaceCard {
                player: p,
                dest: 2,
                card: c
            } if p == player && c == card
        )
}

/// MUTANT: two PlaceCards.
pub fn ringing_bloom_place_log_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 2
        && matches!(effects[0], Effect::PlaceCard { player: p, .. } if p == player)
        && matches!(effects[1], Effect::PlaceCard { player: p, .. } if p == player)
}

// ===========================================================================
// C-R-Fire bird-04 -- discard on empty crystals
// 「奇迹水晶耗尽时将此卡放入弃牌堆」
// Classified against the CounterChanged hook. Determined post: the instance
// is not on the field and the card is in the owner's discard.
// ===========================================================================

/// True when the instance's crystal count is exactly 0.
pub fn fire_bird_exhausted(crystals: i32) -> bool {
    crystals == 0
}

// ===========================================================================
// C-R-蓝玫瑰的骄傲-01 -- 20% of Livehouse deed values
// 「（1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些
// Livehouse格子地契价值20%的资金。」
// OPEN-ROUND10: rounding of 20% is not stated; the integer case is the pin.
// 「地契价值」 = printed tile price.
// ===========================================================================

/// Branch predicate: `#(owned Livehouse) ≥ #(owned non-Livehouse)`.
pub fn blue_rose_rich(n_live: i32, n_other: i32) -> bool {
    n_live >= n_other && n_live > 0
}

/// 20% of the sum of those tiles' printed prices (integer pin only).
/// OPEN-ROUND10: the book does not say how to round; the executable contract
/// pins the integer case (`sum * 20` divisible by 100). Off that case the
/// predicate returns `None`-like `-1` and callers must not assert.
pub fn blue_rose_payout(prices: &[i32]) -> i32 {
    let mut sum: i64 = 0;
    for &p in prices {
        sum += p as i64;
    }
    if (sum * 20) % 100 == 0 {
        ((sum * 20) / 100) as i32
    } else {
        -1 // rounding open
    }
}

/// The body's own rounding (`CeilTo(sum * 0.2, 10)`) is engine-side and is
/// NOT the spec when the book's 20% is non-integral. Kept for the mutant only.
pub fn blue_rose_payout_ceil10(prices: &[i32]) -> i32 {
    let mut sum: i64 = 0;
    for &p in prices {
        sum += p as i64;
    }
    (((sum + 49) / 50) * 10) as i32
}

/// Exactly one GainMoney of the 20% payout.
pub fn blue_rose_payout_log_ok(effects: &[Effect], player: i32, amount: i32) -> bool {
    only_gain_money(effects, player, amount)
}

/// MUTANT: 25% instead of 20%.
pub fn blue_rose_payout_mutant(prices: &[i32]) -> i32 {
    let mut sum: i64 = 0;
    for &p in prices {
        sum += p as i64;
    }
    ((sum * 25) / 100) as i32
}

// ===========================================================================
// C-R-NFO-03 -- face 1 = +1500
// 「若投掷结果为1，获得1500资金」
// (Faces 2-6 are their own clauses; face 6 runs 1-5 in order.)
// ===========================================================================

pub fn nfo_face1_gain() -> i32 {
    1500
}

/// Wrap formula the doc pins for faces > 6: `((face - 1) mod 6) + 1`.
/// Under the Kani host `roll(1,6)` is already 1..=6, so this is the identity.
pub fn nfo_wrap(face: i32) -> i32 {
    let f = face - 1;
    let m = f.rem_euclid(6);
    m + 1
}

/// Face-1 slice: exactly one GainMoney of 1500.
pub fn nfo_face1_log_ok(effects: &[Effect], player: i32) -> bool {
    only_gain_money(effects, player, nfo_face1_gain())
}

/// MUTANT: 1400.
pub fn nfo_face1_gain_mutant() -> i32 {
    1400
}

// ===========================================================================
// C-R-NFO-02 -- [停留] after the turn ends
// 「并在回合结束后获得一层[停留]」
// The AtEnd body is a separate entry: exactly GiveStay{1}.
// ===========================================================================

pub fn nfo_stay_n() -> i32 {
    1
}

pub fn nfo_stay_log_ok(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GiveStay { player: p, n } if p == player && n == nfo_stay_n())
}

/// MUTANT: 2 layers.
pub fn nfo_stay_n_mutant() -> i32 {
    2
}

// ===========================================================================
// Character skills -- fire-pot deltas (C-R-踏上荆棘之路的觉悟-01,
// C-R-即使1cm也要前进-01, C-R-慈爱女神-01, C-R-来练习吧-04/05).
// Caps are host-side (`charge_max`); the Kani `gain_charge` arm does not clamp
// (see the unit table). Specs pin the *delta* the text names.
// ===========================================================================

/// 冰川纱夜 (1): +2 on a CiRCLE pass; +1 when passed; +3 after an elimination.
pub fn sayo_fire_delta(circle: bool, passed: bool, someone_out: bool) -> i32 {
    if circle {
        if someone_out {
            3
        } else {
            2
        }
    } else if passed {
        1
    } else {
        0
    }
}

/// 白金燐子 (1): +3 on a CiRCLE pass.
pub fn rinko_fire_delta() -> i32 {
    3
}

/// 今井莉莎 (1): +1 on a CiRCLE pass.
pub fn lisa_fire_delta() -> i32 {
    1
}

/// 凑友希那 (3) landing on a RiNG: +1 火罐, cap 1.
pub fn yukina_fire_on_ring_delta() -> i32 {
    1
}

/// 凑友希那 (3) off every RiNG: fire becomes 0.
pub fn yukina_fire_drop_ok(post_fire: i32) -> bool {
    post_fire == 0
}

/// Exactly one `GainFire` of `n`.
pub fn gain_charge_log_ok(effects: &[Effect], player: i32, n: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n: m } if p == player && m == n)
}

/// MUTANT: +1 extra fire.
pub fn gain_charge_n_mutant(n: i32) -> i32 {
    n + 1
}

// ===========================================================================
// C-R-学生会的检查-02 -- add one `R:[衍生] 觉悟` to the draw pile
// 「向抽牌堆中加入一张“觉悟”」
// Known red (r.md Findings): the engine never inserts the card. The spec is
// the book's unconditional insert.
// ===========================================================================

/// The derived card's full id.
pub const RESOLVE_ID: &str = "R:[衍生] 觉悟";

/// Draw pile gains exactly one copy of `R:[衍生] 觉悟`.
pub fn student_council_add_resolve_ok(
    pre_deck_n: i32,
    post_deck_n: i32,
    pre_resolve_in_deck: i32,
    post_resolve_in_deck: i32,
) -> bool {
    post_deck_n == pre_deck_n + 1 && post_resolve_in_deck == pre_resolve_in_deck + 1
}

/// The insert is unconditional on the optional placement branch (r.md ruling).
/// Body-local observable: `add_to_deck` of one `R:[衍生] 觉悟` (silent write
/// under the current host — no effect-log entry).
pub fn student_council_add_resolve_delta(pre_deck_n: i32, post_deck_n: i32) -> bool {
    post_deck_n == pre_deck_n + 1
}

// ===========================================================================
// C-R-对音乐的纯粹-01/03 -- band skill half-price buys
// 「（1）本局游戏购买的第一个非Live House格子购买价格减半」
// 「（2）购买任何Live House格子的地契时购买价格减半」
// `trig_price` / `trig_set_price` are unmodeled (`KANI-COVERAGE.md`) --
// classified BLOCKED-HOST. Pure arithmetic the text determines:
// ===========================================================================

/// Half of the running purchase price (integer division; book says 「减半」
/// without a rounding rule -- even prices only in practice).
pub fn half_price(price: i32) -> i32 {
    price / 2
}

/// True when the tile is a Live House for that player (group 6 / pretend).
pub fn is_livehouse_group(group: i32) -> bool {
    group == 6
}

/// First non-Livehouse of the game: half price iff the one-shot is unused.
pub fn band_first_non_livehouse_price(price: i32, done: bool, is_live: bool) -> i32 {
    if is_live {
        return half_price(price);
    }
    if done {
        return price;
    }
    half_price(price)
}

/// MUTANT: three-quarters.
pub fn half_price_mutant(price: i32) -> i32 {
    price * 3 / 4
}

// ===========================================================================
// Classification stubs (no executable body-local spec).
// Listed here so the unit table's OPEN / PROTOCOL-LEVEL rows stay greppable.
// ===========================================================================

/// OPEN-分摊: NFO face 2's split of 1000 among the tile's players is undefined.
pub const OPEN_FENTAN: &str = "OPEN-分摊";
/// OPEN-触发结算: the settle body is undefined (README index).
pub const OPEN_TRIG_SETTLE: &str = "OPEN-触发结算";
/// OPEN-场上: NFO face 5 puts crystals on the character card, outside 「[场上]」.
pub const OPEN_CHANGSHANG: &str = "OPEN-场上";
/// OPEN-强制停下: definition truncated in `rulebook-doc.md:93`.
pub const OPEN_QIANGZHI: &str = "OPEN-强制停下";
/// OPEN-RING-BUILD: 向着顶点-02 vs 「所有RiNG不可升级」.
pub const OPEN_RING_BUILD: &str = "OPEN-RING-BUILD";
/// OPEN-STUDENT-REFUND: 学生会的检查-07 「本卡原应拿到的资金」.
pub const OPEN_STUDENT_REFUND: &str = "OPEN-STUDENT-REFUND";
/// OPEN-RING-MULT: louder 「ring的价格基础乘数」.
pub const OPEN_RING_MULT: &str = "OPEN-RING-MULT";
/// OPEN-RINGING-X: Ringing Bloom (3) X after (2)'s 「视为」.
pub const OPEN_RINGING_X: &str = "OPEN-RINGING-X";
/// OPEN-SAYO-TIMING: 弹奏 「时机合适时」.
pub const OPEN_SAYO_TIMING: &str = "OPEN-SAYO-TIMING";
/// OPEN-R1: Fire bird 1.5× stacking.
pub const OPEN_R1: &str = "OPEN-R1";
/// OPEN-ROUND10: rounding of 1.5× and 20%.
pub const OPEN_ROUND10: &str = "OPEN-ROUND10";
/// OPEN-CHARGE-X: Fire bird 「X（自选）」 range.
pub const OPEN_FIRE_X: &str = "OPEN-CHARGE-X";
/// OPEN-BAND-PRETEND: band (2) 「视为 Live House」.
pub const OPEN_BAND_PRETEND: &str = "OPEN-BAND-PRETEND";

/// Silence unused-import warnings for the pile bound used by future clauses.
#[allow(dead_code)]
fn _bounds(_n: usize, _cards: [bool; MAX_CARDS]) {}
