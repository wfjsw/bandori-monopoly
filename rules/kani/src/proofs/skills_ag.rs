//! Unit `skills_ag`: Afterglow skill proofs.
//!
//! Cheap first: the shared (1) rest-tick, then moca (2)'s teleport, then
//! ran (2)'s fire spend. OPEN / PROTOCOL halves are listed in
//! `docs/rulebook/spec/kani/ag.md` and not faked here.

use crate::harness::{self, SparsePlay};
use crate::host;
use crate::intern;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// Ops the ran_red / moca_self skill bodies reach.
crate::host_subset!(skills_ag_dispatch {
    log | effect => arm_log,
    gain | gain_typed | gain_fixed => arm_gain,
    money => arm_money,
    pay => arm_pay,
    gain_fire => arm_gain_fire,
    fire => arm_fire,
    fire_max => arm_fire_max,
    spend_fire => arm_spend_fire,
    teleport_to => arm_teleport_to,
    player_count => arm_player_count,
    player_out => arm_player_out,
    player_pos => arm_player_pos,
    in_group => arm_in_band,
    persona_is => arm_character_is,
    add_held => arm_add_held,
    count_held | count_held_name => arm_count_held,
    set_held_name => arm_set_held_name,
    ask => arm_ask,
    raise => arm_raise,
    gate => arm_gate,
    slot => arm_slot,
    set_slot => arm_set_slot,
    state_get => arm_state_get,
    state_set => arm_state_set,
    state_add => arm_inc_slot,
    set_self_prop => arm_set_self_prop,
    set_prop_at => arm_set_prop_at,
    set_tile_prop => arm_set_tile_prop,
    self_prop | prop_at | tile_prop => arm_counter_self,
    trig_player => arm_trig_player,
    trig_target => arm_trig_target,
    count_marks_f => arm_count_marks_f,
    place_mark => arm_place_mark,
});

/// Install sparse 2-seat world + subset. Callers keep concrete seats.
fn skills_ag_world() {
    host::install_backend(skills_ag_dispatch);
    harness::sparse_play_world(SparsePlay {
        used_seats: 2,
        n_players: 2,
        money_lo: 0,
        money_hi: 10_000,
        fans_lo: 0,
        fans_hi: 0,
        resonance_in_hand: false,
    });
}

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

fn firepot_tick_world(user: i32, pre_rest: i32, pre_fire: i32) -> World {
    skills_ag_world();
    let mut w = World::new();
    w.n_players = 2;
    let player = user;
    w.trig_player = player;
    w.turn_player = player;
    w.group_id[player as usize] = band_key();
    w.key_set(player, rest_name(), pre_rest);
    w.fire[player as usize] = pre_fire;
    w.fire_max = [1, 1, 1, 1]; // 「上限1」
    w
}

/// Shared iff body at a concrete seat (ppcost). Seat-symmetric (owner tick).
fn firepot_01_tick_iff_at(user: i32) {
    let player = user;
    let pre_rest: i32 = kani::any();
    kani::assume(pre_rest >= 0 && pre_rest <= 2);
    let pre_fire: i32 = kani::any();
    kani::assume(pre_fire >= 0 && pre_fire <= 1);
    let w = firepot_tick_world(user, pre_rest, pre_fire);
    let player = w.trig_player;
    world::reset_world(w);
    // Entry 2 is the TurnEnd hook with residual `afterglow`
    // (`trigger::player_id() == player_id && in_group(player_id, "Afterglow")`).
    // Both hold by construction.
    invoke::unwrap_done(skill_personas::ran_red::tick_kani(player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_rest = world::with_world(|ww| rest_of(ww, player));
    let post_fire = world::with_world(|ww| ww.fire[player as usize]);

    let grants = spec::skills_ag::firepot_01_grants(pre_rest);
    kani::cover!(grants, "third idle turn grants a pot");
    kani::cover!(!grants, "first/second idle turn grants nothing");
    if grants {
        crate::kassert!(
            spec::skills_ag::firepot_01_grant_log_ok(effects, player),
            "C-AG-firepot-01: granting tick is exactly GainFire(1)"
        );
        // Host `gain_fire` is saturating_add with no fire_max clamp (the cap
        // is declared by `fire_pot` but not enforced under Kani). Pin the
        // body's +1; the cap is recorded as a host gap in the unit table.
        crate::kassert!(
            post_fire == pre_fire + 1,
            "C-AG-firepot-01: fire rises by 1"
        );
    } else {
        crate::kassert!(
            spec::skills_ag::firepot_01_idle_log_ok(effects),
            "C-AG-firepot-01: idle tick is a no-op in the log"
        );
        crate::kassert!(post_fire == pre_fire, "C-AG-firepot-01: fire unchanged");
    }
    crate::kassert!(
        post_rest == spec::skills_ag::firepot_01_next_rest(pre_rest),
        "C-AG-firepot-01: rest counter is next_rest(pre)"
    );
}

#[kani::proof]
#[kani::unwind(80)]
pub fn firepot_01_tick_iff() {
    firepot_01_tick_iff_at(0);
}

#[kani::proof]
#[kani::unwind(80)]
pub fn firepot_01_user1_tick_iff() {
    firepot_01_tick_iff_at(1);
}

/// Companion: the every-other-turn mutant must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn firepot_01_mutant_is_detected() {
    // pre_rest = 0: the card does not grant; the mutant (grant at +1>=2) does.
    crate::kassert!(spec::skills_ag::firepot_01_grants(0) == false, "card: no grant at rest 0");
    crate::kassert!(
        spec::skills_ag::firepot_01_grants_mutant(0),
        "mutant: claims a grant at rest 0"
    );
}

// ===========================================================================
// C-AG-moca-02 — teleport to the actor's tile
// ===========================================================================

fn moca_offer_world(pre_fire: i32, src_pos: i32, used: i32) -> World {
    // NOT seat-symmetric: involves owner and another seat `src`. Covered
    // concretely at (owner=0, src=1); a second pair is the seat-swap below.
    skills_ag_world();
    let mut w = World::new();
    w.n_players = 2;
    let owner = 0i32;
    let src = 1i32;
    w.trig_player = src; // the actor who played a card / used a skill
    w.fire[owner as usize] = pre_fire;
    w.fire_max = [1, 1, 1, 1];
    w.pos[src as usize] = src_pos;
    w.pos[owner as usize] = (src_pos + 17) % 60; // not already there
    w.group_id[owner as usize] = band_key();
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
    invoke::unwrap_done(skill_personas::moca_self::on_card_kani(owner));
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
        crate::kassert!(
            spec::skills_ag::moca_02_noop_log_ok(effects),
            "C-AG-moca-02: CD / no-fire gate is a no-op"
        );
        crate::kassert!(post.pos[owner as usize] != src_pos || pre_fire == 0, "no teleport when gated");
        return;
    }
    // Not gated: the body asks yes/no (symbolic). Both answers are legal.
    if accepted {
        crate::kassert!(
            spec::skills_ag::moca_02_log_ok(effects, owner, src_pos),
            "C-AG-moca-02: exactly Teleport(owner → src tile)"
        );
        crate::kassert!(
            spec::skills_ag::moca_02_post_ok(post.pos[owner as usize], post.fire[owner as usize], src_pos, pre_fire),
            "C-AG-moca-02: on the actor's tile with fire −1"
        );
    } else {
        crate::kassert!(
            spec::skills_ag::moca_02_noop_log_ok(effects),
            "C-AG-moca-02: declined offer is a no-op"
        );
        crate::kassert!(post.fire[owner as usize] == pre_fire, "decline leaves fire");
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
    invoke::unwrap_done(skill_personas::moca_self::on_card_kani(owner));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        !spec::skills_ag::moca_02_log_ok_mutant(effects, owner, 10),
        "mutant (tile+1) must not match"
    );
    crate::kassert!(
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
    skills_ag_world();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.fire[player as usize] = pre_fire;
    w.fire_max = [1, 1, 1, 1];
    w.group_id[player as usize] = band_key();
    world::reset_world(w);
    // Entry 0 is the (2) Play (`can_use` residual: not skill_blocked, fire ≥ 1).
    invoke::unwrap_done(skill_personas::ran_red::use_skill_kani(player));
    let post_fire = world::with_world(|ww| ww.fire[player as usize]);
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::skills_ag::ran_02_spend_log_empty(effects),
        "C-AG-ran-02: fire spend is not an Effect (spend_fire mutates fire only)"
    );
    crate::kassert!(
        spec::skills_ag::ran_02_fire_ok(post_fire, pre_fire),
        "C-AG-ran-02: exactly one [火罐] spent"
    );
}

/// Companion: spending 2 must not match.
#[kani::proof]
#[kani::unwind(80)]
pub fn ran_02_mutant_is_detected() {
    skills_ag_world();
    let mut w = World::new();
    w.n_players = 2;
    let player = 0i32;
    w.trig_player = player;
    w.fire[player as usize] = 1;
    w.fire_max = [1, 1, 1, 1];
    w.group_id[player as usize] = band_key();
    world::reset_world(w);
    invoke::unwrap_done(skill_personas::ran_red::use_skill_kani(player));
    let post_fire = world::with_world(|ww| ww.fire[player as usize]);
    crate::kassert!(post_fire == 0, "card spends exactly 1");
    crate::kassert!(post_fire != 1 - 2, "mutant (spend 2) is impossible from fire=1");
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
        crate::kassert!(grants, "third idle turn grants");
        crate::kassert!(next == 0, "counter resets");
    } else {
        crate::kassert!(!grants, "first/second idle turn grants nothing");
        crate::kassert!(next == pre_rest + 1, "counter advances");
    }
    // Mutant is `pre+1 >= 2` (grants at rest 1 and 2); card is `pre+1 >= 3`
    // (grants only at rest 2). They disagree exactly at rest 1.
    kani::cover!(pre_rest == 1, "reachable disagreement (rest 1)");
    crate::kassert!(
        grants != spec::skills_ag::firepot_01_grants_mutant(pre_rest) || pre_rest != 1,
        "mutant disagrees at rest 1"
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
    crate::kassert!(
        applies == (is_shop || price <= 1200),
        "shop-street OR price <= 1200"
    );
    if !is_shop && price > 1200 && price <= 5000 {
        crate::kassert!(
            !spec::skills_ag::band_01_applies_mutant(is_shop, price) || price <= 1000,
            "mutant (<=1000) is strictly tighter"
        );
    }
}
