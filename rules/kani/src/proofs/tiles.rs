//! Unit `tiles`: clause proofs.
//!
//! Per clause (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. mutant whose disagreement is reachable.
//!
//! ## Classification gate (see `docs/rulebook/spec/kani/tiles.md`)
//!
//! | body | status under the current host model |
//! |---|---|
//! | `tile:edogawa` settle | **provable** — `ctx::draw(player, 1)` is modeled |
//! | `tile:circle` settle | **provable** — same draw |
//! | `tile:event` settle | **partial** — hand draw modeled; `draw_event` is a protocol stub |
//! | `tile:property` / `tile:ring` | **BLOCKED-HOST** `trig_move_main` (unmodeled `Op`) |
//! | `tile:agent` | **BLOCKED-HOST** `is_color` (protocol stub ⇒ 同色 set always empty) |
//! | `tile:circle` [经过] reward | **BLOCKED-HOST** `trig_move_from` / `trig_move_resolve` / `trig_move_remaining` |
//!
//! Blocked harnesses are **not** written as vacuous proofs. Specs for those
//! clauses live in `spec/tiles.rs` so they flip on when the host lands.

use crate::invoke;
use crate::spec::tiles as spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Symbolic worlds
// ---------------------------------------------------------------------------

/// Minimal world for a pure settle-draw body (edogawa / circle / event).
/// Isolation: empty field so `self_tile` falls back to `pos[player]`; no
/// standing cards, no marks, no events.
fn symbolic_settle_world(player: i32) -> World {
    crate::host::install();
    let mut w = World::new();
    w.n_players = 2;
    w.trig_player = player;
    w.pos[player as usize] = 1; // any board index; only used for logging
    w.money[player as usize] = kani::any();
    kani::assume(w.money[player as usize] >= 0 && w.money[player as usize] <= 10_000);
    w.hand_n = kani::any();
    kani::assume((w.hand_n as i32) <= 6); // leave room for the draw
    w
}


/// Call the unmodified `On::Settle` body of a tile rule via its `CardDef`,
/// skipping `invoke::find`'s linear scan of the whole registry (that scan is
/// an unwind cliff under CBMC). The function pointer is the same unmodified
/// guest body.
fn call_settle(def: &card_sdk::CardDef, player: i32) {
    for on in def.on.iter() {
        if let card_sdk::On::Settle(_, _, f) = on {
            invoke::unwrap_done(f(player));
            return;
        }
    }
    panic!("no On::Settle on {}", def.id);
}

fn any_seat() -> i32 {
    let p: i32 = kani::any();
    kani::assume(p >= 0 && p < 2);
    p
}

fn effects() -> ([Effect; world::MAX_LOG], usize) {
    let logbuf: [Effect; world::MAX_LOG] = world::with_world(|w| w.log);
    let n = world::with_world(|w| w.log_n);
    (logbuf, n)
}

// ===========================================================================
// G-SETTLE-02 — `tile:edogawa`
// 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡。」
// ===========================================================================

/// IFF: settling 江户川乐器店 draws exactly one hand card for the settler.
#[kani::proof]
#[kani::unwind(48)]
pub fn edogawa_settle_draw_one_iff() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    let pre_hand_n = w.hand_n;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    call_settle(&rules_tiles::edogawa::EDOGAWA, player);

    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    kani::cover!(log_n == 1, "exactly one effect");
    crate::kassert!(
        spec::g_settle_02_draw_one_ok(&effects, player),
        "G-SETTLE-02 edogawa: effect log must be exactly one Draw{player,1}"
    );
    // Post: one more card in the hand pile.
    let post_hand_n = world::with_world(|w| w.hand_n);
    crate::kassert!(
        post_hand_n as i32 == pre_hand_n as i32 + 1,
        "G-SETTLE-02 edogawa: hand count must rise by 1"
    );
    // Frame: money / pos / fire / stay untouched (draw is the only write).
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
}

/// MUTANT: a wrong spec that wants two cards. Must be distinguishable.
#[kani::proof]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn edogawa_settle_mutant_two_cards() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    world::reset_world(w);
    call_settle(&rules_tiles::edogawa::EDOGAWA, player);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::g_settle_02_draw_one_ok_mutant_two(&effects, player),
        "mutant: body draws one card, not two"
    );
}

/// MUTANT: a wrong spec that attributes the draw to another seat.
#[kani::proof]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn edogawa_settle_mutant_wrong_player() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    world::reset_world(w);
    call_settle(&rules_tiles::edogawa::EDOGAWA, player);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::g_settle_02_draw_one_ok_mutant_wrong_player(&effects, player),
        "mutant: the draw goes to the settler"
    );
}

// ===========================================================================
// G-SETTLE-02 — `tile:circle` settle half
// (the [经过] reward is a separate entry, BLOCKED-HOST on trig_move_*)
// ===========================================================================

/// IFF: settling CiRCLE draws exactly one hand card for the settler.
#[kani::proof]
#[kani::unwind(48)]
pub fn circle_settle_draw_one_iff() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    let pre_hand_n = w.hand_n;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    call_settle(&rules_tiles::circle::CIRCLE, player);

    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    kani::cover!(log_n == 1, "exactly one effect");
    crate::kassert!(
        spec::g_settle_02_draw_one_ok(&effects, player),
        "G-SETTLE-02 circle: effect log must be exactly one Draw{player,1}"
    );
    let post_hand_n = world::with_world(|w| w.hand_n);
    crate::kassert!(
        post_hand_n as i32 == pre_hand_n as i32 + 1,
        "G-SETTLE-02 circle: hand count must rise by 1"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
}

/// MUTANT: two cards.
#[kani::proof]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn circle_settle_mutant_two_cards() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    world::reset_world(w);
    call_settle(&rules_tiles::circle::CIRCLE, player);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::g_settle_02_draw_one_ok_mutant_two(&effects, player),
        "mutant: body draws one card, not two"
    );
}

/// MUTANT: wrong player.
#[kani::proof]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn circle_settle_mutant_wrong_player() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    world::reset_world(w);
    call_settle(&rules_tiles::circle::CIRCLE, player);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::g_settle_02_draw_one_ok_mutant_wrong_player(&effects, player),
        "mutant: the draw goes to the settler"
    );
}

// ===========================================================================
// G-SETTLE-03 — `tile:event` (CiRCLE咖啡厅 / 流星堂)
// 「[结算]是：抽取一张手卡，然后抽取一个事件卡。」
//
// Observable half only: the hand draw. `ctx::draw_event` is a protocol stub
// (returns 0, logs no effect) — the event half is PROTOCOL-LEVEL / BLOCKED-HOST
// until the host models `H.DrawEvent` (G-SETTLE-04 / 05 ride on it).
// ===========================================================================

/// IFF (observable half): settling the cafe / 流星堂 draws exactly one hand card.
#[kani::proof]
#[kani::unwind(48)]
pub fn event_settle_draw_one_iff() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    let pre_hand_n = w.hand_n;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    call_settle(&rules_tiles::event::EVENT, player);

    let post = world::snapshot();
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];

    kani::cover!(log_n == 1, "exactly one observable effect (hand draw)");
    crate::kassert!(
        spec::g_settle_03_hand_draw_ok(&effects, player),
        "G-SETTLE-03: observable effect log must be exactly one Draw{player,1}"
    );
    let post_hand_n = world::with_world(|w| w.hand_n);
    crate::kassert!(
        post_hand_n as i32 == pre_hand_n as i32 + 1,
        "G-SETTLE-03: hand count must rise by 1"
    );
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.fire, pre.fire), "frame: fire");
    crate::kassert!(post.stay == pre.stay, "frame: stay");
}

/// MUTANT: two cards.
#[kani::proof]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn event_settle_mutant_two_cards() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    world::reset_world(w);
    call_settle(&rules_tiles::event::EVENT, player);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::g_settle_02_draw_one_ok_mutant_two(&effects, player),
        "mutant: body draws one card, not two"
    );
}

/// MUTANT: wrong player.
#[kani::proof]
#[kani::unwind(48)]
#[kani::should_panic]
pub fn event_settle_mutant_wrong_player() {
    let player = any_seat();
    let w = symbolic_settle_world(player);
    world::reset_world(w);
    call_settle(&rules_tiles::event::EVENT, player);
    let (logbuf, log_n) = effects();
    let effects = &logbuf[..log_n];
    crate::kassert!(
        spec::g_settle_02_draw_one_ok_mutant_wrong_player(&effects, player),
        "mutant: the draw goes to the settler"
    );
}

// ===========================================================================
// Pure-spec unit checks (no body). These pin the book arithmetic so a future
// host landing can reuse them; they are cheap and always green.
// ===========================================================================

/// G-MONEY-10 half-charge rounding: 「向上取整10」.
#[kani::proof]
#[kani::unwind(8)]
pub fn agent_half_charge_rounds_up_to_ten() {
    let full: i32 = kani::any();
    kani::assume(full >= 0 && full <= 10_000);
    let cut = spec::agent_half_charge(full);
    if full <= 0 {
        crate::kassert!(cut == 0);
        return;
    }
    // cut == ceil(full/20)*10, so cut >= full/2 and cut % 10 == 0 and
    // cut - 10 < ceil(full/2).
    crate::kassert!(cut % 10 == 0, "G-MONEY-10: cut is a multiple of 10");
    let half = (full + 1) / 2;
    crate::kassert!(cut >= half, "G-MONEY-10: cut >= ceil(full/2)");
    crate::kassert!(cut < half + 10, "G-MONEY-10: cut is the *smallest* such multiple of 10");
    kani::cover!(full == 301, "301 → 160 (the clause-doc example)");
    if full == 301 {
        crate::kassert!(cut == 160, "G-MONEY-10: 301 → 160");
    }
}

/// G-MONEY-07 / 11: force-buy is exactly twice the buy total.
#[kani::proof]
#[kani::unwind(8)]
pub fn force_buy_is_double_buy_total() {
    let price: i32 = kani::any();
    let houses: i32 = kani::any();
    let house_cost: i32 = kani::any();
    kani::assume(price >= 0 && price <= 10_000);
    kani::assume(houses >= 0 && houses <= 4);
    kani::assume(house_cost >= 0 && house_cost <= 5_000);
    let buy = spec::buy_total(price, houses, house_cost);
    let force = spec::force_buy_total(price, houses, house_cost);
    crate::kassert!(force == 2 * buy, "G-MONEY-07: force-buy is 2 × (price + houses·house)");
}

/// G-SETTLE-08: level cap is `rent.len() - 1`; empty table ⇒ 0 (RiNG).
#[kani::proof]
#[kani::unwind(8)]
pub fn level_cap_is_rent_len_minus_one() {
    let n: usize = kani::any();
    kani::assume(n <= 5);
    let mut table = [0i32; 5];
    let mut i = 0;
    while i < n {
        table[i] = (i as i32) * 100;
        i += 1;
    }
    let cap = spec::level_cap(&table[..n]);
    crate::kassert!(
        cap == (n as i32) - 1,
        "G-SETTLE-08: cap is the last marked level"
    );
    kani::cover!(n == 0, "empty table (RiNG) caps at 0");
    kani::cover!(n == 4, "购物中心 four rent rows cap at 3");
}

/// G-SETTLE-06: unowned leg is the buy of price+houses.
#[kani::proof]
#[kani::unwind(8)]
pub fn property_leg_unowned_is_buy() {
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < 4);
    let owner: i32 = kani::any();
    kani::assume(owner == -1 || (owner >= 0 && owner < 4));
    let mortgaged: bool = kani::any();
    let leg = spec::g_settle_property_leg(owner, player, mortgaged);
    if owner < 0 {
        crate::kassert!(leg == spec::PropertyLeg::UnownedBuy);
    }
    kani::cover!(owner < 0, "unowned → buy");
    kani::cover!(owner == player && !mortgaged, "own + free → build");
    kani::cover!(owner == player && mortgaged, "own + mortgaged → no effect");
    kani::cover!(owner >= 0 && owner != player && !mortgaged, "other + free → rent");
    kani::cover!(owner >= 0 && owner != player && mortgaged, "other + mortgaged → force-buy");
}

/// G-SETTLE-13: the all-owned-by-other gate.
#[kani::proof]
#[kani::unwind(12)]
pub fn agent_all_owned_gate() {
    let player: i32 = kani::any();
    kani::assume(player >= 0 && player < 4);
    let n: usize = kani::any();
    kani::assume(n >= 1 && n <= 3);
    let mut owners = [-1i32; 3];
    let mut i = 0;
    while i < n {
        owners[i] = kani::any();
        kani::assume(owners[i] >= -1 && owners[i] < 4);
        i += 1;
    }
    let all = spec::g_settle_13_all_owned_by_other(&owners[..n], player);
    kani::cover!(all, "all same-colour owned by others → half-charge sweep");
    kani::cover!(!all, "some unowned / self-owned → choose-one settle");
    if all {
        let mut j = 0;
        while j < n {
            crate::kassert!(owners[j] >= 0 && owners[j] != player, "gate: every member is other-owned");
            j += 1;
        }
    }
}
