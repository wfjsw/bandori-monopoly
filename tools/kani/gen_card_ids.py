#!/usr/bin/env python3
"""Generate `rules/kani/src/generated/card_ids.rs`.

Emits, for every `CardDef` in `rules_kani::invoke::RULESET` order:

  * `pub const CARD_COUNT: usize`
  * `pub static IDS: &[&str]` — the id string at each index
  * `pub const <CRATE>_<CONST>: usize` — one named const per id
  * `#[macro_export] macro_rules! card_id!` — compile-time literal → usize
    (`compile_error!` on unknown, same shape as `guard_id!`)

The flat `CardDef` table is **not** emitted here: `invoke::by_idx` walks
`RULESET` with integer arithmetic only (no string scan). `card_id!` is the
only string→index path and it is a compile-time match.

Usage (from the worktree root):
    python3 tools/kani/gen_card_ids.py
    python3 tools/kani/gen_card_ids.py --check   # fail if output would change
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RULES = ROOT / "rules"
OUT = RULES / "kani" / "src" / "generated" / "card_ids.rs"

# Same order as `rules/kani/src/invoke.rs` `RULESET`.
BANDS: list[tuple[str, str, Path]] = [
    ("card_ag", "AG", RULES / "cards" / "card-ag"),
    ("card_crychic", "CRYCHIC", RULES / "cards" / "card-crychic"),
    ("card_general", "GENERAL", RULES / "cards" / "card-general"),
    ("card_hhw", "HHW", RULES / "cards" / "card-hhw"),
    ("card_morfonica", "MOR", RULES / "cards" / "card-morfonica"),
    ("card_mujica", "MUJICA", RULES / "cards" / "card-mujica"),
    ("card_mygo", "MYGO", RULES / "cards" / "card-mygo"),
    ("card_pp", "PP", RULES / "cards" / "card-pp"),
    ("card_ppp", "PPP", RULES / "cards" / "card-ppp"),
    ("card_ras", "RAS", RULES / "cards" / "card-ras"),
    ("card_roselia", "R", RULES / "cards" / "card-roselia"),
    ("card_sumimi", "SUMIMI", RULES / "cards" / "card-sumimi"),
    ("skill_bands", "SKILL_BAND", RULES / "skills" / "skill-bands"),
    ("skill_characters", "SKILL", RULES / "skills" / "skill-characters"),
    ("rules_tiles", "TILE", RULES / "tiles"),
    ("rules_tile_marks", "MARK", RULES / "tile_marks"),
    ("rules_events", "EVENT", RULES / "events"),
]


def parse_cards_list(lib: Path) -> list[str]:
    text = lib.read_text(encoding="utf-8")
    m = re.search(
        r"pub\s+(?:static|const)\s+CARDS\s*:\s*&\s*\[\s*card_sdk::CardDef\s*\]\s*=\s*&\s*\[(.*?)\];",
        text,
        re.S,
    )
    if not m:
        m = re.search(
            r"pub\s+(?:static|const)\s+CARDS\s*:\s*&\s*\[\s*CardDef\s*\]\s*=\s*&\s*\[(.*?)\];",
            text,
            re.S,
        )
    if not m:
        raise SystemExit(f"no CARDS array in {lib}")
    body = re.sub(r"//[^\n]*", "", m.group(1))
    return [x.strip() for x in body.split(",") if x.strip()]


def find_card_id(crate: Path, ident: str) -> str:
    """Locate the id string of `pub const IDENT: CardDef`.

    Accepts `CardDef::new("literal")` and `CardDef::new(CONST)` where
    `CONST: &str = "literal"` lives in the same crate.
    """
    decl = re.compile(
        rf"pub\s+const\s+{re.escape(ident)}\s*:\s*CardDef\s*=\s*CardDef::new\s*\(\s*"
        r"(?:\"((?:\\.|[^\"])*)\"\s*,|([A-Za-z_][A-Za-z0-9_]*)\s*,)",
        re.S,
    )
    files = sorted(crate.rglob("*.rs"))
    texts = {p: p.read_text(encoding="utf-8", errors="ignore") for p in files}
    for path, text in texts.items():
        m = decl.search(text)
        if not m:
            continue
        lit_s, cref_name = m.group(1), m.group(2)
        if lit_s is not None:
            # Unescape the common Rust string escapes used in ids.
            return (
                lit_s.replace(r"\n", "\n")
                .replace(r"\"", '"')
                .replace(r"\\", "\\")
            )
        arg = cref_name.strip()
        # Const reference: look up `const ARG: &str = "..."` in the same file,
        # then anywhere in the crate.
        cref = re.compile(
            rf"(?:pub\s+)?const\s+{re.escape(arg)}\s*:\s*&str\s*=\s*\"((?:\\.|[^\"])*)\""
        )
        cm = cref.search(text)
        if cm:
            return cm.group(1)
        for t2 in texts.values():
            cm = cref.search(t2)
            if cm:
                return cm.group(1)
        raise SystemExit(
            f"cannot resolve id const {arg!r} for {ident} under {crate}"
        )
    raise SystemExit(f"cannot find CardDef::new id for {ident} under {crate}")


def const_name(prefix: str, ident: str) -> str:
    name = re.sub(r"[^A-Za-z0-9_]", "_", ident).upper()
    name = re.sub(r"_+", "_", name).strip("_")
    if not name:
        name = "X"
    if name[0].isdigit():
        name = "N_" + name
    return f"{prefix}_{name}"


def collect() -> list[tuple[str, str, str, int, int]]:
    """Return rows (id, const_name, rust_ident, band, card) in RULESET order."""
    rows: list[tuple[str, str, str, int, int]] = []
    for bi, (_crate, prefix, path) in enumerate(BANDS):
        idents = parse_cards_list(path / "src" / "lib.rs")
        for ci, ident in enumerate(idents):
            cid = find_card_id(path, ident)
            rows.append((cid, const_name(prefix, ident), ident, bi, ci))
    return rows


def render(rows: list[tuple[str, str, str, int, int]]) -> str:
    n = len(rows)
    # Detect duplicate ids / names.
    seen_id: dict[str, int] = {}
    seen_name: dict[str, int] = {}
    for cid, cname, _ident, bi, ci in rows:
        if cid in seen_id:
            raise SystemExit(f"duplicate id {cid!r} at band {bi} card {ci}")
        if cname in seen_name:
            raise SystemExit(f"duplicate const name {cname}")
        seen_id[cid] = len(seen_id)
        seen_name[cname] = len(seen_name)

    lines: list[str] = []
    a = lines.append
    a("//! GENERATED by `tools/kani/gen_card_ids.py`. Do not edit by hand.")
    a("//!")
    a("//! Compile-time id → index map for `rules_kani::invoke`.")
    a("//! Order is `invoke::RULESET` flatten order (bands then cards).")
    a("//!")
    a("//! ```ignore")
    a("//! let idx = card_id!(\"PP:[丸山彩]憧憬的前方\");")
    a("//! invoke::play_idx(idx, player);   // no string scan")
    a("//! ```")
    a("//!")
    a("//! Regenerate after adding a card / skill / event / tile / mark:")
    a("//! ```sh")
    a("//! python3 tools/kani/gen_card_ids.py")
    a("//! ```")
    a("")
    a("#![allow(dead_code)]")
    a("")
    a(f"/// Number of registered `CardDef`s in `invoke::RULESET`.")
    a(f"pub const CARD_COUNT: usize = {n};")
    a("")
    a("/// Id string at each index (for native asserts / debug).")
    a("/// Under Kani never scan this — use [`card_id!`].")
    a("pub static IDS: &[&str] = &[")
    for cid, _c, _i, _b, _cix in rows:
        lit = cid.replace("\\", "\\\\").replace('"', '\\"')
        a(f'    "{lit}",')
    a("];")
    a("")
    a("// ---------------------------------------------------------------------------")
    a("// Named index constants")
    a("// ---------------------------------------------------------------------------")
    a("")
    for cid, cname, ident, bi, ci in rows:
        idx = seen_id[cid]
        a(f"/// `{cid}`  (`RULESET[{bi}][{ci}]`, `{ident}`)")
        a(f"pub const {cname}: usize = {idx};")
        a("")
    a("// ---------------------------------------------------------------------------")
    a("// card_id! — compile-time literal → usize (unknown = compile error)")
    a("// ---------------------------------------------------------------------------")
    a("")
    a("/// Resolve a card / skill / event / tile / mark id to its stable index.")
    a("///")
    a("/// Resolves at **compile time**. Unknown literals are a compile error")
    a("/// (same contract as `guard_id!`). The result indexes")
    a("/// [`crate::invoke::by_idx`].")
    a("#[macro_export]")
    a("macro_rules! card_id {")
    for cid, _c, _i, _b, _cix in rows:
        lit = cid.replace("\\", "\\\\").replace('"', '\\"')
        idx = seen_id[cid]
        a(f'    ("{lit}") => {{ {idx}usize }};')
    a("    ($other:literal) => {")
    a('        compile_error!("card_id!: unknown card/skill/event/tile/mark id; re-run tools/kani/gen_card_ids.py")')
    a("    };")
    a("}")
    a("")
    a("/// Runtime string → index. `None` if unknown.")
    a("/// **Not** for Kani (O(n) compare). Use [`card_id!`] there.")
    a("pub fn card_id_opt(id: &str) -> Option<usize> {")
    a("    let mut i = 0usize;")
    a("    while i < CARD_COUNT {")
    a("        if IDS[i] == id {")
    a("            return Some(i);")
    a("        }")
    a("        i += 1;")
    a("    }")
    a("    None")
    a("}")
    a("")
    a("/// Runtime string → index. Panics on an unknown id.")
    a("pub fn card_id_const(id: &str) -> usize {")
    a("    match card_id_opt(id) {")
    a("        Some(i) => i,")
    a('        None => panic!("card_id!: unknown id {id}"),')
    a("    }")
    a("}")
    a("")
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="fail if output would change")
    args = ap.parse_args()
    rows = collect()
    text = render(rows)
    if args.check:
        old = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if old != text:
            print(f"OUT OF DATE: {OUT} (run tools/kani/gen_card_ids.py)", file=sys.stderr)
            return 1
        print(f"up to date: {len(rows)} ids")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUT} ({len(rows)} ids)")
    return 0


if __name__ == "__main__":
    sys.exit(main())