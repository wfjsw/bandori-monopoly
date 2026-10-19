//! Marker-counter design pins (docs/MARKERS.md).
//!
//! | pin | where |
//! |---|---|
//! | fire numbers live on the skill `CardDef` (no body literals) | here |
//! | two_in_one swap matches the two faces' `CardDef` numbers | here |
//! | I2 「在任意时刻」 value <= max | here |
//! | kernel actuals (two_donuts overflow shape) | here |
//! | fans once-flag + multiplicity | here |
//! | regime protocol (fill / drain / burn) | here |
//!
//! Naming: `<slug>_<what>`. Each assertion block carries the clause it checks.

mod common;
use common::*;

/// Face-up / face-down fan counters (frozen wire keys).
fn fans(t: &Table, who: usize) -> (i32, i32) {
    (t.state(who, "ppFansUp"), t.state(who, "ppFansDown"))
}

// =====================================================================
// fire declaration lives on the skill CardDef
// =====================================================================

/// MARKERS.md §7: the face parenthetical's numbers live on that face's
/// `CardDef` (`.props(&[(fire::FIRE_INITIAL, N), (fire::FIRE_CAP, M)])`). Skill
/// bodies call `fire_pot(player)` -- they must not carry literals. Raw
/// CHARGE / tempFire writes are kernel-only.
#[test]
fn fire_numbers_are_on_the_skill_def() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../skills/skill-personas/src");
    let mut offenders = Vec::new();
    let mut declared = 0usize;
    for entry in std::fs::read_dir(&dir).expect("skill-personas/src") {
        let path = entry.expect("dirent").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("read skill body");
        // A face that prints 「（初始N，上限M）」 declares it on its CardDef.
        if src.contains(".fire_pots(") || src.contains("FIRE_INITIAL") {
            declared += 1;
        }
        for (i, line) in src.lines().enumerate() {
            // `fire_pot(player_id, N, M)` -- the old literal form.
            if line.contains("fire_pot(player_id") {
                let after = line.split("fire_pot(player_id").nth(1).unwrap_or("");
                if after.trim_start().starts_with(',') {
                    offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
            // Raw CHARGE writes are kernel-only (MARKERS.md §6 / §9).
            if line.contains("state::set_bounds") && line.contains("CHARGE") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
            if line.contains("state::set(") && line.contains("state_key::CHARGE") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
            if line.contains("tempFire") {
                offenders.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "stray fire literals / raw CHARGE / tempFire writes in skill bodies:\n{}",
        offenders.join("\n")
    );
    // The 35 faces that print the parenthetical each own their numbers.
    assert!(
        declared >= 35,
        "fire-pot faces must declare fire::FIRE_INITIAL/FIRE_CAP on their CardDef; found {declared}"
    );
}

/// two_in_one's swap writes the other face's 「初始火罐数」 from that face's
/// `CardDef` (hina 2/2 ↔ mana 1/2). Each play is one copy; give one per
/// direction.
#[test]
fn two_in_one_matches_the_pair_defs() {
    let mut t = Table::vanilla(2);
    t.set_character_raw(0, "三角初华（Sumimi）");
    t.set_fire(0, 0, 2);
    t.give(0, &["Sumimi:Sumimi是二人一体的"]);
    t.play(0, "Sumimi:Sumimi是二人一体的").expect("swap");
    assert_eq!(t.charge(0), 1, "mana 初始1");
    assert_eq!(t.state_var(0, "charge").max, 2, "mana 上限2");
    t.give(0, &["Sumimi:Sumimi是二人一体的"]);
    t.play(0, "Sumimi:Sumimi是二人一体的").expect("swap back");
    assert_eq!(t.charge(0), 2, "hina 初始2");
    assert_eq!(t.state_var(0, "charge").max, 2, "hina 上限2");
}

// =====================================================================
// I2 -- value <= declared max 「在任意时刻」
// =====================================================================

/// A gain never lands above the cap (「不能超过持有上限」).
#[test]
fn i2_gain_never_exceeds_max() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 1, 3);
    let got = t
        .m
        .world_mut()
        .gain_marker(0, "charge", 10);
    assert_eq!(got, 2, "room under cap 3 from 1 is 2");
    assert!(t.charge(0) <= 3, "I2: {} <= 3", t.charge(0));
}

/// A cap drop pulls the value under the new ceiling (I2 「在任意时刻」).
#[test]
fn i2_cap_drop_pulls_value_under() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 5, 5);
    t.m.world_mut().add_marker_max(0, "charge", -3);
    assert!(t.charge(0) <= 2, "value pulled under the new cap of 2");
}

// =====================================================================
// kernel actuals
// =====================================================================

/// 「超出上限的每个火罐转化为500资金」 -- the kernel returns actuals so the
/// caller clause can convert the rest (`Sumimi:一人两个甜甜圈`).
#[test]
fn kernel_actuals_overflow_shape() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 1, 1);
    let before = t.p(0).money;
    let got = t
        .m
        .world_mut()
        .gain_marker(0, "charge", 2);
    let missed = 2 - got;
    assert_eq!(got, 0, "cap 1/1 refused the grant");
    assert_eq!(missed, 2, "both pots missed");
    // The caller's clause: missed * 500资金.
    t.m
        .world_mut()
        .gain_money(0, missed * 500, game_core::msg::Msg::new("two_donuts_overflow"));
    assert_eq!(t.p(0).money, before + 1000);
}

// =====================================================================
// fans once-flag + multiplicity
// =====================================================================

/// 「如有场上有多张Pastel✽Palettes乐队卡时此效果不重复发动」 -- M band-card
/// copies grant once. Two PP characters = two band-skill instances; the one
/// non-Palettes seat still receives exactly one face-down fan.
#[test]
fn fans_initial_grant_is_once_per_match() {
    // 丸山彩 / 白鹭千圣 are Pastel✽Palettes (band skill x2); 户山香澄 is MyGO.
    // PP characters may carry their own character-skill start grants (e.g.
    // 丸山彩's +5 face-up); the band clause （1） only deals **face-down** fans
    // to non-Palettes seats. Check the face-down row for the once-flag.
    let t = Table::new(&["丸山彩", "白鹭千圣", "户山香澄"]);
    assert_eq!(fans(&t, 0).1, 0, "PP character is excluded 「非Pastel✽Palettes角色」");
    assert_eq!(fans(&t, 1).1, 0, "PP character is excluded");
    assert_eq!(
        fans(&t, 2),
        (0, 1),
        "non-Palettes seat receives exactly one face-down fan despite two band cards"
    );
}

/// The exclusion test is the **character's** group. A single band card on a
/// non-Palettes table still deals one face-down fan to each non-Palettes seat.
#[test]
fn fans_exclusion_is_character_group() {
    let t = Table::new(&["丸山彩", "户山香澄", "纯田真奈"]);
    // 丸山彩's own character skill grants +5 face-up at game start; the band
    // clause （1） gives 1 face-down to non-Palettes seats (exclusion is the
    // character's group, not the band card).
    assert_eq!(fans(&t, 0), (5, 0), "PP persona skill's own +5 start grant");
    assert_eq!(fans(&t, 1), (0, 1), "MyGO seat receives the reverse fan");
    assert_eq!(fans(&t, 2), (0, 1), "Sumimi seat receives the reverse fan");
}

// =====================================================================
// regime protocol (Ave Mujica)
// =====================================================================

/// 「进入状态2时将火罐补充至上限」 / 「每回合结束时失去1火罐」 / 「退出状态2时，
/// 失去所有剩余火罐」 -- named regime verbs over the owner's pool. The fill
/// is the engine seam `gain_charge` (what `fire::regime::fill_to_cap` calls);
/// drain/burn ride `spend_charge` (the host spend verb) and are covered by the
/// existing spend pins. This checks the fill shape and the I2 pull-under.
#[test]
fn fire_regime_fill_drain_burn() {
    let mut t = Table::vanilla(2);
    t.set_fire(0, 1, 4);
    // fill_to_cap: 1 -> 4.
    let got = t
        .m
        .world_mut()
        .gain_marker(0, "charge", 3);
    assert_eq!(got, 3, "fill_to_cap lands the room under the cap");
    assert_eq!(t.charge(0), 4);
    // drain 1 / burn_all: the spend path is `fire::regime::{drain,burn_all}` →
    // `spend_charge`. Pin the storage fold here (a dumb value op).
    t.m.world_mut().st.players[0].state_add("charge", -1);
    assert_eq!(t.charge(0), 3, "drain 1");
    t.set_fire(0, 0, 4);
    assert_eq!(t.charge(0), 0, "burn_all");
}