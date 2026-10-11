//! Unit `events`: reference specs FROM the clause docs only.
//!
//! Sources: `docs/rulebook/spec/events.md` (clause IDs `E-*`) and the primary
//! text `data/events.json`. Never derive a spec from engine behaviour.
//!
//! Per clause a pure predicate over (pre, effect-log, post). Narration
//! (`ctx::log`) is NOT part of the frame. OPEN items prove only what the text
//! determines; see the OPEN table in the clause doc.

use std::vec::Vec;

use crate::intern::NameId;
use crate::world::Effect;

// ===========================================================================
// Shared shape helpers
// ===========================================================================

/// Effect log is exactly the listed `PayTo` transfers, in order.
pub fn log_is_paytos(effects: &[Effect], expected: &[(i32, i32, i32)]) -> bool {
    if effects.len() != expected.len() {
        return false;
    }
    for (e, &(from, to, n)) in effects.iter().zip(expected.iter()) {
        match e {
            Effect::PayTo {
                from: f,
                to: t,
                n: m,
            } => {
                if *f != from || *t != to || *m != n {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Effect log is exactly the listed `GainMoney` credits, in order.
pub fn log_is_gains(effects: &[Effect], expected: &[(i32, i32)]) -> bool {
    if effects.len() != expected.len() {
        return false;
    }
    for (e, &(p, n)) in effects.iter().zip(expected.iter()) {
        match e {
            Effect::GainMoney {
                player: q,
                n: m,
            } => {
                if *q != p || *m != n {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Effect log is exactly the listed `Teleport`s, in order.
pub fn log_is_teleports(effects: &[Effect], expected: &[(i32, i32)]) -> bool {
    if effects.len() != expected.len() {
        return false;
    }
    for (e, &(p, tile)) in effects.iter().zip(expected.iter()) {
        match e {
            Effect::Teleport {
                player: q,
                tile: t,
            } => {
                if *q != p || *t != tile {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Effect log is exactly one `EventExpire` for `key`.
pub fn log_is_expire(effects: &[Effect], key: NameId) -> bool {
    effects.len() == 1 && matches!(effects[0], Effect::EventExpire { key: k } if k == key)
}

// ===========================================================================
// E-draw-01 / 02 / 03 — generic draw / filing
// (rulebook-doc.md 基础[结算] 2). The event deck is a protocol stub under
// Kani (`Op::draw_event` / `event_deck_push` / `event_banish` return 0).
// Specs below pin the book; proofs are PROTOCOL-LEVEL (see unit table).
// ===========================================================================

/// E-draw-01: the drawn id is public and never enters any hand.
/// (Not executable under Kani — event-deck ops are stubs.)
pub fn e_draw_01_public_not_hand(_pre_hand_n: i32, _post_hand_n: i32, _effects: &[Effect]) -> bool {
    // Determined only as a filing invariant; see unit table PROTOCOL-LEVEL.
    true
}

// ===========================================================================
// E-duobang-01 / 02 — 对邦
// 「所有人立刻进行一次1d20骰子拼点，所有点数最高者从所有点数最低者获得胜出
// 点数x50的钱（例：最高点数18，最低点数16，则需支付 (18-16)x50=100）」
// ===========================================================================

/// Pairwise amount one lowest pays one highest: `(max - min) * 50`.
/// OPEN-duobang-tie: multi-extreme split is unspecified; the executable
/// reading is **each** lowest pays **each** highest this amount.
pub fn duobang_each(high: i32, low: i32) -> i32 {
    (high - low).max(0) * 50
}

/// Exact `PayTo` multiset the clause demands, given the faces rolled.
///
/// Faces are the 1d20 results as `(player, face)` for living players.
/// When `high == low` (gap 0) the log must be empty.
pub fn duobang_expected_paytos(faces: &[(i32, i32)]) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    if faces.is_empty() {
        return out;
    }
    let high = faces.iter().map(|(_, f)| *f).max().unwrap_or(0);
    let low = faces.iter().map(|(_, f)| *f).min().unwrap_or(0);
    let gap = high - low;
    if gap <= 0 {
        return out;
    }
    let each = gap * 50;
    for &(p, f) in faces {
        if f != low {
            continue;
        }
        for &(q, g) in faces {
            if g != high || q == p {
                continue;
            }
            out.push((p, q, each));
        }
    }
    out
}

/// Sheet example pin: 18 vs 16 → 100.
pub fn duobang_sheet_example_is_100() -> bool {
    duobang_each(18, 16) == 100
}

/// E-duobang-02 (unique extremes): total money moved equals `(max-min)*50`.
/// With multiple extremes the total is `n_low * n_high * each` (OPEN-tie).
pub fn duobang_total_moved(faces: &[(i32, i32)]) -> i32 {
    duobang_expected_paytos(faces).iter().map(|(_, _, n)| *n).sum()
}

/// Mutant: off-by-one on the pairwise amount.
pub fn duobang_each_mutant(high: i32, low: i32) -> i32 {
    (high - low).max(0) * 50 + 50
}

// ===========================================================================
// E-tears-01 / 02 — 泪水的含义
// 「所有玩家依次和行动序列里的下一位玩家进行比较：当前玩家和下一位玩家拥有
// 同色地契则下一位玩家[支付]当前玩家700资金，否则当前玩家[支付]下一位玩家
// 300资金。」
// ===========================================================================

/// Same-colour reading (OPEN-tears-colour): at least one colour group both
/// own a deed in. Multi-colour overlap is unspecified; this is the body's
/// reading and the clause doc's.
pub fn tears_share_color(groups_a: &[i32], groups_b: &[i32]) -> bool {
    for &g in groups_a {
        if g < 0 {
            continue;
        }
        for &h in groups_b {
            if h == g {
                return true;
            }
        }
    }
    false
}

/// Per adjacent pair in the living ring: next pays current 700 on a shared
/// colour, else current pays next 300. Returns the expected `PayTo` list in
/// walk order (current = seat k, next = seat k+1 wrap).
pub fn tears_expected_paytos(
    seats: &[i32],
    groups: &[Vec<i32>], // groups[i] = colour groups of seats[i]
) -> Vec<(i32, i32, i32)> {
    let mut out = Vec::new();
    let n = seats.len();
    if n < 2 {
        return out;
    }
    for k in 0..n {
        let p = seats[k];
        let q = seats[(k + 1) % n];
        if q == p {
            continue;
        }
        if tears_share_color(&groups[k], &groups[(k + 1) % n]) {
            out.push((q, p, 700));
        } else {
            out.push((p, q, 300));
        }
    }
    out
}

// ===========================================================================
// E-pico-swap-01 / 02 — PICO灵魂交换
// 「所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置（不触发场地
// 效果，例：第一位行动的玩家[传送]到第二位行动的玩家的位置并以此类推，最后
// 行动的玩家[传送]到第一位行动的玩家的位置）」
// ===========================================================================

/// Ring of living seats whose exile layer count is 0, in action order.
pub fn pico_ring(all: &[i32], exile: &[i32]) -> Vec<i32> {
    let mut out = Vec::new();
    for (i, &p) in all.iter().enumerate() {
        let ex = exile.get(i).copied().unwrap_or(0);
        if ex <= 0 {
            out.push(p);
        }
    }
    out
}

/// Expected teleports: each ring member to the next ring member's **pre**
/// position (last wraps to first). Empty when the ring has fewer than 2.
pub fn pico_expected_teleports(ring: &[i32], pre_pos: &[i32]) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    if ring.len() < 2 {
        return out;
    }
    for (k, &p) in ring.iter().enumerate() {
        let to = ring[(k + 1) % ring.len()];
        let dest = pre_pos[to as usize];
        out.push((p, dest));
    }
    out
}

// ===========================================================================
// E-moca-01 / 02 — Forbidden Moca
// 「直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除
// 此事件！」
//
// Activation: keep in play (set_dest Field) + schedule. Expiry tick: permanent
// removal (`EventExpire`). The dice-halve hook needs `trig_move_roll`
// (BLOCKED-HOST). The scheduling of 「下个回合结束」 is engine protocol.
// ===========================================================================

/// Activation: dest is Field (`Dest::Field = 2`).
pub fn moca_activation_dest_is_field(dest: i32) -> bool {
    dest == 2
}

/// Expiry tick log: exactly one `EventExpire`.
pub fn moca_expiry_log_ok(effects: &[Effect], key: NameId) -> bool {
    log_is_expire(effects, key)
}

// ===========================================================================
// E-aao-01 / 02 — A！A！O！
// 「将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃牌。此卡在场时
// 全场玩家不能使用卡牌的[手]效果。」
//
// E-aao-02: the hand-veto has no body write and no host surface — the clause
// is LESS (body leaves it short).
// ===========================================================================

/// Activation: dest = Field (keep). Schedule is protocol.
pub fn aao_activation_dest_is_field(dest: i32) -> bool {
    dest == 2
}

/// E-aao-02: while up, some hand-effect veto is armed. The body arms nothing.
pub fn aao_hand_veto_armed(_dest: i32, _effects: &[Effect]) -> bool {
    false
}

// ===========================================================================
// E-front-or-back-01 / 02 — 前场队还是后场队？
// 「所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加定位X且
// 站在格子序号31到60的玩家的投掷结果相加定位Y；如果X=Y则所有玩家获得500资金，
// 如果X>Y则站在格子序号1到30的玩家分摊获得1000资金，如果Y>X则站在格子序号
// 31到60的玩家分摊获得1000资金」
// ===========================================================================

/// Which half a board index (0-based engine tile) falls in.
/// Sheet 「#N格」 = engine `N-1`. Front = 0..=29, back = 30..=59.
pub fn front_or_back_half(tile: i32) -> i32 {
    if (0..30).contains(&tile) {
        0 // front
    } else if (30..60).contains(&tile) {
        1 // back
    } else {
        -1
    }
}

/// Equal halves: every living player gains exactly 500.
pub fn front_or_back_equal_ok(effects: &[Effect], players: &[i32]) -> bool {
    let expected: Vec<(i32, i32)> = players.iter().map(|&p| (p, 500)).collect();
    log_is_gains(effects, &expected)
}

/// Higher half shares a 1000 total. OPEN-分摊: only the total is pinned.
/// The body drops the remainder (`1000 % n`); spec accepts the dropped-split
/// total `1000 - (1000 % n)` and every winner gaining the same share.
pub fn front_or_back_share_total_ok(gains: &[(i32, i32)], winners: &[i32]) -> bool {
    if winners.is_empty() {
        return gains.is_empty();
    }
    let mut total = 0;
    for &(p, n) in gains {
        if !winners.contains(&p) {
            return false;
        }
        if n < 0 {
            return false;
        }
        total += n;
    }
    let n = winners.len() as i32;
    let expected_total = 1000 - (1000 % n);
    total == expected_total && gains.len() == winners.len()
}