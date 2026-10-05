#!/usr/bin/env python3
"""Count / list source lines that still carry CJK text (i18n audit).

  python tools/i18n/scan.py            # summary per area and top files
  python tools/i18n/scan.py --list     # every offending line
"""

import glob
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CJK = re.compile("[㐀-鿿＀-￯　-〿]")
AREAS = {
    "webui": "webui/src/**/*.ts*",
    "crates": "crates/**/*.rs",
    "rules": "rules/**/*.rs",
}


def code_lines(path: Path):
    """Lines with CJK outside comments (rough: drops // ... and /// ... tails)."""
    for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.strip()
        if stripped.startswith(("//", "*", "/*", "#[doc")):
            continue
        code = re.sub(r"\s//.*$", "", line)
        if CJK.search(code):
            yield no, line.strip()


def main() -> None:
    listing = "--list" in sys.argv
    for area, pattern in AREAS.items():
        files = [Path(p) for p in glob.glob(str(ROOT / pattern), recursive=True) if "target" not in p and "node_modules" not in p]
        per = {}
        for f in files:
            hits = list(code_lines(f))
            if hits:
                per[f] = hits
        total = sum(len(v) for v in per.values())
        print(f"{area}: {total} lines in {len(per)} files")
        for f, hits in sorted(per.items(), key=lambda x: -len(x[1]))[: None if listing else 15]:
            print(f"  {len(hits):5}  {f.relative_to(ROOT)}")
            if listing:
                for no, text in hits:
                    print(f"         {no}: {text[:140]}")


if __name__ == "__main__":
    main()
