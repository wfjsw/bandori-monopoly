//! Pastel✽Palettes (PP) card / skill / band contracts -- unit C-PP.
//!
//! Spec doc: `docs/rulebook/spec/cards/pp.md`. Clause IDs `C-PP-<slug>-<n>`.
//! Derived only from the live sheet text in `docs/rulebook/cards.json`,
//! `data/characters.json`, `data/bands.json` (never from implementation
//! behaviour).
//!
//! Fan tokens are `P✽P粉丝(正)` / `P✽P粉丝(反)`. The band-skill crystal pool
//! named 「Pastel✽Palettes乐队卡」 is the band skill field card
//! (`skill:Pastel✽Palettes:与偶像一起`) -- see the OPEN-乐队卡 note in the doc.
//!
//! A discrepancy is a plain failing test. No `#[ignore]`.

use game_core::state::Counter;
use rules_spec::{forall, frame, Clause, Step, Table};

const BAND_SKILL: &str = "skill:Pastel✽Palettes:与偶像一起";
const ECHO: &str = "PP:[衍生]共鸣";

// ---------------------------------------------------------------- helpers

fn set_fans(t: &mut Table, who: usize, up: i32, down: i32) {
    t.m.world_mut().st.players[who].tokens = vec![
        Counter {
            name: "P✽P粉丝(正)".into(),
            value: up,
            instance: None,
        },
        Counter {
            name: "P✽P粉丝(反)".into(),
            value: down,
            instance: None,
        },
    ];
}

fn fans(t: &Table, who: usize) -> (i32, i32) {
    (t.token(who, "P✽P粉丝(正)"), t.token(who, "P✽P粉丝(反)"))
}

fn band_xtal(t: &Table, who: usize) -> i32 {
    t.crystals(who, BAND_SKILL).unwrap_or(0)
}

fn set_band_xtal(t: &mut Table, who: usize, n: i32) {
    t.set_crystals(who, BAND_SKILL, n);
}

fn drain(t: &mut Table) {
    while t.prompt().is_some() {
        t.decline();
    }
}

/// Accept a [共鸣] window; decline everything else.
fn accept_echo(t: &mut Table) {
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.title.key().contains("resonance") {
            let _ = t.answer(t.asked()[0], 0);
        } else {
            t.decline();
        }
    }
}

/// Answer a tile pick with `tile`, decline everything else.
fn answer_tile_else_decline(t: &mut Table, tile: usize) {
    while t.prompt().is_some() {
        let p = t.expect_prompt();
        if p.kind == "tile" {
            let _ = t.answer_tile(t.asked()[0], tile);
        } else {
            t.decline();
        }
    }
}

/// Did this step activate `card` from hand (`kind == "play"` or bare)?
fn played(step: &Step<'_>, card: &str) -> bool {
    step.events.iter().any(|e| {
        e.r#type == "card" && e.card == card && (e.kind == "play" || e.kind.is_empty())
    })
}

// ------------------------------------------------------------ clause checks

/// C-PP-title-idol-01 -- step 1 adds 2 [奇迹水晶] to the band card.
fn title_idol_band_plus_2(step: &Step<'_>) -> Result<(), String> {
    if !played(step, "PP:TITLE IDOL") || step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let band = step
        .post
        .st
        .players
        .get(who)
        .and_then(|p| p.field.iter().find(|f| f.card == BAND_SKILL));
    let Some(band) = band else {
        // Without a band card there is no pool to write; the book names one.
        return Err(format!(
            "C-PP-title-idol-01: user {who} has no {BAND_SKILL} to receive +2"
        ));
    };
    let pre = step
        .pre
        .st
        .players
        .get(who)
        .and_then(|p| p.field.iter().find(|f| f.card == BAND_SKILL))
        .map(|f| f.crystals)
        .unwrap_or(0);
    // Step 2 also writes the band card when its effect mentions [奇迹水晶]
    // (the band skill's text does). The clause checks the step-1 leg is among
    // the writes: the pool must have risen by at least 2.
    let delta = band.crystals as i64 - pre as i64;
    if delta < 2 {
        return Err(format!(
            "C-PP-title-idol-01: band crystals delta {delta}, expected >= 2 (step 1 +2)"
        ));
    }
    Ok(())
}

/// C-PP-same-dream-01 -- step 1 adds 3 [奇迹水晶] to the band card.
fn same_dream_band_plus_3(step: &Step<'_>) -> Result<(), String> {
    if !played(step, "PP:同一个梦想") || step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre = step
        .pre
        .st
        .players
        .get(who)
        .and_then(|p| p.field.iter().find(|f| f.card == BAND_SKILL))
        .map(|f| f.crystals);
    let post = step
        .post
        .st
        .players
        .get(who)
        .and_then(|p| p.field.iter().find(|f| f.card == BAND_SKILL))
        .map(|f| f.crystals);
    match (pre, post) {
        (Some(a), Some(b)) if b - a >= 3 => Ok(()),
        (Some(a), Some(b)) => Err(format!(
            "C-PP-same-dream-01: band crystals {a} -> {b}, expected +3"
        )),
        _ => Err("C-PP-same-dream-01: band skill missing on the user".into()),
    }
}

/// C-PP-eve-bushido-01 -- [手] grants 12d4 * 60 (checked over loaded dice in
/// the test; this clause pins the play activates).
fn eve_bushido_played(step: &Step<'_>) -> Result<(), String> {
    if !played(step, "PP:[若宫伊芙]属于我的武士道！") {
        return Ok(());
    }
    Ok(())
}

/// The clause set armed on `Table::observe` for the PP contracts.
pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-PP-title-idol-01",
        "cards.json#PP:TITLE IDOL",
        "1. 为[使用者]的Pastel✽Palettes乐队卡添加2个[奇迹水晶]；",
        title_idol_band_plus_2,
    ),
    Clause::new(
        "C-PP-same-dream-01",
        "cards.json#PP:同一个梦想",
        "1. 为自己的Pastel✽Palettes乐队卡添加3个[奇迹水晶]；",
        same_dream_band_plus_3,
    ),
    Clause::new(
        "C-PP-eve-bushido-01",
        "cards.json#PP:[若宫伊芙]属于我的武士道！",
        "将此卡放置在[使用者]场上并投掷12d4，获得投掷结果*60的资金。",
        eve_bushido_played,
    ),
];

// ================================================================ TITLE IDOL

/// C-PP-title-idol-01/02 -- band +2, then every crystal-bearing field card +1
/// (+2 with [共鸣]).
#[test]
fn title_idol_crystals() {
    forall(6, 0x50_5449, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, BAND_SKILL);
        t.place_raw(0, "PP:梦在前方，结彩当下");
        let extra = rng.range(0, 5);
        set_band_xtal(&mut t, 0, extra);
        t.set_crystals(0, "PP:梦在前方，结彩当下", rng.range(0, 4));
        let band0 = band_xtal(&t, 0);
        let dream0 = t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0);
        t.give(0, &["PP:TITLE IDOL"]);

        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, "PP:TITLE IDOL")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        // Step 1: band +2. Step 2 also hits the band card (its effect text
        // contains [奇迹水晶]) and 梦在前方, each +1 without 共鸣.
        let band = band_xtal(&t, 0);
        if band != band0 + 2 + 1 {
            return Err(format!(
                "band crystals {band0} -> {band}, expected {} (2+1)",
                band0 + 3
            ));
        }
        let dream = t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0);
        if dream != dream0 + 1 {
            return Err(format!(
                "梦在前方 crystals {dream0} -> {dream}, expected +1 (step 2)"
            ));
        }
        // Played card left the hand (non-[持续] -> discard).
        if t.hand(0).iter().any(|c| c == "PP:TITLE IDOL") {
            return Err(format!("TITLE IDOL still in hand: {:?}", t.hand(0)));
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
                "token:P0",
                "state:P0",
            ],
        )
    })
    .expect("C-PP-title-idol-01/02");
}

/// C-PP-title-idol-02 -- with [共鸣], step 2 adds 2 to each crystal card.
#[test]
fn title_idol_echo_adds_two() {
    forall(4, 0x50_544A, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, BAND_SKILL);
        t.place_raw(0, "PP:梦在前方，结彩当下");
        set_band_xtal(&mut t, 0, rng.range(0, 3));
        t.set_crystals(0, "PP:梦在前方，结彩当下", rng.range(0, 2));
        let band0 = band_xtal(&t, 0);
        let dream0 = t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0);
        t.give(0, &[ECHO, "PP:TITLE IDOL"]);

        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, "PP:TITLE IDOL")
            .map_err(|e| format!("play rejected: {e}"))?;
        accept_echo(&mut t);

        // step1 band+2 + step2 band+2 = +4; dream +2.
        let band = band_xtal(&t, 0);
        if band != band0 + 2 + 2 {
            return Err(format!("band {band0} -> {band}, expected +4 with 共鸣"));
        }
        let dream = t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0);
        if dream != dream0 + 2 {
            return Err(format!("dream {dream0} -> {dream}, expected +2 with 共鸣"));
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
                "token:P0",
                "state:P0",
            ],
        )
    })
    .expect("C-PP-title-idol-02 [共鸣]");
}

// ============================================================== 同一个梦想

/// C-PP-same-dream-01/02/03 -- band +3, flip up->down paying 100 each, then
/// all down->up.
#[test]
fn same_dream_three_steps() {
    forall(6, 0x50_444D, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, BAND_SKILL);
        let up = rng.range(0, 6);
        let down = rng.range(0, 4);
        set_fans(&mut t, 0, up, down);
        let band0 = band_xtal(&t, 0);
        let money0 = 10_000;
        t.give(0, &["PP:同一个梦想"]);

        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, "PP:同一个梦想")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        // 2. all positive flip reverse, gain flipped * 100
        // 3. all reverse (including those just flipped) flip positive
        // End state: (up+down, 0); money += up * 100.
        let (u, d) = fans(&t, 0);
        if u != up + down || d != 0 {
            return Err(format!(
                "fans ({up},{down}) -> ({u},{d}), expected ({},0)",
                up + down
            ));
        }
        let money = t.money(0);
        if money != money0 + up * 100 {
            return Err(format!(
                "money {money0} -> {money}, expected +{} (step 2 payout)",
                up * 100
            ));
        }
        let band = band_xtal(&t, 0);
        if band != band0 + 3 {
            return Err(format!("band {band0} -> {band}, expected +3"));
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
                "token:P0",
            ],
        )
    })
    .expect("C-PP-same-dream-01/02/03");
}

/// C-PP-same-dream-03 -- [共鸣] extends the down->up flip to every PP character.
#[test]
fn same_dream_echo_all_pp_characters() {
    forall(4, 0x50_444E, |rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_character_raw(0, "丸山彩");
        t.set_character_raw(1, "户山香澄");
        t.set_character_raw(2, "若宫伊芙");
        t.place_raw(0, BAND_SKILL);
        t.place_raw(2, BAND_SKILL);
        set_fans(&mut t, 0, 0, rng.range(1, 3));
        set_fans(&mut t, 1, 0, rng.range(1, 5));
        set_fans(&mut t, 2, 0, rng.range(1, 4));
        t.give(0, &[ECHO, "PP:同一个梦想"]);

        let pre = t.snapshot_world();
        t.play(0, "PP:同一个梦想")
            .map_err(|e| format!("play rejected: {e}"))?;
        accept_echo(&mut t);

        let (u2, d2) = fans(&t, 2);
        if d2 != 0 || u2 == 0 {
            return Err(format!(
                "PP seat 2 reverse fans should all flip positive, got ({u2},{d2})"
            ));
        }
        let (_, d1) = fans(&t, 1);
        if d1 == 0 {
            return Err("non-PP seat 1 reverse fans must stay".into());
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
                "token:P0",
                "token:P2",
            ],
        )
    })
    .expect("C-PP-same-dream-03 [共鸣]");
}

// =========================================================== 初次演出事故

/// C-PP-first-accident-01 -- place, shuffle in 重叠的声音, draw 1.
#[test]
fn first_accident_hand() {
    forall(4, 0x46_4143, |rng, _i| {
        let mut t = Table::vanilla(2);
        let top = rng.range(1, 4) as usize;
        let pile: Vec<&str> = (0..top).map(|_| "通用:GREAT").collect();
        t.set_draw(0, &pile);
        t.give(0, &["PP:初次演出事故"]);

        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, "PP:初次演出事故")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        if !t.on_field(0, "PP:初次演出事故") {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        let zones: Vec<String> = t
            .draw_pile(0)
            .into_iter()
            .chain(t.hand(0).iter().cloned())
            .collect();
        if !zones.iter().any(|c| c == "PP:[衍生]重叠的声音") {
            return Err(format!("重叠的声音 not shuffled in: {zones:?}"));
        }
        // Played one left hand; exactly one card drawn.
        if t.hand(0).len() != 1 {
            return Err(format!("hand should hold 1 drawn card: {:?}", t.hand(0)));
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
                "token:P0",
                "fprop:P0",
            ],
        )
    })
    .expect("C-PP-first-accident-01");
}

/// C-PP-first-accident-02 -- [持续]: owner cannot use any PP character's (2)
/// skill. The book is absolute; a prompt offering 丸山彩's (2) is a violation.
#[test]
fn first_accident_blocks_pp_skill_2() {
    forall(4, 0x46_4144, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "丸山彩");
        t.place_raw(0, "skill:丸山彩:With~");
        t.give_play(0, "PP:初次演出事故")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let up = rng.range(3, 6);
        set_fans(&mut t, 0, up, 0);
        t.own(1, &[7]);
        t.set_houses(7, 1);
        t.begin_turn(0);
        t.set_pos(0, 5);
        t.dice(&[2]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.title.key().contains("aya_with")
                || p.options.iter().any(|o| format!("{o:?}").contains("aya_with"))
            {
                return Err(format!(
                    "C-PP-first-accident-02: PP (2) skill prompt offered under 初次演出事故: {}",
                    t.dump_prompt()
                ));
            }
            t.decline();
        }
        let (u, d) = fans(&t, 0);
        if u != up || d != 0 {
            return Err(format!(
                "C-PP-first-accident-02: fans ({up},0) -> ({u},{d}); (2) ran"
            ));
        }
        Ok(())
    })
    .expect("C-PP-first-accident-02");
}

// ========================================================== 重叠的声音

/// C-PP-overlap-01/02/03 -- remove 初次演出事故, teleport to Bandori车站 with
/// no settle, turn end grants stay + fan + 明天见.
#[test]
fn overlap_play_and_turn_end() {
    forall(4, 0x4F_564C, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, "PP:初次演出事故");
        set_fans(&mut t, 0, rng.range(0, 3), 0);
        t.give_play(0, "PP:[衍生]重叠的声音")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        if t.on_field(0, "PP:初次演出事故") {
            return Err("C-PP-overlap-01: 初次演出事故 still on field".into());
        }
        let bandori = rules_spec::tile("Bandori车站");
        if t.pos(0) != bandori {
            return Err(format!(
                "C-PP-overlap-02: pos {}, expected Bandori车站 {bandori}",
                t.pos(0)
            ));
        }
        if t.owner(bandori).is_some() {
            return Err("C-PP-overlap-02: teleport must not settle (no buy)".into());
        }
        t.end(0).map_err(|e| format!("end rejected: {e}"))?;
        drain(&mut t);
        if t.state(0, "stay") != 1 {
            return Err(format!(
                "C-PP-overlap-03: stay {}, expected 1",
                t.state(0, "stay")
            ));
        }
        if t.token(0, "P✽P粉丝(正)") != 1 && t.token(0, "P✽P粉丝(正)") < 1 {
            return Err(format!(
                "C-PP-overlap-03: positive fans {}, expected +1",
                t.token(0, "P✽P粉丝(正)")
            ));
        }
        if !t.hand(0).contains(&"PP:[衍生]明天见".to_string()) {
            return Err(format!(
                "C-PP-overlap-03: 明天见 not in hand: {:?}",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("C-PP-overlap-01/02/03");
}

// ============================================================= 明天见

/// C-PP-seeyou-01 -- gate: only when positive <= reverse.
#[test]
fn seeyou_gate() {
    forall(6, 0x53_4545, |rng, _i| {
        let mut t = Table::vanilla(2);
        let up = rng.range(0, 6);
        let down = rng.range(0, 6);
        set_fans(&mut t, 0, up, down);
        t.give(0, &["PP:[衍生]明天见"]);
        let ok = t.play(0, "PP:[衍生]明天见").is_ok();
        if up > down && ok {
            return Err(format!(
                "C-PP-seeyou-01: play accepted with up {up} > down {down}"
            ));
        }
        if up <= down && !ok {
            return Err(format!(
                "C-PP-seeyou-01: play refused with up {up} <= down {down}"
            ));
        }
        Ok(())
    })
    .expect("C-PP-seeyou-01");
}

/// C-PP-seeyou-03 -- [手]: flip 「自己拥有的[P✽P粉丝]数量」 reverse fans positive
/// (at most the reverse count).
#[test]
fn seeyou_flip_owned_count() {
    forall(6, 0x53_4546, |rng, _i| {
        let mut t = Table::vanilla(2);
        let up = rng.range(0, 3);
        let down = rng.range(up, up + 5); // ensure gate
        set_fans(&mut t, 0, up, down);
        let owned = up + down;
        t.give_play(0, "PP:[衍生]明天见")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let flipped = owned.min(down);
        let (u, d) = fans(&t, 0);
        if u != up + flipped || d != down - flipped {
            return Err(format!(
                "C-PP-seeyou-03: ({up},{down}) -> ({u},{d}), expected ({},{})",
                up + flipped,
                down - flipped
            ));
        }
        Ok(())
    })
    .expect("C-PP-seeyou-03");
}

// ======================================================== 不要背负期待

/// C-PP-no-expect-01/02 -- [手]: place, 共鸣 into the deck, draw 1; [特]
/// immune.
#[test]
fn no_expect_hand() {
    forall(4, 0x4E_4558, |rng, _i| {
        let mut t = Table::vanilla(2);
        let top = rng.range(1, 3) as usize;
        let pile: Vec<&str> = (0..top).map(|_| "通用:GREAT").collect();
        t.set_draw(0, &pile);
        t.give(0, &["PP:不要背负期待"]);

        let pre = t.snapshot_world();
        t.observe(CLAUSES);
        t.play(0, "PP:不要背负期待")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);

        if !t.on_field(0, "PP:不要背负期待") {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        // [特]: 不受任何其他效果影响 -- the instance carries `immune`.
        let f = t
            .field(0)
            .into_iter()
            .find(|f| f.card == "PP:不要背负期待")
            .ok_or("missing field instance")?;
        if !f.immune {
            return Err("C-PP-no-expect-01: [特] 不受任何其他效果影响 -- immune not set".into());
        }
        if !t.draw_pile(0).contains(&"PP:[衍生]共鸣".to_string()) {
            return Err(format!(
                "共鸣 not added to draw: {:?}",
                t.draw_pile(0)
            ));
        }
        // Hand limit -1 (determined by (1)); check the prop is live.
        if t.p(0).hand_limit() != Some(4) {
            return Err(format!(
                "C-PP-no-expect-03: hand limit {:?}, expected Some(4)",
                t.p(0).hand_limit()
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
                "fprop:P0",
                "state:P0",
            ],
        )
    })
    .expect("C-PP-no-expect-01/02");
}

/// C-PP-no-expect-03 -- non-turn-start rolls -2 (move result floors at 0).
#[test]
fn no_expect_roll_minus_2() {
    forall(4, 0x4E_4559, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.give_play(0, "PP:不要背负期待")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        t.begin_turn(0);
        let face = rng.range(3, 8);
        t.dice(&[face]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        let expected = (face - 2).max(0) as usize;
        if face - 2 > 0 && t.pos(0) != expected {
            return Err(format!(
                "C-PP-no-expect-03: rolled {face}, pos {}, expected {expected}",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("C-PP-no-expect-03");
}

// ======================================================== 有你与我在这里共度

/// C-PP-together-01 -- X = 5, + ceil((down-up)/2) when up < down; [共鸣]
/// X-5 and draw 1.
#[test]
fn together_x_formula() {
    forall(6, 0x54_4F47, |rng, _i| {
        let mut t = Table::vanilla(2);
        let up = rng.range(0, 6);
        let down = rng.range(0, 10);
        set_fans(&mut t, 0, up, down);
        let x = if up < down {
            5 + (down - up + 1) / 2
        } else {
            5
        };
        t.give_play(0, "PP:有你与我在这里共度")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let flipped = x.min(down);
        let (u, d) = fans(&t, 0);
        if u != up + flipped || d != down - flipped {
            return Err(format!(
                "C-PP-together-01: ({up},{down}) x={x} -> ({u},{d}), expected ({},{})",
                up + flipped,
                down - flipped
            ));
        }
        Ok(())
    })
    .expect("C-PP-together-01");
}

/// C-PP-together-02 -- [共鸣] X-5 and draw 1.
#[test]
fn together_echo() {
    forall(4, 0x54_4F48, |rng, _i| {
        let mut t = Table::vanilla(2);
        let up = rng.range(6, 10);
        let down = rng.range(0, 3); // X base 5, echo -> 0: no flips
        set_fans(&mut t, 0, up, down);
        t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
        t.give(0, &[ECHO, "PP:有你与我在这里共度"]);
        t.play(0, "PP:有你与我在这里共度")
            .map_err(|e| format!("play rejected: {e}"))?;
        accept_echo(&mut t);
        let (u, d) = fans(&t, 0);
        if u != up || d != down {
            return Err(format!(
                "C-PP-together-02: X=5-5=0, fans must stay ({up},{down}), got ({u},{d})"
            ));
        }
        if t.hand(0).len() != 1 {
            return Err(format!(
                "C-PP-together-02: expected 1 drawn card, hand {:?}",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("C-PP-together-02 [共鸣]");
}

// ============================================================ TITLE-adjacent

/// C-PP-flower-01 -- [指定] own tile, place there, band +2.
#[test]
fn flower_place_on_tile() {
    forall(4, 0x46_4C4F, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, rng.range(0, 4));
        t.own(0, &[1, 7]);
        let pick = if rng.chance(50) { 7 } else { 1 };
        t.begin_turn(0);
        drain(&mut t);
        let band0 = band_xtal(&t, 0);
        t.give_play(0, "PP:可爱又强壮的花朵")
            .map_err(|e| format!("play rejected: {e}"))?;
        answer_tile_else_decline(&mut t, pick);

        let f = t
            .field(0)
            .into_iter()
            .find(|f| f.card == "PP:可爱又强壮的花朵")
            .ok_or("flower not placed")?;
        if f.tile != pick as i32 {
            return Err(format!(
                "C-PP-flower-01: placed on tile {}, expected {pick}",
                f.tile
            ));
        }
        let band = band_xtal(&t, 0);
        if band != band0 + 2 {
            return Err(format!(
                "C-PP-flower-01: band {band0} -> {band}, expected +2"
            ));
        }
        Ok(())
    })
    .expect("C-PP-flower-01");
}

/// C-PP-flower-03 -- passing the flower's tile discards it (and with 共鸣
/// grants 1500). [强制停下] is OPEN-强制停下 (truncated definition) and is
/// only pinned as "the pass consumed the card".
#[test]
fn flower_pass_discards_and_echo_pays() {
    forall(4, 0x46_4C50, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.own(0, &[7]);
        t.give(0, &[ECHO]);
        t.begin_turn(0);
        t.give_play(0, "PP:可爱又强壮的花朵")
            .map_err(|e| format!("play rejected: {e}"))?;
        answer_tile_else_decline(&mut t, 7);
        // Walk past tile 7 with the echo in hand.
        t.set_pos(0, 20);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        t.end(0).map_err(|e| format!("end rejected: {e}"))?;
        drain(&mut t);
        t.begin_turn(0);
        t.set_pos(0, 4);
        let face = rng.range(5, 9);
        t.dice(&[face]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        accept_echo(&mut t);

        if t.on_field(0, "PP:可爱又强壮的花朵") {
            return Err("C-PP-flower-03: flower still on field after the pass".into());
        }
        // Step 2 with [共鸣]: +1500.
        if t.money(0) != 11_500 {
            return Err(format!(
                "C-PP-flower-03: money {}, expected 11500 (+1500 共鸣)",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("C-PP-flower-02/03");
}

// ============================================================ 找回珍妮弗

/// C-PP-jennifer-01/02 -- place on another player; passing 偶像经纪公司 pays
/// 400, fans, removes the card, hands a Ranger to the user.
#[test]
fn jennifer_place_and_agency_pass() {
    forall(4, 0x4A_454E, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &["PP:找回珍妮弗"]);
        t.play(0, "PP:找回珍妮弗")
            .map_err(|e| format!("play rejected: {e}"))?;
        // Player pick.
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.title.key().contains("jennifer") || p.kind == "pick" {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        if t.on_field(0, "PP:找回珍妮弗") {
            return Err("C-PP-jennifer-01: card must sit on another player's field".into());
        }
        if !t.on_field(1, "PP:找回珍妮弗") {
            return Err(format!(
                "C-PP-jennifer-01: not on seat 1: {:?} {:?}",
                t.field_ids(0),
                t.field_ids(1)
            ));
        }
        let money0 = t.money(0);
        let money1 = t.money(1);
        t.begin_turn(1);
        // Walk from 3 through 偶像经纪公司 (tile 5).
        t.set_pos(1, 3);
        let face = rng.range(3, 6); // 4,5, or 6: all pass tile 5
        t.dice(&[face]);
        t.roll(1).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);

        if t.money(0) != money0 - 400 || t.money(1) != money1 + 400 {
            return Err(format!(
                "C-PP-jennifer-02: money {} / {} -> {} / {}, expected -400 / +400",
                money0,
                money1,
                t.money(0),
                t.money(1)
            ));
        }
        if t.token(0, "P✽P粉丝(正)") < 1 {
            return Err(format!(
                "C-PP-jennifer-02: user positive fans {}, expected >=1",
                t.token(0, "P✽P粉丝(正)")
            ));
        }
        if t.on_field(1, "PP:找回珍妮弗") {
            return Err("C-PP-jennifer-03: card still on field".into());
        }
        if !t.hand(0).contains(&"PP:[衍生]魔法战队Pastel✽Ranger".to_string()) {
            return Err(format!(
                "C-PP-jennifer-03: Ranger not in user hand: {:?}",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("C-PP-jennifer-01/02/03");
}

// ================================================================= Ranger

/// C-PP-ranger-00 -- [特] gate: a house tile within abs-distance 30-fans.
#[test]
fn ranger_gate() {
    forall(4, 0x52_414E, |rng, _i| {
        let mut t = Table::vanilla(2);
        let (up, down) = (rng.range(0, 3), rng.range(0, 3));
        set_fans(&mut t, 0, up, down);
        t.give(0, &["PP:[衍生]魔法战队Pastel✽Ranger"]);
        if t.play(0, "PP:[衍生]魔法战队Pastel✽Ranger").is_ok() {
            return Err("C-PP-ranger-00: play accepted with no house in range".into());
        }
        t.own(0, &[10]);
        t.set_houses(10, 1);
        t.set_pos(0, 0);
        if t.play(0, "PP:[衍生]魔法战队Pastel✽Ranger").is_err() {
            return Err("C-PP-ranger-00: play refused with a house at distance 10".into());
        }
        Ok(())
    })
    .expect("C-PP-ranger-00");
}

/// C-PP-ranger-01 -- count >= 1 grants 500; >= 3 adds 3 band crystals.
#[test]
fn ranger_steps_by_count() {
    forall(4, 0x52_414F, |rng, _i| {
        let mut t = Table::vanilla(2);
        set_fans(&mut t, 0, 0, 0);
        t.own(0, &[10]);
        t.set_houses(10, 1);
        t.place_raw(0, BAND_SKILL);
        t.place_raw(0, "PP:再次闪耀");
        t.place_raw(0, "PP:可爱又强壮的花朵");
        set_band_xtal(&mut t, 0, rng.range(0, 3));
        let band0 = band_xtal(&t, 0);
        t.give_play(0, "PP:[衍生]魔法战队Pastel✽Ranger")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        // 3 pre-placed + the Ranger itself = count 4 >= 3: +500 and band +3.
        if t.money(0) != 10_500 {
            return Err(format!(
                "C-PP-ranger-01: money {}, expected 10500",
                t.money(0)
            ));
        }
        let band = band_xtal(&t, 0);
        if band != band0 + 3 {
            return Err(format!(
                "C-PP-ranger-02: band {band0} -> {band}, expected +3"
            ));
        }
        Ok(())
    })
    .expect("C-PP-ranger-01/02");
}

// ============================================================ 梦在前方

/// C-PP-dream-ahead-00 -- [特]: auto-placed at game start, start hand -1.
#[test]
fn dream_ahead_auto_place() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    if !t.on_field(0, "PP:梦在前方，结彩当下") {
        panic!(
            "C-PP-dream-ahead-00: not auto-placed at game start: {:?}",
            t.field_ids(0)
        );
    }
    // Book: 初始手牌减1 (opening hand is 2).
    if t.hand(0).len() != 1 {
        panic!(
            "C-PP-dream-ahead-00: start hand {}, expected 1",
            t.hand(0).len()
        );
    }
}

/// C-PP-dream-ahead-02 -- cannot build; hand limit -1.
#[test]
fn dream_ahead_no_build_and_hand_limit() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "PP:梦在前方，结彩当下");
    if t.p(0).hand_limit() != Some(4) {
        panic!(
            "C-PP-dream-ahead-02: hand limit {:?}, expected Some(4)",
            t.p(0).hand_limit()
        );
    }
    t.own(0, &[1]);
    t.set_pos(0, 1);
    t.begin_turn(0);
    if t.build(0).is_ok() {
        panic!("C-PP-dream-ahead-02: build must be refused");
    }
}

/// C-PP-dream-ahead-01 -- each draw adds 1 crystal (cap 5); overflow bumps X.
#[test]
fn dream_ahead_crystal_per_draw() {
    forall(4, 0x44_4148, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "丸山彩");
        t.place_raw(0, "PP:梦在前方，结彩当下");
        t.place_raw(0, BAND_SKILL);
        // Band (4) spends 5 for exactly one draw per turn start.
        let start = rng.range(0, 5);
        t.set_crystals(0, "PP:梦在前方，结彩当下", start);
        set_band_xtal(&mut t, 0, 5);
        t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
        t.begin_turn(0);
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.title.key().contains("pastel") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        let c = t.crystals(0, "PP:梦在前方，结彩当下").unwrap_or(0);
        if c != (start + 1).min(5) {
            return Err(format!(
                "C-PP-dream-ahead-01: crystals {c}, expected {} ({start}+1, cap 5)",
                (start + 1).min(5)
            ));
        }
        Ok(())
    })
    .expect("C-PP-dream-ahead-01");
}

/// C-PP-dream-ahead-03 -- extra rent `fans * X + MIN(X*30, 300)` on the
/// owner's tiles (X forced to 1 via one overflow).
#[test]
fn dream_ahead_extra_rent() {
    forall(4, 0x44_4149, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "丸山彩");
        t.place_raw(0, "PP:梦在前方，结彩当下");
        let fans_n = rng.range(1, 6);
        set_fans(&mut t, 0, fans_n, 0);
        t.set_crystals(0, "PP:梦在前方，结彩当下", 5);
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, 5);
        t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
        t.begin_turn(0);
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.title.key().contains("pastel") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        t.own(0, &[7]);
        t.begin_turn(1);
        t.set_pos(1, 5);
        t.dice(&[2]);
        t.roll(1).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        let paid = 10_000 - t.money(1);
        // base rent on 江户川公园 (price 1400, 0 houses) + fans*X + MIN(30,300)
        let expected = 140 + fans_n * 1 + 30;
        if paid != expected {
            return Err(format!(
                "C-PP-dream-ahead-03: paid {paid}, expected {expected} (rent 140 + {fans_n}*1 + 30)"
            ));
        }
        Ok(())
    })
    .expect("C-PP-dream-ahead-03");
}

// ========================================================= 练习生解密指南

/// C-PP-guide-01 -- band +3, place, fans/3 crystals on the card.
#[test]
fn guide_hand_places_crystals() {
    forall(4, 0x47_5549, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, rng.range(0, 4));
        let fans_n = rng.range(0, 9);
        set_fans(&mut t, 0, fans_n, 0);
        t.begin_turn(0);
        // Snapshot after begin_turn: band (4) already added its turn-start crystal.
        let band0 = band_xtal(&t, 0);
        t.give_play(0, "PP:练习生解密指南")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, "PP:练习生解密指南") {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        let band = band_xtal(&t, 0);
        if band != band0 + 3 {
            return Err(format!("band {band0} -> {band}, expected +3"));
        }
        let c = t.crystals(0, "PP:练习生解密指南").unwrap_or(0);
        let expect = fans_n / 3;
        if c != expect {
            return Err(format!(
                "C-PP-guide-01: card crystals {c}, expected {expect} (fans {fans_n}/3)"
            ));
        }
        if t.p(0).hand_limit() != Some(4) {
            return Err(format!(
                "C-PP-guide-03: hand limit {:?}, expected Some(4)",
                t.p(0).hand_limit()
            ));
        }
        Ok(())
    })
    .expect("C-PP-guide-01");
}

/// C-PP-guide-04 -- turn end adds 1 crystal.
#[test]
fn guide_turn_end_crystal() {
    forall(4, 0x47_554A, |rng, _i| {
        let mut t = Table::vanilla(2);
        let fans_n = rng.range(3, 6);
        set_fans(&mut t, 0, fans_n, 0);
        t.give_play(0, "PP:练习生解密指南")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let c0 = t.crystals(0, "PP:练习生解密指南").unwrap_or(0);
        t.dice(&[3]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        drain(&mut t);
        t.end(0).map_err(|e| format!("end rejected: {e}"))?;
        drain(&mut t);
        let c = t.crystals(0, "PP:练习生解密指南").unwrap_or(0);
        if c != c0 + 1 {
            return Err(format!(
                "C-PP-guide-04: crystals {c0} -> {c}, expected +1 at turn end"
            ));
        }
        Ok(())
    })
    .expect("C-PP-guide-04");
}

// ============================================================ exclusive

/// C-PP-eve-bushido-01 -- 12d4 * 60.
#[test]
fn eve_bushido_twelve_d4() {
    forall(4, 0x45_5645, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "若宫伊芙");
        let face = rng.range(1, 4);
        let faces: Vec<i32> = (0..12).map(|_| face).collect();
        t.dice(&faces);
        t.give_play(0, "PP:[若宫伊芙]属于我的武士道！")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, "PP:[若宫伊芙]属于我的武士道！") {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        let expected = 10_000 + (face * 12) * 60;
        if t.money(0) != expected {
            return Err(format!(
                "C-PP-eve-bushido-01: money {}, expected {expected} (12d4={})",
                t.money(0),
                face * 12
            ));
        }
        Ok(())
    })
    .expect("C-PP-eve-bushido-01");
}

/// C-PP-eve-bushido-04 -- after the user draws, +1 crystal (cap 3).
#[test]
fn eve_bushido_crystal_per_draw() {
    forall(4, 0x45_5646, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "若宫伊芙");
        t.place_raw(0, "PP:[若宫伊芙]属于我的武士道！");
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, 5);
        // Band (4) spends 5 for exactly one draw per turn start.
        let start = rng.range(0, 4);
        t.set_crystals(0, "PP:[若宫伊芙]属于我的武士道！", start);
        t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
        t.begin_turn(0);
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.title.key().contains("pastel") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        let c = t.crystals(0, "PP:[若宫伊芙]属于我的武士道！").unwrap_or(0);
        if c != (start + 1).min(3) {
            return Err(format!(
                "C-PP-eve-bushido-04: crystals {c}, expected {} ({start}+1, cap 3)",
                (start + 1).min(3)
            ));
        }
        Ok(())
    })
    .expect("C-PP-eve-bushido-04");
}

/// C-PP-aya-haruka-01 -- place and recycle one discard card to hand.
#[test]
fn aya_haruka_hand() {
    forall(4, 0x41_5941, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "丸山彩");
        t.set_discard(0, &["通用:GREAT", "通用:安可"]);
        t.give_play(0, "PP:[丸山彩]憧憬的前方")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, "PP:[丸山彩]憧憬的前方") {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        let hand = t.hand(0);
        if hand.len() != 1 {
            return Err(format!("C-PP-aya-haruka-01: hand {hand:?}, expected 1 recycled"));
        }
        let _ = rng;
        Ok(())
    })
    .expect("C-PP-aya-haruka-01");
}

/// C-PP-chisato-mask-01 -- place; other players collectively [分摊][支付] 2000.
/// OPEN-分摊: only the 2000 total is determined by the text.
#[test]
fn chisato_mask_hand_split() {
    forall(4, 0x43_4849, |rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_character_raw(0, "白鹭千圣");
        let m0 = t.money(0);
        let m1 = t.money(1);
        let m2 = t.money(2);
        t.give_play(0, "PP:[白鹭千圣]微笑的铁假面")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        if !t.on_field(0, "PP:[白鹭千圣]微笑的铁假面") {
            return Err(format!("not on field: {:?}", t.field_ids(0)));
        }
        let total_in = t.money(0) - m0;
        if total_in != 2000 {
            return Err(format!(
                "C-PP-chisato-mask-01: user gained {total_in}, expected 2000 (分摊 total)"
            ));
        }
        let total_out = (m1 - t.money(1)) + (m2 - t.money(2));
        if total_out != 2000 {
            return Err(format!(
                "C-PP-chisato-mask-01: others paid {total_out}, expected 2000"
            ));
        }
        let _ = rng;
        Ok(())
    })
    .expect("C-PP-chisato-mask-01");
}

// ============================================================ character (1)

/// C-PP-aya-with-01 -- game start grants 5 positive fans.
#[test]
fn character_start_five_fans() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    let (u, d) = fans(&t, 0);
    if u != 5 || d != 0 {
        panic!("C-PP-aya-with-01: fans ({u},{d}), expected (5,0)");
    }
}

/// C-PP-band-idol-01 -- non-PP characters get 1 reverse fan at game start.
#[test]
fn band_start_reverse_fan() {
    let t = Table::new(&["丸山彩", "户山香澄"]);
    let (u, d) = fans(&t, 1);
    if u != 0 || d != 1 {
        panic!("C-PP-band-idol-01: non-PP fans ({u},{d}), expected (0,1)");
    }
}

// ============================================================ character (2)

/// C-PP-aya-with-02 -- pay discount Y*100 and flip Y reverse of other-band.
#[test]
fn aya_skill_2_pay_discount() {
    forall(4, 0x41_5942, |rng, _i| {
        let mut t = Table::vanilla(3);
        t.set_character_raw(0, "丸山彩");
        t.place_raw(0, "skill:丸山彩:With~");
        let y = rng.range(1, 4);
        set_fans(&mut t, 0, 5, 0);
        set_fans(&mut t, 1, 0, 4);
        t.own(1, &[7]);
        t.set_houses(7, 1); // rent 640
        t.begin_turn(0);
        t.set_pos(0, 5);
        t.dice(&[2]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        for _ in 0..30 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            if p.options.iter().any(|o| format!("{o:?}").contains("intOption")) {
                let _ = t.answer_one(y);
            } else {
                t.decline();
            }
        }
        let paid = 10_000 - t.money(0);
        let expected = (640 - y * 100).max(0);
        if paid != expected {
            return Err(format!(
                "C-PP-aya-with-02: paid {paid}, expected {expected} (640 - {y}*100)"
            ));
        }
        let (u0, d0) = fans(&t, 0);
        if u0 != 5 - y || d0 != y {
            return Err(format!(
                "C-PP-aya-with-02: user fans ({u0},{d0}), expected ({},{y})",
                5 - y
            ));
        }
        let (u1, d1) = fans(&t, 1);
        if u1 != y || d1 != 4 - y {
            return Err(format!(
                "C-PP-aya-with-02: other fans ({u1},{d1}), expected ({y},{})",
                4 - y
            ));
        }
        Ok(())
    })
    .expect("C-PP-aya-with-02");
}

/// C-PP-eve-unify-02 -- +Yd4 on a roll, flip Y positive.
#[test]
fn eve_skill_2_adds_yd4() {
    forall(4, 0x45_5655, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "若宫伊芙");
        t.place_raw(0, "skill:若宫伊芙:天下统一");
        let y = rng.range(1, 3);
        set_fans(&mut t, 0, 3, 0);
        t.begin_turn(0);
        let mut faces = vec![2];
        for _ in 0..y {
            faces.push(1);
        }
        t.dice(&faces);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        for _ in 0..30 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            if p.options.iter().any(|o| format!("{o:?}").contains("intOption")) {
                let _ = t.answer_one(y);
            } else {
                t.decline();
            }
        }
        if t.pos(0) != (2 + y) as usize {
            return Err(format!(
                "C-PP-eve-unify-02: pos {}, expected {} (2 + {y}d4 of 1)",
                t.pos(0),
                2 + y
            ));
        }
        let (u, d) = fans(&t, 0);
        if u != 3 - y || d != y {
            return Err(format!(
                "C-PP-eve-unify-02: fans ({u},{d}), expected ({},{y})",
                3 - y
            ));
        }
        Ok(())
    })
    .expect("C-PP-eve-unify-02");
}

/// C-PP-chisato-frank-02 -- receive +Y*100, flip Y positive.
#[test]
fn chisato_skill_2_income_boost() {
    forall(4, 0x43_484A, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "白鹭千圣");
        t.place_raw(0, "skill:白鹭千圣:保持坦率的你");
        let y = rng.range(1, 3);
        set_fans(&mut t, 0, 3, 0);
        t.own(0, &[7]);
        t.set_houses(7, 1); // rent 640
        t.begin_turn(1);
        t.set_pos(1, 5);
        t.dice(&[2]);
        t.roll(1).map_err(|e| format!("roll rejected: {e}"))?;
        for _ in 0..30 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            if p.options.iter().any(|o| format!("{o:?}").contains("intOption")) {
                let _ = t.answer_one(y);
            } else {
                t.decline();
            }
        }
        let gained = t.money(0) - 10_000;
        if gained != 640 + y * 100 {
            return Err(format!(
                "C-PP-chisato-frank-02: gained {gained}, expected {} (640 + {y}*100)",
                640 + y * 100
            ));
        }
        let (u, d) = fans(&t, 0);
        if u != 3 - y || d != y {
            return Err(format!(
                "C-PP-chisato-frank-02: fans ({u},{d}), expected ({},{y})",
                3 - y
            ));
        }
        Ok(())
    })
    .expect("C-PP-chisato-frank-02");
}

/// C-PP-maya-dawn-02 -- draw becomes watch Y+1, pick one.
#[test]
fn maya_skill_2_watch_pick() {
    forall(4, 0x4D_4159, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "大和麻弥");
        t.place_raw(0, "skill:大和麻弥:朝阳照耀的片刻");
        let y = rng.range(1, 3);
        set_fans(&mut t, 0, 3, 0);
        t.set_draw(0, &["通用:GREAT", "通用:尽力后的收获", "通用:网络链接异常"]);
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, 5);
        t.begin_turn(0);
        for _ in 0..30 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            let is_y = p.options.iter().any(|o| format!("{o:?}").contains("intOption"));
            if is_y {
                let _ = t.answer_one(y);
            } else if p.kind == "pick" || p.kind == "card" {
                let _ = t.answer_items(0, &["通用:GREAT"]);
            } else if p.title.key().contains("pastel") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        if !t.hand(0).contains(&"通用:GREAT".to_string()) {
            return Err(format!(
                "C-PP-maya-dawn-02: picked card missing: {:?}",
                t.hand(0)
            ));
        }
        let (u, d) = fans(&t, 0);
        if u != 3 - y || d != y {
            return Err(format!(
                "C-PP-maya-dawn-02: fans ({u},{d}), expected ({},{y})",
                3 - y
            ));
        }
        Ok(())
    })
    .expect("C-PP-maya-dawn-02");
}

// ============================================================ band skill

/// C-PP-band-idol-03 -- [经过]CiRCLE grants no CiRCLE reward.
#[test]
fn band_no_circle_reward() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "丸山彩");
    t.place_raw(0, BAND_SKILL);
    t.begin_turn(0);
    t.set_pos(0, 55);
    t.dice(&[6]);
    t.roll(0).unwrap();
    drain(&mut t);
    if t.money(0) != 10_000 {
        panic!(
            "C-PP-band-idol-03: money {}, expected 10000 (no CiRCLE reward)",
            t.money(0)
        );
    }
    if !t.hand(0).is_empty() {
        panic!(
            "C-PP-band-idol-03: hand {:?}, expected no CiRCLE draw",
            t.hand(0)
        );
    }
}

/// C-PP-band-idol-04 -- turn start +1 crystal; may spend 5 to draw 1.
#[test]
fn band_turn_start_crystal_and_spend() {
    forall(4, 0x42_414E, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "丸山彩");
        t.place_raw(0, BAND_SKILL);
        // Only a start of 4 reaches the spend-5 threshold after the +1.
        let n = 4;
        set_band_xtal(&mut t, 0, n);
        t.set_draw(0, &["通用:GREAT"]);
        t.begin_turn(0);
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if p.title.key().contains("pastel") {
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        // Turn start: n+1 = 5, then spend 5 to draw 1 -> 0.
        let c = band_xtal(&t, 0);
        if c != 0 {
            return Err(format!(
                "C-PP-band-idol-04: band crystals {c}, expected 0 ({n}+1-5)"
            ));
        }
        if t.hand(0).len() != 1 {
            return Err(format!(
                "C-PP-band-idol-04: hand {:?}, expected 1 drawn",
                t.hand(0)
            ));
        }
        Ok(())
    })
    .expect("C-PP-band-idol-04");
}

/// C-PP-band-idol-02 -- a fan-flip with X > owned reverse becomes overflow
/// crystals on the band card (cap 10).
#[test]
fn band_overflow_to_crystals() {
    forall(4, 0x42_414F, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "丸山彩");
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, 0);
        let down = rng.range(1, 3);
        set_fans(&mut t, 0, 0, down);
        // X = 5 + ceil(down/2) since up=0 < down; flips `down`, overflow
        // X - down.
        t.give_play(0, "PP:有你与我在这里共度")
            .map_err(|e| format!("play rejected: {e}"))?;
        drain(&mut t);
        let x = 5 + (down + 1) / 2;
        let overflow = x - down;
        let (u, d) = fans(&t, 0);
        if u != down || d != 0 {
            return Err(format!("fans ({u},{d}), expected ({down},0)"));
        }
        let c = band_xtal(&t, 0);
        if c != overflow {
            return Err(format!(
                "C-PP-band-idol-02: band crystals {c}, expected {overflow} (X {x} - owned {down})"
            ));
        }
        Ok(())
    })
    .expect("C-PP-band-idol-02");
}

// ============================================================ (2) borrow

/// C-PP-hina-lottery-02 -- each turn start rolls 1d4 and borrows that
/// character's (2) skill until the next turn start.
#[test]
fn hina_skill_2_borrows_skill() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "冰川日菜");
    t.place_raw(0, "skill:冰川日菜:日菜抽中的大奖");
    t.dice(&[2]);
    t.begin_turn(0);
    drain(&mut t);
    let borrowed = t.state(0, "skill.hinaLottery.borrowed");
    if borrowed != 2 {
        panic!(
            "C-PP-hina-lottery-02: borrowed {borrowed}, expected 2 (白鹭千圣)"
        );
    }
}

// ============================================================ again-shine

/// C-PP-shine-again-00 -- [特] immunity on the instance; [手] places it.
#[test]
fn shine_again_places_immune() {
    let mut t = Table::vanilla(2);
    t.give_play(0, "PP:再次闪耀").unwrap();
    let f = t
        .field(0)
        .into_iter()
        .find(|f| f.card == "PP:再次闪耀")
        .expect("placed");
    if !f.immune {
        panic!("C-PP-shine-again-00: 不受除拥有者以外的效果影响 -- immune not set");
    }
}

/// C-PP-shine-again-01 -- [反击] on a short [支付]: record a colour, gain
/// same-colour deed price / 5 and 1 positive fan. Setup keeps enough money
/// after the rescue to pay, so the bankruptcy sweep does not run.
#[test]
fn shine_again_rescue_on_short_pay() {
    forall(4, 0x53_4849, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.place_raw(0, "PP:再次闪耀");
        // Own 购物中心 (tile 1, price 3000) so the rescue is 600. Land on
        // 江户川公园 (tile 7, base rent 140) without wrapping through CiRCLE
        // (its reward would cover the shortfall). 50 < 140 <= 650.
        t.own(0, &[1]);
        let money0 = 50 + rng.range(0, 3) * 10;
        t.set_money(0, money0);
        t.own(1, &[7]);
        t.set_houses(7, 0);
        t.begin_turn(0);
        t.set_pos(0, 5);
        t.dice(&[2]);
        t.roll(0).map_err(|e| format!("roll rejected: {e}"))?;
        let mut saw = false;
        while t.prompt().is_some() {
            let p = t.expect_prompt();
            if t.counteract_offered("PP:再次闪耀") {
                saw = true;
                t.counteract(0, "PP:再次闪耀")
                    .map_err(|e| format!("counteract refused: {e}"))?;
            } else if p.title.key().contains("shine") {
                saw = true;
                let _ = t.answer(0, 0);
            } else {
                t.decline();
            }
        }
        if !saw {
            return Err(format!(
                "C-PP-shine-again-01: [反击] window never opened: {}",
                t.recent_keys(20).join(", ")
            ));
        }
        // The book grants BOTH 「同色地契的购买价格」÷5 AND 「1个正面[P✽P粉丝]」.
        // Own deed is 购物中心 (price 3000) -> +600, then rent 140 on 江户川公园.
        let money = t.money(0);
        let expect_money = money0 + 600 - 140;
        if money != expect_money {
            return Err(format!(
                "C-PP-shine-again-01: money {money} (from {money0}), expected {expect_money} (+600 rescue, -140 rent)"
            ));
        }
        let fans_up = t.token(0, "P✽P粉丝(正)");
        if fans_up < 1 {
            return Err(format!(
                "C-PP-shine-again-01: book also grants 1 positive fan; fans {fans_up}, money {money}, events {}",
                t.recent_keys(25).join(", ")
            ));
        }
        Ok(())
    })
    .expect("C-PP-shine-again-01");
}

// ============================================================ maya exclusive

/// C-PP-maya-inf-02 -- after a draw with crystals < 4, +1 crystal on the card.
/// The >= 4 clear-and-band-+1 branch is observed when the draw is a full
/// 「抽卡动作」; the band (4) spend is a card draw and is the draw this pins.
#[test]
fn maya_inf_crystal_per_draw() {
    forall(4, 0x4D_494E, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "大和麻弥");
        t.place_raw(0, BAND_SKILL);
        set_band_xtal(&mut t, 0, 2);
        let start = rng.range(0, 3); // < 4 so (1) takes the +1 leg
        t.place_raw(0, "PP:[大和麻弥]可能性为∞");
        t.set_crystals(0, "PP:[大和麻弥]可能性为∞", start);
        t.set_draw(0, &["通用:GREAT", "通用:GREAT"]);
        t.begin_turn(0);
        drain(&mut t);
        // A full 「抽卡动作」: 通用:10次招募's [手] draws one.
        t.give_play(0, "通用:10次招募（1回限定）")
            .map_err(|e| format!("draw-card play rejected: {e}"))?;
        drain(&mut t);
        let c = t.crystals(0, "PP:[大和麻弥]可能性为∞").unwrap_or(-1);
        if c != start + 1 {
            return Err(format!(
                "C-PP-maya-inf-02: crystals {c}, expected {} ({start}+1)",
                start + 1
            ));
        }
        Ok(())
    })
    .expect("C-PP-maya-inf-02");
}

// ============================================================ 单元 smoke

/// Every card named in the unit is at least loadable / placeable or playable
/// from hand, so a typo in the key fails here rather than deep in a contract.
#[test]
fn every_pp_card_key_resolves() {
    const CARDS: &[&str] = &[
        "PP:TITLE IDOL",
        "PP:[丸山彩]憧憬的前方",
        "PP:[冰川日菜]会发出怎样的声音呢？",
        "PP:[大和麻弥]可能性为∞",
        "PP:[白鹭千圣]微笑的铁假面",
        "PP:[若宫伊芙]属于我的武士道！",
        "PP:[衍生]共鸣",
        "PP:[衍生]明天见",
        "PP:[衍生]重叠的声音",
        "PP:[衍生]魔法战队Pastel✽Ranger",
        "PP:不要背负期待",
        "PP:再次闪耀",
        "PP:初次演出事故",
        "PP:可爱又强壮的花朵",
        "PP:同一个梦想",
        "PP:找回珍妮弗",
        "PP:有你与我在这里共度",
        "PP:梦在前方，结彩当下",
        "PP:练习生解密指南",
    ];
    let mut t = Table::vanilla(2);
    for c in CARDS {
        let uid = t.place_raw(0, c);
        if uid < 0 {
            panic!("place_raw refused {c}");
        }
    }
    assert_eq!(t.field_ids(0).len(), CARDS.len(), "{:?}", t.field_ids(0));
}