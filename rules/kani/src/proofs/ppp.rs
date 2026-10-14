//! Unit `ppp`: clause proofs.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. MUTANT whose disagreement is reachable.
//!
//! ## Concrete-user pattern (TRIAGE-LOG `ppcost` FLOOR bisect)
//!
//! A symbolic player index (`any_player_in`) is a BMC cliff: CBMC never
//! returns. With `n_players = 2` and a **concrete** user seat the proof
//! verifies in minutes. Iff harnesses are `*_iff_at(user)` called from two
//! `#[kani::proof]`s with `user = 0` and `user = 1` (clause is seat-symmetric
//! 「自己的」). Mutants use a single concrete user (disagreement is also
//! seat-symmetric).
//!
//! Per-harness host-op subset (`host_subset!`) covers only the ops the body
//! reaches; anything else hits `outside_subset` (loud FAIL). Direct
//! `*_kani` body calls keep CBMC's call graph at ONE body (no `call_body_idx`
//! trampoline IF-chain).

use crate::host;
use crate::harness::{self, SparsePlay};
use crate::invoke;
use crate::ir_fill::{self, TriggerOverlay};
use crate::spec;
use crate::world::{self, Effect, World, MAX_LOG, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Per-unit host-op subset (every op any ppp body reaches). Outside → loud FAIL.
// Bodies: popipa, pipopa, signpost, bang_dream, popipapapipopa, auction_pulled,
// star_beat, arisa_wait, random_star, saaya_sky, tomorrows_door, kasumi_love_all,
// returns, saaya_sky, maze, name_sky, everyone.
// ---------------------------------------------------------------------------
crate::host_subset!(ppp_dispatch {
    log | effect => arm_log,
    gain | gain_typed | gain_fixed => arm_gain,
    money => arm_money,
    pay => arm_pay,
    pay_to => arm_pay_to,
    draw => arm_draw,
    add_to_deck | add_to_deck_at => arm_add_to_deck_at,
    add_to_hand => arm_add_to_hand,
    take_card => arm_take_card,
    to_discard => arm_to_discard,
    cards_in => arm_cards_in,
    discard_from_hand => arm_discard_from_hand,
    discard_size => arm_discard_size,
    deck_count => arm_deck_count,
    shuffle_into_deck => arm_shuffle_into_deck,
    place_card_at => arm_place_card,
    place_card_on => arm_place_card_on,
    set_dest => arm_set_dest,
    teleport_to => arm_teleport_to,
    is_placed => arm_is_placed,
    self_tile => arm_self_tile,
    set_self_tile => arm_set_self_tile,
    set_self_immune => arm_set_self_immune,
    unplace_card | unplace_at | unplace_card_named => arm_plan_stub,
    add_band_crystals => arm_add_band_crystals,
    band_crystals => arm_band_crystals,
    counter_self => arm_counter_self,
    add_counter_self => arm_add_counter_self,
    set_counter_self => arm_set_counter_self,
    add_card_counter => arm_add_card_counter,
    card_counter => arm_card_counter,
    count_held | count_held_name => arm_count_held,
    add_held => arm_add_held,
    set_held_name => arm_set_held_name,
    slot => arm_slot,
    set_slot => arm_set_slot,
    inc_slot => arm_inc_slot,
    roll | roll_ask => arm_roll,
    player_pos => arm_player_pos,
    player_count => arm_player_count,
    player_out => arm_player_out,
    others_count => arm_others_count,
    others_at => arm_others_at,
    owned_count => arm_owned_count,
    owned_at => arm_owned_at,
    tile_count => arm_tile_count,
    tile_named => arm_tile_named,
    tile_owner => arm_tile_owner,
    tile_forward | dist => arm_dist,
    houses_of => arm_houses_of,
    mortgaged_of => arm_mortgaged_of,
    set_self_prop => arm_set_self_prop,
    set_tile_prop => arm_set_tile_prop,
    set_prop_at => arm_set_prop_at,
    self_prop | prop_at | tile_prop => arm_counter_self,
    character_is => arm_character_is,
    in_band => arm_in_band,
    target => arm_target,
    ask => arm_ask,
    hand_count => arm_hand_count,
    gate => arm_gate,
    raise => arm_raise,
    // Move-plan / settle / offer protocol stubs (benign 0).
    set_steps | set_reverse | set_stop_at | set_parity | set_resolve
    | set_kind | set_teleport_to | set_start | set_base_dice | set_rent_factor
    | schedule_turn_end | linger | card_offer_build | can_build_on
    | card_move | set_roller | set_signed | set_settle_tile | set_pay_factor
    | set_no_buy | set_no_build | set_build_anywhere | add_base_dice
    | add_extra_dice | set_min_roll | set_extra_steps | set_more_steps
    | set_tag | set_settle_as_agent | plan_add_follower | set_play_doubled
    | set_no_money_loss | set_fixed_roll | set_next_steps | set_build_discount
    | set_build_cost_pct | set_extreme | clear_dice | set_can_build
    | move_stopped | move_stop_at | move_parity | move_resolve | move_steps
    | move_remaining | move_total | move_dir | fixed_roll | do_move_roll
    | turn_main_steps | turn_rolls | turn_snap | turn_start_pos
    | targeted_count => arm_plan_stub,
});

/// Install the sparse 2-seat world + the ppp host subset **in place**.
/// `user` is a **concrete** seat in `{0, 1}` (see module docs).
fn ppp_world(user: i32) {
    debug_assert!(user == 0 || user == 1);
    host::install_backend(ppp_dispatch);
    harness::sparse_play_world(SparsePlay {
        used_seats: 2,
        n_players: 2,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.symbolic_answers = true;
    });
}

/// Same as [`ppp_world`] but also plants a field card (hooks with `card.placed`).
fn ppp_world_placed(user: i32, named_tile: i32, card_slot: u16) {
    ppp_world(user);
    world::with_world_mut(|w| {
        w.trig_tile = named_tile;
        w.field_n = 1;
        w.field[0] = world::FieldCard {
            uid: 1,
            card: card_slot,
            player: user,
            tile: named_tile,
            face_down: false,
            immune: false,
            crystals: 0,
            props: world::NameMap::new(),
        };
    });
}

fn log_slice() -> ([Effect; MAX_LOG], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn effects_of(logbuf: &[Effect; MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

/// IFF: exactly GainMoney(1000) then Draw(1); deck +1 Pipopa; dest Banished.
/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn popipa_01_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_money = w.money[player as usize];
    let pre_deck_pipopa = spec::ppp::deck_count_named(&w, spec::ppp::CARD_PIPOPA);
    let pre_hand_n = w.hand_n as i32;
    kani::cover!(pre_deck_pipopa == 0, "no pre-existing Pipopa in deck");
    kani::cover!(pre_deck_pipopa >= 1, "deck already holds Pipopa");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::popipa::popipa_kani(user));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let dest = world::with_world(|ww| ww.dest);
    let deck_pipopa = world::with_world(|ww| spec::ppp::deck_count_named(ww, spec::ppp::CARD_PIPOPA));
    let hand_n = world::with_world(|ww| ww.hand_n as i32);

    crate::kassert!(
        spec::ppp::popipa_effects_ok(effects_of(&logbuf, log_n), player),
        "popipa: effect log must be exactly GainMoney(1000), Draw(1)"
    );
    crate::kassert!(
        spec::ppp::popipa_post_ok(
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
    crate::kassert!(
        spec::ppp::frame_others_money(&pre, &post, player),
        "popipa: other players' money is frame"
    );
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn popipa_01_iff() {
    popipa_01_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn popipa_01_user1_iff() {
    popipa_01_iff_at(1);
}


/// MUTANT: reference wants +1100. Card must not match.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn popipa_01_mutant_money_off_by_100() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::popipa::popipa_kani(user));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        spec::ppp::popipa_effects_ok_mutant(effects_of(&logbuf, log_n), player),
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

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn pipopa_01_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_money = w.money[player as usize];
    let pre_field_n = w.field_n;
    kani::cover!(pre_field_n == 0, "empty field before play");

    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::pipopa::pipopa_kani(user));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let dest = world::with_world(|ww| ww.dest);
    let field_has_new = world::with_world(|ww| ww.field_n > pre_field_n);

    crate::kassert!(
        spec::ppp::pipopa_effects_ok(effects_of(&logbuf, log_n), player, POPIPAPAPIPOPA_SLOT),
        "pipopa: log must be exactly GainMoney(1000), PlaceCard(Popipapapipopa, Field)"
    );
    crate::kassert!(
        spec::ppp::pipopa_post_ok(pre_money, post.money[player as usize], dest, field_has_new),
        "pipopa: money+1000, field+1, dest Banished"
    );
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn pipopa_01_iff() {
    pipopa_01_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn pipopa_01_user1_iff() {
    pipopa_01_iff_at(1);
}


/// MUTANT: reference wants a 2000 gain.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn pipopa_01_mutant_money_2000() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::pipopa::pipopa_kani(user));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        spec::ppp::pipopa_effects_ok_mutant(effects_of(&logbuf, log_n), player, POPIPAPAPIPOPA_SLOT),
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

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn signpost_02_lose_1000_at_end_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_money = w.money[player as usize];

    // single-source guard: the real CEL string on `On::AtEnd`
    let gid = crate::guard_id!("owner.out == 0");
    let ov = TriggerOverlay::default();
    let ctx = ir_fill::fill_for_guard(&w, gid, player, 0, false, -1, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (owner alive)");
    kani::cover!(!admitted, "guard rejects (owner out)");
    if !admitted {
        // host would not call the body; world untouched
        let (logbuf, log_n) = log_slice();
        crate::kassert!(log_n == 0, "guard reject must be a no-op");
        let _ = logbuf;
        return;
    }

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::signpost::at_end_kani(user));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        spec::ppp::signpost_02_effects_ok(effects_of(&logbuf, log_n), player),
        "signpost-02: log must be exactly PayMoney(1000)"
    );
    // Book: 「失去]1000. Host pay clamps at 0; assert the amount and the
    // unclamped case, cover the clamp.
    if pre_money >= 1000 {
        crate::kassert!(
            spec::ppp::signpost_02_post_ok(pre_money, post.money[player as usize]),
            "signpost-02: money falls by exactly 1000"
        );
    } else {
        crate::kassert!(
            post.money[player as usize] == 0,
            "signpost-02: host pay clamps at 0 when money < 1000"
        );
    }
    kani::cover!(pre_money == 1000, "money exactly 1000");
    kani::cover!(pre_money > 1000, "money above 1000");
    kani::cover!(pre_money < 1000, "money below 1000 (host clamp)");
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(24)]
pub fn signpost_02_lose_1000_at_end_iff() {
    signpost_02_lose_1000_at_end_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(24)]
pub fn signpost_02_lose_1000_at_end_user1_iff() {
    signpost_02_lose_1000_at_end_iff_at(1);
}


/// MUTANT: reference wants a 500 loss.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn signpost_02_mutant_lose_500() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_money = w.money[player as usize];
    kani::assume(pre_money >= 1000);
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::signpost::at_end_kani(user));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        spec::ppp::signpost_02_effects_ok_mutant(effects_of(&logbuf, log_n), player),
        "mutant: card must not match the 500-loss spec"
    );
}

// ===========================================================================
// C-PPP-bang-dream-01
// 「为[使用者]的团卡添加一个[奇迹水晶]」
// Isolation: `band_crystals[p]` is the 团卡 (band-skill instance) crystal
// count under the Kani host. Host `add_band_crystals` logs no typed Effect
// (post-state only); the optional teleport of (02) may appear in the log and
// is frame for this leg.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn bang_dream_01_crystal_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_crystals = w.band_crystals[player as usize];
    // The book's condition is 「user has a 团卡」. The model gives every
    // player a band_crystals counter (the bound band-skill instance), so the
    // condition is the model's standing fact — not assumed away, just true.
    kani::cover!(pre_crystals == 0, "band crystals start at 0");
    kani::cover!(pre_crystals > 0, "band crystals already banked");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::bang_dream::bang_dream_kani(user));

    let post_crystals = world::with_world(|ww| ww.band_crystals[player as usize]);
    crate::kassert!(
        post_crystals == spec::ppp::bang_dream_01_crystals_after(pre_crystals),
        "bang-dream-01: band crystals rise by exactly 1"
    );
    // Frame on this leg: money / pos / fire untouched *by the crystal add*.
    // The (02) teleport leg may move `pos`; that is not this clause's effect.
    let post = world::snapshot();
    crate::kassert!(world::arr4_eq(post.money, pre.money), "bang-dream-01: money is frame");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "bang-dream-01: fire is frame");
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn bang_dream_01_crystal_iff() {
    bang_dream_01_crystal_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn bang_dream_01_crystal_user1_iff() {
    bang_dream_01_crystal_iff_at(1);
}


/// MUTANT: reference wants +2 crystals. Card must not match.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn bang_dream_01_mutant_plus_2() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_crystals = w.band_crystals[player as usize];
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::bang_dream::bang_dream_kani(user));
    let post_crystals = world::with_world(|ww| ww.band_crystals[player as usize]);
    crate::kassert!(
        post_crystals == pre_crystals + 2,
        "mutant: card must not match the +2 spec"
    );
}

// ===========================================================================
// C-PPP-bang-dream-02
// 「然后[传送]至任意[使用者]拥有的格子且可选择盖房」
// Condition: the user owns at least one tile. Isolation: force at least one
// owned tile so the pick prompt is reached. `w.symbolic_answers = true`
// explores every owned pick. `card_offer_build` is a PROTOCOL stub (the
// house is optional and is not asserted).
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn bang_dream_02_teleport_owned_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    // Force one owned tile so `owned_tiles` is non-empty and the prompt runs.
    // World index T; host `owned_at` reports T+1 (1-based board index).
    let t: i32 = kani::any();
    kani::assume((0..60).contains(&t));
    let mut w = w;
    w.tiles[t as usize].owner = player;

    let pre_pos = w.pos[player as usize];
    // Collect the host's owned-tile indices (what `ask_tile` offers).
    let mut owned: [i32; 8] = [0; 8];
    let mut n_owned = 0usize;
    let mut i = 0i32;
    while i < 60 {
        if w.tiles[i as usize].owner == player && n_owned < 8 {
            // host `owned_at` returns the 1-based board index
            owned[n_owned] = i + 1;
            n_owned += 1;
        }
        i += 1;
    }
    kani::cover!(n_owned == 1, "exactly one owned tile");
    kani::cover!(n_owned >= 2, "several owned tiles (free pick)");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::bang_dream::bang_dream_kani(user));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    // Leg (01) logs nothing typed; leg (02) logs exactly one Teleport.
    crate::kassert!(
        spec::ppp::bang_dream_02_log_ok(effects_of(&logbuf, log_n), player),
        "bang-dream-02: log must be exactly Teleport to the pick"
    );
    crate::kassert!(
        spec::ppp::bang_dream_02_pos_is_owned(post.pos[player as usize], &owned, n_owned),
        "bang-dream-02: pos must be one of the owned tiles"
    );
    kani::cover!(
        post.pos[player as usize] != pre_pos,
        "teleport moved the player"
    );
    kani::cover!(
        post.pos[player as usize] == pre_pos,
        "teleport picked the current tile (legal 「任意」)"
    );
    // Frame: other players unmoved; money / fire untouched.
    for p in 0..MAX_PLAYERS {
        if p != player as usize {
            crate::kassert!(post.pos[p] == pre.pos[p], "bang-dream-02: others unmoved");
        }
    }
    crate::kassert!(world::arr4_eq(post.money, pre.money), "bang-dream-02: money is frame");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "bang-dream-02: fire is frame");
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn bang_dream_02_teleport_owned_iff() {
    bang_dream_02_teleport_owned_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn bang_dream_02_teleport_owned_user1_iff() {
    bang_dream_02_teleport_owned_iff_at(1);
}


/// MUTANT: reference wants an extra GainMoney(1) on the teleport leg.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn bang_dream_02_mutant_extra_money() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let t: i32 = kani::any();
    kani::assume((0..60).contains(&t));
    let mut w = w;
    w.tiles[t as usize].owner = player;
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::bang_dream::bang_dream_kani(user));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        spec::ppp::bang_dream_02_log_ok_mutant(effects_of(&logbuf, log_n), player),
        "mutant: card must not match the extra-money spec"
    );
}

// ===========================================================================
// C-PPP-popipapapipopa-c1
// 「此卡拥有者每次[经过]…时为此卡添加1个[奇迹水晶]（上限10个）」
// Guard (single-source): `card.placed && actor == owner && (tile.id ==
// tile_named('东京外') || … || tile_named('流星堂'))`.
// Isolation: pre-crystals < 10 so the host's missing max-clamp cannot
// inflate the count past the book's cap (the cap itself is BLOCKED-HOST:
// `Op::add_counter_*` ignores its `max` argument).
// ===========================================================================

const CRYSTALS_KEY: &str = "crystals";

fn popipa_c1_world(user: i32, named_tile: i32) -> World {
    ppp_world_placed(user, named_tile, 2);
    // Under-cap isolation (host max is ignored; see module note).
    let pre: i32 = kani::any();
    kani::assume((0..10).contains(&pre));
    world::with_world_mut(|w| {
        w.key_set(user, crate::intern::intern(CRYSTALS_KEY), pre);
    });
    world::with_world(|w| w.clone())
}

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn popipapapipopa_c1_crystal_on_pass_iff_at(user: i32) {
    let user = user;

    // One of the five named tiles (the guard's disjunction is symbolic via
    // trig_tile; pick a representative that the IR `tile_named` resolves).
    let named: i32 = kani::any();
    kani::assume(named == 23 || named == 30 || named == 48 || named == 44 || named == 45);
    let w = popipa_c1_world(user, named);
    let player = w.trig_player;
    let key = crate::intern::intern(CRYSTALS_KEY);
    let pre_crystals = w.key_get(player, key);

    // single-source guard: the real CEL string on the PassTile hook
    let gid = crate::guard_id!(
        "card.placed && actor == owner && (tile.id == tile_named('东京外') || tile.id == tile_named('江户川乐器店') || tile.id == tile_named('山吹面包房') || tile.id == tile_named('星之鼓动山丘') || tile.id == tile_named('流星堂'))"
    );
    let ov = ir_fill::TriggerOverlay::default();
    let ctx = ir_fill::fill_for_guard(&w, gid, player, 2, true, named as i64, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (named pass)");
    kani::cover!(!admitted, "guard rejects");
    if !admitted {
        return;
    }

    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::popipapapipopa::pass_tile_kani(user));

    let post_crystals = world::with_world(|ww| ww.key_get(player, key));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        post_crystals == spec::ppp::popipapapipopa_c1_crystals_after(pre_crystals),
        "popipapapipopa-c1: crystals rise by exactly 1 under the cap"
    );
    crate::kassert!(
        spec::ppp::popipapapipopa_c1_log_ok(effects_of(&logbuf, log_n), player, key.0, pre_crystals),
        "popipapapipopa-c1: log is exactly AddCounter(+1) under the cap"
    );
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn popipapapipopa_c1_crystal_on_pass_iff() {
    popipapapipopa_c1_crystal_on_pass_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn popipapapipopa_c1_crystal_on_pass_user1_iff() {
    popipapapipopa_c1_crystal_on_pass_iff_at(1);
}


/// MUTANT: reference wants +2 crystals per pass.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn popipapapipopa_c1_mutant_plus_2() {
    let user = 0i32;
    let named: i32 = kani::any();
    kani::assume(named == 23 || named == 30 || named == 48 || named == 44 || named == 45);
    let w = popipa_c1_world(user, named);
    let player = w.trig_player;
    let key = crate::intern::intern(CRYSTALS_KEY);
    let pre_crystals = w.key_get(player, key);
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::popipapapipopa::pass_tile_kani(user));
    let post_crystals = world::with_world(|ww| ww.key_get(player, key));
    crate::kassert!(
        post_crystals == pre_crystals + 2,
        "mutant: card must not match the +2 spec"
    );
}

// ===========================================================================
// C-PPP-popipapapipopa-c2
// 「消耗或支付时可使用此卡多个奇迹水晶，每个将资金变动减少150（最少0）。」
// Guard: `card.placed && actor == owner && value > 0 && card.counter('crystals') > 0`.
// `w.symbolic_answers = true` explores every k in 0..=max.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn popipapapipopa_c2_pay_reduce_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let key = crate::intern::intern(CRYSTALS_KEY);
    // Placed card with a symbolic crystal count in 1..=8 and a positive spend.
    let pre_crystals: i32 = kani::any();
    kani::assume((1..=8).contains(&pre_crystals));
    let amount: i32 = kani::any();
    kani::assume((1..=2000).contains(&amount));
    let mut w = w;
    w.trig_tile = -1;
    w.trig_value = amount;
    w.pay_amount = amount;
    w.field_n = 1;
    w.field[0] = world::FieldCard {
        uid: 1,
        card: 2,
        player,
        tile: -1,
        face_down: false,
        immune: false,
        crystals: pre_crystals,
        props: world::NameMap::new(),
    };
    w.key_set(player, key, pre_crystals);

    // single-source guard
    let gid = crate::guard_id!(
        "card.placed && actor == owner && value > 0 && card.counter('crystals') > 0"
    );
    let mut ov = ir_fill::TriggerOverlay::default();
    ov.value = Some(amount as i64);
    ov.push_card_counter(CRYSTALS_KEY, pre_crystals as i64);
    let ctx = ir_fill::fill_for_guard(&w, gid, player, 2, true, -1, &ov);
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits");
    if !admitted {
        // value > 0 ∧ crystals > 0 is forced above; a reject is a translator bug
        panic!("guard rejected an admitted window");
    }

    let pre_amount = w.pay_amount;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::popipapapipopa::pay_choose_kani(user));

    let post_crystals = world::with_world(|ww| ww.key_get(player, key));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    let (logbuf, log_n) = log_slice();
    let k = pre_crystals - post_crystals;
    let expected_to = spec::ppp::popipapapipopa_c2_amount_after(pre_amount, k);
    kani::cover!(k == 0, "decline (k=0)");
    kani::cover!(k > 0, "spend k>0");
    kani::cover!(
        k > 0 && pre_amount - 150 * k <= 0,
        "cut reaches the floor at 0"
    );
    crate::kassert!(
        post_amount == expected_to,
        "popipapapipopa-c2: pay_amount is max(pre-150k, 0)"
    );
    crate::kassert!(
        spec::ppp::popipapapipopa_c2_log_ok(
            effects_of(&logbuf, log_n),
            player,
            key.0,
            k,
            expected_to
        ),
        "popipapapipopa-c2: log is exactly crystals-k + SetPayAmount"
    );
    crate::kassert!(k >= 0 && k <= pre_crystals, "popipapapipopa-c2: k in range");
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn popipapapipopa_c2_pay_reduce_iff() {
    popipapapipopa_c2_pay_reduce_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(40)]
pub fn popipapapipopa_c2_pay_reduce_user1_iff() {
    popipapapipopa_c2_pay_reduce_iff_at(1);
}


/// MUTANT: reference floors the cut at 1 instead of 0.
#[kani::proof]
#[kani::unwind(40)]
#[kani::should_panic]
pub fn popipapapipopa_c2_mutant_floor_1() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let key = crate::intern::intern(CRYSTALS_KEY);
    let pre_crystals: i32 = kani::any();
    kani::assume((1..=8).contains(&pre_crystals));
    let amount: i32 = kani::any();
    // Force a cut that reaches 0 under the book (amount < 150*k).
    kani::assume((1..=149).contains(&amount));
    let mut w = w;
    w.trig_value = amount;
    w.pay_amount = amount;
    w.field_n = 1;
    w.field[0] = world::FieldCard {
        uid: 1,
        card: 2,
        player,
        tile: -1,
        face_down: false,
        immune: false,
        crystals: pre_crystals,
        props: world::NameMap::new(),
    };
    w.key_set(player, key, pre_crystals);
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::popipapapipopa::pay_choose_kani(user));
    let post_amount = world::with_world(|ww| ww.pay_amount);
    let post_crystals = world::with_world(|ww| ww.key_get(player, key));
    let k = pre_crystals - post_crystals;
    crate::kassert!(
        post_amount == spec::ppp::popipapapipopa_c2_amount_after_mutant(amount, k),
        "mutant: card must not match the floor-1 spec"
    );
}

// ===========================================================================
// C-PPP-door-01
// 「此卡指定的序列（从前往后）为“流星堂”，…，“大阪中之岛公园」」
// Pure const: the ordered sequence is exactly those 10 named tiles.
// ===========================================================================

#[kani::proof]
#[kani::unwind(40)]
pub fn door_01_sequence_const() {
    crate::kassert!(
        spec::ppp::DOOR_SEQUENCE.len() == 10,
        "door-01: sequence has exactly 10 entries"
    );
    crate::kassert!(
        spec::ppp::door_sequence_names_on_board(),
        "door-01: every printed name sits at its board slot"
    );
    // Mutant shape: a 9-entry reading is distinguishable.
    crate::kassert!(
        spec::ppp::DOOR_SEQUENCE_MUTANT_LEN != spec::ppp::DOOR_SEQUENCE.len(),
        "door-01: the 9-entry mutant disagrees with the book"
    );
}

// ===========================================================================
// C-PPP-auction-off-01 / c1
// 「此卡加入手牌时将此卡放置在自身场上」
// 「此卡拥有者火罐上限加1」
// BLOCKED-HOST: the Drawn body gates on `trigger::card_is(ID)`, and the Kani
// host arm `Op::trig_card_is` is a hard `0` (never identifies the drawn card).
// The positive path is unreachable; the no-op path (identity check fails ⇒
// body is a no-op) is what the model can show.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn auction_off_01_blocked_noop() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    // Hand holds the auction card so `take_from_hand` would succeed if the
    // identity gate admitted.
    let mut w = w;
    w.hand_n = 1;
    w.hand_pile[0] = crate::intern::intern(spec::ppp::CARD_AUCTION_OFF).0;
    let pre_field_n = w.field_n;
    let pre_fire_max = w.fire_max[player as usize];

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    // Host `trig_card_is` is a hard 0, so the Drawn body must early-return.
    invoke::unwrap_done(card_ppp::auction_pulled::hook_kani(user));

    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        log_n == 0,
        "auction-off-01 (blocked): identity gate fails ⇒ no effects"
    );
    let _ = logbuf;
    let post_field_n = world::with_world(|ww| ww.field_n);
    let post_fire_max = world::with_world(|ww| ww.fire_max[player as usize]);
    crate::kassert!(
        post_field_n == pre_field_n && post_fire_max == pre_fire_max,
        "auction-off-01 (blocked): field / fire_max untouched"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "auction-off-01 (blocked): money frame");
}

// ===========================================================================
// C-PPP-star-beat-01
// 「[使用者]获得1层状态“下次结算后可选择在绝对距离5格以内自己拥有的格子上
//   进行一次盖房，随后减少1层”」
// The [手] body grants the layer via `inc_slot("star_beat.layers", 1)` and
// then continues into the move-stage choice (clauses 02/03/04). This harness
// pins the layer grant; the rest of the body is frame for this clause.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn star_beat_01_layer_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let key = crate::intern::intern(spec::ppp::STAR_BEAT_LAYERS_KEY);
    let pre_layers = w.key_get(player, key);
    // Room for the pick prompt's two options.
    kani::cover!(pre_layers == 0, "layers start at 0");

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::star_beat::play_kani(user));

    let post_layers = world::with_world(|ww| ww.key_get(player, key));
    crate::kassert!(
        post_layers == spec::ppp::star_beat_01_layers_after(pre_layers),
        "star-beat-01: layers rise by exactly 1"
    );
    // Money is not moved by the layer grant itself (option 1 may grant
    // stickers; option 2 may walk — both are other clauses).
    let post = world::snapshot();
    crate::kassert!(world::arr4_eq(post.money, pre.money), "star-beat-01: money is frame");
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn star_beat_01_layer_iff() {
    star_beat_01_layer_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(40)]
pub fn star_beat_01_layer_user1_iff() {
    star_beat_01_layer_iff_at(1);
}


/// MUTANT: reference wants +2 layers.
#[kani::proof]
#[kani::unwind(40)]
#[kani::should_panic]
pub fn star_beat_01_mutant_plus_2() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let key = crate::intern::intern(spec::ppp::STAR_BEAT_LAYERS_KEY);
    let pre_layers = w.key_get(player, key);
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::star_beat::play_kani(user));
    let post_layers = world::with_world(|ww| ww.key_get(player, key));
    crate::kassert!(
        post_layers == spec::ppp::star_beat_01_layers_after_mutant(pre_layers),
        "mutant: card must not match the +2 layers spec"
    );
}

// ===========================================================================
// C-PPP-wait-aoki-01
// 「将此卡放置在[使用者]的[场地]。」
// Isolation: the exclusive-character gate is a play-legality check outside
// the body; the body itself is unconditional placement.
// ===========================================================================

/// Host maps unknown PPP names to slot 2 under the Kani intern facade.
const PPP_SLOT: u8 = 2;

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn wait_aoki_01_place_on_field_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let pre_field_n = w.field_n;
    kani::cover!(pre_field_n == 0, "empty field before play");

    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::arisa_wait::arisa_wait_kani(user));

    let (logbuf, log_n) = log_slice();
    let dest = world::with_world(|ww| ww.dest);
    let field_n = world::with_world(|ww| ww.field_n);
    crate::kassert!(
        spec::ppp::wait_aoki_01_effects_ok(effects_of(&logbuf, log_n), player, PPP_SLOT),
        "wait-aoki-01: log must be exactly PlaceCard(self, Field)"
    );
    crate::kassert!(
        dest == spec::ppp::DEST_FIELD && field_n as i32 == pre_field_n as i32 + 1,
        "wait-aoki-01: card sits on the user's field"
    );
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn wait_aoki_01_place_on_field_iff() {
    wait_aoki_01_place_on_field_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn wait_aoki_01_place_on_field_user1_iff() {
    wait_aoki_01_place_on_field_iff_at(1);
}


/// MUTANT: reference wants the card banished instead of placed.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn wait_aoki_01_mutant_banished() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::arisa_wait::arisa_wait_kani(user));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        spec::ppp::wait_aoki_01_effects_ok_mutant(effects_of(&logbuf, log_n), player, PPP_SLOT),
        "mutant: card must not match the banished spec"
    );
}

// ===========================================================================
// C-PPP-random-star-01
// 「将此卡放置自身场上并将弃牌堆和手牌洗入卡组，然后将一张“拍卖撤下来了”
//   放置在卡组底端」
// Proven here: (a) PlaceCard on field; (c) one auction card at the deck
// bottom (last element of the top-first pile).
// BLOCKED-HOST: (b) `Op::shuffle_into_deck` is a silent 0 — hand/discard are
// NOT actually swept into the deck under the Kani host.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn random_star_01_place_and_deck_bottom_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let auction_id = crate::intern::intern(spec::ppp::CARD_AUCTION_OFF_NAME).0;
    // Leave room on the deck for the bottom insert.
    let mut w = w;
    w.deck_n = 0;
    let pre_field_n = w.field_n;

    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::random_star::random_star_kani(user));

    let (logbuf, log_n) = log_slice();
    let dest = world::with_world(|ww| ww.dest);
    let field_n = world::with_world(|ww| ww.field_n);
    let deck_n = world::with_world(|ww| ww.deck_n as i32);
    let last = world::with_world(|ww| {
        if ww.deck_n == 0 {
            u16::MAX
        } else {
            ww.deck_pile[ww.deck_n as usize - 1]
        }
    });
    crate::kassert!(
        spec::ppp::random_star_01_effects_ok(effects_of(&logbuf, log_n), player, PPP_SLOT, PPP_SLOT),
        "random-star-01: log must be exactly PlaceCard(self, Field)"
    );
    crate::kassert!(
        dest == spec::ppp::DEST_FIELD && field_n as i32 == pre_field_n as i32 + 1,
        "random-star-01: card sits on the owner's field"
    );
    crate::kassert!(
        spec::ppp::random_star_01_deck_bottom_ok(deck_n, last, auction_id),
        "random-star-01: one auction card at the deck bottom"
    );
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn random_star_01_place_and_deck_bottom_iff() {
    random_star_01_place_and_deck_bottom_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn random_star_01_place_and_deck_bottom_user1_iff() {
    random_star_01_place_and_deck_bottom_iff_at(1);
}


/// MUTANT: reference wants two auction cards at the bottom.
#[kani::proof]
#[kani::unwind(32)]
#[kani::should_panic]
pub fn random_star_01_mutant_two_auctions() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let auction_id = crate::intern::intern(spec::ppp::CARD_AUCTION_OFF_NAME).0;
    let mut w = w;
    w.deck_n = 0;
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::random_star::random_star_kani(user));
    let deck_n = world::with_world(|ww| ww.deck_n as i32);
    let last = world::with_world(|ww| {
        if ww.deck_n == 0 {
            u16::MAX
        } else {
            ww.deck_pile[ww.deck_n as usize - 1]
        }
    });
    let last_two_equal = world::with_world(|ww| {
        ww.deck_n >= 2 && ww.deck_pile[ww.deck_n as usize - 1] == ww.deck_pile[ww.deck_n as usize - 2]
    });
    crate::kassert!(
        spec::ppp::random_star_01_deck_bottom_ok_mutant(deck_n, last_two_equal, auction_id, last),
        "mutant: card must not match the two-auction spec"
    );
}

// ===========================================================================
// C-PPP-star-beat-03 (sticker leg)
// 「1. 获得2个星星贴纸，…」 / option 2 grants no stickers.
// The walk itself is PROTOCOL-LEVEL (`card_move` / plan stubs return 0).
// Isolation: `w.symbolic_answers` explores both options.
// ===========================================================================

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
fn star_beat_03_stickers_iff_at(user: i32) {
    let user = user;

    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let tok = crate::intern::intern(spec::ppp::TOK_STAR_STICKER);
    let pre_tok = w.key_get(player, tok);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(card_ppp::star_beat::play_kani(user));

    let post_tok = world::with_world(|ww| ww.key_get(player, tok));
    let (logbuf, log_n) = log_slice();
    let delta = post_tok - pre_tok;
    // Option 1 (teleport) grants exactly +2 stickers; option 2 grants none.
    let picked_teleport = delta == 2;
    kani::cover!(picked_teleport, "option 1: teleport + 2 stickers");
    kani::cover!(delta == 0, "option 2: walk, no stickers");
    crate::kassert!(
        delta == 0 || delta == 2,
        "star-beat-03: sticker delta is 0 or 2"
    );
    crate::kassert!(
        spec::ppp::star_beat_03_stickers_ok(
            effects_of(&logbuf, log_n),
            player,
            tok.0,
            picked_teleport
        ),
        "star-beat-03: AddToken(+2) iff option 1 was picked"
    );
    let post = world::snapshot();
    crate::kassert!(world::arr4_eq(post.money, pre.money), "star-beat-03: money is frame");
}
/// C-… iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn star_beat_03_stickers_iff() {
    star_beat_03_stickers_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(40)]
pub fn star_beat_03_stickers_user1_iff() {
    star_beat_03_stickers_iff_at(1);
}


/// MUTANT: reference wants +1 sticker on option 1.
#[kani::proof]
#[kani::unwind(40)]
#[kani::should_panic]
pub fn star_beat_03_mutant_one_sticker() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let tok = crate::intern::intern(spec::ppp::TOK_STAR_STICKER);
    let pre_tok = w.key_get(player, tok);
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::star_beat::play_kani(user));
    let post_tok = world::with_world(|ww| ww.key_get(player, tok));
    // Force the teleport branch so the book wants +2; the mutant wants +1.
    // With symbolic picks this is a reachable-disagreement: whenever option 1
    // is taken the +1 spec disagrees. When option 2 is taken both say 0, so
    // this harness relies on `should_panic` firing on the option-1 path.
    crate::kassert!(
        post_tok - pre_tok == 1,
        "mutant: card must not match the +1 sticker spec"
    );
}

// ===========================================================================
// C-PPP-returns-c4
// 「[拥有者]的通用卡的[手]效果全部生效后获得1个星星贴纸。」
// BLOCKED-HOST: the residual `on_played_is_general` reads `trigger::cards()`,
// and `Op::trig_cards` is a hard 0 under the Kani host. The body itself
// (`add_tok(+1)`) is correct; the engine integration finding LESS (ppp.md)
// is a separate seam. This harness shows the residual rejects ⇒ no sticker.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn returns_c4_blocked_residual_noop() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let tok = crate::intern::intern(spec::ppp::TOK_STAR_STICKER);
    let pre_tok = w.key_get(player, tok);

    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };

    // CardPlayed hook with the residual gate. Host `trig_cards` is 0, so the
    // residual `on_played_is_general` is false and the body must not run.
    invoke::unwrap_done(card_ppp::returns::on_played_kani(user));

    let post_tok = world::with_world(|ww| ww.key_get(player, tok));
    let (logbuf, log_n) = log_slice();
    crate::kassert!(
        post_tok == pre_tok,
        "returns-c4 (blocked): residual rejects ⇒ no sticker"
    );
    let _ = logbuf;
    crate::kassert!(log_n == 0, "returns-c4 (blocked): empty effect log");
}

// ===========================================================================
// C-PPP-maze-01
// 「位于“流星堂”前后5格内时，可打出此卡」
// Pure distance predicate. The body's `tile_named('流星堂')` is -1 under the
// Kani host (BLOCKED-HOST), so the in-body range gate is skipped; this
// harness pins the book's ≤5 reading.
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn maze_01_distance_predicate() {
    let pos: i32 = kani::any();
    kani::assume((0..60).contains(&pos));
    let d = spec::ppp::abs_dist(pos, spec::ppp::TILE_RYUSEIDO);
    kani::cover!(d == 0, "on 流星堂");
    kani::cover!(d == 5, "exactly 5 away (inside)");
    kani::cover!(d == 6, "6 away (outside)");
    crate::kassert!(
        spec::ppp::maze_01_playable(pos) == (d <= 5),
        "maze-01: playable iff absolute distance ≤ 5"
    );
    // Mutant shape: a ≤4 reading is distinguishable at d==5.
    crate::kassert!(
        !(d == 5 && spec::ppp::maze_01_playable(pos) == (d <= 4)),
        "maze-01: the ≤4 mutant disagrees at distance 5"
    );
}

// ===========================================================================
// C-PPP-name-sky-01 / C-PPP-everyone-01
// 「将此卡放置在山吹面包房」 / 「将此卡放置在“星之鼓动山丘”上。」
// BLOCKED-HOST: `ctx::tile_named` returns -1 for every name except
// 江户川乐器店, so `place_card_on` lands on tile -1 rather than the printed
// slot. The body is correct; the host name table is incomplete.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn name_sky_01_blocked_tile_named() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(card_ppp::saaya_sky::play_kani(user));
    let tile = world::with_world(|ww| {
        if ww.field_n == 0 {
            -999
        } else {
            ww.field[ww.field_n as usize - 1].tile
        }
    });
    // Book wants 山吹面包房 (slot 48). Host `tile_named` yields -1.
    crate::kassert!(
        tile == spec::ppp::TILE_YAMABUKI,
        "name-sky-01: card must sit on 山吹面包房 (slot 48)"
    );
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn name_sky_01_mutant_hill() {
    // Mutant: reference wants the card on 星之鼓动山丘 instead.
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    world::reset_world(w);
    invoke::unwrap_done(card_ppp::saaya_sky::play_kani(user));
    let tile = world::with_world(|ww| {
        if ww.field_n == 0 {
            -999
        } else {
            ww.field[ww.field_n as usize - 1].tile
        }
    });
    crate::kassert!(
        tile == spec::ppp::TILE_HILL,
        "mutant: card must not match the hill spec"
    );
}

#[kani::proof]
#[kani::unwind(24)]
pub fn everyone_01_blocked_tile_named() {
    let user = 0i32;
    ppp_world(user);
    let w = world::with_world(|ww| ww.clone());
    let player = user;
    let _pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(card_ppp::kasumi_love_all::play_kani(user));
    let tile = world::with_world(|ww| {
        if ww.field_n == 0 {
            -999
        } else {
            ww.field[ww.field_n as usize - 1].tile
        }
    });
    // Book wants 星之鼓动山丘 (slot 44). Host `tile_named` yields -1.
    crate::kassert!(
        tile == spec::ppp::TILE_HILL,
        "everyone-01: card must sit on 星之鼓动山丘 (slot 44)"
    );
}