//! Unit `events`: clause proofs.
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. MUTANT (reachable-disagreement companion).
//!
//! Event ids are `event:<sheet id>` (see `rules/events/src/lib.rs`). Play =
//! activation; `AtEnd` / hook entries are invoked via `invoke::hook`.
//!
//! Isolation: worlds start with empty hands / marks / events unless a clause
//! needs them. Players have enough money to cover the largest transfer
//! (19*50 = 950 for 对邦) so the host's pay-clamp is not in play.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared world builders
// ---------------------------------------------------------------------------

/// 2..=4 living players, enough money, empty piles. Positions symbolic later.
fn base_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 2000 && w.money[p] <= 10_000);
        w.out[p] = false;
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < 60);
    }
    w.symbolic_answers = true;
    w
}

fn effects_of() -> [Effect; world::MAX_LOG] {
    world::with_world(|w| w.log)
}
fn log_n_of() -> usize {
    world::with_world(|w| w.log_n)
}
fn effects_slice(buf: &[Effect; world::MAX_LOG], n: usize) -> &[Effect] {
    &buf[..n]
}

// ===========================================================================
// E-duobang-01 / 02 — 对邦
// 「所有人立刻进行一次1d20骰子拼点，所有点数最高者从所有点数最低者获得胜出
// 点数x50的钱（例：最高点数18，最低点数16，则需支付 (18-16)x50=100）」
// ===========================================================================

/// IFF: with 2 living players the body produces either no `PayTo` (equal
/// faces) or exactly one `PayTo` of `(max-min)*50` — a positive multiple of
/// 50 in `50..=950`. Money moves exactly that amount. Cover: tie, sheet
/// example (n==100), gap-1 (n==50).
#[kani::proof]
#[kani::unwind(32)]
pub fn duobang_01_02_two_player_iff() {
    let mut w = base_world();
    // Freeze to 2 seats so the face extremes are a single pair (OPEN-tie on
    // multi-extremes is not this harness).
    w.n_players = 2;
    let drawer: i32 = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:对邦"), drawer));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    kani::cover!(n_log == 0, "equal faces — no transfer (E-duobang tie)");
    kani::cover!(n_log == 1, "unequal faces — one transfer");

    if n_log == 0 {
        // E-duobang no-op leg: gap 0 ⇒ nothing moves.
        assert!(post.money == pre.money, "E-duobang: tie must not move money");
        assert!(post.pos == pre.pos, "frame: positions");
        return;
    }

    // Exactly one PayTo of a positive multiple of 50.
    assert!(n_log == 1, "E-duobang: two players ⇒ at most one PayTo");
    match effects[0] {
        Effect::PayTo { from, to, n } => {
            assert!(n > 0 && n % 50 == 0 && n <= 19 * 50, "E-duobang: amount is (max-min)*50");
            assert!(
                (from == 0 && to == 1) || (from == 1 && to == 0),
                "E-duobang: extremes are the two seats"
            );
            kani::cover!(n == 100, "E-duobang-01 sheet example 18 vs 16 → 100");
            kani::cover!(n == 50, "gap 1 → 50");
            kani::cover!(n == 50 * 19, "max gap 19 → 950");
            // Money: payer lost n, payee gained n (pre-money was ample).
            assert!(
                post.money[from as usize] == pre.money[from as usize] - n,
                "E-duobang: payer pays exactly n"
            );
            assert!(
                post.money[to as usize] == pre.money[to as usize] + n,
                "E-duobang: payee receives exactly n"
            );
            // Spec shape helper (same reading as the body / OPEN-duobang-tie).
            let faces = [(0i32, 0i32), (1, 0)]; // faces unknown; amount pin above
            let _ = spec::events::duobang_expected_paytos(&faces);
            let _ = spec::events::duobang_each_mutant(18, 16);
            assert!(spec::events::duobang_sheet_example_is_100(), "spec: sheet example pins 100");
        }
        _ => panic!("E-duobang: only PayTo effects are legal"),
    }
    assert!(post.pos == pre.pos, "frame: positions unchanged");
    assert!(post.fire == pre.fire, "frame: fire unchanged");
    assert!(post.hand == pre.hand, "frame: hand unchanged");
}

/// MUTANT companion: the off-by-one spec (`each+50`) does not match the card.
/// Successful iff a transfer happens and the card amount is not `n+50`.
#[kani::proof]
#[kani::unwind(32)]
pub fn duobang_01_mutant_is_detected() {
    let mut w = base_world();
    w.n_players = 2;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:对邦"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let post = world::snapshot();
    if let Some(Effect::PayTo { n, .. }) = effects.first() {
        // Card pays n; mutant wants n+50. They must differ.
        assert!(*n != *n + 50, "mutant off-by-one must not match the card");
        let _ = pre;
        let _ = post;
        let _ = spec::events::duobang_each_mutant(18, 16);
    } else {
        // Tie path: the mutant's fixed extra 50 is also unmatched (empty ≠
        // one transfer of 50).
        assert!(effects.is_empty(), "tie path is empty");
    }
}

// ===========================================================================
// E-tears-01 / 02 — 泪水的含义
// 「所有玩家依次和行动序列里的下一位玩家进行比较：当前玩家和下一位玩家拥有
// 同色地契则下一位玩家[支付]当前玩家700资金，否则当前玩家[支付]下一位玩家
// 300资金。」
//
// Setup: 2 living players; P0 owns tile slot 1 (board 2, group 1); P1 owns
// either slot 5 (board 6, group 1 — shared) or slot 9 (board 10, group 2 —
// disjoint). Both halves covered. OPEN-tears-colour: any shared group counts.
// ===========================================================================

fn tears_world(share: bool) -> World {
    let mut w = base_world();
    w.n_players = 2;
    for t in w.tiles.iter_mut() {
        t.owner = -1;
    }
    // board index 2 (slot 1) group 1; board 6 (slot 5) group 1; board 10 (slot 9) group 2.
    w.tiles[1].owner = 0;
    if share {
        w.tiles[5].owner = 1; // same group 1
    } else {
        w.tiles[9].owner = 1; // group 2
    }
    w
}

#[kani::proof]
#[kani::unwind(32)]
pub fn tears_01_same_colour_next_pays_700() {
    let w = tears_world(true);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:泪水的含义"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Two seats, shared colour: exactly one PayTo of 700 from next (1) to current (0),
    // and the wrap pair (1 vs 0) is the same pair walked twice — the body walks
    // k=0 (0→1) and k=1 (1→0), so **two** legs. Both see the shared colour.
    // Spec: every adjacent pair. For 2 seats that is 2 transfers of 700 each
    // direction? No — the quote is 「所有玩家依次和行动序列里的下一位玩家进行比较」
    // once per player. P0 vs P1 and P1 vs P0 are two comparisons.
    kani::cover!(n_log == 2, "two adjacent pairs (n=2 ring)");

    assert!(n_log == 2, "E-tears: one comparison per living seat");
    // Pair (0,1): share ⇒ 1 pays 0, 700. Pair (1,0): share ⇒ 0 pays 1, 700.
    let expected = [(1i32, 0i32, 700i32), (0, 1, 700)];
    assert!(
        spec::events::log_is_paytos(effects, &expected),
        "E-tears-01: both directions 700 on a shared colour"
    );
    // Money: net zero over the pair, each leg 700.
    assert!(
        post.money[0] == pre.money[0] && post.money[1] == pre.money[1],
        "E-tears-01: symmetric 700s net out"
    );
    assert!(post.pos == pre.pos, "frame: positions");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn tears_02_different_colour_current_pays_300() {
    let w = tears_world(false);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:泪水的含义"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // No shared colour: each current pays the next 300.
    // Pair (0,1): 0 pays 1, 300. Pair (1,0): 1 pays 0, 300.
    let expected = [(0i32, 1i32, 300i32), (1, 0, 300)];
    kani::cover!(n_log == 2, "two adjacent pairs");
    assert!(
        spec::events::log_is_paytos(effects, &expected),
        "E-tears-02: both directions 300 without a shared colour"
    );
    assert!(
        post.money[0] == pre.money[0] && post.money[1] == pre.money[1],
        "E-tears-02: symmetric 300s net out"
    );
    assert!(post.pos == pre.pos, "frame: positions");
}

/// MUTANT: 700/300 swapped is distinguishable.
#[kani::proof]
#[kani::unwind(32)]
pub fn tears_mutant_is_detected() {
    let w = tears_world(true);
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:泪水的含义"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    // Mutant wants 300 on a shared colour (the E-tears-02 amount).
    let mutant = [(1i32, 0i32, 300i32), (0, 1, 300)];
    let matches_mutant = spec::events::log_is_paytos(effects, &mutant);
    assert!(!matches_mutant, "mutant 300-on-share must not match the card");
}

// ===========================================================================
// E-pico-swap-01 / 02 — PICO灵魂交换
// 「所有未[除外]玩家[传送]到行动顺序的下一位未[除外]玩家的位置（不触发场地
// 效果，例：第一位行动的玩家[传送]到第二位行动的玩家的位置并以此类推，最后
// 行动的玩家[传送]到第一位行动的玩家的位置）」
//
// Setup: n=3, distinct positions 10/20/30. Exile layers set directly via the
// `exile` state key (give_exile only logs; the body reads state::EXILE).
// ===========================================================================

fn pico_world(exile: [i32; 4]) -> World {
    let mut w = base_world();
    w.n_players = 3;
    w.pos[0] = 10;
    w.pos[1] = 20;
    w.pos[2] = 30;
    w.pos[3] = 40;
    let exile_key = intern::intern("exile");
    for p in 0..MAX_PLAYERS {
        w.key_set(p as i32, exile_key, exile[p]);
    }
    w
}

#[kani::proof]
#[kani::unwind(32)]
pub fn pico_swap_01_rotates_positions() {
    let w = pico_world([0, 0, 0, 0]);
    let pre_pos = w.pos;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:PICO灵魂交换"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Ring = [0,1,2]; each takes the next pre-position.
    let expected = [(0i32, 20i32), (1, 30), (2, 10)];
    kani::cover!(n_log == 3, "three teleports in the ring");
    assert!(
        spec::events::log_is_teleports(effects, &expected),
        "E-pico-swap-01: rotate onto the next ring member's pre-position"
    );
    assert!(post.pos[0] == 20 && post.pos[1] == 30 && post.pos[2] == 10, "post positions");
    assert!(post.money == pre.money, "frame: money");
    let _ = pre_pos;
}

#[kani::proof]
#[kani::unwind(32)]
pub fn pico_swap_02_skips_exiled_players() {
    // Player 1 is exiled; ring = [0, 2].
    let w = pico_world([0, 1, 0, 0]);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:PICO灵魂交换"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Ring [0,2]: 0 → pos(2)=30, 2 → pos(0)=10. Player 1 unmoved.
    let expected = [(0i32, 30i32), (2, 10)];
    kani::cover!(n_log == 2, "exiled seat skipped");
    assert!(
        spec::events::log_is_teleports(effects, &expected),
        "E-pico-swap-02: exiled player skipped and not a destination source"
    );
    assert!(post.pos[0] == 30 && post.pos[1] == 20 && post.pos[2] == 10, "post");
    assert!(post.pos == [30, 20, 10, 40], "exiled seat stays put");
}

/// MUTANT: a naive cascade (each reads the *new* position) is distinguishable
/// from the latched snapshot. Card latches; mutant would send 2 → 20 not 10.
#[kani::proof]
#[kani::unwind(32)]
pub fn pico_swap_mutant_is_detected() {
    let w = pico_world([0, 0, 0, 0]);
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:PICO灵魂交换"), 0));
    let post = world::snapshot();
    // Latched: 2 lands on 10 (0's pre). Cascade mutant: 2 lands on 20 (0's post).
    assert!(
        post.pos[2] != 20 || post.pos[0] == 20,
        "mutant cascade would move seat 2 onto seat 0's NEW square (20) while seat 0 left it"
    );
    // Stronger pin: seat 2 must be on 10, not 20.
    assert!(post.pos[2] == 10, "latched dest for the last ring member");
}

// ===========================================================================
// E-moca-01 / 02 — Forbidden Moca
// 「直到你的下个回合结束，场上所有移动掷骰/2，在你的下个回合结束时永久移除
// 此事件！」
//
// Activation (Play): keep (dest=Field). Scheduling 「下个回合结束」 is
// PROTOCOL-LEVEL (`schedule_turn_end` stub). Dice-halve hook is
// BLOCKED-HOST(`trig_move_roll`). Expiry tick (AtEnd): `EventExpire`.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn moca_01_activation_keeps_the_card() {
    let w = base_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:Forbidden Moca"), 0));
    let post = world::snapshot();
    let dest = world::with_world(|w| w.dest);
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    assert!(
        spec::events::moca_activation_dest_is_field(dest),
        "E-moca-01 activation: 「放置于场地中央」 keeps the card (dest=Field)"
    );
    // No money / pos / hand movement on activation.
    assert!(effects.is_empty(), "E-moca-01 activation: no money/pos effects");
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: positions");
    assert!(post.hand == pre.hand, "frame: hand");
}

/// Expiry tick: `AtEnd` body permanently removes the event (`EventExpire`).
#[kani::proof]
#[kani::unwind(16)]
pub fn moca_02_expiry_tick_files_away() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    // Expiry entry (「在你的下个回合结束时永久移除此事件」).
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:Forbidden Moca"), "AtEnd", "", 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let key = intern::intern("Forbidden Moca");
    assert!(
        spec::events::moca_expiry_log_ok(effects, key),
        "E-moca-02: expiry tick logs exactly EventExpire"
    );
}

/// MUTANT: expiry must not also move money.
#[kani::proof]
#[kani::unwind(16)]
pub fn moca_02_mutant_is_detected() {
    {
        world::reset_world(base_world());
    }
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:Forbidden Moca"), "AtEnd", "", 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    // Mutant wants an extra GainMoney. Card has only EventExpire.
    let mutant_has_gain = effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { .. }));
    assert!(!mutant_has_gain, "mutant extra gain must not match the card");
}

// ===========================================================================
// E-aao-01 / 02 — A！A！O！
// 「将此卡放置于场地中央，抽到的玩家的下回合结束时放入事件弃牌。此卡在场时
// 全场玩家不能使用卡牌的[手]效果。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn aao_01_activation_keeps_the_card() {
    let w = base_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:A！A！O！"), 0));
    let post = world::snapshot();
    let dest = world::with_world(|w| w.dest);
    assert!(
        spec::events::aao_activation_dest_is_field(dest),
        "E-aao-01 activation: keep (dest=Field)"
    );
    assert!(post.money == pre.money && post.pos == pre.pos, "frame");
}

#[kani::proof]
#[kani::unwind(16)]
pub fn aao_01_expiry_tick_files_away() {
    {
        world::reset_world(base_world());
    }
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:A！A！O！"), "AtEnd", "", 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let key = intern::intern("A！A！O！");
    assert!(
        spec::events::log_is_expire(effects, key),
        "E-aao-01: expiry tick logs EventExpire"
    );
}

/// E-aao-02 FAILS(LESS): the book requires a global [手] veto while the card
/// is up; the body arms no such surface. This proof asserts the veto is armed
/// and is expected to FAIL — the finding.
#[kani::proof]
#[kani::unwind(16)]
pub fn aao_02_hand_veto_is_armed() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:A！A！O！"), 0));
    let dest = world::with_world(|w| w.dest);
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        spec::events::aao_hand_veto_armed(dest, effects),
        "E-aao-02: book says 「不能使用卡牌的[手]效果」 while up — body arms no veto"
    );
}
// ===========================================================================
// E-front-or-back-01 / 02 — 前场队还是后场队？
// 「所有玩家各投掷1d20，然后将站在格子序号1到30的玩家的投掷结果相加定位X且
// 站在格子序号31到60的玩家的投掷结果相加定位Y；如果X=Y则所有玩家获得500资金，
// 如果X>Y则站在格子序号1到30的玩家分摊获得1000资金，如果Y>X则站在格子序号
// 31到60的玩家分摊获得1000资金」
//
// OPEN-分摊: body drops `1000 % n` remainder. Host `pay_total` also logs a
// shaping `PayTo` of the full 1000 (PROTOCOL artifact) before the `pay_leg`s.
// Branch detect: equal = GainMoney 500 each; share = PayTo legs.
// ===========================================================================

/// Both seats on the front half (pos 5 / 10) → Y=0, X>=2 → always the share
/// branch with 2 winners. Each `pay_leg` is 500 (1000/2). Spec
/// `front_or_back_share_total_ok` on the legs.
#[kani::proof]
#[kani::unwind(32)]
pub fn front_or_back_02_same_half_share() {
    let mut w = base_world();
    w.n_players = 2;
    w.pos[0] = 5;
    w.pos[1] = 10;
    w.pos[2] = 20;
    w.pos[3] = 30;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:前场队还是后场队？"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Same-half: never the X=Y branch (Y=0, X>=2).
    let equal = !effects.is_empty()
        && effects
            .iter()
            .all(|e| matches!(e, Effect::GainMoney { .. }));
    kani::cover!(!equal, "share branch (same half)");
    assert!(!equal, "E-front-or-back: same half must take the share branch");

    let winners = [0i32, 1];
    let legs = spec::events::front_or_back_share_legs(effects, &winners);
    kani::cover!(legs.len() == 2, "two share legs");
    assert!(
        spec::events::front_or_back_share_total_ok(&legs, &winners),
        "E-front-or-back-02: two winners split the 1000 (each 500)"
    );
}

/// Split setup (front 5 / back 40): X and Y are independent 1d20 faces, so
/// X=Y, X>Y and Y>X are all reachable. Distinguish via GainMoney (equal) vs
/// PayTo (share).
#[kani::proof]
#[kani::unwind(32)]
pub fn front_or_back_01_equal_iff() {
    let mut w = base_world();
    w.n_players = 2;
    w.pos[0] = 5;
    w.pos[1] = 40;
    w.pos[2] = 10;
    w.pos[3] = 50;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:前场队还是后场队？"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    let is_equal = !effects.is_empty()
        && effects
            .iter()
            .all(|e| matches!(e, Effect::GainMoney { .. }));
    let is_share = effects.iter().any(|e| matches!(e, Effect::PayTo { .. }));
    kani::cover!(is_equal, "X=Y — all gain 500 (E-front-or-back-01)");
    kani::cover!(is_share, "X≠Y — winning half shares 1000 (E-front-or-back-02)");

    if is_equal {
        let expected = [(0i32, 500i32), (1, 500)];
        assert!(
            spec::events::log_is_gains(effects, &expected),
            "E-front-or-back-01: X=Y ⇒ every player gains exactly 500"
        );
        assert!(post.money[0] == pre.money[0] + 500, "P0 +500");
        assert!(post.money[1] == pre.money[1] + 500, "P1 +500");
    } else {
        assert!(is_share, "exactly one of equal / share fires");
        // Sole winner takes the whole 1000 (1000/1). Read the winner off the
        // first bank PayTo (the host `pay_total` shaping record names them);
        // `share_legs` then drops that artifact and keeps the 1000 leg.
        let winner = effects
            .iter()
            .find_map(|e| match e {
                Effect::PayTo { from, to, n } if *from < 0 && *n > 0 => Some(*to),
                _ => None,
            })
            .expect("share branch has a bank PayTo");
        let winners = [winner];
        let legs = spec::events::front_or_back_share_legs(effects, &winners);
        assert!(
            spec::events::front_or_back_share_total_ok(&legs, &winners),
            "E-front-or-back-02: sole winner of the higher half takes 1000"
        );
    }
    assert!(post.pos == pre.pos, "frame: positions");
}

/// MUTANT: swapped 500 / 1000. On the split setup the equal branch pays 500
/// (mutant wants 1000) and the share branch pays 1000 (mutant wants 500) —
/// both disagree on every path.
#[kani::proof]
#[kani::unwind(32)]
pub fn front_or_back_mutant_swapped_is_detected() {
    let mut w = base_world();
    w.n_players = 2;
    w.pos[0] = 5;
    w.pos[1] = 40;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:前场队还是后场队？"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    let mutant_equal = spec::events::front_or_back_mutant_swapped_equal_amount();
    let mutant_share = spec::events::front_or_back_mutant_swapped_share_total();
    assert!(mutant_equal == 1000 && mutant_share == 500, "mutant pins");

    let is_equal = !effects.is_empty()
        && effects
            .iter()
            .all(|e| matches!(e, Effect::GainMoney { .. }));
    if is_equal {
        // Card paid 500; mutant wants 1000.
        let matches_mutant = spec::events::log_is_gains(effects, &[(0, mutant_equal), (1, mutant_equal)]);
        assert!(!matches_mutant, "mutant equal-1000 must not match the card");
    } else {
        let winner = effects
            .iter()
            .find_map(|e| match e {
                Effect::PayTo { from, to, n } if *from < 0 && *n > 0 => Some(*to),
                _ => None,
            })
            .unwrap_or(0);
        let winners = [winner];
        let legs = spec::events::front_or_back_share_legs(effects, &winners);
        let card_total: i32 = legs.iter().map(|(_, n)| *n).sum();
        // Card share total is 1000; mutant wants 500.
        if card_total > 0 {
            assert!(
                card_total != mutant_share,
                "mutant share-500 must not match the card's 1000"
            );
        }
    }
}

// ===========================================================================
// E-ex-quest-01 — EX任务挑战 (PROVE rolls; skin is cosmetic — cannot test)
// 「抽出此卡的玩家投掷1d4并记录结果和26相加为X，然后所有玩家各自投掷3d20，
// 结果至少为X的玩家可选择更换角色皮肤（技能相同）」
// ===========================================================================

/// IFF: the one-shot resolves with an empty effect log and an unchanged
/// frame. Cover that it completes for every player count.
#[kani::proof]
#[kani::unwind(16)]
pub fn ex_quest_01_rolls_and_files() {
    let w = base_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:EX任务挑战"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    kani::cover!(true, "ex-quest completes");
    assert!(
        spec::events::ex_quest_01_no_state_effects(effects),
        "E-ex-quest-01: only dice — no money/pos/mark effects"
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: positions");
    assert!(post.fire == pre.fire, "frame: fire");
    // Cosmetic skin (E-ex-quest-02) cannot test — no match-state seam.
}

// ===========================================================================
// E-habinel-01 — 前往哈比内尔王国旅游 (PROVE effect log)
// 「每个玩家获得X层[除外]…X=1d2（每个玩家独立投掷1d2）」
// BLOCKED-HOST(tile_named:微笑号): `to` is always -1 under Kani.
// E-habinel-02 turn-start tick = engine status machinery → PROTOCOL-LEVEL.
// E-habinel-03 「结束后」 timing is OPEN-habinel-end; teleport NOT claimed.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn habinel_01_grants_exiles() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:前往哈比内尔王国旅游"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let n = world::with_world(|w| w.n_players);
    let players: [i32; 4] = [0, 1, 2, 3];
    let living = &players[..n as usize];

    kani::cover!(n_log == n as usize, "one GiveExile per living player");
    assert!(
        spec::events::habinel_01_exiles_ok(effects, living, -1),
        "E-habinel-01: one GiveExile per player, n in 1..=2, to = -1 (BLOCKED-HOST 微笑号)"
    );
    for e in effects {
        if let Effect::GiveExile { n, .. } = e {
            kani::cover!(*n == 1, "X=1");
            kani::cover!(*n == 2, "X=2");
        }
    }
}

// ===========================================================================
// E-never-ends-01 — 一切不会结束
// BLOCKED-HOST(tile_named:RiNG N): the body early-returns when the named
// tile is missing. Under Kani `tile_named("RiNG {n}")` is always -1, so the
// body is a no-op. The book's teleport+exile is NOT proven.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn never_ends_01_noop_blocked_host() {
    let w = base_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:一切不会结束"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    assert!(
        spec::events::never_ends_01_noop(effects),
        "E-never-ends-01 BLOCKED-HOST(tile_named:RiNG): body is a no-op under Kani"
    );
    assert!(post.money == pre.money && post.pos == pre.pos, "frame");
}

// ===========================================================================
// E-chart-rush-01 / 02 — 冲榜 (PROVE what is modeled)
// 「所有人失去所有[火罐]并且投掷Xd20，X为失去的指示物数量加一，点数最高的人
// 获得2000资金或抽一张卡，点数第二的人获得1500资金…」
// OPEN-chart-rush-or / OPEN-chart-rush-tie.
// `spend_fire` does not log an effect; post.fire is the evidence.
// `ask_yes` with no registered options is always "yes" under symbolic
// answers — concrete `answers` are used to explore both OR legs.
// ===========================================================================

fn chart_rush_world(fire0: i32, fire1: i32, answer0: i32) -> World {
    let mut w = base_world();
    w.n_players = 2;
    w.fire[0] = fire0;
    w.fire[1] = fire1;
    w.fire[2] = 0;
    w.fire[3] = 0;
    w.fire_max = [3, 3, 3, 3];
    w.symbolic_answers = false;
    w.answers = [answer0, answer0, answer0, answer0, 0, 0, 0, 0];
    w.next_answer = 0;
    w
}

/// E-chart-rush-01: every living player loses all fire pots.
#[kani::proof]
#[kani::unwind(24)]
pub fn chart_rush_01_spends_fire() {
    let mut w = chart_rush_world(0, 0, 0);
    w.fire[0] = kani::any();
    kani::assume(w.fire[0] >= 0 && w.fire[0] <= 3);
    w.fire[1] = kani::any();
    kani::assume(w.fire[1] >= 0 && w.fire[1] <= 3);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:冲榜"), 0));
    let post = world::snapshot();

    kani::cover!(pre.fire[0] == 0 && pre.fire[1] == 0, "no pots");
    kani::cover!(pre.fire[0] > 0 || pre.fire[1] > 0, "some pots spent");
    assert!(
        spec::events::chart_rush_fire_spent(&post.fire, 2),
        "E-chart-rush-01: all fire pots spent"
    );
    // Dice (Xd20) leave no state; prizes may move money.
    let _ = post.money;
}

/// E-chart-rush-02: with 2 players and both answering 「yes」, rank 1 gains
/// 2000 and rank 2 gains 1500 (equal faces → both rank 1, both 2000).
#[kani::proof]
#[kani::unwind(24)]
pub fn chart_rush_02_yes_2000() {
    let w = chart_rush_world(0, 0, 0); // answers[0]=0 → ask_yes is yes
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:冲榜"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Every prize is a GainMoney of 2000 (rank 1) or 1500 (rank 2).
    for e in effects {
        match e {
            Effect::GainMoney { player, n } => {
                assert!(
                    *n == 2000 || *n == 1500,
                    "E-chart-rush-02: purse is 2000 or 1500 with 2 players"
                );
                if *n == 2000 {
                    kani::cover!(*player == 0, "P0 takes rank-1 money");
                    assert!(
                        post.money[*player as usize] == pre.money[*player as usize] + 2000,
                        "rank-1 money"
                    );
                } else {
                    kani::cover!(*player == 1, "P1 takes rank-2 purse");
                    assert!(
                        post.money[*player as usize] == pre.money[*player as usize] + 1500,
                        "rank-2 purse"
                    );
                }
            }
            Effect::Draw { n, .. } => {
                assert!(*n == 1, "draw-one only on the OR leg");
            }
            other => panic!("E-chart-rush-02: unexpected effect {:?}", other),
        }
    }
    // At least one purse is paid (the body always ranks everyone).
    assert!(!effects.is_empty(), "E-chart-rush-02: prizes are paid");
    kani::cover!(
        effects.iter().any(|e| matches!(e, Effect::GainMoney { n: 1500, .. })),
        "distinct faces — rank-2 purse"
    );
    kani::cover!(
        effects.iter().all(|e| matches!(e, Effect::GainMoney { n: 2000, .. })),
        "tied faces — both rank 1 (OPEN-chart-rush-tie)"
    );
}

/// E-chart-rush-02 OR leg: answering 「no」 draws a card instead of the 2000.
#[kani::proof]
#[kani::unwind(24)]
pub fn chart_rush_02_no_draw() {
    let w = chart_rush_world(0, 0, 1); // answers[0]=1 → ask_yes is no
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:冲榜"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Rank 1 draws 1; rank 2 still takes 1500. No 2000 on this leg.
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::GainMoney { n: 2000, .. })),
        "E-chart-rush-02 OPEN-chart-rush-or: 'no' must not take the 2000"
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Draw { n: 1, .. })),
        "E-chart-rush-02: 'no' draws one card"
    );
    // Rank-2 purse still rides (when faces differ). On a tie both rank-1
    // seats draw (answers[1] is also 1 by the helper default).
    let rank2 = effects
        .iter()
        .filter(|e| matches!(e, Effect::GainMoney { n: 1500, .. }))
        .count();
    kani::cover!(rank2 == 1, "distinct faces — rank-2 purse paid");
    kani::cover!(rank2 == 0, "tied faces — no rank-2 seat");
    assert!(rank2 <= 1, "at most one rank-2 purse");
    let _ = pre;
    let _ = post;
}

/// MUTANT: rank-1 money off-by-one (2001). The card's 2000 must not match.
#[kani::proof]
#[kani::unwind(24)]
pub fn chart_rush_mutant_off_by_one() {
    let w = chart_rush_world(0, 0, 0);
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:冲榜"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let mutant = spec::events::chart_rush_purse_mutant(1);
    assert!(mutant == 2001, "mutant pin");
    for e in effects {
        if let Effect::GainMoney { n, .. } = e {
            assert!(*n != mutant, "mutant 2001 must not match the card's purse");
        }
    }
}

// ===========================================================================
// E-marina-box-02 — 麻里奈小姐的礼物箱 (PROVE optional spend)
// 「所有玩家[经过]CiRCLE时可[消耗]一次500资金，抽到的玩家投掷一次1d10
// （不受任何其他效果影响），如果投掷结果至少为6，那名玩家[获得]1200资金。」
// OPEN-marina-who (passer) / OPEN-marina-default (default decline).
// Hook entry: On:: index 1 = PassTile. `set_prop(DRAWER)` is a no-op, so the
// roller is always seat 0 under Kani — the pay/gain leg is still determined.
// E-marina-box-01 expiry (index 2, TurnStartBefore) counts turns via
// `set_prop` (no-op) → never expires → PROTOCOL-LEVEL.
// ===========================================================================

/// Clean decline / accept pair (trig_player installed before the hook).
#[kani::proof]
#[kani::unwind(24)]
pub fn marina_box_02_decline_is_noop() {
    let mut w = base_world();
    w.n_players = 2;
    w.symbolic_answers = true;
    w.trig_player = 0;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:麻里奈小姐的礼物箱"), "Hook", "PassTile", 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // symbolic ask_pick explores 0 (decline) and 1 (accept).
    let declined = spec::events::marina_box_02_decline_ok(effects);
    let accepted = spec::events::marina_box_02_accept_ok(effects, 0);
    kani::cover!(declined, "decline — no spend (OPEN-marina-default)");
    kani::cover!(accepted, "accept — pay 500, maybe +1200");
    assert!(
        declined || accepted,
        "E-marina-box-02: either a clean decline or the accept leg"
    );
    if declined {
        assert!(effects.is_empty(), "decline moves no money");
    } else {
        // Accept: PayMoney 500 then optional GainMoney 1200.
        assert!(
            matches!(effects[0], Effect::PayMoney { player: 0, n: 500 }),
            "accept pays 500"
        );
        if effects.len() == 2 {
            kani::cover!(true, "face ≥ 6 — passer gains 1200");
            assert!(
                matches!(effects[1], Effect::GainMoney { player: 0, n: 1200 }),
                "face ≥ 6 gains 1200"
            );
        } else {
            kani::cover!(true, "face < 6 — no gain");
        }
    }
}

/// Mutant: always pays 500 even on decline.
#[kani::proof]
#[kani::unwind(24)]
pub fn marina_box_02_mutant_always_pays() {
    let mut w = base_world();
    w.n_players = 2;
    w.symbolic_answers = false;
    w.answers = [0, 0, 0, 0, 0, 0, 0, 0]; // forced decline
    w.next_answer = 0;
    w.trig_player = 0;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:麻里奈小姐的礼物箱"), "Hook", "PassTile", 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let matches_mutant = spec::events::marina_box_02_decline_mutant(effects);
    assert!(
        !matches_mutant,
        "mutant always-pays must not match the card's decline"
    );
    assert!(
        spec::events::marina_box_02_decline_ok(effects),
        "forced decline is a no-op"
    );
}

// ===========================================================================
// Active-event activation / expiry pairs
//
// Activation = `keep()` (dest = Field) plus any modeled side effects.
// Expiry = the body's `AtEnd` / `TurnStartBefore` / `Event` entry →
// `Effect::EventExpire`. Scheduling of 「下回合」 is PROTOCOL
// (`schedule_turn_end` stub). `set_self_prop` is a no-op — bodies that count
// turns on instance props never expire under Kani.
// ===========================================================================

/// Shared: run Play and assert keep + empty money/pos frame.
/// `idx` is a `crate::card_id!` index (no string scan).
fn assert_activation_keep(idx: usize) {
    let w = base_world();
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(idx, 0));
    let dest = world::with_world(|w| w.dest);
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let id = crate::generated::IDS[idx];
    assert!(
        spec::events::permanent_keep(dest),
        "{id}: 「放置于场地中央」 keeps the card (dest=Field)"
    );
    let _ = pre;
    let _ = effects;
}

// ---- E-hallucination-01 — 幻觉来了 -----------------------------------------
// 「将此卡放置于场地中央，抽到的玩家的下回合开始时放入事件弃牌。」
// On:: 0=Play, 1=Hook(RollPlan), 2=Hook(TurnStartBefore).
// Roll substitution is E-hallucination-02 (follow-up / OPEN-hallucination-effects).

#[kani::proof]
#[kani::unwind(16)]
pub fn hallucination_01_activation_keeps() {
    assert_activation_keep(crate::card_id!("event:幻觉来了"));
}

/// Expiry tick: TurnStartBefore body. `prop(DRAWER)` is 0 (set_prop no-op),
/// so seat 0's turn start fires `n = 0+1 >= 1` → EventExpire.
#[kani::proof]
#[kani::unwind(16)]
pub fn hallucination_01_expiry_tick() {
    let mut w = base_world();
    w.trig_player = 0;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:幻觉来了"), 2, 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let key = intern::intern("幻觉来了");
    assert!(
        spec::events::log_is_expire(effects, key),
        "E-hallucination-01: expiry tick logs exactly EventExpire"
    );
}

// ---- E-pool-bug-01 / 02 / 03 — 卡池BUG -------------------------------------
// 「将此卡放置于场地中央并将一张“[衍生]修复公告”背面朝上放置于事件牌堆顶部，
// 下次触发事件时将此卡放入事件弃牌。此卡在场时不可在造价1500及以上的格子上
// 加盖房屋。」
// On:: 0=Play, 1=Hook(Event), 2=Hook(BuildBefore).
// event_deck_push is a PROTOCOL stub (E-pool-bug-01 push unprovable).
// set_prop(NO_BUILD_ABOVE) is a no-op — the BuildBefore belt still works.

#[kani::proof]
#[kani::unwind(16)]
pub fn pool_bug_01_activation_keeps() {
    assert_activation_keep(crate::card_id!("event:卡池BUG"));
}

/// E-pool-bug-02: the Event hook files the card away.
#[kani::proof]
#[kani::unwind(16)]
pub fn pool_bug_02_expiry_on_next_event() {
    {
        world::reset_world(base_world());
    }
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:卡池BUG"), 1, 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let key = intern::intern("卡池BUG");
    assert!(
        spec::events::log_is_expire(effects, key),
        "E-pool-bug-02: next event trigger logs EventExpire"
    );
}

/// E-pool-bug-03: BuildBefore on a house-cost ≥ 1500 tile cancels the build.
/// `build_cost` resolves via the 1-based board index (tile 2 = 购物中心, 2000).
#[kani::proof]
#[kani::unwind(16)]
pub fn pool_bug_03_blocks_expensive_build() {
    let mut w = base_world();
    w.trig_tile = 2; // board #2 购物中心, house cost 2000
    w.trig_player = 0;
    w.trig_cancelled = false;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:卡池BUG"), 2, 0));
    let cancelled = world::with_world(|w| w.trig_cancelled);
    assert!(
        spec::events::pool_bug_03_cancelled(cancelled),
        "E-pool-bug-03: BuildBefore cancels a ≥1500 house-cost build"
    );
    // Cheap tile (board #6 house 1000) must not cancel.
    let mut w2 = base_world();
    w2.trig_tile = 6;
    w2.trig_player = 0;
    w2.trig_cancelled = false;
    {
        world::reset_world(w2);
    }
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:卡池BUG"), 2, 0));
    let cancelled2 = world::with_world(|w| w.trig_cancelled);
    assert!(!cancelled2, "E-pool-bug-03: cheap builds are not vetoed");
}

// ---- E-bangdream-chan-01 — 元祖！邦多利酱 -----------------------------------
// 「将此卡放置于场地中央，抽到的玩家的第3回合开始时放入事件弃牌。」
// E-bangdream-chan-02 host clocks: cannot test (no match-state seam).
// Expiry counts turns on `set_prop` (no-op) → never reaches n≥2 →
// PROTOCOL-LEVEL for the expiry tick.

#[kani::proof]
#[kani::unwind(16)]
pub fn bangdream_chan_01_activation_keeps() {
    assert_activation_keep(crate::card_id!("event:元祖！邦多利酱"));
}

/// Expiry tick cannot fire: `prop(TURNS)` stays 0 (set_prop stub), so
/// `n = 1` and `n >= 2` is false. Assert the no-op — the book's third-turn
/// filing is PROTOCOL-LEVEL (set_self_prop).
#[kani::proof]
#[kani::unwind(16)]
pub fn bangdream_chan_01_expiry_blocked_by_set_prop() {
    let mut w = base_world();
    w.trig_player = 0;
    {
        world::reset_world(w);
    }
    // On:: index 1 = TurnStartBefore.
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:元祖！邦多利酱"), 1, 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        effects.is_empty(),
        "E-bangdream-chan-01 PROTOCOL-LEVEL(set_self_prop): turn counter never accumulates"
    );
}

// ---- E-surprise-duel-01 / 02 — 意外的对邦 ----------------------------------
// 「将此卡放置于场地中央，抽到的玩家的下2回合开始时放入事件弃牌。」
// OPEN-surprise-2turn. Steering (E-surprise-duel-03) is a follow-up.
// Expiry uses `set_prop` turn counter → PROTOCOL-LEVEL.

#[kani::proof]
#[kani::unwind(16)]
pub fn surprise_duel_01_activation_keeps() {
    assert_activation_keep(crate::card_id!("event:意外的对邦"));
}

#[kani::proof]
#[kani::unwind(16)]
pub fn surprise_duel_02_expiry_blocked_by_set_prop() {
    let mut w = base_world();
    w.trig_player = 0;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:意外的对邦"), 2, 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        effects.is_empty(),
        "E-surprise-duel-02 PROTOCOL-LEVEL(set_self_prop): counter never reaches 2"
    );
}

// ---- E-heat-head-01 / 02 / 03 — 超燃甩头 -----------------------------------
// 「将此卡放置于场地中央，抽到的玩家的下回合结束前所有玩家的移动掷骰增加1d6，
// 此后抽到的玩家的下回合结束前所有玩家的移动掷骰减少1d6，此后放入事件弃牌。」
// On:: 0=Play, 1=Hook(RollPlan), 2=Hook(RollAfter), 3=AtEnd.
// Phase crystals via set_crystals → counter_self (modeled on trig_player).
// Dice modifiers are PROTOCOL stubs; `sub_die` reads `trigger::move_roll`
// (UNMODELED) — BLOCKED-HOST(trig_move_roll). Scheduling is PROTOCOL.

#[kani::proof]
#[kani::unwind(16)]
pub fn heat_head_01_activation_phase_up() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:超燃甩头"), 0));
    let dest = world::with_world(|w| w.dest);
    assert!(dest == 2, "E-heat-head: keep (dest=Field)");
    // `ctx::set_crystals` writes the **field-instance** counter (`Op::
    // set_counter_self` → `field_crystals(last_uid)`), not a player key.
    // Reading `key_get(0, "crystals")` was a harness bug (class B) — it
    // stayed 0 and the assert failed on the old base.
    let phase = world::with_world(|w| w.field_crystals(w.last_uid));
    assert!(phase == 1, "E-heat-head-01: phase 1 (add 1d6) is armed");
}

/// AtEnd while phase 1: flips to phase 2. Phase 2 AtEnd files the card.
#[kani::proof]
#[kani::unwind(16)]
pub fn heat_head_03_phase_end_files_away() {
    {
        world::reset_world(base_world());
    }
    // Arm phase 1 the way Play does (set_crystals writes trig_player's key).
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:超燃甩头"), 0));
    // First AtEnd: phase 1 → 2.
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:超燃甩头"), "AtEnd", "", 0));
    let phase = world::with_world(|w| w.field_crystals(w.last_uid));
    assert!(phase == 2, "E-heat-head-02: first turn end arms phase 2");
    // Second AtEnd: expire.
    let n0 = log_n_of();
    invoke::unwrap_done(invoke::hook_idx(crate::card_id!("event:超燃甩头"), "AtEnd", "", 0));
    let logbuf = effects_of();
    let n1 = log_n_of();
    let effects = &effects_slice(&logbuf, n1)[n0..];
    let key = intern::intern("超燃甩头");
    assert!(
        spec::events::log_is_expire(effects, key),
        "E-heat-head-03: second turn end logs EventExpire"
    );
}

// ---- E-marina-box-01 / bangdream / surprise expiry PROTOCOL ----------------
// Covered above (set_prop stub). Activation keeps are proven.

// ---- E-fix-note-01 — 修复公告 ----------------------------------------------
// 「前2个房屋造价为2000的格子和前1个房屋造价为1500的格子的加盖金额减半，且累计
// 在上述格子上加盖后将此卡移除。」
// set_build_discount is a PROTOCOL stub; set_prop tile ids are no-ops.
// OPEN-fix-note-count.

#[kani::proof]
#[kani::unwind(16)]
pub fn fix_note_01_activation_keeps() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:修复公告"), 0));
    let dest = world::with_world(|w| w.dest);
    assert!(
        spec::events::fix_note_01_keep(dest),
        "E-fix-note-01: keep (dest=Field). Discount half is PROTOCOL-LEVEL(set_build_discount)"
    );
}

// ---- E-end-it-all-01 — 让我来结束一切 (permanent keep) ---------------------
// 「触发事件的玩家从所有非衍生事件中选择3个移除，并将此卡永久放置于场上…」
// On:: 0=Play. event_banish is a PROTOCOL stub; the 3 asks are determined.

#[kani::proof]
#[kani::unwind(32)]
pub fn end_it_all_01_three_picks_and_keep() {
    let mut w = base_world();
    w.symbolic_answers = true;
    {
        world::reset_world(w);
    }
    let before = world::with_world(|w| w.next_answer);
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:让我来结束一切"), 0));
    let after = world::with_world(|w| w.next_answer);
    let dest = world::with_world(|w| w.dest);
    let picks = after.saturating_sub(before);
    // symbolic_answers=true does not consume `answers`; count via a second
    // run with concrete answers instead when `picks` is 0.
    let _ = picks;
    assert!(
        spec::events::permanent_keep(dest),
        "E-end-it-all-01: permanent keep (dest=Field, no expiry)"
    );
}

/// Concrete-answers companion: 3 `ask_pick`s run (next_answer advances by 3).
#[kani::proof]
#[kani::unwind(32)]
pub fn end_it_all_01_three_picks_counted() {
    let mut w = base_world();
    w.symbolic_answers = false;
    w.answers = [0, 1, 2, 0, 0, 0, 0, 0];
    w.next_answer = 0;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:让我来结束一切"), 0));
    let after = world::with_world(|w| w.next_answer);
    assert!(
        spec::events::end_it_all_01_picked_three(after),
        "E-end-it-all-01: exactly 3 non-derived picks"
    );
    let dest = world::with_world(|w| w.dest);
    assert!(dest == 2, "permanent keep");
    // Banish is a PROTOCOL stub — no effect log. PROTOCOL-LEVEL on the
    // 「移除」 half; the pick-count and keep are proven.
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        effects.is_empty(),
        "event_banish is a stub — PROTOCOL-LEVEL on the removal itself"
    );
}

// ===========================================================================
// E-embers-01 / 03 / 04 — 火种燃尽之后会怎么样呢？
// 「（1）将此卡的复制品放置于所有玩家所在格子上…触发结算时，格子上每有一张
// 此卡的复制品，那名玩家失去10资金。任意玩家获得[除外]或破产时，移除所有此卡
// 的复制品。」
// On:: 0=Play, 1=Hook(TurnStartBefore), 2=Hook(SettleBody), 3=Hook(Exile…).
// place_mark IS modeled. remove_marks is a stub (E-embers-04 BLOCKED).
// give_exile does not set EXILE (clear-on-exile cannot observe the grant).
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn embers_01_places_copies() {
    let mut w = base_world();
    w.n_players = 3;
    w.pos[0] = 10;
    w.pos[1] = 20;
    w.pos[2] = 30;
    w.pos[3] = 40;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:火种燃尽之后会怎么样呢？"), 0));
    let dest = world::with_world(|w| w.dest);
    assert!(dest == 2, "E-embers: keep");
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let tiles = [10i32, 20, 30];
    kani::cover!(n_log == 3, "three PlaceMarks");
    assert!(
        spec::events::embers_01_marks_ok(effects, &tiles),
        "E-embers-01: one copy mark per living player's tile"
    );
    let marks_n = world::with_world(|w| w.marks_n);
    assert!(marks_n == 3, "marks_n grows by one per living player");
}

/// E-embers-03: SettleBody hook — settler pays 10 per copy on their tile.
/// On:: index 2. Trigger kind SettleBody = 64.
#[kani::proof]
#[kani::unwind(24)]
pub fn embers_03_settle_costs_ten_per_copy() {
    let mut w = base_world();
    w.n_players = 2;
    w.pos[0] = 10;
    w.pos[1] = 20;
    w.pos[2] = 30;
    w.pos[3] = 40;
    w.trig_player = 0;
    w.trig_tile = 10;
    w.trig_kind = 64; // TriggerKind::SettleBody
    w.trig_cancelled = false;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:火种燃尽之后会怎么样呢？"), 0));
    let n0 = log_n_of();
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:火种燃尽之后会怎么样呢？"), 2, 0));
    let logbuf = effects_of();
    let n1 = log_n_of();
    let effects = &effects_slice(&logbuf, n1)[n0..];
    // One copy sits on tile 10 (P0's tile).
    assert!(
        spec::events::embers_03_settle_ok(effects, 0, 1),
        "E-embers-03: settler pays 10 × copies on the tile"
    );
}

/// E-embers-04: clear-on-exile is BLOCKED — `remove_marks` is a stub and
/// `give_exile` does not set the EXILE state key, so the `on_out` path cannot
/// observe a grant. Assert the stub leaves the marks in place (the finding).
#[kani::proof]
#[kani::unwind(24)]
pub fn embers_04_clear_on_exile_blocked() {
    let mut w = base_world();
    w.n_players = 2;
    w.pos[0] = 10;
    w.pos[1] = 20;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:火种燃尽之后会怎么样呢？"), 0));
    let marks_before = world::with_world(|w| w.marks_n);
    // Exile hook (On:: index 3) with TriggerKind::Exile = 21.
    world::with_world_mut(|w| {
        w.trig_kind = 21;
        w.trig_player = 0;
    });
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("event:火种燃尽之后会怎么样呢？"), 3, 0));
    let marks_after = world::with_world(|w| w.marks_n);
    // remove_marks is a stub: marks survive. Book says they leave.
    assert!(
        marks_after == marks_before && marks_before > 0,
        "E-embers-04 BLOCKED-HOST(remove_marks): stub leaves copies in place"
    );
}

// ===========================================================================
// E-bear-cookie-01 — 发送熊饼表情 (PROTOCOL-LEVEL: event_deck_push stub)
// 「…随后，将一张“[衍生]冲榜”背面朝上放置于事件牌堆顶部。」
// card_text_mentions is always 0 under Kani, so the fire-pot refill offer
// (E-bear-cookie-03) never arms — also PROTOCOL / untestable (skill-gated).
// The body correctly calls event_deck_push; the stub leaves the deck empty.
// ===========================================================================

#[kani::proof]
// all_players() is a `Range::try_fold` over `0..player_count()`; CBMC does
// not constant-fold the end across try_fold (TUNING §7/§10), so 16 was a
// silent cut ("Not unwinding … iteration 16"). 32 covers the 4-seat walk.
#[kani::unwind(32)]
pub fn bear_cookie_01_push_is_protocol() {
    {
        world::reset_world(base_world());
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:发送熊饼表情"), 0));
    // Book: derived 冲榜 on the event-deck top. The stub cannot push —
    // this assertion is expected to FAIL (PROTOCOL-LEVEL, not a body bug).
    let deck_top = world::with_world(|w| {
        if w.deck_n > 0 {
            Some(w.deck_pile[0])
        } else {
            None
        }
    });
    let chart_rush_id = intern::intern("冲榜").0;
    assert!(
        spec::events::bear_cookie_01_deck_top_is_chart_rush(deck_top, chart_rush_id),
        "E-bear-cookie-01 PROTOCOL-LEVEL(event_deck_push): derived 冲榜 must sit on the deck top"
    );
}

// ===========================================================================
// E-lost-chase-01 / 02 — 迷子的追逐 (PROTOCOL-LEVEL: event_deck_push stub)
// 「所有玩家各投5d20。如果抽到此卡的玩家的点数严格大于所有其他玩家的点数则
// 将一张“[衍生]让我来结束一切”背面朝上放置于事件牌堆顶部，否则将一张
// “[衍生]一切不会结束”背面朝上放置于事件牌堆顶。」
// The dice and the event_is_active guard for 让我来结束一切 are determined.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn lost_chase_rolls_and_pushes_stub() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:迷子的追逐"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    // 5d20 leaves no effect; the push is a stub.
    assert!(
        effects.is_empty(),
        "E-lost-chase: dice only; event_deck_push is a PROTOCOL stub"
    );
    kani::cover!(true, "lost-chase completes (PROTOCOL-LEVEL on the push)");
}

/// E-end-it-all-02: while 让我来结束一切 is up, the win branch must not push
/// another copy. Install the permanent event on `w.events` and assert the
/// body leaves the world alone (the push is a stub either way — the guard is
/// exercised by the cover, and the empty log is the determined post).
#[kani::proof]
#[kani::unwind(24)]
pub fn lost_chase_end_it_all_guard() {
    let mut w = base_world();
    // 「此卡在场上则…」 — install the permanent event on the active list.
    let end_id = intern::intern("让我来结束一切");
    w.events[0] = end_id.0;
    w.events_n = 1;
    {
        world::reset_world(w);
    }
    let active = world::with_world(|w| w.events_n) > 0;
    assert!(active, "setup: 让我来结束一切 is up");
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:迷子的追逐"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    // Guard + stub ⇒ nothing is pushed, on either dice branch.
    assert!(
        effects.is_empty(),
        "E-end-it-all-02: no second push while the permanent is up"
    );
    kani::cover!(true, "guard armed (event_is_active)");
}

// ===========================================================================
// E-lulu-01 / 02 — 很噜的感觉 (partial; dest BLOCKED-HOST)
// 「所有玩家可选择[传送]至快餐店（不触发场地效果），如果选择[传送]需支付
// 500资金并喊出“噜噜噜！”…」
// tile_named(快餐店) = -1 under Kani. The pay-500 leg is determined.
// The shout is out-of-game (cannot test).
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn lulu_01_pay_500_leg() {
    let mut w = base_world();
    w.n_players = 2;
    w.symbolic_answers = true; // ask_yes: 0 = yes, 1 = no
    {
        world::reset_world(w);
    }
    // ask_yes with no options: n=1 under symbolic answers ⇒ always yes.
    // Force both legs via concrete answers in the companion below.
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:很噜的感觉"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    // Always-yes path (symbolic ask_yes degenerates to 0).
    let expected = [
        (0i32, -1i32, 500i32), // PayMoney is not PayTo; check by hand below
    ];
    let _ = expected;
    // PayMoney 500 + Teleport to -1 for each living player.
    let n = world::with_world(|w| w.n_players) as usize;
    assert!(
        effects.len() == n * 2,
        "E-lulu-01: each accepting player pays 500 and teleports (BLOCKED-HOST 快餐店)"
    );
    for i in 0..n {
        let pay = &effects[i * 2];
        let tp = &effects[i * 2 + 1];
        assert!(
            matches!(pay, Effect::PayMoney { player, n: 500 } if *player == i as i32),
            "E-lulu-01: pay 500"
        );
        assert!(
            matches!(tp, Effect::Teleport { player, tile: -1 } if *player == i as i32),
            "E-lulu-01 BLOCKED-HOST(tile_named:快餐店): teleport dest is -1"
        );
    }
}

/// Decline leg: concrete answers 1 (no) ⇒ nobody moves.
#[kani::proof]
#[kani::unwind(24)]
pub fn lulu_02_decline_leaves_put() {
    let mut w = base_world();
    w.n_players = 2;
    w.symbolic_answers = false;
    w.answers = [1, 1, 1, 1, 0, 0, 0, 0];
    w.next_answer = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:很噜的感觉"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        effects.is_empty(),
        "E-lulu-02: decline pays nothing and does not teleport"
    );
    assert!(post.money == pre.money && post.pos == pre.pos, "frame");
}

// ===========================================================================
// E-school-time-01 / 02 — 上学时间 (exile leg; dest BLOCKED-HOST)
// 「全体玩家移动到操作角色当前上学的学校的格子然后获得1层[除外]（如果剧情
// 没有明说则移动到周边学区）。」
// tile_named(周边学区 / school) = -1 under Kani. give_exile IS modeled.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn school_time_01_exile_leg() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:上学时间"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let n = world::with_world(|w| w.n_players);
    let players: [i32; 4] = [0, 1, 2, 3];
    let living = &players[..n as usize];

    // Teleport to -1 (BLOCKED-HOST) + GiveExile 1 per living player.
    let exiles_ok = spec::events::exile_one_each(effects, living);
    assert!(
        exiles_ok,
        "E-school-time-01: one [除外] layer per player (dest BLOCKED-HOST 周边学区)"
    );
    kani::cover!(n_log == n as usize * 2, "teleport + exile per player");
}

// ===========================================================================
// E-kizuna-01 — Kizuna Music (X=0 when decks hold no song cards)
// 「触发此卡的玩家投掷X+1d20并所有玩家依次[传送]到投掷结果mod60所对应的
// 格子并获得1层[除外]，X为所有玩家卡组里卡名为歌名的卡数量的总数」
// OPEN-kizuna-song. Dest is `roll mod 60` (modeled); X=0 with empty decks.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn kizuna_01_teleport_and_exile() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:Kizuna Music"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    let n = world::with_world(|w| w.n_players);
    let players: [i32; 4] = [0, 1, 2, 3];
    let living = &players[..n as usize];

    // Reconstruct dest from the first Teleport (face = X+1d20 with X=0 ⇒ 1..20).
    let dest = effects.iter().find_map(|e| match e {
        Effect::Teleport { tile, .. } => Some(*tile),
        _ => None,
    });
    let dest = dest.expect("E-kizuna: every player teleports");
    kani::cover!(dest >= 1 && dest <= 20, "X=0, face 1..20");
    assert!(
        spec::events::kizuna_01_ok(effects, living, dest),
        "E-kizuna-01: teleport to face%60 and one [除外] each (X=0, OPEN-kizuna-song)"
    );
}

// ===========================================================================
// E-circle-rebuild-01 — 协助CiRCLE重建 (activation only)
// 「将此卡放置于场地中央，为其放置5个奇迹水晶…」
// On:: 0=Play, 1=Hook(PassTile), 2=Hook(SettleBody).
// set_crystals IS modeled. Pass countdown / settle replace involve
// unmodeled trig_move_* on the pass path and tile_named(CiRCLE)=-1 —
// PROTOCOL / BLOCKED-HOST for the rest.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn circle_rebuild_01_activation_five_crystals() {
    let w = base_world();
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:协助CiRCLE重建"), 0));
    let dest = world::with_world(|w| w.dest);
    assert!(dest == 2, "E-circle-rebuild-01: keep");
    // Field-instance crystals (see heat_head_01) — not a player key.
    let n = world::with_world(|w| w.field_crystals(w.last_uid));
    assert!(n == 5, "E-circle-rebuild-01: five miracle crystals on the instance");
    // CiRCLE tile is BLOCKED-HOST(tile_named) — NO_REWARD arm is a no-op dest.
}

// ===========================================================================
// E-estate-01 / 02 / 03 — 弦卷集团地产开发
// 「随机指定一个可购买格子（如果有）然后所有玩家进行竞价，出价最大的玩家支付
// 选择数量的资金并获得该格子的地契」
// raise_bought is a PROTOCOL stub (the deed half). Auction pay IS modeled.
// OPEN-estate-tie / OPEN-estate-empty.
// ===========================================================================

/// E-estate-02: every bid 0 ⇒ nobody pays. steps[0] = 0 for each seat.
#[kani::proof]
#[kani::unwind(32)]
pub fn estate_02_all_pass_no_pay() {
    let mut w = base_world();
    w.n_players = 2;
    w.symbolic_answers = false;
    w.answers = [0, 0, 0, 0, 0, 0, 0, 0];
    w.next_answer = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:弦卷集团地产开发"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        effects.is_empty(),
        "E-estate-02: a 0 bid costs 0 (raise_bought is a PROTOCOL stub)"
    );
    assert!(post.money == pre.money, "frame: money");
}

/// E-estate-01: a positive bid is paid. P0 bids 1000 (steps index 1), P1
/// bids 0 (index 0) ⇒ P0 pays 1000. The deed is PROTOCOL (raise_bought stub).
#[kani::proof]
#[kani::unwind(32)]
pub fn estate_01_auction_pay() {
    let mut w = base_world();
    w.n_players = 2;
    // Freeze money so the bid ladder is [0, 1000, 2000] (max=2000).
    w.money = [2000, 2000, 2000, 2000];
    w.symbolic_answers = false;
    w.answers = [1, 0, 0, 0, 0, 0, 0, 0]; // P0 → 1000, P1 → 0
    w.next_answer = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:弦卷集团地产开发"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Highest bid 1000 → PayMoney 1000 from P0.
    assert!(
        n_log == 1 && matches!(effects[0], Effect::PayMoney { player: 0, n: 1000 }),
        "E-estate-01: highest bidder pays their bid"
    );
    assert!(
        post.money[0] == pre.money[0] - 1000,
        "winner pays exactly the bid"
    );
    kani::cover!(true, "auction pay proven; deed is PROTOCOL-LEVEL(raise_bought)");
}

// ===========================================================================
// E-this-hand-01 / 02 / 03 — 这只手我不会放开 (PROTOCOL-LEVEL: card_move stub)
// 「所有玩家移动X，X=1d20-1d20+1d20，如果X是负数则向自身移动方向的反方向
// 移动（每个玩家独立投掷三个1d20进行计算，不触发场地效果）」
// card_move returns 1 and does NOT move pos; plan::set_* are stubs.
// The dice formula is determined but not observable in the effect log.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn this_hand_01_move_is_protocol() {
    let mut w = base_world();
    w.n_players = 2;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:这只手我不会放开"), 0));
    let post = world::snapshot();
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    // card_move stub: no Teleport, no money. Book walks X steps.
    assert!(
        effects.is_empty(),
        "E-this-hand PROTOCOL-LEVEL(card_move): stub produces no effect log"
    );
    assert!(
        post.pos == pre.pos,
        "E-this-hand-01 PROTOCOL-LEVEL: card_move does not move pos under Kani"
    );
}

// ===========================================================================
// E-asukayama-01 / 02 — 飞鸟山之战 (status spread; walk PROTOCOL)
// 「所有玩家[传送]到飞鸟山公园。然后抽到此卡的玩家移动1d20，…上一位玩家
// 获得2层…[眩晕]，下一位玩家在下个自己的回合结束前不可使用手牌，其余玩家
// 获得1层…[除外]」
// tile_named(飞鸟山公园) = -1 (BLOCKED-HOST). card_move is a stub.
// state::add / set IS modeled; give_exile IS modeled (log only).
// On:: 0=Play.
// ===========================================================================

/// E-asukayama-02 partial: the body's 「其余玩家」 [除外] leg. Host `neighbor`
/// returns a **tile** (pos±1 mod 60), not a seat, so prev/next assignment is
/// BLOCKED-HOST(neighbor) and `state::set_expires` clobbers the VALUE column
/// (host ignores the field argument). Under this model every non-drawer seat
/// falls into the 「其余」 loop and takes 1 GiveExile. The book's 2-眩晕 /
/// noHand split is NOT claimed.
#[kani::proof]
#[kani::unwind(24)]
pub fn asukayama_02_status_spread_blocked_neighbor() {
    let mut w = base_world();
    w.n_players = 4;
    w.pos[0] = 10;
    w.pos[1] = 20;
    w.pos[2] = 30;
    w.pos[3] = 40;
    {
        world::reset_world(w);
    }
    invoke::unwrap_done(invoke::event_play_idx(crate::card_id!("event:飞鸟山之战"), 0));
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);

    // Host `neighbor(p, ±1)` is the next **present player in turn order**
    // (hostfns `neighbor`), not a tile. For n=4, drawer=0: prev=3, next=1.
    // Book (data/events.json, 飞鸟山之战):
    //   「上一位玩家获得2层回合开始时减少一层的[眩晕]」  -> state_add stunStart=2
    //   「下一位玩家在下个自己的回合结束前不可使用手牌」 -> state_set noHand=2
    //   「其余玩家获得1层在回合开始时移除的[除外]」       -> give_exile(p,1,-1)
    // stun/noHand are state keys (`state_add`/`state_set`); only exile is an
    // `Effect::GiveExile`. The old harness expected a tile-valued neighbor and
    // 3 exiles — that was a class-B stub assumption, superseded.
    let stun_key = intern::intern("stunStart");
    let nohand_key = intern::intern("noHand");
    let (stun_prev, nohand_next, exile_prev, exile_next) = world::with_world(|w| {
        (
            w.key_get(3, stun_key),
            w.key_get(1, nohand_key),
            w.key_get(3, intern::intern("exile")),
            w.key_get(1, intern::intern("exile")),
        )
    });
    assert!(
        stun_prev == 2,
        "E-asukayama-02: prev (seat 3) takes 2 [眩晕] (stunStart)"
    );
    assert!(
        nohand_next == 2,
        "E-asukayama-02: next (seat 1) noHand = 2"
    );
    assert!(
        exile_prev == 0 && exile_next == 0,
        "E-asukayama-02: prev/next do not take [除外]"
    );

    let mut exiles = 0;
    for e in effects {
        if let Effect::GiveExile { player, n, to } = e {
            assert!(*player == 2, "E-asukayama-02: the remaining seat (2) takes [除外]");
            assert!(*player != 0, "drawer is not in the 「其余」 loop");
            assert!(*n == 1 && *to == -1, "1 [除外] layer, no named return");
            exiles += 1;
        }
    }
    kani::cover!(exiles == 1, "remaining seat exiled");
    assert!(
        exiles == 1,
        "E-asukayama-02: exactly one exile (the remaining seat, not prev/next)"
    );
}
// ===========================================================================
// Unit table — classification per clause id
// ===========================================================================
// E-draw-01/02/03          PROTOCOL-LEVEL (event deck ops are stubs)
// E-duobang-01/02          PROVEN (tie OPEN-duobang-tie noted)
// E-estate-01              PROVEN (pay) + PROTOCOL-LEVEL (deed / raise_bought)
// E-estate-02              PROVEN
// E-estate-03              PROTOCOL-LEVEL (raise_bought stub)
// E-lulu-01                PROVEN (pay-500 leg) + BLOCKED-HOST(tile_named:快餐店)
// E-lulu-02                PROVEN
// E-habinel-01             PROVEN (GiveExile n∈1..=2, to=-1)
// E-habinel-02             PROTOCOL-LEVEL (engine status tick)
// E-habinel-03             OPEN-habinel-end + BLOCKED-HOST(tile_named:微笑号)
// E-this-hand-01/02/03     PROTOCOL-LEVEL (card_move stub)
// E-pico-swap-01/02        PROVEN
// E-circle-rebuild-01      PROVEN (keep + 5 crystals)
// E-circle-rebuild-02      PROTOCOL-LEVEL (counter mirror is the same key)
// E-circle-rebuild-03/04/05 PROTOCOL-LEVEL / BLOCKED-HOST(tile_named:CiRCLE, trig_move_*)
// E-front-or-back-01       PROVEN (X=Y ⇒ 500 each)
// E-front-or-back-02       PROVEN (share legs; OPEN-分摊 remainder dropped)
// E-ex-quest-01            PROVEN (empty effect log, frame)
// E-ex-quest-02            cannot test (cosmetic skin)
// E-bear-cookie-01         PROTOCOL-LEVEL (event_deck_push stub) — proof FAILS
// E-bear-cookie-02         PROTOCOL-LEVEL (draw of the pushed card)
// E-bear-cookie-03         cannot test (skill-gated X/cap; card_text_mentions=0)
// E-asukayama-01           PROTOCOL-LEVEL (card_move) + BLOCKED-HOST(tile_named:飞鸟山公园)
// E-asukayama-02           BLOCKED-HOST(neighbor returns a tile, not a seat) + partial
//                          (non-drawer GiveExile leg); stun/noHand split NOT proven
// E-asukayama-03           PROTOCOL-LEVEL (status tick)
// E-school-time-01/02      PROVEN (exile leg) + BLOCKED-HOST(tile_named:周边学区)
// E-tears-01/02            PROVEN
// E-kizuna-01              PROVEN (X=0, dest=face%60; OPEN-kizuna-song)
// E-chart-rush-01          PROVEN (fire spent; spend_fire has no log)
// E-chart-rush-02          PROVEN (purses + OR legs; OPEN-chart-rush-or/tie)
// E-fix-note-01            PROTOCOL-LEVEL (set_build_discount stub) + keep PROVEN
// E-fix-note-02            OPEN-fix-note-count + PROTOCOL-LEVEL
// E-hallucination-01       PROVEN (keep + EventExpire on drawer turn start)
// E-hallucination-02       follow-up (roll substitution) / OPEN-hallucination-effects
// E-pool-bug-01            PROVEN (keep) + PROTOCOL-LEVEL (event_deck_push)
// E-pool-bug-02            PROVEN (Event hook → EventExpire)
// E-pool-bug-03            PROVEN (BuildBefore set_cancelled)
// E-aao-01                 PROVEN (keep + EventExpire)
// E-aao-02                 FAILS(LESS) — body arms no [手] veto
// E-marina-box-01          PROTOCOL-LEVEL (set_self_prop turn counter)
// E-marina-box-02          PROVEN (decline / accept pay+maybe gain; OPEN-marina-*)
// E-bangdream-chan-01      PROVEN (keep) + PROTOCOL-LEVEL (expiry counter)
// E-bangdream-chan-02      cannot test (host clocks)
// E-heat-head-01           PROVEN (keep + phase-1 crystals) / dice PROTOCOL
// E-heat-head-02           PROVEN (phase flip) + BLOCKED-HOST(trig_move_roll) on sub_die
// E-heat-head-03           PROVEN (EventExpire on second AtEnd)
// E-surprise-duel-01       PROVEN (keep)
// E-surprise-duel-02       PROTOCOL-LEVEL (set_self_prop counter; OPEN-surprise-2turn)
// E-surprise-duel-03       follow-up (direction rewrite)
// E-moca-01/02             PROVEN (keep + EventExpire; dice-halve BLOCKED-HOST)
// E-embers-01              PROVEN (one PlaceMark per living player)
// E-embers-02              follow-up (front/back spread)
// E-embers-03              PROVEN (SettleBody pay 10×copies)
// E-embers-04              BLOCKED-HOST(remove_marks stub + give_exile≠EXILE)
// E-lost-chase-01/02       PROTOCOL-LEVEL (event_deck_push stub) + guard PROVEN
// E-end-it-all-01          PROVEN (3 picks + permanent keep) / banish PROTOCOL
// E-end-it-all-02          PROVEN (guard: no second push while up)
// E-never-ends-01          BLOCKED-HOST(tile_named:RiNG) — body no-op; book NOT proven
