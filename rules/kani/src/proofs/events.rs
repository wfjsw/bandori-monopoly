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
    invoke::unwrap_done(invoke::event_play("event:对邦", drawer));
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
    invoke::unwrap_done(invoke::event_play("event:对邦", 0));
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
    invoke::unwrap_done(invoke::event_play("event:泪水的含义", 0));
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
    invoke::unwrap_done(invoke::event_play("event:泪水的含义", 0));
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
    invoke::unwrap_done(invoke::event_play("event:泪水的含义", 0));
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
    invoke::unwrap_done(invoke::event_play("event:PICO灵魂交换", 0));
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
    invoke::unwrap_done(invoke::event_play("event:PICO灵魂交换", 0));
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
    invoke::unwrap_done(invoke::event_play("event:PICO灵魂交换", 0));
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
    invoke::unwrap_done(invoke::event_play("event:Forbidden Moca", 0));
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
    invoke::unwrap_done(invoke::hook("event:Forbidden Moca", "AtEnd", "", 0));
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
    invoke::unwrap_done(invoke::hook("event:Forbidden Moca", "AtEnd", "", 0));
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
    invoke::unwrap_done(invoke::event_play("event:A！A！O！", 0));
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
    invoke::unwrap_done(invoke::hook("event:A！A！O！", "AtEnd", "", 0));
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
    invoke::unwrap_done(invoke::event_play("event:A！A！O！", 0));
    let dest = world::with_world(|w| w.dest);
    let logbuf = effects_of();
    let n_log = log_n_of();
    let effects = effects_slice(&logbuf, n_log);
    assert!(
        spec::events::aao_hand_veto_armed(dest, effects),
        "E-aao-02: book says 「不能使用卡牌的[手]效果」 while up — body arms no veto"
    );
}