// Shared `On::*` registration parser (Phase 4 typed DSL).
//
// Parses the Phase 3 call shape:
//
// ```ignore
// On::Filter(Filter::DiceRollPlan(pre, guard, body)),
// On::Filter(bandori_std::kind::Filters::MyRewrite(pre, guard, body)),
// On::Action(Action::TurnStart(pre, guard, body).counter_reason(..)),
// On::Action(Actions::Exile(pre, guard, body).counter_reason(..)),
// On::Play(pre, why_not, body).counter_reason(..),   // and the other non-Filter variants
// ```
//
// Included by `build.rs` (body trampolines) and `tests/on_parse.rs` (loud-miss
// guards). Retired spellings (`On::FilterFlag`, `On::FilterValue`, `FilterBody`)
// are hard errors: a silent miss is the bug this parser exists to prevent.

#![allow(dead_code)] // build.rs uses a subset; the test target uses the rest.

use std::collections::HashMap;

/// Body shape of one registration. Drives trampoline call defaults and the
/// parser unit tests (one case per arm).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BodyShape {
    /// `fn(i32) -> Asked` — Play / Counteract / Action / Settle / …
    Action,
    /// `fn(i32, i32) -> Asked` — an entry carrying `On::declare_arg`'s
    /// activation-argument body (docs/ACTIVATION.md §2). The trampoline passes
    /// an unconstrained `kani::any()` as the arg.
    ActionArg,
    /// `fn(i32, i32) -> i32`
    Value,
    /// `fn(i32, i32) -> bool`
    Flag,
    /// `fn(i32, GateValue) -> GateValue`
    Gate,
    /// `fn(i32, PlanValue) -> PlanValue`
    Plan,
    /// `fn(i32, RollValue) -> RollValue`
    Roll,
    /// `fn(i32, DealValue) -> DealValue`
    Deal,
    /// `fn(i32, PayAddValue) -> PayAddValue`
    PayAdd,
    /// `fn(i32, RedirectValue) -> RedirectValue`
    Redirect,
}

/// One `On::*` registration, in declaration order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OnBody {
    /// `On` variant: `"Filter"` / `"Action"` / `"Play"` / …
    pub variant: String,
    /// Kind constructor name for Filter/Action (`"DiceRollPlan"`, `"TurnStart"`).
    /// `None` for the non-namespaced variants.
    pub kind: Option<String>,
    /// Body function ident (last arg of the inner call).
    pub body: String,
    pub shape: BodyShape,
}

/// Strip `//` line comments and `/* */` block comments so `On::` in docs is
/// not mistaken for a constructor.
pub fn strip_rust_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

/// `On` variants whose body is `fn(i32) -> Asked` (last ctor arg).
const ACTIONISH: &[&str] = &[
    "Play",
    "Counteract",
    "Settle",
    "Mortgage",
    "Redeem",
    "Buy",
    "Build",
    "Message",
];

/// Retired `On` variants / types. A hit is a build error — never a silent skip.
const RETIRED: &[&str] = &["FilterFlag", "FilterValue", "FilterBody", "Gate"];

/// Split a top-level comma-separated argument list (nested `()`, `[]`, `{}`).
fn split_args(inner: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = Vec::new();
    let mut depth = 0i32;
    let mut cur = 0usize;
    for (k, c) in inner.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(inner[cur..k].trim());
                cur = k + 1;
            }
            _ => {}
        }
    }
    parts.push(inner[cur..].trim());
    parts
}

fn last_nonempty<'a>(parts: &[&'a str]) -> Option<&'a str> {
    parts
        .iter()
        .rev()
        .find(|p| !p.trim().is_empty())
        .map(|p| p.trim())
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && s.chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            == Some(true)
}

/// Balanced-paren span starting at `open` (`src[open] == '('`). Returns the
/// index just past the matching `)`.
fn paren_end(src: &str, open: usize) -> Option<usize> {
    let b = src.as_bytes();
    if open >= b.len() || b[open] != b'(' {
        return None;
    }
    let mut depth = 0usize;
    let mut j = open;
    while j < b.len() {
        if b[j] == b'(' {
            depth += 1;
        } else if b[j] == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(j + 1);
            }
        }
        j += 1;
    }
    None
}

/// Last-arg body: a bare ident. (`FilterBody::Arm(fn)` is retired and rejected.)
fn body_ident(last: &str) -> Option<String> {
    if is_ident(last) {
        return Some(last.to_string());
    }
    None
}

/// Path ending in `Ns::Kind` (or bare `Kind`) immediately followed by `(`.
/// Returns `(kind_name, open_paren_index)`.
fn ctor_head(s: &str) -> Option<(String, usize)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let mut last_ident;
    loop {
        let start = i;
        while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
            i += 1;
        }
        if i == start || !is_ident(&s[start..i]) {
            return None;
        }
        last_ident = s[start..i].to_string();
        let save = i;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i + 1 < b.len() && b[i] == b':' && b[i + 1] == b':' {
            i += 2;
            continue;
        }
        i = save;
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'(' {
            return Some((last_ident, i));
        }
        return None;
    }
}

/// Map a `filter_kinds!` payload type path to its [`BodyShape`].
fn payload_shape(ty: &str) -> BodyShape {
    let last = ty.rsplit("::").next().unwrap_or(ty).trim();
    match last {
        "GateValue" => BodyShape::Gate,
        "PlanValue" => BodyShape::Plan,
        "RollValue" => BodyShape::Roll,
        "DealValue" => BodyShape::Deal,
        "PayAddValue" => BodyShape::PayAdd,
        "RedirectValue" => BodyShape::Redirect,
        other => panic!(
            "on_parse: unknown filter payload type `{other}` (expected GateValue/PlanValue/RollValue/DealValue/PayAddValue/RedirectValue)"
        ),
    }
}

/// Parse one `filter_kinds!` table body into `(kind_name, shape)` rows.
/// Accepts both `Name => shape;` and ruleset `Name @ id => shape;`.
pub fn parse_filter_kind_shapes(src: &str) -> Vec<(String, BodyShape)> {
    let src = strip_rust_comments(src);
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = src[search..].find("filter_kinds!") {
        let at = search + rel + "filter_kinds!".len();
        let Some(open) = src[at..].find('{').map(|i| at + i) else {
            break;
        };
        let Some(close_rel) = src[open..].find('}') else {
            break;
        };
        let close = open + close_rel;
        let body = &src[open + 1..close];
        for line in body.split(';') {
            let line = line.trim();
            if line.is_empty() || line.starts_with("on ") || line.starts_with("ns ") {
                continue;
            }
            // `Name => shape` or `Name @ id => shape`
            let Some((name_part, shape_part)) = line.split_once("=>") else {
                continue;
            };
            let name = name_part.split('@').next().unwrap_or("").trim().to_string();
            if !is_ident(&name) {
                continue;
            }
            let shape_part = shape_part.trim();
            let shape = if shape_part == "value" {
                BodyShape::Value
            } else if shape_part == "flag" {
                BodyShape::Flag
            } else if shape_part == "gate" {
                BodyShape::Gate
            } else if let Some(rest) = shape_part.strip_prefix("payload") {
                let rest = rest.trim();
                let ty = rest
                    .strip_prefix('(')
                    .and_then(|r| r.strip_suffix(')'))
                    .unwrap_or_else(|| {
                        panic!("on_parse: malformed payload arm for `{name}`: {shape_part}")
                    });
                payload_shape(ty)
            } else {
                panic!("on_parse: unknown filter shape arm for `{name}`: {shape_part}");
            };
            out.push((name, shape));
        }
        search = close + 1;
    }
    out
}

/// Parse one `action_kinds!` table body into kind names.
pub fn parse_action_kind_names(src: &str) -> Vec<String> {
    let src = strip_rust_comments(src);
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = src[search..].find("action_kinds!") {
        let at = search + rel + "action_kinds!".len();
        let Some(open) = src[at..].find('{').map(|i| at + i) else {
            break;
        };
        let Some(close_rel) = src[open..].find('}') else {
            break;
        };
        let close = open + close_rel;
        let body = &src[open + 1..close];
        for line in body.split(';') {
            let line = line.trim();
            if line.is_empty() || line.starts_with("on ") || line.starts_with("ns ") {
                continue;
            }
            let name = line.split('@').next().unwrap_or("").trim().to_string();
            if is_ident(&name) {
                out.push(name);
            }
        }
        search = close + 1;
    }
    out
}

/// kind name → shape, from every `filter_kinds!` table in `sources`.
pub fn kind_shapes_from(sources: &[String]) -> HashMap<String, BodyShape> {
    let mut m = HashMap::new();
    for src in sources {
        for (name, shape) in parse_filter_kind_shapes(src) {
            if let Some(prev) = m.insert(name.clone(), shape) {
                if prev != shape {
                    panic!("on_parse: kind `{name}` declared with two shapes");
                }
            }
        }
    }
    m
}

/// Parse every `On::*` constructor in a module source.
///
/// `shapes` maps filter kind names to their [`BodyShape`] (from the
/// `filter_kinds!` tables). Unknown filter kinds panic — a miss is loud.
pub fn parse_on_bodies(src: &str, shapes: &HashMap<String, BodyShape>) -> Vec<OnBody> {
    let src = strip_rust_comments(src);
    let src = src.as_str();
    let mut bodies = Vec::new();
    let mut search_from = 0;
    while let Some(rel) = src[search_from..].find("On::") {
        let start = search_from + rel;
        let after = start + 4;
        let mut name_end = after;
        while name_end < src.len()
            && (src.as_bytes()[name_end].is_ascii_alphanumeric()
                || src.as_bytes()[name_end] == b'_')
        {
            name_end += 1;
        }
        let variant = src[after..name_end].to_string();
        for retired in RETIRED {
            if &variant == retired {
                panic!(
                    "on_parse: retired spelling `On::{variant}` at byte {start} — use On::Filter(Filter::Name(...))"
                );
            }
        }
        // skip whitespace to '('
        let mut p = name_end;
        while p < src.len() && src.as_bytes()[p].is_ascii_whitespace() {
            p += 1;
        }
        if p >= src.len() || src.as_bytes()[p] != b'(' {
            search_from = name_end;
            continue;
        }
        let Some(j) = paren_end(src, p) else {
            search_from = name_end;
            continue;
        };
        let inner = &src[p + 1..j - 1];

        if variant == "Filter" || variant == "Action" {
            // `On::Filter(Ns::Kind(pre, …, body))` — one ctor call.
            if inner.contains("FilterBody::") {
                panic!("on_parse: retired `FilterBody::` arm in `On::{variant}` at byte {start}");
            }
            let Some((kind, open)) = ctor_head(inner) else {
                panic!(
                    "on_parse: `On::{variant}(` at byte {start} is not `Ns::Kind(...)` — got `{}`",
                    inner.chars().take(60).collect::<String>()
                );
            };
            let Some(end) = paren_end(inner, open) else {
                panic!("on_parse: unbalanced ctor call for `On::{variant}::{kind}`");
            };
            let args = split_args(&inner[open + 1..end - 1]);
            let Some(last) = last_nonempty(&args) else {
                panic!("on_parse: `On::{variant}::{kind}` has no body arg");
            };
            let Some(body) = body_ident(last) else {
                panic!("on_parse: `On::{variant}::{kind}` last arg `{last}` is not a body ident");
            };
            let shape = if variant == "Filter" {
                *shapes.get(&kind).unwrap_or_else(|| {
                    panic!(
                        "on_parse: filter kind `{kind}` not in the filter_kinds! tables (known: {:?})",
                        {
                            let mut names: Vec<_> = shapes.keys().cloned().collect();
                            names.sort();
                            names
                        }
                    )
                })
            } else {
                BodyShape::Action
            };
            // A chained `.declare(...)` / `.declare_arg(pick, body)` is part of
            // the same registration (docs/ACTIVATION.md §2). `.declare_arg`
            // replaces the constructor body with an `fn(i32, i32) -> Asked`.
            let (body, shape) = apply_declare(src, j, body, shape);
            bodies.push(OnBody {
                variant: variant.clone(),
                kind: Some(kind),
                body,
                shape,
            });
        } else if ACTIONISH.contains(&variant.as_str()) {
            let args = split_args(inner);
            let Some(last) = last_nonempty(&args) else {
                search_from = j;
                continue;
            };
            let Some(body) = body_ident(last) else {
                panic!(
                    "on_parse: `On::{variant}` last arg `{last}` is not a body ident (byte {start})"
                );
            };
            let (body, shape) = apply_declare(src, j, body, BodyShape::Action);
            bodies.push(OnBody {
                variant,
                kind: None,
                body,
                shape,
            });
        } else {
            // Unknown On variant: skip (other arms of the enum may appear in
            // type positions). Do not invent a body.
        }
        search_from = j;
    }
    bodies
}

/// Fold a chained `On::declare` / `On::declare_arg` into one registration.
/// `after` is the byte just past `On::X(...)`'s closing paren.
///
/// * `.declare(pick)` -- decline-only; the body stays `fn(i32) -> Asked`.
/// * `.declare_arg(pick, body)` -- the body becomes `fn(i32, i32) -> Asked`
///   and the trampoline must hand it an arg.
fn apply_declare(
    src: &str,
    after: usize,
    body: String,
    shape: BodyShape,
) -> (String, BodyShape) {
    let tail = src[after..].trim_start();
    if let Some(rest) = tail.strip_prefix(".declare_arg(") {
        // `rest` starts *inside* the call; `paren_end` indexes the `(` itself,
        // which sits immediately before `rest` within `tail`.
        let open = tail.len() - rest.len() - 1;
        // Second argument is the replacement body ident.
        let Some(end) = paren_end(tail, open) else {
            return (body, shape);
        };
        let args = split_args(&tail[open + 1..end - 1]);
        if args.len() < 2 {
            return (body, shape);
        }
        if let Some(ident) = body_ident(args[1].trim()) {
            return (ident, BodyShape::ActionArg);
        }
        return (body, shape);
    }
    if tail.starts_with(".declare(") {
        // Decline-only: body unchanged.
        return (body, shape);
    }
    (body, shape)
}

/// Trampoline second-argument default for a filter body. `Action` has no
/// second argument.
pub fn default_expr(shape: BodyShape) -> &'static str {
    match shape {
        BodyShape::Action => "/* action: no second arg */",
        BodyShape::ActionArg => "/* action-arg: second arg is the activation arg */",
        BodyShape::Value | BodyShape::Flag => "0",
        BodyShape::Gate => "card_sdk::GateValue::default()",
        BodyShape::Plan => "card_sdk::PlanValue::default()",
        BodyShape::Roll => "card_sdk::RollValue::default()",
        BodyShape::Deal => "card_sdk::DealValue::default()",
        BodyShape::PayAdd => "card_sdk::PayAddValue::default()",
        BodyShape::Redirect => "card_sdk::RedirectValue::default()",
    }
}

/// First-argument `pre` of an `On::*` constructor.
///
/// - `On::Filter` / `On::Action`: `pre` is ctor arg 1 of the inner
///   `Ns::Kind(pre, …)` call.
/// - Everything else: the first top-level argument (string literal or
///   `pre::MINE` / guard const).
pub fn on_pre_string(after_on: &str) -> Option<String> {
    // `after_on` is the text starting at `On::Variant`'s trailing `(` content
    // (i.e. just after `(`).
    let b = after_on.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    // Detect nested ctor: `Path::Ident(`
    if let Some((_, open)) = ctor_head(after_on) {
        let Some(end) = paren_end(after_on, open) else {
            return None;
        };
        let args = split_args(&after_on[open + 1..end - 1]);
        let first = args.iter().find(|p| !p.trim().is_empty())?;
        return pre_string(first.trim());
    }
    // Flat args: first top-level arg is pre.
    // `after_on` starts *inside* the outer `(`, so depth 0 + `)` is the closer.
    let mut depth = 0usize;
    let mut j = 0;
    while j < b.len() {
        if b[j] == b'(' {
            depth += 1;
        } else if b[j] == b')' {
            if depth == 0 {
                break;
            }
            depth -= 1;
        }
        j += 1;
    }
    let args = split_args(&after_on[..j]);
    let first = args.iter().find(|p| !p.trim().is_empty())?;
    pre_string(first.trim())
}

/// `pre` argument: a string literal, `pre::MINE`, or a known guard const.
/// (Also used for the flat `On::Play(pre, …)` shape.)
pub fn pre_string(s: &str) -> Option<String> {
    let rest = s.trim_start();
    if rest.starts_with("pre::MINE") {
        return Some("actor == owner".to_string());
    }
    // Known guard-const path references (`bandori_std::guards::OWN_START`, …)
    // resolve through the installed extension.
    let path_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == ':' || c == '_'))
        .unwrap_or(rest.len());
    let path = &rest[..path_end];
    if let Some(lit) = rules_cond::ext::vocab().guard_const(path) {
        return Some(lit.to_string());
    }
    // First string literal (handles `\"` escapes).
    let bytes = rest.as_bytes();
    let mut j = 0;
    while j < bytes.len() && bytes[j] != b'"' {
        if bytes[j] == b';' {
            return None;
        }
        j += 1;
    }
    if j >= bytes.len() {
        return None;
    }
    j += 1;
    let start = j;
    while j < bytes.len() && bytes[j] != b'"' {
        if bytes[j] == b'\\' {
            j += 1;
        }
        j += 1;
    }
    if j > bytes.len() {
        return None;
    }
    let raw = &rest[start..j];
    Some(raw.replace("\\\"", "\"").replace("\\\\", "\\"))
}
