#!/usr/bin/env python3
"""Check that every card in rules/cards/ cites its rule book passage.

The rule book (docs/rulebook/cards-sheet.csv -> cards.json via extract.py) is the
spec; a card's file quotes the passage (`//! > …`) and its code lines cite the
sentents they implement (`// 规则书: …`). A card the sheet omits must say so and
cite the C# class it was translated from instead.

Usage: python tools/rulebook/check.py
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BOOK = json.load(open(ROOT / "docs" / "rulebook" / "cards.json", encoding="utf-8"))
CARD_ID = re.compile(r'(?:id: |CardDef::new\()"([^"]+)"')


def norm(text: str) -> str:
    return re.sub(r"\s+", "", text)


def quoted_text(src: str) -> str:
    """The passage a file quotes: its `//! > ` lines with the prefix removed."""
    return "".join(line[5:] for line in src.splitlines() if line.startswith("//! >"))


def main() -> int:
    bad = 0
    checked = 0
    for src in sorted(ROOT.glob("rules/cards/*/src/*.rs")):
        if src.name == "lib.rs":
            continue
        text = src.read_text(encoding="utf-8")
        m = CARD_ID.search(text)
        if not m:
            continue
        checked += 1
        cid = m.group(1)
        rel = src.relative_to(ROOT)
        book = BOOK.get(cid)
        if book is None:
            # the sheet is the spec but not exhaustive: a card it omits must cite
            # the C# class it was translated from instead
            if "C#" not in text:
                bad += 1
                print(f"{rel}: {cid} has no rule book entry and no C# citation")
            continue
        if norm(book) not in norm(quoted_text(text)):
            bad += 1
            print(f"{rel}: {cid} does not quote its rule book passage")
        elif "规则书" not in text:
            bad += 1
            print(f"{rel}: {cid} quotes the passage but no code line cites it (// 规则书: ...)")
    print(f"{checked} cards checked, {bad} problem(s).")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())