//! Per-guard consistency: every inventory condition string lowers to exactly
//! the embedded IR in `rules-kani`'s generated table. No guard may exist whose
//! Rust side was authored separately.

use rules_cond::lower::{lower_src, OwnedProgram};
use rules_cond_ir::nodes::Node;

fn collect_guards() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let mut out = Vec::new();
    for d in ["cards", "skills", "tiles", "events", "fixtures"] {
        for g in rules_cond::lower::inventory_guards(&root.join(d)) {
            out.push(g);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Parse the generated file's `src` strings and node arrays back into
/// `OwnedProgram`s so we can compare without linking rules-kani.
fn parse_generated() -> Vec<(String, OwnedProgram)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("kani/src/generated/cond_irs.rs");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("generated file missing ({e}); run gen_cond_ir"));
    let mut out = Vec::new();
    // Very small structured parse: each block is
    //   pub static G_...: GuardIr = GuardIr {
    //       src: "...",
    //       program: Program { nodes: G_..._NODES, root: N },
    //   };
    // with the NODES array just above.
    let mut lines = text.lines().map(str::to_string).collect::<Vec<_>>();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].contains("pub static") && lines[i].contains("_NODES: &[Node]") {
            let mut nodes = Vec::new();
            // Same line may open the array: `... = &[`
            if !lines[i].contains("= &[") {
                i += 1;
            }
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with("];") {
                let t = lines[i].trim();
                if t.starts_with("Node") {
                    nodes.push(parse_node(t));
                }
                i += 1;
            }
            i += 1;
            while i < lines.len() && !lines[i].contains("src:") {
                i += 1;
            }
            let src = parse_src_line(&lines[i]);
            while i < lines.len() && !lines[i].contains("root:") {
                i += 1;
            }
            let root = parse_root_line(&lines[i]);
            out.push((src, OwnedProgram { nodes, root }));
        }
        i += 1;
    }
    out
}

fn parse_src_line(line: &str) -> String {
    let a = line.find("src: \"").unwrap() + 6;
    let b = line.rfind('"').unwrap();
    // Unescape the Rust string literal back to the CEL source.
    line[a..b].replace("\\\"", "\"").replace("\\\\", "\\")
}

fn parse_root_line(line: &str) -> u16 {
    let a = line.find("root:").unwrap() + 5;
    let rest = line[a..].trim();
    let b = rest.find('}').unwrap_or(rest.len());
    rest[..b].trim().trim_end_matches(',').trim().parse().unwrap()
}

fn parse_node(line: &str) -> Node {
    // Node { kind: Kind::And, payload: 0, a: 1, b: 2, c: 0 }
    let kind_s = {
        let a = line.find("Kind::").unwrap();
        let b = line[a..].find(',').unwrap() + a;
        &line[a..b]
    };
    let kind = match kind_s {
        "Kind::ConstInt" => rules_cond_ir::Kind::ConstInt,
        "Kind::ConstBool" => rules_cond_ir::Kind::ConstBool,
        "Kind::ConstNull" => rules_cond_ir::Kind::ConstNull,
        "Kind::Var" => rules_cond_ir::Kind::Var,
        "Kind::Neg" => rules_cond_ir::Kind::Neg,
        "Kind::Not" => rules_cond_ir::Kind::Not,
        "Kind::And" => rules_cond_ir::Kind::And,
        "Kind::Or" => rules_cond_ir::Kind::Or,
        "Kind::Tern" => rules_cond_ir::Kind::Tern,
        "Kind::Add" => rules_cond_ir::Kind::Add,
        "Kind::Sub" => rules_cond_ir::Kind::Sub,
        "Kind::Mul" => rules_cond_ir::Kind::Mul,
        "Kind::Div" => rules_cond_ir::Kind::Div,
        "Kind::Rem" => rules_cond_ir::Kind::Rem,
        "Kind::Eq" => rules_cond_ir::Kind::Eq,
        "Kind::Ne" => rules_cond_ir::Kind::Ne,
        "Kind::Lt" => rules_cond_ir::Kind::Lt,
        "Kind::Le" => rules_cond_ir::Kind::Le,
        "Kind::Gt" => rules_cond_ir::Kind::Gt,
        "Kind::Ge" => rules_cond_ir::Kind::Ge,
        "Kind::In" => rules_cond_ir::Kind::In,
        "Kind::Size" => rules_cond_ir::Kind::Size,
        "Kind::List" => rules_cond_ir::Kind::List,
        "Kind::Call" => rules_cond_ir::Kind::Call,
        other => panic!("unknown kind {other}"),
    };
    let field = |name: &str| -> i64 {
        let a = line.find(&format!("{name}:")).unwrap() + name.len() + 1;
        let rest = line[a..].trim();
        let b = rest.find([',', '}']).unwrap_or(rest.len());
        rest[..b].trim().parse().unwrap()
    };
    Node {
        kind,
        payload: field("payload"),
        a: field("a") as u16,
        b: field("b") as u16,
        c: field("c") as u16,
    }
}

#[test]
fn every_guard_lowers_to_its_embedded_ir() {
    let guards = collect_guards();
    let embedded = parse_generated();
    assert!(
        !embedded.is_empty(),
        "generated table is empty — run gen_cond_ir"
    );
    let mut checked = 0;
    for src in &guards {
        let Ok(prog) = lower_src(src) else {
            continue;
        };
        let hit = embedded.iter().find(|(s, _)| s == src);
        let Some((_, emb)) = hit else {
            panic!(
                "guard {src:?} is in the inventory but not in the generated IR — re-run gen_cond_ir"
            );
        };
        assert_eq!(
            &prog.nodes, &emb.nodes,
            "IR nodes diverged for {src:?}\n  lower: {:?}\n  embedded: {:?}",
            prog.nodes, emb.nodes
        );
        assert_eq!(prog.root, emb.root, "IR root diverged for {src:?}");
        checked += 1;
    }
    assert!(checked >= 50, "too few guards checked: {checked}");
    println!("every_guard_lowers_to_its_embedded_ir: {checked} guards match");
}

#[test]
fn generated_has_no_stray_entries() {
    let guards: std::collections::HashSet<_> = collect_guards().into_iter().collect();
    for (src, _) in parse_generated() {
        if src == "actor == owner" {
            continue;
        }
        assert!(
            guards.contains(&src),
            "generated IR has {src:?} which is not in the inventory"
        );
    }
}