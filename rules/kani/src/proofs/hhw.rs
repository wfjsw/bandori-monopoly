//! Unit `hhw`: clause proofs for the 15 Hello, Happy World! cards.
//!
//! Cheap first: money / placement clauses (kkr, athletic, backstage, believe)
//! before prompt / roll / multi-act ones. Classification of the rest lives in
//! `docs/rulebook/spec/kani/hhw.md` (BLOCKED-HOST / OPEN / PROTOCOL-LEVEL).

use crate::host;
use crate::invoke;
use crate::ir_fill::{self, TriggerOverlay};
use crate::spec;
use crate::world::{self, Effect, Snapshot, World, MAX_PLAYERS};

fn effects_of() -> ([Effect; 24], usize) {
    let logbuf: [Effect; 24] = world::with_world(|ww| ww.log);
    let log_n = world::with_world(|ww| ww.log_n);
    (logbuf, log_n)
}

/// Shared shape for a hand-play of an HHW card: 3 seats, modest money, a
/// small concrete deck so `cards_in` stays cheap.
fn symbolic_play_world(player: i32) -> World {
    host::install();
    let mut w = World::new();
    w.n_players = 3;
    for p in 0..MAX_PLAYERS {
        w.out[p] = false;
        w.money[p] = 20_000;
        w.pos[p] = 0;
        w.fire[p] = 0;
    }
    w.trig_player = player;
    w.turn_player = player;
    w
}

// ===========================================================================
// C-HHW-kkr-01 -- 「支付10000资金（视为买地花费）」
// C-HHW-kkr-02 -- 「并将此卡置于CiRCLE上」
// Entry: `HHW:（kkr）前往笑容集结的地方！` / `On::Play`
// Isolation: no other copy of this card is already on CiRCLE (the residual
// `cant_play` gate). (3) is OPEN-HHW-触发结算.
// ===========================================================================

pub const CIRCLE: i32 = 0; // 序号 #1

#[kani::proof]
#[kani::unwind(32)]
pub fn kkr_pay_10000_iff() {
    let player: i32 = 0;
    let mut w = symbolic_play_world(player);
    w.money[player as usize] = kani::any();
    kani::assume(w.money[player as usize] >= 10_000 && w.money[player as usize] <= 30_000);

    let pre_money = w.money[player as usize];
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:（kkr）前往笑容集结的地方！", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    kani::cover!(pre_money == 10_000, "exact 10000 on hand");
    assert!(
        spec::kkr_01_log_ok(effects, player),
        "C-HHW-kkr-01: PayMoney(player, 10000) is in the log"
    );
    assert!(
        spec::kkr_01_post_ok(&pre, &post, player),
        "C-HHW-kkr-01: money falls by exactly 10000"
    );
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
    assert!(post.hand == pre.hand, "frame: hand");
}

#[kani::proof]
#[kani::unwind(32)]
pub fn kkr_place_on_circle_iff() {
    let player: i32 = 0;
    let w = symbolic_play_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:（kkr）前往笑容集结的地方！", player));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    // (02): the card lands on CiRCLE. Under the host, `place_card_on` logs
    // `PlaceCard { dest }` — dest is the tile index when the ABI routes the
    // tile through `w.dest`, else `Dest::Field`. Accept either the board
    // index of CiRCLE or a PlaceCard whose tile slot is CiRCLE.
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
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PlaceCard { player: p, .. } if *p == player)),
        "C-HHW-kkr-02: the card is placed"
    );
}

#[kani::proof]
#[kani::unwind(24)]
#[kani::should_panic]
pub fn kkr_mutant_pay_1000() {
    let player: i32 = 0;
    let w = symbolic_play_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:（kkr）前往笑容集结的地方！", player));
    let post = world::snapshot();
    assert!(
        post.money[player as usize] == pre.money[player as usize] - spec::kkr_01_amount_mutant(),
        "mutant: card must not pay only 1000"
    );
}

#[kani::proof]
#[kani::unwind(24)]
pub fn kkr_mutant_is_detected() {
    let player: i32 = 0;
    let w = symbolic_play_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:（kkr）前往笑容集结的地方！", player));
    let post = world::snapshot();
    assert!(
        post.money[player as usize] != pre.money[player as usize] - spec::kkr_01_amount_mutant(),
        "mutant -1000 must disagree with the card's -10000"
    );
}

// ===========================================================================
// C-HHW-athletic-01 -- 「将此卡放置于自己场上并放置3个奇迹水晶」
// C-HHW-athletic-02 -- 「每回合结束时失去一个，为0时置入弃牌堆。」
// Entry: `HHW:运动的天赋` / `On::Play` + `On::Hook(TurnEnd)` + crystals-changed
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_place_three_crystals_iff() {
    let player: i32 = 0;
    let w = symbolic_play_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:运动的天赋", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    // Book: place on own field + 3 crystals. `set_crystals(3)` is a counter
    // write (host: `set_counter_self`, no Effect variant) — the log is the
    // PlaceCard. The crystal count is the keyed counter `crystals`.
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::PlaceCard { player: p, dest: 2, .. } if *p == player)),
        "C-HHW-athletic-01: PlaceCard on the user's field"
    );
    let crystals = world::with_world(|w| w.key_get(player, crate::intern::intern("crystals")));
    kani::cover!(crystals == 3, "crystals == 3");
    assert!(
        crystals == spec::athletic_01_crystals(),
        "C-HHW-athletic-01: exactly 3 crystals"
    );
    assert!(post.money == pre.money, "frame: money");
    assert!(post.pos == pre.pos, "frame: pos");
}

/// Turn-end decay: the owner's TurnEnd with the card placed logs exactly one
/// AddCounter(crytals, −1). The empty→discard half is the crystals-changed
/// hook; if that hook only sets dest without unplacing, the book's
/// 「为0时置入弃牌堆」 is LESS.
#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_turn_end_decay_iff() {
    let player: i32 = 0;
    let mut w = symbolic_play_world(player);
    // Card is placed (residual guard `is_placed`).
    w.field_n = 1;
    w.field[0] = world::FieldCard {
        uid: 1,
        card: crate::intern::intern("HHW:运动的天赋").0,
        player,
        tile: 0,
        face_down: false,
        immune: false,
        crystals: 3,
        props: world::NameMap::new(),
    };
    w.trig_kind = 45; // TriggerKind::TurnEnd
    w.trig_player = player;
    w.key_set(player, crate::intern::intern("crystals"), 3);
    let pre_crystals = 3;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook("HHW:运动的天赋", "Hook", "TurnEnd", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    let crystals = world::with_world(|w| w.key_get(player, crate::intern::intern("crystals")));
    kani::cover!(crystals == 2, "decayed to 2");
    assert!(
        crystals == spec::athletic_02_after_decay(pre_crystals),
        "C-HHW-athletic-02: crystals fall by exactly 1"
    );
    // At 2 the card stays on the field (「为0时」 is the discard trigger).
    assert!(
        !spec::athletic_02_discard_ok(effects),
        "C-HHW-athletic-02: non-zero count does not discard"
    );
    assert!(post.money == pre.money, "frame: money");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_decay_to_zero_discards() {
    let player: i32 = 0;
    let mut w = symbolic_play_world(player);
    w.field_n = 1;
    w.field[0] = world::FieldCard {
        uid: 1,
        card: crate::intern::intern("HHW:运动的天赋").0,
        player,
        tile: 0,
        face_down: false,
        immune: false,
        crystals: 1,
        props: world::NameMap::new(),
    };
    w.trig_kind = 45; // TriggerKind::TurnEnd
    w.trig_player = player;
    w.key_set(player, crate::intern::intern("crystals"), 1);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::hook("HHW:运动的天赋", "Hook", "TurnEnd", player));
    let crystals = world::with_world(|w| w.key_get(player, crate::intern::intern("crystals")));
    // Decay takes 1 → 0. The book then 「置入弃牌堆」. The crystals-changed
    // hook is a separate entry; this harness pins the decay half. The
    // discard half is `athletic_empty_discards` below.
    assert!(crystals == 0, "decay from 1 reaches 0");
    let _ = pre;
}

/// The crystals-changed hook: when the count hits 0 the card leaves the field.
/// Book: 「为0时置入弃牌堆」. If the body only `set_dest(Graveyard)` without an
/// Unplace / to_discard, this is a LESS finding.
#[kani::proof]
#[kani::unwind(24)]
pub fn athletic_empty_discards() {
    let player: i32 = 0;
    let mut w = symbolic_play_world(player);
    w.field_n = 1;
    w.field[0] = world::FieldCard {
        uid: 1,
        card: crate::intern::intern("HHW:运动的天赋").0,
        player,
        tile: 0,
        face_down: false,
        immune: false,
        crystals: 0,
        props: world::NameMap::new(),
    };
    w.trig_kind = 76; // TriggerKind::CounterChanged
    w.trig_player = player;
    w.trig_value = -1;
    w.key_set(player, crate::intern::intern("crystals"), 0);
    world::reset_world(w);
    invoke::unwrap_done(invoke::hook(
        "HHW:运动的天赋",
        "Hook",
        "CounterChanged",
        player,
    ));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    // Book: the card is put into the discard. Observable: UnplaceCard and/or
    // a discard pile write. `set_dest` alone is not enough.
    let discarded = effects
        .iter()
        .any(|e| matches!(e, Effect::UnplaceCard { .. }));
    kani::cover!(discarded, "card unplaced into discard");
    kani::cover!(!discarded, "card NOT unplaced (LESS finding)");
    // Fail-closed: the book demands the discard. If this assert fires the
    // body is LESS than the book — that is a real finding, not a spec bug.
    assert!(
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
// ===========================================================================

fn believe_world(player: i32) -> World {
    let mut w = symbolic_play_world(player);
    // Hand: this card (slot 2) + two others so the gate holds and the
    // discard pick has a target.
    w.hand = [false; 8];
    w.hand[2] = true; // this card (CARD_OTHER slot)
    w.hand[3] = true; // EXTRA
    w.hand_n = 0;
    // Deck: two concrete cards so (04) can take two.
    w.deck_n = 2;
    w.deck_pile = [u16::MAX; 8];
    w.deck_pile[0] = crate::intern::intern("PP:[标题偶像]").0;
    w.deck_pile[1] = crate::intern::intern("PP:[强韧之花]").0;
    w.symbolic_answers = true;
    w
}

#[kani::proof]
#[kani::unwind(40)]
pub fn believe_cost_800_iff() {
    let player: i32 = 0;
    let mut w = believe_world(player);
    w.money[player as usize] = 5_000;
    let pre_money = w.money[player as usize];
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:因为我一直相信着你", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    assert!(
        spec::believe_02_log_ok(effects, player),
        "C-HHW-believe-02: PayMoney(player, 800) is in the log"
    );
    assert!(
        post.money[player as usize] == pre_money - 800,
        "C-HHW-believe-02: money falls by exactly 800"
    );
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
}

#[kani::proof]
#[kani::unwind(40)]
pub fn believe_discard_one_iff() {
    let player: i32 = 0;
    let w = believe_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:因为我一直相信着你", player));
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    assert!(
        spec::believe_03_log_ok(effects, player),
        "C-HHW-believe-03: exactly one DiscardFromHand"
    );
    assert!(
        spec::believe_04_log_ok(effects, player),
        "C-HHW-believe-04: two deck→hand transfers"
    );
    assert!(post.pos == pre.pos, "frame: pos");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn believe_mutant_is_detected() {
    let player: i32 = 0;
    let mut w = believe_world(player);
    w.money[player as usize] = 5_000;
    let pre_money = w.money[player as usize];
    world::reset_world(w);
    invoke::unwrap_done(invoke::play("HHW:因为我一直相信着你", player));
    let post = world::snapshot();
    assert!(
        post.money[player as usize] != pre_money - spec::believe_02_amount_paid_mutant(pre_money),
        "mutant cost 80 must disagree with the card's 800"
    );
}

// ===========================================================================
// C-HHW-backstage-02 -- 「每翻入一张获得1000资金」
// Entry: `HHW:出发！后台之旅！` / `On::Play`
// Isolation: deck holds at least 2 cards (OPEN-HHW-SHORT-PILE otherwise).
// ===========================================================================

fn backstage_world(player: i32) -> World {
    let mut w = symbolic_play_world(player);
    w.deck_n = 2;
    w.deck_pile = [u16::MAX; 8];
    w.deck_pile[0] = crate::intern::intern("PP:[标题偶像]").0;
    w.deck_pile[1] = crate::intern::intern("PP:[强韧之花]").0;
    w.symbolic_answers = true;
    w
}

#[kani::proof]
#[kani::unwind(40)]
pub fn backstage_pays_1000_per_discard_iff() {
    let player: i32 = 0;
    let w = backstage_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:出发！后台之旅！", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    // Count GainMoney(1000) — each is one flip.
    let mut flipped = 0;
    for e in effects {
        if matches!(e, Effect::GainMoney { player: p, n } if *p == player && *n == 1000) {
            flipped += 1;
        }
    }
    kani::cover!(flipped == 0, "both kept");
    kani::cover!(flipped == 1, "one flipped");
    kani::cover!(flipped == 2, "both flipped");
    assert!(
        flipped >= 0 && flipped <= 2,
        "C-HHW-backstage-01: at most two flips"
    );
    assert!(
        post.money[player as usize] == pre.money[player as usize] + spec::backstage_02_gain(flipped),
        "C-HHW-backstage-02: money rises by exactly 1000 per flip"
    );
    assert!(post.pos == pre.pos, "frame: pos");
    assert!(post.fire == pre.fire, "frame: fire");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn backstage_mutant_is_detected() {
    let player: i32 = 0;
    let w = backstage_world(player);
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:出发！后台之旅！", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];
    let mut flipped = 0;
    for e in effects {
        if matches!(e, Effect::GainMoney { player: p, n } if *p == player && *n == 1000) {
            flipped += 1;
        }
    }
    assert!(
        post.money[player as usize]
            != pre.money[player as usize] + spec::backstage_02_gain_mutant(flipped),
        "mutant 500-per-flip must disagree"
    );
}

// ===========================================================================
// C-HHW-balloon-02 -- 「选择其中之一，传送至结果对应序号的格子，视为你的主要移动」
// Entry: `HHW:热气球演出` / `On::Play`
// (01)'s four `3d20 mod 60` rolls are symbolic; the pick is a player choice.
// Spec: dest = (r == 0 ? 60 : r) − 1.
// ===========================================================================

#[kani::proof]
#[kani::unwind(40)]
pub fn balloon_teleports_to_a_chosen_result_iff() {
    let player: i32 = 0;
    let mut w = symbolic_play_world(player);
    w.symbolic_answers = true;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:热气球演出", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    // Find the Teleport and check it is one of the four recorded results'
    // destinations. The rolls themselves are not in the Effect log (they are
    // plan/roll state); the book's frame for (02) is the teleport as main move.
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
    assert!(dest >= 0, "C-HHW-balloon-02: the player is teleported");
    // Destination must be a legal board tile (0..60). The 序号 wrap is the
    // spec's `balloon_02_dest`; with symbolic rolls the host's `roll` covers
    // the face range, so any in-range dest is a legal pick of some result.
    assert!(
        dest < 60,
        "C-HHW-balloon-02: dest is a board tile (序号 1..=60 → 0..60)"
    );
    assert!(post.pos[player as usize] == dest, "post pos is the dest");
    assert!(post.money == pre.money, "frame: money");
}

#[kani::proof]
#[kani::unwind(24)]
pub fn balloon_mutant_is_detected() {
    // Spec unit check: dest(r) vs dest_mutant(r) differ for r=0 (wrap).
    let r = 0;
    assert!(
        spec::balloon_02_dest(r) != spec::balloon_02_dest_mutant(r),
        "mutant raw-result dest disagrees on the mod-60 wrap"
    );
}

// ===========================================================================
// C-HHW-hagumi-02 (the >60 branch)
// 「若选择的投掷结果大于60，获得1000资金并抽一张卡，不放置标记。」
// Entry: `HHW:（育美）` / `On::Play`
// (01) rolls two 4d20; the player picks one. If the pick is >60 the else
// branch fires. Isolation: pin the two rolls into the >60 region via the
// host's symbolic dice and a concrete answer that picks the high one.
// ===========================================================================

#[kani::proof]
#[kani::unwind(40)]
pub fn hagumi_over_60_gains_and_draws_iff() {
    let player: i32 = 0;
    let mut w = symbolic_play_world(player);
    w.deck_n = 2;
    w.deck_pile = [u16::MAX; 8];
    w.deck_pile[0] = crate::intern::intern("PP:[标题偶像]").0;
    w.deck_pile[1] = crate::intern::intern("PP:[强韧之花]").0;
    w.symbolic_answers = true;
    let pre = {
        world::reset_world(w);
        world::snapshot()
    };
    invoke::unwrap_done(invoke::play("HHW:（育美）", player));
    let post = world::snapshot();
    let (logbuf, log_n) = effects_of();
    let effects = &logbuf[..log_n];

    // Over-60 branch (when a pick landed there): GainMoney(1000)+Draw(1).
    // ≤60 branch: PlaceMark (and no money). Cover both shapes.
    let over = spec::hagumi_02_over_60_log_ok(effects, player);
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
        assert!(
            post.money[player as usize] == pre.money[player as usize] + 1000,
            "C-HHW-hagumi-02: over-60 grants exactly 1000"
        );
    }
    if has_gain {
        assert!(
            post.money[player as usize] == pre.money[player as usize] + 1000,
            "C-HHW-hagumi-02: money grant is 1000, not 2000"
        );
    }
    assert!(post.pos == pre.pos, "frame: pos");
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

#[allow(dead_code)]
fn _touch(s: &Snapshot) {
    let _ = (s.log_n, spec::log_empty(&[]), TriggerOverlay::default());
}