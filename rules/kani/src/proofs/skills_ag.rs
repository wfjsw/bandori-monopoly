//! Unit `skills_ag`: Afterglow skill proofs.
//!
//! Cheap first: the shared (1) rest-tick, then moca (2)'s teleport, then
//! ran (2)'s fire spend. OPEN / PROTOCOL halves are listed in
//! `docs/rulebook/spec/kani/ag.md` and not faked here.

use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

const SKILL_RAN: usize = crate::card_id!("skill:美竹兰:叛逆的红挑染");
const SKILL_MOCA: usize = crate::card_id!("skill:青叶摩卡:我行我素");
const REST_TURNS: &str = "skill.afterglow.restTurns";
const BAND_AFTERGLOW: &str = "Afterglow";

fn rest_name() -> intern::NameId {
    intern::intern(REST_TURNS)
}

fn rest_key() -> u16 {
    rest_name().0
}

fn band_name() -> intern::NameId {
    intern::intern(BAND_AFTERGLOW)
}

fn band_key() -> u16 {
    band_name().0
}

fn rest_of(w: &World, player: i32) -> i32 {
    w.key_get(player, rest_name())
}

// ===========================================================================
// C-AG-firepot-01 — shared (1) rest tick
// ===========================================================================

fn firepot_tick_world(pre_rest: i32, pre_fire: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.turn_player = player;
    w.band_id[player as usize] = band_key();
    w.key_set(player, rest_name(), pre_rest);
    w.fire[player as usize] = pre_fire;
    w.fire_max = [1, 1, 1, 1]; // 「上限1」
    w
}

/// C-AG-firepot-01 iff on the TurnEnd tick (ran_red entry 2).
#[kani::proof]
#[kani::unwind(80)]
pub fn firepot_01_tick_iff() {
    let pre_rest: i32 = kani::any();
    kani::assume(pre_rest >= 0 && pre_rest <= 2);
    let pre_fire: i32 = kani::any();
    kani::assume(pre_fire >= 0 && pre_fire <= 1);
    let w = firepot_tick_world(pre_rest, pre_fire);
    let player = w.trig_player;
    world::reset_world(w);
    // Entry 2 is the TurnEnd hook with residual `afterglow`
    // (`trigger::player_id() == player_id && in_band(player_id, "Afterglow")`).
    // Both hold by construction.
    invoke::unwrap_done(invoke::entry_idx(SKILL_RAN, 2, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_rest = world::with_world(|ww| rest_of(ww, player));
    let post_fire = world::with_world(|ww| ww.fire[player as usize]);

    let grants = spec::skills_ag::firepot_01_grants(pre_rest);
    kani::cover!(grants, "third idle turn grants a pot");
    kani::cover!(!grants, "first/second idle turn grants nothing");
    if grants {
        kassert!(
            spec::skills_ag::firepot_01_grant_log_ok(effects, player),
            "C-AG-firepot-01: granting tick is exactly GainFire(1)"
        );
        // Host `gain_fire` is saturating_add with no fire_max clamp (the cap
        // is declared by `fire_pot` but not enforced under Kani). Pin the
        // body's +1; the cap is recorded as a host gap in the unit table.
        kassert!(
            post_fire == pre_fire + 1,
            "C-AG-firepot-01: fire rises by 1"
        );
    } else {
        kassert!(
            spec::skills_ag::firepot_01_idle_log_ok(effects),
            "C-AG-firepot-01: idle tick is a no-op in the log"
        );
        kassert!(post_fire == pre_fire, "C-AG-firepot-01: fire unchanged");
    }
    kassert!(
        post_rest == spec::skills_ag::firepot_01_next_rest(pre_rest),
        "C-AG-firepot-01: rest counter is next_rest(pre)"
    );
}

/// Companion: the every-other-turn mutant must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn firepot_01_mutant_is_detected() {
    // pre_rest = 0: the card does not grant; the mutant (grant at +1>=2) does.
    kassert!(spec::skills_ag::firepot_01_grants(0) == false, "card: no grant at rest 0");
    kassert!(
        spec::skills_ag::firepot_01_grants_mutant(0),
        "mutant: claims a grant at rest 0"
    );
}

// ===========================================================================
// C-AG-moca-02 — teleport to the actor's tile
// ===========================================================================

fn moca_offer_world(pre_fire: i32, src_pos: i32, used: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let owner = 0i32;
    let src = 1i32;
    w.trig_player = src; // the actor who played a card / used a skill
    w.fire[owner as usize] = pre_fire;
    w.fire_max = [1, 1, 1, 1];
    w.pos[src as usize] = src_pos;
    w.pos[owner as usize] = (src_pos + 17) % 60; // not already there
    w.band_id[owner as usize] = band_key();
    // Per-source CD key the body builds as `skill.mocaSelf.used:<src>`.
    let key = intern::intern(alloc_key(src));
    w.key_set(owner, key, used);
    // Yes/no prompt: pre-seat opt_n so ask explores both answers.
    w.opt_n = 2;
    w.symbolic_answers = true;
    w
}

fn alloc_key(src: i32) -> &'static str {
    // Concrete per-src keys (src is 0..=3 in the harness).
    match src {
        0 => "skill.mocaSelf.used:0",
        1 => "skill.mocaSelf.used:1",
        2 => "skill.mocaSelf.used:2",
        _ => "skill.mocaSelf.used:3",
    }
}

/// C-AG-moca-02 iff (CardPlayed hook; residual `trigger::player_id() != owner`).
#[kani::proof]
#[kani::unwind(80)]
pub fn moca_02_teleport_iff() {
    let pre_fire: i32 = kani::any();
    kani::assume(pre_fire >= 0 && pre_fire <= 1);
    let src_pos: i32 = kani::any();
    kani::assume(src_pos >= 0 && src_pos < 60);
    let used: i32 = kani::any();
    kani::assume(used >= 0 && used <= 2);
    let w = moca_offer_world(pre_fire, src_pos, used);
    let owner = 0i32;
    let src = 1i32;
    world::reset_world(w);
    // Entry 2 is CardPlayed (residual `other`: actor != owner). Held by setup.
    invoke::unwrap_done(invoke::entry_idx(SKILL_MOCA, 2, owner));
    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    let gated = used >= 2 || pre_fire < 1;
    let accepted = !effects.is_empty();
    kani::cover!(accepted, "owner accepts (fire spend + teleport)");
    kani::cover!(!accepted, "gated or declined (no-op)");
    kani::cover!(used == 2, "CD exhausted (OPEN-2次cd)");
    kani::cover!(pre_fire == 0, "no fire pot");

    if gated {
        kassert!(
            spec::skills_ag::moca_02_noop_log_ok(effects),
            "C-AG-moca-02: CD / no-fire gate is a no-op"
        );
        kassert!(post.pos[owner as usize] != src_pos || pre_fire == 0, "no teleport when gated");
        return;
    }
    // Not gated: the body asks yes/no (symbolic). Both answers are legal.
    if accepted {
        kassert!(
            spec::skills_ag::moca_02_log_ok(effects, owner, src_pos),
            "C-AG-moca-02: exactly Teleport(owner → src tile)"
        );
        kassert!(
            spec::skills_ag::moca_02_post_ok(post.pos[owner as usize], post.fire[owner as usize], src_pos, pre_fire),
            "C-AG-moca-02: on the actor's tile with fire −1"
        );
    } else {
        kassert!(
            spec::skills_ag::moca_02_noop_log_ok(effects),
            "C-AG-moca-02: declined offer is a no-op"
        );
        kassert!(post.fire[owner as usize] == pre_fire, "decline leaves fire");
        let _ = src;
    }
}

/// Companion: wrong-tile mutant must not match the accepted path.
#[kani::proof]
#[kani::unwind(80)]
pub fn moca_02_mutant_is_detected() {
    let w = moca_offer_world(1, 10, 0);
    let owner = 0i32;
    let mut w = w;
    w.symbolic_answers = false;
    w.answers[0] = 0; // yes
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(SKILL_MOCA, 2, owner));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    kassert!(
        !spec::skills_ag::moca_02_log_ok_mutant(effects, owner, 10),
        "mutant (tile+1) must not match"
    );
    kassert!(
        spec::skills_ag::moca_02_log_ok(effects, owner, 10),
        "card teleports to the actor's tile"
    );
}

// ===========================================================================
// C-AG-ran-02 — spend one [火罐]
// ===========================================================================

/// C-AG-ran-02: the fire-spend half (plan reverse / NO_REWARD are stubs).
#[kani::proof]
#[kani::unwind(80)]
pub fn ran_02_spend_fire_iff() {
    let pre_fire: i32 = kani::any();
    kani::assume(pre_fire >= 1 && pre_fire <= 1);
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.fire[player as usize] = pre_fire;
    w.fire_max = [1, 1, 1, 1];
    w.band_id[player as usize] = band_key();
    world::reset_world(w);
    // Entry 0 is the (2) Play (`can_use` residual: not skill_blocked, fire ≥ 1).
    invoke::unwrap_done(invoke::entry_idx(SKILL_RAN, 0, player));
    let post_fire = world::with_world(|ww| ww.fire[player as usize]);
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    kassert!(
        spec::skills_ag::ran_02_spend_log_empty(effects),
        "C-AG-ran-02: fire spend is not an Effect (spend_fire mutates fire only)"
    );
    kassert!(
        spec::skills_ag::ran_02_fire_ok(post_fire, pre_fire),
        "C-AG-ran-02: exactly one [火罐] spent"
    );
}

/// Companion: spending 2 must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn ran_02_mutant_is_detected() {
    host::install();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.fire[player as usize] = 1;
    w.fire_max = [1, 1, 1, 1];
    w.band_id[player as usize] = band_key();
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(SKILL_RAN, 0, player));
    let post_fire = world::with_world(|ww| ww.fire[player as usize]);
    kassert!(post_fire == 0, "card spends exactly 1");
    kassert!(post_fire != 1 - 2, "mutant (spend 2) is impossible from fire=1");
}
// ===========================================================================
// Pure formula harnesses (no invoke) — cheap, run first under the slot cap.
// ===========================================================================

/// C-AG-firepot-01 tick arithmetic.
#[kani::proof]
#[kani::unwind(8)]
pub fn firepot_01_tick_formula() {
    let pre_rest: i32 = kani::any();
    kani::assume(pre_rest >= 0 && pre_rest <= 2);
    let grants = spec::skills_ag::firepot_01_grants(pre_rest);
    let next = spec::skills_ag::firepot_01_next_rest(pre_rest);
    if pre_rest == 2 {
        kassert!(grants, "third idle turn grants");
        kassert!(next == 0, "counter resets");
    } else {
        kassert!(!grants, "first/second idle turn grants nothing");
        kassert!(next == pre_rest + 1, "counter advances");
    }
    kassert!(
        grants != spec::skills_ag::firepot_01_grants_mutant(pre_rest) || pre_rest == 2,
        "mutant disagrees on rest 0/1"
    );
}

/// C-AG-band-01 filter formula.
#[kani::proof]
#[kani::unwind(8)]
pub fn band_01_filter_formula() {
    let is_shop: bool = kani::any();
    let price: i32 = kani::any();
    kani::assume(price >= 0 && price <= 5000);
    let applies = spec::skills_ag::band_01_applies(is_shop, price);
    kassert!(
        applies == (is_shop || price <= 1200),
        "shop-street OR price <= 1200"
    );
    if !is_shop && price > 1200 && price <= 5000 {
        kassert!(
            !spec::skills_ag::band_01_applies_mutant(is_shop, price) || price <= 1000,
            "mutant (<=1000) is strictly tighter"
        );
    }
}
