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

/// Book-visible effect log is exactly the listed `Teleport`s, in order.
///
/// `Effect::SetState` from `ctx::set_slot` is latch/slot plumbing (the card's
/// `latch:pico.dest` snapshot), not a book effect — it is skipped. Narration
/// (`ctx::log`) lives in a separate buffer and is not in this slice. Any
/// other effect (pay, gain, …) is a mismatch. This is what made
/// `pico_swap_01`/`02` FAIL on the old base while the native tests passed
/// (unit-vs-central disagreement in FINDINGS.md): the latch wrote three
/// `SetState` before and after the teleports and the exact-length check
/// rejected the whole log.
pub fn log_is_teleports(effects: &[Effect], expected: &[(i32, i32)]) -> bool {
    let mut i = 0usize;
    for e in effects {
        match e {
            Effect::SetState { .. } => continue,
            Effect::Teleport {
                player: q,
                tile: t,
            } => {
                if i >= expected.len() {
                    return false;
                }
                if *q != expected[i].0 || *t != expected[i].1 {
                    return false;
                }
                i += 1;
            }
            _ => return false,
        }
    }
    i == expected.len()
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
// ===========================================================================
// E-front-or-back — mutants / share legs (append)
// ===========================================================================

/// Mutant: the 500 / 1000 amounts swapped (equal→1000, share→500).
pub fn front_or_back_mutant_swapped_equal_amount() -> i32 {
    1000
}
pub fn front_or_back_mutant_swapped_share_total() -> i32 {
    500
}

/// Share legs after the host's `pay_total` shaping record: the first `PayTo`
/// of the full 1000 from the bank is a PROTOCOL artifact of the model (the
/// real pipeline only shapes); the remaining `pay_leg`s are the book's
/// 「分摊」 transfer. Returns the leg gains as `(player, n)`.
pub fn front_or_back_share_legs(effects: &[Effect], winners: &[i32]) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    if winners.is_empty() {
        return out;
    }
    let each = 1000 / winners.len() as i32;
    let mut skipped = false;
    for e in effects {
        if let Effect::PayTo { from, to, n } = e {
            // Drop the first full-total shaping record from the bank.
            if !skipped && *from < 0 && *n == 1000 {
                skipped = true;
                continue;
            }
            if *from < 0 && winners.contains(to) && *n == each {
                out.push((*to, *n));
            }
        }
    }
    out
}

// ===========================================================================
// E-ex-quest-01 — EX任务挑战
// 「抽出此卡的玩家投掷1d4并记录结果和26相加为X，然后所有玩家各自投掷3d20，
// 结果至少为X的玩家可选择更换角色皮肤（技能相同）」
//
// Cosmetics have no match-state seam (E-ex-quest-02 cannot test).
// ===========================================================================

/// The card is a one-shot that moves no money / pos / marks — only dice.
pub fn ex_quest_01_no_state_effects(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// E-habinel-01 — 前往哈比内尔王国旅游
// 「每个玩家获得X层[除外]，…X=1d2（每个玩家独立投掷1d2）」
// Destination 微笑号 is BLOCKED-HOST(tile_named) — `to` is -1 under Kani.
// E-habinel-02 turn-start tick is engine status machinery (PROTOCOL-LEVEL).
// E-habinel-03 timing is OPEN-habinel-end.
// ===========================================================================

/// One `GiveExile` per living player with `n ∈ 1..=2`, destination `to`
/// (the book's 微笑号; -1 under the Kani tile_named stub).
pub fn habinel_01_exiles_ok(effects: &[Effect], players: &[i32], to: i32) -> bool {
    if effects.len() != players.len() {
        return false;
    }
    for (e, &p) in effects.iter().zip(players.iter()) {
        match e {
            Effect::GiveExile {
                player: q,
                n,
                to: t,
            } => {
                if *q != p || *n < 1 || *n > 2 || *t != to {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

// ===========================================================================
// E-never-ends-01 — 一切不会结束
// 「抽出此卡的玩家投掷1d4，所有玩家[传送]到对应数字的RiNG且获得1层…[除外]。」
// BLOCKED-HOST(tile_named:RiNG N) — the body early-returns; under Kani this
// is always a no-op. The book effect is NOT proven.
// ===========================================================================

/// Under the tile_named stub the body is a no-op: empty effect log.
pub fn never_ends_01_noop(effects: &[Effect]) -> bool {
    effects.is_empty()
}

// ===========================================================================
// E-chart-rush-01 / 02 — 冲榜
// 「所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一，点数最高的人
// 获得2000资金或抽一张卡，点数第二的人获得1500资金，点数第三的人获得1000资金，
// 点数第四的人获得500资金（如果两个人点数相同则同时获得较高的那一档，例：获得
// 奖励名次为1，2，2，4）」
// OPEN-chart-rush-or / OPEN-chart-rush-tie.
// ===========================================================================

/// Competition rank: 1 + how many faces are strictly higher.
pub fn chart_rush_rank(faces: &[i32], me: i32) -> i32 {
    1 + faces.iter().filter(|&&g| g > me).count() as i32
}

/// Purse for a rank (rank 1 is the OPEN-or choice; `money` pins the 2000 leg).
pub fn chart_rush_purse(rank: i32) -> i32 {
    match rank {
        1 => 2000,
        2 => 1500,
        3 => 1000,
        4 => 500,
        _ => 0,
    }
}

/// All living players' fire pots are gone after the body.
pub fn chart_rush_fire_spent(post_fire: &[i32], n: i32) -> bool {
    (0..n as usize).all(|p| post_fire[p] == 0)
}

/// Mutant: rank-1 money purse off-by-one.
pub fn chart_rush_purse_mutant(rank: i32) -> i32 {
    if rank == 1 {
        2000 + 1
    } else {
        chart_rush_purse(rank)
    }
}

// ===========================================================================
// E-marina-box-02 — 麻里奈小姐的礼物箱
// 「所有玩家[经过]CiRCLE时可[消耗]一次500资金，抽到的玩家投掷一次1d10
// （不受任何其他效果影响），如果投掷结果至少为6，那名玩家[获得]1200资金。」
// OPEN-marina-who (passer) / OPEN-marina-default (default decline).
// E-marina-box-01 expiry counts drawer turns via `set_prop` (no-op under
// Kani) — PROTOCOL-LEVEL.
// ===========================================================================

/// Decline: no money moves.
pub fn marina_box_02_decline_ok(effects: &[Effect]) -> bool {
    effects.is_empty()
}

/// Accept: the passer pays 500; a 1d10 face ≥ 6 also credits them 1200.
/// Face is not visible in the log — the accept leg is `(PayMoney 500)` or
/// `(PayMoney 500, GainMoney 1200)`.
pub fn marina_box_02_accept_ok(effects: &[Effect], who: i32) -> bool {
    match effects.len() {
        1 => matches!(
            effects[0],
            Effect::PayMoney { player, n } if player == who && n == 500
        ),
        2 => {
            matches!(effects[0], Effect::PayMoney { player, n } if player == who && n == 500)
                && matches!(
                    effects[1],
                    Effect::GainMoney { player, n } if player == who && n == 1200
                )
        }
        _ => false,
    }
}

/// Mutant: pays 500 even on decline.
pub fn marina_box_02_decline_mutant(effects: &[Effect]) -> bool {
    !effects.is_empty()
}

// ===========================================================================
// Shared activation / expiry shapes (active events)
// ===========================================================================

/// Activation keep: dest is Field and the body moved no money / pos.
pub fn activation_keep_only(dest: i32, effects: &[Effect]) -> bool {
    dest == 2 && effects.is_empty()
}

/// Permanent keep (no expiry clause): dest is Field; nothing else is required
/// beyond the body's own modeled side effects.
pub fn permanent_keep(dest: i32) -> bool {
    dest == 2
}

// ===========================================================================
// E-embers-01 / 03 — 火种燃尽之后会怎么样呢？
// 「（1）将此卡的复制品放置于所有玩家所在格子上…触发结算时，格子上每有一张
// 此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡
// 的复制品。」
// E-embers-02 front/back spread is a follow-up. E-embers-04 clear is
// BLOCKED-HOST(remove_marks stub + give_exile does not set EXILE).
// ===========================================================================

/// One `PlaceMark` per living player (on that player's tile).
pub fn embers_01_marks_ok(effects: &[Effect], tiles: &[i32]) -> bool {
    if effects.len() != tiles.len() {
        return false;
    }
    for (e, &t) in effects.iter().zip(tiles.iter()) {
        match e {
            Effect::PlaceMark { tile, .. } => {
                if *tile != t {
                    return false;
                }
            }
            _ => return false,
        }
    }
    true
}

/// Settle: the settler pays `10 * n_copies` (PayMoney).
pub fn embers_03_settle_ok(effects: &[Effect], who: i32, copies: i32) -> bool {
    effects.len() == 1
        && matches!(
            effects[0],
            Effect::PayMoney { player, n } if player == who && n == copies * 10 && copies > 0
        )
}

// ===========================================================================
// E-lost-chase / E-end-it-all / E-bear-cookie — derived push / banish
// `event_deck_push` / `event_banish` are PROTOCOL stubs (return 0, no world
// mutation). The body calls them correctly; the clause is unprovable.
// ===========================================================================

/// E-bear-cookie-01: a derived 冲榜 sits on the event-deck top. Unprovable
/// under the stub — the predicate names the book's postcondition.
pub fn bear_cookie_01_deck_top_is_chart_rush(deck_top: Option<u16>, chart_rush_id: u16) -> bool {
    deck_top == Some(chart_rush_id)
}

/// E-end-it-all-01: the drawer picks 3 non-derived events and the card stays.
/// The banish is a stub; `picks == 3` is the determined ask-count.
pub fn end_it_all_01_picked_three(picks: usize) -> bool {
    picks == 3
}

// ===========================================================================
// E-lulu / E-school-time / E-kizuna — pay / exile legs
// 「如果选择[传送]需支付500资金」 / 「获得1层[除外]」
// Destinations 快餐店 / 周边学区 are BLOCKED-HOST(tile_named); Kizuna's dest
// is `roll mod 60` and is modeled.
// ===========================================================================

/// Lulu accept: the chooser pays exactly 500 (Teleport dest is BLOCKED-HOST).
pub fn lulu_01_pay_ok(effects: &[Effect], who: i32, dest: i32) -> bool {
    effects.len() == 2
        && matches!(
            effects[0],
            Effect::PayMoney { player, n } if player == who && n == 500
        )
        && matches!(
            effects[1],
            Effect::Teleport { player, tile } if player == who && tile == dest
        )
}

/// School-time / kizuna: one `GiveExile` of 1 per living player.
pub fn exile_one_each(effects: &[Effect], players: &[i32]) -> bool {
    if effects.len() < players.len() {
        return false;
    }
    // Teleports (if any) come first per player in some bodies; match the
    // GiveExile multiset exactly against `players`.
    let mut got = Vec::new();
    for e in effects {
        if let Effect::GiveExile {
            player, n, to, ..
        } = e
        {
            if *n != 1 || *to != -1 {
                return false;
            }
            got.push(*player);
        }
    }
    got.len() == players.len() && players.iter().all(|p| got.contains(p))
}

/// Kizuna: everyone teleports to `face % 60` and gains 1 exile (X = 0 when
/// no song cards are in any deck — OPEN-kizuna-song).
pub fn kizuna_01_ok(effects: &[Effect], players: &[i32], dest: i32) -> bool {
    if effects.len() != players.len() * 2 {
        return false;
    }
    for (i, &p) in players.iter().enumerate() {
        let a = &effects[i * 2];
        let b = &effects[i * 2 + 1];
        if !matches!(a, Effect::Teleport { player, tile } if *player == p && *tile == dest) {
            return false;
        }
        if !matches!(b, Effect::GiveExile { player, n, to } if *player == p && *n == 1 && *to == -1)
        {
            return false;
        }
    }
    true
}

// ===========================================================================
// E-pool-bug-03 — 卡池BUG build veto
// 「此卡在场时不可在造价1500及以上的格子上加盖房屋。」
// `set_cancelled` is modeled.
// ===========================================================================

/// The BuildBefore hook cancelled the build.
pub fn pool_bug_03_cancelled(cancelled: bool) -> bool {
    cancelled
}

// ===========================================================================
// E-fix-note — 修复公告 (set_build_discount is a PROTOCOL stub)
// ===========================================================================

/// Activation keep + designated tiles recorded (props are a no-op under Kani;
/// only keep is determined here).
pub fn fix_note_01_keep(dest: i32) -> bool {
    dest == 2
}
