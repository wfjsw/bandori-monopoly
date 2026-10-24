//! Fill-key drift (brief 9.1) and install smoke (brief 9.3).
//!
//! `state_table()` must contain every [`bandori_std::cel::STATE_KEYS`] row,
//! and the first real sugar guard must compile after `install_vocab()`.

use std::collections::BTreeMap;

use game_rules::cond_pre;
use rules_cond::view::CondView;
use rules_cond::{CandidateCtx, PlayerSnap, SnapshotView, WindowCtx};

fn win_with_sugar() -> WindowCtx {
    let mut p0 = PlayerSnap {
        money: 1200,
        ..PlayerSnap::default()
    };
    p0.state = [
        ("charge", 7i64),
        ("crystals", 3),
        ("persona", cond_pre::id_of("三角初华（Sumimi）")),
        ("group", cond_pre::id_of("Pastel✽Palettes")),
        ("noHand", 1),
        ("stay", 2),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    WindowCtx {
        players: vec![p0, PlayerSnap::default()],
        actor: 0,
        ..WindowCtx::default()
    }
}

#[test]
fn state_table_carries_every_sugar_key() {
    let win = win_with_sugar();
    let cand = CandidateCtx::default();
    let view = SnapshotView {
        win: &win,
        cand: &cand,
    };
    let rows = view.state_table();
    let mut seen: BTreeMap<&str, i64> = BTreeMap::new();
    for (seat, key, v) in &rows {
        if *seat == 0 {
            seen.insert(key.as_str(), *v);
        }
    }
    for key in bandori_std::cel::STATE_KEYS {
        assert!(
            seen.contains_key(key),
            "state_table() missing sugar key {key:?}: {seen:?}"
        );
    }
    assert_eq!(seen[bandori_std::cel::KEY_CHARGE], 7);
    assert_eq!(seen[bandori_std::cel::KEY_CRYSTALS], 3);
    assert_eq!(
        seen[bandori_std::cel::KEY_PERSONA],
        cond_pre::id_of("三角初华（Sumimi）")
    );
    assert_eq!(
        seen[bandori_std::cel::KEY_GROUP],
        cond_pre::id_of("Pastel✽Palettes")
    );
    assert_eq!(seen[bandori_std::cel::KEY_NO_HAND], 1);
    // `state()` agrees with the table (one door).
    for key in bandori_std::cel::STATE_KEYS {
        assert_eq!(view.state(0, key), seen[key], "state() vs state_table() {key}");
    }
}

#[test]
fn install_vocab_then_first_sugar_guard_compiles() {
    cond_pre::install_vocab();
    // A real production spelling. Must compile after install.
    let c = rules_cond::compile("charge(owner) > 0").expect("sugar guard compiles");
    let win = win_with_sugar();
    let cand = CandidateCtx {
        owner: 0,
        ..Default::default()
    };
    assert!(c.eval(&win, &cand), "charge(owner=0) = 7 > 0");
}

#[test]
fn install_vocab_then_owner_sugar_compiles() {
    cond_pre::install_vocab();
    let c = rules_cond::compile("owner.charge == 7 && owner.group > 0").expect("owner sugar");
    let win = win_with_sugar();
    let cand = CandidateCtx {
        owner: 0,
        ..Default::default()
    };
    assert!(c.eval(&win, &cand));
}

#[test]
fn persona_group_name_spellings_hash() {
    // Brief 9.2: the host publishes `id_of(persona_name())` /
    // `id_of(group_name())` — the exact spelling the CallEq sugar compares.
    cond_pre::install_vocab();
    for (name, key, cel) in [
        (
            "三角初华（Sumimi）",
            bandori_std::cel::KEY_PERSONA,
            "persona_is(0, '三角初华（Sumimi）')",
        ),
        (
            "纯田真奈",
            bandori_std::cel::KEY_PERSONA,
            "persona_is(0, '纯田真奈')",
        ),
        (
            "Pastel✽Palettes",
            bandori_std::cel::KEY_GROUP,
            "group_is(0, 'Pastel✽Palettes')",
        ),
        ("CRYCHIC", bandori_std::cel::KEY_GROUP, "group_is(0, 'CRYCHIC')"),
    ] {
        let mut win = win_with_sugar();
        win.players[0]
            .state
            .insert(key.to_string(), cond_pre::id_of(name));
        let cand = CandidateCtx {
            owner: 0,
            ..Default::default()
        };
        let view = SnapshotView {
            win: &win,
            cand: &cand,
        };
        assert_eq!(
            view.state(0, key),
            cond_pre::id_of(name),
            "state(0, {key}) must be id_of({name:?})"
        );
        let c = rules_cond::compile(cel).unwrap();
        assert!(c.eval(&win, &cand), "{cel}");
    }
}