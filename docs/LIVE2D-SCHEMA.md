# Live2D (Cubism 2) render schema

The pipeline, the rebuild steps and the UI component live in [LIVE2D.md](LIVE2D.md);
this document is the data contract between the converter and `webui/src/live2d/`.

The 55 character models ship as Cubism 2 `model.moc`. Quadrism converts them to
an OMG3 `CAFF` archive (`.cxx3`); `tools/live2d/unpack.py` extracts `main.xml`
plus PNG textures, and `tools/live2d/convert.py` compiles `main.xml` into the
compact `model.json` the web renderer consumes. This document is the contract
between the converter and `webui/src/live2d/`.

Provenance: the XML is the Cubism 2 editor document (`com.live2d.cubism.doc`).
Semantics below were derived from quadrism's converter
(`crates/quadrism-lib/src/{archive/alp2,convert/alp3_to_omg3}`) and checked by
composing mesh #434's deformer chain in model `001` end to end: it lands in the
face region of the canvas with a sane size and shape.

Trap: a mesh source also carries an *editor* mesh (`GEditableMesh2` `point`,
identical to the source-level `positions` array) which lives in **PSD/atlas**
space, not the runtime canvas -- the `CAffine` matrices on the texture elements
are the atlas-packing transforms. Do **not** validate composed canvas positions
against that array; a constant offset of tens of pixels between the two spaces
is expected.

## Coordinate spaces

* **canvas** — `CImageCanvas` `pixelWidth` × `pixelHeight` (e.g. 2000×2500).
  Objects whose parent is the root write their coordinates in canvas pixels.
  `ppu` (pixels per unit) equals the canvas width and is informational only --
  the runtime never divides a rotation's scale by it (see Transforms).
* **warp local** — every warp deformer gives its children a *cage parameter*
  space: `(u, v)`, nominally `[0,1]²`, interpolated over the warp's control
  lattice (`cols × rows` points stored in the warp's own parent space).
* **rotation local** — every rotation deformer gives its children a *pixel*
  space whose origin is the rotation's pivot: child coordinates are canvas-pixel
  offsets from the pivot.

## Deform tree

Each art mesh, warp and rotation has `targetDeformerGuid` → parent deformer
(`None` = root). Parts (`CPartGuid`) group for visibility only — they never move
geometry. Draw order and opacity live on the drawable forms.

## Transforms

The shipped Cubism 2 runtime (`Live2DUnity.dll`, `BDBoxGrid` / `BDAffine`)
flattens the chain: **every deformer's `transformPoints` maps its child space
straight to canvas pixels**. The parent chain is folded in when the deformer
is evaluated, so composing a mesh is one step through its immediate parent.

Per node, with `p` in the node's child space (the node's `transformPoints`):

```
root        p_canvas = p                 # child geometry is already canvas px

warp W      # lattice L of cols×rows points, in W's parent's child space
    L_canvas = parent.transformPoints(L) # lattice carried to canvas first
    p_canvas = ffd(L_canvas, p.u, p.v)   # barycentric over the lattice cells
    # gx = u*(cols-1), gy = v*(rows-1); cell (i,j) = floor; t = frac(gx),
    # s = frac(gy).  t+s < 1 -> triangle (i,j),(i+1,j),(i,j+1); else the
    # other half of the cell.  (NOT bilinear -- the runtime splits each cell
    # along its diagonal; bilinear differs strictly inside a cell.)
    # (u,v) may fall outside [0,1] -- the runtime's BDBoxGrid skirt: build a
    # parallelogram frame F(a,b)=O+a·U+b·V from the four lattice corners
    # (O = centroid − (U+V)/2, U = ((BR−TL)+(TR−BL))/2, V = ((BR−TL)−(TR−BL))/2),
    # cover (-2,3)² with 8 corner/edge patches (each a diagonal-split quad
    # whose (s,t) maps the region onto the patch), and beyond that use the
    # linear frame F(u,v) itself. Regression-tested against the DLL.

rotation R  # pivot O in R's parent's child space, angle θ degrees, scale s,
            # reflect flags, from the keyform
    O_canvas = parent.transformPoints(O) # a warp parent pre-warps the pivot
    θ_canvas = θ + parentAngleAt(O)      # the parent chain's local rotation at
                                         # O, by finite-difference probe (the
                                         # runtime measures (0,-0.1) under a
                                         # warp parent, (0,-10) under a rot)
    totalScale = parent.totalScale * s   # Π of rotation scales down the chain;
                                         # warps add no scale
    q = (reflect_x ? -p.x : p.x, reflect_y ? -p.y : p.y)
    p_canvas = O_canvas + totalScale * Rot(θ_canvas) * q
```

`Rot(θ)` is counter-clockwise. Reflection happens before the rotation.

Two consequences of the flattened form, both faithful to `Live2DUnity`:

* a warp ancestor **only** pre-warps a rotation's pivot (and contributes the
  local rotation at that pivot to `θ_canvas`). It does not FFD or rescale the
  rotation's child points -- those are rotated and scaled by `totalScale`
  about the canvas-space pivot;
* a rotation's scale is a plain `px -> px` factor at every level. There is no
  `scale / ppu` step anywhere: the runtime never converts rotation offsets
  into warp cage parameters. (A `s / ppu` rule here collapses geometry on
  real data -- warp cages do not span `ppu` pixels.)

Root-parented nodes' geometry (cage points, mesh vertices, rotation pivots) is
already in canvas pixels.

## Keyforms

Every mesh/warp/rotation has a keyform grid (`KeyformGridSource`): a sparse
lattice of forms addressed by `(binding, keyIndex)` per parameter axis
(`KeyformGridAccessKey`). `KeyformBindingSource.keys` lists that axis's parameter
values. Evaluating at parameter values `V`:

1. per axis, bracket `V[param]` between `keys[i0]` and `keys[i1]` with `t`
   (clamp outside; `t=0` when only one key);
2. multilinear-interpolate the corner forms: each corner picks `i0` or `i1` per
   axis, weight = product of `t` or `1-t`;
3. a missing corner falls back to the nearest present corner (rare).

Form payloads (all linearly interpolatable):

* mesh form: `positions` (flat `x,y,…` in warp/rotation-local space), plus
  `opacity`, `drawOrder`, `multiplyColor`, `screenColor`
* warp form: `positions` (flat lattice points)
* rotation form: `angle` (degrees), `originX`, `originY`, `scale`,
  `isReflectX`, `isReflectY`

## model.json

```json
{
  "id": "001",
  "canvas": [2000, 2500],
  "ppu": 2000,
  "textures": ["texture_0.png"],
  "params": [ {"id": "PARAM_ANGLE_X", "min": -30, "max": 30, "def": 0, "repeat": false} ],
  "meshes": [ {
      "name": "D_PSD2_15", "tex": 0, "visible": true,
      // Mesh order IS the runtime's drawDataList order (parts in document
      // order, art meshes reversed within a part) -- the renderer sorts by
      // `drawOrder` and breaks ties on this order, exactly like
      // Live2DUnity's ModelContext.
      "parent": 4,                      // index into "deformers", -1 = root
      "blend": 0,                       // 0 normal, 1 multiply, 2 additive, 3 screen
      "doubleSided": false,
      "clip": { "meshes": [7, 8], "invert": false },
      "uvs": [0.31, 0.55, …],           // flat x,y in texture space
      "indices": [0, 1, 2, …],          // triangles
      "forms": [ [x, y, …], … ],        // per keyform, local space
      "grid": {
        "binds": [ {"param": 12, "keys": [0, 1]} ],
        "at":    [ {"at": [[0, 0], [1, 0]], "form": 0} ]
      }
  } ],
  "deformers": [ {
      "name": "B_EYE_BALL_31", "kind": "warp",   // or "rot"
      "parent": 3, "visible": true,
      "cols": 6, "rows": 6,                     // warp only
      "forms": [ [x, y, …], … ],                // warp: lattice points
      "rot":   [ [angle, ox, oy, sx, rx, ry], … ],  // rot only (per form)
      "grid": { … }
  } ]
}
```

`grid.binds[].param` indexes `params`. `grid.at[].at` is a list of
`[axis, keyIndex]` pairs (empty = the single base form). Floats are rounded to
4 decimals; the file is minified JSON.

## motion + physics data

The `.mtn` / `physics.json` files that ship next to `model.json` are the
original game files (copied from `StreamingAssets/BandoriLive2D/<id>/` by
`tools/live2d/build.mjs`); `catalog.json` indexes them per id. They are parsed
and played by `webui/src/live2d/{motion,physics,anim}.ts`, which port the
shipped runtime (`Live2DMotion`, `AMotion`, `MotionQueueManager`,
`PhysicsHair`, `L2DPhysics`, `L2DEyeBlink`) and
`BandoriMonopoly.UI.Live2DPortrait.RenderFrame`. This section is the data
contract for those files.

### `.mtn` (Live2DMotion.loadMotion)

```
# comment                     # to end of line
$fps=30                       # (5, 121); default 30
$fadein=MS  $fadeout=MS       # global fades; default 1000/1000
$fadein:PARAM=MS              # per-parameter override; replaces the global
$fadeout:PARAM=MS             # fade for the FIRST curve with that id
PARAM=v0,v1,...               # one sample per frame; one value = constant
VISIBLE:PARAM=...             # set, no lerp (the id keeps the prefix)
LAYOUT:ANCHOR_X|ANCHOR_Y|SCALE_X|SCALE_Y|X|Y=...
                              # parsed; updateParamExe never applies them
```

Numbers are the runtime's scanner: optional `-`, digits, optional `.` fraction
(no `+`, no exponent). Sampling (`updateParamExe`):

* `t = (now - startTimeMSec) * fps / 1000`; `i = floor(t)`, `frac = t - i`;
  sample index and `i+1` clamp to the last value.
* lerp between `i` and `i+1` **unless** `|v1 - v0| > 0.4 * (paramMax -
  paramMin)` (wrap/discontinuity guard -- hold `i`).
* fade weights are `fadeWeight(x)` = 0 for `x <= 0`, 1 for `x >= 1`, else
  `0.5 - 0.5*cos(x*PI)` (the cosine ease `live2d._3000_3000_0020_0020_0020`,
  not a linear clamp). Global: `fadeWeight((now - fadeInStart) / fadeInMsec)`
  and `fadeWeight((endTime - now) / fadeOutMsec)` (`fadeInMsec == 0` or
  `endTime < 0` short-circuits to 1). A curve with its own `$fadein:` /
  `$fadeout:` override replaces both globals for that curve.
* the final weight multiplies a lerp from the *current* parameter value toward
  the sample: `cur + (sample - cur) * w`. `VISIBLE:` curves write their sample
  straight through; `setParamFloat` is NaN -> 0 and clamps to `[min, max]`.
* `getDurationMSec = 1000 * maxLength / fps` while not looping (`-1` while
  looping). When `i >= maxLength`: looping restarts `startTimeMSec` /
  `fadeInStartTimeMSec` at `now`, otherwise the queue ent is marked finished.

`MotionQueueManager.startMotion` schedules a fade-out on every ent already
queued (each with *its own* `fadeOutMsec`) and appends the new one;
`updateParam` applies the queue in order and drops the finished ents.

### `physics.json` (L2DPhysics.load)

```json
{ "type": "Live2D Physics",
  "physics_hair": [ {
      "label": "hair front",
      "setup":  { "length": 0.2, "regist": 1, "mass": 0.3 },
      "src":    [ { "id": "PARAM_BODY_ANGLE_X", "ptype": "x", "scale": 0.007, "weight": 1 },
                  { "id": "PARAM_BODY_ANGLE_Z", "ptype": "angle", "scale": 0.8, "weight": 1 } ],
      "targets":[ { "id": "PARAM_HAIR_FRONT", "ptype": "angle", "scale": 0.025, "weight": 1 } ]
  } ] }
```

Each hair is a two-point spring (`PhysicsHair`): `setup.length` is the rest
length, `regist` the air resistance, `mass` both point masses. Sources project
a parameter into a force (`x`/`y` drags the root point with
`p += (scale * value - p) * weight`; `angle` tilts gravity the same way);
targets read the tip angle back into parameters
(`setParamFloat(id, scale * angle, weight)` -- or `angle_v`, the angular
velocity). The integration timestep is the caller's (`updateParam` passes
`now - startTimeMSec` in msec, `dt = (time - lastTime) / 1000`, unclamped);
the first call only seeds the clock (`startTime == 0` means "not started").

### RenderFrame

See [LIVE2D.md](LIVE2D.md) for the frame order. Two consequences worth
spelling out: the saved param base is the *motion output* while no reaction is
playing (so the idle blends from its own previous pose), and `L2DEyeBlink` is
enabled next to the idle and overwrites `PARAM_EYE_L_OPEN`/`PARAM_EYE_R_OPEN`
after the bake -- the eye blinks baked into `idle.mtn` never show. During a
reaction the bake and the blink are suppressed and reactions never stack.

## Converter notes

`main.xml` is an object graph: definitions carry `xs.id="#N"`, references
`xs.ref="#N"`, fields `xs.n="name"`. Field lookup is **deep** (values nest under
`xs.n="super"` wrappers). Key points:

* a `CArtMeshSource` / `CWarpDeformerSource` / `CRotationDeformerSource` is a
  definition only when it has `xs.id`; its forms are `CArtMeshForm` /
  `CWarpDeformerForm` / `CRotationDeformerForm` elements whose `_source` ref
  points at it and whose `guid` ref is the form's identity;
* the keyform grid maps combos to those form guids;
* `CParameterSource` holds `minValue`/`maxValue`/`defaultValue` and
  `CParameterId idstr=…`; the `parameterGuid` ref links bindings to params;
* clipping: `isClipping`, `clipGuidList`, `invertClippingMask` on the drawable
  source; blend: the `blend` field (values 0..3 as above);
* texture: one or more PNGs in the archive (`imageFileBuf.png` →
  `texture_0.png`); UVs are already normalized;
* `CAffine` matrices on texture elements are atlas-packing metadata — ignore.

## Renderer notes (`webui/src/live2d/`)

* WebGL (1 or 2) canvas; one program, textured triangles, premultiplied alpha,
  per-mesh blend mode and opacity; clipping via stencil or a mask draw.
* Upload each mesh's base positions once; per frame, if any parameter changed,
  re-evaluate keyforms and write the deformed positions (walk the chain).
* Parameter values come from the motion/physics pipeline (and the `params`
  prop underneath it): clamp to `min`/`max`, honor `repeat`.
* Draw in `drawOrder` (stable sort), skip `visible: false`.
* Fit: map canvas pixels to the viewport with a simple orthographic scale.