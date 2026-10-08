#!/usr/bin/env python3
"""Compile Quadrism's OMG3 model XML into the renderer's `model.json`.

Input:  web/live2d-src/<id>/main.xml (+ imageFileBuf*.png)  -- see tools/live2d/unpack.py
Output: webui/public/assets/live2d/<id>/model.json
        webui/public/assets/live2d/<id>/texture_<n>.png

Only `model.json` and the texture PNGs may live under the served tree; the raw
XML stays in `web/live2d-src/` (it is ~5 MB per model and must never ship).

The XML is the Cubism 2 editor document (`com.live2d.cubism.doc`) serialized as
an object graph: definitions carry `xs.id`, references `xs.ref`, fields
`xs.n="name"`, and values nest under `xs.n="super"` wrappers.  Semantics follow
`docs/LIVE2D-SCHEMA.md` (coordinate spaces, deform math, model.json shape).

  python tools/live2d/convert.py            # all models
  python tools/live2d/convert.py 001 026    # just these ids

model.json (see docs/LIVE2D-SCHEMA.md for the full contract):

  id, canvas [w,h], ppu, textures [file names]
  params    [{id, min, max, def, repeat}]
  meshes    [{name, tex, visible, parent, blend, doubleSided, clip,
              uvs, indices, forms, opacity, drawOrder,
              multiplyColor, screenColor, grid}]
  deformers [{name, kind, parent, visible, cols, rows, forms | rot, grid}]

`forms` is one flat `x,y,…` array per keyform (mesh: positions in the parent
deformer's local space; warp: lattice points in the warp's parent space).  The
per-form scalars listed under "Form payloads" in the schema doc are emitted as
parallel arrays keyed by the same form index (`opacity`, `drawOrder`,
`multiplyColor`, `screenColor`); `rot` holds `[angle, ox, oy, scale,
reflectX, reflectY]` rows for rotation deformers.  `grid` maps
`(binding, keyIndex)` combos to form indices exactly as the spec describes.
Floats are rounded to 4 decimals and the file is minified JSON.
"""

import os
import json
import re
import shutil
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
IN = ROOT / "live2d-src"
OUT = ROOT / "webui" / "public" / "assets" / "live2d"
# the shipped game keeps the original Cubism 2 textures; used only when a
# .cxx3 archive is missing one of the PNGs its XML references (27 of the 55
# models reference a second texture that the archive does not embed).
GAME = ROOT / os.environ.get("GAME_LIVE2D", "../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D")

BLEND = {"NORMAL": 0, "MULTIPLY": 1, "ADD": 2, "ADDITIVE": 2, "SCREEN": 3}
NUM = re.compile(r"[-+0-9.eE]+")


def rnd(x: float) -> float:
    """Round to 4 decimals; keep ints/booleans clean."""
    v = round(float(x) + 0.0, 4)
    return 0.0 if v == 0 else v


def rnd_list(vals) -> list:
    return [rnd(v) for v in vals]


# ---------------------------------------------------------------- XML helpers

def flat_fields(el) -> dict:
    """Fields of `el`: its own children plus the `super` chain.

    Field lookup is deep in the sense that real values live under nested
    `xs.n="super"` wrappers, but we must not descend into payload containers
    (`keyforms`, `_extensions`, …) or their inner fields shadow the source's.
    """
    out = {}

    def walk(node):
        for ch in node:
            n = ch.get("xs.n")
            if n == "super":
                walk(ch)
            elif n and n not in out:
                out[n] = ch

    walk(el)
    return out


def text(el, name: str, default: str = "") -> str:
    f = flat_fields(el).get(name)
    if f is None or f.text is None:
        return default
    return f.text.strip()


def floats(el, name: str) -> list:
    f = flat_fields(el).get(name)
    if f is None or not (f.text or "").strip():
        return []
    return [float(x) for x in f.text.split()]


def ints(el, name: str) -> list:
    f = flat_fields(el).get(name)
    if f is None or not (f.text or "").strip():
        return []
    return [int(float(x)) for x in f.text.split()]


def ref_of(el, name: str):
    f = flat_fields(el).get(name)
    return None if f is None else f.get("xs.ref")


def enum_field(el, name: str, default: str = "") -> str:
    """Value of an enum-valued field (`<ColorComposition v="ADD"/>`)."""
    f = flat_fields(el).get(name)
    if f is None:
        return default
    return (f.get("v") or (f.text or "")).strip() or default


def bool_field(el, name: str, default: bool = True) -> bool:
    v = text(el, name, None)
    if v is None:
        return default
    return v != "false"


# ---------------------------------------------------------------- conversion

def convert(mid: str) -> dict:
    src = IN / mid / "main.xml"
    if not src.exists():
        raise FileNotFoundError(f"missing {src} (run tools/live2d/unpack.py)")
    root = ET.parse(src).getroot()

    by_id = {}
    for el in root.iter():
        i = el.get("xs.id")
        if i:
            by_id[i] = el

    # ---- canvas --------------------------------------------------------
    width = height = None
    for el in root.iter("CImageCanvas"):
        f = flat_fields(el)
        if "pixelWidth" in f:
            width = float(f["pixelWidth"].text)
            height = float(f["pixelHeight"].text)
    if width is None:
        raise ValueError("no CImageCanvas")
    canvas = [int(width), int(height)]
    ppu = int(width)

    # ---- textures ------------------------------------------------------
    # CImageResource definitions carry the archive path of their PNG; the
    # GTexture2D a mesh points at names the image resource.
    res_order = []          # (raw file path, CImageResource xs.id)
    res_index = {}
    for el in root.iter("CImageResource"):
        if not el.get("xs.id"):
            continue
        for ch in el:
            if ch.tag == "file" and ch.get("path"):
                res_order.append((ch.get("path"), el.get("xs.id")))
    res_order.sort(key=lambda t: texture_rank(t[0]))
    for i, (_, rid) in enumerate(res_order):
        res_index[rid] = i
    raw_names = [p for p, _ in res_order]
    textures = [f"texture_{i}.png" for i in range(len(res_order))]

    tex_of_gtex = {}
    for el in root.iter("GTexture2D"):
        if not el.get("xs.id"):
            continue
        rid = ref_of(el, "srcImageResource")
        if rid in res_index:
            tex_of_gtex[el.get("xs.id")] = res_index[rid]

    # ---- params --------------------------------------------------------
    params = []
    param_index = {}
    for el in root.iter("CParameterSource"):
        f = flat_fields(el)
        pid = f["id"].get("idstr")
        param_index[f["guid"].get("xs.ref")] = len(params)
        params.append({
            "id": pid,
            "min": rnd(f["minValue"].text),
            "max": rnd(f["maxValue"].text),
            "def": rnd(f["defaultValue"].text),
            "repeat": bool_field(el, "isRepeat", False),
        })

    # ---- deformers (warp + rotation share one index list) --------------
    deformer_defs = []      # elements, document order
    deformer_index = {}     # CDeformerGuid ref -> index
    for el in root.iter():
        if el.tag not in ("CWarpDeformerSource", "CRotationDeformerSource"):
            continue
        if not el.get("xs.id"):
            continue
        deformer_index[ref_of(el, "guid")] = len(deformer_defs)
        deformer_defs.append(el)

    # mesh index by drawable guid (for clip masks)
    # Meshes are emitted in the runtime's draw order, which is what
    # Live2DUnity's ModelContext buckets by drawOrder on: equal drawOrder
    # values fall back to drawDataList order.  That order is the model's
    # parts in document order, and within each part the art meshes in
    # reverse document order (verified against the shipped model.moc for
    # every model).  Emitting them in document order instead hides an
    # iris behind its eye-white clip mask, for instance.
    mesh_defs = [el for el in root.iter("CArtMeshSource") if el.get("xs.id")]
    part_group: dict = {}
    part_order: list = []
    for el in mesh_defs:
        part = ref_of(el, "parentGuid")
        if part not in part_group:
            part_group[part] = []
            part_order.append(part)
        part_group[part].append(el)
    mesh_defs = [el for part in part_order for el in reversed(part_group[part])]
    mesh_index = {}
    for i, el in enumerate(mesh_defs):
        mesh_index[ref_of(el, "guid")] = i

    # parts: visibility groups only (CPartGuid -> isVisible)
    part_visible = {}
    for el in root.iter("CPartSource"):
        if not el.get("xs.id"):
            continue
        part_visible[ref_of(el, "guid")] = bool_field(el, "isVisible", True)

    def parent_of(el) -> int:
        g = ref_of(el, "targetDeformerGuid")
        if not g or g not in deformer_index:
            return -1
        return deformer_index[g]

    def visible_of(el) -> bool:
        v = bool_field(el, "isVisible", True)
        part = ref_of(el, "parentGuid")
        if part in part_visible:
            v = v and part_visible[part]
        return v

    def grid_of(el) -> dict:
        f = flat_fields(el)
        gref = ref_of(el, "keyformGridSource")
        binds, at = [], []
        if gref and gref in by_id:
            g = by_id[gref]
            gf = {ch.get("xs.n"): ch for ch in g if ch.get("xs.n")}
            bind_axis = {}
            for e in gf.get("keyformBindings", []):
                bref = e.get("xs.ref")
                b = flat_fields(by_id[bref])
                pref = b["parameterGuid"].get("xs.ref")
                if pref not in param_index:
                    raise ValueError(f"{mid}: binding {bref} -> unknown param {pref}")
                bind_axis[bref] = len(binds)
                binds.append({
                    "param": param_index[pref],
                    "keys": [rnd(x.text) for x in b["keys"]],
                })
            for kog in gf.get("keyformsOnGrid", []):
                kf = flat_fields(kog)
                combo = []
                acc = kf.get("accessKey")
                if acc is not None:
                    for kop in acc.iter("KeyOnParameter"):
                        kof = flat_fields(kop)
                        bref = kof["binding"].get("xs.ref")
                        if bref not in bind_axis:
                            raise ValueError(f"{mid}: key on unknown binding {bref}")
                        combo.append([bind_axis[bref], int(kof["keyIndex"].text)])
                combo.sort()
                at.append({"at": combo, "form": kf["keyformGuid"].get("xs.ref")})
        return {"binds": binds, "at": at}

    def form_scalars(form_el: ET.Element) -> dict:
        f = flat_fields(form_el)
        mul = f.get("multiplyColor")
        scr = f.get("screenColor")

        def color(c, r, g, b, a):
            if c is None:
                return [r, g, b, a]
            return [rnd(c.get("red", r)), rnd(c.get("green", g)),
                    rnd(c.get("blue", b)), rnd(c.get("alpha", a))]

        return {
            "opacity": rnd(f["opacity"].text) if "opacity" in f else 1.0,
            "drawOrder": int(float(f["drawOrder"].text)) if "drawOrder" in f else 0,
            "multiplyColor": color(mul, 1, 1, 1, 1),
            "screenColor": color(scr, 0, 0, 0, 1),
        }

    # ---- meshes --------------------------------------------------------
    meshes = []
    for el in mesh_defs:
        f = flat_fields(el)
        forms = list(f["keyforms"])
        form_guids = [ref_of(fm, "guid") for fm in forms]
        grid = grid_of(el)
        for entry in grid["at"]:
            g = entry["form"]
            if g not in form_guids:
                raise ValueError(f"{mid}: {f['id'].get('idstr')} grid form {g} not in keyforms")
            entry["form"] = form_guids.index(g)

        gtid = ref_of(el, "texture")
        tex = tex_of_gtex.get(gtid, 0)
        cc = enum_field(el, "colorComposition", "NORMAL").upper()
        if cc not in BLEND:
            raise ValueError(f"{mid}: unknown colorComposition {cc}")
        clip_guids = [c.get("xs.ref") for c in flat_fields(el).get("clipGuidList", [])]
        clip = None
        if clip_guids:
            masks = [mesh_index[g] for g in clip_guids if g in mesh_index]
            if masks:
                clip = {"meshes": masks,
                        "invert": bool_field(el, "invertClippingMask", False)}

        meshes.append({
            "name": f["id"].get("idstr"),
            "tex": tex,
            "visible": visible_of(el),
            "parent": parent_of(el),
            "blend": BLEND[cc],
            "doubleSided": not bool_field(el, "culling", False),
            "clip": clip,
            "uvs": rnd_list(floats(el, "uvs")),
            "indices": ints(el, "indices"),
            "forms": [rnd_list(floats(fm, "positions")) for fm in forms],
            "opacity": [form_scalars(fm)["opacity"] for fm in forms],
            "drawOrder": [form_scalars(fm)["drawOrder"] for fm in forms],
            "multiplyColor": [form_scalars(fm)["multiplyColor"] for fm in forms],
            "screenColor": [form_scalars(fm)["screenColor"] for fm in forms],
            "grid": grid,
        })

    # ---- deformers -----------------------------------------------------
    deformers = []
    for el in deformer_defs:
        f = flat_fields(el)
        forms = list(f["keyforms"])
        form_guids = [ref_of(fm, "guid") for fm in forms]
        grid = grid_of(el)
        for entry in grid["at"]:
            g = entry["form"]
            if g not in form_guids:
                raise ValueError(f"{mid}: {f['id'].get('idstr')} grid form {g} not in keyforms")
            entry["form"] = form_guids.index(g)

        d = {
            "name": flat_fields(el)["id"].get("idstr"),
            "kind": "warp" if el.tag == "CWarpDeformerSource" else "rot",
            "parent": parent_of(el),
            "visible": visible_of(el),
            "grid": grid,
        }
        if el.tag == "CWarpDeformerSource":
            d["cols"] = int(f["col"].text) + 1   # lattice points, not cells
            d["rows"] = int(f["row"].text) + 1
            d["forms"] = [rnd_list(floats(fm, "positions")) for fm in forms]
        else:
            rot = []
            for fm in forms:
                a = fm.attrib
                rot.append([
                    rnd(a.get("angle", 0)), rnd(a.get("originX", 0)),
                    rnd(a.get("originY", 0)), rnd(a.get("scale", 1)),
                    1 if a.get("isReflectX") == "true" else 0,
                    1 if a.get("isReflectY") == "true" else 0,
                ])
            d["rot"] = rot
        deformers.append(d)

    return {
        "id": mid,
        "canvas": canvas,
        "ppu": ppu,
        "textures": textures,
        "_raw_textures": raw_names,
        "params": params,
        "meshes": meshes,
        "deformers": deformers,
    }


def texture_rank(path: str) -> tuple:
    """imageFileBuf.png -> 0, imageFileBuf_1.png -> 1, …"""
    m = re.search(r"_(\d+)$", Path(path).stem)
    return (int(m.group(1)) if m else 0, path)


def install_textures(mid: str, model: dict) -> None:
    """Copy the raw PNGs into the served tree as texture_<n>.png.

    `web/live2d-src/<id>/` holds what the .cxx3 archive carried
    (`imageFileBuf.png`, sometimes `imageFileBuf_1.png`); 27 of the archives
    omit the second texture their XML references, and those bytes come from the
    original game's `BandoriLive2D/<id>/texture_<n>.png` (byte-identical to the
    archive's `imageFileBuf.png` for texture 0).
    """
    raw_names = model.pop("_raw_textures")
    for i, (name, raw) in enumerate(zip(model["textures"], raw_names)):
        candidates = [IN / mid / raw, GAME / mid / f"texture_{i}.png"]
        srcp = next((c for c in candidates if c.exists()), None)
        if srcp is None:
            raise FileNotFoundError(f"{mid}: no source PNG for {name} ({raw})")
        shutil.copyfile(srcp, OUT / mid / name)


def main() -> int:
    ids = sys.argv[1:]
    if not ids:
        ids = sorted(p.name for p in IN.iterdir() if p.is_dir())
    if not ids:
        print(f"no extracted models in {IN} (run tools/live2d/unpack.py)")
        return 1

    ok = 0
    total_bytes = 0
    for mid in ids:
        try:
            model = convert(mid)
            dest = OUT / mid
            dest.mkdir(parents=True, exist_ok=True)
            # the served tree holds model.json + texture PNGs and nothing else
            for stale in dest.iterdir():
                if stale.name == "model.json" or re.fullmatch(r"texture_\d+\.png", stale.name):
                    continue
                stale.unlink()
            install_textures(mid, model)
            blob = json.dumps(model, separators=(",", ":"), ensure_ascii=False)
            (dest / "model.json").write_text(blob, encoding="utf-8")
        except Exception as e:  # noqa: BLE001 - report per model and continue
            print(f"{mid}: FAIL {type(e).__name__}: {e}")
            continue
        n_mesh, n_def = len(model["meshes"]), len(model["deformers"])
        n_par = len(model["params"])
        forms = sum(len(m["forms"]) for m in model["meshes"])
        forms += sum(len(d.get("forms", d.get("rot", []))) for d in model["deformers"])
        total_bytes += len(blob)
        print(f"{mid}: {n_mesh} meshes, {n_def} deformers, {n_par} params, "
              f"{forms} forms, {len(model['textures'])} textures, "
              f"canvas {model['canvas'][0]}x{model['canvas'][1]}, {len(blob)} B")
        ok += 1
    print(f"converted {ok}/{len(ids)} models, model.json total {total_bytes} B "
          f"({total_bytes / 1024 / 1024:.1f} MB) -> {OUT}")
    return 0 if ok == len(ids) else 1


if __name__ == "__main__":
    raise SystemExit(main())