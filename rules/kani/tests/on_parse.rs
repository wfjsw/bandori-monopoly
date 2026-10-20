//! Parser unit tests + loud-miss guards for the Phase 3 typed `On` DSL.
//!
//! A scanner that matches nothing is the bug this suite exists to prevent.
//! These tests feed representative snippets through `parse_on_bodies` (one per
//! body shape) and then walk the real rules tree to assert the parser is not
//! silently empty and every registration names a declared kind.

#[path = "../on_parse.rs"]
mod on_parse;
use on_parse::*;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn shapes() -> HashMap<String, BodyShape> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rules = manifest.parent().expect("rules/");
    let mut sources = Vec::new();
    for rel in ["card-sdk/src/kinds.rs", "bandori-std/src/kind.rs"] {
        let p = rules.join(rel);
        sources.push(std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}")));
    }
    kind_shapes_from(&sources)
}

/// One registration per body shape (value, flag, gate, plan, roll, deal,
/// payadd, redirect) plus an `Action::`.
const SNIPPET: &str = r#"
const C: CardDef = CardDef::new("t", &[
    On::Filter(Filter::PaySplitMul("", None, mul_body)),
    On::Filter(Filter::PaySplitChoose("", None, choose_body)),
    On::Filter(Filter::ActGate("", None, gate_body)),
    On::Filter(Filter::DiceRollPlan("", None, plan_body)),
    On::Filter(Filter::DiceRollValue("", None, roll_body)),
    On::Filter(Filter::BuyAssign("", None, deal_body)),
    On::Filter(Filter::PaySplitAdd("", None, payadd_body)),
    On::Filter(Filter::Redirect("", None, redirect_body)),
    On::Action(Action::TurnStart("", "turn", None, on_turn_start)),
    On::Filter(bandori_std::kind::Filters::MyRewrite("", None, my_rewrite)),
]);
"#;

#[test]
fn parse_on_bodies_one_per_shape() {
    let shapes = shapes();
    let got = parse_on_bodies(SNIPPET, &shapes);
    let summary: Vec<(&str, &str, BodyShape)> = got
        .iter()
        .map(|b| {
            (
                b.variant.as_str(),
                b.kind.as_deref().unwrap_or(""),
                b.shape,
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("Filter", "PaySplitMul", BodyShape::Value),
            ("Filter", "PaySplitChoose", BodyShape::Flag),
            ("Filter", "ActGate", BodyShape::Gate),
            ("Filter", "DiceRollPlan", BodyShape::Plan),
            ("Filter", "DiceRollValue", BodyShape::Roll),
            ("Filter", "BuyAssign", BodyShape::Deal),
            ("Filter", "PaySplitAdd", BodyShape::PayAdd),
            ("Filter", "Redirect", BodyShape::Redirect),
            ("Action", "TurnStart", BodyShape::Action),
            ("Filter", "MyRewrite", BodyShape::Plan),
        ],
        "got {summary:?}"
    );
    let bodies: Vec<&str> = got.iter().map(|b| b.body.as_str()).collect();
    assert_eq!(
        bodies,
        vec![
            "mul_body",
            "choose_body",
            "gate_body",
            "plan_body",
            "roll_body",
            "deal_body",
            "payadd_body",
            "redirect_body",
            "on_turn_start",
            "my_rewrite",
        ]
    );
}

#[test]
fn parse_on_bodies_trailing_comma_and_multiline() {
    let shapes = shapes();
    let src = r#"
    On::Filter(
        Filter::DiceRollPlan(
            "",
            None,
            on_plan,
        ),
    ),
    On::Action(Action::Pass(
        "",
        "",
        None,
        on_pass,
    )),
"#;
    let got = parse_on_bodies(src, &shapes);
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!(got[0].body, "on_plan");
    assert_eq!(got[0].shape, BodyShape::Plan);
    assert_eq!(got[1].body, "on_pass");
    assert_eq!(got[1].shape, BodyShape::Action);
}

#[test]
fn retired_spellings_are_loud() {
    let shapes = shapes();
    for src in [
        "On::FilterFlag(&[FilterKind::ActGate], \"\", None, f)",
        "On::FilterValue(&[FilterKind::DiceRollPlan], \"\", None, f)",
        "On::Filter(Filter::DiceRollPlan(\"\", None, FilterBody::Plan(f)))",
    ] {
        let result = std::panic::catch_unwind(|| parse_on_bodies(src, &shapes));
        assert!(
            result.is_err(),
            "expected a loud failure for {src:?}, got {result:?}"
        );
    }
}

#[test]
fn default_exprs_are_shape_typed() {
    assert_eq!(default_expr(BodyShape::Value), "0");
    assert_eq!(default_expr(BodyShape::Flag), "0");
    assert_eq!(default_expr(BodyShape::Gate), "card_sdk::GateValue::default()");
    assert_eq!(default_expr(BodyShape::Plan), "card_sdk::PlanValue::default()");
    assert_eq!(default_expr(BodyShape::Roll), "card_sdk::RollValue::default()");
    assert_eq!(default_expr(BodyShape::Deal), "card_sdk::DealValue::default()");
    assert_eq!(default_expr(BodyShape::PayAdd), "card_sdk::PayAddValue::default()");
    assert_eq!(
        default_expr(BodyShape::Redirect),
        "card_sdk::RedirectValue::default()"
    );
}

// ---------------------------------------------------------------------------
// Loud miss on the real tree
// ---------------------------------------------------------------------------

fn walk_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk_rs(&p, out);
        } else if p.extension().and_then(|s| s.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

fn rules_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rules/")
        .to_path_buf()
}

/// Count `On::*` constructor spellings the parser is supposed to recognize
/// (comment-stripped): Filter / Action / Play / Counteract / Settle / …
fn count_on_spellings(src: &str) -> usize {
    let s = strip_rust_comments(src);
    let mut n = 0;
    let mut rest = s.as_str();
    while let Some(i) = rest.find("On::") {
        let after = &rest[i + 4..];
        let name_end = after
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(after.len());
        let variant = &after[..name_end];
        if matches!(
            variant,
            "Filter"
                | "Action"
                | "Play"
                | "Counteract"
                | "Settle"
                | "Mortgage"
                | "Redeem"
                | "Buy"
                | "Build"
                | "Message"
        ) {
            n += 1;
        }
        rest = &rest[i + 4 + name_end..];
    }
    n
}

#[test]
fn real_tree_registrations_are_not_silently_empty() {
    let shapes = shapes();
    let root = rules_root();
    let mut files = Vec::new();
    for dir in [
        "cards",
        "skills",
        "events",
        "tiles",
        "markers",
        "status",
        "fixtures",
        "bandori-std",
    ] {
        walk_rs(&root.join(dir), &mut files);
    }
    assert!(!files.is_empty(), "no rules sources under {root:?}");

    let mut parsed = 0usize;
    let mut spelled = 0usize;
    let mut filter_kinds: HashMap<String, usize> = HashMap::new();
    let mut action_kinds: HashMap<String, usize> = HashMap::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let spell = count_on_spellings(&src);
        spelled += spell;
        if spell == 0 {
            continue;
        }
        let got = parse_on_bodies(&src, &shapes);
        if got.is_empty() {
            panic!(
                "{} has {} On::* spellings but parse_on_bodies matched 0",
                f.display(),
                spell
            );
        }
        parsed += got.len();
        for b in &got {
            let Some(kind) = b.kind.as_deref() else {
                continue;
            };
            if b.variant == "Filter" {
                *filter_kinds.entry(kind.to_string()).or_default() += 1;
            } else {
                *action_kinds.entry(kind.to_string()).or_default() += 1;
            }
        }
    }
    assert!(
        parsed > 0,
        "parse_on_bodies matched 0 registrations across the real rules tree (spelled {spelled})"
    );
    assert_eq!(
        parsed, spelled,
        "parser missed registrations: spelled {spelled}, parsed {parsed}"
    );
    eprintln!(
        "on_parse: {parsed} registrations, {} filter kinds, {} action kinds",
        filter_kinds.len(),
        action_kinds.len()
    );
}

#[test]
fn every_registration_names_a_declared_kind() {
    let shapes = shapes();
    let root = rules_root();
    let mut files = Vec::new();
    for dir in ["cards", "skills", "events", "tiles", "markers", "status", "fixtures"] {
        walk_rs(&root.join(dir), &mut files);
    }

    // Declared kind names from the tables (and the live KINDS registry).
    let mut declared_filter: Vec<String> = shapes.keys().cloned().collect();
    declared_filter.sort();
    let mut declared_action: Vec<String> = parse_action_kind_names(
        &std::fs::read_to_string(root.join("card-sdk/src/kinds.rs")).unwrap(),
    );
    declared_action.extend(parse_action_kind_names(
        &std::fs::read_to_string(root.join("bandori-std/src/kind.rs")).unwrap(),
    ));
    declared_action.sort();
    declared_action.dedup();

    // Cross-check the tables against `Filter::KINDS` / `Action::KINDS`.
    let sdk_filter: Vec<String> = card_sdk::Filter::KINDS.iter().map(|k| k.name.to_string()).collect();
    let sdk_action: Vec<String> = card_sdk::Action::KINDS.iter().map(|k| k.name.to_string()).collect();
    for name in &sdk_filter {
        assert!(
            shapes.contains_key(name),
            "Filter::KINDS has `{name}` but the filter_kinds! table parse missed it"
        );
    }
    for name in &sdk_action {
        assert!(
            declared_action.contains(name),
            "Action::KINDS has `{name}` but the action_kinds! table parse missed it"
        );
    }
    assert_eq!(sdk_filter.len(), 27, "Filter::KINDS row count");

    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        for b in parse_on_bodies(&src, &shapes) {
            let Some(kind) = b.kind.as_deref() else {
                continue;
            };
            if b.variant == "Filter" {
                assert!(
                    shapes.contains_key(kind),
                    "{}: filter kind `{kind}` is not declared in filter_kinds!",
                    f.display()
                );
            } else if b.variant == "Action" {
                // Engine `Action` and ruleset `Actions` both land in the
                // combined declared list; a kind missing from both is a miss.
                assert!(
                    declared_action.contains(&kind.to_string()),
                    "{}: action kind `{kind}` is not declared in action_kinds!",
                    f.display()
                );
            }
        }
    }
}

#[test]
fn kind_shapes_cover_all_payload_families() {
    let shapes = shapes();
    let mut seen = std::collections::BTreeSet::new();
    for s in shapes.values() {
        seen.insert(*s);
    }
    for need in [
        BodyShape::Value,
        BodyShape::Flag,
        BodyShape::Gate,
        BodyShape::Plan,
        BodyShape::Roll,
        BodyShape::Deal,
        BodyShape::PayAdd,
        BodyShape::Redirect,
    ] {
        assert!(seen.contains(&need), "filter_kinds! tables missing shape {need:?}");
    }
}

#[test]
fn pre_string_reads_inner_ctor_arg() {
    let shapes = shapes();
    let _ = &shapes;
    // Filter: pre is ctor arg 1 of the inner call.
    let after = r#"Filter::DiceRollPlan(pre::MINE, None, on_plan)),"#;
    assert_eq!(on_pre_string(after).as_deref(), Some("actor == owner"));

    let after = r#"Action::PassTile("card.placed && actor == owner", "", None, on_pass)),"#;
    assert_eq!(
        on_pre_string(after).as_deref(),
        Some("card.placed && actor == owner")
    );

    // Ruleset namespace with a full path.
    let after = r#"bandori_std::kind::Filters::MyRewrite("false", None, my_rewrite)),"#;
    assert_eq!(on_pre_string(after).as_deref(), Some("false"));

    // Flat On::Play shape (text after `On::Play(`).
    let after = r#""", "", None, play)"#;
    assert_eq!(on_pre_string(after).as_deref(), Some(""));
}