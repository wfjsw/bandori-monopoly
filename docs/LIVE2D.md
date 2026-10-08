# Live2D pipeline and UI

How a Bandori Live2D model becomes a stand on the web stage. The data contract
between the converter and the renderer -- coordinate spaces, deform math and the
`model.json` shape -- is [LIVE2D-SCHEMA.md](LIVE2D-SCHEMA.md).

## Pipeline

The original game ships Cubism 2 models twice over: as the `model.moc` folder in
`BandoriMonopoly_Data/StreamingAssets/BandoriLive2D/<id>/`, and as Quadrism's
converted archives in `web/live2d/<id>.cxx3` (an OMG3 `CAFF` vault). The web
build compiles the archives into the compact `model.json` the browser renderer
consumes:

```
BandoriLive2D/<id>/model.moc            (Cubism 2 editor export)
  -> Quadrism conv                      -> web/live2d/<id>.cxx3
  -> tools/live2d/unpack.py             -> web/live2d-src/<id>/main.xml + imageFileBuf*.png
  -> tools/live2d/convert.py            -> webui/public/assets/live2d/<id>/model.json + texture_<n>.png
  -> tools/live2d/selfcheck.py          -> structural + geometry self-check
  -> webui/src/live2d/ (Live2DModel)    -> WebGL canvas
```

* `unpack.py` extracts the raw pieces of the vault. They are **build inputs**:
  `main.xml` alone is ~5 MB per model and must never be served to the browser.
* `convert.py` compiles the Cubism 2 editor XML (`com.live2d.cubism.doc`) into
  `model.json` exactly as [LIVE2D-SCHEMA.md](LIVE2D-SCHEMA.md) describes. 27 of
  the 55 archives embed one PNG fewer than their XML references; the missing
  texture bytes come from the original game's `BandoriLive2D/<id>/texture_<n>.png`.
* `build.mjs` also copies each model's `*.mtn` motion curves and `physics.json`
  out of the game's `StreamingAssets/BandoriLive2D/<id>/` (override the root
  with `GAME_LIVE2D=…`) -- they are tiny (~12 MB for all 55 models) and are what
  `catalog.json` indexes per id (`motion`, `reactions`, `physics`).
* Each `webui/public/assets/live2d/<id>/` holds **only** `model.json`,
  `texture_<n>.png`, `*.mtn` and `physics.json` (55 model.json + 82 textures +
  757 motions + 55 physics). `main.xml` and `model.moc` never ship.

## Rebuilding the assets

```sh
node tools/live2d/build.mjs                  # unpack + convert + copy motions/physics
FORCE=1 CHECK=1 node tools/live2d/build.mjs  # rebuild all + self-check all
```

`build.mjs` runs unpack and convert per model and skips a model whose outputs are
newer than its inputs, copies the `.mtn` / `physics.json` files that are missing
or older than the game's, then sweeps every output directory down to
`model.json` + `texture_<n>.png` + `*.mtn` + `physics.json`. It is wired into
the webui build as the `prebuild` script (`webui/package.json`), so
`npm run build` always leaves the served tree complete. `FORCE=1` ignores
timestamps; `CHECK=1` runs `tools/live2d/selfcheck.py` over every model instead
of only the rebuilt ones.

The catalog and the measuring sheet next to the models
(`webui/public/assets/live2d/catalog.json`, `framing.json`) are copied from the
game's `StreamingAssets/BandoriLive2D` by `tools/asset-pipe/extract.py`;
`manifest.json` points at them under `live2d`. `catalog.json` lists the motions
and physics the original plays per id (`motion` = the idle, `reactions` = the
tap set, `physics` = the hair/clothes springs); the web renderer plays them
through the ported `Live2DPortrait.RenderFrame` (below).

### Preparing missing archives

If `live2d/<id>.cxx3` archives are unavailable, create them explicitly from the
game's `model.moc` files with [Quadrism](https://codeberg.org/Podimium/Quadrism).
This optional step is not part of `build.mjs` or the npm prebuild. Existing
archives are kept even when the game's source files are newer; `FORCE=1` opts
into replacing them.

The following Quadrism revision was verified locally. Its wildcard quick-xml
dependency needs the documented 0.41 version pinned. From the repository root:

```sh
git clone https://codeberg.org/Podimium/Quadrism.git /tmp/bandori-assets-quadrism
git -C /tmp/bandori-assets-quadrism checkout cb3f8557bdb2ebfd98c685c23ec47400d767423d5f2f3b4e45e889f3d369a60c
CARGO_HOME=/tmp/bandori-assets-cargo cargo update --manifest-path /tmp/bandori-assets-quadrism/Cargo.toml -p quick-xml --precise 0.41.0
CARGO_HOME=/tmp/bandori-assets-cargo cargo build --manifest-path /tmp/bandori-assets-quadrism/Cargo.toml --profile fast-release --workspace --locked
GAME_LIVE2D=../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D QUADRISM=/tmp/bandori-assets-quadrism/target/fast-release/quadexec node tools/live2d/prepare.mjs
```

Preparation passes every `texture_<n>.png` with `--tex atlas`. Afterwards run
the usual `build.mjs`; set `PYTHON=/path/to/python` if needed (the default remains
`python`). `GAME_LIVE2D` is shared by preparation, texture fallback and motion
copying. Catalog/framing metadata still comes from `extract.py`.

## Renderer

`webui/src/live2d/` is a Cubism 2 renderer over WebGL: it fetches `model.json`,
evaluates the keyform grid and the deform chain
([LIVE2D-SCHEMA.md](LIVE2D-SCHEMA.md)), and draws the textured meshes with the
per-mesh blend modes and clipping. Public API:

```ts
import { Live2DModel, loadLive2D, /* ...deform helpers, see index.ts */ } from "../live2d";

<Live2DModel id="001" params={{ PARAM_ANGLE_X: 5 }} className={styles.stand} />
```

* `id` -- the model folder under `webui/public/assets/live2d`.
* `params` -- `PARAM_*` values; omitted keys use the model defaults. They are
  layered *under* the motion (they seed the saved base the motion blends from),
  and clamped to the model's `min`/`max`.
* `className` -- goes on the canvas wrapper (it fills its own box).
* `ref` -- `Live2DModelHandle` with `playReaction()`.

### Motion, physics, blink

The stands are alive: idle motion loops by default, clicking the stand plays a
random one-shot reaction, and hair/clothes physics runs on top. The data files
are the shipped ones (`idle.mtn`, the `reactions` set, `physics.json`), parsed
and played by `webui/src/live2d/{motion,physics,anim}.ts` -- ports of
`Live2DMotion` / `AMotion` / `MotionQueueManager`, `PhysicsHair` / `L2DPhysics`
and `Live2DPortrait.RenderFrame()` with the frame order the game uses:

```
loadParam()                       // restore the saved base
if (queue finished) restart idle  // and clear IsPlayingReaction
queue.updateParam()               // idle + any reaction blend into params
if (!reaction) saveParam()        // motion bakes into the saved base
if (!reaction) blink.updateParam()// L2DEyeBlink overwrites the eye params
physics.updateParam()             // spring chain writes hair/clothes params
model.update()                    // evaluate the pose
```

* **idle** -- `catalog.json` `motion`; one-shot but restarted by the queue
  whenever it empties, exactly like the game (the file's `$fadein`/`$fadeout`
  apply).
* **reactions** -- `catalog.json` `reactions`, loaded with `LoadOneShot`'s
  250/350ms fades and `loop = false`. A click plays one at random (never the
  one that just played) and sets `IsPlayingReaction`, which suppresses the bake
  and the blink until the queue empties. Reactions never stack.
* **physics** -- `physics.json` (`"type": "Live2D Physics"`, `physics_hair[]`
  with `setup {length, regist, mass}`, `src` and `targets`), integrated with the
  shipped spring math on every frame, reactions included.
* **blink** -- the game enables `L2DEyeBlink` next to the idle, so the eye
  blinks baked into `idle.mtn` are overwritten at runtime (only a playing
  reaction shows its own eye curves). The web port matches that instead of
  stacking a second blink layer on the baked one.
* **debug** -- `window.__live2dTick(dtMsec)` steps the mounted stand by hand
  (for throttled tabs) and returns `{ values, isPlayingReaction, now }`.
  `window.__live2dState()` dumps the motion queue (per-ent start/end, `loop`,
  `maxLength`) and `window.__live2dForce(id, v)` pins one parameter (it also
  drops the idle so only physics writes) -- both exist so a test can watch a
  reaction finish and a spring settle without taking browser focus.

Node-side checks: `node src/live2d/check.mjs` (deform math) and
`node src/live2d/check-motion.mjs` (parser, sampling, fades, loop restart,
frame semantics, real shipped `.mtn` reactions, physics stability).

## UI: `Live2DStand`

`webui/src/ui/Live2DStand.tsx` places a model where a stand image used to be and
keeps the layout of the scene around it:

```tsx
<Live2DStand id={D.artId(c)} className={s.stand} zoom={1.36} focusTop={0.14} headroom={0.025} />
```

* **id mapping** -- a model id *is* the character art id: `D.artId(c)`
  (`c.art ?? c.cnId`, see `webui/src/core/data.ts` and `data/characters.json`),
  e.g. `"001"` (户山香澄), `"036c"` (高松灯（CRYCHIC）). 54 characters, 54 models;
  `015m` (美咲, the `015` alt look in `catalog.json`) has no character yet.
* **fallback** -- the static stand art (`charArt(id, "stand")`) stays on screen
  until the model is loaded and drawn, and stays for good when the model is
  missing or fails to load (the same fallback `Live2DPortrait` keeps). Once the
  model paints, the art fades out and is dropped from the tree ~500ms later --
  the drop is a timer, not `transitionend`, because a hidden tab freezes CSS
  transition timelines and would leave the art ghosting through the canvas.
* **framing** -- the model draws into a 3:4 window centred in the stand box and
  is fitted exactly like the original `BandoriMonopoly.UI.Live2DPortrait.Draw`:
  `zoom` / `focusTop` / `headroom` are that call's arguments, and `framing.json`
  supplies the per-model `top` / `scale` measuring sheet. Call-site values match
  the game: menu `1.36 / 0.14 / 0.025` (`HomeCharacterView`), gallery detail
  `1 / 0 / 0.103` (`CharacterDetailView`), character select `1 / 0 / 0.03`
  (`CharacterSelectController`). The deck editor stand is a web addition
  (the game keeps static art there) and uses the gallery values.
* **strings** -- `live2d.loading` while the model loads, `live2d.missing` when
  there is neither a model nor stand art (`webui/src/i18n/locales/*/ui.json`).
* **tap** -- clicking the stand plays a random reaction (`TryPlayReaction`) and
  still runs the scene's `onClick` handler (the menu dialogue lines).

Wired scenes: `scenes/menu/Menu.tsx` (home stand), `scenes/gallery/Gallery.tsx`
(character detail), `scenes/select/Select.tsx` (pick preview),
`scenes/deck/Deck.tsx` (deck character). The character-grid thumbnails and the
board SD pieces stay static: one WebGL context per stand, not fifty.

## Not ported yet

* lip sync (the menu voice lines) and the per-character look switcher
  (`catalog.json` `altOf`, `ProfileService.SetLive2D`, e.g. `015` <-> `015m`).
* named motion playback (`Live2DPortrait.PlayMotion`, `motionsFrom` in
  `catalog.json`) -- only the idle and the tap reaction set are wired.