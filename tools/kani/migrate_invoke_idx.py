#!/usr/bin/env python3
"""Rewrite string-keyed `invoke::*` call sites to the indexed API.

Mechanical only — no spec changes. Patterns:

    invoke::play("ID", p)            → invoke::play_idx(card_id!("ID"), p)
    invoke::entry("ID", n, p)        → invoke::entry_idx(card_id!("ID"), n, p)
    invoke::hook_only("ID", k, p)    → invoke::hook_only_idx(card_id!("ID"), k, p)
    invoke::hook("ID", k, h, p)      → invoke::hook_idx(card_id!("ID"), k, h, p)
    invoke::counteract("ID", p)      → invoke::counteract_idx(card_id!("ID"), p)
    invoke::event_play("ID", p)      → invoke::event_play_idx(card_id!("ID"), p)
    invoke::skill_play("ID", p)      → invoke::skill_play_idx(card_id!("ID"), p)

    const FOO: &str = "ID"; … invoke::play(FOO, p)
        → const FOO: usize = card_id!("ID"); … invoke::play_idx(FOO, p)

The id argument may be a string literal (possibly multi-line) or an identifier
bound by `const NAME: &str = "…"` in the same file.

Also rewrites `invoke::find("ID")` → `invoke::by_idx(card_id!("ID"))`.

Usage (worktree root):
    python3 tools/kani/migrate_invoke_idx.py            # rewrite in place
    python3 tools/kani/migrate_invoke_idx.py --check    # report only
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROOFS = ROOT / "rules" / "kani" / "src" / "proofs"

# fn name → (idx fn, n_extra_before_player)  where extra args sit between id and player
FUNCS = {
    "play": ("play_idx", 0),
    "entry": ("entry_idx", 1),
    "hook": ("hook_idx", 2),
    "hook_only": ("hook_only_idx", 1),
    "counteract": ("counteract_idx", 0),
    "event_play": ("event_play_idx", 0),
    "skill_play": ("skill_play_idx", 0),
    "find": ("by_idx", 0),  # special-cased: no trailing player
}

CONST_RE = re.compile(
    r"const\s+([A-Z][A-Z0-9_]*)\s*:\s*&str\s*=\s*\"((?:\\.|[^\"])*)\"\s*;"
)


def split_args(argstr: str) -> list[str]:
    """Split a Rust call argument list on top-level commas."""
    args: list[str] = []
    cur: list[str] = []
    depth = 0
    in_str = False
    esc = False
    i = 0
    while i < len(argstr):
        ch = argstr[i]
        if in_str:
            cur.append(ch)
            if esc:
                esc = False
            elif ch == "\\":
                esc = True
            elif ch == '"':
                in_str = False
            i += 1
            continue
        if ch == '"':
            in_str = True
            cur.append(ch)
        elif ch in "([{":
            depth += 1
            cur.append(ch)
        elif ch in ")]}":
            depth -= 1
            cur.append(ch)
        elif ch == "," and depth == 0:
            args.append("".join(cur).strip())
            cur = []
        else:
            cur.append(ch)
        i += 1
    tail = "".join(cur).strip()
    if tail:
        args.append(tail)
    return args


def build_global_consts() -> dict[str, str]:
    """Map every `const NAME: &str = "id"` / `spec::mod::NAME` in rules/kani/src."""
    reg: dict[str, str] = {}
    src_root = ROOT / "rules" / "kani" / "src"
    for path in src_root.rglob("*.rs"):
        text = path.read_text(encoding="utf-8")
        mod = path.stem  # ppp.rs → ppp
        for m in CONST_RE.finditer(text):
            name, lit = m.group(1), m.group(2)
            reg[name] = lit
            reg[f"spec::{mod}::{name}"] = lit
            reg[f"crate::spec::{mod}::{name}"] = lit
            # also the use-site path without crate::
            if path.parent.name == "spec":
                reg[f"spec::{mod}::{name}"] = lit
    return reg


def lit_of(arg: str, consts: dict[str, str]) -> str | None:
    """Return the id string if `arg` is a literal or a known id const."""
    arg = arg.strip()
    m = re.fullmatch(r"\"((?:\\.|[^\"])*)\"", arg)
    if m:
        return m.group(1)
    if arg in consts:
        return consts[arg]
    # bare name also matches the local-file map
    return consts.get(arg)


def rewrite_file(path: Path, check: bool, glob_consts: dict[str, str]) -> tuple[int, list[str]]:
    src = path.read_text(encoding="utf-8")
    local_consts = {m.group(1): m.group(2) for m in CONST_RE.finditer(src)}
    notes: list[str] = []
    n_hits = 0

    # Local consts used as invoke targets become `usize` indices. Spec-module
    # consts (`spec::ppp::CARD_*`) stay `&str` (they also feed `intern` / pile
    # names) — call sites inline `card_id!("…")` instead.
    to_usize: set[str] = set()
    for name, lit in local_consts.items():
        used_as_id = re.search(
            rf"invoke::(?:play|entry|hook|hook_only|counteract|event_play|skill_play|find)\s*\(\s*{name}\b",
            src,
        )
        # Also used as a pass-through helper argument that reaches invoke.
        used_as_helper = re.search(
            rf"\b(?:pass_gain_iff|play_gate_allows|assert_activation_keep)\s*\(\s*{name}\b",
            src,
        )
        looks_like_card = ":" in lit and not lit.startswith("skill.")
        if used_as_id or used_as_helper or looks_like_card and used_as_id:
            to_usize.add(name)

    def const_sub(m: re.Match) -> str:
        nonlocal n_hits
        name, lit = m.group(1), m.group(2)
        if name in to_usize:
            n_hits += 1
            return f'const {name}: usize = card_id!("{lit}");'
        return m.group(0)

    out = CONST_RE.sub(const_sub, src)

    # invoke::fn( … )
    call_re = re.compile(
        r"invoke::(play|entry|hook|hook_only|counteract|event_play|skill_play|find)\s*\(",
    )

    pieces: list[str] = []
    pos = 0
    while True:
        m = call_re.search(out, pos)
        if not m:
            pieces.append(out[pos:])
            break
        pieces.append(out[pos : m.start()])
        fname = m.group(1)
        i = m.end()
        depth = 1
        in_str = False
        esc = False
        while i < len(out) and depth:
            ch = out[i]
            if in_str:
                if esc:
                    esc = False
                elif ch == "\\":
                    esc = True
                elif ch == '"':
                    in_str = False
            else:
                if ch == '"':
                    in_str = True
                elif ch == "(":
                    depth += 1
                elif ch == ")":
                    depth -= 1
            i += 1
        inner = out[m.end() : i - 1]
        args = split_args(inner)
        idx_fn, _n_extra = FUNCS[fname]
        id_arg = (args[0] if args else "").strip()
        lit = lit_of(id_arg, {**glob_consts, **local_consts})
        if lit is None:
            notes.append(
                f"{path.name}: could not resolve id arg {id_arg!r} for invoke::{fname}"
            )
            pieces.append(out[m.start() : i])
            pos = i
            continue
        # Prefer the named const when it is already a usize index.
        id_expr = (
            id_arg
            if id_arg in to_usize
            else f'card_id!("{lit}")'
        )
        if fname == "find":
            new = f"invoke::by_idx({id_expr})"
        else:
            rest = args[1:]
            rest_s = ", ".join(rest) if rest else ""
            if rest_s:
                new = f"invoke::{idx_fn}({id_expr}, {rest_s})"
            else:
                new = f"invoke::{idx_fn}({id_expr})"
        pieces.append(new)
        n_hits += 1
        pos = i

    new_src = "".join(pieces)
    if new_src != src and not check:
        path.write_text(new_src, encoding="utf-8")
    return n_hits, notes


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("paths", nargs="*", type=Path)
    args = ap.parse_args()
    files = args.paths or sorted(PROOFS.glob("*.rs"))
    glob_consts = build_global_consts()
    total = 0
    notes: list[str] = []
    for p in files:
        n, ns = rewrite_file(p, args.check, glob_consts)
        total += n
        notes.extend(ns)
        if n:
            print(f"{p.relative_to(ROOT)}: {n} site(s)")
    print(f"total sites: {total}")
    for n in notes:
        print(f"NOTE: {n}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())