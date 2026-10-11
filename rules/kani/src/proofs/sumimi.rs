//! Unit `sumimi`: clause proofs.
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. mutant (reachable-disagreement companion preferred).
//!
//! Isolation assumes are listed per harness.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::ir_fill;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

fn base_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 8);
        w.fire_max[p] = 2;
        w.out[p] = false;
    }
    w
}

fn log_slice() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

fn effects_of(logbuf: &[Effect; 24], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

/// Play gate of a `On::Play(_, Some(gate), _)` entry. `true` = playable.
fn play_gate_allows(id: &str, player: i32) -> bool {
    let card = match invoke::find(id) {
        Some(c) => c,
        None => return false,
    };
    for on in card.on.iter() {
        if let card_sdk::On::Play(_, gate, _) = on {
            return match gate {
                Some(g) => g(player).is_none(),
                None => true,
            };
        }
    }
    false
}

// ===========================================================================
// C-SUMIMI-idolband -- Sumimi:兼顾偶像与乐队
// 「当你本回合未进行过赎回操作时可打出，将你的资金重设为3000。
//  （若你的资金为3000以上则无效果）」
// ===========================================================================

const IDOLBAND: &str = "Sumimi:兼顾偶像与乐队";

fn idolband_world(redeemed: i32) -> World {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    let key = intern::intern("redeemed");
    w.key_set(player, key, redeemed);
    w
}

/// C-SUMIMI-idolband-01 + -02 + -03 + -frame.
#[kani::proof]
#[kani::unwind(24)]
pub fn idolband_reset_iff() {
    let w = idolband_world(kani::any());
    let player = w.trig_player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let redeemed = world::with_world(|ww| ww.key_get(player, intern::intern("redeemed")));
    let playable = play_gate_allows(IDOLBAND, player);

    kani::cover!(playable, "gate admits (not redeemed)");
    kani::cover!(!playable, "gate rejects (redeemed)");
    kani::cover!(pre.money[player as usize] < 3000, "pre money < 3000");
    kani::cover!(pre.money[player as usize] >= 3000, "pre money >= 3000");

    assert!(
        playable == (redeemed == 0),
        "C-SUMIMI-idolband-01: gate must allow exactly the not-redeemed case"
    );

    if !playable {
        let post = world::snapshot();
        assert!(post.money == pre.money, "rejected play must not touch money");
        let (logbuf, log_n) = log_slice();
        assert!(log_n == 0, "rejected play must not log");
        let _ = effects_of(&logbuf, log_n);
        return;
    }

    invoke::unwrap_done(invoke::play(IDOLBAND, player));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let pre_money = pre.money[player as usize];
    assert!(
        spec::sumimi::idolband_money_ok(effects, player, pre_money),
        "C-SUMIMI-idolband-02/03: money path must be a set-to-3000 or a no-op"
    );
    assert!(
        post.money[player as usize] == spec::sumimi::idolband_post_money(pre_money),
        "C-SUMIMI-idolband-02: post money must be exactly 3000 when pre < 3000"
    );
    for p in 0..MAX_PLAYERS {
        if p != player as usize {
            assert!(post.money[p] == pre.money[p], "frame: other players' money");
        }
        assert!(post.fire[p] == pre.fire[p], "frame: fire");
        assert!(post.pos[p] == pre.pos[p], "frame: pos");
        assert!(post.stay[p] == pre.stay[p], "frame: stay");
    }
    assert!(post.hand == pre.hand, "frame: hand bitset");
    assert!(post.discard == pre.discard, "frame: discard bitset");
}

/// Mutant companion: the off-by-one target (2999) must NOT match the card.
#[kani::proof]
#[kani::unwind(24)]
pub fn idolband_mutant_is_detected() {
    let mut w = idolband_world(0);
    let player = w.trig_player;
    w.money[player as usize] = kani::any();
    kani::assume(w.money[player as usize] >= 0 && w.money[player as usize] < 3000);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play(IDOLBAND, player));
    let post = world::snapshot();
    let pre_money = pre.money[player as usize];
    let wrong = spec::sumimi::idolband_post_money_mutant(pre_money);
    kani::cover!(pre_money < 3000, "mutant region live");
    assert!(
        post.money[player as usize] != wrong,
        "mutant: card must not match the 2999 target"
    );
}

// ===========================================================================
// C-SUMIMI-hina-01 -- pass CiRCLE → +1 [火罐]
// `skill:三角初华（Sumimi）:成为偶像` `On::Hook(Pass)` with the single-source
// guard `"actor == owner && is_circle(tile.id)"`.
// Isolation: the turn-start pot placement (clause (2)) is a different entry.
// ===========================================================================

const HINA: &str = "skill:三角初华（Sumimi）:成为偶像";

/// C-SUMIMI-hina-01 iff: pass CiRCLE → +1 [火罐] under the hold cap.
#[kani::proof]
#[kani::unwind(24)]
pub fn hina_01_pass_circle_iff() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    // Board slot 0 is 「CiRCLE」 (board.json index 1). Symbolic over the
    // 60-tile board so both the circle and non-circle covers are live.
    let tile: i32 = kani::any();
    kani::assume(tile >= 0 && tile < 60);
    w.trig_tile = tile;

    // single-source guard: the real CEL string, lowered once
    let gid = crate::guard_id!("actor == owner && is_circle(tile.id)");
    let ov = ir_fill::TriggerOverlay::default();
    let ctx = ir_fill::fill_from_world(&w, player, 0, true, -1, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (on a CiRCLE tile)");
    kani::cover!(!admitted, "guard rejects (not CiRCLE / not owner)");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    if !admitted {
        // Host would not call the body. Untouched.
        let post = world::snapshot();
        assert!(post.fire == pre.fire, "rejected hook must not touch fire");
        let (logbuf, log_n) = log_slice();
        assert!(log_n == 0, "rejected hook must not log");
        let _ = effects_of(&logbuf, log_n);
        return;
    }

    let pre_fire = pre.fire[player as usize];
    let cap = spec::sumimi::FIRE_CAP_SUMIMI;
    kani::cover!(pre_fire < cap, "under cap (gain applies)");
    kani::cover!(pre_fire >= cap, "at/over cap (global hold-cap forbids)");

    invoke::unwrap_done(invoke::hook_only(HINA, "Pass", player));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    assert!(
        spec::sumimi::pass_circle_fire_ok(effects, player, pre_fire, cap),
        "C-SUMIMI-hina-01: effect log must be exactly +1 fire under the cap"
    );
    assert!(
        post.fire[player as usize] == spec::sumimi::pass_circle_fire_post(pre_fire, cap),
        "C-SUMIMI-hina-01: post fire must be pre+1 under the cap and unchanged at the cap"
    );
    for p in 0..MAX_PLAYERS {
        if p != player as usize {
            assert!(post.fire[p] == pre.fire[p], "frame: other players' fire");
        }
        assert!(post.money[p] == pre.money[p], "frame: money");
        assert!(post.pos[p] == pre.pos[p], "frame: pos");
    }
}

/// Mutant: +2 instead of +1 (under the cap).
#[kani::proof]
#[kani::unwind(24)]
pub fn hina_01_mutant_is_detected() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    w.trig_tile = 0; // CiRCLE
    w.fire[player as usize] = 0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook_only(HINA, "Pass", player));
    let post = world::snapshot();
    let wrong = spec::sumimi::pass_circle_fire_post_mutant(pre.fire[player as usize], spec::sumimi::FIRE_CAP_SUMIMI);
    assert!(
        post.fire[player as usize] != wrong,
        "mutant: card must not match the +2 spec"
    );
}

// ===========================================================================
// C-SUMIMI-childhood-02 -- after-move +1 [火罐]
// `Sumimi:(初华（Sumimi）)儿时玩伴的鼓励` `On::Hook(MoveAfter)`.
// Guard: `"actor == owner && move.main && card.placed"`.
// C-SUMIMI-childhood-01 (move start = 小豆岛) is PROTOCOL-LEVEL (plan stub).
// ===========================================================================

const CHILDHOOD: &str = "Sumimi:(初华（Sumimi）)儿时玩伴的鼓励";

/// C-SUMIMI-childhood-02 iff: after the re-based move, +1 [火罐].
#[kani::proof]
#[kani::unwind(24)]
pub fn childhood_02_fire_iff() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;

    let gid = crate::guard_id!("actor == owner && move.main && card.placed");
    let mut ov = ir_fill::TriggerOverlay::default();
    ov.move_main = true;
    let ctx = ir_fill::fill_from_world(&w, player, 0, true, -1, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits");
    kani::cover!(!admitted, "guard rejects");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    if !admitted {
        let post = world::snapshot();
        assert!(post.fire == pre.fire, "rejected hook must not touch fire");
        let (logbuf, log_n) = log_slice();
        assert!(log_n == 0, "rejected hook must not log");
        let _ = effects_of(&logbuf, log_n);
        return;
    }

    let pre_fire = pre.fire[player as usize];
    let cap = spec::sumimi::FIRE_CAP_SUMIMI;
    kani::cover!(pre_fire < cap, "under cap");
    kani::cover!(pre_fire >= cap, "at cap");

    invoke::unwrap_done(invoke::hook_only(CHILDHOOD, "MoveAfter", player));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    assert!(
        spec::sumimi::childhood_fire_ok(effects, player, pre_fire, cap),
        "C-SUMIMI-childhood-02: after-move must be exactly +1 fire under the cap"
    );
    assert!(
        post.fire[player as usize] == spec::sumimi::childhood_fire_post(pre_fire, cap),
        "C-SUMIMI-childhood-02: post fire"
    );
    for p in 0..MAX_PLAYERS {
        if p != player as usize {
            assert!(post.fire[p] == pre.fire[p], "frame: other fire");
        }
        assert!(post.money[p] == pre.money[p], "frame: money");
        assert!(post.pos[p] == pre.pos[p], "frame: pos");
    }
}

#[kani::proof]
#[kani::unwind(24)]
pub fn childhood_02_mutant_is_detected() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    w.fire[player as usize] = 0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook_only(CHILDHOOD, "MoveAfter", player));
    let post = world::snapshot();
    let wrong = spec::sumimi::childhood_fire_post(0, spec::sumimi::FIRE_CAP_SUMIMI) + 1;
    assert!(
        post.fire[player as usize] != wrong,
        "mutant: after-move is +1, not +2"
    );
}

// ===========================================================================
// C-SUMIMI-twodonuts-01 -- 「（1）获得[除外]…」
// `Sumimi:一人两个甜甜圈` play body.
// (2)/(3) are PROTOCOL-LEVEL (multi-act pass window).
// ===========================================================================

const TWODONUTS: &str = "Sumimi:一人两个甜甜圈";

#[kani::proof]
#[kani::unwind(24)]
pub fn twodonuts_01_exile_iff() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play(TWODONUTS, player));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    assert!(
        spec::sumimi::twodonuts_exile_ok(effects, player),
        "C-SUMIMI-twodonuts-01: exactly one [除外] grant and no money/pos/fire side effects"
    );
    for p in 0..MAX_PLAYERS {
        assert!(post.money[p] == pre.money[p], "frame: money");
        assert!(post.pos[p] == pre.pos[p], "frame: pos");
        assert!(post.fire[p] == pre.fire[p], "frame: fire");
    }
}

#[kani::proof]
#[kani::unwind(24)]
pub fn twodonuts_01_mutant_is_detected() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play(TWODONUTS, player));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    // Mutant: no exile grant at all. The card does grant — disagreement is live.
    let wrong = !spec::sumimi::twodonuts_exile_grant_in(effects, player);
    assert!(!wrong, "mutant: card does grant [除外]");
}

// ===========================================================================
// C-SUMIMI-twoinone-01b -- fire becomes the other character's 初始火罐数
// `Sumimi:Sumimi是二人一体的` play body.
// C-SUMIMI-twoinone-01 (character identity) is a known engine gap (the
// bound skill is swapped but `player.character` stays) — the fire half is
// what this harness pins. C-SUMIMI-twoinone-02 is PROTOCOL-LEVEL.
// ===========================================================================

const TWOINONE: &str = "Sumimi:Sumimi是二人一体的";

#[kani::proof]
#[kani::unwind(24)]
pub fn twoinone_01b_fire_iff() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    let is_hina: bool = kani::any();
    let name = if is_hina {
        "三角初华（Sumimi）"
    } else {
        "纯田真奈"
    };
    w.character_id[player as usize] = intern::intern(name).0;

    let to = spec::sumimi::twoinone_fire_target(is_hina);
    let fire_key = intern::intern(card_sdk::abi::state_key::FIRE).0;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play(TWOINONE, player));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    kani::cover!(is_hina, "from 三角初华（Sumimi） → 纯田真奈 fire 1");
    kani::cover!(!is_hina, "from 纯田真奈 → 三角初华（Sumimi） fire 2");

    assert!(
        spec::sumimi::twoinone_fire_effects_ok(effects, player, fire_key, to),
        "C-SUMIMI-twoinone-01b: fire must be set to the other character's 初始火罐数"
    );
    let _ = pre;
    let _ = post;
}

#[kani::proof]
#[kani::unwind(24)]
pub fn twoinone_01b_mutant_is_detected() {
    let mut w = base_world();
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < w.n_players);
    w.trig_player = player;
    let is_hina: bool = kani::any();
    let name = if is_hina {
        "三角初华（Sumimi）"
    } else {
        "纯田真奈"
    };
    w.character_id[player as usize] = intern::intern(name).0;
    let to = spec::sumimi::twoinone_fire_target(is_hina);
    let wrong = to + 1;
    let fire_key = intern::intern(card_sdk::abi::state_key::FIRE).0;
    world::reset_world(w);
    invoke::unwrap_done(invoke::play(TWOINONE, player));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let mut saw_wrong = false;
    for e in effects {
        if let Effect::SetState { player: p, key, n } = e {
            if *p == player && *key == fire_key && *n == wrong {
                saw_wrong = true;
            }
        }
    }
    assert!(!saw_wrong, "mutant: fire is not set to 初始+1");
}
// ===========================================================================
// Cheap pure-formula harnesses (spec math, no host invoke) — run these first.
// ===========================================================================

/// C-SUMIMI-idolband money formula.
#[kani::proof]
#[kani::unwind(8)]
pub fn idolband_money_formula() {
    let pre: i32 = kani::any();
    kani::assume(pre >= 0 && pre <= 10_000);
    let post = spec::sumimi::idolband_post_money(pre);
    if pre >= 3000 {
        assert!(post == pre, "no-effect region keeps money");
    } else {
        assert!(post == 3000, "under-3000 region sets exactly 3000");
    }
    let wrong = spec::sumimi::idolband_post_money_mutant(pre);
    if pre < 3000 {
        assert!(wrong == 2999 && post != wrong, "mutant 2999 is distinguishable");
    }
    kani::cover!(pre < 3000, "under 3000");
    kani::cover!(pre >= 3000, "at/over 3000");
}

/// C-SUMIMI-mana-01 / hina-01 fire formula under the hold cap.
#[kani::proof]
#[kani::unwind(8)]
pub fn pass_circle_fire_formula() {
    let pre_fire: i32 = kani::any();
    kani::assume(pre_fire >= 0 && pre_fire <= 8);
    let cap = spec::sumimi::FIRE_CAP_SUMIMI;
    let post = spec::sumimi::pass_circle_fire_post(pre_fire, cap);
    if pre_fire < cap {
        assert!(post == pre_fire + 1, "under cap: +1");
    } else {
        assert!(post == pre_fire, "at cap: no-op (global hold-cap)");
    }
    let wrong = spec::sumimi::pass_circle_fire_post_mutant(pre_fire, cap);
    assert!(post != wrong, "mutant +2 / past-cap is distinguishable");
    kani::cover!(pre_fire < cap, "under cap");
    kani::cover!(pre_fire >= cap, "at cap");
}

/// C-SUMIMI-band-02 rent-bonus formula.
#[kani::proof]
#[kani::unwind(8)]
pub fn band_02_rent_bonus_formula() {
    let pre_amount: i32 = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 10_000);
    let crystals: i32 = kani::any();
    kani::assume(crystals >= 0 && crystals <= 8);
    let post = spec::sumimi::band_rent_bonus_amount(pre_amount, crystals);
    assert!(post == pre_amount + crystals * 100, "bonus is +100 per crystal");
    let wrong = spec::sumimi::band_rent_bonus_amount_mutant(pre_amount, crystals);
    if crystals != 1 {
        assert!(post != wrong, "mutant +100-total distinguishable when c != 1");
    }
    kani::cover!(crystals == 0, "c=0");
    kani::cover!(crystals >= 2, "c>=2");
}
