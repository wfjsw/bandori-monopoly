#!/usr/bin/env python3
"""Append cfg(kani) pub wrappers for every On::* body fn in card sources.

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
    r"On::(Play|Counteract|Hook|Gate|RollPlan|AtEnd|Settle|Message)\s*\(",
)

WRAPPER_HEAD = "\n// Kani-only direct-call exports (ppcost). Production builds omit these.\n"

ONE = """#[cfg(kani)]
#[inline(always)]
pub fn {name}_kani(player_id: i32) -> card_sdk::Asked {{
    {name}(player_id)
}}
"""


def parse_on_bodies(src: str) -> list[str]:
    bodies: list[str] = []
    for m in ON_RE.finditer(src):
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
        if not parts:
            continue
        last = parts[-1]
        if re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", last):
            bodies.append(last)
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
            uniq: list[str] = []
            for b in bodies:
                if b not in seen:
                    seen.add(b)
                    uniq.append(b)
            if not uniq:
                continue
            missing = [
                b
                for b in uniq
                if not re.search(rf"\bfn\s+{re.escape(b)}\s*\(", src)
            ]
            if missing:
                print(f"{path}: On bodies missing fn: {missing}", file=sys.stderr)
                return 1
            wrappers = "\n".join(ONE.format(name=b) for b in uniq)
            path.write_text(
                src.rstrip() + "\n" + WRAPPER_HEAD + wrappers, encoding="utf-8"
            )
            print(f"wrapped {path}: {uniq}")
            changed += 1
    print(f"files changed: {changed}")
    return 0


if __name__ == "__main__":
    roots = [Path(p) for p in sys.argv[1:]] or [
        Path("/home/monopoly/kani-ppcost-wt/rules/cards"),
        Path("/home/monopoly/kani-ppcost-wt/rules/skills"),
        Path("/home/monopoly/kani-ppcost-wt/rules/events"),
        Path("/home/monopoly/kani-ppcost-wt/rules/tiles"),
        Path("/home/monopoly/kani-ppcost-wt/rules/tile_marks"),
    ]
    sys.exit(run(roots))