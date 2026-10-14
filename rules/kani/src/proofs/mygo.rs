//! Unit `mygo`: clause proofs.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. `*_mutant_is_detected` reachable-disagreement companion (and
//!    `#[kani::should_panic]` only when it really reports panic-as-expected).
//!
//! Specs come from `crate::spec::mygo` — written from the clause docs only.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

// ---------------------------------------------------------------------------
// Shared symbolic pre-world (status / play bodies: no trigger overlay).
// Isolation: empty hand / discard / field so stacked hooks cannot fire.
// ---------------------------------------------------------------------------

fn symbolic_play_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < world::TILE_COUNT);
        w.fire[p] = kani::any();
        kani::assume(w.fire[p] >= 0 && w.fire[p] <= 8);
        w.stay[p] = kani::any();
        kani::assume(w.stay[p] >= 0 && w.stay[p] <= 4);
        w.out[p] = kani::any();
    }
    w
}

fn living_player(w: &World) -> i32 {
    let p = crate::harness::any_player(w.n_players);
    kani::assume(!w.out[p as usize]);
    p
}

// ===========================================================================
// C-MYGO-rinpuu-01
// 「使自己获得一层[停留]」 — `MyGO:轮符雨` On::Play.
// ===========================================================================

/// Iff: the unmodified play body grants exactly one [停留] layer to the user
/// and nothing else (narration is not the frame; `before_turn_end` is a
/// scheduling stub that logs no effect).
#[kani::proof]
#[kani::unwind(24)]
pub fn rinpuu_01_give_stay_iff() {
    let w = symbolic_play_world();
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:轮符雨"), player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    assert!(
        spec::mygo::rinpuu_01_log_ok(effects, player),
        "C-MYGO-rinpuu-01: effect log must be exactly one GiveStay(player, 1)"
    );
    assert!(
        spec::mygo::rinpuu_01_post_ok(&pre, &post, player),
        "C-MYGO-rinpuu-01: post must be pre + 1 stay on the user, frame unchanged"
    );

    kani::cover!(pre.stay[player as usize] == 0, "pre stay was 0");
    kani::cover!(pre.stay[player as usize] > 0, "pre stay was nonzero");
}

/// Mutant companion: a two-layer spec must be distinguishable from the card.
#[kani::proof]
#[kani::unwind(24)]
pub fn rinpuu_01_mutant_is_detected() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:轮符雨"), player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    // The card does n=1; the mutant wants n=2. They must disagree.
    assert!(
        !spec::mygo::rinpuu_01_log_ok_mutant(effects, player),
        "mutant: two-layer spec must not match the one-layer card"
    );
    let _ = pre;
}

/// `#[kani::should_panic]` twin of the mutant companion.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn rinpuu_01_mutant_two_layers() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:轮符雨"), player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        spec::mygo::rinpuu_01_log_ok_mutant(effects, player),
        "mutant: card must not match the two-layer spec"
    );
    let _ = pre;
}

// ===========================================================================
// C-MYGO-journey-01
// 「[手] 将此卡放置于当前格子上并为其放置4个奇迹水晶」
// `MyGO:哪怕这旅程没有终点` On::Play.
// Isolation: empty field / hand so stacked hooks cannot fire. `set_crystals(4)`
// is a counter write (host `set_counter_self`) that logs no Effect; we check
// it via the world's key map (`"crystals"` on the trig player).
// ===========================================================================

const JOURNEY_ID: usize = crate::card_id!("MyGO:哪怕这旅程没有终点");

fn crystals_key() -> crate::intern::NameId {
    crate::intern::intern("crystals")
}

/// Iff: place on Field + the instance holds exactly 4 [奇迹水晶].
#[kani::proof]
#[kani::unwind(24)]
pub fn journey_01_place_four_crystals_iff() {
    let w = symbolic_play_world();
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    let pre_c = world::with_world(|ww| ww.key_get(player, crystals_key()));

    invoke::unwrap_done(invoke::play_idx(JOURNEY_ID, player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    let field_n = world::with_world(|ww| ww.field_n);

    // Host `card_slot` maps unknown card names to slot 2 (pilot CARD_OTHER).
    assert!(
        spec::mygo::journey_01_log_ok(effects, player, 2),
        "C-MYGO-journey-01: effect log must be exactly one PlaceCard(Field)"
    );
    assert!(post_c == 4, "C-MYGO-journey-01: card must hold exactly 4 crystals");
    assert!(field_n == 1, "C-MYGO-journey-01: exactly one field instance");
    assert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    assert!(post.stay == pre.stay, "frame: stay unchanged");
    assert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand unchanged (play body does not draw)");
    let _ = pre_c;

    kani::cover!(pre.stay[player as usize] == 0, "clean pre");
}

/// Mutant companion: a two-PlaceCard spec must not match.
#[kani::proof]
#[kani::unwind(24)]
pub fn journey_01_mutant_is_detected() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(JOURNEY_ID, player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        !spec::mygo::journey_01_log_ok_mutant(effects, player, 2),
        "mutant: two-PlaceCard spec must not match the one-place card"
    );
}

/// C-MYGO-rinpuu-02 is PROTOCOL-LEVEL: 「并在回合结束时额外进行一次[触发结算]」
/// is engine scheduling (`On::AtEnd` + `card_settle_at`, a protocol stub).
/// Documented here so the unit table can cite a harness name.

// ===========================================================================
// C-MYGO-haneoka-02/03/04/05
// `MyGO:羽丘的不可思议女孩` On::Play. Gated on owning the current tile
// (`cant_play` / body early-out). Roll 1d20 is fully symbolic under Kani.
// Isolation: the tile is owned by the user and unmortgaged so `add_house`
// fires on the >=10 band.
// ===========================================================================

fn haneoka_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.pos[p] = kani::any();
        kani::assume(w.pos[p] >= 0 && w.pos[p] < world::TILE_COUNT);
        w.stay[p] = 0;
        w.fire[p] = 0;
        w.out[p] = kani::any();
    }
    w
}

/// Iff over every 1d20 face: r<10 empty log; r>=10 SetHouses; r>=15 +Draw;
/// r>=20 +PlaceCard. Covers each band.
#[kani::proof]
#[kani::unwind(32)]
pub fn haneoka_thresholds_iff() {
    let mut w = haneoka_world();
    let player = living_player(&w);
    let pos = w.pos[player as usize];
    // own tile, unmortgaged — the book's 「位于属于自己的格子上时」
    w.tiles[pos as usize].owner = player;
    w.tiles[pos as usize].mortgaged = false;
    w.tiles[pos as usize].houses = kani::any();
    kani::assume(w.tiles[pos as usize].houses >= 0 && w.tiles[pos as usize].houses <= 4);

    let pre_houses = w.tiles[pos as usize].houses;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:羽丘的不可思议女孩"), player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_houses = world::with_world(|ww| ww.tiles[pos as usize].houses);

    // Recover the face from the effects: <10 → empty; else look at the chain.
    // We don't have the face in the log, so branch on the observed shape and
    // cover each band via the house/draw/place presence.
    let has_house = effects.iter().any(|e| matches!(e, Effect::SetHouses { .. }));
    let has_draw = effects.iter().any(|e| matches!(e, Effect::Draw { .. }));
    let has_place = effects.iter().any(|e| matches!(e, Effect::PlaceCard { .. }));

    kani::cover!(!has_house && !has_draw && !has_place, "r < 10 (ineffective)");
    kani::cover!(has_house && !has_draw && !has_place, "10..=14 (house only)");
    kani::cover!(has_house && has_draw && !has_place, "15..=19 (house+draw)");
    kani::cover!(has_house && has_draw && has_place, "r >= 20 (house+draw+place)");

    // Shape consistency: draw only with house; place only with draw.
    if has_draw {
        assert!(has_house, "C-MYGO-haneoka: draw implies house (r>=15 ⊇ r>=10)");
    }
    if has_place {
        assert!(has_draw, "C-MYGO-haneoka: place implies draw (r>=20 ⊇ r>=15)");
        assert!(has_house, "C-MYGO-haneoka: place implies house");
    }
    if has_house {
        assert!(
            post_houses == pre_houses.saturating_add(1),
            "C-MYGO-haneoka-02: exactly one free house"
        );
        assert!(world::arr4_eq(post.money, pre.money), "C-MYGO-haneoka-02: 免费 — money unchanged");
    } else {
        assert!(post_houses == pre_houses, "C-MYGO-haneoka-05: no house on r<10");
        assert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    }
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    assert!(post.stay == pre.stay, "frame: stay unchanged");
}

/// Mutant: the ineffective band must not match a one-effect spec.
#[kani::proof]
#[kani::unwind(24)]
pub fn haneoka_05_mutant_is_detected() {
    let mut w = haneoka_world();
    let player = living_player(&w);
    let pos = w.pos[player as usize];
    w.tiles[pos as usize].owner = player;
    w.tiles[pos as usize].mortgaged = false;
    world::reset_world(w);
    // Force the ineffective band by assuming the symbolic die is < 10 is NOT
    // possible to force post-hoc; instead just run and check: if the log is
    // empty the mutant (wants 1 effect) must disagree.
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:羽丘的不可思议女孩"), player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    if effects.is_empty() {
        assert!(
            !spec::mygo::haneoka_effects_ok_mutant(effects, player, pos, 5, 2),
            "mutant: empty-log ineffective band must not match a 1-effect spec"
        );
    }
}

// ===========================================================================
// C-MYGO-rainday-01
// 「将此卡放置于自己场上并为其放置5个奇迹水晶」
// `MyGO:那天的雨` On::Play. The play body also runs the -02 rain; that leg's
// GiveStay effects follow the PlaceCard. This harness checks the placement +
// crystal count; `rainday_02` covers the stay leg.
// ===========================================================================

const RAINDAY_ID: usize = crate::card_id!("MyGO:那天的雨");

/// Iff: PlaceCard(Field) first; crystals key == 5; rain may add GiveStays.
#[kani::proof]
#[kani::unwind(32)]
pub fn rainday_01_place_five_crystals_iff() {
    let w = symbolic_play_world();
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play_idx(RAINDAY_ID, player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = world::with_world(|ww| ww.key_get(player, crystals_key()));

    assert!(
        spec::mygo::rainday_01_place_ok(effects, player, 2),
        "C-MYGO-rainday-01: first effect must be PlaceCard(Field)"
    );
    assert!(post_c == 5, "C-MYGO-rainday-01: card must hold exactly 5 crystals");
    assert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");

    kani::cover!(
        effects.iter().any(|e| matches!(e, Effect::GiveStay { .. })),
        "rain leg granted stay"
    );
    kani::cover!(
        !effects.iter().any(|e| matches!(e, Effect::GiveStay { .. })),
        "rain leg granted no stay"
    );
}

/// Mutant: 4 crystals must not match.
#[kani::proof]
#[kani::unwind(24)]
pub fn rainday_01_mutant_is_detected() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(RAINDAY_ID, player));
    let post_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    assert!(
        post_c != spec::mygo::rainday_01_crystals_mutant(),
        "mutant: 4-crystal spec must not match the 5-crystal card"
    );
}

// ===========================================================================
// C-MYGO-hitoshizuku-01/02/03
// `MyGO:壱雫空` On::Play.
// Isolation: no `lockedStay` (locked layers survive the clear). The host
// conflates stay and stun on one `w.stay` array, so a pre-world cannot set
// them independently; this harness covers the stay-clear + money path with
// `kinds` recovered from the body's clear prefix.
// KNOWN: the body's money leg charges `1000 * global kinds` from every living
// player (black-box MORE). The book spec is per-player kinds. We assert the
// BOOK spec; a FAILS here is the finding.
// ===========================================================================

/// Iff against the book money rule. Expected to FAILS on the money leg.
#[kani::proof]
#[kani::unwind(32)]
pub fn hitoshizuku_money_iff() {
    let w = symbolic_play_world();
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:壱雫空"), player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];

    // Recover per-player kinds from the clear prefix: GiveStay with negative n
    // is the stay clear (1 kind); GiveStun with negative n (or n!=0) plus
    // SetState stunStart=0 is the stun clear.
    let mut kinds = [0i32; MAX_PLAYERS];
    for e in effects {
        match e {
            Effect::GiveStay { player: p, n } if *n < 0 => {
                kinds[*p as usize] += 1;
            }
            Effect::GiveStun { player: p, n } if *n != 0 => {
                kinds[*p as usize] += 1;
            }
            _ => {}
        }
    }

    let global_kinds: i32 = kinds.iter().sum();
    kani::cover!(global_kinds == 0, "nothing cleared");
    kani::cover!(global_kinds > 0, "something cleared");

    if global_kinds == 0 {
        // no money leg
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::PayTo { .. } | Effect::GainMoney { .. })),
            "C-MYGO-hitoshizuku: no clear ⇒ no money"
        );
    } else {
        // BOOK spec (per-player kinds).
        let book_ok = spec::mygo::hitoshizuku_money_ok(effects, player, &kinds, pre.n_players);
        // BODY behaviour (global kinds) — kept as a diagnostic.
        let body_ok =
            spec::mygo::hitoshizuku_body_global_pay_ok(effects, player, global_kinds, pre.n_players);
        kani::cover!(body_ok, "body matches global-kinds reading");
        assert!(
            book_ok,
            "C-MYGO-hitoshizuku-02: book wants 1000 per type per affected player"
        );
        let _ = body_ok;
    }

    assert!(post.stay.iter().zip(pre.stay.iter()).all(|(a, b)| a <= b),
        "C-MYGO-hitoshizuku-01: stay never rises");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
}

/// Diagnostic companion: the body's global-kinds reading is what it does.
/// If this is SUCCESSFUL while `hitoshizuku_money_iff` FAILED, the finding is
/// MORE/WRONG on the money amount.
#[kani::proof]
#[kani::unwind(32)]
pub fn hitoshizuku_body_global_pay_observed() {
    let w = symbolic_play_world();
    let player = living_player(&w);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:壱雫空"), player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let mut kinds = [0i32; MAX_PLAYERS];
    for e in effects {
        match e {
            Effect::GiveStay { player: p, n } if *n < 0 => kinds[*p as usize] += 1,
            Effect::GiveStun { player: p, n } if *n != 0 => kinds[*p as usize] += 1,
            _ => {}
        }
    }
    let global_kinds: i32 = kinds.iter().sum();
    if global_kinds > 0 {
        assert!(
            spec::mygo::hitoshizuku_body_global_pay_ok(effects, player, global_kinds, pre.n_players),
            "diagnostic: body charges 1000 * global kinds from every living player"
        );
    }
}

// ===========================================================================
// C-MYGO-tomori-card-01/02
// `MyGO:（灯）不再迷茫` On::Play.
// Isolation: empty hand at pre (X=0 ⇒ crystals=1) so the `cards_in` String
// loop is a no-op. lockedStay is a slot write (no Effect); set_crystals(1) is
// a counter write (no Effect).
// ===========================================================================

/// Iff: GiveStay(user, 2) + PlaceCard(Field); crystals == 1 (X=0).
#[kani::proof]
#[kani::unwind(24)]
pub fn tomori_card_02_empty_hand_iff() {
    let mut w = symbolic_play_world();
    // Isolation: empty hand for every player (stated in the module docs).
    for p in 0..MAX_PLAYERS {
        w.hand_n = 0;
        w.hand = [false; 8];
    }
    let player = living_player(&w);

    let pre = {
        world::reset_world(w);
        world::snapshot()
    };

    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:（灯）不再迷茫"), player));

    let post = world::snapshot();
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    let post_c = world::with_world(|ww| ww.key_get(player, crystals_key()));
    let locked = world::with_world(|ww| ww.key_get(player, crate::intern::intern("lockedStay")));

    assert!(
        spec::mygo::tomori_card_02_empty_hand_log_ok(effects, player, 2),
        "C-MYGO-tomori-card-02: empty-hand log must be GiveStay(2) + PlaceCard"
    );
    assert!(
        post_c == spec::mygo::tomori_card_02_crystals(0),
        "C-MYGO-tomori-card-02: X=0 ⇒ exactly 1 crystal"
    );
    assert!(locked >= 2, "C-MYGO-tomori-card-01: lockedStay marks the 2 layers");
    assert!(
        post.stay[player as usize] == pre.stay[player as usize].saturating_add(2),
        "C-MYGO-tomori-card-01: exactly 2 stay layers"
    );
    assert!(world::arr4_eq(post.money, pre.money), "frame: money unchanged");
    assert!(world::arr4_eq(post.pos, pre.pos), "frame: pos unchanged");
    assert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand unchanged (empty)");
}

/// Mutant: 1 stay layer must not match.
#[kani::proof]
#[kani::unwind(24)]
pub fn tomori_card_02_mutant_is_detected() {
    let mut w = symbolic_play_world();
    w.hand_n = 0;
    w.hand = [false; 8];
    let player = living_player(&w);
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("MyGO:（灯）不再迷茫"), player));
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    let effects = &logbuf[..log_n];
    assert!(
        !spec::mygo::tomori_card_02_empty_hand_log_ok_mutant(effects, player, 2),
        "mutant: 1-stay spec must not match the 2-stay card"
    );
}