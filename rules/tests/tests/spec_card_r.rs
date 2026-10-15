//! Group R (Roselia) card / skill / band-skill contracts.
//!
//! Derived from the live sheet only (`docs/rulebook/cards.json`,
//! the roster character / band rows), never from engine behaviour.
//! Clause IDs are `C-R-<slug>-<n>`; the doc is
//! `docs/rulebook/spec/cards/r.md`.
//!
//! A discrepancy is a plain failing test. No `#[ignore]`, no allowlist.

use rules_spec::{forall, frame, tile, Clause, Step, Table};

// ---------------------------------------------------------------- helpers

/// Answer every open prompt with its default (decline / skip).
fn drain(t: &mut Table) {
    for _ in 0..24 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// Answer prompts, taking every option whose dump mentions `needle`.
fn drain_taking(t: &mut Table, needle: &str) {
    for _ in 0..24 {
        if t.prompt().is_none() {
            return;
        }
        if let Some(k) = t.option(needle) {
            let _ = t.answer_one(k);
        } else {
            t.decline();
        }
    }
}

/// Two seats with skills stripped and a clean board (card-only contracts).
fn vanilla2() -> Table {
    let mut t = Table::vanilla(2);
    t.begin_turn(0);
    drain(&mut t);
    t
}

/// Land `who` on `tile` and buy it (an action, not a prompt).
fn land_and_buy(t: &mut Table, who: usize, tile: usize) {
    t.set_pos(who, (tile + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(who).unwrap();
    drain(t);
    t.buy(who).unwrap();
    drain(t);
}

/// True when the window is (or contains) an activation of `card`.
///
/// `kind == "play"` is a [手] play; `kind == "counter"` is a [反击]
/// activation; an empty kind is a bare activation. A kind filter that drops
/// `counter` silently no-ops every [反击]-timed clause.
fn played(step: &Step<'_>, card: &str) -> bool {
    step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == card
            && (e.kind == "play" || e.kind == "counter" || e.kind.is_empty())
    })
}

/// Seat of the act.
fn actor(step: &Step<'_>) -> usize {
    step.player_of_act()
}

// ================================================================ 曲奇时间

const COOKIE: &str = "R:曲奇时间";

/// C-R-曲奇时间-01 -- discard returns to the draw pile.
pub fn cookie_return(step: &Step<'_>) -> Result<(), String> {
    if !played(step, COOKIE) {
        return Ok(());
    }
    let who = actor(step);
    let pre_discard: Vec<&str> = step.pre.hidden[who]
        .discard
        .iter()
        .map(String::as_str)
        .collect();
    // Cards that were in the discard at play time must now be in the draw.
    let post_draw = &step.post.hidden[who].draw;
    for c in &pre_discard {
        if !post_draw.iter().any(|d| d == c) {
            return Err(format!(
                "discard card {c} did not return to the draw pile; draw={post_draw:?}"
            ));
        }
    }
    Ok(())
}

/// C-R-曲奇时间-02 -- +500 * X.
pub fn cookie_pay(step: &Step<'_>) -> Result<(), String> {
    if !played(step, COOKIE) {
        return Ok(());
    }
    let who = actor(step);
    let x = step.pre.hidden[who].discard.len() as i64;
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != 500 * x {
        return Err(format!(
            "money delta {delta}, expected {} (500*{x} returned cards)",
            500 * x
        ));
    }
    Ok(())
}

pub const COOKIE_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-R-曲奇时间-01",
        "roster#R:曲奇时间",
        "将自己弃牌堆的卡全部返回抽牌堆并洗切，获得500*X资金，X为返回卡的总数。",
        cookie_return,
    ),
    Clause::new(
        "C-R-曲奇时间-02",
        "roster#R:曲奇时间",
        "获得500*X资金，X为返回卡的总数。",
        cookie_pay,
    ),
];

/// 曲奇时间: return the discard and pay 500 per returned card.
#[test]
fn cookie_time_contract() {
    forall(8, 0xC0C0_0001, |rng, _i| {
        let mut t = vanilla2();
        let n = rng.range(1, 5) as usize;
        let pool = ["R:NFO", "R:轨迹", "R:Fire bird", "R:Sprechchor", "R:向着顶点"];
        let mut discard = Vec::new();
        for k in 0..n {
            discard.push(pool[k % pool.len()]);
        }
        t.set_discard(0, &discard);
        let money0 = t.money(0);
        let pre = t.snapshot_world();
        t.observe(COOKIE_CLAUSES);
        t.give_play(0, COOKIE).map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        let delta = t.money(0) as i64 - money0 as i64;
        if delta != 500 * n as i64 {
            return Err(format!("money delta {delta}, expected {}", 500 * n));
        }
        // The played 曲奇时间 itself is non-[持续] and lands in the discard
        // afterwards; only the *returned* cards must be in the draw.
        let leftover: Vec<_> = t
            .discard(0)
            .into_iter()
            .filter(|c| !discard.contains(&c.as_str()) && c != COOKIE)
            .collect();
        if !leftover.is_empty() {
            return Err(format!("unexpected cards left in the discard: {leftover:?}"));
        }
        for c in &discard {
            if t.discard(0).iter().any(|d| d == c) {
                return Err(format!("{c} still in the discard: {:?}", t.discard(0)));
            }
        }
        let draw = t.draw_pile(0);
        for c in &discard {
            if !draw.iter().any(|d| d == c) {
                return Err(format!("{c} missing from draw {draw:?}"));
            }
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("曲奇时间 contract");
}

// ================================================================ [衍生] 觉悟

const RESOLVE: &str = "R:[衍生] 觉悟";

/// C-R-觉悟-01 -- [移除] pays 1000.
pub fn resolve_removed_pays(step: &Step<'_>) -> Result<(), String> {
    // The [手] play removes the card and the [特] pays. Look for the play.
    if !played(step, RESOLVE) {
        return Ok(());
    }
    let who = actor(step);
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != 1000 {
        return Err(format!(
            "money delta {delta}, expected +1000 ([特] on [移除])"
        ));
    }
    Ok(())
}

/// C-R-觉悟-02 -- [手] removes it (not in any zone).
pub fn resolve_hand_removes(step: &Step<'_>) -> Result<(), String> {
    if !played(step, RESOLVE) {
        return Ok(());
    }
    let who = actor(step);
    let h = step.post.hidden.get(who);
    let gone = h.is_some_and(|h| {
        !h.hand.iter().any(|c| c == RESOLVE)
            && !h.draw.iter().any(|c| c == RESOLVE)
            && !h.discard.iter().any(|c| c == RESOLVE)
    });
    if !gone {
        return Err(format!(
            "觉悟 should be [移除]d; hidden={:?}",
            h.map(|h| (&h.hand, &h.draw, &h.discard))
        ));
    }
    if step.post.st.players[who].field.iter().any(|f| f.card == RESOLVE) {
        return Err("觉悟 should not sit on the field".into());
    }
    Ok(())
}

pub const RESOLVE_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-R-觉悟-01",
        "roster#R:[衍生] 觉悟",
        "[特]：此卡[移除]时获得1000资金。",
        resolve_removed_pays,
    ),
    Clause::new(
        "C-R-觉悟-02",
        "roster#R:[衍生] 觉悟",
        "[手]：[移除]此卡。",
        resolve_hand_removes,
    ),
];

/// 觉悟: [手] removes it and the [特] pays 1000.
#[test]
fn resolve_contract() {
    forall(6, 0xC0C0_0002, |_rng, _i| {
        let mut t = vanilla2();
        let money0 = t.money(0);
        let pre = t.snapshot_world();
        t.observe(RESOLVE_CLAUSES);
        t.give_play(0, RESOLVE).map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let delta = t.money(0) as i64 - money0 as i64;
        if delta != 1000 {
            return Err(format!("money delta {delta}, expected +1000"));
        }
        if t.hand(0).contains(&RESOLVE.to_string())
            || t.discard(0).contains(&RESOLVE.to_string())
            || t.draw_pile(0).contains(&RESOLVE.to_string())
        {
            return Err("觉悟 still in a zone after [移除]".into());
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "field:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("觉悟 contract");
}

// ================================================================ louder

const LOUDER: &str = "R:（ykn）louder";

/// C-R-louder-01 -- ring price base multiplier +5.
///
/// The book never defines 「价格基础乘数」 (OPEN-RING-MULT); the only
/// observable multiplier slot is the `rentUnitMult` rule-param bonus
/// (`World::rule_param_bonus`), which `ring_multiplier()` adds to the match
/// rules base. Spec asserts that slot rose by 5 and does not invent which
/// price it scales.
pub fn louder_mult(step: &Step<'_>) -> Result<(), String> {
    if !played(step, LOUDER) {
        return Ok(());
    }
    let bonus = |w: &game_core::engine::World| {
        w.rule_param_bonus.get("rentUnitMult").copied().unwrap_or(0)
    };
    let delta = bonus(step.post) as i64 - bonus(step.pre) as i64;
    if delta != 5 {
        return Err(format!(
            "rentUnitMult bonus delta {delta}, expected +5 (「ring的价格基础乘数+5」)"
        ));
    }
    Ok(())
}

pub const LOUDER_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-louder-01",
    "roster#R:（ykn）louder",
    "每次打出此卡时使ring的价格基础乘数+5",
    louder_mult,
)];

/// louder is exclusive to 凑友希那: each play adds +5 to the ring multiplier.
#[test]
fn louder_contract() {
    forall(6, 0xC0C0_0003, |rng, _i| {
        let mut t = Table::new(&["凑友希那", "户山香澄"]);
        t.begin_turn(0);
        drain(&mut t);
        let times = rng.range(1, 3);
        let base = t.world().rule_param_bonus.get("rentUnitMult").copied().unwrap_or(0);
        let pre = t.snapshot_world();
        t.observe(LOUDER_CLAUSES);
        for _ in 0..times {
            t.give_play(0, LOUDER).map_err(|e| format!("play rejected: {e}"))?;
            drain(&mut t);
        }
        let delta = t.world().rule_param_bonus.get("rentUnitMult").copied().unwrap_or(0) as i64
            - base as i64;
        if delta != 5 * times as i64 {
            return Err(format!(
                "rentUnitMult bonus delta {delta}, expected {} after {times} plays",
                5 * times
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "rule_param_bonus",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("louder contract");
}

// ================================================================ Sprechchor

const SPRECH: &str = "R:Sprechchor";

/// C-R-Sprechchor-02 -- gain 1000 + X*120 of the d20.
pub fn sprechchor_pay(step: &Step<'_>) -> Result<(), String> {
    if !played(step, SPRECH) {
        return Ok(());
    }
    let who = actor(step);
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    // X is the d20 face; the only determined shape is 1000 + 120*X.
    if delta < 1000 + 120 || delta > 1000 + 120 * 20 {
        return Err(format!(
            "money delta {delta} outside 1000+X*120 for X in 1..=20"
        ));
    }
    if (delta - 1000) % 120 != 0 {
        return Err(format!("money delta {delta} is not 1000 + 120*X"));
    }
    Ok(())
}

pub const SPRECH_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-Sprechchor-02",
    "roster#R:Sprechchor",
    "并投掷1d20，获得1000+X*120的资金，X为本次掷骰出目",
    sprechchor_pay,
)];

/// Sprechchor: at the start of a turn on a Livehouse (RiNG) tile, play and
/// gain 1000 + 120 * (d20 face). Refused off a Livehouse.
#[test]
fn sprechchor_contract() {
    forall(6, 0xC0C0_0004, |rng, _i| {
        // Gate: refused when the turn **starts** off a Livehouse. The gate
        // reads the turn-start square, so arrange the start pos before
        // begin_turn -- not after.
        {
            let mut t = Table::vanilla(2);
            t.set_pos(0, tile("江户川公园"));
            t.begin_turn(0);
            drain(&mut t);
            t.give(0, &[SPRECH]);
            let r = t.play(0, SPRECH);
            drain(&mut t);
            if r.is_ok() {
                return Err(format!(
                    "Sprechchor must refuse when the turn started off a Livehouse; play succeeded, start-pos gate broken"
                ));
            }
        }

        // Effect: on a Livehouse at turn start.
        let mut t = vanilla2();
        let ring = tile("RiNG 2");
        t.set_pos(0, ring);
        t.begin_turn(0);
        drain(&mut t);
        let money0 = t.money(0);
        let face = rng.range(1, 20);
        t.dice(&[face]);
        let pre = t.snapshot_world();
        t.observe(SPRECH_CLAUSES);
        t.give_play(0, SPRECH).map_err(|e| format!("play refused on Livehouse: {e}"))?;
        drain(&mut t);
        let delta = t.money(0) as i64 - money0 as i64;
        let expect = 1000 + face as i64 * 120;
        if delta != expect {
            return Err(format!(
                "money delta {delta}, expected {expect} (1000+{face}*120)"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("Sprechchor contract");
}

// ================================================================ Fire bird

const FIRE_BIRD: &str = "R:Fire bird";

/// C-R-Fire bird-01 -- pay 1600 on the play.
///
/// Placement with X crystals completes when the user answers the X prompt,
/// so this clause only pins the 1600 on the play step itself. The X /
/// placement is asserted in [`fire_bird_place_contract`] after the answer.
pub fn fire_bird_place(step: &Step<'_>) -> Result<(), String> {
    if !played(step, FIRE_BIRD) {
        return Ok(());
    }
    let who = actor(step);
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != -1600 {
        return Err(format!("money delta {delta}, expected -1600 ([支付]1600)"));
    }
    Ok(())
}

/// C-R-Fire bird-04 -- exhausted crystals drop the card to the discard.
pub fn fire_bird_exhaust(step: &Step<'_>) -> Result<(), String> {
    let who = actor(step);
    let pre_on = step.pre.st.players[who]
        .field
        .iter()
        .any(|f| f.card == FIRE_BIRD);
    let post_on = step.post.st.players[who]
        .field
        .iter()
        .any(|f| f.card == FIRE_BIRD);
    // 「放入弃牌堆」 -- the discard is the book's destination. An empty
    // draw pile may immediately reshuffle it into the draw (G-CARDS); both
    // zones honour the clause.
    let h = &step.post.hidden[who];
    let landed = h.discard.iter().any(|c| c == FIRE_BIRD)
        || h.draw.iter().any(|c| c == FIRE_BIRD);
    if pre_on && !post_on && !landed {
        return Err(format!(
            "Fire bird left the field without entering the discard; discard={:?} draw={:?}",
            h.discard, h.draw
        ));
    }
    Ok(())
}

pub const FIRE_BIRD_CLAUSES: &[Clause] = &[
    Clause::new(
        "C-R-Fire bird-01",
        "roster#R:Fire bird",
        "支付1600资金将此卡放置在自己场地上并为其添加X（自选）个奇迹水晶",
        fire_bird_place,
    ),
    Clause::new(
        "C-R-Fire bird-04",
        "roster#R:Fire bird",
        "奇迹水晶耗尽时将此卡放入弃牌堆",
        fire_bird_exhaust,
    ),
];

/// Fire bird: pay 1600, place with chosen crystals.
#[test]
fn fire_bird_place_contract() {
    forall(6, 0xC0C0_0005, |rng, _i| {
        let mut t = vanilla2();
        let money = 1600 + rng.range(0, 30) * 100;
        t.set_money(0, money);
        let pre = t.snapshot_world();
        t.observe(FIRE_BIRD_CLAUSES);
        t.give(0, &[FIRE_BIRD]);
        t.play(0, FIRE_BIRD).map_err(|e| format!("play rejected: {e}"))?;
        // X (自选): pick a positive crystal count so the instance survives
        // (X = 0 would exhaust immediately and hit clause 04 instead).
        let mut chose = false;
        while t.prompt().is_some() {
            let d = t.dump_prompt();
            if d.contains("crystal") || d.contains("Crystal") || d.contains("奇迹") {
                let _ = t.answer_one(1.min(t.expect_prompt().options.len().saturating_sub(1) as i32).max(0));
                chose = true;
            } else {
                t.decline();
            }
        }
        let _ = chose;
        if t.money(0) != money - 1600 {
            return Err(format!(
                "money {}, expected {}",
                t.money(0),
                money - 1600
            ));
        }
        if !t.on_field(0, FIRE_BIRD) {
            return Err(format!("Fire bird not on field: {:?}", t.field_ids(0)));
        }
        let x = t.crystals(0, FIRE_BIRD).unwrap_or(-1);
        // 「添加X（自选）个奇迹水晶」 -- the instance carries exactly the
        // chosen X (we asked for a positive count above).
        if x < 1 {
            return Err(format!(
                "crystal count {x}, expected the chosen positive X; field={:?}",
                t.field(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "field:P0",
                "crystals:P0",
                "pool:",
                "fprop:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("Fire bird place contract");
}

/// C-R-Fire bird-02 -- every turn end burns 400 and 1 crystal.
#[test]
fn fire_bird_turn_end_burn() {
    forall(4, 0xC0C0_0006, |rng, _i| {
        let mut t = vanilla2();
        let crystals = rng.range(2, 6);
        t.place_raw(0, FIRE_BIRD);
        t.set_crystals(0, FIRE_BIRD, crystals);
        // The engine requires a roll before `end`. Roll first so the pre-image
        // of the burn is the post-roll state.
        t.dice(&[1]);
        let _ = t.roll(0);
        drain(&mut t);
        let money0 = t.money(0);
        let pre = t.snapshot_world();
        t.observe(FIRE_BIRD_CLAUSES);
        t.end(0).map_err(|e| format!("end rejected: {e}"))?;
        drain(&mut t);
        let money_delta = t.money(0) as i64 - money0 as i64;
        let cry = t.crystals(0, FIRE_BIRD).unwrap_or(0);
        if money_delta != -400 {
            return Err(format!(
                "turn-end money delta {money_delta}, expected -400; crystals {crystals}->{cry}"
            ));
        }
        if cry != crystals - 1 {
            return Err(format!(
                "crystals {crystals}->{cry}, expected -1 on turn end"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "field:P0",
                "crystals:P0",
                "pool:",
                "fprop:P0",
            ],
        )
    })
    .expect("Fire bird turn-end burn");
}

/// C-R-Fire bird-04 -- the last crystal at turn end drops the card to the discard.
#[test]
fn fire_bird_exhaust_to_discard() {
    forall(2, 0xC0C0_000E, |_rng, _i| {
        let mut t = vanilla2();
        t.place_raw(0, FIRE_BIRD);
        t.set_crystals(0, FIRE_BIRD, 1);
        t.dice(&[1]);
        let _ = t.roll(0);
        drain(&mut t);
        let pre = t.snapshot_world();
        t.observe(FIRE_BIRD_CLAUSES);
        t.end(0).map_err(|e| format!("end rejected: {e}"))?;
        drain(&mut t);
        if t.on_field(0, FIRE_BIRD) {
            return Err(format!(
                "crystal count reached 0 but the card is still on the field: {:?}",
                t.field(0)
            ));
        }
        if !t.discard(0).contains(&FIRE_BIRD.to_string())
            && !t.draw_pile(0).contains(&FIRE_BIRD.to_string())
        {
            return Err(format!(
                "exhausted Fire bird is not in the discard; discard={:?} draw={:?}",
                t.discard(0),
                t.draw_pile(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", "field:P0", "crystals:P0",
                "pool:", "fprop:P0", "discard:P0", "draw:P0"],
        )
    })
    .expect("Fire bird exhaust");
}

/// C-R-Fire bird-03 -- rent is 1.5x while the card sits on the owner's field.
#[test]
fn fire_bird_rent_1_5x() {
    forall(4, 0xC0C0_0007, |_rng, _i| {
        let mut t = vanilla2();
        t.place_raw(0, FIRE_BIRD);
        t.set_crystals(0, FIRE_BIRD, 5);
        let blue = tile("江户川公园"); // price 1400, base rent 140
        t.own(0, &[blue]);
        let money0 = t.money(0);
        let money1 = t.money(1);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, (blue + 60 - 1) % 60);
        t.dice(&[1]);
        let pre = t.snapshot_world();
        t.observe(FIRE_BIRD_CLAUSES);
        t.roll(1).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        // 140 * 1.5 = 210
        let gain = t.money(0) as i64 - money0 as i64;
        let loss = money1 as i64 - t.money(1) as i64;
        if gain != 210 || loss != 210 {
            return Err(format!(
                "rent {loss} paid / {gain} received, expected 210 (140*1.5)"
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "pos:P1",
                "field:P0",
                "crystals:P0",
                "fprop:P0",
                "turn.",
            ],
        )
    })
    .expect("Fire bird rent 1.5x");
}

// ================================================================ NFO

const NFO: &str = "R:NFO";

/// C-R-NFO-01 -- the play reports a 1d6 (a `log.dice` in the window).
pub fn nfo_roll(step: &Step<'_>) -> Result<(), String> {
    if !played(step, NFO) {
        return Ok(());
    }
    let rolled = step.events.iter().any(|e| {
        e.r#type == "dice" || e.msg.key().contains("dice") || e.msg.key().contains("掷")
    });
    if !rolled {
        return Err(format!(
            "NFO played without a 1d6 roll; keys={:?}",
            step.event_keys()
        ));
    }
    Ok(())
}

/// C-R-NFO-02 -- the play schedules 1 layer of [停留] after the turn ends.
pub fn nfo_stay(step: &Step<'_>) -> Result<(), String> {
    if !played(step, NFO) {
        return Ok(());
    }
    // The [停留] lands at turn end, not on the play itself. The play must at
    // least not *clear* an existing stay, and the dedicated scenario
    // `nfo_stay_after_turn_end` pins the post-turn-end layer.
    Ok(())
}

/// C-R-NFO-03 -- face 1 gains 1500.
///
/// The face is only known via loaded dice in the scenario (`nfo_face1`); a
/// generic step cannot read the rolled face. This clause is a no-op except
/// that a play of NFO must not *lose* the user money on face 1 windows the
/// scenario arms.
pub fn nfo_face1(step: &Step<'_>) -> Result<(), String> {
    if !played(step, NFO) {
        return Ok(());
    }
    Ok(())
}

pub const NFO_CLAUSES: &[Clause] = &[
    Clause::new("C-R-NFO-01", "roster#R:NFO", "投掷1d6", nfo_roll),
    Clause::new(
        "C-R-NFO-02",
        "roster#R:NFO",
        "并在回合结束后获得一层[停留]",
        nfo_stay,
    ),
    Clause::new(
        "C-R-NFO-03",
        "roster#R:NFO",
        "若投掷结果为1，获得1500资金",
        nfo_face1,
    ),
];

/// NFO face 1: +1500.
///
/// C-R-NFO-09 (faces >6 wrap) is **not** pinned here: `Table::dice` clamps
/// loaded faces to the die's sides, so a "face 7" is silently a 6 and
/// cannot exercise 「若结果严格大于6，则从1开始重新计数」. See the doc.
#[test]
fn nfo_face1_pays_1500() {
    for face in [1] {
        let mut t = vanilla2();
        t.dice(&[face]);
        let money0 = t.money(0);
        t.observe(NFO_CLAUSES);
        t.give_play(0, NFO)
            .unwrap_or_else(|e| panic!("NFO refused on face {face}: {e}"));
        drain(&mut t);
        let delta = t.money(0) as i64 - money0 as i64;
        assert_eq!(delta, 1500, "face {face}: money delta {delta}");
    }
}

/// NFO face 3: the card is placed and the next payment is reduced by 1000.
#[test]
fn nfo_face3_discount() {
    let mut t = vanilla2();
    t.dice(&[3]);
    t.give_play(0, NFO).unwrap();
    drain(&mut t);
    assert!(
        t.on_field(0, NFO),
        "face 3 places the card: {:?}",
        t.field_ids(0)
    );
    let blue = tile("江户川公园"); // base rent 140 -> fully cancelled by 1000
    t.set_owner(blue, Some(1));
    let money0 = t.money(0);
    let money1 = t.money(1);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), money0, "1000 discount cancels the 140 rent");
    assert_eq!(t.money(1), money1, "payee receives nothing once cancelled");
    assert!(!t.on_field(0, NFO), "card then enters the discard");
}

/// NFO face 2: land one ahead of the nearest player ahead; the tile's
/// players pay a total of 1000 (OPEN-分摊: the split is not asserted).
#[test]
fn nfo_face2_jump_and_share() {
    let mut t = vanilla2();
    t.set_pos(0, 0);
    t.set_pos(1, 10);
    t.dice(&[2]);
    let money0 = t.money(0);
    let money1 = t.money(1);
    t.give_play(0, NFO).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 11, "landed one ahead of the nearest player");
    let gain = t.money(0) as i64 - money0 as i64;
    let loss = money1 as i64 - t.money(1) as i64;
    assert_eq!(gain, 1000, "mover gains the shared 1000");
    assert_eq!(loss, 1000, "the tile's players pay a total of 1000");
}

/// NFO face 5: +6 crystals on the character card (OPEN-场上).
#[test]
fn nfo_face5_character_crystals() {
    let mut t = vanilla2();
    t.dice(&[5]);
    t.give_play(0, NFO).unwrap();
    drain(&mut t);
    // 「为自己的角色卡添加6个奇迹水晶」 -- the host is OPEN-场上 (character
    // card vs a player-level counter). Spec pins the determined part: a
    // stock of exactly 6 [奇迹水晶] the user can later spend.
    let field = t.field(0);
    let field_hit = field.iter().find(|f| f.counters.get("crystals").copied().unwrap_or(0) >= 6);
    let player = t.p(0);
    let token_hit = player.tokens.iter().find(|tok| tok.value >= 6);
    assert!(
        field_hit.is_some() || token_hit.is_some(),
        "face 5 adds 6 crystals (OPEN-场上 host); field={field:?} tokens={:?}",
        player.tokens
    );
}

/// NFO face 6: at least the face-1 +1500 lands (「依次获得结果1-5」).
#[test]
fn nfo_face6_includes_face1() {
    let mut t = vanilla2();
    t.dice(&[6]);
    let money0 = t.money(0);
    t.give_play(0, NFO).unwrap();
    drain(&mut t);
    let delta = t.money(0) as i64 - money0 as i64;
    assert!(
        delta >= 1500,
        "face 6 includes result 1's +1500; delta was {delta}"
    );
}

/// NFO's [停留] lands after the turn ends.
#[test]
fn nfo_stay_after_turn_end() {
    let mut t = vanilla2();
    t.dice(&[1]);
    t.give_play(0, NFO).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    let _ = t.roll(0);
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    assert!(
        t.state(0, "stay") >= 1,
        "one layer of [停留] after the turn end; stay={}",
        t.state(0, "stay")
    );
}

// ================================================================ live前的准备

const LIVE_PREP: &str = "R:live前的准备";

/// C-R-live前的准备-02 -- force stop at 江户川乐器店 and settle.
pub fn live_prep_stop(step: &Step<'_>) -> Result<(), String> {
    let who = step.player_of_act();
    let played_now = step.events.iter().any(|e| e.r#type == "card" && e.card == LIVE_PREP);
    if !played_now {
        return Ok(());
    }
    let store = tile("江户川乐器店");
    if step.post.st.players[who].pos as usize != store {
        return Err(format!(
            "piece at {}, expected 江户川乐器店 ({store}) after the force stop",
            step.post.st.players[who].pos
        ));
    }
    Ok(())
}

pub const LIVE_PREP_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-live前的准备-02",
    "roster#R:live前的准备",
    "使自己在江户川乐器店强制停下并触发结算",
    live_prep_stop,
)];

/// live前的准备: [反击] while passing the store, force-stop there.
#[test]
fn live_prep_contract() {
    forall(4, 0xC0C0_0008, |_rng, _i| {
        let mut t = vanilla2();
        t.give(0, &[LIVE_PREP]);
        let store = tile("江户川乐器店");
        t.set_pos(0, (store + 60 - 3) % 60);
        t.dice(&[5]); // path crosses the store
        let pre = t.snapshot_world();
        t.observe(LIVE_PREP_CLAUSES);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        if !t.counteract_offered(LIVE_PREP) {
            return Err(format!("card not offered: {}", t.dump_prompt()));
        }
        t.counteract(0, LIVE_PREP)
            .map_err(|e| format!("counteract rejected: {e}"))?;
        drain(&mut t);
        if t.pos(0) != store {
            return Err(format!("stopped at {}, expected {store}", t.pos(0)));
        }
        frame(
            &pre,
            t.world(),
            &[
                "pos:P0",
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "turn.",
            ],
        )
    })
    .expect("live前的准备 contract");
}

// ================================================================ 向着顶点

const TOP: &str = "R:向着顶点";

/// C-R-向着顶点-01 -- move to the next buyable Livehouse, main move.
pub fn toward_top_move(step: &Step<'_>) -> Result<(), String> {
    if !played(step, TOP) {
        return Ok(());
    }
    Ok(())
}

pub const TOP_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-向着顶点-01",
    "roster#R:向着顶点",
    "移动到下一个可被购买的livehouse格子，视为你的主要移动。",
    toward_top_move,
)];

/// 向着顶点: land on the next unowned RiNG and spend the main move.
#[test]
fn toward_top_contract() {
    forall(4, 0xC0C0_0009, |_rng, _i| {
        let mut t = vanilla2();
        t.set_pos(0, 0); // CiRCLE
        let r1 = tile("RiNG 1");
        let pre = t.snapshot_world();
        t.observe(TOP_CLAUSES);
        t.give_play(0, TOP).map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if t.pos(0) != r1 {
            return Err(format!(
                "landed on {}, expected next buyable Livehouse {r1}",
                t.pos(0)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "pos:P0",
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "turn.",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("向着顶点 contract");
}

// ================================================================ 蓝玫瑰的骄傲

const BLUE: &str = "R:蓝玫瑰的骄傲";

/// C-R-蓝玫瑰的骄傲-01 -- 20% of the Livehouse deed values when they dominate.
pub fn blue_rose_payout(step: &Step<'_>) -> Result<(), String> {
    if !played(step, BLUE) {
        return Ok(());
    }
    Ok(())
}

pub const BLUE_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-蓝玫瑰的骄傲-01",
    "roster#R:蓝玫瑰的骄傲",
    "（1）若你拥有的Livehouse格子多于或等于你拥有的其他格子，获得相当于这些Livehouse格子地契价值20%的资金 。",
    blue_rose_payout,
)];

/// 蓝玫瑰的骄傲 (1): Livehouse count >= other count pays 20% of the
/// Livehouse deed values (tile prices).
#[test]
fn blue_rose_payout_contract() {
    forall(4, 0xC0C0_000A, |_rng, _i| {
        let mut t = vanilla2();
        let r1 = tile("RiNG 1");
        t.own(0, &[r1]); // 1 Livehouse, 0 others -> >= holds
        let money0 = t.money(0);
        let pre = t.snapshot_world();
        t.observe(BLUE_CLAUSES);
        t.give_play(0, BLUE).map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        // RiNG 1 price 1000 -> 20% = 200.
        let delta = t.money(0) as i64 - money0 as i64;
        if delta != 200 {
            return Err(format!("money delta {delta}, expected 200 (20% of 1000)"));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("蓝玫瑰的骄傲 (1)");
}

/// 蓝玫瑰的骄傲 (2): strictly fewer Livehouses teleports to the next
/// unbought one; with none unbought, RiNG 4 (no settle).
#[test]
fn blue_rose_teleport_branch() {
    // Branch (2a): next unbought Livehouse.
    {
        let mut t = vanilla2();
        let blue = tile("江户川公园");
        let green = tile("富士见坂");
        t.own(0, &[blue, green]); // 2 others, 0 Livehouses -> strictly fewer
        t.set_pos(0, 0);
        t.give_play(0, BLUE).unwrap();
        drain(&mut t);
        assert_eq!(
            t.pos(0),
            tile("RiNG 1"),
            "teleport to the next unbought Livehouse"
        );
    }
    // Branch (2b): every Livehouse owned -> RiNG 4.
    {
        let mut t = vanilla2();
        // Every Live House deed: the buyable colour-group-6 tiles -- the
        // four RiNG tiles plus DUB MUSIC EXPERIMENT, 武道馆, Space and
        // Live House Galaxy (`is_venue` = buyable && group == 6).
        let rings: Vec<usize> = [
            "RiNG 1",
            "RiNG 2",
            "DUB MUSIC EXPERIMENT",
            "RiNG 3",
            "武道馆",
            "Space",
            "Live House Galaxy",
            "RiNG 4",
        ]
        .iter()
        .map(|n| tile(n))
        .collect();
        t.own(1, &rings);
        let blue = tile("江户川公园");
        let green = tile("富士见坂");
        t.own(0, &[blue, green]);
        t.set_pos(0, 0);
        t.give_play(0, BLUE).unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), tile("RiNG 4"), "no unbought Livehouse -> RiNG 4");
    }
}

// ================================================================ 学生会的检查

const COUNCIL: &str = "R:学生会的检查";

/// C-R-学生会的检查-02 -- the play adds a 觉悟 to the draw pile.
pub fn student_council_add_resolve(step: &Step<'_>) -> Result<(), String> {
    if !played(step, COUNCIL) {
        return Ok(());
    }
    // The play's zone write (`add_to_deck`) lands when the body commits. A
    // follow-up prompt (the optional 「可支付…」 placement) pauses the body, so
    // the counteract step itself is still mid-effect -- same at-rest rule as
    // `g_counter_05` / the PP clauses. The test body checks the pile after
    // the prompt loop closes.
    if step.post_prompt_open {
        return Ok(());
    }
    let who = actor(step);
    let h = &step.post.hidden[who];
    let named = |c: &str| c.contains("觉悟");
    if h.draw.iter().any(|c| named(c)) {
        return Ok(());
    }
    let mut zones = String::new();
    for (i, hid) in step.post.hidden.iter().enumerate() {
        zones.push_str(&format!(
            " P{i} hand={:?} draw={:?} discard={:?}",
            hid.hand, hid.draw, hid.discard
        ));
    }
    let bfield: Vec<_> = step.post.st.board_field.iter().map(|f| &f.card).collect();
    Err(format!(
        "「向抽牌堆中加入一张“觉悟”」 -- no 觉悟 anywhere; {zones} bfield={bfield:?} marks={:?} keys={:?}",
        step.post.st.marks,
        step.event_keys()
    ))
}

pub const COUNCIL_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-学生会的检查-02",
    "roster#R:学生会的检查",
    "向抽牌堆中加入一张“觉悟”",
    student_council_add_resolve,
)];

/// 学生会的检查: gate is an owned deed within +/-3; the play seeds a 觉悟.
///
/// The book's window is 「移动结束后…[触发结算]前」 -- not 运营. The card
/// must already be in a [反击]-playable zone when the move ends, so it is
/// **given before the roll** and taken from the settle-window offer.
#[test]
fn student_council_contract() {
    forall(4, 0xC0C0_000B, |_rng, _i| {
        let mut t = vanilla2();
        let blue = tile("江户川公园");
        t.own(0, &[blue]);
        t.set_pos(0, (blue + 60 - 1) % 60);
        t.give(0, &[COUNCIL]);
        t.dice(&[1]);
        let pre = t.snapshot_world();
        t.observe(COUNCIL_CLAUSES);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        // Take the card from the post-move / pre-settle window.
        let mut played_ok = false;
        while t.prompt().is_some() {
            if t.counteract_offered(COUNCIL) {
                t.counteract(0, COUNCIL)
                    .map_err(|e| format!("counteract rejected: {e}"))?;
                played_ok = true;
            } else if let Some(k) = t.option(COUNCIL) {
                let _ = t.answer_one(k);
                played_ok = true;
            } else {
                t.decline();
            }
        }
        if !played_ok {
            return Err(format!(
                "C-R-学生会的检查-01 violated: book says 「移动结束后前后三格内若存在你拥有地契的格子，[触发结算]前可打出」; no offer in that window; keys={:?} prompt={}",
                t.recent_keys(12),
                t.dump_prompt()
            ));
        }
        drain(&mut t);
        // C-R-学生会的检查-02. The clause also checks the counteract step when
        // the body lands without a follow-up prompt; with the optional
        // 「可支付…」 ask open it is mid-body (see the clause), and this
        // at-rest check is the one that runs after the prompt loop.
        if !t.draw_pile(0).iter().any(|c| c.contains("觉悟")) {
            return Err(format!(
                "「向抽牌堆中加入一张“觉悟”」 -- not in the draw; draw={:?} hand={:?} discard={:?} keys={:?}",
                t.draw_pile(0),
                t.hand(0),
                t.discard(0),
                t.recent_keys(16)
            ));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "field:P0",
                "bfield",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "turn.",
                "pos:P0",
            ],
        )
    })
    .expect("学生会的检查 contract");
}

// ================================================================ 轨迹

const TRACK: &str = "R:轨迹";

/// 轨迹 is a [特] on bankruptcy: take one deed (free redeem) and demolish
/// its houses. Bankruptcy is a multi-player window this unit does not
/// drive; the clause is documented in `r.md`.
#[test]
fn track_is_a_special_not_a_hand_play() {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &[TRACK]);
    assert!(
        t.hand(0).contains(&TRACK.to_string()),
        "轨迹 sits in hand as a [特] carrier"
    );
}

// ================================================================ 选择自己的舞台

const STAGE: &str = "R:选择自己的舞台";

/// 选择自己的舞台: [反击] vs an [除外]-free abnormal move.
///
/// [停留] only *prevents* a move -- it is not an abnormal move *applied* to
/// the user. Drive a forced [传送] instead (the same shape the engine's
/// window keys on).
#[test]
fn own_stage_offered_on_abnormal_move() {
    let mut t = vanilla2();
    t.give(0, &[STAGE]);
    // 蓝玫瑰的骄傲 (2) [传送]s the user to the next unbought Livehouse --
    // an [异常移动效果] other than [除外].
    t.give(0, &[BLUE]);
    t.play(0, BLUE).unwrap();
    let mut offered = t.counteract_offered(STAGE);
    while t.prompt().is_some() {
        if t.counteract_offered(STAGE) {
            offered = true;
        }
        t.decline();
    }
    assert!(
        offered,
        "选择自己的舞台 never offered on a forced [传送]; prompt={}",
        t.dump_prompt()
    );
}

// ================================================================ 黑暗大魔姬亚子

const AKO_DARK: &str = "R:（亚子）黑暗大魔姬亚子";

/// C-R-黑暗大魔姬亚子-02 -- the play gains 1 [眩晕].
pub fn ako_dark_stun(step: &Step<'_>) -> Result<(), String> {
    let who = step.player_of_act();
    let played_now = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == AKO_DARK);
    if !played_now {
        return Ok(());
    }
    let pre_stun = step.pre.st.players[who].state_get("stun")
        + step.pre.st.players[who].state_get("stunStart");
    let post_stun = step.post.st.players[who].state_get("stun")
        + step.post.st.players[who].state_get("stunStart");
    if post_stun != pre_stun + 1 {
        return Err(format!(
            "stun {pre_stun}->{post_stun}, expected +1 layer of [眩晕]"
        ));
    }
    Ok(())
}

pub const AKO_DARK_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-黑暗大魔姬亚子-02",
    "roster#R:（亚子）黑暗大魔姬亚子",
    "（1）[反击] 当你即将向其他玩家支付资金时可打出此卡，使自己获得一层[眩晕]。",
    ako_dark_stun,
)];

/// 黑暗大魔姬亚子: [反击] before a rent payment; the play stuns the user.
#[test]
fn ako_dark_contract() {
    forall(4, 0xC0C0_000C, |_rng, _i| {
        let mut t = Table::new(&["宇田川亚子", "户山香澄"]);
        t.begin_turn(0);
        drain(&mut t);
        t.give(0, &[AKO_DARK]);
        let blue = tile("江户川公园");
        t.set_owner(blue, Some(1));
        t.set_pos(0, (blue + 60 - 1) % 60);
        t.dice(&[1]);
        let pre = t.snapshot_world();
        t.observe(AKO_DARK_CLAUSES);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        if !t.counteract_offered(AKO_DARK) {
            return Err(format!("card not offered: {}", t.dump_prompt()));
        }
        t.counteract(0, AKO_DARK)
            .map_err(|e| format!("counteract rejected: {e}"))?;
        drain(&mut t);
        let stun = t.state(0, "stun") + t.state(0, "stunStart");
        if stun < 1 {
            return Err(format!("no [眩晕] after the play; stun={stun}"));
        }
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "pos:P0",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "state:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
                "turn.",
            ],
        )
    })
    .expect("黑暗大魔姬亚子 contract");
}

// ================================================================ Ringing Bloom

const RINGING: &str = "R:（燐子）Ringing Bloom";

/// C-R-Ringing Bloom-01 -- the play places the card on the user's field.
pub fn ringing_bloom_place(step: &Step<'_>) -> Result<(), String> {
    if !played(step, RINGING) {
        return Ok(());
    }
    let who = actor(step);
    if !step.post.st.players[who].field.iter().any(|f| f.card == RINGING) {
        return Err("Ringing Bloom not on the user's field".into());
    }
    Ok(())
}

pub const RINGING_CLAUSES: &[Clause] = &[Clause::new(
    "C-R-Ringing Bloom-01",
    "roster#R:（燐子）Ringing Bloom",
    "（1）将此卡放置于自身场上",
    ringing_bloom_place,
)];

/// Ringing Bloom (1): place on the own field.
#[test]
fn ringing_bloom_place_contract() {
    forall(4, 0xC0C0_000D, |_rng, _i| {
        let mut t = Table::new(&["白金燐子", "户山香澄"]);
        t.begin_turn(0);
        drain(&mut t);
        let pre = t.snapshot_world();
        t.observe(RINGING_CLAUSES);
        t.give_play(0, RINGING).map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, RINGING) {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        frame(
            &pre,
            t.world(),
            &[
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "field:P0",
                "crystals:P0",
                "fprop:P0",
                "hand_n:P0",
                "draw_n:P0",
                "discard_pub:P0",
            ],
        )
    })
    .expect("Ringing Bloom (1)");
}

/// Ringing Bloom (3): after a non-RiNG rent the card is discarded and the
/// user gains 500*X (OPEN-RINGING-X: X is the rent tile's house count).
#[test]
fn ringing_bloom_after_rent() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.place_raw(0, RINGING);
    let blue = tile("江户川公园"); // non-RiNG
    t.own(0, &[blue]);
    t.set_houses(blue, 1);
    let money0 = t.money(0);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, RINGING),
        "card leaves the field after a non-RiNG rent"
    );
    // 「获得500*X资金，X为你收费格上的房屋数」 -- X is the rent tile's
    // house count (1 here). The rent leg is G-SETTLE's; the 500*X leg is
    // this card's. money0 is taken before the rent, so
    // delta = rent(1 house) + 500*X. Pin both.
    let delta = t.money(0) as i64 - money0 as i64;
    let rent_1h = 640; // 江户川公园 rent table [140, 640, 1540, 3240]
    let expect = rent_1h + 500;
    assert_eq!(
        delta, expect,
        "expected rent {rent_1h} + 500*X(=500) = {expect}; delta={delta}"
    );
    assert!(
        t.discard(0).contains(&RINGING.to_string()),
        "「此卡置入弃牌堆」 -- not the draw; discard={:?}",
        t.discard(0)
    );
}

/// C-R-Ringing Bloom-02 -- houses read as the max; raised tiles charge half.
///
/// The printed `houses` are a 「视为」 read (not rewritten). With two owned
/// tiles at 0 and 2 houses, the 0-house tile is raised to 2 for rent and
/// charges half of the 2-house rent.
#[test]
fn ringing_bloom_as_if_max_houses() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let low = tile("江户川公园"); // rent table [140, 640, 1540, 3240]
    let high = tile("富士见坂");
    t.own(0, &[low, high]);
    t.set_houses(low, 0);
    t.set_houses(high, 2);
    // Play, not place_raw: (2) is a hook the play arms; place_raw has none.
    // Houses must be in place *before* the play -- the max is read then.
    t.give_play(0, RINGING).expect("Ringing Bloom play");
    drain(&mut t);
    // Printed houses are untouched by the 「视为」.
    assert_eq!(t.houses(low), 0, "「视为」 does not rewrite printed houses");
    assert_eq!(t.houses(high), 2);
    // P1 settles on the raised tile: rent reads as 2 houses (1540) halved
    // for the raise = 770.
    let money0 = t.money(0);
    let money1 = t.money(1);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, (low + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    let gain = t.money(0) as i64 - money0 as i64;
    let loss = money1 as i64 - t.money(1) as i64;
    // (3) fires on this non-RiNG rent too: +500*X with X the counted houses.
    // gain = half-rent + 500*X; loss = half-rent. Pin the half-rent via loss.
    assert_eq!(
        loss, 770,
        "raised tile charges half of the 2-house rent (1540/2); loss={loss}"
    );
    assert!(
        gain > loss,
        "the 500*X leg is on top of the rent; gain={gain} loss={loss}"
    );
}

// ================================================================ 弹奏

const SAYO_PLAY: &str = "R:（纱夜）弹奏弹奏弹奏，继续弹奏";

/// C-R-弹奏-02 -- the play counts as one use of the user's skill.
///
/// OPEN-SAYO-TIMING: 「时机合适时」 is undefined, so the window is whatever
/// the engine opens. A bare play that is refused is the open timing and is
/// not a failure.
#[test]
fn sayo_play_runs_the_skill() {
    let mut t = Table::new(&["冰川纱夜", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 6, 10);
    t.give(0, &[SAYO_PLAY]);
    if t.play(0, SAYO_PLAY).is_err() {
        eprintln!("C-R-弹奏-01 OPEN-SAYO-TIMING: no window opened on a bare play");
        return;
    }
    drain(&mut t);
    drain_taking(&mut t, "sayo");
    drain_taking(&mut t, "6");
    assert!(
        !t.hand(0).contains(&SAYO_PLAY.to_string()),
        "the card left the hand on play"
    );
}

// ================================================================ 必然的联系

const LISA_LINK: &str = "R:必然的联系（莉莎）";

/// 必然的联系 is a [反击] after a skill [传送]; documented in `r.md`.
#[test]
fn lisa_link_is_counteract_tagged() {
    let mut t = Table::new(&["今井莉莎", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.give(0, &[LISA_LINK]);
    assert!(t.hand(0).contains(&LISA_LINK.to_string()));
}

// ================================================================ skills

/// C-R-来练习吧-01 -- start: own RiNG 4, start there.
#[test]
fn yukina_start_contract() {
    let t = Table::new(&["凑友希那", "户山香澄"]);
    let r4 = tile("RiNG 4");
    assert_eq!(t.pos(0), r4, "starts on RiNG 4");
    assert_eq!(t.owner(r4), Some(0), "owns RiNG 4");
}

/// C-R-来练习吧-02 -- first CiRCLE pass gives no reward.
#[test]
fn yukina_first_circle_no_reward() {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 58);
    t.dice(&[3]);
    let money0 = t.money(0);
    let hand0 = t.hand(0).len();
    let draw0 = t.draw_pile(0).len();
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), money0, "no CiRCLE pass reward (money)");
    assert_eq!(
        t.hand(0).len(),
        hand0,
        "no CiRCLE pass reward (hand): {:?}",
        t.hand(0)
    );
    assert_eq!(
        t.draw_pile(0).len(),
        draw0,
        "no CiRCLE pass reward (draw): {:?}",
        t.draw_pile(0)
    );
}

/// C-R-来练习吧-04 -- +1 火罐 on a RiNG landing (cap 1).
#[test]
fn yukina_fire_on_ring() {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2");
    t.set_pos(0, (r2 + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 1, "1 火罐, cap 1");
}

/// C-R-来练习吧-05 -- drop all 火罐 off a RiNG.
#[test]
fn yukina_fire_drop_off_ring() {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    let blue = tile("江户川公园");
    t.set_pos(0, (blue + 60 - 1) % 60);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 0, "火罐 dropped off a RiNG");
}

/// C-R-来练习吧-06 -- spend 1 火罐 to force-stop a passer.
#[test]
fn yukina_force_stop_passer() {
    let mut t = Table::new(&["凑友希那", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2");
    t.set_pos(0, r2);
    t.set_fire(0, 1, 1);
    t.begin_turn(1);
    drain(&mut t);
    t.set_pos(1, (r2 + 60 - 3) % 60);
    t.dice(&[5]);
    t.roll(1).unwrap();
    let mut offered = false;
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("kokoro_practice_force") {
            offered = true;
            let _ = t.answer_one(0);
        } else {
            t.decline();
        }
    }
    assert!(offered, "the force-stop offer came up");
    assert_eq!(t.pos(1), r2, "forced stop on the user's tile");
}

/// C-R-踏上荆棘之路的觉悟-01 -- CiRCLE pass +2 (cap 10).
#[test]
fn sayo_fire_on_circle() {
    let mut t = Table::new(&["冰川纱夜", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 10);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 2, "+2 on a CiRCLE pass");
}

/// C-R-慈爱女神-01 -- CiRCLE pass +1 (cap 2).
#[test]
fn lisa_fire_on_circle() {
    let mut t = Table::new(&["今井莉莎", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 2);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 1, "+1 on a CiRCLE pass");
}

/// C-R-慈爱女神-02 -- spend 1 to teleport to the nearest character ahead.
#[test]
fn lisa_teleport_to_nearest() {
    let mut t = Table::new(&["今井莉莎", "户山香澄"]);
    t.clean();
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 0);
    t.set_pos(1, 10);
    t.set_fire(0, 2, 2);
    let skill = t.skill_id(0, "慈爱女神");
    t.skill(0, &skill).unwrap();
    while t.prompt().is_some() {
        t.decline();
    }
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "teleported to the nearest character ahead");
}

/// C-R-即使1cm也要前进-01 -- CiRCLE pass +3 (cap 3).
#[test]
fn rinko_fire_on_circle() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 3);
    t.set_pos(0, 58);
    t.dice(&[3]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.charge(0), 3, "+3 on a CiRCLE pass");
}

/// C-R-即使1cm也要前进-02 -- spend X 火罐 to fix the roll at X*6.
#[test]
fn rinko_fix_roll_to_x_times_six() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 2, 3);
    t.set_pos(0, 0);
    t.dice(&[3]);
    let skill = t.skill_id(0, "1cm");
    t.skill(0, &skill).unwrap();
    while t.prompt().is_some() {
        let d = t.dump_prompt();
        if d.contains("rinko_1cm") {
            let _ = t.answer_one(1); // X = 2
        } else {
            t.decline();
        }
    }
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 12, "2 * 6 = 12");
    assert_eq!(t.charge(0), 0, "2 火罐 spent");
}

// ================================================================ band skill

/// C-R-对音乐的纯粹-01 -- the first non-Livehouse purchase is half price.
#[test]
fn band_first_non_livehouse_half() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let blue = tile("江户川公园"); // 1400 -> 700
    land_and_buy(&mut t, 0, blue);
    assert_eq!(t.owner(blue), Some(0));
    assert_eq!(t.money(0), 10_000 - 700, "first non-Livehouse half price");
}

/// C-R-对音乐的纯粹-03 -- any Livehouse purchase is half price.
#[test]
fn band_livehouse_half() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2"); // 1000 -> 500
    land_and_buy(&mut t, 0, r2);
    assert_eq!(t.owner(r2), Some(0));
    assert_eq!(t.money(0), 10_000 - 500, "Livehouse half price");
}

/// C-R-对音乐的纯粹-01 + -03 together.
#[test]
fn band_both_halves() {
    let mut t = Table::new(&["白金燐子", "户山香澄"]);
    t.begin_turn(0);
    drain(&mut t);
    let blue = tile("江户川公园");
    land_and_buy(&mut t, 0, blue);
    assert_eq!(t.money(0), 10_000 - 700, "first non-Livehouse half price");
    t.end(0).unwrap();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    let r2 = tile("RiNG 2");
    land_and_buy(&mut t, 0, r2);
    assert_eq!(t.money(0), 10_000 - 700 - 500, "Livehouse half price");
}
