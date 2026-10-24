#!/usr/bin/env python3
"""Check that every roster card's authored `CardMeta` text matches the rulebook
sheet cell it came from.

The sheet (`docs/rulebook/cards-sheet.csv`, one column per band; refreshed from
the live `新卡组卡` tab) is the spec. `tools/rulebook/extract.py` turns it into
`docs/rulebook/cards.json` (id -> the whole cell). This tool compares the
**body** of that cell -- everything after the name line -- against the
`CardMeta::text` the ruleset exports (`docs/DISPLAY.md`, via
`cargo run -q -p rules-native --example dump_display`).

Accepted encoding differences (the `CardText` invariants, `crates/card-sdk/src/text.rs`):
  * whitespace runs
  * list markers: `（N）` / `(N)` / a bare `N.` are positional in `CardText`,
    so the flat form may re-space or re-style them
  * a standalone `：` line between the name and the body (the sheet's
    `名字 \\n ：效果` shape) is a separator, not body prose
  * a `\\` before `[` (the 游击演出 stray `\\[特]` escape) is not prose
Everything else is compared exactly, including full-width punctuation and the
sheet's `1dNmodM` dice notation. Sheet-side typos therefore show up here on
purpose -- the card text is the sheet text.

Usage:
  node tools/roster-dump.mjs            # ids + names for matching
  cargo run -q -p rules-native --example dump_display -- /tmp/display.json
  python tools/rulebook/check-card-text.py --display /tmp/display.json
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BOOK = ROOT / "docs" / "rulebook" / "cards.json"


def norm(text: str) -> str:
    """Whitespace-stripped comparison form; `\\[` sheet escapes drop the `\\`."""
    return re.sub(r"\s+", "", text).replace("\\[", "[")


def norm_markers(text: str) -> str:
    """Also drop positional list markers, which `CardText` re-styles."""
    t = norm(text)
    t = re.sub(r"（\d+）", "", t)
    t = re.sub(r"\(\d+\)", "", t)
    t = re.sub(r"(?<![A-Za-z0-9])\d+\.", "", t)
    return t


def body_of(cell: str, names: list[str]) -> str:
    """The cell's effect text: drop the name line and its `：` separator.

    The sheet writes the name as `名字：效果`, `名字 \\n ：效果`, or -- for a
    prefix-only face like 「（美咲）」 -- `名字效果` with no separator at all.
    """
    text = cell
    # longest matching name first, so `（育美）We Can☆Hurray!` wins over `（育美）`
    for name in sorted((n for n in names if n), key=len, reverse=True):
        if text.startswith(name):
            text = text[len(name) :]
            break
    # one name/body separator: `：` / `:` after the name, possibly on the next
    # line (`名字 \\n ：效果`). Only the first one -- `[手]：` labels stay.
    text = text.lstrip()
    if text.startswith("：") or text.startswith(":"):
        text = text[1:]
    return text.strip()


def names_of(card_id: str, row: dict) -> list[str]:
    out = []
    raw = row.get("rawName") or ""
    if raw:
        out.append(raw)
    suffix = card_id.split(":", 1)[1] if ":" in card_id else card_id
    if suffix:
        out.append(suffix)
    name = row.get("name") or ""
    if name:
        out.append(name)
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--display", required=True, help="dump_display output")
    args = ap.parse_args()

    if not BOOK.is_file():
        print(f"missing {BOOK} -- run: python tools/rulebook/extract.py")
        return 2
    book: dict[str, str] = json.loads(BOOK.read_text(encoding="utf-8"))
    display: dict = json.loads(Path(args.display).read_text(encoding="utf-8"))
    cards: dict = display.get("cards", {})

    bad = 0
    matched = 0
    for cid, cell in sorted(book.items()):
        row = cards.get(cid)
        if row is None:
            print(f"{cid}: on the sheet but absent from display.json")
            bad += 1
            continue
        matched += 1
        authored = row.get("text") or ""
        body = body_of(cell, names_of(cid, row))
        if norm_markers(authored) == norm_markers(body):
            continue
        bad += 1
        print(f"{cid}: CardMeta text does not match the sheet body")
        a, s = norm(authored), norm(body)
        i = next((k for k in range(min(len(a), len(s))) if a[k] != s[k]), min(len(a), len(s)))
        print(f"  sheet : …{s[max(0, i - 20): i + 50]}")
        print(f"  text  : …{a[max(0, i - 20): i + 50]}")

    # cards the sheet does not carry (derived tokens, rule-only pseudos) are fine
    extra = sorted(set(cards) - set(book))
    print(f"{matched} sheet cards checked, {bad} problem(s).")
    print(f"{len(extra)} display card(s) with no sheet cell (derived / rule-only).")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())