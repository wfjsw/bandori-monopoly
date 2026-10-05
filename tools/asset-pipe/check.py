#!/usr/bin/env python3
"""Asset sanity check (plan: Verification #6).

Verifies that the extracted assets cover everything the original game loads:
  * every file the manifest points to exists
  * every card / character / band in game data has its art
  * every voice line, BGM track, SFX, boot image and animated background the C#
    code requests resolves through manifest["resources"]
  * every Live2D model compiles to model.json + its textures (tools/live2d/build.sh)

Keys are copied from the decompiled C# (file:line in comments). Exit status 1 on
any hard failure; warnings are printed but do not fail.

  python tools/asset-pipe/check.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

WEB = Path(__file__).resolve().parents[2]
ASSETS = WEB / "webui" / "public" / "assets"
DATA = WEB / "data"

# BgmPlayer.cs:10-17 (SceneTracks)
BGM_TRACKS = {"Boot": "21_Enjoy", "MainMenu": "21_Enjoy", "Lobby": "20_LiveLobby",
              "CharacterSelect": "23_BeforeLive", "Board": "12_Odekake", "CharacterGallery": "04_Nobiri"}
# BootController.cs:33-44 (BandKeys) -> Resources "BandoriBoot/<key>"
BOOT_KEYS = {"001", "002", "003", "004", "005", "018", "021", "045"}
# every Sfx.Play("<name>") in BandoriMonopoly.UI, plus TapFeedback.cs:85 "BandoriSE/tap"
SFX = {"place", "prompt", "coin_pay", "card_play", "buy", "teleport", "step", "mortgage",
       "event_card", "draw", "dice_roll", "dice_result", "card_cutin", "build", "tap"}
# AnimatedBackground.cs:64-71 -> BandoriAnimBg/<band>/{bg,small,big}; "common" has no bg
ANIM_LAYERS = ("bg", "small", "big")


def load(p: Path):
    return json.loads(p.read_text(encoding="utf-8-sig"))


def main() -> int:
    errors: list[str] = []
    warnings: list[str] = []
    m = load(ASSETS / "manifest.json")
    res = m["resources"]

    def need_res(path: str, why: str):
        if path.lower() not in res:
            errors.append(f"missing Resources/{path} ({why})")

    # 1. every manifest path exists
    def walk(v):
        if isinstance(v, str):
            yield v
        elif isinstance(v, dict):
            for x in v.values():
                yield from walk(x)
    files = [f for f in walk({k: v for k, v in m.items() if k != "version"}) if "/" in f]
    missing = [f for f in files if not (ASSETS / f).exists()]
    errors += [f"manifest points to missing file {f}" for f in missing]

    # 2. game data coverage
    cards = load(DATA / "cards.json")["cards"]
    no_art = [c["id"] for c in cards if c["id"] not in m["cards"]]
    errors += [f"card without art: {i}" for i in no_art]

    chars = load(DATA / "characters.json")["characters"]
    for c in chars:
        art_id = c.get("art") or c["cnId"]  # CharacterData.cs:34-38 (art ?? cnId)
        art = m["characters"].get(art_id)
        if not art:
            errors.append(f"character without any art: {c['name']} ({art_id})")
            continue
        for kind in ("stand", "sd"):
            if kind not in art:
                errors.append(f"character {c['name']} ({art_id}) missing {kind}")
        for kind in ("kv", "sdHappy", "namePlate"):
            if kind not in art:
                warnings.append(f"character {c['name']} ({art_id}) has no {kind}")

    band_names = {b["name"] for b in load(DATA / "bands.json")["bands"]}
    no_logo = sorted(band_names - set(m["bands"]))
    warnings += [f"band without logo in BandoriDatabase: {b}" for b in no_logo]

    # 3. what the code loads
    for scene, track in BGM_TRACKS.items():
        need_res(f"bandoribgm/{track}", f"BGM for scene {scene}")
    for key in BOOT_KEYS:
        need_res(f"bandoriboot/{key}", "boot screen")
    for name in SFX:
        need_res(f"bandorise/{name}", "Sfx.Play")
    need_res("bandoriui/transition_logo", "SceneFader")
    # folders only; "bandorianimbg/index" is a TextAsset, not a background
    for key in sorted({p.split("/")[1] for p in res if p.startswith("bandorianimbg/") and p.count("/") >= 2}):
        for layer in ANIM_LAYERS:
            if key == "common" and layer == "bg":
                continue
            need_res(f"bandorianimbg/{key}/{layer}", "AnimatedBackground")

    voice = load(DATA / "voice_lines.json")["characters"]
    n_lines = 0
    for c in voice:
        for line in c["lines"]:
            n_lines += 1
            need_res(f"bandorivoice/{c['id']}/{line['voice']}", f"voice line of {c['name']}")

    songs_idx = res.get("bandorisongs/index")
    n_songs = 0
    if not songs_idx:
        errors.append("missing Resources/BandoriSongs/index (CardSongs.cs:38)")
    else:
        idx = load(ASSETS / songs_idx)
        entries = idx if isinstance(idx, list) else next((v for v in idx.values() if isinstance(v, list)), [])
        for e in entries:
            clip = e.get("clip") if isinstance(e, dict) else None
            if clip:
                n_songs += 1
                need_res(f"bandorisongs/{clip}", "CardSongs")

    # 4. Live2D (compiled by tools/live2d/build.sh; see docs/LIVE2D.md)
    l2d = ASSETS / "live2d"
    catalog = load(l2d / "catalog.json")["models"]
    framing = {f["id"] for f in load(l2d / "framing.json")["models"]}
    ids = {e["id"] for e in catalog}
    for mid in sorted(ids):
        d = l2d / mid
        model = d / "model.json"
        if not model.is_file():
            errors.append(f"live2d {mid}: missing model.json (run tools/live2d/build.sh)")
            continue
        for tex in load(model).get("textures", []):
            if not (d / tex).is_file():
                errors.append(f"live2d {mid}: missing {tex}")
        for stray in d.iterdir():
            if stray.name != "model.json" and not re.fullmatch(r"texture_\d+\.png", stray.name):
                errors.append(f"live2d {mid}: unexpected {stray.name} (only model.json + textures may ship)")
    warnings += [f"framing.json entry {i} has no catalog model" for i in sorted(framing - ids)]

    # report
    print(f"manifest files      {len(files):5d}  missing {len(missing)}")
    print(f"cards with art      {len(cards) - len(no_art):5d} / {len(cards)}")
    print(f"characters          {len(chars):5d}   bands with logo {len(band_names) - len(no_logo)}/{len(band_names)}")
    print(f"voice lines         {n_lines:5d}   song clips {n_songs}")
    print(f"live2d models       {len(catalog):5d}")
    for w in warnings:
        print("  warn:", w)
    for e in errors:
        print("  FAIL:", e)
    print("OK" if not errors else f"{len(errors)} failure(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
