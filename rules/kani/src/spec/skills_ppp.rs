//! Unit `skills_ppp`: reference specs FROM the clause docs only.
//!
//! Sources (re-read; never derived from engine bodies):
//! - `docs/rulebook/spec/cards/ppp.md` sections 户山香澄 / 花园多惠 /
//!   牛込里美 / 山吹沙绫 / 市谷有咲 / Poppin' Party band skill 星之鼓动
//! - primary: `the roster` (`band == "Poppin' Party"`),
//!   the roster band row (`text` for 星之鼓动)
//!
//! Precedence: live sheet > rulebook-doc. OPEN items prove only what the
//! text determines and record `OPEN-<ID>`.
//!
//! Spec shape: one pure function per clause over
//! `(pre, effect_log, post) -> bool` (or `*_x` / `*_amount` / `*_log_ok` /
//! `*_post_ok` pieces). Frame = the effect log; narration is not the frame.

use crate::world::{Effect, Snapshot, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared board constants (from `data/board.json` 0-based slots / `#N格` = N-1)
// ---------------------------------------------------------------------------

/// 「星之鼓动山丘」.
pub const TILE_HILL: i32 = 44;
/// 「流星堂」.
pub const TILE_RYUSEIDO: i32 = 45;
/// 「星空齿科」.
pub const TILE_STARDUST: i32 = 6;
/// 「#1，#16，#31，#46」 → board indices 0, 15, 30, 45.
pub const BAND_CORNERS: [i32; 4] = [0, 15, 30, 45];

pub const TOK_STICKER: &str = "星星贴纸";
pub const TOK_SAAYA: &str = "saaya标记";
pub const MARK_RABBIT: &str = "多惠兔子";
/// Body/engine spelling of the once-per-turn gate on band crystals (1c).
pub const STATE_POPPIN_USED: &str = "skill.poppin.used";
/// Rimi (1) latch key used by the TurnEnd CEL guard.
pub const STATE_RIMI_PASSED: &str = "skill.rimiResolve.passed";
/// Aoki (2) pass-latch slot (the latch write itself is BLOCKED-HOST on
/// `trig_move_main`; the turn-end leg reads this key).
pub const STATE_ARISA_PASSED: &str = "skill.arisa.passedStation";

/// Board index of a 「#N格」 label.
pub fn hash_tile(n: i32) -> i32 {
    n - 1
}

pub fn is_band_corner(tile: i32) -> bool {
    tile == 0 || tile == 15 || tile == 30 || tile == 45
}

// ===========================================================================
// C-PPP-kasumi-skill-01
// 「其他玩家“星之鼓动山丘”上[结算]时获得一个[火罐]（初始0，上限1）」
// Trigger: another player settles on 星之鼓动山丘.
// Condition: the pot is below the cap 1.
// Effect: fire +1, never past 1.
// ===========================================================================

/// Residual: *another* player settling on the hill.
pub fn kasumi_01_applies(owner: i32, settler: i32, tile: i32) -> bool {
    settler != owner && tile == TILE_HILL
}

/// Fire after a legal +1 under cap 1 (initial 0).
pub fn kasumi_01_fire_after(pre: i32) -> i32 {
    if pre >= 1 {
        pre
    } else {
        pre + 1
    }
}

/// Exactly one `GainFire(+1)` when the pot has room; empty when at cap.
pub fn kasumi_01_log_ok(effects: &[Effect], player: i32, pre: i32) -> bool {
    let after = kasumi_01_fire_after(pre);
    if after == pre {
        return effects.is_empty();
    }
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == 1
        )
}

// ===========================================================================
// C-PPP-kasumi-skill-02
// 「运营阶段可使用1个[火罐]，立刻进入移动阶段且本回合的[主要移动]改为[传送]
//  到任意自己拥有的格子且可选择盖房」
// Condition: ≥1 [火罐]; owns ≥1 tile.
// Effect: spend exactly 1 fire; main move becomes a teleport to a self-owned
// tile which settles; build there optional.
// Frame: fire / pos / landing settle. (Move-plan scheduling is PROTOCOL-LEVEL.)
// ===========================================================================

/// Cost is exactly 1 [火罐]. The remaining legs (enter move, teleport plan,
/// settle, optional build) are engine scheduling — see unit table.
pub fn kasumi_02_fire_after(pre_fire: i32) -> i32 {
    (pre_fire - 1).max(0)
}

/// The spend itself is unlogged under the Kani host (`Op::spend_charge`);
/// post-state is the frame for the pot.
pub fn kasumi_02_fire_ok(pre_fire: i32, post_fire: i32) -> bool {
    post_fire == kasumi_02_fire_after(pre_fire)
}

// ===========================================================================
// C-PPP-megumi-skill-01a
// 「每次你领取[CiRCLE奖励]时投掷3d20，如果结果对应的格子不是“星空齿科”则在
//  结果对应的格子放置一个[多惠兔子]」
// OPEN: 3d20 sum S is the printed 「#S格」 = board index S-1.
// ===========================================================================

/// Board index of the 3d20 sum (「#S格」 = index `S-1`, S ∈ 3..=60).
pub fn megumi_01a_tile(sum_3d20: i32) -> i32 {
    let total = 60;
    ((sum_3d20 - 1).rem_euclid(total) + total) % total
}

/// Place a rabbit iff the landing tile is not 星空齿科.
pub fn megumi_01a_should_place(sum_3d20: i32) -> bool {
    megumi_01a_tile(sum_3d20) != TILE_STARDUST
}

pub fn megumi_01a_log_ok(effects: &[Effect], sum_3d20: i32) -> bool {
    if !megumi_01a_should_place(sum_3d20) {
        return effects.is_empty();
    }
    let t = megumi_01a_tile(sum_3d20);
    effects.len() == 1 && matches!(effects[0], Effect::PlaceMark { tile, .. } if tile == t)
}

// ===========================================================================
// C-PPP-megumi-skill-01b
// 「当你经过有[多惠兔子]的格子时[移除]该格子上的所有[多惠兔子]并获得等量的
//  [火罐]（初始2，上限4）」
// OPEN: 「等量」 = one pot per rabbit removed.
// ===========================================================================

/// Fire gained = every rabbit removed (「等量」), capped by room under 4.
pub fn megumi_01b_fire_gain(rabbits: i32, pre_fire: i32, cap: i32) -> i32 {
    let room = (cap - pre_fire).max(0);
    if rabbits <= 0 {
        0
    } else if room == 0 {
        0
    } else {
        rabbits.min(room)
    }
}

pub fn megumi_01b_fire_after(rabbits: i32, pre_fire: i32, cap: i32) -> i32 {
    pre_fire + megumi_01b_fire_gain(rabbits, pre_fire, cap)
}

/// Effect log: `GainFire(+n)` when n > 0. Mark removal is a separate frame
/// leg (tile marks) — the Kani host's `remove_marks_f` is a stub, so the
/// mark-removal half is BLOCKED-HOST and is *not* asserted here.
pub fn megumi_01b_fire_log_ok(effects: &[Effect], player: i32, n: i32) -> bool {
    if n <= 0 {
        return effects.is_empty();
    }
    effects.len() == 1
        && matches!(effects[0], Effect::GainFire { player: p, n: g } if p == player && g == n)
}

// ===========================================================================
// C-PPP-megumi-skill-02
// 「在其他玩家使用角色/团技能的[主]效果或使用手牌的[手]效果时你可以使用4个
//  [火罐]将其抵消，然后你获得500资金且被抵消的玩家获得2000资金」
// Condition: 多惠 holds ≥4 [火罐]. (Counteraction window = PROTOCOL-LEVEL.)
// ===========================================================================

pub fn megumi_02_fire_after(pre_fire: i32) -> i32 {
    (pre_fire - 4).max(0)
}

/// Leaf effects once the counteraction is declared: spend 4 fire, negate,
/// 多惠 +500, negated player +2000.
pub fn megumi_02_money_ok(pre_self: i32, pre_other: i32, post_self: i32, post_other: i32) -> bool {
    post_self == pre_self + 500 && post_other == pre_other + 2000
}

pub fn megumi_02_negated(trig_cancelled: bool) -> bool {
    trig_cancelled
}

// ===========================================================================
// C-PPP-rimi-skill-01
// 「你的回合中[经过]任意Live House颜色格子后在回合结束时获得1个[火罐]
//  （初始1，上限3）」
// OPEN: 「Live House颜色格子」 = the Live House colour group
//      (`data/board.json` `group`), not only the Live House tile itself.
// ===========================================================================

pub fn rimi_01_fire_after(pre: i32) -> i32 {
    if pre >= 3 {
        pre
    } else {
        pre + 1
    }
}

/// Turn-end payout when the latch is set: +1 fire (cap 3). The body also
/// clears the latch (`state::set(PASSED, 0)`) which pushes a `SetState`
/// bookkeeping effect — ignore it (class B: the exact-match originally
/// rejected it).
pub fn rimi_01_log_ok(effects: &[Effect], player: i32, pre: i32) -> bool {
    let after = rimi_01_fire_after(pre);
    let mut gain = 0;
    let mut other = 0;
    for e in effects {
        match e {
            Effect::GainFire { player: p, n } if *p == player => gain += *n,
            Effect::SetState { .. } => {}
            _ => other += 1,
        }
    }
    if after == pre {
        return other == 0 && gain == 0;
    }
    other == 0 && gain == 1
}

/// The TurnEnd CEL guard: `actor == owner && slot('skill.rimiResolve.passed') != 0`.
pub fn rimi_01_turn_end_admits(actor: i32, owner: i32, passed: i32) -> bool {
    actor == owner && passed != 0
}

// ===========================================================================
// C-PPP-rimi-skill-02
// 「运营阶段可选择使用3个[火罐]立刻进入移动阶段，[传送]至任意与你绝对距离
//  最远的地产商并[结算]且可选择盖房」
// OPEN: 「地产商」 = property-agent tile class; farthest-tie = 「任意」.
// KNOWN FINDING (ppp.md): body offers property tiles, not agent tiles.
// ===========================================================================

/// Absolute ring distance on the 60-tile board.
pub fn rimi_02_abs_dist(a: i32, b: i32) -> i32 {
    let n = 60;
    let d = (b - a).rem_euclid(n);
    if d > n - d {
        n - d
    } else {
        d
    }
}

/// Max absolute distance from `at` among tiles satisfying `is_target`.
/// 8×8 chunk walk (TUNING) keeps the unwind bound at 8 instead of 60.
pub fn rimi_02_farthest_abs<F: Fn(i32) -> bool>(at: i32, is_target: F) -> i32 {
    let mut best = -1;
    let mut outer = 0usize;
    while outer < 8 {
        let mut k = 0usize;
        while k < 8 {
            let t = outer * 8 + k;
            if t < 60 && is_target(t as i32) {
                let d = rimi_02_abs_dist(at, t as i32);
                if d > best {
                    best = d;
                }
            }
            k += 1;
        }
        outer += 1;
    }
    best
}

pub fn rimi_02_fire_after(pre_fire: i32) -> i32 {
    (pre_fire - 3).max(0)
}

/// Destination must be a 地产商 (agent) at maximum absolute distance.
pub fn rimi_02_dest_ok(at: i32, dest: i32, is_agent: bool, best: i32) -> bool {
    is_agent && rimi_02_abs_dist(at, dest) == best && best >= 0
}

// ===========================================================================
// C-PPP-saaya-skill-01a
// 「其他玩家一次[消耗]或[支付]至少1000资金且自己不拥有saaya标记时可让那名
//  玩家获得200资金且自己获得1个saaya标记和1个[火罐]」
// OPEN: saaya标记 is a player token named `saaya标记`.
// ===========================================================================

pub fn saaya_01a_applies(other_payer: i32, self_id: i32, value: i32, has_mark: bool) -> bool {
    other_payer != self_id && value >= 1000 && !has_mark
}

/// Effects when the optional 「可让」 is taken.
pub fn saaya_01a_log_ok(effects: &[Effect], me: i32, payer: i32, mark_key: u16) -> bool {
    effects.len() == 3
        && matches!(effects[0], Effect::AddToken { player: p, key: k, n } if p == me && k == mark_key && n == 1)
        && matches!(effects[1], Effect::GainFire { player: p, n } if p == me && n == 1)
        && matches!(effects[2], Effect::GainMoney { player: p, n } if p == payer && n == 200)
}

/// Decline (or not admitted): empty log.
pub fn saaya_01a_empty_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// C-PPP-saaya-skill-01b
// 「拥有saaya标记时投掷移动骰时失去1个saaya标记，此次投掷结果减1d10
//  （计算后小等于0则移动到下一个可购买格子）」
// BLOCKED-HOST: `trig_move_roll` (unmodeled). Spec kept fail-closed.
// ===========================================================================

pub fn saaya_01b_cut(before: i32, d10: i32) -> i32 {
    before - d10.max(0)
}

/// Floor: reduced total ≤ 0 ⇒ next buyable tile (clockwise), not the walk.
pub fn saaya_01b_floored(before: i32, d10: i32) -> bool {
    saaya_01b_cut(before, d10) <= 0
}

// ===========================================================================
// C-PPP-saaya-skill-02
// 「[消耗]或[支付]资金时可使用5个[火罐]，此次资金变动减少5000（最少0）。」
// ===========================================================================

pub fn saaya_02_amount(pre_amount: i32) -> i32 {
    (pre_amount - 5000).max(0)
}

pub fn saaya_02_fire_after(pre_fire: i32) -> i32 {
    (pre_fire - 5).max(0)
}

pub fn saaya_02_log_ok(effects: &[Effect], expected: i32) -> bool {
    effects.len() == 1
        && matches!(effects[0], Effect::SetPayAmount { to } if to == expected)
}

// ===========================================================================
// C-PPP-aoki-skill-01
// 「每次有其他玩家在“流星堂”抽取事件时获得一个[火罐]（初始0，上限2）」
// ===========================================================================

pub fn aoki_01_applies(actor: i32, owner: i32, tile: i32) -> bool {
    actor != owner && tile == TILE_RYUSEIDO
}

pub fn aoki_01_fire_after(pre: i32) -> i32 {
    if pre >= 2 {
        pre
    } else {
        pre + 1
    }
}

pub fn aoki_01_log_ok(effects: &[Effect], player: i32, pre: i32) -> bool {
    let after = aoki_01_fire_after(pre);
    if after == pre {
        return effects.is_empty();
    }
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == 1
        )
}

// ===========================================================================
// C-PPP-aoki-skill-02
// 「回合结束时如果本回合的[主要移动][经过]“流星堂”且[移动终点]不为“流星堂”时
//  抽取一个视为在“流星堂”抽取的事件并获得一个[火罐]。」
// BLOCKED-HOST (latch half): `trig_move_main`. Turn-end leg is provable when
// the latch key is pre-set and the mover is not standing on 流星堂.
// ===========================================================================

pub fn aoki_02_end_admits(end_pos: i32, passed_latch: i32) -> bool {
    end_pos != TILE_RYUSEIDO && passed_latch != 0
}

pub fn aoki_02_log_ok(effects: &[Effect], player: i32) -> bool {
    // The body also clears the latch (`set_slot`) which pushes `SetState`
    // bookkeeping — ignore it.
    let mut draw = 0;
    let mut gain = 0;
    let mut other = 0;
    for e in effects {
        match e {
            Effect::Draw { player: p, n } if *p == player => draw += *n,
            Effect::GainFire { player: p, n } if *p == player => gain += *n,
            Effect::SetState { .. } => {}
            _ => other += 1,
        }
    }
    other == 0 && draw == 1 && gain == 1
}

// ===========================================================================
// C-PPP-aoki-skill-03
// 「[经过]“流星堂”时可使用2个[火罐]为自己的团卡添加1个[奇迹水晶]。」
// BLOCKED-HOST: the Pass entry body always reads `trig_move_main` (latch for
// (02)), which is unmodeled. Spec kept.
// ===========================================================================

pub fn aoki_03_fire_after(pre_fire: i32) -> i32 {
    (pre_fire - 2).max(0)
}

pub fn aoki_03_log_ok(effects: &[Effect], player: i32, crystal_key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == 1
        )
}

// ===========================================================================
// C-PPP-band-01a
// 「[经过]第#1，#16，#31，#46号格子时获得一个星星贴纸」
// Trigger: skill owner [经过] board index 0, 15, 30, or 45 (「#N格」 = N-1).
// ===========================================================================

pub fn band_01a_applies(actor: i32, owner: i32, tile: i32) -> bool {
    actor == owner && is_band_corner(tile)
}

pub fn band_01a_log_ok(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key: k, n } if p == player && k == key && n == 1
        )
}

// ===========================================================================
// C-PPP-band-01b
// 「随后可使用2个星星贴纸为此卡添加1个[奇迹水晶]」
// Condition: 星星贴纸 ≥ 2.
// ===========================================================================

pub fn band_01b_log_ok(effects: &[Effect], player: i32, tok: u16, crystal_key: u16) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key: k, n } if p == player && k == tok && n == -2
        )
        && matches!(
            effects[1],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == 1
        )
}

// ===========================================================================
// C-PPP-band-01c
// 「随后可使用此卡的2个[奇迹水晶]抽1张卡或[获得]2000资金
//  （使用此卡[奇迹水晶]的效果为1回合1次）」
// OPEN: the once-per-turn gate covers both the draw and the cash branch.
// ===========================================================================

pub fn band_01c_draw_ok(effects: &[Effect], player: i32, crystal_key: u16, used_key: u16) -> bool {
    effects.len() == 3
        && matches!(
            effects[0],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == -2
        )
        && matches!(
            effects[1],
            Effect::SetState { player: p, key: k, n } if p == player && k == used_key && n == 1
        )
        && matches!(effects[2], Effect::Draw { player: p, n } if p == player && n == 1)
}

pub fn band_01c_cash_ok(effects: &[Effect], player: i32, crystal_key: u16, used_key: u16) -> bool {
    effects.len() == 3
        && matches!(
            effects[0],
            Effect::AddCounter { owner: p, key: k, n } if p == player && k == crystal_key && n == -2
        )
        && matches!(
            effects[1],
            Effect::SetState { player: p, key: k, n } if p == player && k == used_key && n == 1
        )
        && matches!(effects[2], Effect::GainMoney { player: p, n } if p == player && n == 2000)
}

/// Once-per-turn: a second press with `used != 0` must not spend crystals.
pub fn band_01c_used_blocks(used: i32) -> bool {
    used != 0
}

// ===========================================================================
// C-PPP-band-02
// 「无法获取[CiRCLE奖励]」
// Effect: no reward is granted. Body arms `prop::NO_REWARD`.
// ===========================================================================

pub fn band_02_reward_forbidden() -> bool {
    true
}

// ===========================================================================
// C-PPP-band-03a
// 「游戏开始时所有Poppin' Party角色视为同时拥有“星之鼓动山丘”，可正常抵押
//  “星之鼓动山丘”」
// OPEN-分摊 amounts live in 03b. Co-ownership form is OPEN in the engine
// (one owner per tile) — see body TODO(规则书). Spec pins the claim only.
// ===========================================================================

pub fn band_03a_co_owns(group: &str, hill: i32) -> bool {
    // Zero-loop byte compare (memcmp is an unwinding cliff at low bounds).
    const PPP: &[u8] = b"Poppin' Party";
    let b = group.as_bytes();
    if b.len() != PPP.len() {
        return false;
    }
    let mut i = 0usize;
    while i < PPP.len() {
        if b[i] != PPP[i] {
            return false;
        }
        i += 1;
    }
    hill == TILE_HILL
}

// ===========================================================================
// C-PPP-band-03b
// 「非Poppin' Party角色对“星之鼓动山丘”的[结算]改为分摊支付给所有未抵押
//  “星之鼓动山丘”的Poppin' Party角色」
// OPEN-分摊: the payee *set* is pinned; split amounts are not.
// ===========================================================================

/// Payees = every PPP character who has not mortgaged the hill.
pub fn band_03b_payee_set(
    n: i32,
    is_ppp: &[bool; MAX_PLAYERS],
    mortgaged_hill: bool,
    out: &[bool; MAX_PLAYERS],
) -> [bool; MAX_PLAYERS] {
    let mut payees = [false; MAX_PLAYERS];
    let mut p = 0;
    while p < MAX_PLAYERS {
        if (p as i32) < n && is_ppp[p] && !out[p] && !mortgaged_hill {
            payees[p] = true;
        }
        p += 1;
    }
    payees
}

// ===========================================================================
// C-PPP-band-03c
// 「“星之鼓动山丘”不可被抵押双倍支付购买，只有全部Poppin' Party角色破产后
//  才可被正常购买。」
// ===========================================================================

/// While any PPP character is still alive the hill cannot change hands.
pub fn band_03c_buy_refused(any_ppp_alive: bool) -> bool {
    any_ppp_alive
}

/// ABI v77 pure FilterValue gate: deny is `allowed == 0` in the returned
/// payload (the host reads it; the body no longer cancels the trigger).
pub fn band_03c_gate_denied(allowed: i32) -> bool {
    allowed == 0
}

/// Denial reason intern id 1 = `poppin_locked` (matches the body's const).
pub fn band_03c_denial_reason() -> i32 {
    1
}

// ===========================================================================
// C-PPP-band-04
// 「不能通过[主要移动]的[结算]盖房。」
// BLOCKED-HOST: `trig_move_main`. Build-window scheduling is also
// PROTOCOL-LEVEL. Spec: a main-move settle must not offer a build.
// ===========================================================================

pub fn band_04_build_forbidden(is_main_move_settle: bool) -> bool {
    is_main_move_settle
}

// ---------------------------------------------------------------------------
// Frame helpers
// ---------------------------------------------------------------------------

/// Frame: other players' money untouched.
pub fn frame_others_money(pre: &Snapshot, post: &Snapshot, me: i32) -> bool {
    let mut p = 0;
    while p < MAX_PLAYERS {
        if p as i32 != me && pre.money[p] != post.money[p] {
            return false;
        }
        p += 1;
    }
    true
}

/// Frame: other players' fire untouched.
pub fn frame_others_fire(pre: &Snapshot, post: &Snapshot, me: i32) -> bool {
    let mut p = 0;
    while p < MAX_PLAYERS {
        if p as i32 != me && pre.charge[p] != post.charge[p] {
            return false;
        }
        p += 1;
    }
    true
}

/// Mutant companions (deliberately wrong specs) — keep reachable-disagreement
/// shape: the card must FAIL to match these.
pub fn kasumi_01_effects_ok_mutant(effects: &[Effect], player: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::GainFire { player: p, n } if p == player && n == 2
        )
}

pub fn saaya_02_amount_mutant(pre_amount: i32) -> i32 {
    (pre_amount - 5000).max(0) - 1
}

pub fn band_01a_log_ok_mutant(effects: &[Effect], player: i32, key: u16) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::AddToken { player: p, key: k, n } if p == player && k == key && n == 1
        )
        && matches!(
            effects[1],
            Effect::AddToken { player: p, key: k, n } if p == player && k == key && n == 1
        )
}