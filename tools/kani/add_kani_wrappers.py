#!/usr/bin/env python3
"""Append cfg(kani) pub wrappers for every On::* body fn in card sources.

Phase 3 typed DSL body shapes:
  Play / Counteract / Settle / Mortgage / Redeem / Buy / Build / Message
      `fn(player_id) -> Asked`
  On::Action(Action::Name(pre, reason, guard, body))  /  Actions::…
      `fn(player_id) -> Asked`
  On::Filter(Filter::Name(pre, guard, body))  /  Filters::…  — shape from the
  kind name (filter_kinds! table):
      value       `fn(player_id, value: i32) -> i32`
      flag        `fn(player_id, value: i32) -> bool`
      gate        `fn(i32, GateValue) -> GateValue`
      PlanValue   `fn(i32, PlanValue) -> PlanValue`
      RollValue   `fn(i32, RollValue) -> RollValue`
      DealValue   `fn(i32, DealValue) -> DealValue`
      PayAddValue `fn(i32, PayAddValue) -> PayAddValue`
      RedirectValue `fn(i32, RedirectValue) -> RedirectValue`

The wrapper is a pub `NAME_kani` that direct-calls the (possibly private)
body, so rules-kani trampolines can invoke it without a `code*` IF-chain.
Production (`cfg(not(kani))`) is untouched — the wrapper does not exist there.

Usage: python3 -I tools/kani/add_kani_wrappers.py [root ...]
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ON_RE = re.compile(r"On::(Play|Counteract|Action|Filter|Settle|Mortgage|Redeem|Buy|Build|Message)\s*\(")
RETIRED_RE = re.compile(r"On::(FilterFlag|FilterValue|Gate)\b|FilterBody::")

WRAPPER_HEAD = "\n// Kani-only direct-call exports (ppcost). Production builds omit these.\n"

ASKED = """#[cfg(kani)]
#[inline(always)]
pub fn __NAME___kani(player_id: i32) -> card_sdk::Asked {
    __NAME__(player_id)
}
"""

FILTER_VALUE = """#[cfg(kani)]
#[inline(always)]
pub fn __NAME___kani(player_id: i32, value: i32) -> i32 {
    __NAME__(player_id, value)
}
"""

FILTER_FLAG = """#[cfg(kani)]
#[inline(always)]
pub fn __NAME___kani(player_id: i32, value: i32) -> bool {
    __NAME__(player_id, value)
}
"""


def _payload(payload: str) -> str:
    return f"""#[cfg(kani)]
#[inline(always)]
pub fn __NAME___kani(player_id: i32, v: card_sdk::{payload}) -> card_sdk::{payload} {{
    __NAME__(player_id, v)
}}
"""


SHAPE = {
    "Action": ASKED,
    "Play": ASKED,
    "Counteract": ASKED,
    "Settle": ASKED,
    "Mortgage": ASKED,
    "Redeem": ASKED,
    "Buy": ASKED,
    "Build": ASKED,
    "Message": ASKED,
    "Value": FILTER_VALUE,
    "Flag": FILTER_FLAG,
    "Gate": _payload("GateValue"),
    "Plan": _payload("PlanValue"),
    "Roll": _payload("RollValue"),
    "Deal": _payload("DealValue"),
    "PayAdd": _payload("PayAddValue"),
    "Redirect": _payload("RedirectValue"),
}

# Engine filter kinds → body shape (mirrors rules/card-sdk/src/kinds.rs).
# Ruleset filter kinds are appended from bandori-std/src/kind.rs below.
FILTER_KIND_SHAPE = {
    "PaySplitChoose": "Flag",
    "PaySplitAdd": "PayAdd",
    "PaySplitMul": "Value",
    "PaySplitAt": "Value",
    "DiceRollPlan": "Plan",
    "AbnormalGuard": "Gate",
    "ImmuneAll": "Gate",
    "Untargetable": "Gate",
    "Redirect": "Redirect",
    "BuyGate": "Gate",
    "BuyAdd": "Value",
    "BuyMul": "Value",
    "BuySet": "Value",
    "BuyAssign": "Deal",
    "PayCombinedAdd": "Value",
    "PayCombinedMul": "Value",
    "PayCombinedCancel": "Flag",
    "ActGate": "Gate",
    "PayGate": "Gate",
    "MoveGate": "Gate",
    "ExclusiveUse": "Gate",
    "TurnBonusTime": "Value",
    "TurnBaseTime": "Value",
    "PlayGate": "Gate",
    "DiceRollValue": "Roll",
    "OnBoardGate": "Gate",
    "TargetableGate": "Gate",
}


def _load_ruleset_filter_shapes() -> dict[str, str]:
    """Parse `Name => payload(Ty)` / `Name => gate` arms from filter_kinds! tables."""
    extra: dict[str, str] = {}
    for rel in (
        Path("rules/bandori-std/src/kind.rs"),
        Path("rules/card-sdk/src/kinds.rs"),
    ):
        if not rel.is_file():
            continue
        text = rel.read_text(encoding="utf-8")
        for block in re.findall(r"filter_kinds!\s*\{([^{}]+)\}", text, flags=re.S):
            for arm in re.finditer(
                r"([A-Za-z_][A-Za-z0-9_]*)\s*(?:@\s*[^=]+)?=>\s*(value|flag|gate|payload\s*\(([^)]+)\))",
                block,
            ):
                name, shape_tok, ty = arm.group(1), arm.group(2), arm.group(3)
                if shape_tok == "value":
                    extra[name] = "Value"
                elif shape_tok == "flag":
                    extra[name] = "Flag"
                elif shape_tok == "gate":
                    extra[name] = "Gate"
                else:
                    last = ty.strip().rsplit("::", 1)[-1]
                    extra[name] = {
                        "GateValue": "Gate",
                        "PlanValue": "Plan",
                        "RollValue": "Roll",
                        "DealValue": "Deal",
                        "PayAddValue": "PayAdd",
                        "RedirectValue": "Redirect",
                    }.get(last, "Plan")
    return extra


def _paren_end(src: str, open_i: int) -> int:
    depth = 0
    j = open_i
    while j < len(src):
        if src[j] == "(":
            depth += 1
        elif src[j] == ")":
            depth -= 1
            if depth == 0:
                return j + 1
        j += 1
    return len(src)


def _split_args(inner: str) -> list[str]:
    parts: list[str] = []
    depth = 0
    cur: list[str] = []
    for c in inner:
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
        if c == "," and depth == 0:
            parts.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
    if cur:
        parts.append("".join(cur).strip())
    return [p for p in parts if p]


def _strip_comments(src: str) -> str:
    out = []
    i = 0
    n = len(src)
    while i < n:
        if src.startswith("//", i):
            while i < n and src[i] != "\n":
                i += 1
        elif src.startswith("/*", i):
            i += 2
            while i + 1 < n and not src.startswith("*/", i):
                i += 1
            i = min(i + 2, n)
        else:
            out.append(src[i])
            i += 1
    return "".join(out)


def parse_on_bodies(src: str, filter_shapes: dict[str, str]) -> list[tuple[str, str]]:
    """Return `(shape_key, body_ident)` in declaration order."""
    code = _strip_comments(src)
    if RETIRED_RE.search(code):
        raise SystemExit("retired On::FilterFlag/FilterValue/FilterBody spelling still present")
    src = code
    bodies: list[tuple[str, str]] = []
    for m in ON_RE.finditer(src):
        variant = m.group(1)
        open_i = m.end() - 1  # at '('
        end = _paren_end(src, open_i)
        inner = src[open_i + 1 : end - 1]
        if variant in ("Filter", "Action"):
            # `Ns::Kind(args…)` — one nested ctor call.
            cm = re.match(r"\s*(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Za-z_][A-Za-z0-9_]*)\s*\(", inner)
            if not cm:
                continue
            kind = cm.group(1)
            ctor_open = cm.end() - 1
            ctor_end = _paren_end(inner, ctor_open)
            args = _split_args(inner[ctor_open + 1 : ctor_end - 1])
            if not args:
                continue
            last = args[-1]
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", last):
                continue
            if variant == "Action":
                bodies.append(("Action", last))
            else:
                shape = filter_shapes.get(kind)
                if shape is None:
                    raise SystemExit(f"unknown filter kind `{kind}` (not in filter_kinds! tables)")
                bodies.append((shape, last))
        else:
            args = _split_args(inner)
            if not args:
                continue
            last = args[-1]
            if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", last):
                bodies.append(("Action", last))
    return bodies


WRAPPER_MARK = "Kani-only direct-call exports (ppcost)"


def _strip_wrappers(src: str) -> str:
    """Drop a trailing `NAME_kani` export block so it can be regenerated."""
    idx = src.find(WRAPPER_MARK)
    if idx < 0:
        return src
    # Cut from the comment head (keep any preceding blank line tidy).
    head = src[:idx].rstrip()
    return head + "\n"


def _wrapper_ok(existing: str, name: str, shape: str) -> bool:
    """True if `name_kani` is already present with the expected signature."""
    want = {
        "Action": rf"pub fn {name}_kani\(player_id: i32\) -> card_sdk::Asked",
        "Value": rf"pub fn {name}_kani\(player_id: i32, value: i32\) -> i32",
        "Flag": rf"pub fn {name}_kani\(player_id: i32, value: i32\) -> bool",
        "Gate": rf"pub fn {name}_kani\(player_id: i32, v: card_sdk::GateValue\) -> card_sdk::GateValue",
        "Plan": rf"pub fn {name}_kani\(player_id: i32, v: card_sdk::PlanValue\) -> card_sdk::PlanValue",
        "Roll": rf"pub fn {name}_kani\(player_id: i32, v: card_sdk::RollValue\) -> card_sdk::RollValue",
        "Deal": rf"pub fn {name}_kani\(player_id: i32, v: card_sdk::DealValue\) -> card_sdk::DealValue",
        "PayAdd": rf"pub fn {name}_kani\(player_id: i32, v: card_sdk::PayAddValue\) -> card_sdk::PayAddValue",
        "Redirect": rf"pub fn {name}_kani\(player_id: i32, v: card_sdk::RedirectValue\) -> card_sdk::RedirectValue",
    }[shape]
    return re.search(want, existing) is not None


def run(roots: list[Path]) -> int:
    filter_shapes = dict(FILTER_KIND_SHAPE)
    filter_shapes.update(_load_ruleset_filter_shapes())
    changed = 0
    for root in roots:
        for path in sorted(root.rglob("*.rs")):
            if path.name in ("lib.rs", "mod.rs"):
                continue
            raw = path.read_text(encoding="utf-8")
            src = _strip_wrappers(raw)
            bodies = parse_on_bodies(src, filter_shapes)
            seen: set[str] = set()
            uniq: list[tuple[str, str]] = []
            for variant, b in bodies:
                if b not in seen:
                    seen.add(b)
                    uniq.append((variant, b))
            if not uniq:
                if src != raw:
                    path.write_text(src, encoding="utf-8")
                    print(f"stripped stale wrappers: {path}")
                    changed += 1
                continue
            missing = [
                b
                for _, b in uniq
                if not re.search(rf"\bfn\s+{re.escape(b)}\s*\(", src)
            ]
            if missing:
                print(f"{path}: On bodies missing fn: {missing}", file=sys.stderr)
                return 1
            stale = [
                (shape, b)
                for shape, b in uniq
                if not _wrapper_ok(raw, b, shape)
            ]
            if not stale and raw == src:
                continue
            # Regenerate the whole export block (also fixes stale signatures).
            wrappers = "\n".join(SHAPE[shape].replace("__NAME__", b) for shape, b in uniq)
            path.write_text(src.rstrip() + "\n" + WRAPPER_HEAD + wrappers, encoding="utf-8")
            print(f"wrapped {path}: {[b for _, b in uniq]}")
            changed += 1
    print(f"files changed: {changed}")
    return 0


if __name__ == "__main__":
    roots = [Path(p) for p in sys.argv[1:]] or [
        Path("rules/cards"),
        Path("rules/skills"),
        Path("rules/events"),
        Path("rules/tiles"),
        Path("rules/markers"),
        Path("rules/status"),
    ]
    sys.exit(run(roots))