#!/usr/bin/env python3
"""Append cfg(kani) pub wrappers for every On::* body fn in card sources.

Body shapes (ABI v75+):
  Play / Counteract / Action / Settle / Mortgage / Redeem / Buy / Build
      `fn(player_id) -> Asked`
  Filter      `fn(player_id, value: i32) -> i32`
  FilterFlag  `fn(player_id, value: i32) -> bool`
  FilterValue `fn(player_id, v: FilterValue) -> FilterValue`

The wrapper is a pub `NAME_kani` that direct-calls the (possibly private)
body, so rules-kani trampolines can invoke it without a `code*` IF-chain.
Production (`cfg(not(kani))`) is untouched — the wrapper does not exist there.

Usage: python3 -I tools/kani/add_kani_wrappers.py [root ...]
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ON_RE = re.compile(
    r"On::(Play|Counteract|Action|Filter|FilterFlag|FilterValue|"
    r"Settle|Mortgage|Redeem|Buy|Build)\s*\(",
)

WRAPPER_HEAD = "\n// Kani-only direct-call exports (ppcost). Production builds omit these.\n"

ASKED = """#[cfg(kani)]
#[inline(always)]
pub fn {name}_kani(player_id: i32) -> card_sdk::Asked {{
    {name}(player_id)
}}
"""

FILTER = """#[cfg(kani)]
#[inline(always)]
pub fn {name}_kani(player_id: i32, value: i32) -> i32 {{
    {name}(player_id, value)
}}
"""

FILTER_FLAG = """#[cfg(kani)]
#[inline(always)]
pub fn {name}_kani(player_id: i32, value: i32) -> bool {{
    {name}(player_id, value)
}}
"""

FILTER_VALUE = """#[cfg(kani)]
#[inline(always)]
pub fn {name}_kani(player_id: i32, v: card_sdk::abi::FilterValue) -> card_sdk::abi::FilterValue {{
    {name}(player_id, v)
}}
"""

SHAPE = {
    "Play": ASKED,
    "Counteract": ASKED,
    "Action": ASKED,
    "Settle": ASKED,
    "Mortgage": ASKED,
    "Redeem": ASKED,
    "Buy": ASKED,
    "Build": ASKED,
    "Filter": FILTER,
    "FilterFlag": FILTER_FLAG,
    "FilterValue": FILTER_VALUE,
}


def parse_on_bodies(src: str) -> list[tuple[str, str]]:
    bodies: list[tuple[str, str]] = []
    for m in ON_RE.finditer(src):
        variant = m.group(1)
        i = m.end() - 1  # at '('
        depth = 0
        j = i
        while j < len(src):
            c = src[j]
            if c == "(":
                depth += 1
            elif c == ")":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        inner = src[i + 1 : j]
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
        # Trailing comma in On::Ctor(...) leaves an empty last part — drop it.
        parts = [p for p in parts if p]
        if not parts:
            continue
        last = parts[-1]
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", last):
            bodies.append((variant, last))
    return bodies


def run(roots: list[Path]) -> int:
    changed = 0
    for root in roots:
        for path in sorted(root.rglob("*.rs")):
            if path.name in ("lib.rs", "mod.rs"):
                continue
            src = path.read_text(encoding="utf-8")
            if "Kani-only direct-call exports (ppcost)" in src:
                continue
            bodies = parse_on_bodies(src)
            seen: set[str] = set()
            uniq: list[tuple[str, str]] = []
            for variant, b in bodies:
                if b not in seen:
                    seen.add(b)
                    uniq.append((variant, b))
            if not uniq:
                continue
            missing = [
                b
                for _, b in uniq
                if not re.search(rf"\bfn\s+{re.escape(b)}\s*\(", src)
            ]
            if missing:
                print(f"{path}: On bodies missing fn: {missing}", file=sys.stderr)
                return 1
            wrappers = "\n".join(SHAPE[variant].format(name=b) for variant, b in uniq)
            path.write_text(
                src.rstrip() + "\n" + WRAPPER_HEAD + wrappers, encoding="utf-8"
            )
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
    ]
    sys.exit(run(roots))