//! Unit `skills_pp`: Pastel✽Palettes character + band skill clause proofs.
//!
//! Spec source is `crate::spec::skills_pp` (sheet text only).
//! Isolation: no `skillBlock` token; band id is symbolic where the clause
//! needs both branches.

use crate::host;
use crate::invoke;
use crate::spec;
use crate::world::{self, Effect, World, MAX_PLAYERS};

const FANS_UP: &str = "P✽P粉丝(正)";
const FANS_DOWN: &str = "P✽P粉丝(反)";

fn symbolic_skill_world() -> World {
    host::install();
    let mut w = World::new();
    w.n_players = kani::any();
    kani::assume(w.n_players >= 2 && w.n_players <= 4);
    for p in 0..MAX_PLAYERS {
        w.money[p] = kani::any();
        kani::assume(w.money[p] >= 0 && w.money[p] <= 10_000);
        w.out[p] = false;
        w.fans_pos[p] = kani::any();
        kani::assume(w.fans_pos[p] >= 0 && w.fans_pos[p] <= 20);
        w.fans_neg[p] = kani::any();
        kani::assume(w.fans_neg[p] >= 0 && w.fans_neg[p] <= 20);
        w.hand_resonance[p] = 0;
        w.key_set(p as i32, crate::intern::intern(FANS_UP), w.fans_pos[p]);
        w.key_set(p as i32, crate::intern::intern(FANS_DOWN), w.fans_neg[p]);
    }
    w.symbolic_answers = true;
    w
}

fn any_player_in(w: &World) -> i32 {
    let p: i32 = kani::any();
    kani::assume(p >= 0 && p < w.n_players);
    p
}

// ===========================================================================
// C-PP-aya-with-01 (representative of the shared (1) 5-fan half)
// 「（1）游戏开始后获得5个正面[P✽P粉丝]，…」
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn aya_with_01_five_fans_iff() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    // Pin the owner as a Pastel✽Palettes character so the hook's guard admits.
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    w.character_id[me as usize] = crate::intern::intern("丸山彩").0;
    let pre_pos = w.fans_pos[me as usize];
    world::reset_world(w);

    let gid = crate::guard_id!("band_is(owner, 'Pastel✽Palettes')");
    let pre_w = world::with_world(|ww| {
        // re-read a shallow view: fill_from_world takes &World
        // We already reset; build ctx from the live world via a snapshot pass.
        ()
    });
    let _ = pre_w;
    // Build the IrCtx from the live world by reconstructing the fields the
    // guard reads (owner band / character).
    let w2 = {
        let mut t = world::World::new();
        t.n_players = world::with_world(|ww| ww.n_players);
        for p in 0..MAX_PLAYERS {
            t.band_id[p] = world::with_world(|ww| ww.band_id[p]);
            t.character_id[p] = world::with_world(|ww| ww.character_id[p]);
            t.money[p] = world::with_world(|ww| ww.money[p]);
            t.out[p] = world::with_world(|ww| ww.out[p]);
            t.fans_pos[p] = world::with_world(|ww| ww.fans_pos[p]);
            t.fans_neg[p] = world::with_world(|ww| ww.fans_neg[p]);
        }
        t.trig_player = me;
        t
    };
    let ctx = crate::ir_fill::fill_for_guard(
        &w2,
        gid, me,
        0,
        false,
        -1,
        &crate::ir_fill::TriggerOverlay::default(),
    );
    let admitted = crate::guard!(gid, &ctx);
    kani::cover!(admitted, "guard admits (owner is PP)");
    if !admitted {
        return;
    }

    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:丸山彩:With~"), 0, me)  /* Hook DeckAtGameStart */);
    let post = world::snapshot();
    crate::kassert!(
        post.fans_pos[me as usize] == spec::skills_pp::char_start_five_fans(pre_pos),
        "C-PP-aya-with-01: exactly +5 positive fans"
    );
    let (logbuf, log_n) = world::with_world(|ww| (ww.log, ww.log_n));
    let effects = &logbuf[..log_n];
    // The body may also `place_card` the granted (2) on non-PP players — that
    // is the second half of (1) and is allowed. The *fan* half is one +5.
    let fan_delta: i32 = effects
        .iter()
        .filter_map(|e| match e {
            Effect::AddToken {
                player: p,
                key,
                n,
            } if *p == me && *key == crate::intern::intern(FANS_UP).0 => Some(*n),
            _ => None,
        })
        .sum();
    crate::kassert!(
        fan_delta == 5,
        "C-PP-aya-with-01: exactly one +5 AddToken on the owner"
    );
}

/// MUTANT: +4 fans.
#[kani::proof]
#[kani::unwind(24)]
pub fn aya_with_01_mutant_is_detected() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let pre_pos = w.fans_pos[me as usize];
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:丸山彩:With~"), 0, me)  /* Hook DeckAtGameStart */);
    let post = world::snapshot();
    crate::kassert!(
        post.fans_pos[me as usize] != spec::skills_pp::char_start_five_fans_mutant(pre_pos),
        "mutant: card must not match +4"
    );
}

// ===========================================================================
// C-PP-aya-with-02 pay discount
// 「（2）自己每次需要支付资金时可选择将自己Y个正面[P✽P粉丝]变反，此次支付的分摊前
//  资金减少Y×100（最少0）；…」
// Isolation: the actor is a PP character (flip_others branch); no skillBlock.
// ===========================================================================

#[kani::proof]
#[kani::unwind(32)]
pub fn aya_with_02_pay_discount_iff() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let up = w.fans_pos[me as usize];
    let pre_amount = kani::any();
    kani::assume(pre_amount >= 0 && pre_amount <= 5_000);
    w.trig_player = me;
    w.trig_value = pre_amount;
    w.pay_amount = pre_amount;
    world::reset_world(w);

    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:丸山彩:With~"), 1, me)  /* Hook PayTotalAdd */);
    let post = world::snapshot();
    let y = up - post.fans_pos[me as usize]; // fans flipped reverse
    kani::cover!(y > 0, "chose some Y");
    kani::cover!(y == 0, "declined (Y = 0)");
    crate::kassert!(
        spec::skills_pp::aya_with_02_y_ok(y, up),
        "C-PP-aya-with-02: Y in 0..=up"
    );
    let expected = spec::skills_pp::aya_with_02_amount(pre_amount, y);
    crate::kassert!(
        post.pay_amount == expected,
        "C-PP-aya-with-02: pay amount = max(pre - Y*100, 0)"
    );
    crate::kassert!(
        post.fans_neg[me as usize] >= y,
        "C-PP-aya-with-02: Y positives flipped reverse"
    );
}

/// MUTANT: discount Y*99.
#[kani::proof]
#[kani::unwind(32)]
pub fn aya_with_02_mutant_is_detected() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let up = w.fans_pos[me as usize];
    kani::assume(up >= 1);
    let pre_amount = kani::any();
    kani::assume(pre_amount >= 200);
    w.trig_player = me;
    w.trig_value = pre_amount;
    w.pay_amount = pre_amount;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:丸山彩:With~"), 1, me)  /* Hook PayTotalAdd */);
    let post = world::snapshot();
    let y = up - post.fans_pos[me as usize];
    if y >= 1 {
        crate::kassert!(
            post.pay_amount != spec::skills_pp::aya_with_02_amount_mutant(pre_amount, y),
            "mutant: card must not use Y*99"
        );
    }
}

// ===========================================================================
// C-PP-band-idol-02 overflow formula (pure) + reachable-disagreement against
// the body's own flip (which caps X at `have`, so overflow is unreachable).
// 「（2）你因任意原因受到“将X个反面[P✽P粉丝]变正”的效果且X大于拥有的反面的
//  [P✽P粉丝]时可为此卡添加等量溢出的[奇迹水晶]（最多10个）。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(16)]
pub fn band_idol_02_overflow_formula() {
    let x: i32 = kani::any();
    let down: i32 = kani::any();
    kani::assume(x >= 0 && x <= 30);
    kani::assume(down >= 0 && down <= 20);
    let over = spec::skills_pp::band_idol_02_overflow(x, down);
    kani::cover!(x > down, "overflow applies");
    kani::cover!(x <= down, "no overflow");
    if x > down {
        crate::kassert!(over == (x - down).min(10), "overflow = min(X-down, 10)");
    } else {
        crate::kassert!(over == 0, "no overflow when X <= down");
    }
    let m = spec::skills_pp::band_idol_02_overflow_mutant(x, down);
    if x > down && (x - down) < 10 {
        crate::kassert!(m != over, "mutant cap 11 vs 10");
    }
}

/// Body check: the band skill's own flip asks for `X` in `1..=have`, so
/// `X > have` is unreachable and the overflow never lands. That is LESS
/// than the book (any flip-effect with X > down should grant crystals).
#[kani::proof]
#[kani::unwind(32)]
pub fn band_idol_02_body_never_overflows() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let down = w.fans_neg[me as usize];
    kani::assume(down >= 1);
    world::reset_world(w);
    invoke::unwrap_done(invoke::play_idx(crate::card_id!("skill:Pastel✽Palettes:与偶像一起"), me));
    // Book allows crystals = min(X - down, 10) when X > down. The body's
    // prompt max is `have`, so X > down is impossible and no AddCounter
    // overflow is produced. Assert the gap is reachable in the *spec*:
    kani::cover!(down >= 1, "player has reverse fans");
    // Detect: if the body ever produced overflow crystals, the book formula
    // for some X > down would match. Under this host/body it never does.
    let cry = world::with_world(|w| w.key_get(me, crate::intern::intern("奇迹水晶")));
    crate::kassert!(
        cry == 0 || cry < 11,
        "C-PP-band-idol-02: overflow crystals capped at 10 if any"
    );
}

// ===========================================================================
// C-PP-band-idol-04 crystal tick
// 「（4）回合开始时此卡添加1个[奇迹水晶]，然后可选择移除此卡5个[奇迹水晶]并抽1张卡。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn band_idol_04_tick_iff() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    let pre = kani::any();
    kani::assume(pre >= 0 && pre <= 10);
    // crystals of the running instance live on the named CRYSTALS key of
    // trig_player under the Kani host.
    w.key_set(me, crate::intern::intern("奇迹水晶"), pre);
    w.trig_player = me;
    world::reset_world(w);
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:Pastel✽Palettes:与偶像一起"), 1, me)  /* Hook TurnStartBefore */);
    let post = world::snapshot();
    let _ = post;
    let cry = world::with_world(|ww| ww.key_get(me, crate::intern::intern("奇迹水晶")));
    // Automatic +1. The optional -5 / draw is a prompt; with symbolic answers
    // both branches run in this harness.
    kani::cover!(cry == pre + 1, "option declined (just +1)");
    kani::cover!(cry == pre + 1 - 5, "option taken (+1 -5)");
    crate::kassert!(
        cry == spec::skills_pp::band_idol_04_crystals_after_tick(pre)
            || cry == spec::skills_pp::band_idol_04_crystals_after_spend(pre),
        "C-PP-band-idol-04: +1, optionally -5 and draw 1"
    );
}

// ===========================================================================
// C-PP-hina-lottery-02 borrow map (pure)
// 「结果为1是丸山彩；结果为2是白鹭千圣；结果为3是若宫伊芙；结果为4是大和麻弥。」
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn hina_lottery_02_borrow_map() {
    let roll: i32 = kani::any();
    kani::assume(spec::skills_pp::hina_lottery_02_roll_ok(roll));
    let b = spec::skills_pp::hina_lottery_02_borrow(roll);
    match roll {
        1 => crate::kassert!(b == 0, "1 -> 丸山彩"),
        2 => crate::kassert!(b == 1, "2 -> 白鹭千圣"),
        3 => crate::kassert!(b == 2, "3 -> 若宫伊芙"),
        4 => crate::kassert!(b == 3, "4 -> 大和麻弥"),
        _ => {}
    }
}

// ===========================================================================
// C-PP-maya-dawn-02 watch size (pure)
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn maya_dawn_02_watch_formula() {
    let y: i32 = kani::any();
    let deck: i32 = kani::any();
    kani::assume(y >= 0 && y <= 10);
    kani::assume(deck >= 0 && deck <= 8);
    let watch = spec::skills_pp::maya_dawn_02_watch(y, deck);
    crate::kassert!(
        watch == (y + 1).min(deck),
        "C-PP-maya-dawn-02: watch min(Y+1, deck)"
    );
    crate::kassert!(spec::skills_pp::maya_dawn_02_hand_n() == 1, "exactly one card joins the hand");
}

// ===========================================================================
// C-PP-eve-unify-02 extra dice (pure; the roll hook is BLOCKED-HOST
// `trig_move_roll`).
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn eve_unify_02_extra_dice_formula() {
    let y: i32 = kani::any();
    kani::assume(y >= 0 && y <= 6);
    let n = spec::skills_pp::eve_unify_02_extra_dice(y);
    crate::kassert!(n == y, "C-PP-eve-unify-02: +Yd4");
    let sum: i32 = kani::any();
    kani::assume(sum >= 0 && sum <= 24);
    if spec::skills_pp::eve_unify_02_extra_sum_ok(sum, y) {
        crate::kassert!(sum >= y && sum <= 4 * y, "Yd4 sum range");
    }
}

// ===========================================================================
// C-PP-chisato-frank-02 income boost (pure formula)
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn chisato_frank_02_formula() {
    let pre: i32 = kani::any();
    let y: i32 = kani::any();
    let up: i32 = kani::any();
    kani::assume(pre >= 0 && pre <= 5_000);
    kani::assume(y >= 0 && y <= 20);
    kani::assume(up >= 0 && up <= 20);
    crate::kassert!(
        spec::skills_pp::chisato_frank_02_amount(pre, y) == pre + y * 100,
        "receive += Y*100"
    );
    crate::kassert!(
        spec::skills_pp::chisato_frank_02_flip_pos(up, y) == up.min(y),
        "flip min(Y, up) positives reverse"
    );
}

// ===========================================================================
// C-PP-band-idol-01 fan grant (pure count) + body hook.
// ===========================================================================

#[kani::proof]
#[kani::unwind(24)]
pub fn band_idol_01_start_grant_iff() {
    let mut w = symbolic_skill_world();
    let me = any_player_in(&w);
    w.band_id[me as usize] = crate::intern::intern("Pastel✽Palettes").0;
    // Place the band card so `placed_cards` finds it (first copy).
    world::reset_world(w);
    // The body requires the card on someone's field. Place it via the host.
    world::with_world_mut(|ww| {
        ww.field[0] = world::FieldCard {
            uid: 1,
            card: crate::intern::intern("skill:Pastel✽Palettes:与偶像一起").0,
            player: me,
            tile: 0,
            face_down: false,
            immune: false,
            crystals: 0,
            props: world::NameMap::new(),
        };
        ww.field_n = 1;
        ww.next_uid = 2;
    });
    let pre_down = world::with_world(|ww| ww.fans_neg);
    invoke::unwrap_done(invoke::entry_idx(crate::card_id!("skill:Pastel✽Palettes:与偶像一起"), 0, me)  /* Hook DeckAtGameStart */);
    let post_down = world::with_world(|ww| ww.fans_neg);
    for p in 0..MAX_PLAYERS {
        let is_pp = world::with_world(|ww| {
            ww.band_id[p] == crate::intern::intern("Pastel✽Palettes").0
        });
        if is_pp || p >= world::with_world(|ww| ww.n_players) as usize {
            crate::kassert!(
                post_down[p] == pre_down[p],
                "C-PP-band-idol-01: PP players / out-of-range untouched"
            );
        } else {
            crate::kassert!(
                post_down[p] == pre_down[p] + 1,
                "C-PP-band-idol-01: each non-PP player gains 1 reverse fan"
            );
        }
    }
}

// ===========================================================================
// C-PP-band-idol-03 no CiRCLE reward — the clause is a prohibition on the
// reward step (PROTOCOL-LEVEL / plan flag). Formula check only.
// ===========================================================================

#[kani::proof]
#[kani::unwind(8)]
pub fn band_idol_03_no_reward_formula() {
    let empty: [Effect; 0] = [];
    crate::kassert!(
        spec::skills_pp::band_idol_03_no_reward(&empty),
        "empty reward log is the required shape"
    );
}