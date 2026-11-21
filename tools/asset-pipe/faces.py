#!/usr/bin/env python3
"""Bake square, face-centered avatar crops from the SD art.

The SD art frames every character differently (face height, body width, pose),
so a fixed CSS crop (`left: 50%, top: -4%, width: 150%`) shows some faces cut
off and others lost in empty space. The crops here are computed per character
from a face anchor, so every avatar puts the head at the same place and scale:

  img/char/sd_<id>.webp -> img/char/avatar_<id>.webp (192x192 WebP)

The anchors live in `faces.json` beside this script (tracked, display data):
id -> [cx, cy, side, score] in raw source pixels. `--detect` fills missing ids
with the YOLOv3 anime-face detector (anime-face-detector, CPU), and hand edits
survive because detection only touches missing ids unless --force is given.
Regenerating the crops needs only Pillow; the detector is optional.

Usage:
  python faces.py --detect     # detect missing anchors, then build + patch manifest
  python faces.py              # rebuild crops from faces.json
  python faces.py --force      # re-detect every id (hand edits are lost)
  python faces.py --sheet /tmp/avatars.png   # contact sheet for review
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

try:
    from PIL import Image, ImageDraw
except ImportError:
    sys.exit("Pillow is required:  python -m pip install Pillow")

WEB = Path(__file__).resolve().parents[2]
OUT_DEFAULT = WEB / "webui" / "public" / "assets"
ANCHORS = Path(__file__).with_name("faces.json")

SIZE = 192          # 92 px rendered at 2x DPR
QUALITY = 90
# The detected box covers hair + face. 1.5x puts the head at ~2/3 of the frame
# with the chin and shoulders in it; the clamps keep very narrow / very wide
# detections from zooming the crop into the face or out to the whole body.
SCALE = 1.5
# Landmark spread is much narrower than the head (eyes/mouth/contour only).
LANDMARK_SCALE = 1.6
MIN_FRAC = 0.35
MAX_FRAC = 0.80
# Eyes sit slightly below the box center; nudging the crop down keeps the
# forehead from eating the frame.
DROP = 0.02


def crop_box(anchor: list[float], w: int, h: int) -> tuple[int, int, int]:
    """Anchor -> (left, top, side), clamped inside the source."""
    cx, cy, side = anchor[:3]
    side = min(max(side, w * MIN_FRAC), w * MAX_FRAC, w, h)
    left = min(max(cx - side / 2, 0), w - side)
    top = min(max(cy + side * DROP - side / 2, 0), h - side)
    return round(left), round(top), round(side)


def make_avatar(src: Path, dest: Path, anchor: list[float]) -> int:
    """Write the face-centered crop at `dest`; returns bytes written."""
    with Image.open(src) as im:
        im = im.convert("RGBA")
        left, top, side = crop_box(anchor, *im.size)
        im = im.crop((left, top, left + side, top + side)).resize((SIZE, SIZE), Image.LANCZOS)
        tmp = dest.with_suffix(".tmp")
        im.save(tmp, "WEBP", quality=QUALITY, method=4)
    os.replace(tmp, dest)
    return dest.stat().st_size


_WORKER_DET = None


def _init_worker() -> None:
    global _WORKER_DET
    from anime_face_detector import create_detector  # only needed for --detect

    _WORKER_DET = create_detector("yolov3", device="cpu", flip_test=False)


def _detect_one(item: tuple[str, Path]) -> tuple[str, list[float] | None]:
    import numpy as np
    from PIL import Image

    cid, src = item
    try:
        with Image.open(src) as im:
            hits = _WORKER_DET(np.array(im.convert("RGB")))
    except Exception as e:  # noqa: BLE001
        print(f"   faces: {cid}: {e}", file=sys.stderr)
        return cid, None
    if not hits:
        return cid, None
    best = max(hits, key=lambda f: float(f["bbox"][4]))
    b = best["bbox"]
    # The YOLO box is hair-inclusive and occasionally drifts onto props; the
    # facial landmarks (eyes, nose, mouth, contour) are what should sit at the
    # centre, so they win whenever the model is confident enough.
    kp = [(float(p[0]), float(p[1])) for p in best["keypoints"] if float(p[2]) >= 0.3]
    hi = [(float(p[0]), float(p[1])) for p in best["keypoints"] if float(p[2]) >= 0.5]
    if len(hi) >= 4:
        xs = sorted(p[0] for p in kp)
        ys = sorted(p[1] for p in kp)
        cx, cy = xs[len(xs) // 2], ys[len(ys) // 2]
        hx = sorted(p[0] for p in hi)
        side = (hx[-1] - hx[0]) * LANDMARK_SCALE
    else:
        cx, cy = (float(b[0]) + float(b[2])) / 2, (float(b[1]) + float(b[3])) / 2
        side = (float(b[2]) - float(b[0])) * SCALE
    return cid, [round(cx, 1), round(cy, 1), round(side, 1), round(float(b[4]), 3)]


def detect(paths: dict[str, Path], anchors: dict[str, list[float]], force: bool, jobs: int = 1) -> None:
    from concurrent.futures import ProcessPoolExecutor

    todo = sorted((i, p) for i, p in paths.items() if force or i not in anchors)
    if not todo:
        return
    jobs = max(1, min(jobs, len(todo)))
    done = 0
    with ProcessPoolExecutor(max_workers=jobs, initializer=_init_worker) as pool:
        for cid, anchor in pool.map(_detect_one, todo):
            done += 1
            print(f"   faces: {done}/{len(todo)} {cid}: {'hit' if anchor else 'no face'}", file=sys.stderr)
            if anchor:
                anchors[cid] = anchor
                ANCHORS.write_text(json.dumps(anchors, indent=1, sort_keys=True), encoding="utf-8")


def build(out: Path, characters: dict, anchors: dict[str, list[float]], force: bool, quiet: bool) -> dict[str, dict[str, str]]:
    """Write an avatar per anchored character; return {id: {"avatar": rel}}."""
    extra: dict[str, dict[str, str]] = {}
    made = skipped = missing = 0
    for cid, art in characters.items():
        if not isinstance(art, dict) or not art.get("sd"):
            continue
        anchor = anchors.get(cid)
        if not anchor:
            missing += 1
            continue
        src_rel = art["sd"]
        dest_rel = str(Path(src_rel).with_name("avatar_" + Path(src_rel).name)).replace("\\", "/")
        src, dest = out / src_rel, out / dest_rel
        if not src.exists():
            print(f"   faces: missing source {src_rel}", file=sys.stderr)
            continue
        if dest.exists() and not force and dest.stat().st_mtime >= src.stat().st_mtime:
            skipped += 1
        else:
            make_avatar(src, dest, anchor)
            made += 1
        extra.setdefault(cid, {})["avatar"] = dest_rel
    if not quiet:
        print(f"== faces: {made} written, {skipped} up to date, {missing} without an anchor")
    return extra


def sheet(out: Path, extra: dict[str, dict[str, str]], dest: Path) -> None:
    """Contact sheet of every written avatar, for eyeballing the fits."""
    items = sorted(extra.items())
    cell, cols = 96, 9
    rows = (len(items) + cols - 1) // cols
    im = Image.new("RGB", (cols * cell, rows * cell), "white")
    for n, (cid, e) in enumerate(items):
        with Image.open(out / e["avatar"]) as a:
            tile = a.convert("RGBA").resize((cell, cell))
        bg = Image.new("RGBA", (cell, cell), "white")
        bg.alpha_composite(tile)
        ImageDraw.Draw(bg).text((4, 2), cid, fill="black")
        im.paste(bg.convert("RGB"), ((n % cols) * cell, (n // cols) * cell))
    dest.parent.mkdir(parents=True, exist_ok=True)
    im.save(dest)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", type=Path, default=OUT_DEFAULT)
    ap.add_argument("--detect", action="store_true", help="detect anchors for ids that have none")
    ap.add_argument("--jobs", type=int, default=4, help="detector worker processes (default 4)")
    ap.add_argument("--force", action="store_true", help="regenerate everything (re-detect with --detect)")
    ap.add_argument("--no-manifest", action="store_true", help="do not patch manifest.json")
    ap.add_argument("--sheet", type=Path, help="write a contact sheet of the result here")
    args = ap.parse_args()
    out: Path = args.out
    mf = out / "manifest.json"
    if not mf.exists():
        sys.exit(f"no manifest at {mf} -- run extract.py first")
    manifest = json.loads(mf.read_text(encoding="utf-8"))
    chars = manifest.get("personas") or {}
    anchors = json.loads(ANCHORS.read_text(encoding="utf-8")) if ANCHORS.exists() else {}
    if args.detect:
        detect({cid: out / a["sd"] for cid, a in chars.items() if isinstance(a, dict) and a.get("sd")}, anchors, args.force, args.jobs)
    if not anchors:
        sys.exit("no anchors -- run with --detect")
    extra = build(out, chars, anchors, args.force, quiet=False)
    if not args.no_manifest:
        for cid, e in extra.items():
            chars.setdefault(cid, {}).update(e)
        manifest["personas"] = chars
        mf.write_text(json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8")
        print(f"   manifest patched: {sum(len(e) for e in extra.values())} avatar paths")
    if args.sheet:
        sheet(out, extra, args.sheet)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
