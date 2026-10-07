#!/usr/bin/env python3
"""Check that every card in rules/cards/, every tile in rules/tiles/ and every
event in rules/events/ cites its rule book passage.

The rule book (docs/rulebook/cards-sheet.csv -> cards.json via extract.py) is the
spec; a card's file quotes the passage (`//! > …`) and its code lines cite the
sentents they implement (`// 规则书: …`). A card the sheet omits must say so and
cite the C# class it was translated from instead.

Tile rules (rules/tiles/src/*.rs) have no `tile:*` ids in cards.json, so their
quotes live in the file header and must appear in `data/rules.txt` -- the
authoritative rulebook text (「基础[结算]规则」 + 「专有名词」) -- verbatim
(whitespace aside). Each file must also carry at least one `// 规则书: …`
citation line. `TODO(规则书)` notes do not exempt a file from quoting.

Event rules (rules/events/src/*.rs) are `event:*` ids with no cards.json entry
either; their quote is the event's `text` in `data/events.json` (the sheet's
中立事件 tab), matched the same way.

Usage: python tools/rulebook/check.py
"""

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BOOK = json.load(open(ROOT / "docs" / "rulebook" / "cards.json", encoding="utf-8"))
RULES = (ROOT / "data" / "rules.txt").read_text(encoding="utf-8")
EVENTS = {
    e["id"]: e["text"]
    for e in json.load(open(ROOT / "data" / "events.json", encoding="utf-8"))["events"]
}
CARD_ID = re.compile(r'(?:id: |CardDef::new\(\s*)"([^"]+)"')
CITES = re.compile(r"//+\s*规则书\s*[:：]")


def norm(text: str) -> str:
    return re.sub(r"\s+", "", text)


def quoted_text(src: str) -> str:
    """The passage a file quotes: its `//! > ` lines with the prefix removed."""
    return "".join(line[5:] for line in src.splitlines() if line.startswith("//! >"))


def quoted_passages(src: str):
    """Each run of consecutive `//! > ` lines is one quoted passage.

    A file may quote several distinct passages (e.g. 「基础[结算]规则」 plus a
    「专有名词」 glossary line); they are not contiguous in `data/rules.txt`, so
    each is checked on its own.
    """
    block = None
    for line in src.splitlines():
        if line.startswith("//! >"):
            if block is None:
                block = []
            block.append(line[5:])
        elif block is not None:
            yield "".join(block)
            block = None
    if block is not None:
        yield "".join(block)


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
    card_bad = bad
    print(f"{checked} cards checked, {card_bad} problem(s).")

    # Tile rules cite data/rules.txt from the file header (cards.json has no
    # `tile:*` ids). Every quoted passage must appear there; TODO(规则书) notes
    # do not excuse a missing or non-verbatim quote.
    tiles = 0
    rules_n = norm(RULES)
    for src in sorted(ROOT.glob("rules/tiles/src/*.rs")):
        if src.name == "lib.rs":
            continue
        text = src.read_text(encoding="utf-8")
        tiles += 1
        rel = src.relative_to(ROOT)
        passages = list(quoted_passages(text))
        if not passages:
            bad += 1
            print(f"{rel}: no rule book passage quoted (//! > ...)")
        for passage in passages:
            if norm(passage) not in rules_n:
                bad += 1
                print(f"{rel}: quote not in data/rules.txt: {norm(passage)[:60]}")
        if not CITES.search(text):
            bad += 1
            print(f"{rel}: quotes the passage but no code line cites it (// 规则书: ...)")
    print(f"{tiles} tiles checked, {bad - card_bad} problem(s).")

    # Event rules cite data/events.json from the file header (cards.json has no
    # `event:*` ids). The quote is the event's `text` field, matched the same
    # way a tile's quote is matched against data/rules.txt.
    events = 0
    ev_bad = bad
    for src in sorted(ROOT.glob("rules/events/src/*.rs")):
        if src.name in ("lib.rs", "util.rs"):
            continue
        text = src.read_text(encoding="utf-8")
        m = CARD_ID.search(text)
        if not m:
            continue
        events += 1
        rel = src.relative_to(ROOT)
        cid = m.group(1)
        if not cid.startswith("event:"):
            bad += 1
            print(f"{rel}: {cid} is not an event:* rule id")
            continue
        want = EVENTS.get(cid.removeprefix("event:"))
        if want is None:
            bad += 1
            print(f"{rel}: {cid} has no data/events.json entry")
            continue
        if norm(want) not in norm(quoted_text(text)):
            bad += 1
            print(f"{rel}: {cid} does not quote its event text")
        elif not CITES.search(text):
            bad += 1
            print(f"{rel}: {cid} quotes the text but no code line cites it (// 规则书: ...)")
    print(f"{events} events checked, {bad - ev_bad} problem(s).")
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())