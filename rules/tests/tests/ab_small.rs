//! Small A-class rulebook fixes: stage markers, an atomic re-link, the
//! thanks-party offer, the shared crystals guard, and the fire save fold.
//!
//! * HOST-03 (行动阶段 3 / 6): `turnStartAfter` fires once after the
//!   `turnStart` handling; `manageAfter` fires once when 运营 ends, and not on
//!   a 眩晕-skipped turn.
//! * LOY-ANON-01: a capped re-link pays nothing (place-or-refuse stays atomic).
//! * LOY-THANKS-01: a pay-gated (stunned) seat still gets the 「所有其他玩家」 offer.
//! * CD-02: the folded `crystals_changed_of` guard still fires the j11 drain.
//! * FIRE-TOKENS-1: a v22 save's `state["charge"]` / `["tempFire"]` fold into
//!   held-token rows bound to the persona-skill instance.

mod common;
use common::*;

use std::collections::BTreeMap;

use game_core::msg::Msg;
use game_core::state::StateVar;

/// Decline every open prompt.
fn drain(t: &mut Table) {
    for _ in 0..40 {
        if t.prompt().is_none() {
            return;
        }
        t.decline();
    }
    panic!("drain: prompts never stopped: {}", t.dump_prompt());
}

/// Count occurrences of `needle` in `keys` (fixture keys carry a module
/// namespace, so match by substring).
fn count(keys: &[String], needle: &str) -> usize {
    keys.iter().filter(|k| k.contains(needle)).count()
}

// =====================================================================
// HOST-03 -- 行动阶段 3 「回合开始后」
// =====================================================================

/// 行动阶段 (data/rules.txt): 3 「回合开始后」 comes after 2 「回合开始」.
/// The probe logs the two kinds with distinct keys; the marker fires once and
/// after the window, and not again at the move / end / another seat's start.
#[test]
fn turn_start_after_fires_once_after_the_start_window() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "TEST:turnStartAfterProbe");
    let mark = t.mark();
    t.begin_turn(0);
    let keys = t.keys_since(mark);
    let window = keys
        .iter()
        .position(|k| k.contains("turn_start_window"))
        .unwrap_or_else(|| panic!("no turnStart window line: {keys:?}"));
    let after = keys
        .iter()
        .position(|k| k.contains("turn_start_after"))
        .unwrap_or_else(|| panic!("no turnStartAfter line: {keys:?}"));
    assert!(window < after, "the marker follows the window: {keys:?}");
    assert_eq!(count(&keys, "turn_start_after"), 1, "{keys:?}");

    // Not at the move, the settle or the end.
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert_eq!(count(&keys, "turn_start_after"), 1, "{keys:?}");

    // Seat 1's turn start does not fire the seat-0 owner hook; ending it
    // starts seat 0's next turn, which fires it once more.
    t.dice(&[1]);
    t.roll(1).unwrap();
    drain(&mut t);
    t.end(1).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert_eq!(count(&keys, "turn_start_after"), 2, "{keys:?}");
}

// =====================================================================
// HOST-03 -- 行动阶段 6 「经营/出牌阶段后」
// =====================================================================

/// 行动阶段 6 sits between 5 (经营/出牌) and 7 (主要移动阶段前): taking the
/// main move raises it once, before the move.
#[test]
fn manage_after_fires_once_before_the_main_move() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "TEST:manageAfterProbe");
    let mark = t.mark();
    // 运营 is still open.
    assert_eq!(count(&t.keys_since(mark), "manage_after"), 0);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert_eq!(count(&keys, "manage_after"), 1, "{keys:?}");
}

/// Ending the turn straight from 运营 closes the manage phase too: 6 comes
/// before the end phase. Only a 停留 turn may end without rolling (the move
/// gate is closed), and it still opens 运营.
#[test]
fn manage_after_fires_on_an_end_turn_from_the_manage_phase() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "TEST:manageAfterProbe");
    t.set_state(0, bandori_std::state_key::STAY, 1);
    t.begin_turn(0);
    let mark = t.mark();
    t.end(0).unwrap();
    drain(&mut t);
    let keys = t.keys_since(mark);
    assert_eq!(count(&keys, "manage_after"), 1, "{keys:?}");
}

/// 眩晕 (stage 4) skips 经营 and the main move, so stages 5 and 6 never open.
#[test]
fn manage_after_skips_a_stunned_turn() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, "TEST:manageAfterProbe");
    t.set_state(0, "stun", 1);
    let mark = t.mark();
    t.begin_turn(0);
    let keys = t.keys_since(mark);
    assert_eq!(count(&keys, "manage_after"), 0, "{keys:?}");
}

// =====================================================================
// LOY-ANON-01 -- the half runs only when the pair places
// =====================================================================

const ANON_TOKYO: &str = "MyGO:[千早爱音]Anon Tokyo";

/// 规则书: 「[消耗]…并在上述的两个格子间放置1个[奇迹水晶]（上限1）」. The
/// pair is place-or-refuse atomic; a second link on the same pair places
/// nothing, so it must not spend the half either.
#[test]
fn anon_tokyo_capped_relink_pays_nothing() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "千早爱音");
    let spot = tile("购物中心");
    let other = tile("天文馆");
    t.set_pos(0, spot);
    t.own(0, &[spot, other]);
    t.set_money(0, 20_000);
    t.give_play(0, ANON_TOKYO).unwrap();
    t.answer_tile(0, other).unwrap();
    drain(&mut t);
    assert!(
        !t.marks_on(spot).is_empty() && !t.marks_on(other).is_empty(),
        "the first link landed"
    );
    let money = t.money(0);

    // A second copy on the same pair: the cap refuses, nothing is paid.
    t.set_pos(0, spot);
    t.give(0, &[ANON_TOKYO]);
    let mark = t.mark();
    t.play(0, ANON_TOKYO).unwrap();
    t.answer_tile(0, other).unwrap();
    drain(&mut t);
    assert_eq!(t.money(0), money, "a refused re-link costs nothing");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().all(|k| !k.contains("anon_tokyo_pay")),
        "no half spent: {keys:?}"
    );
}

// =====================================================================
// LOY-THANKS-01 -- every other player gets the offer
// =====================================================================

const THANKS_PARTY: &str = "通用:CiRCLE THANKS PARTY!";

/// 规则书: 「所有其他玩家可选择[消耗]500资金」 -- no payability filter. A
/// [晕眩] seat is still asked; accepting spends nothing (its pay gate
/// refuses) and it does not count toward X.
#[test]
fn thanks_party_offers_a_stunned_seat() {
    let mut t = Table::vanilla(2);
    t.set_state(1, "stun", 1);
    t.give(0, &[THANKS_PARTY]);
    let mark = t.mark();
    t.play(0, THANKS_PARTY).unwrap();
    let mut saw_offer = false;
    for _ in 0..40 {
        let Some(p) = t.prompt() else { break };
        if format!("{:?}", p.title).contains("thanks") {
            saw_offer = true;
            let _ = t.answer_one(1); // accept the 500 chip-in
        } else {
            t.decline();
        }
    }
    assert!(saw_offer, "the pay-gated seat is still offered");
    assert_eq!(t.money(1), 10_000, "the stunned seat paid nothing");
    let keys = t.keys_since(mark);
    assert!(
        keys.iter().any(|k| k.contains("thanks_party_extra")),
        "X == 1: nobody counted, the user gets the extra turn: {keys:?}"
    );
}

// =====================================================================
// CD-02 -- the shared crystals guard still fires the j11 drain
// =====================================================================

const J11: &str = "Mujica:#J11";

/// 规则书: 「每回合结束时移除1个，为0时置入弃牌堆」. The last removal writes
/// the counter to 0, and the folded `crystals_changed_of` guard (with its
/// `value <= 0` / `crystals == 0` residuals) leaves the card.
#[test]
fn j11_leaves_when_the_last_crystal_is_removed() {
    let mut t = Table::vanilla(2);
    t.place_raw(0, J11);
    t.set_crystals(0, J11, 1);
    t.dice(&[1]);
    t.roll(0).unwrap();
    drain(&mut t);
    t.end(0).unwrap();
    drain(&mut t);
    assert!(
        !t.on_field(0, J11),
        "the emptied card went to the graveyard: {:?}",
        t.field_ids(0)
    );
}

// =====================================================================
// FIRE-TOKENS-1 -- a v22 save folds the legacy fire state
// =====================================================================

/// The version before this one kept 火罐 / temp pots in keyed state. A v22
/// save folds each into a token row of the persona-skill instance (value +
/// `StateVar.max`), drops the dead key, and only creates a missing row.
#[test]
fn a_v22_save_folds_legacy_fire_state_into_token_rows() {
    let mut t = Table::vanilla(2);
    t.strip_skills();
    t.clean();
    let skill = t
        .m
        .world_mut()
        .place_card(&data(), 0, "skill:测试:测火", Msg::default(), BTreeMap::new());
    assert!(skill >= 0);
    t.set_state_var(
        0,
        "charge",
        StateVar {
            value: 3,
            max: 5,
            ..Default::default()
        },
    );
    t.set_state_var(
        0,
        "tempFire",
        StateVar {
            value: 1,
            max: 2,
            ..Default::default()
        },
    );
    let mut v: serde_json::Value = serde_json::from_str(&t.m.save()).unwrap();
    v["version"] = 22.into();
    v["world"]["st"]["players"][0]["tokens"] = serde_json::json!([]);

    let restored =
        game_core::engine::Match::restore(data(), rules(), &v.to_string()).expect("v22 restores");
    let p = &restored.state().players[0];
    let charge = p
        .tokens
        .iter()
        .find(|c| c.name == "charge")
        .expect("charge row");
    assert_eq!(charge.value, 3, "the legacy value folded");
    assert_eq!(charge.max, 5, "the advertised cap folded");
    assert_eq!(charge.instance, Some(skill), "bound to the persona skill");
    assert_eq!(p.token("tempFire"), 1);
    assert_eq!(p.token_max("tempFire"), 2);
    assert_eq!(p.state_get("charge"), 0, "the dead state key is gone");

    // An existing row wins; the state key is still dropped.
    let mut v2 = v.clone();
    v2["world"]["st"]["players"][0]["tokens"] = serde_json::json!([{
        "name": "charge", "value": 9, "instance": -1, "max": 0
    }]);
    let restored =
        game_core::engine::Match::restore(data(), rules(), &v2.to_string()).expect("v22 restores");
    let p = &restored.state().players[0];
    assert_eq!(p.token("charge"), 9, "the live row is not overwritten");
    assert_eq!(p.state_get("charge"), 0);

    // No persona skill: the row still folds, unbound.
    let mut v3 = v.clone();
    v3["world"]["st"]["players"][0]["field"] = serde_json::json!([]);
    let restored =
        game_core::engine::Match::restore(data(), rules(), &v3.to_string()).expect("v22 restores");
    let p = &restored.state().players[0];
    assert_eq!(p.token("charge"), 3);
    assert_eq!(
        p.tokens.iter().find(|c| c.name == "charge").unwrap().instance,
        None
    );

    // Exactly one older version is a migration source.
    let mut v4 = v.clone();
    v4["version"] = 21.into();
    let err = game_core::engine::Match::restore(data(), rules(), &v4.to_string())
        .err()
        .expect("a v21 save is refused");
    assert_eq!(err.key(), "err.save_version");
}
