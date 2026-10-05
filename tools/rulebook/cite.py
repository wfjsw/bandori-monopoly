"""Insert the rule book quote after the first doc line of each ported card, then
verify the quote is on disk. Re-runnable: removes an existing quote block first."""
import json
import re
from pathlib import Path

CARDS = Path("rules/cards")
BOOK = json.load(open("docs/rulebook/cards.json", encoding="utf-8"))
ID_LINE = re.compile(r'id: "([^"]+)"')
QUOTE_HEAD = "//! 规则书（docs/rulebook/cards.json, id `"  # marker of a quote block


def quote_block(cid: str, text: str) -> list[str]:
    out = ["//!", f"//! 规则书（docs/rulebook/cards.json, id `{cid}`）:"]
    for line in text.split("\n"):
        out.append(f"//! > {line}" if line else "//!")
    out.append("//!")
    return out


def main() -> int:
    problems = []
    for src in sorted(CARDS.glob("card-*/src/*.rs")):
        if src.name == "lib.rs":
            continue
        lines = src.read_text(encoding="utf-8").split("\n")
        m = next((ID_LINE.search(l) for l in lines if 'id: "' in l), None)
        if not m:
            continue
        cid = m.group(1)
        # drop a previous quote block (quote lines + its markers) so the run is idempotent
        keep = [
            l for l in lines
            if not l.startswith(QUOTE_HEAD) and not l.startswith("//! >") and l.strip() != "//!"
        ]
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