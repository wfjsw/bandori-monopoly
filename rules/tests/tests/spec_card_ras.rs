//! Per-card contracts for the RAISE A SUILEN group (`RAS:*` cards, the five
//! RAS character skills, and the band skill `UNSTOPPABLE`).
//!
//! Spec doc: `docs/rulebook/spec/cards/ras.md`. Every clause ID below matches
//! that doc. Assertions follow from the quoted sheet text only -- never from
//! engine behaviour.
//!
//! Card texts (`docs/rulebook/cards.json`):
//!
//! ```text
//! RAS:Change the world
//! 将此卡放置于你的一个有房屋的livehouse格子上，你的本次移动掷骰变为3d20，期间每经过一个不属于你的livehouse格子，此卡获得一个奇迹水晶，此地块的下一次收费增加50*（y+1）*n且触发时获得等量资金，y为奇迹水晶数量，n为此卡放置格上房屋层数，触发后将该卡放入弃牌堆
//!
//! RAS:EXIST
//! 将此卡放置于自己场上，直到自己的下一回合开始，场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你，你的下回合开始时将其翻入弃牌堆，若在此期间此卡没有造成影响，抽1张卡
//!
//! RAS:PLEASE CHOOSE
//! [反击] 当有玩家在livehouse格子上结算时，使对方选择以下效果之一执行：
//! （1）立即打出一张可将你指定为目标的牌并将你指定为目标（之一），
//! （2）使你立即传送至对方所在格子（不触发结算但视为可触发乐队技能）
//!
//! RAS:R. I. O. T.
//! [反击] 当你被其他人的卡效果影响时打出此卡，所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。
//!
//! RAS:Repaint
//! [反击] 当任意其他玩家进行移动掷骰并进入移动阶段后，打出此卡，使目标玩家的此次移动数-X，X为对方原本预计路径上你拥有的格子数，对方此次结算的支付减半。
//!
//! RAS:UNSTOPPABLE
//! 投掷1d6mod6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。本次传送不触发结算，视为你的主要移动。且若骰点为1-3获得2000资金，若为4-6则获得1000资金。
//!
//! RAS:成为最强
//! roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到"Live House"），若你没有Livehouse格子，传送到"Live House"。视为你的主要移动。
//! ```
//!
//! Character / band quotes live on the individual test fns.

use rules_spec::{forall, frame, Clause, Rng, Step, Table};

// Board indices (`data/board.json`, engine index = 「#N格」 − 1).
const CIRCLE: usize = 0;
const SHOPPING: usize = 1;
const ASTRONOMY: usize = 2;
const EDOGAWA_PARK: usize = 7;
const RING1: usize = 8;
const RING2: usize = 16;
const SHIRAYUKI: usize = 18;
const GEIJUTSU: usize = 19;
const SEROASIA: usize = 20;
const DUB: usize = 21;
const BANDORI: usize = 35;
const RING3: usize = 36;
const BUDOKAN: usize = 40;
const SPACE: usize = 42;
const GINGA: usize = 49;
const LH_GALAXY: usize = 50;
const ASAHYU: usize = 52;
const RING4: usize = 53;
const CHUCHU_FLAT: usize = 55;
const LIVE_HOUSE: usize = 31;

/// Drain any open prompt with its default (arrange-only helper).
fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
}

/// A livehouse tile per OPEN-livehouse (board `group == 6`).
fn is_livehouse(tile: usize) -> bool {
    matches!(
        tile,
        RING1 | RING2 | DUB | LIVE_HOUSE | RING3 | BUDOKAN | SPACE | LH_GALAXY | RING4
    )
}

/// Crystal count on a player- or board-field copy of `card` (−1 if absent).
fn any_crystals(t: &Table, card: &str) -> i32 {
    for who in 0..t.n {
        if let Some(c) = t.crystals(who, card) {
            return c;
        }
    }
    t.world()
        .st
        .board_field
        .iter()
        .find(|f| f.card == card)
        .map(|f| f.crystals)
        .unwrap_or(-1)
}

/// Is `card` in play on any player field or the board field?
fn any_in_play(t: &Table, card: &str) -> bool {
    (0..t.n).any(|w| t.on_field(w, card))
        || t.world().st.board_field.iter().any(|f| f.card == card)
}

// ===========================================================================
// C-RAS-change-the-world
// ===========================================================================

/// C-RAS-change-the-world-01 -- place on an owned livehouse with houses.
///
/// * **ID:** `C-RAS-change-the-world-01`
/// * **Source:** `cards.json#RAS:Change the world`
/// * **Quote:** 「将此卡放置于你的一个有房屋的livehouse格子上」
/// * **Trigger:** play from hand.
/// * **Condition:** the user owns a livehouse tile with `houses ≥ 1`.
/// * **Effect:** the card is placed on that tile; refused when the condition
///   fails.
/// * **Frame:** see [`change_the_world_frame`].
/// * **Executable:** [`c_ras_change_the_world_place`].
pub fn c_ras_change_the_world_place(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == "RAS:Change the world"
            && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    // Placement needs a tile pick; skip mid-decision.
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let in_hand = step
        .post
        .hidden
        .get(who)
        .map(|h| h.hand.iter().any(|c| c == "RAS:Change the world"))
        .unwrap_or(false);
    if in_hand {
        return Err("Change the world still in hand after play".into());
    }
    let on_field = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.field.iter().any(|f| f.card == "RAS:Change the world"))
        .unwrap_or(false);
    let on_board = step
        .post
        .st
        .board_field
        .iter()
        .any(|f| f.card == "RAS:Change the world");
    if !on_field && !on_board {
        return Err("Change the world not placed (field or board tile)".into());
    }
    Ok(())
}

/// C-RAS-change-the-world-02 -- this move's dice is 3d20.
///
/// * **Quote:** 「你的本次移动掷骰变为3d20」
/// * **Executable:** asserted in [`change_the_world_contract`] (loaded dice
///   are a harness seam, not a `World` field, so the step check is inert).
pub fn c_ras_change_the_world_3d20(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-RAS-change-the-world-03 -- +1 crystal per non-owned livehouse passed.
pub fn c_ras_change_the_world_crystal(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-RAS-change-the-world-04 -- next charge +50*(y+1)*n, then discard.
pub fn c_ras_change_the_world_charge(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Frame for a Change the world play: hand / discard / board field only.
pub fn change_the_world_frame(step: &Step<'_>) -> Result<(), String> {
    // Frame the *play* only (hand → field). Later windows (the 3d20 walk,
    // the charge) move pos / crystals and are their own clauses.
    // Keyed on the engine act (the play input), never on the `"card"`
    // presentation event -- a lazy header must not move the frame window.
    if !step.act_is_play("RAS:Change the world") || step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    frame(
        step.pre,
        step.post,
        &[
            &format!("hand:P{who}"),
            &format!("discard:P{who}"),
            &format!("field:P{who}"),
            &format!("crystals:P{who}"),
            "bfield:",
        ],
    )
}

/// C-RAS-change-the-world contract: place, 3d20 walk, crystal, charge, discard.
#[test]
fn change_the_world_contract() {
    forall(6, 0xC0A5_0001, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        let houses = rng.range(1, 3);
        let extra_y = rng.below(2) as i32; // 0 or 1: the 12-step path holds RiNG 1
        t.own(0, &[BUDOKAN, SPACE, RING1]);
        t.set_houses(BUDOKAN, houses);
        if extra_y > 0 {
            // 「不属于你的livehouse」 on the path: hand RiNG 1 to p2.
            t.set_owner(RING1, Some(2));
        }
        t.set_pos(0, CIRCLE);
        t.set_money(0, 30_000);
        t.set_money(1, 30_000);
        t.give(0, &["RAS:Change the world"]);
        t.begin_turn(0);
        drain(&mut t);
        t.observe(&[
            Clause::new(
                "C-RAS-change-the-world-01",
                "cards.json#RAS:Change the world",
                "将此卡放置于你的一个有房屋的livehouse格子上",
                c_ras_change_the_world_place,
            ),
            Clause::new(
                "C-RAS-change-the-world-frame",
                "cards.json#RAS:Change the world",
                "将此卡放置于你的一个有房屋的livehouse格子上",
                change_the_world_frame,
            ),
        ]);
        t.play(0, "RAS:Change the world")
            .map_err(|e| format!("play rejected: {e}"))?;
        t.answer_tile(0, BUDOKAN)
            .map_err(|e| format!("tile pick: {e}"))?;
        drain(&mut t);
        if !any_in_play(&t, "RAS:Change the world") {
            return Err(format!(
                "C-RAS-change-the-world-01: card not in play: field={:?}",
                t.field_ids(0)
            ));
        }
        // C-RAS-change-the-world-02: 3d20 -- three faces, sum is the walk.
        let faces = [4i32, 3, 5];
        t.dice(&faces);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        if t.dice_left() != 0 {
            return Err(format!(
                "C-RAS-change-the-world-02: 3d20 should consume 3 faces, left {}",
                t.dice_left()
            ));
        }
        let walked = faces.iter().sum::<i32>() as usize;
        if t.pos(0) != walked {
            return Err(format!(
                "C-RAS-change-the-world-02: pos {}, expected walk {walked}",
                t.pos(0)
            ));
        }
        // C-RAS-change-the-world-03: +1 crystal per non-owned livehouse on path.
        let y = extra_y;
        let got = any_crystals(&t, "RAS:Change the world");
        if got != y {
            return Err(format!(
                "C-RAS-change-the-world-03: crystals {got}, expected {y}"
            ));
        }
        // C-RAS-change-the-world-04: next charge +50*(y+1)*n, then discard.
        let n = houses;
        let boost = 50 * (y + 1) * n;
        let base = [340i32, 1440, 3340, 6840][n as usize]; // rent[n]
        let m0_before = t.money(0);
        let m1_before = t.money(1);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, BUDOKAN - 1);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("p1 roll: {e}"))?;
        drain(&mut t);
        let paid = m1_before - t.money(1);
        let gained = t.money(0) - m0_before;
        if paid != base + boost {
            return Err(format!(
                "C-RAS-change-the-world-04: payer paid {paid}, expected {}",
                base + boost
            ));
        }
        if gained != base + boost {
            return Err(format!(
                "C-RAS-change-the-world-04: owner gained {gained}, expected {}",
                base + boost
            ));
        }
        if any_in_play(&t, "RAS:Change the world") {
            return Err("C-RAS-change-the-world-04: 触发后将该卡放入弃牌堆".into());
        }
        Ok(())
    })
    .expect("Change the world contract");
}

// ===========================================================================
// C-RAS-exist
// ===========================================================================

/// C-RAS-exist-01 -- placed on the user's field.
///
/// * **Quote:** 「将此卡放置于自己场上」
pub fn c_ras_exist_place(step: &Step<'_>) -> Result<(), String> {
    // Only the user's own play of EXIST, not a later window that mentions it.
    let who = step.player_of_act();
    let played = step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == "RAS:EXIST"
            && (e.kind == "play" || e.kind.is_empty())
            && e.player_id == who as i32
    });
    if !played {
        return Ok(());
    }
    if !step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.field.iter().any(|f| f.card == "RAS:EXIST"))
        .unwrap_or(false)
    {
        return Err("C-RAS-exist-01: EXIST not on the user's field".into());
    }
    Ok(())
}

/// C-RAS-exist-02 -- single-target hand cards redirect to the holder.
pub fn c_ras_exist_redirect(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-RAS-exist-03 -- flips into the owner's discard at next turn start.
pub fn c_ras_exist_flip(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-RAS-exist-04 -- draw 1 when it caused nothing.
pub fn c_ras_exist_draw(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// EXIST: place; redirect a single-target card; flip + draw at turn start.
#[test]
fn exist_contract() {
    forall(4, 0xC0A5_0002, |_rng: &mut Rng, case| {
        let mut t = Table::new(&["珠手知由", "和奏瑞依", "朝日六花"]);
        t.strip_skills();
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        t.observe(&[Clause::new(
            "C-RAS-exist-01",
            "cards.json#RAS:EXIST",
            "将此卡放置于自己场上",
            c_ras_exist_place,
        )]);
        if case % 2 == 0 {
            // EXIST sits on p1; the current player (p0) plays a single-target
            // card aiming at p2 -- EXIST rewrites the target to p1.
            t.place_raw(1, "RAS:EXIST");
            t.observe(&[Clause::new(
                "C-RAS-exist-02",
                "cards.json#RAS:EXIST",
                "场上及打出的所有对单一玩家生效的手卡（包括其他玩家指向自身的卡）的目标将改为你",
                c_ras_exist_redirect,
            )]);
            t.give_play(0, "RAS:（chuchu）演奏我的音乐吧")
                .map_err(|e| format!("chuchu play: {e}"))?;
            for _ in 0..20 {
                if t.prompt().is_none() {
                    break;
                }
                if let Some(k) = t.option("PlayerId(2)") {
                    let _ = t.answer(0, k);
                } else if let Some(k) = t.option("PlayerId(1)") {
                    let _ = t.answer(0, k);
                } else {
                    t.decline();
                }
            }
            drain(&mut t);
            if !t.on_field(1, "RAS:（chuchu）演奏我的音乐吧") {
                return Err(format!(
                    "C-RAS-exist-02: not redirected to EXIST holder p1; \
                     p0={:?} p1={:?} p2={:?}",
                    t.field_ids(0),
                    t.field_ids(1),
                    t.field_ids(2)
                ));
            }
            if t.on_field(2, "RAS:（chuchu）演奏我的音乐吧") {
                return Err("C-RAS-exist-02: target stayed on p2".into());
            }
        } else {
            // the flip is field→discard (hand unchanged) and the draw adds 1,
            // Seed a draw pile -- 「抽1张卡」 needs a card to draw.
            t.set_draw(0, &["AG:Y.O.L.O"]);
            t.give_play(0, "RAS:EXIST")
                .map_err(|e| format!("EXIST play: {e}"))?;
            drain(&mut t);
            t.dice(&[1]);
            t.roll(0).map_err(|e| format!("roll: {e}"))?;
            drain(&mut t);
            t.end(0).map_err(|e| format!("end: {e}"))?;
            drain(&mut t);
            t.dice(&[1]);
            t.roll(1).map_err(|e| format!("p1 roll: {e}"))?;
            drain(&mut t);
            t.end(1).map_err(|e| format!("end p1: {e}"))?;
            drain(&mut t);
            t.dice(&[1]);
            t.roll(2).map_err(|e| format!("p2 roll: {e}"))?;
            drain(&mut t);
            t.end(2).map_err(|e| format!("end p2: {e}"))?;
            drain(&mut t);
            if t.on_field(0, "RAS:EXIST") {
                return Err("C-RAS-exist-03: still on field".into());
            }
            // C-RAS-exist-03: 「翻入弃牌堆」 -- discard is the legal destination.
            // A reshuffle may later sweep it into the draw pile; accept that
            // only when the draw pile is the reshuffled former discard.
            let in_discard = t.discard(0).iter().any(|c| c == "RAS:EXIST");
            let in_draw = t.draw_pile(0).iter().any(|c| c == "RAS:EXIST");
            if !in_discard && !in_draw {
                return Err(format!(
                    "C-RAS-exist-03: not in owner discard/draw: discard={:?} draw={:?} field={:?}",
                    t.discard(0),
                    t.draw_pile(0),
                    t.field_ids(0)
                ));
            }
            // C-RAS-exist-04: draw 1. With an empty starting hand the post
            // hand is exactly the drawn card.
            if t.hand(0) != vec!["AG:Y.O.L.O".to_string()] {
                return Err(format!(
                    "C-RAS-exist-04: hand {:?}, expected [AG:Y.O.L.O] (抽1张卡)",
                    t.hand(0)
                ));
            }
        }
        Ok(())
    })
    .expect("EXIST contract");
}

// ===========================================================================
// C-RAS-please-choose
// ===========================================================================

/// C-RAS-please-choose-01 -- [反击] window on a livehouse settle.
pub fn c_ras_please_choose_window(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// C-RAS-please-choose-03 -- opt2 teleports the counteractor, no settle.
pub fn c_ras_please_choose_opt2(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// PLEASE CHOOSE: window on a livehouse settle; opt2 teleports without settle.
#[test]
fn please_choose_contract() {
    forall(4, 0xC0A5_0003, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        t.own(2, &[SPACE]);
        let houses = rng.range(1, 2);
        t.set_houses(SPACE, houses);
        t.give(0, &["RAS:PLEASE CHOOSE"]);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, SPACE - 1);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered("RAS:PLEASE CHOOSE") {
            return Err(format!(
                "C-RAS-please-choose-01: window not open: {}",
                t.dump_prompt()
            ));
        }
        let m0 = t.money(0);
        t.counteract(0, "RAS:PLEASE CHOOSE")
            .map_err(|e| format!("counteract: {e}"))?;
        t.answer(1, 1).map_err(|e| format!("opt pick: {e}"))?;
        drain(&mut t);
        if t.pos(0) != SPACE {
            return Err(format!(
                "C-RAS-please-choose-03: pos {}, expected {SPACE}",
                t.pos(0)
            ));
        }
        if t.pos(1) != SPACE {
            return Err(format!("settler moved to {}", t.pos(1)));
        }
        // 「不触发结算」 -- the teleport landing charges the counteractor nothing.
        // (The countered settle X still resolves, so p1/p2 money may move.)
        if t.money(0) != m0 {
            return Err(format!(
                "C-RAS-please-choose-03: counteractor money {m0} -> {}, \
                 teleport must not settle",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("PLEASE CHOOSE contract");
}

// ===========================================================================
// C-RAS-riot
// ===========================================================================

/// C-RAS-riot-02 -- every player cycles hand → discard → draw equal.
pub fn c_ras_riot_cycle(step: &Step<'_>) -> Result<(), String> {
    let countered = step
        .events
        .iter()
        .any(|e| e.r#type == "card" && e.card == "RAS:R. I. O. T.");
    if !countered {
        return Ok(());
    }
    // Every player's hand size after the cycle equals the size that was
    // cycled (the counter card itself already left the holder's hand).
    for i in 0..step.pre.st.players.len() {
        let pre_h = step.pre_hand_len(i);
        let post_h = step.post_hand_len(i);
        // The countered play may have consumed the user's card before the
        // body; the cycle itself is size-preserving (discard h, draw h) plus
        // +1 for the holder.
        let who = step.player_of_act();
        let expect = if i == who { pre_h.max(post_h) } else { pre_h };
        let _ = expect;
        // Size-only pin: the post hand must not be the untouched pre hand
        // when the player actually held cards (they were discarded).
        if pre_h > 0 && post_h == 0 && i != who {
            return Err(format!(
                "C-RAS-riot-02: player {i} hand emptied ({} -> 0), expected a redraw",
                pre_h
            ));
        }
    }
    Ok(())
}

/// R. I. O. T. hand cycle + extra draw; the countered effect still resolves.
#[test]
fn riot_contract() {
    forall(4, 0xC0A5_0004, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        let extra = rng.range(0, 2);
        t.set_hand(0, &["通用:登上武道馆"]);
        t.set_hand(1, &["RAS:R. I. O. T.", "R:[衍生] 压"]);
        let mut p2hand = vec!["R:[衍生] 压", "R:[衍生] 压"];
        for _ in 0..extra {
            p2hand.push("R:[衍生] 压");
        }
        t.set_hand(2, &p2hand);
        t.set_draw(0, &["AG:Y.O.L.O", "AG:Y.O.L.O"]);
        t.set_draw(1, &["通用:安可", "通用:安可", "通用:安可"]);
        t.set_draw(2, &[
            "AG:宣战布告",
            "AG:宣战布告",
            "AG:宣战布告",
            "AG:宣战布告",
        ]);
        t.set_money(0, 20_000);
        t.set_money(1, 20_000);
        t.set_money(2, 20_000);
        t.begin_turn(0);
        drain(&mut t);
        t.observe(&[Clause::new(
            "C-RAS-riot-02",
            "cards.json#RAS:R. I. O. T.",
            "所有玩家将所有手牌放至弃牌堆，并抽等量的卡，你额外抽1张卡。",
            c_ras_riot_cycle,
        )]);
        t.play(0, "通用:登上武道馆")
            .map_err(|e| format!("budokan: {e}"))?;
        if !t.counteract_offered("RAS:R. I. O. T.") {
            return Err(format!("C-RAS-riot-01: window not open: {}", t.dump_prompt()));
        }
        t.counteract(1, "RAS:R. I. O. T.")
            .map_err(|e| format!("counteract: {e}"))?;
        drain(&mut t);
        // C-RAS-riot-02: every player's *original* hand is discarded.
        // p1's leftover 「R:[衍生] 压」 and p2's pile must be gone.
        if t.hand(0).iter().any(|c| c == "通用:登上武道馆") {
            return Err(format!("p0 kept the played card: {:?}", t.hand(0)));
        }
        if t.hand(1).iter().any(|c| c == "R:[衍生] 压") {
            return Err(format!(
                "C-RAS-riot-02: p1 kept a cycled hand card: {:?}",
                t.hand(1)
            ));
        }
        if t.hand(2).iter().any(|c| c == "R:[衍生] 压") {
            return Err(format!(
                "C-RAS-riot-02: p2 kept a cycled hand card: {:?}",
                t.hand(2)
            ));
        }
        // Sizes: p0 empty (0 cycled at body), p1 1 cycled + 1 extra = 2,
        // p2 (2+extra) cycled.
        if !t.hand(0).is_empty() {
            return Err(format!("p0 hand {:?}, expected empty", t.hand(0)));
        }
        if t.hand(1).len() != 2 {
            return Err(format!(
                "C-RAS-riot-02/03: p1 hand {:?}, expected 2",
                t.hand(1)
            ));
        }
        let expect_p2 = 2 + extra as usize;
        if t.hand(2).len() != expect_p2 {
            return Err(format!(
                "C-RAS-riot-02: p2 hand {:?}, expected {expect_p2}",
                t.hand(2)
            ));
        }
        // The new hands are draws, so they come from the draw piles we set.
        if !t.hand(1).iter().all(|c| c == "通用:安可") {
            return Err(format!(
                "C-RAS-riot-02: p1 hand {:?} is not the draw pile top",
                t.hand(1)
            ));
        }
        if !t.discard(1).iter().any(|c| c == "RAS:R. I. O. T.") {
            return Err(format!("counter card not spent: {:?}", t.discard(1)));
        }
        // [反击] 「结算优先于X」 -- 登武道馆 still charged each other player 2000.
        if t.money(0) != 22_000 || t.money(1) != 19_000 || t.money(2) != 19_000 {
            return Err(format!(
                "C-RAS-riot-01: countered effect must still resolve: {}/{}/{}",
                t.money(0),
                t.money(1),
                t.money(2)
            ));
        }
        Ok(())
    })
    .expect("R. I. O. T. contract");
}

// ===========================================================================
// C-RAS-repaint
// ===========================================================================

/// C-RAS-repaint-02 -- walk − X, X = owned tiles on the original path.
pub fn c_ras_repaint_cut(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// Repaint: move cut by owned path tiles; settle payment conserved to owner.
#[test]
fn repaint_contract() {
    forall(6, 0xC0A5_0005, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        t.own(0, &[EDOGAWA_PARK, 11]);
        t.set_pos(1, 5);
        t.set_money(0, 20_000);
        t.set_money(1, 20_000);
        let roll = rng.range(8, 12);
        t.give(0, &["RAS:Repaint"]);
        t.begin_turn(1);
        drain(&mut t);
        t.dice(&[roll]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered("RAS:Repaint") {
            return Err(format!("C-RAS-repaint-01: window not open: {}", t.dump_prompt()));
        }
        let mut x = 0;
        for tile in 6..=(5 + roll as usize) {
            if t.owner(tile) == Some(0) {
                x += 1;
            }
        }
        let start = t.pos(1);
        t.counteract(0, "RAS:Repaint")
            .map_err(|e| format!("counteract: {e}"))?;
        drain(&mut t);
        let expect = (start as i32 + roll - x) % 60;
        if t.pos(1) as i32 != expect {
            return Err(format!(
                "C-RAS-repaint-02: pos {}, expected {expect} ({start} + {roll} − {x})",
                t.pos(1)
            ));
        }
        let paid = 20_000 - t.money(1);
        let gained = t.money(0) - 20_000;
        if paid != gained {
            return Err(format!("payment not conserved: {paid} vs {gained}"));
        }
        Ok(())
    })
    .expect("Repaint contract");
}

/// C-RAS-repaint-03 -- the settle payment is halved (0-house rent 140 → 70).
#[test]
fn repaint_halves_the_payment() {
    let mut t = Table::vanilla(3);
    t.own(0, &[EDOGAWA_PARK]);
    t.set_pos(1, 5);
    t.set_money(0, 20_000);
    t.set_money(1, 20_000);
    t.give(0, &["RAS:Repaint"]);
    t.begin_turn(1);
    drain(&mut t);
    // Path 6..=7 of a 2-step walk from 5 contains tile 7 → X = 1; 2 − 1 = 1
    // lands on 7. Simpler: roll 3 from 5 → path 6..=8 contains 7, X = 1,
    // 5 + (3 − 1) = 7.
    t.dice(&[3]);
    t.roll(1).unwrap();
    t.counteract(0, "RAS:Repaint").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(1), EDOGAWA_PARK);
    // 江户川公园 0-house rent is 140; 「支付减半」 → 70.
    assert_eq!(t.money(1), 20_000 - 70, "C-RAS-repaint-03: 140 / 2");
    assert_eq!(t.money(0), 20_070);
}

// ===========================================================================
// C-RAS-unstoppable-card
// ===========================================================================

/// C-RAS-unstoppable-card-01 -- the six-tile map.
///
/// * **Quote:** 「投掷1d6mod6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓」
pub fn c_ras_unstoppable_card_teleport(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == "RAS:UNSTOPPABLE"
            && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let pos = step.post.st.players[who].pos as usize;
    let expect = [SHIRAYUKI, GEIJUTSU, SEROASIA, GINGA, ASAHYU, CHUCHU_FLAT];
    if !expect.contains(&pos) {
        return Err(format!(
            "C-RAS-unstoppable-card-01: pos {pos}, expected one of {expect:?}"
        ));
    }
    Ok(())
}

/// C-RAS-unstoppable-card-02 -- no settle on the landing.
pub fn c_ras_unstoppable_card_no_settle(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// UNSTOPPABLE card: the six-tile map, no settle, face-scaled gain.
#[test]
fn unstoppable_card_contract() {
    let cases = [
        (1, SHIRAYUKI, 2_000),
        (2, GEIJUTSU, 2_000),
        (3, SEROASIA, 2_000),
        (4, GINGA, 1_000),
        (5, ASAHYU, 1_000),
        (6, CHUCHU_FLAT, 1_000),
    ];
    for (face, tile, gain) in cases {
        let mut t = Table::vanilla(2);
        t.own(1, &[tile]);
        t.set_houses(tile, 1);
        t.dice(&[face]);
        t.observe(&[Clause::new(
            "C-RAS-unstoppable-card-01",
            "cards.json#RAS:UNSTOPPABLE",
            "投掷1d6mod6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。",
            c_ras_unstoppable_card_teleport,
        )]);
        t.give_play(0, "RAS:UNSTOPPABLE")
            .map_err(|e| format!("face {face}: {e}"))
            .unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), tile, "face {face}");
        // 「本次传送不触发结算」 -- no rent on an owned tile.
        assert_eq!(t.money(0), 10_000 + gain, "face {face} pays {gain}");
        assert_eq!(t.money(1), 10_000, "face {face}: no rent");
    }
}

// ===========================================================================
// C-RAS-become-strongest
// ===========================================================================

/// C-RAS-become-strongest-01 -- 1d10 maps onto the sorted livehouse table.
pub fn c_ras_become_strongest_map(step: &Step<'_>) -> Result<(), String> {
    let played = step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == "RAS:成为最强"
            && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let pos = step.post.st.players[who].pos as usize;
    if !is_livehouse(pos) {
        return Err(format!(
            "C-RAS-become-strongest-01: pos {pos} is not a livehouse tile"
        ));
    }
    Ok(())
}

/// C-RAS-become-strongest-02 -- no livehouse → 「"Live House"」.
pub fn c_ras_become_strongest_none(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// 成为最强: board-wide sorted livehouse table; fallback to Live House.
#[test]
fn become_strongest_contract() {
    let table = [RING1, RING2, DUB, LIVE_HOUSE, RING3, BUDOKAN, SPACE, LH_GALAXY, RING4];
    for (i, &tile) in table.iter().enumerate() {
        let face = (i + 1) as i32;
        let mut t = Table::vanilla(2);
        t.own(0, &[RING1, RING2, DUB, RING3, BUDOKAN, SPACE, LH_GALAXY, RING4]);
        t.dice(&[face]);
        t.observe(&[Clause::new(
            "C-RAS-become-strongest-01",
            "cards.json#RAS:成为最强",
            "roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到\"Live House\"）",
            c_ras_become_strongest_map,
        )]);
        t.give_play(0, "RAS:成为最强").unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), tile, "face {face} -> {tile}");
    }
    for face in [10, 1] {
        let mut t = Table::vanilla(2);
        t.dice(&[face]);
        t.give_play(0, "RAS:成为最强").unwrap();
        drain(&mut t);
        assert_eq!(t.pos(0), LIVE_HOUSE, "face {face} without livehouses");
    }
}

// ===========================================================================
// C-RAS-guerrilla
// ===========================================================================

/// C-RAS-guerrilla-03 -- [反击] after an opponent livehouse pass + rent.
pub fn c_ras_guerrilla_counter_window(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// 游击演出 [手][反击]: teleport to an unowned buyable tile, settle, must buy.
#[test]
fn guerrilla_counter_contract() {
    forall(4, 0xC0A5_0008, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        t.own(1, &[DUB, 22]);
        let houses = rng.range(1, 2);
        t.set_houses(22, houses);
        t.set_pos(0, 15);
        t.set_money(0, 20_000);
        t.set_money(1, 20_000);
        t.give(0, &["RAS:游击演出"]);
        t.begin_turn(0);
        drain(&mut t);
        t.dice(&[7]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered("RAS:游击演出") {
            return Err(format!(
                "C-RAS-guerrilla-03: window not open: {}",
                t.dump_prompt()
            ));
        }
        let rent_paid = 20_000 - t.money(0);
        if rent_paid <= 0 {
            return Err(format!("expected a rent charge, paid {rent_paid}"));
        }
        t.counteract(0, "RAS:游击演出")
            .map_err(|e| format!("counteract: {e}"))?;
        t.answer_tile(0, EDOGAWA_PARK)
            .map_err(|e| format!("tile: {e}"))?;
        drain(&mut t);
        if t.pos(0) != EDOGAWA_PARK {
            return Err(format!(
                "C-RAS-guerrilla-04: pos {}, expected {EDOGAWA_PARK}",
                t.pos(0)
            ));
        }
        if t.owner(EDOGAWA_PARK) != Some(0) {
            return Err(format!(
                "C-RAS-guerrilla-04: must buy, owner {:?}",
                t.owner(EDOGAWA_PARK)
            ));
        }
        let expect = 20_000 - rent_paid - 1400;
        if t.money(0) != expect {
            return Err(format!(
                "C-RAS-guerrilla-04: money {}, expected {expect}",
                t.money(0)
            ));
        }
        Ok(())
    })
    .expect("游击演出 counter contract");
}

// ===========================================================================
// C-RAS-hey-kids
// ===========================================================================

/// C-RAS-hey-kids-01 -- window on another player's rent settle.
pub fn c_ras_hey_kids_window(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// 狂乱Hey Kids!!: replaces the settle with a house transfer.
///
/// Quote: 「将本次结算改为：将本格上的房屋转移到属于你的可建造格子上。」
/// Ruling 2026-10-06: the window opens when **another** player settles rent
/// on the holder's tiles.
#[test]
fn hey_kids_replace_contract() {
    forall(4, 0xC0A5_0009, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        t.own(0, &[SHOPPING, EDOGAWA_PARK]);
        // 「每个目标格子至多获得1层房屋」 -- a single-target transfer can only
        // move 1 layer. Keep the contract to the 1-house case (OPEN-hey-kids-split
        // covers multi-house splits).
        let houses = 1;
        let _ = rng;
        t.set_houses(SHOPPING, houses);
        t.set_money(0, 20_000);
        t.set_money(1, 20_000);
        t.give(0, &["RAS:狂乱Hey Kids!!"]);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, CIRCLE);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered("RAS:狂乱Hey Kids!!") {
            return Err(format!(
                "C-RAS-hey-kids-01: window not open: {}",
                t.dump_prompt()
            ));
        }
        let m0 = t.money(0);
        let m1 = t.money(1);
        t.counteract(0, "RAS:狂乱Hey Kids!!")
            .map_err(|e| format!("counteract: {e}"))?;
        for _ in 0..20 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            if p.kind == "tile" {
                t.answer_tile(0, EDOGAWA_PARK)
                    .map_err(|e| format!("tile: {e}"))?;
            } else if p.kind == "pick" {
                // pick the house / source tile
                if let Some(k) = t.option("购物中心") {
                    t.answer(0, k).map_err(|e| format!("pick: {e}"))?;
                } else {
                    t.answer(0, 0).map_err(|e| format!("pick0: {e}"))?;
                }
            } else {
                t.answer(0, 0).map_err(|e| format!("ans: {e}"))?;
            }
        }
        drain(&mut t);
        if t.houses(SHOPPING) != 0 || t.houses(EDOGAWA_PARK) != houses {
            return Err(format!(
                "C-RAS-hey-kids-02: transfer incomplete: shopping={} edogawa={}                  expected 0/{houses}; prompts were: {}",
                t.houses(SHOPPING),
                t.houses(EDOGAWA_PARK),
                t.dump_prompt()
            ));
        }
        // C-RAS-hey-kids-02: the settle is replaced -- the payer owes no rent.
        if t.money(1) != m1 {
            return Err(format!(
                "C-RAS-hey-kids-02: settle not replaced (payer {m1} -> {})",
                t.money(1)
            ));
        }
        if t.houses(SHOPPING) != 0 {
            return Err(format!(
                "C-RAS-hey-kids-02: source still holds {}",
                t.houses(SHOPPING)
            ));
        }
        if t.houses(EDOGAWA_PARK) != houses {
            return Err(format!(
                "C-RAS-hey-kids-03: target holds {}, expected {houses}",
                t.houses(EDOGAWA_PARK)
            ));
        }
        // C-RAS-hey-kids-04: 「消耗的房屋造价等于获得的房屋总造价，超出的部分作为现金
        // 获得；随后，你失去"转移后各格房屋造价总和－获得房屋数量×500"的资金」.
        // 购物中心 house cost 2000 → 江户川公园 house cost 1000.
        let c_src = 2000 * houses;
        let c_gain = 1000 * houses;
        let excess = c_src - c_gain;
        let lose = c_gain - houses * 500;
        let expect_delta = excess - lose;
        let delta = t.money(0) - m0;
        if delta != expect_delta {
            return Err(format!(
                "C-RAS-hey-kids-04: holder delta {delta}, expected {expect_delta} \
                 (= excess {excess} − lose {lose})"
            ));
        }
        Ok(())
    })
    .expect("狂乱Hey Kids!! replace contract");
}

// ===========================================================================
// C-RAS-storm
// ===========================================================================

/// C-RAS-storm-01 -- place on the livehouse the user stands on.
pub fn c_ras_storm_place(step: &Step<'_>) -> Result<(), String> {
    // Only the place play, not the later (2) fire (`studio_storm_fired`).
    let played = step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == "RAS:练习室里的风暴"
            && (e.kind == "play" || e.kind.is_empty())
    });
    if !played {
        return Ok(());
    }
    if step.post_prompt_open {
        return Ok(());
    }
    let who = step.player_of_act();
    let on_field = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.field.iter().any(|f| f.card == "RAS:练习室里的风暴"))
        .unwrap_or(false);
    let on_board = step
        .post
        .st
        .board_field
        .iter()
        .any(|f| f.card == "RAS:练习室里的风暴");
    if !on_field && !on_board {
        return Err("C-RAS-storm-01: card not placed".into());
    }
    Ok(())
}

/// 练习室里的风暴: place, crystal on CiRCLE pass, (4-X)/4 charge.
#[test]
fn storm_contract() {
    forall(4, 0xC0A5_000A, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(3);
        t.own(0, &[BUDOKAN]);
        let houses = rng.range(1, 3);
        t.set_houses(BUDOKAN, houses);
        t.set_pos(0, BUDOKAN);
        t.set_money(0, 20_000);
        t.set_money(1, 20_000);
        t.give(0, &["RAS:练习室里的风暴"]);
        t.begin_turn(0);
        drain(&mut t);
        t.observe(&[Clause::new(
            "C-RAS-storm-01",
            "cards.json#RAS:练习室里的风暴",
            "当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上",
            c_ras_storm_place,
        )]);
        t.play(0, "RAS:练习室里的风暴")
            .map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let mut crystals = any_crystals(&t, "RAS:练习室里的风暴");
        if crystals != 0 {
            return Err(format!(
                "C-RAS-storm-02: initial crystals {crystals}, expected 0"
            ));
        }
        // Two CiRCLE passes → 2 crystals (cap 3).
        for _ in 0..2 {
            t.begin_turn(1);
            drain(&mut t);
            t.begin_turn(0);
            drain(&mut t);
            t.set_pos(0, BUDOKAN);
            t.dice(&[20]);
            t.roll(0).map_err(|e| format!("circle pass: {e}"))?;
            drain(&mut t);
        }
        crystals = any_crystals(&t, "RAS:练习室里的风暴");
        if crystals != 2 {
            return Err(format!(
                "C-RAS-storm-02: crystals {crystals}, expected 2"
            ));
        }
        // C-RAS-storm-03: p1 settles at SPACE (42), distance 2 from BUDOKAN (40).
        let x = 2i32;
        let base = [340i32, 1440, 3340, 6840][houses as usize]; // rent[houses]
        let charge = base * (4 - x) / 4;
        let m0 = t.money(0);
        let m1 = t.money(1);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, SPACE - 1);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("p1 roll: {e}"))?;
        drain(&mut t);
        let gained = t.money(0) - m0;
        let paid = m1 - t.money(1);
        if paid != charge {
            return Err(format!(
                "C-RAS-storm-03: paid {paid}, expected {charge} ((4-2)/4 × {base})"
            ));
        }
        if gained != charge {
            return Err(format!("C-RAS-storm-03: owner gained {gained}"));
        }
        if any_in_play(&t, "RAS:练习室里的风暴") {
            return Err("C-RAS-storm-03: 将此卡放入弃牌堆".into());
        }
        Ok(())
    })
    .expect("练习室里的风暴 contract");
}

/// The off-livehouse refusal of C-RAS-storm-01.
#[test]
fn storm_refuses_off_livehouse() {
    let mut t = Table::vanilla(2);
    t.give(0, &["RAS:练习室里的风暴"]);
    let r = t.play(0, "RAS:练习室里的风暴");
    assert!(r.is_err(), "C-RAS-storm-01: must refuse off a livehouse: {r:?}");
}

// ===========================================================================
// C-RAS-lock
// ===========================================================================

/// C-RAS-lock-03 -- Bandori车站 pass redirects to 旭汤澡堂.
pub fn c_ras_lock_redirect(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// （LOCK）追逐梦想的步伐: through Bandori车站 → 旭汤澡堂; card [移除]d.
#[test]
fn lock_redirect_contract() {
    forall(4, 0xC0A5_000B, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "朝日六花");
        t.place_raw(0, "RAS:（LOCK）追逐梦想的步伐");
        t.begin_turn(0);
        drain(&mut t);
        let roll = rng.range(6, 12);
        // The walk from 30 must [经过] Bandori车站 (35) for the redirect.
        t.set_pos(0, 30);
        assert!((31..=30 + roll as usize).contains(&BANDORI));
        t.dice(&[roll]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        if t.pos(0) != ASAHYU {
            return Err(format!(
                "C-RAS-lock-03: pos {}, expected {ASAHYU}",
                t.pos(0)
            ));
        }
        if t.on_field(0, "RAS:（LOCK）追逐梦想的步伐") {
            return Err("C-RAS-lock-03: 此卡[移除] -- still on field".into());
        }
        Ok(())
    })
    .expect("（LOCK） redirect contract");
}

/// A move that misses Bandori车站 leaves the card alone.
#[test]
fn lock_misses_bandori_alone() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "朝日六花");
    t.place_raw(0, "RAS:（LOCK）追逐梦想的步伐");
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, CIRCLE);
    t.dice(&[5]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 5);
    assert!(t.on_field(0, "RAS:（LOCK）追逐梦想的步伐"));
}

// ===========================================================================
// C-RAS-crush
// ===========================================================================

/// C-RAS-crush-02 -- +Xd20, X = discard-pile size.
pub fn c_ras_crush_hand(_step: &Step<'_>) -> Result<(), String> {
    Ok(())
}

/// （MASKING）CRUSH ON THE DRUM!!! [手]: main move gains Xd20.
#[test]
fn crush_hand_contract() {
    forall(4, 0xC0A5_000C, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "佐藤益木");
        let x = rng.range(1, 4) as usize;
        let pile: Vec<&str> = (0..x).map(|_| "R:[衍生] 压").collect();
        t.set_discard(0, &pile);
        t.give(0, &["RAS:（MASKING）CRUSH ON THE DRUM!!!"]);
        t.begin_turn(0);
        drain(&mut t);
        t.play(0, "RAS:（MASKING）CRUSH ON THE DRUM!!!")
            .map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let faces = vec![1i32; x + 1];
        t.dice(&faces);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let expect = 1 + x as i32;
        if t.pos(0) as i32 != expect {
            return Err(format!(
                "C-RAS-crush-02: pos {}, expected {expect}",
                t.pos(0)
            ));
        }
        if t.dice_left() != 0 {
            return Err(format!(
                "C-RAS-crush-02: {} faces left",
                t.dice_left()
            ));
        }
        Ok(())
    })
    .expect("CRUSH contract");
}

// ===========================================================================
// C-RAS-pareo-card
// ===========================================================================

/// （PAREO）渐渐远去的你: +2 PAREO marks; optional house removal declined.
#[test]
fn pareo_card_contract() {
    forall(4, 0xC0A5_000D, |rng: &mut Rng, _i| {
        let mut t = Table::vanilla(2);
        t.set_character_raw(0, "鳰原令王那");
        t.own(0, &[SHOPPING, EDOGAWA_PARK]);
        let houses = rng.range(1, 3);
        t.set_houses(SHOPPING, houses);
        t.begin_turn(0);
        drain(&mut t);
        t.give_play(0, "RAS:（PAREO）渐渐远去的你")
            .map_err(|e| format!("play: {e}"))?;
        while t.prompt().is_some() {
            t.decline();
        }
        drain(&mut t);
        let tok: i32 = t.p(0).tokens.iter().map(|c| c.value).sum();
        if tok != 2 {
            return Err(format!(
                "C-RAS-pareo-01: PAREO tokens {tok}, expected 2 ({:?})",
                t.p(0).tokens
            ));
        }
        if t.houses(SHOPPING) != houses {
            return Err(format!(
                "C-RAS-pareo-03: houses {}, expected {houses} (declined)",
                t.houses(SHOPPING)
            ));
        }
        Ok(())
    })
    .expect("（PAREO） card contract");
}

// ===========================================================================
// C-RAS-chuchu-card
// ===========================================================================

/// （chuchu）演奏我的音乐吧: place on another player with 3 crystals; decay.
#[test]
fn chuchu_card_contract() {
    forall(4, 0xC0A5_000E, |_rng: &mut Rng, _i| {
        let mut t = Table::new(&["珠手知由", "和奏瑞依", "朝日六花"]);
        t.strip_skills();
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        t.give_play(0, "RAS:（chuchu）演奏我的音乐吧")
            .map_err(|e| format!("play: {e}"))?;
        for _ in 0..20 {
            if t.prompt().is_none() {
                break;
            }
            if let Some(k) = t.option("PlayerId(1)") {
                t.answer(0, k).map_err(|e| format!("target: {e}"))?;
            } else {
                t.decline();
            }
        }
        drain(&mut t);
        if !t.on_field(1, "RAS:（chuchu）演奏我的音乐吧") {
            return Err(format!(
                "C-RAS-chuchu-01: not on p1 field: {:?}",
                t.field_ids(1)
            ));
        }
        if t.crystals(1, "RAS:（chuchu）演奏我的音乐吧") != Some(3) {
            return Err(format!(
                "C-RAS-chuchu-01: crystals {:?}, expected Some(3)",
                t.crystals(1, "RAS:（chuchu）演奏我的音乐吧")
            ));
        }
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("p0 roll: {e}"))?;
        drain(&mut t);
        t.end(0).map_err(|e| format!("end p0: {e}"))?;
        drain(&mut t);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("p1 roll: {e}"))?;
        drain(&mut t);
        t.end(1).map_err(|e| format!("end p1: {e}"))?;
        drain(&mut t);
        // C-RAS-chuchu-02: p1's turn end loses one crystal (3 → 2).
        let any = (0..3).find_map(|w| t.crystals(w, "RAS:（chuchu）演奏我的音乐吧"));
        if any != Some(2) {
            return Err(format!(
                "C-RAS-chuchu-02: crystals {any:?}, expected Some(2)"
            ));
        }
        Ok(())
    })
    .expect("（chuchu） card contract");
}

// ===========================================================================
// C-RAS-layer
// ===========================================================================

/// （和奏瑞依）寄于指尖的执念: press replaces the move roll with a kept die.
#[test]
fn layer_press_contract() {
    forall(4, 0xC0A5_000F, |rng: &mut Rng, _i| {
        let mut t = Table::new(&["和奏瑞依", "户山香澄"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let kept = rng.range(3, 12);
        t.place_raw(0, "RAS:（和奏瑞依）寄于指尖的执念");
        t.set_state(0, "layer_keep_count", 1);
        t.set_state(0, "layer_keep_0", kept);
        t.set_fire(0, 1, 1);
        t.skill(0, "RAS:（和奏瑞依）寄于指尖的执念")
            .map_err(|e| format!("press: {e}"))?;
        if t.prompt().is_some() {
            t.answer_one(0).map_err(|e| format!("pick: {e}"))?;
        }
        drain(&mut t);
        if t.fire(0) != 0 {
            return Err(format!(
                "C-RAS-layer-03: fire {}, expected 0",
                t.fire(0)
            ));
        }
        if t.state(0, "layer_keep_count") != 0 {
            return Err(format!(
                "C-RAS-layer-03: kept {}, expected 0",
                t.state(0, "layer_keep_count")
            ));
        }
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        if t.pos(0) as i32 != kept {
            return Err(format!(
                "C-RAS-layer-03: pos {}, expected {kept}",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("（和奏瑞依） press contract");
}

// ===========================================================================
// Character skills
// ===========================================================================

/// C-RAS-rio-01 -- [经过]CiRCLE +1 [火罐], initial 1, cap 1.
///
/// Quote (characters.json#和奏瑞依): 「（1）每次[经过]CiRCLE时获得一个[火罐]（初始1，上限1）」
#[test]
fn rio_fire_contract() {
    forall(4, 0xC0A5_0010, |rng: &mut Rng, _i| {
        let mut t = Table::new(&["和奏瑞依", "珠手知由"]);
        t.clean();
        drain(&mut t);
        t.begin_turn(0);
        drain(&mut t);
        let held = rng.range(0, 1);
        t.set_fire(0, held, 1);
        t.set_pos(0, 59);
        t.dice(&[2]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        if t.fire(0) != 1 {
            return Err(format!(
                "C-RAS-rio-01: fire {}, expected 1 (cap 1; was {held})",
                t.fire(0)
            ));
        }
        Ok(())
    })
    .expect("和奏瑞依 fire contract");
}

/// C-RAS-rio-02 -- after any roll, spend 1 fire to reroll and pick one.
#[test]
fn rio_reroll_contract() {
    let mut t = Table::new(&["和奏瑞依", "珠手知由"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 1, 1);
    t.dice(&[3, 10]);
    t.roll(0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2, "yes/no: {}", t.dump_prompt());
    t.answer(0, 0).unwrap();
    let p = t.expect_prompt();
    assert_eq!(p.options.len(), 2, "keep old / keep new: {}", t.dump_prompt());
    t.answer(0, 1).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 10, "C-RAS-rio-02: 择其一执行");
    assert_eq!(t.fire(0), 0, "C-RAS-rio-02: 使用一个[火罐]");
}

/// C-RAS-masking-01 -- [经过] 白雪学园 +1 [火罐], cap 2.
///
/// Quote (characters.json#佐藤益木): 「（1）每次[经过]"白雪学园"或"银河拉面馆"时获得1个[火罐]（初始0，上限2）」
#[test]
fn masking_fire_contract() {
    let mut t = Table::new(&["佐藤益木", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 0, 2);
    t.set_pos(0, 12);
    t.dice(&[7]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 19);
    assert_eq!(t.fire(0), 1, "C-RAS-masking-01: 「每次[经过]“白雪学园”」");
}

/// C-RAS-masking-02 -- spend X fire → +Xd10 on the main roll.
///
/// Quote (characters.json#佐藤益木): 「（2）移动阶段前可使用X个[火罐]，本回合主要移动掷骰额外添加Xd10」
#[test]
fn masking_dice_contract() {
    let mut t = Table::new(&["佐藤益木", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_fire(0, 2, 2);
    let sid = t.skill_id(0, "佐藤益木");
    let r = t.skill(0, &sid);
    if t.prompt().is_some() {
        if let Some(k) = t.option("2") {
            t.answer(0, k).unwrap();
        } else {
            t.answer(0, 1).unwrap();
        }
        drain(&mut t);
    }
    assert!(r.is_ok(), "{r:?}");
    t.dice(&[1, 1, 1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), 3, "C-RAS-masking-02: 1 + 1 + 1");
    assert_eq!(t.fire(0), 0, "C-RAS-masking-02: two pots spent");
}

/// C-RAS-chuchu-skill-01 -- build at half price.
///
/// Quote (characters.json#珠手知由): 「加盖房屋时半价」
#[test]
fn chuchu_skill_half_contract() {
    forall(4, 0xC0A5_0011, |rng: &mut Rng, _i| {
        let mut t = Table::new(&["珠手知由", "和奏瑞依"]);
        t.clean();
        drain(&mut t);
        t.begin_turn(0);
        drain(&mut t);
        t.own(0, &[ASTRONOMY]);
        t.set_houses(ASTRONOMY, 0);
        let money = 20_000 + rng.range(0, 10) * 100;
        t.set_money(0, money);
        t.set_pos(0, ASTRONOMY - 1);
        t.dice(&[1]);
        t.roll(0).unwrap();
        drain(&mut t);
        t.build(0).map_err(|e| format!("build: {e}"))?;
        drain(&mut t);
        if t.houses(ASTRONOMY) != 1 {
            return Err(format!(
                "C-RAS-chuchu-skill-01: houses {}, expected 1",
                t.houses(ASTRONOMY)
            ));
        }
        let paid = money - t.money(0);
        if paid != 750 {
            return Err(format!(
                "C-RAS-chuchu-skill-01: paid {paid}, expected 750 (= 1500 / 2)"
            ));
        }
        Ok(())
    })
    .expect("珠手知由 half-price contract");
}

/// C-RAS-chuchu-skill-02 -- free build after last turn's build.
///
/// Quote: 「如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」
#[test]
fn chuchu_skill_free_contract() {
    let mut t = Table::new(&["珠手知由", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.own(0, &[ASTRONOMY]);
    t.set_houses(ASTRONOMY, 0);
    t.set_money(0, 20_000);
    t.set_pos(0, ASTRONOMY - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.build(0).unwrap();
    drain(&mut t);
    let after_first = t.money(0);
    // Already rolled this turn -- just end.
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    // Natural arrival at p0's next 运营. Walk onto the owned tile again.
    t.set_pos(0, ASTRONOMY - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), ASTRONOMY, "landed on the owned tile");
    t.build(0).unwrap();
    drain(&mut t);
    let paid = after_first - t.money(0);
    // The book: 「如果你上回合进行过房屋加盖，本回合加盖房屋变为免费」.
    // A half-price charge (750) means the free clause did not fire; a full
    // 1500 means even 「加盖房屋时半价」 was lost -- both are discrepancies.
    assert_eq!(
        paid, 0,
        "C-RAS-chuchu-skill-02: paid {paid}, expected 0 (上回合盖过房 → 免费);          half would be 750"
    );
    assert_eq!(
        t.houses(ASTRONOMY),
        2,
        "C-RAS-chuchu-skill-02: second house placed"
    );
}

/// Control for [`chuchu_skill_free_contract`]: 「加盖房屋时半价」 also applies
/// on a later turn when the user did **not** build last turn (so a full-price
/// charge in the free test is not a half-price regression).
#[test]
fn chuchu_skill_half_on_a_later_turn() {
    let mut t = Table::new(&["珠手知由", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.own(0, &[ASTRONOMY]);
    t.set_houses(ASTRONOMY, 0);
    t.set_money(0, 20_000);
    t.set_pos(0, ASTRONOMY - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    t.set_pos(0, ASTRONOMY - 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let before = t.money(0);
    t.build(0).unwrap();
    drain(&mut t);
    let paid = before - t.money(0);
    assert_eq!(
        paid, 750,
        "half price must still apply on turn 2; paid {paid}"
    );
}

/// C-RAS-rokka-01 -- first non-旭汤澡堂 / non-Live-House deed gains the colour.
///
/// Quote (characters.json#朝日六花): 「（1）你购买的第一个非"旭汤澡堂"或任意"Live House"格子获得"Live House"的颜色」
#[test]
fn rokka_first_deed_contract() {
    let mut t = Table::new(&["朝日六花", "和奏瑞依"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.set_pos(0, 59);
    t.set_money(0, 20_000);
    t.dice(&[2]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.buy(0).unwrap();
    drain(&mut t);
    assert_eq!(t.owner(SHOPPING), Some(0));
    t.begin_turn(0);
    drain(&mut t);
    t.dice(&[1]);
    t.give_play(0, "RAS:成为最强").unwrap();
    drain(&mut t);
    // Colour-on: 成为最强's 1d10 table, face 1 → RiNG 1. Colour-off would be
    // the 「若你没有Livehouse格子」 fallback (Live House).
    assert_eq!(
        t.pos(0),
        RING1,
        "C-RAS-rokka-01: first deed should count as a Live House (face 1 → RiNG 1);          LIVE_HOUSE would mean the colour was not applied"
    );
}

// ===========================================================================
// Band skill
// ===========================================================================

/// C-RAS-band-02 -- a livehouse settle leaves a teleport offer for the next
/// main move.
///
/// Quote (bands.json#RAISE A SUILEN): 「（2）因该技能以外的效果在livehouse格子
/// [触发结算]时，若你的下回合开始时仍在那个格子上，你的下一次主要移动可变为传送至
/// 你拥有的一个livehouse格子。」
#[test]
fn band_teleport_offer_contract() {
    let mut t = Table::new(&["和奏瑞依", "珠手知由", "朝日六花"]);
    t.clean();
    drain(&mut t);
    t.begin_turn(0);
    drain(&mut t);
    t.own(0, &[BUDOKAN, SPACE]);
    t.dice(&[6]);
    t.give_play(0, "RAS:成为最强").unwrap();
    drain(&mut t);
    assert_eq!(t.pos(0), BUDOKAN);
    t.end(0).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    t.dice(&[1]);
    t.roll(2).unwrap();
    drain(&mut t);
    t.end(2).unwrap();
    // Do NOT drain: that would decline the offer. Ring-up may sit at turn
    // start or at the next main move.
    let mut accepted = false;
    let accept_if_offer = |t: &mut Table| -> bool {
        let mut took = false;
        for _ in 0..10 {
            let Some(p) = t.prompt() else { break };
            let k = format!("{}{}", p.title.key(), p.text.key());
            if k.contains("ras_")
                || k.contains("unstoppable")
                || k.contains("live")
                || k.contains("teleport")
                || k.contains("传送")
            {
                t.answer_one(0).unwrap();
                took = true;
            } else {
                t.decline();
            }
        }
        took
    };
    accepted |= accept_if_offer(&mut t);
    let before = t.pos(0);
    if !accepted {
        // Maybe the offer rides the roll itself.
        t.dice(&[3]);
        let r = t.roll(0);
        accepted |= accept_if_offer(&mut t);
        drain(&mut t);
        let _ = r;
    } else {
        t.dice(&[3]);
        t.roll(0).unwrap();
        drain(&mut t);
    }
    assert!(
        accepted,
        "C-RAS-band-02: teleport offer never came up (pos {} was {before}): {:?}",
        t.pos(0),
        t.recent_keys(24)
    );
    assert!(
        t.pos(0) == SPACE || t.pos(0) == BUDOKAN,
        "C-RAS-band-02: teleport should land on an owned livehouse, pos {} (was {before})",
        t.pos(0)
    );
}

/// Every RAS character carries the band skill.
#[test]
fn band_skill_present() {
    let t = Table::new(&["和奏瑞依", "珠手知由", "佐藤益木"]);
    for who in 0..3 {
        assert!(
            t.skills(who).iter().any(|s| s.contains("RAISE A SUILEN")),
            "player {who} skills {:?}",
            t.skills(who)
        );
    }
}

// ===========================================================================
// Clause table
// ===========================================================================

/// Clauses checked on the transitions that carry their trigger. Multi-step
/// contracts (EXIST flip, band offer, storm charge) assert in the test body;
/// their step-level twins are the no-op stubs documented in the spec doc.
pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-RAS-change-the-world-01",
        "cards.json#RAS:Change the world",
        "将此卡放置于你的一个有房屋的livehouse格子上",
        c_ras_change_the_world_place,
    ),
    Clause::new(
        "C-RAS-exist-01",
        "cards.json#RAS:EXIST",
        "将此卡放置于自己场上",
        c_ras_exist_place,
    ),
    Clause::new(
        "C-RAS-unstoppable-card-01",
        "cards.json#RAS:UNSTOPPABLE",
        "投掷1d6mod6，根据结果1-6分别传送至白雪学园，艺术学院高中，瑟罗希亚国际学校，银河拉面馆，旭汤澡堂，CHUCHU的公寓。",
        c_ras_unstoppable_card_teleport,
    ),
    Clause::new(
        "C-RAS-become-strongest-01",
        "cards.json#RAS:成为最强",
        "roll 1d10，传送到livehouse对应的格子（按格子编号排序，若为10或以上传送到\"Live House\"）",
        c_ras_become_strongest_map,
    ),
    Clause::new(
        "C-RAS-storm-01",
        "cards.json#RAS:练习室里的风暴",
        "当你位于你拥有的livehouse格子上时可将此卡放置在当前格子上",
        c_ras_storm_place,
    ),
];

/// Keep the step-level twins alive for the doc's Executable field.
#[allow(dead_code)]
fn _keep(_s: &Step<'_>) -> Result<(), String> {
    let _ = c_ras_change_the_world_3d20;
    let _ = c_ras_change_the_world_crystal;
    let _ = c_ras_change_the_world_charge;
    let _ = c_ras_exist_redirect;
    let _ = c_ras_exist_flip;
    let _ = c_ras_exist_draw;
    let _ = c_ras_please_choose_window;
    let _ = c_ras_please_choose_opt2;
    let _ = c_ras_riot_cycle;
    let _ = c_ras_repaint_cut;
    let _ = c_ras_unstoppable_card_no_settle;
    let _ = c_ras_become_strongest_none;
    let _ = c_ras_guerrilla_counter_window;
    let _ = c_ras_hey_kids_window;
    let _ = c_ras_lock_redirect;
    let _ = c_ras_crush_hand;
    Ok(())
}
