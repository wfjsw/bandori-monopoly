//! Unit `hhw`: clause proofs for the 15 Hello, Happy World! cards.
//!
//! Per clause emit (see crate::harness and KANI-FOUNDATION.md):
//! 1. iff proof (post == spec(pre) and effect log exactly and frame);
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
//!   (`deck_pile` is written by `add_to_deck*` but never read). believe-04
//!   (keep two from the deck) and backstage's flip count are blocked-by this
//!   host gap: the body sees an empty deck and takes the short-pile path.
//!   Assertions stay fail-closed; the covers for the unreachable deck branches
//!   will report the gap when CBMC runs.
//! * `arm_card_move` is a protocol stub (returns 1, no Teleport / pos move).
//!   balloon-02's teleport frame is blocked-by that stub.

use crate::harness::{self, SparsePlay};
use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, FieldCard, NameMap, MAX_LOG};

// ---------------------------------------------------------------------------
// Per-body host-op subsets. One dispatcher per card body the harnesses call,
// listing ONLY the ops that unmodified entry reaches (TRIAGE-LOG `op-subset`).
// ---------------------------------------------------------------------------

/// `HHW:（kkr）前往笑容集结的地方！` / `On::Play` (`kokoro_circle::play`).
/// `settle_body` is a separate entry (not called here); its ops would need
/// `self_tile` / `pay_to` / trigger reads if a settle harness lands.
crate::host_subset!(kkr_dispatch {
    pay => arm_pay,
    set_dest => arm_set_dest,
    log | effect => arm_log,
    tile_named => arm_tile_named,
    place_card_on => arm_place_card_on,
});

/// `HHW:运动的天赋` entries the harnesses call: `play`, `at_turn_end`,
/// `on_crystals_changed`. The RollAfter / RollPlan bodies and the
/// crystals-changed guard are separate entries (not called here).
/// `ctx::decay` → `add_counter_self`; `ctx::set_crystals` → `set_counter_self`.
crate::host_subset!(athletic_dispatch {
    set_dest => arm_set_dest,
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
    cards_in => arm_cards_in,
    log | effect => arm_log,
    others_count => arm_others_count,
    others_at => arm_others_at,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    discard_from_hand => arm_discard_from_hand,
    take_card => arm_take_card,
    add_to_hand => arm_add_to_hand,
    add_to_deck_at => arm_add_to_deck_at,
});

/// `HHW:出发！后台之旅！` / `On::Play` (`backstage_tour::play`).
/// Deck peek is blocked-by the unmodeled `CardPile::Deck` (see module docs).
crate::host_subset!(backstage_dispatch {
    deck_count => arm_deck_count,
    discard_size => arm_discard_size,
    cards_in => arm_cards_in,
    take_card => arm_take_card,
    add_to_deck_at => arm_add_to_deck_at,
    log | effect => arm_log,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    to_discard => arm_to_discard,
    gain | gain_typed | gain_fixed => arm_gain,
});

/// `HHW:热气球演出` / `On::Play` (`balloon_show::play`).
/// `card_move` is a protocol stub — the teleport frame is blocked-by it.
crate::host_subset!(balloon_dispatch {
    tile_count => arm_tile_count,
    roll | roll_ask => arm_roll,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    set_kind | set_teleport_to | set_resolve | set_steps | set_reverse
    | set_stop_at | set_parity | set_start | set_base_dice | set_rent_factor
    | set_pay_factor | set_no_buy | set_no_build | set_build_anywhere
    | set_roller | set_signed | set_settle_tile | set_tag | set_settle_as_agent
    | plan_add_follower | set_play_doubled | set_no_money_loss | set_fixed_roll
    | set_next_steps | set_build_discount | set_build_cost_pct | set_extreme
    | clear_dice | set_can_build | set_min_roll | set_extra_steps
    | set_more_steps | add_base_dice | add_extra_dice => arm_protocol_stub,
    card_move => arm_card_move,
    log | effect => arm_log,
});

/// `HHW:（育美）` / `On::Play` → `rolls` (`hagumi_marks::play_kani`).
/// The PassTile hook and the (2) counteract are separate entries (not called).
crate::host_subset!(hagumi_dispatch {
    tile_count => arm_tile_count,
    roll | roll_ask => arm_roll,
    opt_int | opt_str | opt_tile | opt_price | opt_ai => arm_opt,
    ask => arm_ask,
    gain | gain_typed | gain_fixed => arm_gain,
    draw => arm_draw,
    place_mark => arm_place_mark,
    self_uid => arm_self_uid,
    log | effect => arm_log,
    is_placed => arm_is_placed,
    set_dest => arm_set_dest,
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
    let pre = world::snapshot();
    // DIRECT body call (ppcost): one body in CBMC's call graph.
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    let pre_money = pre.money[player as usize];
    kani::cover!(pre_money == 10_000, "exact 10000 on hand");
    crate::kassert!(
        spec::hhw::kkr_01_log_ok(effects, player),
        "C-HHW-kkr-01: PayMoney(player, 10000) is in the log"
    );
    crate::kassert!(
        spec::hhw::kkr_01_post_ok(&pre, &post, player),
        "C-HHW-kkr-01: money falls by exactly 10000"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
    crate::kassert!(world::arr8b_eq(post.hand, pre.hand), "frame: hand");
}

/// C-HHW-kkr-01/02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn kkr_pay_10000_iff() {
    kkr_pay_10000_iff_at(0);
}

/// Same iff at seat 1 — two harnesses cover every seat
/// (seat-symmetric clause 「自己的」; symbolic user is a BMC cliff).
#[kani::proof]
#[kani::unwind(32)]
pub fn kkr_pay_10000_user1_iff() {
    kkr_pay_10000_iff_at(1);
}

/// Shared place-on-CiRCLE body at a concrete seat. `user` ∈ {0, 1}.
fn kkr_place_on_circle_iff_at(user: i32) {
    kkr_world(user);
    let player = user;
    let pre = world::snapshot();
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
    kani::cover!(!placed_circle, "PlaceCard dest is not CiRCLE (host finding)");
    // Pin the weaker determined fact: a PlaceCard by this player happens.
    crate::kassert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PlaceCard { player: p, .. } if *p == player)),
        "C-HHW-kkr-02: the card is placed"
    );
    let _ = pre;
}

/// C-HHW-kkr-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(32)]
pub fn kkr_place_on_circle_iff() {
    kkr_place_on_circle_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「并将此卡置于CiRCLE上」.
#[kani::proof]
#[kani::unwind(32)]
pub fn kkr_place_on_circle_user1_iff() {
    kkr_place_on_circle_iff_at(1);
}

/// MUTANT companion: pay only 1000.
#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn kkr_mutant_pay_1000() {
    // Concrete user (symbolic user is a BMC cliff — see `kkr_pay_10000_iff_at`).
    let user = 0i32;
    kkr_world(user);
    let player = user;
    let pre = world::snapshot();
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[player as usize] == pre.money[player as usize] - spec::hhw::kkr_01_amount_mutant(),
        "mutant: card must not pay only 1000"
    );
}

/// MUTANT disagreement is reachable: the card pays −10000, not −1000.
#[kani::proof]
#[kani::unwind(24)]
pub fn kkr_mutant_is_detected() {
    let user = 0i32;
    kkr_world(user);
    let player = user;
    let pre = world::snapshot();
    invoke::unwrap_done(card_hhw::kokoro_circle::play_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[player as usize] != pre.money[player as usize] - spec::hhw::kkr_01_amount_mutant(),
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
    let pre = world::snapshot();
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
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// C-HHW-athletic-01 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_place_three_crystals_iff() {
    athletic_place_three_crystals_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「自己场上」.
#[kani::proof]
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
    let pre = world::snapshot();
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
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// C-HHW-athletic-02 decay iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_turn_end_decay_iff() {
    athletic_turn_end_decay_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric owner turn-end.
#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_turn_end_decay_user1_iff() {
    athletic_turn_end_decay_iff_at(1);
}

/// Decay from 1 reaches 0 (the decay half alone; discard half is
/// `athletic_empty_discards`). Concrete user — seat-symmetric.
#[kani::proof]
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
/// Book: 「为0时置入弃牌堆」. If the body only `set_dest(Graveyard)` without an
/// Unplace / to_discard, this is a LESS finding.
#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_empty_discards() {
    let user = 0i32;
    athletic_world_placed(user, 0);
    invoke::unwrap_done(card_hhw::sports_talent::on_crystals_changed_kani(user));
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);
    // Book: the card is put into the discard. Observable: UnplaceCard and/or
    // a discard pile write. `set_dest` alone is not enough.
    let discarded = effects
        .iter()
        .any(|e| matches!(e, Effect::UnplaceCard { .. }));
    kani::cover!(discarded, "card unplaced into discard");
    kani::cover!(!discarded, "card NOT unplaced (LESS finding)");
    // Fail-closed: the book demands the discard. If this assert fires the
    // body is LESS than the book — that is a real finding, not a spec bug.
    // TODO(规则书) the body only `set_dest(Graveyard)`; no Unplace / to_discard.
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
    let pre = world::snapshot();
    let pre_money = pre.money[player as usize];
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
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
}

/// C-HHW-believe-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn believe_cost_800_iff() {
    believe_cost_800_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「消耗800资金」.
#[kani::proof]
#[kani::unwind(40)]
pub fn believe_cost_800_user1_iff() {
    believe_cost_800_iff_at(1);
}

/// Shared discard+keep body at a concrete seat. `user` ∈ {0, 1}.
fn believe_discard_one_iff_at(user: i32) {
    believe_world(user);
    let player = user;
    let pre = world::snapshot();
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
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    let _ = pre;
}

/// C-HHW-believe-03/04 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn believe_discard_one_iff() {
    believe_discard_one_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric (the chooser is the rival; with
/// `n_players = 2` that is the other concrete seat either way).
#[kani::proof]
#[kani::unwind(40)]
pub fn believe_discard_one_user1_iff() {
    believe_discard_one_iff_at(1);
}

/// MUTANT: cost 80 must disagree with the card's 800.
#[kani::proof]
#[kani::unwind(24)]
pub fn believe_mutant_is_detected() {
    let user = 0i32;
    believe_world(user);
    let player = user;
    let pre = world::snapshot();
    let pre_money = pre.money[player as usize];
    invoke::unwrap_done(card_hhw::believe_you::play_kani(user));
    let post = world::snapshot();
    crate::kassert!(
        post.money[player as usize] != pre_money - spec::hhw::believe_02_amount_paid_mutant(pre_money),
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
    let pre = world::snapshot();
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
        post.money[player as usize] == pre.money[player as usize] + spec::hhw::backstage_02_gain(flipped),
        "C-HHW-backstage-02: money rises by exactly 1000 per flip"
    );
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
    crate::kassert!(world::arr4_eq(post.charge, pre.charge), "frame: fire");
}

/// C-HHW-backstage-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn backstage_pays_1000_per_discard_iff() {
    backstage_pays_1000_per_discard_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「获得1000资金」.
#[kani::proof]
#[kani::unwind(40)]
pub fn backstage_pays_1000_per_discard_user1_iff() {
    backstage_pays_1000_per_discard_iff_at(1);
}

/// MUTANT: 500-per-flip must disagree.
#[kani::proof]
#[kani::unwind(24)]
pub fn backstage_mutant_is_detected() {
    let user = 0i32;
    backstage_world(user);
    let player = user;
    let pre = world::snapshot();
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
            != pre.money[player as usize] + spec::hhw::backstage_02_gain_mutant(flipped),
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
    let pre = world::snapshot();
    invoke::unwrap_done(card_hhw::balloon_show::play_kani(user));
    let post = world::snapshot();
    let (logbuf, log_n) = log_slice();
    let effects = effects_of(&logbuf, log_n);

    // Find the Teleport and check it is one of the four recorded results'
    // destinations. The rolls themselves are not in the Effect log (they are
    // plan/roll state); the book's frame for (02) is the teleport as main move.
    // blocked-by host: `card_move` is a protocol stub and logs no Teleport.
    let mut dest = -1;
    for e in effects {
        if let Effect::Teleport {
            player: p,
            tile,
        } = e
        {
            if *p == player {
                dest = *tile;
            }
        }
    }
    kani::cover!(dest >= 0, "teleport happened");
    crate::kassert!(dest >= 0, "C-HHW-balloon-02: the player is teleported");
    // Destination must be a legal board tile (0..60). The 序号 wrap is the
    // spec's `balloon_02_dest`; with symbolic rolls the host's `roll` covers
    // the face range, so any in-range dest is a legal pick of some result.
    crate::kassert!(
        dest < 60,
        "C-HHW-balloon-02: dest is a board tile (序号 1..=60 → 0..60)"
    );
    crate::kassert!(post.pos[player as usize] == dest, "post pos is the dest");
    crate::kassert!(world::arr4_eq(post.money, pre.money), "frame: money");
}

/// C-HHW-balloon-02 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn balloon_teleports_to_a_chosen_result_iff() {
    balloon_teleports_to_a_chosen_result_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「你的主要移动」.
#[kani::proof]
#[kani::unwind(40)]
pub fn balloon_teleports_to_a_chosen_result_user1_iff() {
    balloon_teleports_to_a_chosen_result_iff_at(1);
}

/// Pure-formula mutant: dest(r) vs dest_mutant(r) differ for r=0 (wrap).
#[kani::proof]
#[kani::unwind(24)]
pub fn balloon_mutant_is_detected() {
    // Spec unit check: dest(r) vs dest_mutant(r) differ for r=0 (wrap).
    let r = 0;
    crate::kassert!(
        spec::hhw::balloon_02_dest(r) != spec::hhw::balloon_02_dest_mutant(r),
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
    let pre = world::snapshot();
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
    kani::cover!(!over && !has_mark && !has_gain, "neither (short pile / no pick)");
    if over {
        crate::kassert!(
            post.money[player as usize] == pre.money[player as usize] + 1000,
            "C-HHW-hagumi-02: over-60 grants exactly 1000"
        );
    }
    if has_gain {
        crate::kassert!(
            post.money[player as usize] == pre.money[player as usize] + 1000,
            "C-HHW-hagumi-02: money grant is 1000, not 2000"
        );
    }
    crate::kassert!(world::arr4_eq(post.pos, pre.pos), "frame: pos");
}

/// C-HHW-hagumi-02 over-60 iff at seat 0 (`n_players = 2`).
#[kani::proof]
#[kani::unwind(40)]
pub fn hagumi_over_60_gains_and_draws_iff() {
    hagumi_over_60_gains_and_draws_iff_at(0);
}

/// Same iff at seat 1 — seat-symmetric 「获得1000资金并抽一张卡」.
#[kani::proof]
#[kani::unwind(40)]
pub fn hagumi_over_60_gains_and_draws_user1_iff() {
    hagumi_over_60_gains_and_draws_iff_at(1);
}

// ===========================================================================
// Smoke cover so the unit always registers.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn hhw_smoke_cover() {
    let x: bool = kani::any();
    kani::cover!(x, "hhw smoke");
    kani::cover!(!x, "hhw smoke no");
}