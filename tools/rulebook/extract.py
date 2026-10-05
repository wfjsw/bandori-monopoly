#!/usr/bin/env python3
"""Build docs/rulebook/cards.json: card id -> its rule text from the rule book sheet.

The sheet (docs/rulebook/cards-sheet.csv, one column per band) holds one cell per
card: "卡名：[标签]：效果文本". Cells encode line breaks as the two-character
sequence `\\n`, and the name is the id's text after "BAND:" (with its prefix).
Matching is by prefix against each card in data/cards.json.

Usage: python tools/rulebook/extract.py
"""

import csv
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SHEET = ROOT / "docs" / "rulebook" / "cards-sheet.csv"
OUT = ROOT / "docs" / "rulebook" / "cards.json"


def cells():
    rows = list(csv.reader(open(SHEET, encoding="utf-8-sig")))
    for row in rows[1:]:
        for cell in row:
            # the export writes line breaks as the two-character sequence \n
            c = cell.replace("\\n", "\n").strip()
            # list markers （1）（2）… each start their own line: the sheet often
            # runs them together mid-line
            c = re.sub(r"(?<!^)（\d+）", lambda m: "\n" + m.group(0), c)
            if c:
                yield c


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
    data = json.load(open(ROOT / "data" / "cards.json", encoding="utf-8-sig"))
    all_cells = list(cells())
    found, missing = {}, []
    for card in data["cards"]:
        hit = next((cell for cell in all_cells if any(cell.startswith(n) for n in names_of(card))), None)
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