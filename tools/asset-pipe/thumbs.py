#!/usr/bin/env python3
"""Generate display-size thumbnails for the character art (plan: gallery weight).

The gallery / select / deck grids render the full-body stand art (582x910,
~105 KB each -- 5.6 MB for 54 characters) into 114x128 cards, and the board
tokens / avatars render the sd art (272x321, ~30 KB) at 44-92 px. This tool
resizes those sources once into small alpha WebPs:

  img/char/stand_<id>.webp      -> img/char/thumb_stand_<id>.webp      (300 px tall)
  img/char/sd_<id>.webp         -> img/char/thumb_sd_<id>.webp         (220 px tall)
  img/char/sd_<id>_happy.webp   -> img/char/thumb_sd_<id>_happy.webp   (220 px tall)

and records the new paths under `characters.<id>.thumb|sdThumb|sdHappyThumb` in
manifest.json, so the manifest stays the single source of paths. extract.py's
manifest writer calls build() after the img export, so a full re-extract keeps
the keys; this script is the fast standalone path (also patches the manifest).

Name plates (430x115 shown at 355 px) are already display-size: no thumb.

Usage:
  python thumbs.py             # generate what is missing/stale + patch manifest
  python thumbs.py --force     # regenerate everything
  python thumbs.py --no-manifest   # only write the thumb files
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:
    sys.exit("Pillow is required:  python -m pip install Pillow")

WEB = Path(__file__).resolve().parents[2]
OUT_DEFAULT = WEB / "webui" / "public" / "assets"

# manifest key -> (source kind, target height in px). Quality 85 keeps alpha and
# is visually indistinguishable at these sizes (the sources are 2-4x larger).
THUMBS: dict[str, tuple[str, int]] = {
    "thumb": ("stand", 300),
    "sdThumb": ("sd", 220),
    "sdHappyThumb": ("sdHappy", 220),
}
QUALITY = 85


def thumb_rel(src_rel: str) -> str:
    """`img/char/stand_001.webp` -> `img/char/thumb_stand_001.webp` (same dir)."""
    p = Path(src_rel)
    return str(p.with_name("thumb_" + p.name)).replace("\\", "/")


def make_thumb(src: Path, dest: Path, height: int) -> int:
    """Write `dest` as a `height`-px-tall WebP (alpha kept). Returns bytes written."""
    with Image.open(src) as im:
        im = im.convert("RGBA")
        w, h = im.size
        tw = max(1, round(w * height / h))
        if (tw, height) != (w, h):
            im = im.resize((tw, height), Image.LANCZOS)
        tmp = dest.with_suffix(".tmp")
        im.save(tmp, "WEBP", quality=QUALITY, method=4)  # method=6 is 80x slower for ~1 KB
    os.replace(tmp, dest)
    return dest.stat().st_size


def build(out: Path, characters: dict, force: bool = False, quiet: bool = False) -> dict[str, dict[str, str]]:
    """Generate thumbs for every character entry; return {id: {manifest key: rel}}.

    `characters` is manifest["characters"] (id -> kind -> rel path). Only reads it.
    """
    extra: dict[str, dict[str, str]] = {}
    made = skipped = stale = 0
    src_bytes = out_bytes = 0
    for cid, art in characters.items():
        if not isinstance(art, dict):
            continue
        for key, (kind, height) in THUMBS.items():
            src_rel = art.get(kind)
            if not src_rel:
                continue
            src = out / src_rel
            if not src.exists():
                print(f"   thumbs: missing source {src_rel}", file=sys.stderr)
                continue
            dest_rel = thumb_rel(src_rel)
            dest = out / dest_rel
            src_bytes += src.stat().st_size
            if dest.exists() and not force and dest.stat().st_mtime >= src.stat().st_mtime:
                skipped += 1
                out_bytes += dest.stat().st_size
            else:
                if dest.exists():
                    stale += 1
                try:
                    out_bytes += make_thumb(src, dest, height)
                except Exception as e:  # noqa: BLE001
                    print(f"   thumbs: {dest_rel}: {e}", file=sys.stderr)
                    continue
                made += 1
            extra.setdefault(cid, {})[key] = dest_rel
    if not quiet:
        print(f"== thumbs: {made} written ({stale} stale), {skipped} up to date, "
              f"{src_bytes / 1024:.0f} KB source -> {out_bytes / 1024:.0f} KB")
    return extra


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, default=OUT_DEFAULT)
    ap.add_argument("--force", action="store_true", help="regenerate even if up to date")
    ap.add_argument("--no-manifest", action="store_true", help="do not patch manifest.json")
    args = ap.parse_args()
    out: Path = args.out
    mf = out / "manifest.json"
    if not mf.exists():
        sys.exit(f"no manifest at {mf} -- run extract.py first")
    manifest = json.loads(mf.read_text(encoding="utf-8"))
    chars = manifest.get("characters") or {}
    extra = build(out, chars, force=args.force)
    if not args.no_manifest:
        for cid, e in extra.items():
            chars.setdefault(cid, {}).update(e)
        manifest["characters"] = chars
        mf.write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
        print(f"   manifest patched: {sum(len(e) for e in extra.values())} thumb paths")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())