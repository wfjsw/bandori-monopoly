#!/usr/bin/env python3
"""Subset the CJK webfonts to the glyphs the app actually renders.

The two Resource Han Rounded CN TTFs are ~14 MB each (28 MB of the 192 MB
dist) and ship every glyph in the font. The app renders a fixed corpus:
UI strings (webui/src/i18n/locales) and game data (web/data). Subsetting to
that corpus drops the pair to a few hundred KB. Files are replaced in place,
so manifest paths and `@font-face` in webui/src/core/assets.ts are unchanged.

Uses a venv (per project rule):

  python -m venv web/.venv
  web/.venv/Scripts/python -m pip install fonttools brotli
  web/.venv/Scripts/python tools/asset-pipe/fonts.py

Re-run after `extract.py --force --only fonts` (which restores full TTFs), or
whenever data/ or the locales gain new text. `--check` reports sizes only.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FONTS = ROOT / "webui" / "public" / "assets" / "fonts"
LOCALES = ROOT / "webui" / "src" / "i18n" / "locales"
DATA = ROOT / "data"

TARGETS = [
    "ResourceHanRoundedCN-Medium.ttf",
    "ResourceHanRoundedCN-Bold.ttf",
]

# Always keep printable ASCII, CJK punctuation and the format characters the
# message formatter may emit.
ALWAYS = (
    "".join(chr(c) for c in range(0x20, 0x7F))
    + "，。、；：！？（）【】《》「」『』…—·×÷±＝￥℃‰"
    + "‐‑‒–—―‘’“”„†‡•…‰′″‹›⁄€™"
    + " ​﻿"
)


def used_chars() -> set[str]:
    out = set(ALWAYS)
    for pattern in (LOCALES.glob("*/*.json"), DATA.glob("*.json"), DATA.glob("*.txt")):
        for path in pattern:
            out |= set(path.read_text(encoding="utf-8", errors="replace"))
    # drop control chars; keep everything printable the app might show
    return {c for c in out if c.isprintable() or c in " ​"}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="report sizes, change nothing")
    args = ap.parse_args()

    try:
        from fontTools import subset
    except ImportError:
        print("fonttools missing -- run this with web/.venv/Scripts/python")
        return 2

    chars = used_chars()
    print(f"glyph corpus: {len(chars)} characters")

    # write the corpus once so fontTools can read it (unicodes flag is fine too)
    changed = 0
    for name in TARGETS:
        path = FONTS / name
        if not path.exists():
            print(f"skip {name}: not found")
            continue
        before = path.stat().st_size
        if args.check:
            print(f"  {name}: {before / 1e6:.2f} MB (full)")
            continue

        opts = subset.Options()
        opts.layout_features = ["*"]  # keep kerning / vert features
        opts.name_IDs = ["*"]
        opts.name_languages = ["*"]
        opts.notdef_outline = True
        opts.recommended_glyphs = True
        opts.drop_tables += ["DSIG"]
        font = subset.load_font(str(path), opts)
        subsetter = subset.Subsetter(options=opts)
        subsetter.populate(text="".join(sorted(chars)))
        subsetter.subset(font)
        subset.save_font(font, str(path), opts)
        after = path.stat().st_size
        changed += 1
        print(f"  {name}: {before / 1e6:.2f} MB -> {after / 1e6:.2f} MB "
              f"(-{100 * (1 - after / before):.0f}%)")

    if args.check:
        return 0
    print(f"subset {changed} font(s). Re-run extract.py --only fonts --force to restore.")
    return 0


if __name__ == "__main__":
    sys.exit(main())