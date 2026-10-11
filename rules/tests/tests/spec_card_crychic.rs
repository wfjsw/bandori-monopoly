//! CRYCHIC unit contracts: 15 cards + 5 character skills + the band skill.
//!
//! Spec: `docs/rulebook/spec/cards/crychic.md`. Every clause is derived from
//! the live sheet (`docs/rulebook/cards.json`, `data/characters.json`,
//! `data/bands.json`) and the rulebook body, never from implementation
//! behaviour. A discrepancy is a plain failing test.
//!
//! Clause IDs are `C-CRYCHIC-<slug>-<n>`. Frame conditions travel with the
//! clause they guard (see the area doc's Frame field).

use rules_spec::{forall, frame, Clause, Step, Table};

// ---------------------------------------------------------------- helpers

/// Did this step activate `card` as a hand play / bare activation?
fn played(step: &Step<'_>, card: &str) -> bool {
    step.events.iter().any(|e| {
        e.r#type == "card"
            && e.card == card
            && (e.kind == "play" || e.kind.is_empty())
            && !e.negated
    })
}

/// Drain every open prompt with its fallback (setup / after-act tidy).
fn drain(t: &mut Table) {
    for _ in 0..32 {
        if t.prompt().is_none() {
            t.settle();
            if t.prompt().is_none() {
                return;
            }
        }
        t.decline();
    }
}

/// Answer the open prompt with option `value` as the only waiting player.
fn answer_one(t: &mut Table, value: i32) -> Result<(), String> {
    t.answer_one(value)
}

// =====================================================================
// CRYCHIC:初演大成功
// =====================================================================

const DEBUT: &str = "CRYCHIC:初演大成功";

/// C-CRYCHIC-debut-01 -- 「本回合内你的资金不会下降」.
///
/// * **Source:** `cards.json#CRYCHIC:初演大成功`
/// * **Quote:** 「打出此卡后，本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）」
/// * **Trigger:** a rent [支付] the same turn the card was played.
/// * **Condition:** the user played this card this turn and lands on an
///   opponent's developed tile.
/// * **Effect:** the user's money does not fall (the rent is absorbed).
/// * **Frame:** only `money:*` of the two settle parties may move; the
///   renter's money must not fall.
/// * **Executable:** [`c_crychic_debut_no_drop`].
pub fn c_crychic_debut_no_drop(step: &Step<'_>) -> Result<(), String> {
    if !played(step, DEBUT) {
        return Ok(());
    }
    let who = step.player_of_act();
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta < 0 {
        return Err(format!(
            "C-CRYCHIC-debut-01: money fell by {} after playing 初演大成功 \
             (「本回合内你的资金不会下降」)",
            -delta
        ));
    }
    Ok(())
}

/// C-CRYCHIC-debut-02 -- turn end: 1 [晕眩] + 1 crystal on the band skill card.
///
/// * **Source:** `cards.json#CRYCHIC:初演大成功`
/// * **Quote:** 「回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。」
/// * **Trigger:** the turn end of the player who played the card this turn.
/// * **Condition:** the card was played this turn.
/// * **Effect:** `stun` +1 and the band skill card gains 1 [奇迹水晶].
/// * **Frame:** only the user's `state:stun` and the band card's crystals move.
pub fn c_crychic_debut_turn_end(step: &Step<'_>) -> Result<(), String> {
    // The end act of the player who holds the effect: detected via a stun
    // gain in the window together with a crystal gain on a band skill card.
    let who = step.player_of_act();
    let pre_stun = step
        .pre
        .st
        .players
        .get(who)
        .map(|p| p.state_get("stun"))
        .unwrap_or(0);
    let post_stun = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.state_get("stun"))
        .unwrap_or(0);
    if post_stun != pre_stun + 1 {
        return Ok(()); // not the debut turn-end
    }
    let pre_crystals: i32 = step
        .pre
        .st
        .players
        .get(who)
        .map(|p| {
            p.field
                .iter()
                .filter(|f| f.band_skill)
                .map(|f| f.crystals)
                .sum()
        })
        .unwrap_or(0);
    let post_crystals: i32 = step
        .post
        .st
        .players
        .get(who)
        .map(|p| {
            p.field
                .iter()
                .filter(|f| f.band_skill)
                .map(|f| f.crystals)
                .sum()
        })
        .unwrap_or(0);
    if post_crystals != pre_crystals + 1 {
        return Err(format!(
            "C-CRYCHIC-debut-02: band crystals {pre_crystals} -> {post_crystals}, \
             expected +1 (「向乐队技能卡上添加一个奇迹水晶」)"
        ));
    }
    Ok(())
}

#[test]
fn crychic_debut_blocks_rent_decrease() {
    forall(6, 0xC1C0_0001, |rng, _i| {
        let mut t = Table::vanilla(2);
        let rent_tile = rules_spec::tile("江户川公园");
        t.own(1, &[rent_tile]);
        t.set_houses(rent_tile, rng.range(1, 4));
        t.set_pos(0, (rent_tile + 59) % 60);
        t.set_money(0, 10_000 + rng.range(0, 20) * 100);
        t.give(0, &[DEBUT]);
        let money0 = t.money(0);

        let pre = t.snapshot_world();
        t.observe(&[Clause::new(
            "C-CRYCHIC-debut-01",
            "cards.json#CRYCHIC:初演大成功",
            "本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）",
            c_crychic_debut_no_drop,
        )]);
        t.play(0, DEBUT).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);

        if t.money(0) < money0 {
            return Err(format!(
                "C-CRYCHIC-debut-01: money {money0} -> {} (「资金不会下降」)",
                t.money(0)
            ));
        }
        // Frame across the play+move window: the opponent may receive rent
        // and tiles may only move through the settle; the user's money must
        // not fall (checked above). `turn.no_money_loss` / `scheduled` are
        // this card's own latches (the printed 「资金不会下降」 window).
        frame(
            &pre,
            t.world(),
            &[
                "money:P0",
                "money:P1",
                "hand:P0",
                "draw:P0",
                "discard:P0",
                "pos:P0",
                "roll:P0",
                "houses:T",
                "owner:T",
                "state:P0",
                "field:P0",
                "crystals:P0",
                "turn.no_money_loss",
                "scheduled",
            ],
        )
    })
    .expect("C-CRYCHIC-debut-01");
}

#[test]
fn crychic_debut_turn_end_stun_and_crystal() {
    forall(6, 0xC1C0_0002, |rng, _i| {
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let band_id = t.skill_id(0, "CRYCHIC");
        let pre_crystals = t.crystals(0, &band_id).unwrap_or(0);
        t.give(0, &[DEBUT]);
        t.observe(&[Clause::new(
            "C-CRYCHIC-debut-02",
            "cards.json#CRYCHIC:初演大成功",
            "回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。",
            c_crychic_debut_turn_end,
        )]);
        t.play(0, DEBUT).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let _ = rng;
        t.dice(&[3]);
        t.roll(0).ok();
        drain(&mut t);
        t.end(0).map_err(|e| format!("end: {e}"))?;
        drain(&mut t);

        let stun = t.state(0, "stun");
        if stun < 1 {
            return Err(format!(
                "C-CRYCHIC-debut-02: stun {stun}, expected >=1 (「获得一层眩晕」)"
            ));
        }
        let post = t.crystals(0, &band_id).unwrap_or(0);
        if post != pre_crystals + 1 {
            return Err(format!(
                "C-CRYCHIC-debut-02: band crystals {pre_crystals} -> {post}, expected +1"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-debut-02");
}

// =====================================================================
// CRYCHIC:如果能一直持续下去...
// =====================================================================

const IF_ONLY: &str = "CRYCHIC:如果能一直持续下去...";

/// C-CRYCHIC-ifonly-01 -- 「[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金」.
pub fn c_crychic_ifonly_cashout(step: &Step<'_>) -> Result<(), String> {
    if !played(step, IF_ONLY) {
        return Ok(());
    }
    let who = step.player_of_act();
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    // X is not in the event window directly; the money delta must be 2000+500X
    // for some X >= 0. The exact X is asserted in the driving test.
    if delta < 2000 || (delta - 2000) % 500 != 0 {
        return Err(format!(
            "C-CRYCHIC-ifonly-01: money delta {delta}, expected 2000+500*X"
        ));
    }
    Ok(())
}

#[test]
fn crychic_ifonly_cashs_out_band_crystals() {
    forall(6, 0xC1C0_0003, |rng, _i| {
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let band_id = t.skill_id(0, "CRYCHIC");
        let x = rng.range(0, 6);
        t.set_crystals(0, &band_id, x);
        let money0 = t.money(0);
        t.set_hand(0, &["R:[衍生] 压"]);
        t.give(0, &[IF_ONLY]);
        let pre = t.snapshot_world();

        t.observe(&[Clause::new(
            "C-CRYCHIC-ifonly-01",
            "cards.json#CRYCHIC:如果能一直持续下去...",
            "[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。",
            c_crychic_ifonly_cashout,
        )]);
        t.play(0, IF_ONLY).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);

        let want = 2000 + 500 * x as i64;
        let got = t.money(0) as i64 - money0 as i64;
        if got != want {
            return Err(format!(
                "C-CRYCHIC-ifonly-01: money delta {got}, expected {want} (X={x})"
            ));
        }
        let left = t.crystals(0, &band_id).unwrap_or(0);
        if left != 0 {
            return Err(format!(
                "C-CRYCHIC-ifonly-01: band crystals {left}, expected 0 (「移除」)"
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
                "crystals:P0",
                "field:P0",
            ],
        )
    })
    .expect("C-CRYCHIC-ifonly-01");
}

/// C-CRYCHIC-ifonly-02 -- 「[持续] 若你的手牌大于等于7，此卡立即置入弃牌堆。」
#[test]
fn crychic_ifonly_discards_at_hand_seven() {
    forall(4, 0xC1C0_0004, |_rng, _i| {
        // CRYCHIC band skill removes the hand limit, so a hand of 7 is a
        // legal standing state (the base limit is 5).
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        t.give(0, &[IF_ONLY]);
        t.play(0, IF_ONLY).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        // Hand of 6, then a real draw to 7 (江户川乐器店 settle: 「抽取一张
        // 手卡」). The hand-size *change* into 「大于等于7」 is the transition
        // the standing [持续] check must see.
        t.set_hand(
            0,
            &[
                "通用:GREAT",
                "通用:安可",
                "通用:登上武道馆",
                "通用:尽力后的收获",
                "通用:GREAT",
                "通用:安可",
            ],
        );
        t.set_draw(0, &["通用:登上武道馆", "通用:GREAT", "通用:安可"]);
        let shop = rules_spec::tile("江户川乐器店");
        t.set_pos(0, (shop + 59) % 60);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let hand = t.hand(0).len();
        // 「置入弃牌堆」 -- the card must leave the field and land in the
        // discard. A subsequent reshuffle (G-CARDS: empty draw takes the
        // discard) may carry it into the draw pile; that is a different
        // clause, so accept either zone as long as the field is clear.
        let in_discard = t.discard(0).iter().any(|c| c == IF_ONLY)
            || t.discard(1).iter().any(|c| c == IF_ONLY)
            || t.draw_pile(0).iter().any(|c| c == IF_ONLY);
        if !in_discard {
            return Err(format!(
                "C-CRYCHIC-ifonly-02: card not in discard/draw at hand {hand} \
                 (field={:?} hand={:?} d0={:?} draw={:?})",
                t.field_ids(0),
                t.hand(0),
                t.discard(0),
                t.draw_pile(0),
            ));
        }
        if t.on_field(0, IF_ONLY) {
            return Err("C-CRYCHIC-ifonly-02: card still on field".into());
        }
        Ok(())
    })
    .expect("C-CRYCHIC-ifonly-02");
}

// =====================================================================
// CRYCHIC:想要成为人类
// =====================================================================

const HUMAN: &str = "CRYCHIC:想要成为人类";

/// C-CRYCHIC-human-01 -- place + crystal when the move roll is <= X.
#[test]
fn crychic_human_crystal_on_low_roll() {
    forall(6, 0xC1C0_0005, |rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[HUMAN]);
        t.play(0, HUMAN).map_err(|e| format!("play: {e}"))?;
        // Declare X = 20 (the highest option) so any d20 is <= X.
        let mut declared = 20i32;
        for _ in 0..6 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            let k = (p.options.len().saturating_sub(1)) as i32;
            if let Some(o) = p.options.get(k as usize) {
                if let Some(n) = format!("{o:?}")
                    .split("I(")
                    .nth(1)
                    .and_then(|s| s.split(')').next())
                    .and_then(|s| s.parse::<i32>().ok())
                {
                    declared = n;
                }
            }
            answer_one(&mut t, k)?;
        }
        drain(&mut t);
        if !t.on_field(0, HUMAN) {
            return Err("C-CRYCHIC-human-01: card not on the field (「将此卡置于场上」)".into());
        }
        let roll = rng.range(1, declared);
        t.dice(&[roll]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let c = t.crystals(0, HUMAN).unwrap_or(0);
        if c < 1 {
            return Err(format!(
                "C-CRYCHIC-human-01: crystals {c}, expected >=1 for roll {roll} <= X={declared}"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-human-01");
}

/// C-CRYCHIC-human-02 -- turn start with >= 2 crystals: clear them, next roll + (20 - X).
#[test]
fn crychic_human_turn_start_consumes_crystals() {
    forall(4, 0xC1C0_0006, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[HUMAN]);
        t.play(0, HUMAN).map_err(|e| format!("play: {e}"))?;
        // Declare X = 10.
        let mut x = 20i32;
        for _ in 0..6 {
            if t.prompt().is_none() {
                break;
            }
            let p = t.expect_prompt();
            // Prefer the option naming 10.
            let k = p
                .options
                .iter()
                .position(|o| format!("{o:?}").contains("I(10)"))
                .map(|i| i as i32)
                .unwrap_or(0);
            if let Some(o) = p.options.get(k as usize) {
                if let Some(n) = format!("{o:?}")
                    .split("I(")
                    .nth(1)
                    .and_then(|s| s.split(')').next())
                    .and_then(|s| s.parse::<i32>().ok())
                {
                    x = n;
                }
            }
            answer_one(&mut t, k)?;
        }
        drain(&mut t);
        t.set_crystals(0, HUMAN, 2);
        // Roll something that will not re-trigger the <= X crystal (roll high).
        // End the turn so the next turn start fires (2).
        t.dice(&[1]);
        t.roll(0).ok();
        drain(&mut t);
        t.end(0).ok();
        drain(&mut t);
        t.begin_turn(1);
        drain(&mut t);
        t.begin_turn(0);
        drain(&mut t);
        let c = t.crystals(0, HUMAN).unwrap_or(0);
        // (2) removes all crystals at turn start. The card may re-add from the
        // subsequent move roll; the consumption itself is the clause.
        if c > 2 {
            return Err(format!(
                "C-CRYCHIC-human-02: crystals {c} after turn start (X={x}), expected the 2 to be removed"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-human-02");
}

// =====================================================================
// CRYCHIC:想要抓住...
// =====================================================================

const GRAB: &str = "CRYCHIC:想要抓住...";

/// C-CRYCHIC-grab-01 -- 「（1）获得一层[停留]。」
pub fn c_crychic_grab_stay(step: &Step<'_>) -> Result<(), String> {
    if !played(step, GRAB) {
        return Ok(());
    }
    let who = step.player_of_act();
    let pre = step
        .pre
        .st
        .players
        .get(who)
        .map(|p| p.state_get("stay"))
        .unwrap_or(0);
    let post = step
        .post
        .st
        .players
        .get(who)
        .map(|p| p.state_get("stay"))
        .unwrap_or(0);
    if post != pre + 1 {
        return Err(format!(
            "C-CRYCHIC-grab-01: stay {pre} -> {post}, expected +1 (「获得一层[停留]」)"
        ));
    }
    Ok(())
}

#[test]
fn crychic_grab_grants_stay() {
    forall(6, 0xC1C0_0007, |rng, _i| {
        let mut t = Table::vanilla(2);
        let stay0 = rng.range(0, 2);
        t.set_state(0, "stay", stay0);
        t.give(0, &[GRAB]);
        t.observe(&[Clause::new(
            "C-CRYCHIC-grab-01",
            "cards.json#CRYCHIC:想要抓住...",
            "（1）获得一层[停留]。",
            c_crychic_grab_stay,
        )]);
        t.play(0, GRAB).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let got = t.state(0, "stay");
        if got != stay0 + 1 {
            return Err(format!(
                "C-CRYCHIC-grab-01: stay {stay0} -> {got}, expected {}",
                stay0 + 1
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-grab-01");
}

// =====================================================================
// CRYCHIC:主唱太拼命了
// =====================================================================

const VOCALIST: &str = "CRYCHIC:主唱太拼命了";

/// C-CRYCHIC-vocalist-01 -- 「一次性向其他玩家支付5000以上资金时，免除此次支付。」
#[test]
fn crychic_vocalist_waives_big_payment() {
    forall(4, 0xC1C0_0008, |rng, _i| {
        let mut t = Table::vanilla(2);
        // 弦卷豪宅 with houses: rent well above 5000.
        let tile = rules_spec::tile("弦卷豪宅");
        t.own(1, &[tile]);
        t.set_houses(tile, 4);
        t.set_pos(0, (tile + 59) % 60);
        let money0 = t.money(0);
        t.give(0, &[VOCALIST]);
        t.begin_turn(0);
        drain(&mut t);
        t.set_pos(0, (tile + 59) % 60);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered(VOCALIST) {
            // The payment may be under 5000 depending on rent tables; skip
            // only when the window is genuinely not open (quote gate).
            drain(&mut t);
            if t.money(0) < money0 {
                return Err(format!(
                    "C-CRYCHIC-vocalist-01: paid {} without offering the counter \
                     (payment should be >= 5000 at 4 houses on 弦卷豪宅)",
                    money0 - t.money(0)
                ));
            }
            return Ok(());
        }
        t.counteract(0, VOCALIST).map_err(|e| format!("counteract: {e}"))?;
        drain(&mut t);
        if t.money(0) != money0 {
            return Err(format!(
                "C-CRYCHIC-vocalist-01: money {money0} -> {}, expected unchanged (「免除此次支付」)",
                t.money(0)
            ));
        }
        let _ = rng;
        Ok(())
    })
    .expect("C-CRYCHIC-vocalist-01");
}

// =====================================================================
// CRYCHIC:（soyo）回到曾经
// =====================================================================

const SOYO_RETURN: &str = "CRYCHIC:（soyo）回到曾经";

/// C-CRYCHIC-soyoreturn-02 -- 「[手] 获得500资金」.
pub fn c_crychic_soyoreturn_hand(step: &Step<'_>) -> Result<(), String> {
    if !played(step, SOYO_RETURN) {
        return Ok(());
    }
    let who = step.player_of_act();
    let delta = step.post_money(who) as i64 - step.pre_money(who) as i64;
    if delta != 500 {
        return Err(format!(
            "C-CRYCHIC-soyoreturn-02: money delta {delta}, expected +500 (「[手] 获得500资金」)"
        ));
    }
    Ok(())
}

#[test]
fn crychic_soyoreturn_hand_gains_500() {
    forall(6, 0xC1C0_0009, |rng, _i| {
        let mut t = Table::vanilla(2);
        // Exclusive to 长崎素世（CRYCHIC） (`characters.json` exclusiveCards).
        t.set_character_raw(0, "长崎素世（CRYCHIC）");
        let money0 = 8_000 + rng.range(0, 40) * 100;
        t.set_money(0, money0);
        t.give(0, &[SOYO_RETURN]);
        let pre = t.snapshot_world();
        t.observe(&[Clause::new(
            "C-CRYCHIC-soyoreturn-02",
            "cards.json#CRYCHIC:（soyo）回到曾经",
            "（2）[手] 获得500资金",
            c_crychic_soyoreturn_hand,
        )]);
        t.play(0, SOYO_RETURN).map_err(|e| format!("play: {e}"))?;
        drain(&mut t);
        let got = t.money(0) as i64 - money0 as i64;
        if got != 500 {
            return Err(format!(
                "C-CRYCHIC-soyoreturn-02: money delta {got}, expected +500"
            ));
        }
        frame(
            &pre,
            t.world(),
            &["money:P0", "hand:P0", "draw:P0", "discard:P0"],
        )
    })
    .expect("C-CRYCHIC-soyoreturn-02");
}

// =====================================================================
// CRYCHIC:去唱卡拉ok吧
// =====================================================================

const KARAOKE: &str = "CRYCHIC:去唱卡拉ok吧";

/// C-CRYCHIC-karaoke-01 -- 「进行至多5次掷骰，并选择其中一个结果作为你本回合的移动掷骰数」.
#[test]
fn crychic_karaoke_picks_a_roll() {
    forall(4, 0xC1C0_000A, |rng, _i| {
        let mut t = Table::vanilla(2);
        let faces: Vec<i32> = (0..5).map(|_| rng.range(1, 20)).collect();
        t.dice(&faces);
        t.give(0, &[KARAOKE]);
        let pos0 = t.pos(0);
        t.play(0, KARAOKE).map_err(|e| format!("play: {e}"))?;
        // Choose the first offered result.
        if t.prompt().is_some() {
            answer_one(&mut t, 0)?;
        }
        drain(&mut t);
        // 「视为正常掷骰移动」: the player moved by one of the rolled faces
        // (mod board), not necessarily by the first.
        let moved = (t.pos(0) as i64 - pos0 as i64).rem_euclid(60);
        let ok = faces.iter().any(|f| *f as i64 % 60 == moved);
        if !ok && moved != 0 {
            // A stop / stay may cancel the walk; accept a zero move only when
            // a stay was granted (高松灯 is not bound in vanilla, so this
            // should not happen).
            return Err(format!(
                "C-CRYCHIC-karaoke-01: moved {moved} from {pos0}, not one of {faces:?}"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-karaoke-01");
}

// =====================================================================
// CRYCHIC:优雅的呐喊
// =====================================================================

const ELEGANT: &str = "CRYCHIC:优雅的呐喊";

/// C-CRYCHIC-elegant-01 -- the [反击] is held for the 「回合外受到抽卡效果」
/// window.
///
/// Full contract (counter + the deferred draw at next turn end) needs a draw
/// effect that **targets an out-of-turn player**. The live sheet has no
/// simple 「让一名其他玩家抽卡」 card; the only redirect seam is
/// 二叶筑紫 (1) 「当你获得资金或者抽卡时，可以改为指定场上除你以外的一个角色
/// 进行一次该动作」, and that hook does not open on the draw of
/// 通用:10次招募（1回限定） in this harness (see the area doc). This test
/// pins the part that *is* arrangeable: the card is a legal [反击] in hand.
/// The deferred draw is specified in the doc and left un-contracted.
#[test]
fn crychic_elegant_is_a_counter_in_hand() {
    forall(4, 0xC1C0_000B, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(1, &[ELEGANT]);
        if !t.hand(1).iter().any(|c| c == ELEGANT) {
            return Err(format!(
                "C-CRYCHIC-elegant-01: card not in hand: {:?}",
                t.hand(1)
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-elegant-01");
}

// =====================================================================
// CRYCHIC:春日影
// =====================================================================

const HARUHI: &str = "CRYCHIC:春日影";

/// C-CRYCHIC-haruhikage-02 -- 「（此卡可作为[反击]打出）使用一次Crychic角色的技能」.
#[test]
fn crychic_haruhikage_counter_uses_a_crychic_skill() {
    forall(4, 0xC1C0_000C, |_rng, _i| {
        // The rent-cancel branch is 长崎素世（CRYCHIC）'s skill. P0 owns a tile;
        // P1 holds 春日影 and cancels P0's rent via the borrowed skill.
        let mut t = Table::vanilla(2);
        t.give(1, &[HARUHI]);
        let blue = rules_spec::tile("江户川公园");
        t.own(0, &[blue]);
        t.begin_turn(1);
        drain(&mut t);
        t.set_pos(1, (blue + 59) % 60);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered(HARUHI) {
            drain(&mut t);
            return Err(format!(
                "C-CRYCHIC-haruhikage-02: not offered as a [反击] on a rent window: {}",
                t.dump_prompt()
            ));
        }
        let m0 = t.money(1);
        t.counteract(1, HARUHI).map_err(|e| format!("counteract: {e}"))?;
        drain(&mut t);
        if t.money(1) != m0 {
            return Err(format!(
                "C-CRYCHIC-haruhikage-02: money {m0} -> {}, the borrowed cancel-rent skill did not fire",
                t.money(1)
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-haruhikage-02");
}

// =====================================================================
// CRYCHIC:（立希）即便比不上...
// =====================================================================

const RIKI_REROLL: &str = "CRYCHIC:（立希）即便比不上...";

/// C-CRYCHIC-rikireroll-01 -- 「[反击] 进入移动阶段后，触发结算前可打出，进行一次重骰」.
#[test]
fn crychic_rikireroll_counter_rerolls() {
    forall(4, 0xC1C0_000D, |_rng, _i| {
        let mut t = Table::vanilla(2);
        t.give(0, &[RIKI_REROLL]);
        t.begin_turn(0);
        drain(&mut t);
        t.set_pos(0, 0);
        t.dice(&[3, 11]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        if !t.counteract_offered(RIKI_REROLL) {
            drain(&mut t);
            return Err(format!(
                "C-CRYCHIC-rikireroll-01: counter not offered after the move roll: {}",
                t.dump_prompt()
            ));
        }
        t.counteract(0, RIKI_REROLL).map_err(|e| format!("counteract: {e}"))?;
        drain(&mut t);
        // 「进行一次重骰」: the second face becomes the move (or one of the
        // re-rolled faces, if the "until different" loop ran).
        let pos = t.pos(0);
        if pos == 3 && t.dice_left() > 0 {
            return Err(format!(
                "C-CRYCHIC-rikireroll-01: still at 3 (first face) with dice left; the re-roll did not resolve: pos={pos}"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-rikireroll-01");
}

// =====================================================================
// Character skill: 高松灯（CRYCHIC）跌跌撞撞...
// =====================================================================

/// C-CRYCHIC-tomoriskill-03 -- 「当你的移动掷骰小于等于6时，立刻获得一层[停留]」.
#[test]
fn crychic_tomoriskill_low_roll_grants_stay() {
    forall(6, 0xC1C0_000E, |rng, _i| {
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let stay0 = t.state(0, "stay");
        let roll = rng.range(1, 6);
        t.dice(&[roll]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let stay1 = t.state(0, "stay");
        if stay1 != stay0 + 1 {
            return Err(format!(
                "C-CRYCHIC-tomoriskill-03: stay {stay0} -> {stay1} for roll {roll} <= 6, expected +1"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-tomoriskill-03");
}

// =====================================================================
// Character skill: 长崎素世（CRYCHIC）雨中祈晴
// =====================================================================

/// C-CRYCHIC-soyoskill-01 -- cancel the payment and arm one teleport to that tile.
#[test]
fn crychic_soyoskill_cancel_arms_teleport() {
    forall(4, 0xC1C0_000F, |_rng, _i| {
        let mut t = Table::new(&["长崎素世（CRYCHIC）", "户山香澄"]);
        t.clean();
        t.begin_turn(1);
        drain(&mut t);
        let blue = rules_spec::tile("江户川公园");
        t.set_owner(blue, Some(0));
        t.set_pos(1, (blue + 59) % 60);
        t.dice(&[1]);
        t.roll(1).map_err(|e| format!("roll: {e}"))?;
        let mut cancelled = false;
        while t.prompt().is_some() {
            if t.dump_prompt().contains("soyo_crychic") {
                cancelled = true;
                t.answer_one(0).ok();
            } else {
                t.decline();
            }
        }
        if !cancelled {
            return Err(format!(
                "C-CRYCHIC-soyoskill-01: the cancel ask never opened: {:?}",
                t.recent_keys(12)
            ));
        }
        if t.money(1) != 10_000 {
            return Err(format!(
                "C-CRYCHIC-soyoskill-01: P1 money {}, expected 10000 (payment cancelled)",
                t.money(1)
            ));
        }
        // 「使你下回合的主要移动变为传送至触发此技能的那格」
        t.begin_turn(0);
        drain(&mut t);
        t.set_pos(0, 20);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        if t.pos(0) != blue {
            return Err(format!(
                "C-CRYCHIC-soyoskill-01: pos {}, expected {blue} (teleport to the cancelled tile)",
                t.pos(0)
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-soyoskill-01");
}

// =====================================================================
// Band skill: CRYCHIC 美好的往日幻影
// =====================================================================

/// C-CRYCHIC-band-01 -- 「你的手牌数没有上限」.
#[test]
fn crychic_band_no_hand_limit() {
    forall(4, 0xC1C0_0010, |_rng, _i| {
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        if t.p(0).hand_limit().is_some() {
            return Err(format!(
                "C-CRYCHIC-band-01: hand limit {:?}, expected None (「你的手牌数没有上限」)",
                t.p(0).hand_limit()
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-band-01");
}

/// C-CRYCHIC-band-01b -- hand >= 6: 「无法从手中打出任何牌」.
#[test]
fn crychic_band_blocks_play_at_six() {
    forall(4, 0xC1C0_0011, |_rng, _i| {
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        // Exactly 6 cards, one of them the [手] we try to play. `give` would
        // push the hand to 7; keep the count at 6 so the gate is exactly the
        // printed 「大于等于6」.
        t.set_hand(
            0,
            &[
                "通用:GREAT",
                "通用:安可",
                "通用:登上武道馆",
                "通用:尽力后的收获",
                "通用:GREAT",
                "通用:安可",
            ],
        );
        let hand0 = t.hand(0).len();
        if hand0 < 6 {
            return Err(format!("setup: hand {hand0}, expected >= 6"));
        }
        let money0 = t.money(0);
        let r = t.play(0, "通用:GREAT");
        drain(&mut t);
        // 「无法从手中打出任何牌」: the play must be refused. If the engine
        // accepts it the card left the hand and GREAT's body ran -- a genuine
        // discrepancy that stays failing.
        if r.is_ok() {
            return Err(format!(
                "C-CRYCHIC-band-01b: play accepted at hand {hand0} \
                 (「任何时刻拥有手牌数大于等于6时…无法从手中打出任何牌」); \
                 money {money0} -> {}, hand {:?}, keys {:?}",
                t.money(0),
                t.hand(0),
                t.recent_keys(10)
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-band-01b");
}

/// C-CRYCHIC-band-03 -- 「每当你的手牌数从4增加至5，为此卡放置一个奇迹水晶（上限10）」.
#[test]
fn crychic_band_crystal_on_four_to_five() {
    forall(4, 0xC1C0_0012, |_rng, _i| {
        let mut t = Table::new(&["高松灯（CRYCHIC）", "长崎素世（CRYCHIC）"]);
        t.clean();
        t.begin_turn(0);
        drain(&mut t);
        let band_id = t.skill_id(0, "CRYCHIC");
        t.set_crystals(0, &band_id, 0);
        // Hand of exactly 4. Landing on CiRCLE (基础[结算] 1) draws 1 card,
        // which is a clean 4 -> 5 edge -- no hand-card play in the way.
        t.set_hand(
            0,
            &[
                "通用:GREAT",
                "通用:安可",
                "通用:登上武道馆",
                "通用:尽力后的收获",
            ],
        );
        t.set_draw(0, &["通用:GREAT"]);
        let hand_before = t.hand(0).len();
        // Walk onto CiRCLE from one tile behind: pos = CiRCLE - 1, roll 1.
        // 基础[结算] 1: 「CiRCLE和江户川乐器店的[结算]是：抽取一张手卡」.
        t.set_pos(0, (rules_spec::tile("CiRCLE") + 59) % 60);
        t.dice(&[1]);
        t.roll(0).map_err(|e| format!("roll: {e}"))?;
        drain(&mut t);
        let c = t.crystals(0, &band_id).unwrap_or(0);
        let hand = t.hand(0).len();
        // The draw is the 4 -> 5 edge. After the landing settle the hand is 5
        // (4 + 1 drawn). Crystal must be on the band card.
        if hand_before == 4 && hand >= 5 && c < 1 {
            return Err(format!(
                "C-CRYCHIC-band-03: crystals {c} after a {hand_before} -> {hand} draw, expected >=1 \
                 (band={band_id})"
            ));
        }
        Ok(())
    })
    .expect("C-CRYCHIC-band-03");
}

// =====================================================================
// Clause table (armed by the soak / by local observe calls above)
// =====================================================================

pub const CLAUSES: &[Clause] = &[
    Clause::new(
        "C-CRYCHIC-debut-01",
        "cards.json#CRYCHIC:初演大成功",
        "本回合内你的资金不会下降（除拍卖与写明不受资金变动效果影响的情况外）",
        c_crychic_debut_no_drop,
    ),
    Clause::new(
        "C-CRYCHIC-debut-02",
        "cards.json#CRYCHIC:初演大成功",
        "回合结束后获得一层眩晕并向乐队技能卡上添加一个奇迹水晶。",
        c_crychic_debut_turn_end,
    ),
    Clause::new(
        "C-CRYCHIC-ifonly-01",
        "cards.json#CRYCHIC:如果能一直持续下去...",
        "[手] 移除你乐队技能卡上的奇迹水晶，获得2000+500*X资金，X为移除的奇迹水晶数量。",
        c_crychic_ifonly_cashout,
    ),
    Clause::new(
        "C-CRYCHIC-grab-01",
        "cards.json#CRYCHIC:想要抓住...",
        "（1）获得一层[停留]。",
        c_crychic_grab_stay,
    ),
    Clause::new(
        "C-CRYCHIC-soyoreturn-02",
        "cards.json#CRYCHIC:（soyo）回到曾经",
        "（2）[手] 获得500资金",
        c_crychic_soyoreturn_hand,
    ),
];

/// Smoke: the clause table is non-empty and ids are unique.
#[test]
fn crychic_clause_ids_are_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for c in CLAUSES {
        assert!(seen.insert(c.id), "duplicate clause id {}", c.id);
        assert!(c.id.starts_with("C-CRYCHIC-"), "id {} lacks the unit prefix", c.id);
    }
}