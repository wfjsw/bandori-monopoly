# Asset pipeline (P1)

Extracts web-ready assets from the Unity build into `webui/public/assets/` and copies
the game data JSON into `data/`.

```sh
python -m pip install UnityPy           # Pillow with WebP comes with it
python tools/asset-pipe/extract.py      # ~30 s first run; re-runs skip existing files
node tools/live2d/build.mjs          # compile the Live2D models (see docs/LIVE2D.md)
python tools/asset-pipe/check.py        # coverage check; exits 1 on any failure
```

Needs `ffmpeg` with `libopus` (found on PATH or via the WinGet link).
Flags: `--only img,audio,fonts,text,live2d,data`, `--force`, `--jobs N`, `--probe`.

## Optional setup from a sibling Unity build (macOS/Linux)

The existing Windows defaults are unchanged. Set `GAME_DATA_DIR` explicitly
to extract from another build. `GAME_JSON_DIR` overrides the source JSON export;
omit the `data` stage when using the repository's versioned game data.
From the repository root:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install -r tools/asset-pipe/requirements.txt
GAME_DATA_DIR=../BandoriMonopoly_Data .venv/bin/python tools/asset-pipe/extract.py --only img,audio,fonts,text,live2d
.venv/bin/python tools/asset-pipe/fonts.py
# Only when live2d/*.cxx3 archives are missing; obtain quadexec as documented below.
GAME_LIVE2D=../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D QUADRISM=/path/to/quadexec node tools/live2d/prepare.mjs
GAME_LIVE2D=../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D PYTHON="$PWD/.venv/bin/python" node tools/live2d/build.mjs
.venv/bin/python tools/asset-pipe/check.py
```

`FFMPEG=/path/to/ffmpeg` opts into a specific encoder. If ffmpeg is absent from
PATH, the optional requirements include a bundled binary: set
`FFMPEG="$(.venv/bin/python -c 'import imageio_ffmpeg; print(imageio_ffmpeg.get_ffmpeg_exe())')"`
on the extraction command. Without `FFMPEG`, PATH and WinGet lookup stay unchanged.
`PYTHON` overrides Live2D's interpreter only when explicitly set; its default is
still `python`. Archive preparation is a separate command and is never run by
`npm run build`. See [Live2D setup](../../docs/LIVE2D.md#preparing-missing-archives).

Extraction regenerates `manifest.json` for the selected Unity build. Keep that
local output with its matching extracted files; a different build can have
different Resources and scene entries. This setup does not migrate an existing
manifest or require collaborators to adopt a newer asset set.

## Output

| Path | Source | Format |
|---|---|---|
| `img/res/<resources path>.webp` | `Resources/` images, keyed by their original path | WebP q92 |
| `audio/<resources path>.ogg` | `Resources/` audio (BGM 96k, songs 96k, SE 64k, voice 32k mono) | Ogg/Opus |
| `text/<resources path>.json` | `Resources/` text assets (e.g. `bandorisongs/index`) | as-is |
| `img/char/`, `img/card/`, `img/band/`, `img/room/`, `img/fx/` | sprites referenced by `BandoriDatabase` | WebP |
| `img/scene/<name>.webp` | other sprites/textures used by scenes | WebP |
| `fonts/*.ttf` | embedded fonts (Source Han Rounded CN Medium/Bold, OFL; LiberationSans) | TTF |
| `live2d/` | `catalog.json` + `framing.json` from `StreamingAssets/BandoriLive2D`; models compiled by `tools/live2d/build.mjs` | `model.json` + PNG per id |
| `manifest.json` | index of all of the above | JSON |

### `manifest.json`

```jsonc
{
  "resources":  { "bandoribgm/21_enjoy": "audio/bandoribgm/21_enjoy.ogg", ... }, // lower-case keys
  "characters": { "001": { "stand": "...", "kv": "...", "sd": "...", "sdHappy": "...", "namePlate": "..." } },
  "bands":      { "Poppin' Party": "img/band/band_001.webp" },
  "rooms":      { "<band>": "img/room/..." },
  "cards":      { "AG:Y.O.L.O": "img/card/....webp" },          // card id from cards.json
  "fx":         { "tapRing": "...", "tapLight": "...", "tapStar": "...", "tapSparkle": "..." },
  "scene":      { "img": { "<name>": "..." }, "audio": {} },
  "fonts":      { "<font name>": "fonts/....ttf" },
  "live2d":     { "catalog": "live2d/catalog.json", "framing": "live2d/framing.json" }
}
```

Lookup rules the client must follow (they mirror the C#):

* Resources keys are **lower-case** (Unity stores them that way; `Resources.Load` is
  case-insensitive). Lower-case the C# path before looking it up.
* Character art is keyed by `art` if present, else `cnId` (`CharacterData.cs:34-38`).
* Newer characters have no `sdHappy` / `namePlate`, and 4 bands have no logo, in the
  original data too. The UI needs the same fallbacks as the Unity UI.

## How it works

* `Resources/` paths come from the `ResourceManager` in `globalgamemanagers`
  (the count depends on the build). Duplicate paths are encoded once.
  TextMeshPro atlases, shaders and style sheets are skipped.
* `BandoriDatabase` is a MonoBehaviour whose type tree is stripped in player builds.
  `unitydb.py` parses its raw bytes using the field order from the decompiled
  `BandoriDatabase.cs`, and asserts the parse ends exactly at the end of the object.
* Decoding runs on the main thread; WebP and Opus encoding run in a thread pool.

## Regression checks

```sh
python -m unittest discover -s tools/asset-pipe -p 'test_*.py'
node --test tools/live2d/test_build.mjs
```
