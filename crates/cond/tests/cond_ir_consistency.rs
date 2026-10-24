//! Per-guard consistency: every inventory condition string lowers to exactly
//! the embedded IR in `rules-kani`'s generated table. No guard may exist whose
//! Rust side was authored separately.
//!
//! Installs the real [`bandori_std::cel`] extension before lowering — the
//! production inventory is full of sugar (`charge` / `group_is` / …) that
//! fails to lower against an empty `CondExt`. A guard that will not lower
//! fails the test; the only skips are the named [`UNLOWERABLE_ALLOWLIST`].

use rules_cond::lower::{lower_with_src, OwnedProgram};
use rules_cond::Vocab;
use rules_cond_ir::nodes::Node;

/// Guards that legitimately have no lowering. Each entry needs a reason.
/// Empty today — do not add a blanket `else { continue }`.
const UNLOWERABLE_ALLOWLIST: &[&str] = &[];

/// `crates/cond` → `rules/` (the inventory lives under the guest tree).
fn rules_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../rules")
}

fn install_ext() {
    let _ = rules_cond::install_ext(bandori_std::cel::ext());
}

fn vocab() -> Vocab<'static> {
    Vocab::with_ext(bandori_std::cel::ext())
}

fn collect_guards() -> Vec<String> {
    let root = rules_dir();
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

/// The table exactly as `rules/kani/build.rs` generates it (it is built, not
/// committed).
fn generated_text() -> String {
    install_ext();
    rules_cond::gen_ir::generate(&rules_dir()).0
}

/// Parse the generated file's `src` strings and node arrays back into
/// `OwnedProgram`s so we can compare without linking rules-kani.
fn parse_generated() -> Vec<(String, OwnedProgram)> {
    let text = generated_text();
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
    rest[..b]
        .trim()
        .trim_end_matches(',')
        .trim()
        .parse()
        .unwrap()
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
    install_ext();
    let guards = collect_guards();
    let embedded = parse_generated();
    assert!(
        !embedded.is_empty(),
        "generated table is empty — the generator output is broken"
    );
    let v = vocab();
    let mut checked = 0;
    let mut allowlisted = 0;
    for src in &guards {
        let prog = match lower_with_src(src, &v) {
            Ok(p) => p,
            Err(e) => {
                if UNLOWERABLE_ALLOWLIST.contains(&src.as_str()) {
                    allowlisted += 1;
                    continue;
                }
                panic!(
                    "guard {src:?} failed to lower: {e}\n\
                     (install the real CondExt, or add it to UNLOWERABLE_ALLOWLIST \
                     with a reason — blanket skips are not allowed)"
                );
            }
        };
        let hit = embedded.iter().find(|(s, _)| s == src);
        let Some((_, emb)) = hit else {
            panic!(
                "guard {src:?} is in the inventory but not in the generated IR — \
                 re-run the generator output is broken"
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
    assert!(
        checked >= 50,
        "too few guards checked: {checked} (allowlisted {allowlisted})"
    );
    // Sugar guards must be part of the matched set, not silently skipped.
    for sugar in ["charge(", "group_is(", "persona_is("] {
        assert!(
            guards.iter().any(|g| g.contains(sugar)),
            "inventory no longer contains a `{sugar}` guard — the gate lost its target"
        );
    }
    println!(
        "every_guard_lowers_to_its_embedded_ir: {checked} guards match, \
         {allowlisted} allowlisted, {} in inventory",
        guards.len()
    );
}

/// `generate()` must not quietly drop unlowerable guards: its skip list has
/// to be exactly the allow-list.
#[test]
fn generate_skips_are_only_the_allowlist() {
    install_ext();
    let generated = rules_cond::gen_ir::generate_with(&rules_dir(), &vocab())
        .expect("gen_ir generate_with");
    for (src, err) in &generated.skipped {
        assert!(
            UNLOWERABLE_ALLOWLIST.contains(&src.as_str()),
            "generate() skipped {src:?}: {err}\n\
             not on UNLOWERABLE_ALLOWLIST — the inventory guard must lower"
        );
    }
    for allowed in UNLOWERABLE_ALLOWLIST {
        assert!(
            generated.skipped.iter().any(|(s, _)| s == allowed),
            "UNLOWERABLE_ALLOWLIST entry {allowed:?} actually lowered — remove it"
        );
    }
    println!(
        "generate_skips_are_only_the_allowlist: {} lowered, {} skipped",
        generated.count,
        generated.skipped.len()
    );
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

/// Guard const ids are `0..n` in file order (BY_ID index). `guard_id!` and
/// `BY_ID[id]` rely on this.
#[test]
fn generated_guard_ids_are_dense_and_ordered() {
    let text = generated_text();
    let mut ids = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("id: ") && t.ends_with(',') {
            let n: usize = t[4..t.len() - 1].trim().parse().unwrap();
            ids.push(n);
        }
    }
    assert!(!ids.is_empty(), "no id: fields in generated table");
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(*id, i, "guard ids must be dense 0..n in file order");
    }
    // BY_ID length matches.
    let by_id_count = text
        .lines()
        .filter(|l| l.trim_start().starts_with("&G_"))
        .count();
    assert_eq!(by_id_count, ids.len(), "BY_ID length != number of guards");
    println!(
        "generated_guard_ids_are_dense_and_ordered: {} ids",
        ids.len()
    );
}