"""Insert the rule book quote after the first doc line of each ported card, then
verify the quote is on disk. Re-runnable: removes an existing quote block first."""
import json
import re
from pathlib import Path

CARDS = Path("rules/cards")
BOOK = json.load(open("docs/rulebook/cards.json", encoding="utf-8"))
# the card id lives in `CardDef::new("BAND:Name", ...)`, sometimes also in a
# `const ID: &str = "..."` or the older `id: "..."` form
ID_LINES = [
    re.compile(r'CardDef::new\(\s*"([^"]+)"'),
    re.compile(r'const ID: &str = "([^"]+)"'),
    re.compile(r'id: "([^"]+)"'),
]
QUOTE_HEAD = "//! 规则书（docs/rulebook/cards.json, id `"  # marker of a quote block


def quote_block(cid: str, text: str) -> list[str]:
    out = ["//!", f"//! 规则书（docs/rulebook/cards.json, id `{cid}`）:"]
    for line in text.split("\n"):
        out.append(f"//! > {line}" if line else "//!")
    out.append("//!")
    return out


def drop_quote_block(lines: list[str]) -> list[str]:
    """Remove one existing quote block (and its bare `//!` markers) so the run is
    idempotent. Only the contiguous block is touched -- other bare `//!` lines in
    the module doc stay put."""
    start = next((i for i, l in enumerate(lines) if l.startswith(QUOTE_HEAD)), None)
    if start is None:
        return lines
    b = start - 1 if start > 0 and lines[start - 1].strip() == "//!" else start
    e = start + 1
    while e < len(lines) and (lines[e].startswith("//! >") or lines[e].strip() == "//!"):
        e += 1
    return lines[:b] + lines[e:]


def main() -> int:
    problems = []
    for src in sorted(CARDS.glob("card-*/src/*.rs")):
        if src.name == "lib.rs":
            continue
        lines = src.read_text(encoding="utf-8").split("\n")
        text = "\n".join(lines)
        cid = next((m.group(1) for pat in ID_LINES if (m := pat.search(text))), None)
        if not cid:
            continue
        keep = drop_quote_block(lines)
        book = BOOK.get(cid)
        if book is None:
            # the sheet is not exhaustive: a card it omits cites the C# instead
            print(f"note    {src}: {cid} is not in docs/rulebook/cards.json (C# citation required)")
            continue
        block = quote_block(cid, book)
        out = keep[:1] + block + keep[1:]
        src.write_text("\n".join(out), encoding="utf-8")
        check = src.read_text(encoding="utf-8")
        ok = all(l in check for l in block)
        print(("ok      " if ok else "MISSING ") + str(src) + f" ({cid})")
        if not ok:
            problems.append(f"{src}: quote did not land")
    for p in problems:
        print(p)
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(main())