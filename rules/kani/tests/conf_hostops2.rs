//! Host-model conformance for the hostops-2 defect batch.
//!
//! Each test pins the **engine** contract (from `game-rules/src/hostfns.rs`
//! and `game-core/src/engine/ops.rs`) against `rules_kani::host::dispatch`.
//! Fail-closed: no `#[ignore]`. See `docs/rulebook/spec/kani/HOSTOPS-ROUND2.md`.

use std::sync::{Mutex, MutexGuard, OnceLock};

use card_sdk::kani_host::Op;
// Pull in `native_shims` so the `bandori_*` host-import symbols link
// (unmodified card bodies / ctx:: call them on native).
use game_rules::native_shims as _;
use rules_kani::world::{self as kworld, World as KWorld};

fn model_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn install(w: KWorld) {
    rules_kani::host::install();
    kworld::reset_world(w);
}

fn base() -> KWorld {
    let mut w = KWorld::new();
    w.n_players = 3;
    w.out = [false, false, false, true];
    // Real-board positions so the pilot TILE_SHOP=3 shortcut does not fire
    // (`pos.iter().all(|p| p < 8)` selects the 8-tile pilot fixture).
    w.pos = [20, 25, 30, 0];
    w
}

/// `state_set(player, ptr, len, field, v)`: a3 = field, a4 = value.
/// Field 0 writes VALUE and logs `SetState`; 1/2/3 write MAX/MIN/EXPIRES.
#[test]
fn state_set_packs_field_and_value() {
    let _g = model_lock();
    install(base());
    // key "k" interned via the model's name path: pass through host's name_id
    // by using set_slot's packing first to create the key, then state_set.
    // set_slot(player, ptr, len, v) — v at a3.
    let key = rules_kani::intern::intern("k");
    // Drive via dispatch with interned-name handles is awkward; use world API
    // through the same field the host writes.
    // field 0: VALUE = 7 (a4), not the field id (a3).
    let (ptr, len) = (0i32, 1i32); // will be interned as raw bytes via name_id
    // Use the real host path: intern the name bytes into the guest arena.
    let h = card_sdk::native::intern(b"k");
    let ret = rules_kani::host::dispatch(
        Op::state_set,
        [0, h.0, h.1, 0, 7, 0], // player 0, field 0, v 7
    );
    assert_eq!(ret, 7, "state_set field 0 returns the value");
    let got = kworld::with_world(|w| w.key_get(0, key));
    assert_eq!(got, 7, "VALUE column must be a4, not a3");
    let _ = (ptr, len);

    // field 1 = MAX
    rules_kani::host::dispatch(Op::state_set, [0, h.0, h.1, 1, 9, 0]);
    let max = kworld::with_world(|w| w.key_max(0, key));
    assert_eq!(max, 9, "field 1 writes MAX");
    // field 2 = MIN
    rules_kani::host::dispatch(Op::state_set, [0, h.0, h.1, 2, 3, 0]);
    let min = kworld::with_world(|w| w.key_min(0, key));
    assert_eq!(min, 3, "field 2 writes MIN");
    // field 3 = EXPIRES
    rules_kani::host::dispatch(Op::state_set, [0, h.0, h.1, 3, 2, 0]);
    let exp = kworld::with_world(|w| w.key_expires(0, key));
    assert_eq!(exp, 2, "field 3 writes EXPIRES");
}

/// `ctx::n(k, v)` doubles only when `play_doubled() == k`. Default is -1
/// (engine `None`) so `n(0, 600)` yields 600, not 1200.
#[test]
fn play_doubled_defaults_to_no_doubling() {
    let _g = model_lock();
    install(base());
    let d = rules_kani::host::dispatch(Op::play_doubled, [0; 6]);
    assert_eq!(d, -1, "engine default is None → -1 (no doubling)");
    // set_play_doubled(0) arms slot 0
    rules_kani::host::dispatch(Op::set_play_doubled, [0, 0, 0, 0, 0, 0]);
    assert_eq!(rules_kani::host::dispatch(Op::play_doubled, [0; 6]), 0);
    // set -1 clears
    rules_kani::host::dispatch(Op::set_play_doubled, [-1, 0, 0, 0, 0, 0]);
    assert_eq!(rules_kani::host::dispatch(Op::play_doubled, [0; 6]), -1);
}

/// `trig_set_cancelled` / `trig_set_negate_effect` take no value — `sys_kani`
/// packs `[0;6]`. The call **is** the effect.
#[test]
fn trig_set_cancelled_and_negate_are_noarg_setters() {
    let _g = model_lock();
    install(base());
    assert_eq!(
        kworld::with_world(|w| w.trig_cancelled),
        false,
        "pre: not cancelled"
    );
    rules_kani::host::dispatch(Op::trig_set_cancelled, [0; 6]);
    assert_eq!(
        kworld::with_world(|w| w.trig_cancelled),
        true,
        "setter with [0;6] must still cancel"
    );
    rules_kani::host::dispatch(Op::trig_set_negate_effect, [0; 6]);
    assert_eq!(kworld::with_world(|w| w.trig_negate), true);
}

/// `tile_named` resolves every `board.json` name (0-based engine index).
#[test]
fn tile_named_resolves_board_names() {
    let _g = model_lock();
    install(base());
    for (name, expect) in [
        ("CiRCLE", 0),
        ("江户川乐器店", 30),
        ("Live House", 31),
        ("Bandori车站", 35),
        ("流星堂", 45),
        ("商店街", 46),
        ("飞鸟山公园", 56),
        ("no-such-tile", -1),
    ] {
        let h = card_sdk::native::intern(name.as_bytes());
        let got = rules_kani::host::dispatch(Op::tile_named, [h.0, h.1, 0, 0, 0, 0]);
        assert_eq!(got, expect as i64, "tile_named({name})");
    }
}

/// `neighbor(player, dir)` = living-seat neighbour in turn order (not a tile).
#[test]
fn neighbor_is_living_seat() {
    let _g = model_lock();
    let mut w = base();
    w.n_players = 3;
    w.out = [false, true, false, true]; // seat 1 is out
    install(w);
    // from 0 forward: skip out seat 1 → seat 2
    assert_eq!(
        rules_kani::host::dispatch(Op::neighbor, [0, 1, 0, 0, 0, 0]),
        2
    );
    // from 2 forward: wrap to 0
    assert_eq!(
        rules_kani::host::dispatch(Op::neighbor, [2, 1, 0, 0, 0, 0]),
        0
    );
    // from 0 backward: skip out seat 1 (wrap) → seat 2? seats 0,1,2; back from 0
    // walks 2 then 1(out) — first present is 2? j=1 → k=2 present → 2.
    // engine: for j in 1..n { k = wrap(player+dir*j); if present return k }
    // dir=-1: j=1 → k=2 (present) → 2.
    assert_eq!(
        rules_kani::host::dispatch(Op::neighbor, [0, -1, 0, 0, 0, 0]),
        2
    );
}

/// stay / stun / exile are distinct (`key::STAY` / `STUN` / `EXILE`).
#[test]
fn stay_stun_exile_are_distinct() {
    let _g = model_lock();
    install(base());
    rules_kani::host::dispatch(Op::give_stay, [0, 1, 0, 0, 0, 0]);
    rules_kani::host::dispatch(Op::give_stun, [0, 2, 0, 0, 0, 0]);
    rules_kani::host::dispatch(Op::give_exile, [0, 3, 5, 0, 0, 0]);
    assert_eq!(rules_kani::host::dispatch(Op::stay_of, [0, 0, 0, 0, 0, 0]), 1);
    assert_eq!(rules_kani::host::dispatch(Op::stun_of, [0, 0, 0, 0, 0, 0]), 2);
    assert_eq!(rules_kani::host::dispatch(Op::exile_of, [0, 0, 0, 0, 0, 0]), 3);
    assert_eq!(
        kworld::with_world(|w| w.exile_to[0]),
        5,
        "give_exile stores EXILE_TO"
    );
}

/// `add_counter_*` clamps: `(was+n).max(0)`, then `min(max)` when `max > 0`.
#[test]
fn add_counter_clamps_to_max() {
    let _g = model_lock();
    install(base());
    // Place a field instance so `add_counter_self` (running instance) has a
    // target — engine `add_counter` is `add_counter_at(current_uid)`.
    let ch = card_sdk::native::intern(b"A");
    let uid = rules_kani::host::dispatch(Op::place_card_at, [0, ch.0, ch.1, 0, 0, 0]);
    assert!(uid >= 0);
    let h = card_sdk::native::intern(b"crystals");
    // +5 with max 3 → 3
    let got = rules_kani::host::dispatch(
        Op::add_counter_self,
        [h.0, h.1, 5, 3, 0, 0],
    );
    assert_eq!(got, 3, "clamp at max");
    // +5 with max 0 (uncapped) → 8
    let got = rules_kani::host::dispatch(
        Op::add_counter_self,
        [h.0, h.1, 5, 0, 0, 0],
    );
    assert_eq!(got, 8, "max=0 is uncapped");
    // -100 floor at 0
    let got = rules_kani::host::dispatch(
        Op::add_counter_self,
        [h.0, h.1, -100, 0, 0, 0],
    );
    assert_eq!(got, 0, "floor at 0");
}

/// `gain_fire` clamps at `fire_max` and returns the amount gained.
#[test]
fn gain_fire_clamps_at_cap() {
    let _g = model_lock();
    let mut w = base();
    w.fire[0] = 2;
    w.fire_max[0] = 3;
    install(w);
    let got = rules_kani::host::dispatch(Op::gain_fire, [0, 5, 0, 0, 0, 0]);
    assert_eq!(got, 1, "only (cap-was) is gained");
    assert_eq!(kworld::with_world(|w| w.fire[0]), 3);
    // at cap → 0
    let got = rules_kani::host::dispatch(Op::gain_fire, [0, 5, 0, 0, 0, 0]);
    assert_eq!(got, 0, "at cap gains nothing");
}

/// `tile_in(_, _, "shop")` = buyable && group == 10 (商店街), not `kind == 5`.
/// (Was `Op::is_shop` before ABI v56.)
#[test]
fn is_shop_is_buyable_group_10() {
    let _g = model_lock();
    install(base());
    // Class string "shop"; tile_in(player, tile, class_ptr, class_len).
    let (cp, cl) = card_sdk::native::intern(b"shop");
    // 羽泽咖啡厅 (47) is property + group 10 — a 商店街 deed.
    assert_eq!(
        rules_kani::host::dispatch(Op::tile_in, [-1, 47, cp, cl, 0, 0]),
        1,
        "羽泽咖啡厅 (group 10 property) is a shop"
    );
    // 江户川乐器店 is kind edogawa group 0 — NOT a shop.
    assert_eq!(
        rules_kani::host::dispatch(Op::tile_in, [-1, 30, cp, cl, 0, 0]),
        0,
        "江户川乐器店 is not group 10"
    );
    // The agent tile 「商店街」 (46) is the group marker, not buyable.
    assert_eq!(
        rules_kani::host::dispatch(Op::tile_in, [-1, 46, cp, cl, 0, 0]),
        0,
        "商店街 agent itself is not buyable"
    );
    // Live House Galaxy is group 6 — not a shop.
    assert_eq!(
        rules_kani::host::dispatch(Op::tile_in, [-1, 50, cp, cl, 0, 0]),
        0,
        "Live House Galaxy is group 6"
    );
}

/// `shuffle_into_deck` moves hand/discard into draw (multiset; order irrelevant).
#[test]
fn shuffle_into_deck_moves_piles() {
    let _g = model_lock();
    let mut w = base();
    w.hand_pile[0] = 1;
    w.hand_pile[1] = 2;
    w.hand_n = 2;
    w.discard_pile[0] = 3;
    w.discard_n = 1;
    w.deck_n = 0;
    install(w);
    let moved = rules_kani::host::dispatch(Op::shuffle_into_deck, [0, 1, 1, 0, 0, 0]);
    assert_eq!(moved, 3, "three cards moved");
    let (hand_n, disc_n, deck_n) =
        kworld::with_world(|w| (w.hand_n, w.discard_n, w.deck_n));
    assert_eq!(hand_n, 0, "hand emptied");
    assert_eq!(disc_n, 0, "discard emptied");
    assert_eq!(deck_n, 3, "draw holds the multiset");
}

/// `place_card_on` ABI is `[player, tile, cp, cl, ptr, len]` — the card name
/// is at (a2, a3), the landing tile at a1 (HHW kkr-02 → CiRCLE).
#[test]
fn place_card_on_reads_name_from_cp_cl() {
    let _g = model_lock();
    install(base());
    // 1-byte slot facade for card name (see ctx::cards_in under cfg(kani)).
    // slot 0 = CARD_AYA. cp/cl for "A" (1 byte).
    let h = card_sdk::native::intern(b"A");
    // Place on tile 0 (CiRCLE), name at (a2,a3) = (h.0, h.1).
    let uid = rules_kani::host::dispatch(
        Op::place_card_on,
        [0, 0, h.0, h.1, 0, 0],
    );
    assert!(uid >= 0, "place_card_on returns a uid");
    let tile = kworld::with_world(|w| w.field[0].tile);
    assert_eq!(tile, 0, "lands on the given tile (CiRCLE), not dest");
}

/// Generated card-id table matches RULESET flatten order.
#[test]
fn generated_card_ids_match_ruleset() {
    let _g = model_lock();
    let n = rules_kani::invoke::ruleset_len();
    assert_eq!(n, rules_kani::generated::CARD_COUNT);
    for i in 0..n {
        let def = rules_kani::invoke::by_idx(i);
        assert_eq!(def.id, rules_kani::generated::IDS[i], "index {i}");
    }
}
/// Event instances (no `field[]` slot) still own their crystals / phase.
/// `set_counter_self`/`counter_self` fall back to `World::self_crystals` —
/// heat_head's two-phase machine and circle_rebuild's 5 miracle crystals.
#[test]
fn self_crystals_roundtrip_without_field_slot() {
    let _g = model_lock();
    install(base());
    // last_uid=1 has no field slot in a fresh world.
    let n = kworld::with_world(|w| w.field_n);
    assert_eq!(n, 0, "fresh world has no field cards");
    let h0 = card_sdk::native::intern(b"crystals");
    let before = rules_kani::host::dispatch(Op::counter_self, [h0.0, h0.1, 0, 0, 0, 0]);
    assert_eq!(before, 0, "unset self crystals are 0");
    let h = card_sdk::native::intern(b"crystals");
    rules_kani::host::dispatch(Op::set_counter_self, [h.0, h.1, 5, 0, 0, 0]);
    let after = rules_kani::host::dispatch(Op::counter_self, [h.0, h.1, 0, 0, 0, 0]);
    assert_eq!(after, 5, "self crystals stick without a field slot");
    let stored = kworld::with_world(|w| w.self_crystals);
    assert_eq!(stored, 5, "stored on World::self_crystals");
}

/// `neighbor(p, ±1)` is the **next** present seat in turn order, wrapping —
/// not the last one scanned (that was the far side for n=4).
#[test]
fn neighbor_is_the_next_present_seat() {
    let _g = model_lock();
    install(base());
    kworld::reset_world(KWorld {
        n_players: 4,
        ..KWorld::new()
    });
    let fwd = rules_kani::host::dispatch(Op::neighbor, [0, 1, 0, 0, 0, 0]);
    assert_eq!(fwd, 1, "forward neighbour of 0 is seat 1");
    let back = rules_kani::host::dispatch(Op::neighbor, [0, -1, 0, 0, 0, 0]);
    assert_eq!(back, 3, "backward neighbour of 0 is seat 3");
    // Seat 1 out: forward neighbour of 0 becomes 2.
    kworld::with_world_mut(|w| w.out[1] = true);
    let fwd2 = rules_kani::host::dispatch(Op::neighbor, [0, 1, 0, 0, 0, 0]);
    assert_eq!(fwd2, 2, "skip the out seat");
}

/// `Op::raise` returns `(!cancelled)`: **1 = the link settled**. Returning 0
/// makes `spend_fire` treat the cost as substituted and skip the real spend
/// (class C — `chart_rush` lost no [火罐]). Pin the settled polarity.
#[test]
fn raise_returns_settled_not_cancelled() {
    let _g = model_lock();
    install(base());
    let settled = rules_kani::host::dispatch(Op::raise, [0, 0, 0, 0, 0, 0]);
    assert_eq!(settled, 1, "raise must report settled (1), not cancelled (0)");
}

/// `Op::gate` returns `allowed`: **1 = may land**. 0 means guarded / immune /
/// out and callers skip the effect — same polarity class as `raise`.
#[test]
fn gate_returns_allowed_for_living_seat() {
    let _g = model_lock();
    install(base());
    let ok = rules_kani::host::dispatch(Op::gate, [0, 0, 0, 0, 0, 0]);
    assert_eq!(ok, 1, "living seat with no guard: effect may land");
    // Seat 3 is `out` in `base()`.
    let blocked = rules_kani::host::dispatch(Op::gate, [3, 0, 0, 0, 0, 0]);
    assert_eq!(blocked, 0, "out seat: gate refuses");
}

/// `Op::raise_bought` returns HOST-06 `did`: **1 = the hand-over landed**.
#[test]
fn raise_bought_reports_landed() {
    let _g = model_lock();
    install(base());
    let did = rules_kani::host::dispatch(Op::raise_bought, [0, 0, 0, 0, 0, 0]);
    assert_eq!(did, 1, "hand-over landed when no hook denies");
}

/// `Op::ring_multiplier` is a multiplier: 0 would zero rent. Identity is 1.
#[test]
fn ring_multiplier_is_never_zero() {
    let _g = model_lock();
    install(base());
    let mult = rules_kani::host::dispatch(Op::ring_multiplier, [0, 0, 0, 0, 0, 0]);
    assert!(mult >= 1, "ring multiplier must be >= 1, got {mult}");
}
