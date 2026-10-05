#!/usr/bin/env python3
"""Extract web-ready assets from the Unity build (plan phase P1).

Input : D:/BanG Dream 大富翁/BandoriMonopoly_Data
Output: web/webui/public/assets/   (images, audio, fonts, live2d, text, manifest.json)
        web/data/                  (game-data JSON for the server and game-core)

  Resources/...      -> keyed by their original Resources path (lower-case, as Unity
                        stores it), e.g. "bandoribgm/21_enjoy" -> audio/bandoribgm/21_enjoy.ogg
  BandoriDatabase    -> character / band / room / card / tap-fx art, keyed semantically
  everything else    -> img/scene/<name>.webp, audio/scene/<name>.ogg

Formats: images WebP (quality 92, alpha kept), audio Ogg/Opus, fonts raw TTF/OTF.

Usage:
  python extract.py                       # everything (skips outputs that already exist)
  python extract.py --only img,audio      # subset: img,audio,fonts,text,live2d,data
  python extract.py --force --jobs 12
  python extract.py --probe               # P0 spike: one texture + one audio clip
"""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import json
import os
import shutil
import struct
import subprocess
import sys
import threading
import time
from concurrent.futures import Future, ThreadPoolExecutor
from pathlib import Path

try:
    import UnityPy
except ImportError:
    sys.exit("UnityPy is required:  python -m pip install UnityPy")

sys.path.insert(0, str(Path(__file__).parent))
from unitydb import CHAR_SPRITES, Resolver, read_database  # noqa: E402

# The game build to extract from; the upstream ships a dated folder
# ("BanG Dream 大富翁10-04") alongside the old one -- override with GAME_DATA_DIR.
BUILD = Path(os.environ.get("GAME_DATA_DIR", r"D:/BanG Dream 大富翁/BanG Dream 大富翁10-04/BandoriMonopoly_Data"))
GAME_DATA = Path(r"D:/BanG Dream 大富翁/源码导出_SourceExport/game-data")
WEB = Path(__file__).resolve().parents[2]
OUT_DEFAULT = WEB / "webui" / "public" / "assets"
DATA_OUT = WEB / "data"

# Opus bitrate (kbit/s) and channel count per Resources family.
AUDIO_PROFILE = {
    "bandoribgm": (96, 2),
    "bandorisongs": (96, 2),
    "bandorivoice": (32, 1),
    "bandorise": (64, 2),
}
AUDIO_DEFAULT = (64, 2)
WEBP_QUALITY = 92

# TextMeshPro SDF atlases/materials, shaders, UI style sheets: engine plumbing. The web
# client renders text with the real TTFs instead.
SKIP_RESOURCE_FAMILIES = {"fonts & materials", "shader", "style sheets"}

# Engine/editor leftovers that are not game art.
SKIP_IMAGE_PREFIXES = ("Splash Screen", "UnitySplash", "Font Texture", "Default-", "UnityWatermark", "Checkmark", "UISprite", "Knob", "Background", "InputFieldBackground", "DropdownArrow")


def find_ffmpeg() -> str:
    exe = shutil.which("ffmpeg")
    if exe:
        return exe
    winget = Path(os.environ.get("LOCALAPPDATA", "")) / "Microsoft" / "WinGet" / "Links" / "ffmpeg.exe"
    if winget.exists():
        return str(winget)
    sys.exit("ffmpeg (with libopus) is required for audio")


def is_engine_image(name: str) -> bool:
    """Unity/TextMeshPro plumbing, not game art (TMP glyph atlases end in 'SDF Atlas')."""
    return not name or name.startswith(SKIP_IMAGE_PREFIXES) or " SDF" in name


def safe(name: str) -> str:
    """File-system-safe file name (keeps CJK)."""
    out = "".join("_" if c in '<>:"/\\|?*' or ord(c) < 32 else c for c in name).strip().rstrip(".")
    return out or "_"


class Stats:
    def __init__(self):
        self.lock = threading.Lock()
        self.counts: dict[str, int] = {}
        self.bytes: dict[str, int] = {}
        self.skipped = 0
        self.errors: list[str] = []

    def add(self, cat: str, path: Path):
        with self.lock:
            self.counts[cat] = self.counts.get(cat, 0) + 1
            self.bytes[cat] = self.bytes.get(cat, 0) + (path.stat().st_size if path.exists() else 0)

    def error(self, msg: str):
        with self.lock:
            self.errors.append(msg)


class Pipeline:
    def __init__(self, out: Path, only: set[str], force: bool, jobs: int):
        self.out, self.only, self.force = out, only, force
        self.pool = ThreadPoolExecutor(max_workers=jobs)
        self.pending: list[Future] = []
        self.max_pending = jobs * 3
        self.stats = Stats()
        self.ffmpeg = find_ffmpeg() if "audio" in only else None
        self.manifest: dict = {
            "version": 1,
            "resources": {},  # lower-case Resources path -> asset file
            "characters": {}, "bands": {}, "rooms": {}, "cards": {}, "fx": {},
            "scene": {"img": {}, "audio": {}},
            "fonts": {},
            "live2d": None,
        }
        self.done: set[tuple[str, int]] = set()  # objects already exported
        self.written: dict[str, str] = {}        # rel path -> content hash (dedup scene names)

    # -- scheduling ---------------------------------------------------------------
    def submit(self, fn, *args):
        self.pending.append(self.pool.submit(fn, *args))
        if len(self.pending) >= self.max_pending:
            self.drain(keep=self.max_pending // 2)

    def drain(self, keep: int = 0):
        while len(self.pending) > keep:
            f = self.pending.pop(0)
            try:
                f.result()
            except Exception as e:  # noqa: BLE001
                self.stats.error(str(e))

    def want(self, rel: str) -> bool:
        p = self.out / rel
        if p.exists() and not self.force:
            self.stats.skipped += 1
            return False
        p.parent.mkdir(parents=True, exist_ok=True)
        return True

    # -- encoders (run in the pool) -----------------------------------------------
    def _save_webp(self, img, rel: str, cat: str):
        dest = self.out / rel
        tmp = dest.with_suffix(".tmp")
        img.save(tmp, "WEBP", quality=WEBP_QUALITY, method=4)
        os.replace(tmp, dest)
        self.stats.add(cat, dest)

    def _encode_opus(self, wav: bytes, rel: str, kbps: int, channels: int, cat: str):
        dest = self.out / rel
        tmp = dest.with_name(dest.stem + ".tmp.ogg")
        cmd = [self.ffmpeg, "-hide_banner", "-loglevel", "error", "-y", "-f", "wav", "-i", "pipe:0",
               "-c:a", "libopus", "-b:a", f"{kbps}k", "-vbr", "on", "-ac", str(channels), str(tmp)]
        r = subprocess.run(cmd, input=wav, capture_output=True)
        if r.returncode != 0:
            raise RuntimeError(f"ffmpeg failed for {rel}: {r.stderr.decode(errors='replace')[:200]}")
        os.replace(tmp, dest)
        self.stats.add(cat, dest)

    # -- exporters (main thread: decoding) ----------------------------------------
    def image(self, obj, rel: str, cat: str) -> str | None:
        """Export a Sprite/Texture2D to `rel` (.webp). Returns rel or None."""
        key = (obj.assets_file.name, obj.path_id)
        if "img" not in self.only:
            return rel
        self.done.add(key)
        if not self.want(rel):
            return rel
        try:
            img = obj.read().image
        except Exception as e:  # noqa: BLE001
            self.stats.error(f"{rel}: decode failed: {e}")
            return None
        self.submit(self._save_webp, img, rel, cat)
        return rel

    def audio(self, obj, rel: str, family: str, cat: str) -> str | None:
        key = (obj.assets_file.name, obj.path_id)
        if "audio" not in self.only:
            return rel
        self.done.add(key)
        if not self.want(rel):
            return rel
        try:
            samples = obj.read().samples
        except Exception as e:  # noqa: BLE001
            self.stats.error(f"{rel}: decode failed: {e}")
            return None
        if not samples:
            self.stats.error(f"{rel}: no samples")
            return None
        wav = next(iter(samples.values()))
        kbps, ch = AUDIO_PROFILE.get(family, AUDIO_DEFAULT)
        self.submit(self._encode_opus, wav, rel, kbps, ch, cat)
        return rel

    def text(self, obj, rel_base: str) -> str | None:
        if "text" not in self.only:
            return None
        d = obj.read()
        script = d.m_Script
        data = script.encode("utf-8", "surrogateescape") if isinstance(script, str) else bytes(script)
        ext = ".json" if data.lstrip(b"\xef\xbb\xbf \r\n\t").startswith((b"{", b"[")) else ".txt"
        rel = rel_base + ext
        self.done.add((obj.assets_file.name, obj.path_id))
        if self.want(rel):
            (self.out / rel).write_bytes(data)
            self.stats.add("text", self.out / rel)
        return rel


def run(args) -> int:
    t0 = time.time()
    only = set(args.only.split(",")) if args.only else {"img", "audio", "fonts", "text", "live2d", "data"}
    out: Path = args.out
    out.mkdir(parents=True, exist_ok=True)
    p = Pipeline(out, only, args.force, args.jobs)

    print(f"== loading {BUILD}")
    env = UnityPy.load(str(BUILD))
    res = Resolver(env)
    ggm = next(o for o in env.objects if o.type.name == "ResourceManager")
    print(f"   {len(env.objects)} objects")

    # 1) Resources/ -- keep the original path as the key
    container = ggm.read_typetree()["m_Container"]
    print(f"== Resources: {len(container)} entries")
    from unitydb import PPtr
    for path, ptr in container:
        obj = res.resolve(ggm, PPtr(ptr["m_FileID"], ptr["m_PathID"]))
        if obj is None:
            p.stats.error(f"unresolved Resources entry {path}")
            continue
        family = path.split("/")[0]
        if family in SKIP_RESOURCE_FAMILIES:
            continue
        t = obj.type.name
        if t in ("Texture2D", "Sprite"):
            rel = p.image(obj, f"img/res/{path}.webp", "img:resources")
        elif t == "AudioClip":
            rel = p.audio(obj, f"audio/{path}.ogg", family, f"audio:{family}")
        elif t == "TextAsset":
            rel = p.text(obj, f"text/{path}")
        else:
            continue  # BandoriDatabase, shaders, TMP materials, style sheets
        if rel:
            p.manifest["resources"][path] = rel

    # 2) BandoriDatabase -- semantic art (skipped by --only data, which needs none)
    db_obj = None
    if "img" in only:
        db_obj = next((o for o in env.objects if o.type.name == "MonoBehaviour"
                       and len(o.get_raw_data()) > 2000 and _is_db(o)), None)
        if db_obj is None:
            print("   img: no BandoriDatabase matched -- the build's db layout changed", file=sys.stderr)
    if db_obj is not None:
        _, db = read_database(db_obj.get_raw_data())
        print(f"== BandoriDatabase: {len(db.character_art)} characters, {len(db.card_art)} cards")

        def art(ptr, rel_dir):
            o = res.resolve(db_obj, ptr)
            if o is None:
                return None
            return p.image(o, f"{rel_dir}/{safe(o.read().m_Name)}.webp", "img:" + rel_dir.split("/")[1])

        for c in db.character_art:
            entry = {k: art(c[k], "img/char") for k in CHAR_SPRITES}
            p.manifest["characters"][c["id"]] = {k: v for k, v in entry.items() if v}
        for b in db.band_art:
            if (rel := art(b["logo"], "img/band")):
                p.manifest["bands"][b["band"]] = rel
        for i, r in enumerate(db.room_art):
            if (rel := art(r["card"], "img/room")):
                p.manifest["rooms"][r["band"] or f"default{i}"] = rel
        for c in db.card_art:
            if (rel := art(c["art"], "img/card")):
                p.manifest["cards"][c["id"]] = rel
        for k, ptr in db.fx.items():
            if (rel := art(ptr, "img/fx")):
                p.manifest["fx"][k] = rel

    # 3) everything else the scenes reference
    print("== scene art + audio")
    for obj in env.objects:
        key = (obj.assets_file.name, obj.path_id)
        if key in p.done:
            continue
        t = obj.type.name
        if t == "Sprite":
            name = obj.read().m_Name
            if is_engine_image(name):
                continue
            rel = _unique(p, f"img/scene/{safe(name)}", ".webp")
            if (r := p.image(obj, rel, "img:scene")):
                p.manifest["scene"]["img"][name] = r
        elif t == "AudioClip":
            name = obj.read().m_Name
            rel = _unique(p, f"audio/scene/{safe(name)}", ".ogg")
            if (r := p.audio(obj, rel, "scene", "audio:scene")):
                p.manifest["scene"]["audio"][name] = r
    # bare textures that no sprite was cut from (e.g. backgrounds used as RawImage)
    sprite_textures: set[tuple[str, int]] = set()
    for obj in env.objects:
        if obj.type.name == "Sprite":
            try:
                tex = obj.read().m_RD.texture.deref()
                sprite_textures.add((tex.assets_file.name, tex.path_id))
            except Exception:  # noqa: BLE001
                pass
    for obj in env.objects:
        if obj.type.name != "Texture2D" or (obj.assets_file.name, obj.path_id) in p.done:
            continue
        if (obj.assets_file.name, obj.path_id) in sprite_textures:
            continue
        d = obj.read()
        name = d.m_Name
        if is_engine_image(name) or "Alpha8" in str(d.m_TextureFormat):
            continue  # TMP SDF atlases etc.
        rel = _unique(p, f"img/scene/{safe(name)}", ".webp")
        if (r := p.image(obj, rel, "img:scene")):
            p.manifest["scene"]["img"][name] = r

    # 4) fonts
    if "fonts" in only:
        for obj in env.objects:
            if obj.type.name != "Font":
                continue
            d = obj.read()
            data = bytes(d.m_FontData) if d.m_FontData else b""
            if len(data) < 1024:
                continue
            ext = ".otf" if data[:4] == b"OTTO" else ".ttf"
            rel = f"fonts/{safe(d.m_Name)}{ext}"
            if p.want(rel):
                (out / rel).write_bytes(data)
                p.stats.add("fonts", out / rel)
            p.manifest["fonts"][d.m_Name] = rel

    # 5) Live2D -- catalog/framing metadata only. The models are compiled from
    # web/live2d/<id>.cxx3 by tools/live2d/build.sh into out/live2d/<id>/
    # {model.json,texture_<n>.png} and never copied raw (model.moc, physics and
    # motions are not web assets and main.xml is ~5 MB per model).
    if "live2d" in only:
        src = BUILD / "StreamingAssets" / "BandoriLive2D"
        dst = out / "live2d"
        dst.mkdir(parents=True, exist_ok=True)
        for name in ("catalog.json", "framing.json"):
            shutil.copy2(src / name, dst / name)
        p.manifest["live2d"] = {"catalog": "live2d/catalog.json", "framing": "live2d/framing.json"}
        p.stats.counts["live2d"] = sum(1 for _ in dst.rglob("*") if _.is_file())
        p.stats.bytes["live2d"] = sum(f.stat().st_size for f in dst.rglob("*") if f.is_file())
        if not any(dst.glob("*/model.json")):
            print("   live2d: no compiled models yet -- run tools/live2d/build.sh")

    # 6) game data for server / game-core
    if "data" in only:
        DATA_OUT.mkdir(parents=True, exist_ok=True)
        # The source export is the base; web/data is extended by hand (bot
        # names, presets, ...). Copy only files we do not have -- overwriting
        # would silently drop those extensions.
        for f in GAME_DATA.iterdir():
            if f.suffix not in (".json", ".txt") or f.name.startswith("LineBreaking"):
                continue
            dst = DATA_OUT / f.name
            if dst.exists():
                print(f"   data: keeping the extended {f.name} (the export is the base)")
                continue
            shutil.copy2(f, dst)
            p.stats.add("data", dst)
        # 6b) the build's `skill_simple` TextAsset (C# GameDataLoader.WithSimple):
        # a simplified skill text per character / band, merged into the data JSONs
        # as `simple` (the full `text` stays as-is).
        simple = read_text_asset("skill_simple")
        if simple:
            for fname, key in (("characters.json", "characters"), ("bands.json", "bands")):
                dst = DATA_OUT / fname
                if not dst.exists():
                    continue
                doc = json.loads(dst.read_text(encoding="utf-8-sig"))
                rows = doc.get(key) if isinstance(doc, dict) else doc
                by_name = {e.get("name"): e.get("text") for e in simple.get(key, [])}
                n = 0
                for row in rows:
                    text = by_name.get(row.get("name"))
                    if text and row.get("simple") != text:
                        row["simple"] = text
                        n += 1
                dst.write_text(json.dumps(doc, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
                print(f"   data: {fname} += simple skill text on {n} {key}")

    p.drain()
    p.pool.shutdown()

    # 7) display-size stand/sd thumbs for the gallery grid and board tokens. The
    #    generated paths go into the manifest (single source of paths) as
    #    characters.<id>.thumb|sdThumb|sdHappyThumb -- see tools/asset-pipe/thumbs.py.
    from thumbs import build as build_thumbs  # noqa: E402
    for cid, extra in build_thumbs(out, p.manifest["characters"]).items():
        p.manifest["characters"].setdefault(cid, {}).update(extra)

    # A partial run (--only img,audio,...) never touches the fonts / live2d
    # sections; carry them over from the previous manifest instead of clobbering.
    mf = out / "manifest.json"
    if mf.exists():
        try:
            old = json.loads(mf.read_text(encoding="utf-8"))
        except Exception:  # noqa: BLE001
            old = {}
        if "fonts" not in only:
            p.manifest["fonts"] = old.get("fonts") or {}
        if "live2d" not in only and old.get("live2d"):
            p.manifest["live2d"] = old["live2d"]
    mf.write_text(json.dumps(p.manifest, ensure_ascii=False, indent=1), encoding="utf-8")

    print(f"\n== done in {time.time() - t0:.0f}s -> {out}")
    for cat in sorted(p.stats.counts):
        print(f"   {cat:22s} {p.stats.counts[cat]:5d} files  {p.stats.bytes.get(cat, 0) / 1e6:8.1f} MB")
    print(f"   skipped (already present): {p.stats.skipped}")
    if p.stats.errors:
        print(f"   ERRORS: {len(p.stats.errors)}")
        for e in p.stats.errors[:20]:
            print("     " + e)
        return 1
    return 0


def _is_db(obj) -> bool:
    try:
        read_database(obj.get_raw_data())
        return True
    except Exception:  # noqa: BLE001
        return False


def _unique(p: Pipeline, base: str, ext: str) -> str:
    """Scene asset names can repeat across files; suffix later ones."""
    rel, n = base + ext, 2
    while rel in p.written:
        rel, n = f"{base}_{n}{ext}", n + 1
    p.written[rel] = ""
    return rel


# ---------------------------------------------------------------------------- probe
def probe(out: Path) -> int:
    """P0 spike: extract exactly one Texture2D and one AudioClip, then validate."""
    out.mkdir(parents=True, exist_ok=True)
    got_tex = got_aud = False
    for f in sorted(BUILD.glob("*.assets")):
        env = UnityPy.load(str(f))
        for obj in env.objects:
            if obj.type.name == "Texture2D" and not got_tex:
                d = obj.read()
                dest = out / "probe" / f"{safe(d.m_Name)}.png"
                dest.parent.mkdir(parents=True, exist_ok=True)
                d.image.save(dest)
                ok = dest.read_bytes()[:8].startswith(b"\x89PNG")
                print(f"  Texture2D {d.m_Name!r}: {d.image.size} -> PNG ok={ok}")
                got_tex = ok
            elif obj.type.name == "AudioClip" and not got_aud:
                d = obj.read()
                wav = next(iter(d.samples.values()))
                ok = wav[:4] == b"RIFF" and wav[8:12] == b"WAVE"
                print(f"  AudioClip {d.m_Name!r}: {len(wav)} bytes WAV ok={ok} fmt={struct.unpack('<H', wav[20:22])[0]}")
                got_aud = ok
            if got_tex and got_aud:
                print("  PROBE OK")
                return 0
    print("  PROBE FAILED")
    return 1

def read_text_asset(name: str):
    """One named TextAsset from the build's resources (e.g. `skill_simple`)."""
    try:
        import UnityPy
    except ImportError:
        return None
    env = UnityPy.load(str(BUILD / "resources.assets"))
    for obj in env.objects:
        if obj.type.name != "TextAsset":
            continue
        d = obj.read()
        if getattr(d, "m_Name", "") != name:
            continue
        raw = d.m_Script.encode("utf-8") if isinstance(d.m_Script, str) else d.m_Script
        return json.loads(raw)
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--probe", action="store_true")
    ap.add_argument("--only", help="comma list of: img,audio,fonts,text,live2d,data")
    ap.add_argument("--force", action="store_true", help="re-export even if outputs exist")
    ap.add_argument("--jobs", type=int, default=max(2, (os.cpu_count() or 4) - 1))
    ap.add_argument("--out", type=Path, default=OUT_DEFAULT)
    args = ap.parse_args()
    return probe(args.out) if args.probe else run(args)


if __name__ == "__main__":
    raise SystemExit(main())
