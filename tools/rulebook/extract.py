#!/usr/bin/env python3
"""Build docs/rulebook/cards.json: card id -> its rule text from the rule book sheet.

The sheet (docs/rulebook/cards-sheet.csv, one column per band) holds one cell per
card: "卡名：[标签]：效果文本". Cells encode line breaks as the two-character
sequence `\\n`, and the name is the id's text after "BAND:" (with its prefix).
Matching is by prefix against each card in the ruleset roster dump
(tools/roster-dump.mjs; not data/cards.json -- that file is gone). A row whose
name line is blank is named after its sheet cell instead (`#J11`), and matches
on that A1 reference.

Note: docs/rulebook/cards.json here is the rulebook sheet extract, a different
file from the retired data/cards.json.

Usage: python tools/rulebook/extract.py
"""

import csv
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from roster import load_cards  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SHEET = ROOT / "docs" / "rulebook" / "cards-sheet.csv"
OUT = ROOT / "docs" / "rulebook" / "cards.json"


def a1(row_i: int, col_i: int) -> str:
    """Spreadsheet A1 reference for a 0-based CSV (row, col) -- CSV row 0 is
    the header, i.e. spreadsheet row 1; column 0 is A."""
    n, letters = col_i, ""
    while True:
        n, r = divmod(n, 26)
        letters = chr(ord("A") + r) + letters
        if n == 0:
            return f"{letters}{row_i + 1}"
        n -= 1


def cells():
    """Yield ``(a1, text)`` for every non-empty body cell."""
    rows = list(csv.reader(open(SHEET, encoding="utf-8-sig")))
    for row_i, row in enumerate(rows):
        if row_i == 0:
            continue
        for col_i, cell in enumerate(row):
            # the export writes line breaks as the two-character sequence \n
            c = cell.replace("\\n", "\n").strip()
            # list markers （1）（2）… each start their own line: the sheet often
            # runs them together mid-line
            c = re.sub(r"(?<!^)（\d+）", lambda m: "\n" + m.group(0), c)
            if c:
                yield a1(row_i, col_i), c


def head_of(cell: str) -> str:
    """Everything before the first colon, trimmed -- the sheet sometimes writes
    '名字 \\n ：效果' instead of '名字：效果'."""
    return cell.split("：")[0].split(":")[0].strip()


def names_of(card):
    """The sheet leads with the name as written in the id (after "BAND:"),
    falling back to the display name."""
    out = []
    if ":" in card["id"]:
        out.append(card["id"].split(":", 1)[1])
    if card.get("name"):
        out.append(card["name"])
    return out


def main() -> int:
    cards = load_cards()
    all_cells = list(cells())
    found, missing = {}, []
    for card in cards:
        names = names_of(card)
        hit = None
        for ref, cell in all_cells:
            # a row whose name line is blank is named after its sheet cell
            # (`#J11`); every other row leads with its name
            if any(n == f"#{ref}" for n in names) or any(
                cell.startswith(n) for n in names if not n.startswith("#")
            ):
                hit = cell
                break
        if hit:
            found[card["id"]] = hit
        else:
            missing.append(f'{card["id"]} ({card.get("name", "")})')
    OUT.write_text(json.dumps(dict(sorted(found.items())), ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    print(f"cards.json: {len(found)} ids matched, {len(missing)} unmatched")
    for m in missing:
        print("   ", m)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())