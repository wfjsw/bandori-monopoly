# Asset pipeline (P1)

Extracts web-ready assets from the Unity build into `webui/public/assets/` and copies
the game data JSON into `data/`.

```sh
python -m pip install UnityPy           # Pillow with WebP comes with it
python tools/asset-pipe/extract.py      # ~30 s first run; re-runs skip existing files
sh tools/live2d/build.sh                # compile the Live2D models (see docs/LIVE2D.md)
python tools/asset-pipe/check.py        # coverage check; exits 1 on any failure
```

Needs `ffmpeg` with `libopus` (found on PATH or via the WinGet link).
Flags: `--only img,audio,fonts,text,live2d,data`, `--force`, `--jobs N`, `--probe`.

The TTS board is a separate, versioned asset at
`webui/public/assets/tts/board.png`, copied unchanged from the local Workshop
cache for mod `3506424344`, board object `53c41e`. `tts/source.json` records the
cache filename, dimensions, and SHA-256. It is independent of this Unity
extractor and needs no network access. The board UI preserves its 1800 × 1500
(6:5) aspect ratio; its 60 clickable cells match the mod's Lua `pathXY`.

## Output

| Path | Source | Format |
|---|---|---|
| `img/res/<resources path>.webp` | `Resources/` images, keyed by their original path | WebP q92 |
| `audio/<resources path>.ogg` | `Resources/` audio (BGM 96k, songs 96k, SE 64k, voice 32k mono) | Ogg/Opus |
| `text/<resources path>.json` | `Resources/` text assets (e.g. `bandorisongs/index`) | as-is |
| `img/char/`, `img/card/`, `img/band/`, `img/room/`, `img/fx/` | sprites referenced by `BandoriDatabase` | WebP |
| `img/scene/<name>.webp` | other sprites/textures used by scenes | WebP |
| `fonts/*.ttf` | embedded fonts (Source Han Rounded CN Medium/Bold, OFL; LiberationSans) | TTF |
| `live2d/` | `catalog.json` + `framing.json` from `StreamingAssets/BandoriLive2D`; models compiled by `tools/live2d/build.sh` | `model.json` + PNG per id |
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
  (619 entries). TextMeshPro atlases, shaders and style sheets are skipped.
* `BandoriDatabase` is a MonoBehaviour whose type tree is stripped in player builds.
  `unitydb.py` parses its raw bytes using the field order from the decompiled
  `BandoriDatabase.cs`, and asserts the parse ends exactly at the end of the object.
* Decoding runs on the main thread; WebP and Opus encoding run in a thread pool.
