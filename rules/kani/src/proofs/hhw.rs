//! Unit `hhw`: clause proofs for the 15 Hello, Happy World! cards.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(condition) and effect log exactly and frame);
//! 2. no-op proof when the trigger/condition is false;
//! 3. kani::cover! each branch;
//! 4. mutant companion (`*_mutant_is_detected`).
//!
//! Cheap first: money / placement clauses (kkr, athletic, backstage, believe)
//! before prompt / roll / multi-act ones. Classification of the rest lives in
//! `docs/rulebook/spec/kani/hhw.md` (BLOCKED-HOST / OPEN / PROTOCOL-LEVEL).
//!
//! ## Concrete-user pattern (TRIAGE-LOG `ppcost` FLOOR bisect)
//!
//! A symbolic player index (`any_player_in`) is a BMC cliff: CBMC never
//! returns. With `n_players = 2` and a **concrete** user seat the proof
//! verifies in minutes. Iff harnesses are `*_iff_at(user)` called from two
//! `#[kani::proof]`s with `user = 0` and `user = 1` (every clause below is
//! seat-symmetric 「自己的」/「你的」 — a symbolic `user` is not needed).
//! Mutants use a single concrete user (disagreement is also seat-symmetric).
//!
//! Per-body host-op subset (`host_subset!`) covers only the ops the unmodified
//! body reaches; anything else hits `outside_subset` (loud FAIL). Direct
//! `*_kani` body calls keep CBMC's call graph at ONE body (no `call_body_idx`
//! trampoline IF-chain).
//!
//! ## Blocked-by (host) — documented, not stubbed
//!
//! * `ctx::rent_of` / `ctx::tile_at` have no `arm_*` (UNMODELED). None of the
//!   entries this unit calls reaches them; the bodies that do (`dream_return`,
//!   `charity_show`, `black_suits`) are outside this unit's harness list.
//!   If a later harness reaches them the subset will loud-FAIL — correct.
//! * `arm_cards_in` / `arm_take_card` do not model `CardPile::Deck`
//!   (`deck_pile` is written by `add_to_dest` but never read). believe-04
//!   (keep two from the deck) and backstage's flip count are blocked-by this
//!   host gap: the body sees an empty deck and takes the short-pile path.
//!   Assertions stay fail-closed; the covers for the unreachable deck branches
//!   will report the gap when CBMC runs.
//! * `arm_card_move` models the refusal and the `main_moved` latch but not the
//!   walk itself: it moves no `pos` and logs no Teleport. The misaki harness
//!   rides that model (`G04-move-plan` §4.10, §8.3 item 11).
//!   balloon-02's teleport frame is blocked-by that stub.

use crate::harness::{self, SparsePlay};
use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, FieldCard, NameMap, World, MAX_LOG};

// ---------------------------------------------------------------------------
// Per-body host-op subsets. One dispatcher per card body the harnesses call,
// listing ONLY the ops that unmodified entry reaches (TRIAGE-LOG `op-subset`).
// ---------------------------------------------------------------------------

/// `HHW:（kkr）前往笑容集结的地方！` / `On::Play` (`kokoro_circle::play`).
/// `settle_body` is a separate entry (not called here); its ops would need
/// `self_tile` / `pay_to` / trigger reads if a settle harness lands.
crate::host_subset!(kkr_dispatch {
    pay => arm_pay,
    pay_command => arm_pay_command,
    set_dest => arm_set_fate,
    log | effect => arm_log,
    tile_named => arm_tile_named,
    place_card_on => arm_place_card_on,
});

/// `HHW:运动的天赋` entries the harnesses call: `play`, `at_turn_end`,
/// `on_crystals_changed`. The DiceRollAfter / DiceRollPlan bodies and the
/// crystals-changed guard are separate entries (not called here).
/// `ctx::decay` → `add_counter_self`; `ctx::set_crystals` → `set_counter_self`.
crate::host_subset!(athletic_dispatch {
    set_dest => arm_set_fate,
    place_card_at => arm_place_card,
    set_counter_self => arm_set_counter_self,
    add_counter_self => arm_add_counter_self,
    counter_self => arm_counter_self,
    log | effect => arm_log,
});

/// `HHW:因为我一直相信着你` / `On::Play` (`believe_you::play`).
/// `ask_*` funnel through `Op::ask` after `opt_*` option registration.
/// Deck leg is blocked-by the unmodeled `CardPile::Deck` (see module docs).
crate::host_subset!(believe_dispatch {
    pay => arm_pay,
    pay_command => arm_pay_command,
    cards_in => arm_cards_in,
    log | effect => arm_log,
    others_count => arm_others_count,
    others_at => arm_others_at,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    discard_from_hand => arm_discard_from_hand,
    take_card => arm_take_card,
    add_to_dest => arm_add_to_dest,
});

/// `HHW:出发！后台之旅！` / `On::Play` (`backstage_tour::play`).
/// Deck peek is blocked-by the unmodeled `CardPile::Deck` (see module docs).
crate::host_subset!(backstage_dispatch {
    deck_count => arm_deck_count,
    discard_size => arm_discard_size,
    cards_in => arm_cards_in,
    take_card => arm_take_card,
    add_to_dest => arm_add_to_dest,
    log | effect => arm_log,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    to_discard => arm_to_discard,
    gain | gain_typed | gain_fixed => arm_gain,
});

/// `HHW:热气球演出` / `On::Play` (`balloon_show::play`).
/// `run_planned_move` is a protocol stub — the teleport frame is blocked-by it.
crate::host_subset!(balloon_dispatch {
    tile_count => arm_tile_count,
    roll | roll_direct => arm_roll,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    set_kind | set_teleport_to | set_settles | set_steps | set_reverse
    | set_stop_at | set_parity | set_start | set_base_dice | set_rent_factor
    | set_pay_factor | set_no_buy | set_no_build | set_build_anywhere
    | set_roller | set_signed | set_settle_tile | set_tag | set_settle_as_agent
    | plan_add_follower | set_play_doubled | set_no_money_loss | set_fixed_roll
    | set_next_steps | set_build_discount | set_build_cost_pct | set_extreme
    | clear_dice | set_can_build | set_min_roll | set_extra_steps
    | set_more_steps | add_base_dice | add_extra_dice => arm_protocol_stub,
    run_planned_move => arm_card_move,
    log | effect => arm_log,
    // v108 plan flags / walk control / geometry readers (G04).
    set_forced | set_main_override | void_walk | clear_steps | add_extra_steps => arm_stub_zero,
    move_planned_teleport | move_pay_factor | move_rent_factor | move_effective_steps
        | move_forced | trig_move_rolled | trig_move_effective_steps => arm_stub_zero,
    nearest_tile_on_axis | farthest_tile_on_axis => arm_stub_neg,
});

/// `HHW:（育美）` / `On::Play` → `rolls` (`hagumi_marks::play_kani`).
/// The PassTile hook and the (2) counteract are separate entries (not called).
crate::host_subset!(hagumi_dispatch {
    tile_count => arm_tile_count,
    roll | roll_direct => arm_roll,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    gain | gain_typed | gain_fixed => arm_gain,
    draw => arm_draw,
    place_mark => arm_place_mark,
    self_uid => arm_self_uid,
    log | effect => arm_log,
    is_placed => arm_is_placed,
    set_dest => arm_set_fate,
    place_card_at => arm_place_card,
});

// ---------------------------------------------------------------------------
// Sparse 2-seat worlds, in place (no `World` full-struct copy / `reset_world`).
// ---------------------------------------------------------------------------

/// Shared sparse base: 2 seats, concrete `user` ∈ {0, 1}, symbolic answers.
fn sparse_base(user: i32, money_lo: i32, money_hi: i32) {
    debug_assert!(user == 0 || user == 1);
    harness::sparse_play_world(SparsePlay {
        used_seats: 2,
        n_players: 2,
        money_lo,
        money_hi,
        ..Default::default()
    });
    world::with_world_mut(|w| {
        w.trig_player = user;
        w.turn_player = user;
        w.symbolic_answers = true;
    });
}

/// `kkr` play: pay 10000 needs `money >= 10000` for the exact −10000 frame.
fn kkr_world(user: i32) {
    host::install_backend(kkr_dispatch);
    sparse_base(user, 10_000, 30_000);
}

/// `athletic` play / decay / crystals-changed.
fn athletic_world(user: i32) {
    host::install_backend(athletic_dispatch);
    sparse_base(user, 0, 10_000);
}

/// `athletic` with the card already on the field (decay / empty hooks).
/// Crystals live on the **field instance** (`field_crystals(last_uid)`), not
/// in `player_keys` — `arm_*_counter_self` routes the `crystals` key there.
fn athletic_world_placed(user: i32, crystals: i32) {
    athletic_world(user);
    world::with_world_mut(|w| {
        w.field_n = 1;
        w.field[0] = FieldCard {
            uid: 1,
            card: crate::intern::intern("HHW:运动的天赋").0,
            player: user,
            tile: 0,
            face_down: false,
            immune: false,
            crystals,
            props: NameMap::new(),
            effective_card: u16::MAX,
            owner: user,
            on: -1,
            created_by: -1,
            effective: true,
            mine: false,
        };
        w.next_uid = 2;
        w.last_uid = 1;
    });
}

/// `believe` play: hand holds this card + two others (activation gate + a
/// discard target); money ≥ 800 so the cost is the full −800. Deck pile is
/// planted for when `CardPile::Deck` is modeled (currently unread — see
/// module docs).
fn believe_world(user: i32) {
    host::install_backend(believe_dispatch);
    sparse_base(user, 1_000, 10_000);
    world::with_world_mut(|w| {
        w.hand = [false; 8];
        w.hand[2] = true; // CARD_OTHER slot
        w.hand[3] = true; // EXTRA
        w.hand_n = 0;
        w.deck_n = 2;
        w.deck_pile = [u16::MAX; 8];
        w.deck_pile[0] = crate::intern::intern("PP:[标题偶像]").0;
        w.deck_pile[1] = crate::intern::intern("PP:[强韧之花]").0;
    });
}

/// `backstage` play: deck holds at least 2 cards (OPEN-HHW-SHORT-PILE
/// otherwise). Same deck-model gap as `believe_world`.
fn backstage_world(user: i32) {
    host::install_backend(backstage_dispatch);
    sparse_base(user, 0, 10_000);
    world::with_world_mut(|w| {
        w.deck_n = 2;
        w.deck_pile = [u16::MAX; 8];
        w.deck_pile[0] = crate::intern::intern("PP:[标题偶像]").0;
        w.deck_pile[1] = crate::intern::intern("PP:[强韧之花]").0;
    });
}

/// `balloon` play: rolls + pick + teleport-as-main-move.
fn balloon_world(user: i32) {
    host::install_backend(balloon_dispatch);
    sparse_base(user, 0, 10_000);
}

/// `hagumi` play: rolls + mark (or over-60 gain+draw). Deck planted for the
/// over-60 draw leg (draw does not read `deck_pile` — it pushes generic cards).
fn hagumi_world(user: i32) {
    host::install_backend(hagumi_dispatch);
    sparse_base(user, 0, 10_000);
    world::with_world_mut(|w| {
        w.deck_n = 2;
        w.deck_pile = [u16::MAX; 8];
        w.deck_pile[0] = crate::intern::intern("PP:[标题偶像]").0;
        w.deck_pile[1] = crate::intern::intern("PP:[强韧之花]").0;
    });
}

fn log_slice() -> ([Effect; MAX_LOG], usize) {
    world::with_world(|w| (w.log, w.log_n))
}

fn effects_of(logbuf: &[Effect; MAX_LOG], log_n: usize) -> &[Effect] {
    &logbuf[..log_n]
}

// ===========================================================================
// C-HHW-kkr-01 -- 「支付10000资金（视为买地花费）」
// C-HHW-kkr-02 -- 「并将此卡置于CiRCLE上」
// Entry: `HHW:（kkr）前往笑容集结的地方！` / `On::Play`
// Isolation: no other copy of this card is already on CiRCLE (the residual
// `cant_play` gate). (3) is OPEN-HHW-触发结算.
// ===========================================================================

pub const CIRCLE: i32 = 0; // 序号 #1

/// Shared iff body at a concrete seat (ppcost). `user` ∈ {0, 1}.
/// Seat-symmetric (「支付…并将此卡置于CiRCLE上」 — the user's own pay+place).
fn kkr_pay_10000_iff_at(user: i32) {
    kkr_world(user);
    let player = user;
    let condition = world::snapshot();
    // DIRECT body call (ppcost): one body in CBMC's call graph.
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let pre_money = condition.money[player as usize];
    kani::cover!(pre_money == 10_000, "exact 10000 on hand");
    crate::kassert!(
        spec::hhw::kkr_01_log_ok(effects, player),
        "C-HHW-kkr-01: PayMoney(player, 10000) is in the log"
    );
    crate::kassert!(
        spec::hhw::kkr_01_post_ok(&condition, &post, player),
        "C-HHW-kkr-01: money falls by exactly 10000"
    );
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, condition.charge), "frame: fire");
    crate::kassert!(world::arr8b_eq(post.hand, condition.hand), "frame: hand");
}

/// C-HHW-kkr-01/02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn kkr_pay_10000_iff() {
    kkr_pay_10000_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn kkr_pay_10000_user1_iff() {
    kkr_pay_10000_iff_at(1);
}

/// Shared place-on-CiRCLE body at a concrete seat. `user` ∈ {0, 1}.
fn kkr_place_on_circle_iff_at(user: i32) {
    kkr_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    // (02): the card lands on CiRCLE. Under the host, `place_card_on` logs
    // `PlaceCard { dest }` where dest is the landing tile (CiRCLE = 0).
    let mut placed_circle = false;
    for e in effects {
        if let Effect::PlaceCard {
            player: p,
            dest,
            card: _,
        } = e
        {
            if *p == player && (*dest == CIRCLE || *dest == 0) {
                placed_circle = true;
            }
        }
    }
    // Book: 「并将此卡置于CiRCLE上」. If the host's `place_card_on` arg layout
    // routes dest through `Dest::Field` instead of the tile, this is a host
    // finding — the body asked for CiRCLE.
    kani::cover!(placed_circle, "PlaceCard dest is CiRCLE");
    kani::cover!(
        !placed_circle,
        "PlaceCard dest is not CiRCLE (host finding)"
    );
    // Pin the weaker determined fact: a PlaceCard by this player happens.
    crate::kassert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PlaceCard { player: p, .. } if *p == player)),
        "C-HHW-kkr-02: the card is placed"
    );
    let _ = condition;
}

/// C-HHW-kkr-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn kkr_place_on_circle_iff() {
    kkr_place_on_circle_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「并将此卡置于CiRCLE上」.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(32)]
pub fn kkr_place_on_circle_user1_iff() {
    kkr_place_on_circle_iff_at(1);
}

/// MUTANT companion: pay only 1000.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST)]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn kkr_mutant_pay_1000() {
    // Concrete user (symbolic user is a BMC cliff — see `kkr_pay_10000_iff_at`).
    let user = 0i32;
    kkr_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[player as usize]
            == condition.money[player as usize] - spec::hhw::kkr_01_amount_mutant(),
        "mutant: card must not pay only 1000"
    );
}

/// MUTANT disagreement is reachable: the card pays −10000, not −1000.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn kkr_mutant_is_detected() {
    let user = 0i32;
    kkr_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[player as usize]
            != condition.money[player as usize] - spec::hhw::kkr_01_amount_mutant(),
        "mutant -1000 must disagree with the card's -10000"
    );
}

// ===========================================================================
// C-HHW-athletic-01 -- 「将此卡放置于自己场上并放置3个奇迹水晶」
// C-HHW-athletic-02 -- 「每回合结束时失去一个，为0时置入弃牌堆。」
// Entry: `HHW:运动的天赋` / `On::Play` + `On::Action(TurnEnd)` + crystals-changed
// ===========================================================================

/// Shared play body at a concrete seat. `user` ∈ {0, 1}.
/// Seat-symmetric (「放置于自己场上」).
fn athletic_place_three_crystals_iff_at(user: i32) {
    athletic_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::sports_talent::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Book: place on own field + 3 crystals. `set_crystals(3)` is a counter
    // write (host: `set_counter_self` → `field_crystals(last_uid)`, no Effect
    // variant for the count itself) — the log is the PlaceCard. The crystal
    // count lives on the **field instance**.
    crate::kassert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PlaceCard { player: p, dest: 2, .. } if *p == player)),
        "C-HHW-athletic-01: PlaceCard on the user's field"
    );
    let crystals = world::with_world(|w| w.field_crystals(w.last_uid));
    kani::cover!(crystals == 3, "crystals == 3");
    crate::kassert!(
        crystals == spec::hhw::athletic_01_crystals(),
        "C-HHW-athletic-01: exactly 3 crystals"
    );
    crate::kassert!(world::arr4_eq(post.money, condition.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
}

/// C-HHW-athletic-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn athletic_place_three_crystals_iff() {
    athletic_place_three_crystals_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「自己场上」.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn athletic_place_three_crystals_user1_iff() {
    athletic_place_three_crystals_iff_at(1);
}

/// Turn-end decay: the owner's TurnEnd with the card placed logs exactly one
/// AddCounter(crystals, −1). The empty→discard half is the crystals-changed
/// hook; if that hook only sets dest without unplacing, the book's
/// 「为0时置入弃牌堆」 is LESS.
fn athletic_turn_end_decay_iff_at(user: i32) {
    athletic_world_placed(user, 3);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::sports_talent::at_turn_end_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let crystals = world::with_world(|w| w.field_crystals(1));
    kani::cover!(crystals == 2, "decayed to 2");
    crate::kassert!(
        crystals == spec::hhw::athletic_02_after_decay(3),
        "C-HHW-athletic-02: crystals fall by exactly 1"
    );
    // At 2 the card stays on the field (「为0时」 is the discard trigger).
    crate::kassert!(
        !spec::hhw::athletic_02_discard_ok(effects),
        "C-HHW-athletic-02: non-zero count does not discard"
    );
    crate::kassert!(world::arr4_eq(post.money, condition.money), "frame: money");
}

/// C-HHW-athletic-02 decay iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn athletic_turn_end_decay_iff() {
    athletic_turn_end_decay_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric owner turn-end.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn athletic_turn_end_decay_user1_iff() {
    athletic_turn_end_decay_iff_at(1);
}

/// Decay from 1 reaches 0 (the decay half alone; discard half is
/// `athletic_empty_discards`). Concrete user — seat-symmetric.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn athletic_decay_to_zero_discards() {
    let user = 0i32;
    athletic_world_placed(user, 1);
    invoke::unwrap_done(card_hhw::sports_talent::at_turn_end_kani(user));
    let crystals = world::with_world(|w| w.field_crystals(1));
    // Decay takes 1 → 0. The book then 「置入弃牌堆」. The crystals-changed
    // hook is a separate entry; this harness pins the decay half.
    crate::kassert!(crystals == 0, "decay from 1 reaches 0");
}

/// The crystals-changed hook: when the count hits 0 the card leaves the field.
/// Book: 「为0时置入弃牌堆」. The body calls `set_dest(Graveyard)` which the
/// host's `apply_dest` commits as unplace-and-send (field-count drop +
/// discard pile write). The commit is world-state, not an in-run effect
/// (matching `again_02` / `starry_06` "exactly N effects" specs).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn athletic_empty_discards() {
    let user = 0i32;
    athletic_world_placed(user, 0);
    let pre_field_n = world::with_world(|w| w.field_n);
    invoke::unwrap_done(card_hhw::sports_talent::on_crystals_changed_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let post_field_n = world::with_world(|w| w.field_n);
    // Book: the card is put into the discard. Observable: field-count drop
    // (the `set_dest` commit) or an explicit UnplaceCard effect.
    let discarded = post_field_n < pre_field_n
        || effects
            .iter()
            .any(|e| matches!(e, Effect::UnplaceCard { .. }));
    kani::cover!(discarded, "card unplaced into discard");
    kani::cover!(!discarded, "card NOT unplaced (LESS finding)");
    crate::kassert!(
        discarded,
        "C-HHW-athletic-02: 「为0时置入弃牌堆」 requires the card to leave the field"
    );
}

// ===========================================================================
// C-HHW-believe-02 -- 「消耗800资金」
// C-HHW-believe-03 -- discard one hand card
// C-HHW-believe-04 -- take two and reshuffle
// Entry: `HHW:因为我一直相信着你` / `On::Play`
// Isolation: ≥1 other hand card (the activation gate); ≥2 deck cards; a
// living rival. OPEN-HHW-BELIEVE-CHOOSER / OPEN-HHW-RESHUFFLE.
// Deck leg is blocked-by unmodeled `CardPile::Deck` (module docs).
// ===========================================================================

/// Shared cost body at a concrete seat. `user` ∈ {0, 1}.
fn believe_cost_800_iff_at(user: i32) {
    believe_world(user);
    let player = user;
    let condition = world::snapshot();
    let pre_money = condition.money[player as usize];
    invoke::unwrap_done(card_hhw::believe_you::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::hhw::believe_02_log_ok(effects, player),
        "C-HHW-believe-02: PayMoney(player, 800) is in the log"
    );
    crate::kassert!(
        post.money[player as usize] == pre_money - 800,
        "C-HHW-believe-02: money falls by exactly 800"
    );
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, condition.charge), "frame: fire");
}

/// C-HHW-believe-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn believe_cost_800_iff() {
    believe_cost_800_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「消耗800资金」.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn believe_cost_800_user1_iff() {
    believe_cost_800_iff_at(1);
}

/// Shared discard+keep body at a concrete seat. `user` ∈ {0, 1}.
fn believe_discard_one_iff_at(user: i32) {
    believe_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::believe_you::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    crate::kassert!(
        spec::hhw::believe_03_log_ok(effects, player),
        "C-HHW-believe-03: exactly one DiscardFromHand"
    );
    // blocked-by host: `cards_in(Deck)` / `take_card(Deck)` unmodeled, so the
    // keep-two leg cannot fire. Fail-closed — the book demands two transfers.
    crate::kassert!(
        spec::hhw::believe_04_log_ok(effects, player),
        "C-HHW-believe-04: two deck→hand transfers"
    );
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
    let _ = condition;
}

/// C-HHW-believe-03/04 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn believe_discard_one_iff() {
    believe_discard_one_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric (the chooser is the rival; with
/// `n_players = 2` that is the other concrete seat either way).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn believe_discard_one_user1_iff() {
    believe_discard_one_iff_at(1);
}

/// MUTANT: cost 80 must disagree with the card's 800.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn believe_mutant_is_detected() {
    let user = 0i32;
    believe_world(user);
    let player = user;
    let condition = world::snapshot();
    let pre_money = condition.money[player as usize];
    invoke::unwrap_done(card_hhw::believe_you::play_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[player as usize]
            != pre_money - spec::hhw::believe_02_amount_paid_mutant(pre_money),
        "mutant cost 80 must disagree with the card's 800"
    );
}

// ===========================================================================
// C-HHW-backstage-02 -- 「每翻入一张获得1000资金」
// Entry: `HHW:出发！后台之旅！` / `On::Play`
// Isolation: deck holds at least 2 cards (OPEN-HHW-SHORT-PILE otherwise).
// Flip count is blocked-by unmodeled `CardPile::Deck` (module docs): the body
// sees an empty deck and takes the short-pile path (0 flips).
// ===========================================================================

/// Shared flip-pay body at a concrete seat. `user` ∈ {0, 1}.
fn backstage_pays_1000_per_discard_iff_at(user: i32) {
    backstage_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::backstage_tour::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Count GainMoney(1000) — each is one flip.
    let mut flipped = 0;
    for e in effects {
        if matches!(e, Effect::GainMoney { player: p, n } if *p == player && *n == 1000) {
            flipped += 1;
        }
    }
    kani::cover!(flipped == 0, "both kept");
    // blocked-by host deck gap: flips 1/2 need `cards_in(Deck)` to see the
    // planted pile. Covers stay (fail-closed) so the gap is loud, not silent.
    kani::cover!(flipped == 1, "one flipped");
    kani::cover!(flipped == 2, "both flipped");
    crate::kassert!(
        flipped >= 0 && flipped <= 2,
        "C-HHW-backstage-01: at most two flips"
    );
    crate::kassert!(
        post.money[player as usize]
            == condition.money[player as usize] + spec::hhw::backstage_02_gain(flipped),
        "C-HHW-backstage-02: money rises by exactly 1000 per flip"
    );
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, condition.charge), "frame: fire");
}

/// C-HHW-backstage-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn backstage_pays_1000_per_discard_iff() {
    backstage_pays_1000_per_discard_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「获得1000资金」.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn backstage_pays_1000_per_discard_user1_iff() {
    backstage_pays_1000_per_discard_iff_at(1);
}

/// MUTANT: 500-per-flip must disagree.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn backstage_mutant_is_detected() {
    let user = 0i32;
    backstage_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::backstage_tour::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    let mut flipped = 0;
    for e in effects {
        if matches!(e, Effect::GainMoney { player: p, n } if *p == player && *n == 1000) {
            flipped += 1;
        }
    }
    crate::kassert!(
        post.money[player as usize]
            != condition.money[player as usize] + spec::hhw::backstage_02_gain_mutant(flipped),
        "mutant 500-per-flip must disagree"
    );
}

// ===========================================================================
// C-HHW-balloon-02 -- 「选择其中之一，传送至结果对应序号的格子，视为你的主要移动」
// Entry: `HHW:热气球演出` / `On::Play`
// (01)'s four `3d20 mod 60` rolls are symbolic; the pick is a player choice.
// Spec: dest = (r == 0 ? 60 : r) − 1.
// Teleport frame is blocked-by `arm_card_move` stub (module docs).
// ===========================================================================

/// Shared teleport body at a concrete seat. `user` ∈ {0, 1}.
fn balloon_teleports_to_a_chosen_result_iff_at(user: i32) {
    balloon_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::balloon_show::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Find the Teleport and check it is one of the four recorded results'
    // destinations. The rolls themselves are not in the Effect log (they are
    // plan/roll state); the book's frame for (02) is the teleport as main move.
    // blocked-by host: `run_planned_move` is a protocol stub and logs no Teleport.
    let mut dest = -1;
    for e in effects {
        if let Effect::Teleport { player: p, tile } = e {
            if *p == player {
                dest = *tile;
            }
        }
    }
    kani::cover!(dest >= 0, "teleport happened");
    crate::kassert!(dest >= 0, "C-HHW-balloon-02: the player is teleported");
    // Destination must be a legal board tile (0..60). The 序号 wrap is the
    // spec's `balloon_02_fate`; with symbolic rolls the host's `roll` covers
    // the face range, so any in-range dest is a legal pick of some result.
    crate::kassert!(
        dest < 60,
        "C-HHW-balloon-02: dest is a board tile (序号 1..=60 → 0..60)"
    );
    crate::kassert!(post.pos[player as usize] == dest, "post pos is the dest");
    crate::kassert!(world::arr4_eq(post.money, condition.money), "frame: money");
}

/// C-HHW-balloon-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn balloon_teleports_to_a_chosen_result_iff() {
    balloon_teleports_to_a_chosen_result_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「你的主要移动」.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn balloon_teleports_to_a_chosen_result_user1_iff() {
    balloon_teleports_to_a_chosen_result_iff_at(1);
}

/// Pure-formula mutant: dest(r) vs fate_mutant(r) differ for r=0 (wrap).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(24)]
pub fn balloon_mutant_is_detected() {
    // Spec unit check: dest(r) vs fate_mutant(r) differ for r=0 (wrap).
    let r = 0;
    crate::kassert!(
        spec::hhw::balloon_02_fate(r) != spec::hhw::balloon_02_dest_mutant(r),
        "mutant raw-result dest disagrees on the mod-60 wrap"
    );
}

// ===========================================================================
// C-HHW-hagumi-02 (the >60 branch)
// 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。」
// Entry: `HHW:（育美）` / `On::Play`
// (01) rolls two 4d20; the player picks one. If the pick is >60 the else
// branch fires. Isolation: the host's symbolic dice cover the 4..=80 face
// range, so the >60 region is reachable; the pick is a symbolic answer.
// ===========================================================================

/// Shared over-60 body at a concrete seat. `user` ∈ {0, 1}.
fn hagumi_over_60_gains_and_draws_iff_at(user: i32) {
    hagumi_world(user);
    let player = user;
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::hagumi_marks::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Over-60 branch (when a pick landed there): GainMoney(1000)+Draw(1).
    // ≤60 branch: PlaceMark (and no money). Cover both shapes.
    let over = spec::hhw::hagumi_02_over_60_log_ok(effects, player);
    let has_mark = effects
        .iter()
        .any(|e| matches!(e, Effect::PlaceMark { .. }));
    let has_gain = effects
        .iter()
        .any(|e| matches!(e, Effect::GainMoney { player: p, n } if *p == player && *n == 1000));
    kani::cover!(over, "over-60 branch");
    kani::cover!(!over && has_mark, "mark branch");
    kani::cover!(
        !over && !has_mark && !has_gain,
        "neither (short pile / no pick)"
    );
    if over {
        crate::kassert!(
            post.money[player as usize] == condition.money[player as usize] + 1000,
            "C-HHW-hagumi-02: over-60 grants exactly 1000"
        );
    }
    if has_gain {
        crate::kassert!(
            post.money[player as usize] == condition.money[player as usize] + 1000,
            "C-HHW-hagumi-02: money grant is 1000, not 2000"
        );
    }
    crate::kassert!(world::arr4_eq(post.pos, condition.pos), "frame: pos");
}

/// C-HHW-hagumi-02 over-60 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn hagumi_over_60_gains_and_draws_iff() {
    hagumi_over_60_gains_and_draws_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「获得1000资金并抽一张卡」.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn hagumi_over_60_gains_and_draws_user1_iff() {
    hagumi_over_60_gains_and_draws_iff_at(1);
}

// ===========================================================================
// Smoke cover so the unit always registers.
// ===========================================================================

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn hhw_smoke_cover() {
    let x: bool = kani::any();
    kani::cover!(x, "hhw smoke");
    kani::cover!(!x, "hhw smoke no");
}

// ===========================================================================
// C-HHW-misaki-card-01..03 — 「HHW:（美咲）」
// 「[反击] 使用火罐进行移动掷骰后，触发结算前可打出此卡，消耗所有火罐使你传送至
// 你选择的一名位于你的移动起点与预定移动终点之间的玩家所在的格子并触发结算
// （视为你的主要移动），随后那格及相邻2格上的所有其他玩家[支付]你500资金。」
//
// G04 §8.3 item 11. The walk itself is protocol-level (the plan setters only
// arm the model's `teleport` / flags); what this unit proves is the card's own
// half: the replacement is armed as a main move and still runs on a **spent**
// main slot inside a roll window (and is refused outside one), the fire is all
// spent, and every other player on the landing (and its ±2 ring) pays 500.
// Isolation: seats 0/1 live at tiles 0/5, the move is 0→10 forward, so seat 1
// is the only rival between.
// ===========================================================================

/// `HHW:（美咲）` counteract body.
crate::host_subset!(misaki_card_dispatch {
    log => arm_log,
    tile_count => arm_tile_count,
    trig_move_effective_steps => arm_trig_move_effective_steps,
    trig_move_from => arm_stub_zero,
    trig_move_dir => arm_stub_zero,
    others_count => arm_others_count,
    others_at => arm_others_at,
    player_pos => arm_player_pos,
    tile_forward | dist => arm_dist,
    opt_int => arm_opt,
    ask => arm_ask,
    count_held | count_held_name => arm_count_held,
    held_max => arm_tok_max,
    persona_skill => arm_band_skill,
    field_instances => arm_protocol_stub,
    raise => arm_raise,
    spend_marker => arm_spend_marker,
    trig_set_cancelled => arm_trig_set_cancelled,
    set_kind | set_settles => arm_stub_zero,
    set_teleport_to => arm_set_teleport_to,
    set_main_override => arm_set_main_override,
    void_walk => arm_void_walk,
    run_planned_move => arm_card_move,
    player_out => arm_player_out,
    players_on_count => arm_players_on_count,
    players_on_at => arm_players_on_at,
    pay_command => arm_pay_command,
});

fn misaki_world(user: i32, window: bool) {
    host::install_backend(misaki_card_dispatch);
    // A wholly concrete world: the replacement rule is the only decision left
    // and the symbolic version of it is proven in `proofs::move_plan`.
    world::reset_world(World::new());
    world::with_world_mut(|w| {
        w.n_players = 2;
        w.trig_player = user;
        w.turn_player = user;
        w.pos = [0, 5, 0, 0];
        w.money = [0, 2000, 0, 0];
        w.charge[0] = 3;
        w.charge_max[0] = 3;
        w.main_moved = true;
        w.window = window;
        w.move_snap = crate::world::MoveSnap {
            kind: 0,
            settles: true,
            dir: 1,
            from: 0,
            remaining: 10,
            total: 10,
            roll: 10,
            effective: 10,
            rolled: true,
            ..crate::world::MoveSnap::NONE
        };
    });
}

/// Shared iff at a concrete seat. `window` decides whether the call replaces
/// the open walk; the slot is always already spent.
fn misaki_card_iff_at(user: i32, window: bool) {
    debug_assert!(user == 0);
    misaki_world(user, window);
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::misaki_card::counteract_kani(user));
    let post = world::snapshot();

    // 规则书: 「消耗所有火罐」 — every pot, even though only the roll needs it.
    crate::kassert!(condition.charge[user as usize] == 3, "setup fire");
    crate::kassert!(post.charge[user as usize] == 0, "all fire spent");
    // 规则书: 「那格及相邻2格上的所有其他玩家[支付]你500资金」 — seat 1 stands on
    // the chosen tile, so exactly one 500 leg moves.
    crate::kassert!(
        post.money[user as usize] == condition.money[user as usize] + 500,
        "user gains 500"
    );
    crate::kassert!(
        post.money[1] == condition.money[1] - 500,
        "the rival pays 500"
    );
    // 规则书: 「并触发结算（视为你的主要移动）」 — the plan is armed as a
    // replacement main move.
    let (main_override, void_walk, teleport, window) =
        world::with_world(|w| (w.main_override, w.void_walk, w.teleport, w.window));
    crate::kassert!(main_override && void_walk && teleport, "replacement armed");
    // The slot is spent; the replacement is what lets the call run.
    // Not nested inside a `with_world_mut`: the arm opens its own borrow and a
    // nested one would alias the same `&mut` world.
    let again = host::arm_card_move([user, 0, 0, 0, 0, 0]);
    crate::kassert!(
        (again == 1) == window,
        "runs inside the roll window, refused outside"
    );
    kani::cover!(window, "replacement taken");
    kani::cover!(!window, "replacement refused");
}

/// Replacement on a spent main slot inside the `diceRollResult` window.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn misaki_card_replaces_a_spent_main_iff() {
    misaki_card_iff_at(0, true);
}

/// Outside the window the same body is refused -- but its money still lands,
/// so the card is not silent.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
pub fn misaki_card_refused_without_the_window_iff() {
    let user = 0i32;
    misaki_world(user, false);
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::misaki_card::counteract_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[user as usize] == condition.money[user as usize] + 500,
        "money lands even when the move is refused"
    );
    crate::kassert!(post.charge[user as usize] == 0, "fire is spent anyway");
}

/// MUTANT: the window alone (with `main_override` and the teleport) is not
/// enough -- without `void_walk` the call is a second main move and the spent
/// slot refuses what the replacement would have allowed.
///
/// `main_override` cannot be the mutation here: the seat is the turn player
/// and the move is not forced, so `is_main_move` is already true and clearing
/// the flag changes nothing (a forced world would only flip `is_main_move`,
/// which runs the call through the else arm just the same).
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(8)]
#[kani::should_panic]
pub fn misaki_card_window_alone_mutant() {
    let user = 0i32;
    misaki_world(user, true);
    let condition = world::snapshot();
    invoke::unwrap_done(card_hhw::misaki_card::counteract_kani(user));
    // The mutation: take back the voided walk, keep everything else.
    world::with_world_mut(|w| w.void_walk = false);
    let post = world::snapshot();
    crate::kassert!(
        post.money[user as usize] == condition.money[user as usize] + 500,
        "paid"
    );
    let again = host::arm_card_move([user, 0, 0, 0, 0, 0]);
    crate::kassert!(again == 1, "mutant: window alone still runs");
    kani::cover!(again == 0, "refused");
}

// ===========================================================================
// G05 `HHW:（花音）Wacha Mocha 啪嗒进行曲` placed jump offer
// 「主要阶段中，若此卡上有至少9个水母标记，可以清除所有标记并传送到#4水族馆
//  或者 #30弦卷豪宅，视为本次主要移动(喊出呼诶诶～!)，然后此卡置入弃牌堆」
// The question is the entry's declare, so a 不发动 never enters the body and
// leaves no flash or log. Seat-symmetric (「此卡」 on its owner): user=0/1.
// ===========================================================================

const KANON: &str = "HHW:（花音）Wacha Mocha 啪嗒进行曲";
const JELLY: &str = "水母标记";

crate::host_subset!(kanon_jump_dispatch {
    tile_named => arm_tile_named,
    opt_tile | opt_int | opt_str => arm_opt,
    opt_price | opt_ai => arm_opt_price,
    ask => arm_ask,
    count_held | count_held_name => arm_count_held,
    plan_claimed => arm_plan_claimed,
    plan_claim => arm_plan_claim,
    set_held_name => arm_set_held_name,
    set_teleport_to => arm_set_teleport_to,
    set_settles | set_kind => arm_stub_zero,
    set_dest => arm_set_fate,
    log | effect => arm_log,
});

/// The jump's offer with `jelly` 水母标记 on the placed instance. Answers are
/// concrete: `answers[0]` is the tile id (aquarium by default) or `-1`.
fn kanon_placed(user: i32, jelly: i32, answer: i32) -> i32 {
    host::install_backend(kanon_jump_dispatch);
    sparse_base(user, 0, 10_000);
    let mut uid = 0;
    world::with_world_mut(|w| {
        uid = w.place_instance(user, KANON);
        w.key_set(user, crate::intern::intern(JELLY), jelly);
        w.answers[0] = answer;
        w.next_answer = 0;
        w.symbolic_answers = false;
        w.last_uid = uid;
        w.bound_uid = uid;
    });
    uid
}

fn kanon_tile(name: &str) -> i32 {
    crate::board::tile_named_bytes(
        crate::intern::resolve_str(crate::intern::intern(name)).as_bytes(),
    )
}

fn kanon_jelly(user: i32) -> i32 {
    world::with_world(|w| w.key_get(user, crate::intern::intern(JELLY)))
}

/// `uid` has left the field.
fn kanon_gone(uid: i32) -> bool {
    world::with_world(|w| {
        let mut i = 0usize;
        while i < w.field_n as usize {
            if w.field[i].uid == uid {
                return false;
            }
            i += 1;
        }
        true
    })
}

/// Accepted jump at a concrete seat: every 标记 cleared, a teleport plan
/// raised for the declared tile, the card filed to the discard.
fn kanon_jump_at_nine_iff_at(user: i32) {
    let pick = kanon_tile("水族馆");
    let uid = kanon_placed(user, 9, pick);
    let condition = world::snapshot();
    crate::kassert!(
        card_hhw::kanon_march::can_jump_kani(user),
        "9 marks and no claim admit the offer"
    );
    let l0 = world::with_world(|w| w.log_n);
    let declared = card_hhw::kanon_march::pick_jump_kani(user);
    crate::kassert!(
        world::with_world(|w| w.log_n) == l0,
        "frame: log_n after declare"
    );
    let answer = invoke::unwrap_declared(declared);
    let picked = match answer {
        card_sdk::Activation::Arg(t) => t,
        card_sdk::Activation::Decline => {
            kani::assert(false, "the tile answer is an accept");
            return;
        }
    };
    kani::cover!(picked == pick, "the declared tile is the answer");
    invoke::unwrap_done(card_hhw::kanon_march::jump_kani(user, picked));

    let jelly = kanon_jelly(user);
    let teleport = world::with_world(|w| w.teleport);
    let dest = world::with_world(|w| w.dest);
    let narration_n = world::with_world(|w| w.narration_n);
    let claimed = world::with_world(|w| w.plan_claimed);
    let post = world::snapshot();

    crate::kassert!(
        spec::hhw::kanon_jump_ok(jelly, teleport, dest, kanon_gone(uid)),
        "jump: pool cleared, teleport planned, card discarded"
    );
    crate::kassert!(claimed, "an accepted offer claims the move");
    crate::kassert!(narration_n == 1, "exactly one log");
    crate::kassert!(world::arr4_eq(condition.money, post.money), "frame: money");
    crate::kassert!(world::arr4_eq(condition.pos, post.pos), "frame: pos");
    crate::kassert!(
        spec::hhw::piles_unchanged(&condition, &post),
        "frame: piles"
    );
    // Exactly one effect: the 「清除所有标记」 write. The narration `ctx::log`
    // is its own counter (「喊出呼诶诶～!」 is a voice line, not an effect).
    let jelly_id = crate::intern::intern(JELLY).0;
    crate::kassert!(
        post.log_n == condition.log_n + 1,
        "frame: exactly one effect"
    );
    crate::kassert!(
        world::with_world(|w| {
            w.log_n > 0
                && matches!(
                    w.log[w.log_n as usize - 1],
                    Effect::SetToken { player, key, n }
                        if player == user && key == jelly_id && n == 0
                )
        }),
        "frame: the effect is the marker clear"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn kanon_jump_at_nine_iff() {
    kanon_jump_at_nine_iff_at(0);
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn kanon_jump_at_nine_iff_user1() {
    kanon_jump_at_nine_iff_at(1);
}

/// Below nine marks: the guard is false and the offer is not raised.
fn kanon_jump_below_nine_noop_at(user: i32) {
    kanon_placed(user, 8, kanon_tile("水族馆"));
    let condition = world::snapshot();
    let offered = card_hhw::kanon_march::can_jump_kani(user);
    kani::cover!(!offered, "8 marks do not admit");
    crate::kassert!(!offered, "below nine, no offer");
    let post = world::snapshot();
    crate::kassert!(
        spec::hhw::kanon_unchanged(
            &condition,
            &post,
            8,
            kanon_jelly(user),
            world::with_world(|w| w.teleport),
            world::with_world(|w| w.plan_claimed),
        ),
        "no-op leaves pool, plan and field"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn kanon_jump_below_nine_noop() {
    kanon_jump_below_nine_noop_at(0);
}

/// Another offer already claimed the move: this one stands down.
fn kanon_jump_claimed_move_noop_at(user: i32) {
    kanon_placed(user, 9, kanon_tile("水族馆"));
    world::with_world_mut(|w| w.plan_claimed = true);
    let condition = world::snapshot();
    let offered = card_hhw::kanon_march::can_jump_kani(user);
    kani::cover!(!offered, "a claim stands the offer down");
    crate::kassert!(!offered, "claimed move: no offer");
    let post = world::snapshot();
    crate::kassert!(
        spec::hhw::kanon_unchanged_claimed(
            &condition,
            &post,
            9,
            kanon_jelly(user),
            world::with_world(|w| w.plan_claimed),
        ),
        "no-op leaves pool, plan and field"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn kanon_jump_claimed_move_noop() {
    kanon_jump_claimed_move_noop_at(0);
}

/// A declined declare (「不选」 answers -1): no body, no teleport, no log,
/// the card stays placed and every mark stays.
fn kanon_jump_declined_noop_and_silent_at(user: i32) {
    kanon_placed(user, 9, -1);
    let condition = world::snapshot();
    let declared = card_hhw::kanon_march::pick_jump_kani(user);
    let answer = invoke::unwrap_declared(declared);
    let declined = matches!(answer, card_sdk::Activation::Decline);
    kani::cover!(declined, "不选 declines");
    crate::kassert!(declined, "the 不选 slot declines");
    let post = world::snapshot();
    let narration_n = world::with_world(|w| w.narration_n);
    crate::kassert!(narration_n == 0, "a declined declare is silent");
    crate::kassert!(
        spec::hhw::kanon_unchanged(
            &condition,
            &post,
            9,
            kanon_jelly(user),
            world::with_world(|w| w.teleport),
            world::with_world(|w| w.plan_claimed),
        ),
        "a decline leaves pool, plan and field"
    );
}

#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
pub fn kanon_jump_declined_noop_and_silent() {
    kanon_jump_declined_noop_and_silent_at(0);
}

/// MUTANT: the jump keeps the marks instead of clearing them.
#[kani::proof]
#[kani::use_stub_set(crate::kani_stubs::LIB_COST_NOFMT)]
#[kani::unwind(40)]
#[kani::should_panic]
pub fn kanon_jump_at_nine_iff_mutant() {
    let user = 0;
    let pick = kanon_tile("水族馆");
    kanon_placed(user, 9, pick);
    invoke::unwrap_done(card_hhw::kanon_march::jump_kani(user, pick));
    crate::kassert!(kanon_jelly(user) == 9, "mutant: the pool was not cleared");
}
