//! Unit `ppp`: clause proofs.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. MUTANT whose disagreement is reachable.
//!
//! Pattern taken from `proofs/teleport.rs` (pilot `PPP:（多惠）寻找更好的声音`).
//! Isolation assumes are listed in the module docs of each harness.

use crate::host;
use crate::invoke;
use crate::ir_fill::{self, TriggerOverlay};
use crate::spec;
use crate::world::{self, Effect, World, MAX_LOG, MAX_PLAYERS};

/// Shared symbolic play world: 2..=4 living players, small money, one actor.
fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume((2..=4).contains(&w.n_players));
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume((0..=10_000).contains(&w.money[p]));
        w.pos[p] = kani::any();
        kani::assume((0..60).contains(&w.pos[p]));
        w.fire[p] = kani::any();
        kani::assume((0..=20).contains(&w.fire[p]));
        w.out[p] = false;
    }
    // Leave room on the draw pile / field so inserts are not silently dropped.
    w.deck_n = 0;
    w.hand_n = 0;
    w.field_n = 0;
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    w.symbolic_answers = true;
    w
}

fn log_slice() -> ([Effect; MAX_LOG], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn effects_of(logbuf: &[Effect; MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

// ===========================================================================
// C-PPP-popipa-01
// 「获得1000资金并将一张“Pipopa”加入抽牌堆，然后抽一张牌，然后此卡[移除]」
// Isolation: deck has room for the Pipopa insert; the draw is unconditional
// in the host model (empty deck still logs `Draw`).
// ===========================================================================

/// IFF: exactly GainMoney(1000) then Draw(1); deck +1 Pipopa; dest Banished.
#[kani::proof]
#[kani::unwind(32)]
pub fn popipa_01_iff() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    let pre_money = w.money[player as usize];
    let pre_deck_pipopa = spec::deck_count_named(&w, spec::CARD_PIPOPA);
    let pre_hand_n = w.hand_n as i32;
    kani::cover!(pre_deck_pipopa == 0, "no pre-existing Pipopa in deck");
    kani::cover!(pre_deck_pipopa >= 1, "deck already holds Pipopa");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play(spec::CARD_POPIPA, player));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let dest = world::with_world(|ww| ww.dest);
    let deck_pipopa = world::with_world(|ww| spec::deck_count_named(ww, spec::CARD_PIPOPA));
    let hand_n = world::with_world(|ww| ww.hand_n as i32);

    assert!(
        spec::popipa_effects_ok(effects_of(&logbuf, log_n), player),
        "popipa: effect log must be exactly GainMoney(1000), Draw(1)"
    );
    assert!(
        spec::popipa_post_ok(
            pre_money,
            pre_deck_pipopa,
            pre_hand_n,
            post.money[player as usize],
            deck_pipopa,
            hand_n,
            dest
        ),
        "popipa: money+1000, deck+1 Pipopa, hand+1, dest Banished"
    );
    assert!(
        spec::frame_others_money(&pre, &post, player),
        "popipa: other players' money is frame"
    );
}

/// MUTANT: reference wants +1100. Card must not match.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn popipa_01_mutant_money_off_by_100() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play(spec::CARD_POPIPA, player));
    let (logbuf, log_n) = log_slice();
    assert!(
        spec::popipa_effects_ok_mutant(effects_of(&logbuf, log_n), player),
        "mutant: card must not match the +1100 spec"
    );
}

// ===========================================================================
// C-PPP-pipopa-01
// 「获得1000资金，然后将一张“Popipapapipopa”放置在自身场上，然后此卡[移除]」
// Isolation: field has room for the placed instance.
// ===========================================================================

/// Host maps unknown PPP names to slot 2 under the Kani intern facade.
const POPIPAPAPIPOPA_SLOT: u8 = 2;

#[kani::proof]
#[kani::unwind(32)]
pub fn pipopa_01_iff() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    let pre_money = w.money[player as usize];
    let pre_field_n = w.field_n;
    kani::cover!(pre_field_n == 0, "empty field before play");

    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play(spec::CARD_PIPOPA, player));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let dest = world::with_world(|ww| ww.dest);
    let field_has_new = world::with_world(|ww| ww.field_n > pre_field_n);

    assert!(
        spec::pipopa_effects_ok(effects_of(&logbuf, log_n), player, POPIPAPAPIPOPA_SLOT),
        "pipopa: log must be exactly GainMoney(1000), PlaceCard(Popipapapipopa, Field)"
    );
    assert!(
        spec::pipopa_post_ok(pre_money, post.money[player as usize], dest, field_has_new),
        "pipopa: money+1000, field+1, dest Banished"
    );
}

/// MUTANT: reference wants a 2000 gain.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn pipopa_01_mutant_money_2000() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play(spec::CARD_PIPOPA, player));
    let (logbuf, log_n) = log_slice();
    assert!(
        spec::pipopa_effects_ok_mutant(effects_of(&logbuf, log_n), player, POPIPAPAPIPOPA_SLOT),
        "mutant: card must not match the +2000 spec"
    );
}

// ===========================================================================
// C-PPP-signpost-02
// 「回合结束时[失去]1000资金」
// Condition (entry guard): `owner.out == 0` (single-source IR).
// Isolation: the AtEnd entry is invoked directly; no other turn-end hooks.
// Host `pay` clamps money at 0; the book's 「失去] is the money-out leg, so
// the clean case (`pre_money >= 1000`) is asserted and the clamp is covered.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn signpost_02_lose_1000_at_end_iff() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    let pre_money = w.money[player as usize];

    // single-source guard: the real CEL string on `On::AtEnd`
    let gid = guard_id!("owner.out == 0");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, player, 0, false, -1, &ov);
    let admitted = guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (owner alive)");
    kani::cover!(!admitted, "guard rejects (owner out)");
    if !admitted {
        // host would not call the body; world untouched
        let (logbuf, log_n) = log_slice();
        assert!(log_n == 0, "guard reject must be a no-op");
        let _ = logbuf;
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::hook("PPP:向着未来的路标", "AtEnd", "", player));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    assert!(
        spec::signpost_02_effects_ok(effects_of(&logbuf, log_n), player),
        "signpost-02: log must be exactly PayMoney(1000)"
    );
    // Book: 「失去]1000. Host pay clamps at 0; assert the amount and the
    // unclamped case, cover the clamp.
    if pre_money >= 1000 {
        assert!(
            spec::signpost_02_post_ok(pre_money, post.money[player as usize]),
            "signpost-02: money falls by exactly 1000"
        );
    } else {
        assert!(
            post.money[player as usize] == 0,
            "signpost-02: host pay clamps at 0 when money < 1000"
        );
    }
    kani::cover!(pre_money == 1000, "money exactly 1000");
    kani::cover!(pre_money > 1000, "money above 1000");
    kani::cover!(pre_money < 1000, "money below 1000 (host clamp)");
}

/// MUTANT: reference wants a 500 loss.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn signpost_02_mutant_lose_500() {
    let w = symbolic_play_world();
    let player = w.trig_player;
    let pre_money = w.money[player as usize];
    kani::assume(pre_money >= 1000);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook("PPP:向着未来的路标", "AtEnd", "", player));
    let (logbuf, log_n) = log_slice();
    assert!(
        spec::signpost_02_effects_ok_mutant(effects_of(&logbuf, log_n), player),
        "mutant: card must not match the 500-loss spec"
    );
}