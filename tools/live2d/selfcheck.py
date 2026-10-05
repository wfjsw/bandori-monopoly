#!/usr/bin/env python3
"""Structural + geometric self-check for the compiled `model.json` files.

For every model under `webui/public/assets/live2d/<id>/`:

  * every `parent` index resolves (``-1`` = root, else an index into
    ``deformers``);
  * every keyform-grid ``form`` index is in range for its node, every
    ``binds[].param`` indexes ``params``, and every ``at`` axis indexes
    ``binds``;
  * every mesh carries non-empty ``uvs``, ``indices`` and ``forms``, with
    ``len(uvs) == 2 * nverts``, every form of matching length, and every index
    inside ``[0, nverts)``;
  * warp deformers have ``forms`` of length ``2 * cols * rows``, rotation
    deformers have ``rot`` rows of 6 numbers;
  * ``tex``/``clip`` indices resolve and the texture PNGs exist on disk;
  * for 3 sample meshes per model, the deform chain is walked at the default
    parameter values (multilinear keyform interpolation, then the
    ``docs/LIVE2D-SCHEMA.md`` transform math) and the composed canvas position
    must stay inside a sane box around the canvas -- the canvas scaled 3x about
    its centre, i.e. ``x in [-w, 2w]``, ``y in [-h, 2h]``.

  python tools/live2d/selfcheck.py            # all models
  python tools/live2d/selfcheck.py 001 026    # just these ids

Exit status is non-zero if any model fails.
"""

import json
import math
import sys
from itertools import product
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DIR = ROOT / "webui" / "public" / "assets" / "live2d"


# ------------------------------------------------------------- keyform grid

def form_weights(grid: dict, values: list) -> dict:
    """Multilinear weights over the grid's form indices at `values`.

    Per axis the parameter value is bracketed between two key values (clamped
    outside the range); each corner of the resulting box picks one form and
    contributes the product of its per-axis weights.  A corner with no form
    falls back to the nearest present corner.
    """
    binds = grid["binds"]
    entries = grid["at"]
    present = {tuple(map(tuple, e["at"])): e["form"] for e in entries}
    if not binds:
        return {entries[0]["form"]: 1.0} if entries else {}

    axes = []
    for b in binds:
        keys = b["keys"]
        v = values[b["param"]]
        if len(keys) == 1:
            axes.append((0, 0, 1.0, 0.0))
        elif v <= keys[0]:
            axes.append((0, 0, 1.0, 0.0))
        elif v >= keys[-1]:
            n = len(keys) - 1
            axes.append((n, n, 1.0, 0.0))
        else:
            i0 = 0
            for a in range(len(keys) - 1):
                if keys[a] <= v <= keys[a + 1]:
                    i0 = a
                    break
            i1 = i0 + 1
            span = keys[i1] - keys[i0]
            t = 0.0 if span == 0 else (v - keys[i0]) / span
            axes.append((i0, i1, 1.0 - t, t))

    fixed = [(a, axes[a][0]) for a in range(len(axes)) if axes[a][0] == axes[a][1]]
    free = [a for a in range(len(axes)) if axes[a][0] != axes[a][1]]
    weights: dict[int, float] = {}
    for bits in product((0, 1), repeat=len(free)):
        w = 1.0
        combo = list(fixed)
        for k, a in enumerate(free):
            i0, i1, w0, w1 = axes[a]
            if bits[k]:
                combo.append((a, i1))
                w *= w1
            else:
                combo.append((a, i0))
                w *= w0
        combo.sort()
        form = present.get(tuple(map(tuple, combo)))
        if form is None:
            # nearest present corner: most matching (axis, key) pairs
            best, score = None, -1
            cset = set(map(tuple, combo))
            for e in entries:
                sc = sum(1 for x in map(tuple, e["at"]) if x in cset)
                if sc > score:
                    score, best = sc, e["form"]
            form = best
        if form is not None:
            weights[form] = weights.get(form, 0.0) + w
    return weights


def lerp(rows: list, weights: dict) -> list:
    out = None
    for idx, w in weights.items():
        row = rows[idx]
        if out is None:
            out = [0.0] * len(row)
        for i in range(len(row)):
            out[i] += row[i] * w
    return out


# ------------------------------------------------------------ deform math
#
# Mirrors the shipped Cubism 2 runtime (Live2DUnity `BDBoxGrid` / `BDAffine`):
# every deformer's `transformPoints` maps its child space straight to canvas
# pixels. Warps carry their lattice to canvas first and then FFD cage params
# through it (barycentric per grid cell); rotations map pivot-relative pixel
# offsets to canvas as `O + totalScale * Rot(theta) * reflect * p`, where
# `O` is the pivot already carried to canvas and `totalScale` is the product
# of rotation scales along the chain. Ancestor warps pre-warp the pivot only.

def _quad(q0, q1, q2, q3, s, t):
    """Diagonal-split quad patch (BDBoxGrid's cell/skirt primitive)."""
    if s + t <= 1:
        return (q0[0] + (q1[0] - q0[0]) * s + (q2[0] - q0[0]) * t,
                q0[1] + (q1[1] - q0[1]) * s + (q2[1] - q0[1]) * t)
    return (q3[0] + (q2[0] - q3[0]) * (1 - s) + (q1[0] - q3[0]) * (1 - t),
            q3[1] + (q2[1] - q3[1]) * (1 - s) + (q1[1] - q3[1]) * (1 - t))


def ffd(lat, cols, rows, u, v):
    """BDBoxGrid: barycentric FFD over the warp lattice, runtime skirt outside."""
    col, row = cols - 1, rows - 1
    x, y = u * col, v * row
    if x < 0 or y < 0 or x >= col or y >= row:
        return _ffd_skirt(lat, cols, col, row, u, v, x, y)

    def pt(a, b):
        return lat[(b * cols + a) * 2], lat[(b * cols + a) * 2 + 1]

    i, j = math.floor(x), math.floor(y)
    fx, fy = x - i, y - j
    x00, y00 = pt(i, j)
    x10, y10 = pt(i + 1, j)
    x01, y01 = pt(i, j + 1)
    x11, y11 = pt(i + 1, j + 1)
    if fx + fy < 1:
        return (x00 * (1 - fx - fy) + x10 * fx + x01 * fy,
                y00 * (1 - fx - fy) + y10 * fx + y01 * fy)
    return (x11 * (fx - 1 + fy) + x01 * (1 - fx) + x10 * (1 - fy),
            y11 * (fx - 1 + fy) + y01 * (1 - fx) + y10 * (1 - fy))


def _ffd_skirt(lat, cols, col, row, u, v, x, y):
    """BDBoxGrid's out-of-lattice skirt (see webui/src/live2d/deform.ts)."""
    def pt(a, b):
        return lat[(b * cols + a) * 2], lat[(b * cols + a) * 2 + 1]

    tl, tr = pt(0, 0), pt(col, 0)
    bl, br = pt(0, row), pt(col, row)
    cx = 0.25 * (tl[0] + tr[0] + bl[0] + br[0])
    cy = 0.25 * (tl[1] + tr[1] + bl[1] + br[1])
    d1 = (br[0] - tl[0], br[1] - tl[1])
    d2 = (tr[0] - bl[0], tr[1] - bl[1])
    ux, uy = (d1[0] + d2[0]) * 0.5, (d1[1] + d2[1]) * 0.5
    vx, vy = (d1[0] - d2[0]) * 0.5, (d1[1] - d2[1]) * 0.5
    ox, oy = cx - 0.5 * (ux + vx), cy - 0.5 * (uy + vy)

    def F(a, b):
        return (ox + a * ux + b * vx, oy + a * uy + b * vy)

    if not (-2 < u < 3 and -2 < v < 3):
        return F(u, v)
    if u <= 0:
        if v <= 0:
            return _quad(F(-2, -2), F(0, -2), F(-2, 0), tl,
                         0.5 * (u + 2), 0.5 * (v + 2))
        if v >= 1:
            return _quad(F(-2, 1), bl, F(-2, 3), F(0, 3),
                         0.5 * (u + 2), 0.5 * (v - 1))
        j = min(math.floor(y), row - 1)
        return _quad(F(-2, j / row), pt(0, j), F(-2, (j + 1) / row), pt(0, j + 1),
                     0.5 * (u + 2), y - j)
    if u >= 1:
        if v <= 0:
            return _quad(F(1, -2), F(3, -2), tr, F(3, 0),
                         0.5 * (u - 1), 0.5 * (v + 2))
        if v >= 1:
            return _quad(br, F(3, 1), F(1, 3), F(3, 3),
                         0.5 * (u - 1), 0.5 * (v - 1))
        j = min(math.floor(y), row - 1)
        return _quad(pt(col, j), F(3, j / row), pt(col, j + 1), F(3, (j + 1) / row),
                     0.5 * (u - 1), y - j)
    if v <= 0:
        i = min(math.floor(x), col - 1)
        return _quad(F(i / col, -2), F((i + 1) / col, -2), pt(i, 0), pt(i + 1, 0),
                     x - i, 0.5 * (v + 2))
    i = min(math.floor(x), col - 1)
    return _quad(pt(i, row), pt(i + 1, row), F(i / col, 3), F((i + 1) / col, 3),
                 x - i, 0.5 * (v - 1))


def rot_step(p, node):
    """p_canvas = O + totalScale * Rot(theta) * reflect * p (canvas-space node)."""
    qx = -p[0] if node["rx"] else p[0]
    qy = -p[1] if node["ry"] else p[1]
    c, s = node["cos"], node["sin"]
    k = node["k"]
    return node["ox"] + k * (qx * c - qy * s), node["oy"] + k * (qx * s + qy * c)


def apply_node(node, p):
    if node["kind"] == "warp":
        return ffd(node["lat"], node["cols"], node["rows"], p[0], p[1])
    return rot_step(p, node)


def eval_deformers(model, defaults):
    """Flattened per-deformer child->canvas transforms at `defaults`."""
    deformers = model["deformers"]
    memo = {}

    def ev(i, depth=0):
        if i in memo:
            return memo[i]
        if depth > 64:
            raise RuntimeError(f"deformer cycle at {i}")
        d = deformers[i]
        parent = ev(d["parent"], depth + 1) if d["parent"] != -1 else None
        if d["kind"] == "warp":
            lat = lerp(d["forms"], form_weights(d["grid"], defaults))
            if lat is None:
                raise RuntimeError(f"deformer {d['name']} has no form at defaults")
            if parent is not None:
                tlat = []
                for k in range(0, len(lat), 2):
                    x, y = apply_node(parent, (lat[k], lat[k + 1]))
                    tlat += [x, y]
            else:
                tlat = list(lat)
            node = {
                "kind": "warp",
                "name": d["name"],
                "lat": tlat,
                "cols": d["cols"],
                "rows": d["rows"],
                "totalScale": parent["totalScale"] if parent else 1.0,
            }
        else:
            row = lerp(d["rot"], form_weights(d["grid"], defaults))
            if row is None:
                raise RuntimeError(f"deformer {d['name']} has no form at defaults")
            ang, ox, oy, sc, rx, ry = row
            if parent is None:
                node = {
                    "kind": "rot",
                    "name": d["name"],
                    "ox": ox,
                    "oy": oy,
                    "k": sc,
                    "cos": math.cos(math.radians(ang)),
                    "sin": math.sin(math.radians(ang)),
                    "rx": rx >= 0.5,
                    "ry": ry >= 0.5,
                    "totalScale": sc,
                }
            else:
                pcx, pcy = apply_node(parent, (ox, oy))
                probe = -10.0 if parent["kind"] == "rot" else -0.1
                px, py = apply_node(parent, (ox, oy + probe))
                pangle = math.atan2(py - pcy, px - pcx) - math.atan2(probe, 0.0)
                a = math.radians(ang) + pangle
                k = parent["totalScale"] * sc
                node = {
                    "kind": "rot",
                    "name": d["name"],
                    "ox": pcx,
                    "oy": pcy,
                    "k": k,
                    "cos": math.cos(a),
                    "sin": math.sin(a),
                    "rx": rx >= 0.5,
                    "ry": ry >= 0.5,
                    "totalScale": k,
                }
        memo[i] = node
        return node

    for i in range(len(deformers)):
        ev(i)
    return memo


# ------------------------------------------------------------------ checks

def check_model(mid: str, model: dict) -> list:
    errs = []
    params = model["params"]
    meshes = model["meshes"]
    deformers = model["deformers"]
    n_def = len(deformers)
    n_mesh = len(meshes)
    n_par = len(params)
    ppu = model["ppu"]
    w, h = model["canvas"]
    box = (-w, 2 * w, -h, 2 * h)  # canvas scaled 3x about its centre

    def check_grid(tag, grid, nforms):
        for bi, b in enumerate(grid["binds"]):
            if not (0 <= b["param"] < n_par):
                errs.append(f"{tag}: binds[{bi}].param {b['param']} out of range")
            if not b["keys"]:
                errs.append(f"{tag}: binds[{bi}].keys empty")
        for ei, e in enumerate(grid["at"]):
            if not (0 <= e["form"] < nforms):
                errs.append(f"{tag}: grid.at[{ei}].form {e['form']} out of range")
            for axis, key in e["at"]:
                if not (0 <= axis < len(grid["binds"])):
                    errs.append(f"{tag}: grid.at[{ei}] axis {axis} out of range")
                elif not (0 <= key < len(grid["binds"][axis]["keys"])):
                    errs.append(f"{tag}: grid.at[{ei}] key {key} out of range "
                                f"for axis {axis}")
        if not grid["at"]:
            errs.append(f"{tag}: grid has no entries")

    for i, m in enumerate(meshes):
        tag = f"mesh[{i}] {m['name']}"
        if not (-1 <= m["parent"] < n_def):
            errs.append(f"{tag}: parent {m['parent']} does not resolve")
        nverts = len(m["uvs"]) // 2
        if not m["uvs"] or len(m["uvs"]) % 2:
            errs.append(f"{tag}: bad uvs")
        if not m["indices"]:
            errs.append(f"{tag}: empty indices")
        if not m["forms"]:
            errs.append(f"{tag}: empty forms")
        for fi, f in enumerate(m["forms"]):
            if len(f) != 2 * nverts:
                errs.append(f"{tag}: form {fi} has {len(f) // 2} verts, uvs {nverts}")
        for idx in m["indices"]:
            if not (0 <= idx < nverts):
                errs.append(f"{tag}: index {idx} out of range")
                break
        for key in ("opacity", "drawOrder", "multiplyColor", "screenColor"):
            if key in m and len(m[key]) != len(m["forms"]):
                errs.append(f"{tag}: {key} length != forms")
        if not (0 <= m["tex"] < len(model["textures"])):
            errs.append(f"{tag}: tex {m['tex']} out of range")
        if m["clip"] is not None:
            for c in m["clip"]["meshes"]:
                if not (0 <= c < n_mesh):
                    errs.append(f"{tag}: clip mesh {c} out of range")
        check_grid(tag, m["grid"], len(m["forms"]))

    for i, d in enumerate(deformers):
        tag = f"deformer[{i}] {d['name']}"
        if not (-1 <= d["parent"] < n_def):
            errs.append(f"{tag}: parent {d['parent']} does not resolve")
        if d["parent"] == i:
            errs.append(f"{tag}: parent points at itself")
        if d["kind"] == "warp":
            n = 2 * d["cols"] * d["rows"]
            if d["cols"] < 2 or d["rows"] < 2:
                errs.append(f"{tag}: degenerate lattice {d['cols']}x{d['rows']}")
            if not d.get("forms"):
                errs.append(f"{tag}: no forms")
            for fi, f in enumerate(d.get("forms", [])):
                if len(f) != n:
                    errs.append(f"{tag}: form {fi} has {len(f) // 2} pts, want {n // 2}")
            check_grid(tag, d["grid"], len(d.get("forms", [])))
        elif d["kind"] == "rot":
            if not d.get("rot"):
                errs.append(f"{tag}: no rot forms")
            for fi, r in enumerate(d.get("rot", [])):
                if len(r) != 6:
                    errs.append(f"{tag}: rot[{fi}] has {len(r)} values, want 6")
            check_grid(tag, d["grid"], len(d.get("rot", [])))
        else:
            errs.append(f"{tag}: unknown kind {d['kind']}")

    # ---- walk 3 sample meshes through the flattened deform transform
    # at default params (keyform interpolation + the docs/LIVE2D-SCHEMA.md /
    # Live2DUnity transform math).
    if n_mesh:
        samples = {0, n_mesh // 2, n_mesh - 1}
        defaults = [p["def"] for p in params]
        try:
            nodes = eval_deformers(model, defaults)
        except RuntimeError as ex:
            errs.append(f"deform evaluation: {ex}")
            nodes = None
        if nodes is not None:
            for si in sorted(samples):
                m = meshes[si]
                tag = f"mesh[{si}] {m['name']}"
                rows = [m["forms"][k] for k in range(len(m["forms"]))]
                wts = form_weights(m["grid"], defaults)
                pos = lerp(rows, wts)
                if pos is None:
                    errs.append(f"{tag}: no form evaluated at defaults")
                    continue
                pts = [(pos[2 * k], pos[2 * k + 1]) for k in range(len(pos) // 2)]
                if m["parent"] != -1:
                    pts = [apply_node(nodes[m["parent"]], p) for p in pts]
                for k, (x, y) in enumerate(pts):
                    if not (box[0] <= x <= box[1] and box[2] <= y <= box[3]):
                        errs.append(f"{tag}: vertex {k} rest pose ({x:.1f}, {y:.1f}) "
                                    f"outside sanity box {box}")
                        break

    for i, t in enumerate(model["textures"]):
        if not (DIR / mid / t).exists():
            errs.append(f"texture[{i}] {t} missing on disk")
    return errs


def main() -> int:
    ids = sys.argv[1:]
    if not ids:
        ids = sorted(p.name for p in DIR.iterdir() if p.is_dir())
    failed = 0
    checked = 0
    for mid in ids:
        path = DIR / mid / "model.json"
        if not path.exists():
            print(f"{mid}: FAIL no model.json")
            failed += 1
            continue
        model = json.loads(path.read_text(encoding="utf-8"))
        errs = check_model(mid, model)
        checked += 1
        if errs:
            failed += 1
            print(f"{mid}: FAIL {len(errs)} problem(s)")
            for e in errs[:12]:
                print(f"    {e}")
            if len(errs) > 12:
                print(f"    ... {len(errs) - 12} more")
        else:
            n_forms = sum(len(m["forms"]) for m in model["meshes"])
            n_forms += sum(len(d.get("forms", d.get("rot", []))) for d in model["deformers"])
            print(f"{mid}: ok  {len(model['meshes'])} meshes, "
                  f"{len(model['deformers'])} deformers, {len(model['params'])} params, "
                  f"{n_forms} forms")
    print(f"selfcheck: {checked - failed}/{checked} models clean")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())