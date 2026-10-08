#!/usr/bin/env python3
"""Unpack Quadrism's OMG3 "vault" archives (.cxx3) into raw source files.

The container (`CAFF`, see quadrism `archive/vault`) holds a XOR-keyed directory
and compressed entries; the converter wants the raw pieces: the model XML
(`main.xml`), the PNG textures and the motion/physics side files. Those raw
extracts are **build inputs** and must never be served to the browser (`main.xml`
alone is ~5 MB per model), so they land outside the web tree:

  web/live2d-src/<id>/main.xml
  web/live2d-src/<id>/imageFileBuf.png
  ...

`tools/live2d/convert.py` then compiles `main.xml` into the served
`webui/public/assets/live2d/<id>/` (model.json + texture PNGs only).

Some archives embed fewer PNGs than their `main.xml` references (27 of the 55
models name a second `imageFileBuf_1.png` that the CAFF does not carry). Those
bytes come from the original game's `BandoriLive2D/<id>/texture_<n>.png`, which
is byte-identical to the archive's `imageFileBuf.png` for texture 0.

Usage:
  python tools/live2d/unpack.py            # web/live2d/*.cxx3 -> web/live2d-src/<id>/
  python tools/live2d/unpack.py 001        # just one model
"""

import os
import re
import struct
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
IN = ROOT / "live2d"
OUT = ROOT / "live2d-src"
GAME = ROOT / os.environ.get("GAME_LIVE2D", "../BandoriMonopoly_Data/StreamingAssets/BandoriLive2D")

MAGIC = b"CAFF"
HEADER_SIZE = 0x3A
COMPRESS_RAW = 0x10


def varint(data: bytes, pos: int, key: int) -> tuple[int, int]:
    out = 0
    for i in range(10):
        byte = data[pos] ^ key
        pos += 1
        if i == 0 and byte == 0x80:
            raise ValueError("non-canonical varint")
        out = (out << 7) | (byte & 0x7F)
        if byte & 0x80 == 0:
            return out, pos
    raise ValueError("varint too long")


def masked_u32(data: bytes, off: int, mask: int) -> int:
    return struct.unpack_from(">I", data, off)[0] ^ mask


def masked_u64(data: bytes, off: int, mask: int) -> int:
    # quadrism `sign_extended_mask`: the u32 key duplicated into both halves
    signed = mask if mask < 0x80000000 else mask - 0x100000000
    mask64 = ((signed & 0xFFFFFFFF) << 32) | (signed & 0xFFFFFFFF)
    return struct.unpack_from(">Q", data, off)[0] ^ mask64


def inflate(raw: bytes, compression: int) -> bytes:
    if compression == COMPRESS_RAW:
        return raw
    # a compressed entry is a standard zip local file record (method 0 stored or
    # 8 raw deflate) -- see quadrism `zip_read_xml`
    if raw[:4] != b"PK\x03\x04":
        raise ValueError("zip payload does not start with a local file record")
    method = struct.unpack_from("<H", raw, 8)[0]
    csize = struct.unpack_from("<I", raw, 18)[0]
    name_len = struct.unpack_from("<H", raw, 26)[0]
    extra_len = struct.unpack_from("<H", raw, 28)[0]
    start = 30 + name_len + extra_len
    body = raw[start:start + csize] if csize else raw[start:]
    if method == 0:
        return body
    if method == 8:
        return zlib.decompress(body, -15)
    raise ValueError(f"unsupported zip method {method}")


def unpack(path: Path) -> dict[str, bytes]:
    data = path.read_bytes()
    if data[:4] != MAGIC:
        raise ValueError(f"{path.name}: not a CAFF archive")
    stored = struct.unpack_from("<I", data, 0x0E)[0]
    field_key = struct.unpack(">I", struct.pack("<I", stored))[0]
    key = field_key & 0xFF
    count = masked_u32(data, 0x36, field_key)
    pos = HEADER_SIZE
    entries = []
    for _ in range(count):
        nlen, pos = varint(data, pos, key)
        name = bytes(b ^ key for b in data[pos:pos + nlen]).decode("utf-8", "replace")
        pos += nlen
        alen, pos = varint(data, pos, key)
        alias = bytes(b ^ key for b in data[pos:pos + alen]).decode("utf-8", "replace")
        pos += alen
        meta = data[pos:pos + 22]
        pos += 22
        offset = masked_u64(meta, 0, field_key)
        size = masked_u32(meta, 8, field_key)
        obfuscated = (meta[12] ^ key) != 0
        compression = meta[13] ^ key
        raw = data[offset:offset + size]
        if obfuscated:
            raw = bytes(b ^ key for b in raw)
        entries.append((name or alias, inflate(raw, compression)))
    return dict(entries)


def referenced_files(xml: bytes) -> list[str]:
    """`<file … path="…"/>` entries named by the document (textures)."""
    return re.findall(rb'<file[^>]*\spath="([^"]+)"', xml)


def fill_missing(d: Path, mid: str, parts: dict[str, bytes]) -> list[str]:
    """Copy PNGs the XML references but the archive did not carry."""
    xml = parts.get("main.xml")
    if xml is None:
        return []
    filled = []
    for path in referenced_files(xml):
        name = path.decode("utf-8", "replace")
        if name in parts or (d / name).exists():
            continue
        # imageFileBuf.png -> texture_0.png, imageFileBuf_1.png -> texture_1.png, …
        stem = Path(name).stem
        m = re.search(r"_(\d+)$", stem)
        idx = int(m.group(1)) if m else 0
        cand = GAME / mid / f"texture_{idx}.png"
        if not cand.exists():
            print(f"{mid}: missing {name} (no {cand})")
            continue
        (d / name).write_bytes(cand.read_bytes())
        filled.append(name)
    return filled


def main() -> int:
    only = sys.argv[1] if len(sys.argv) > 1 else None
    files = sorted(IN.glob("*.cxx3"))
    if only:
        files = [f for f in files if f.stem == only]
    if not files:
        print(f"no .cxx3 in {IN} (run Quadrism conv first)")
        return 1
    ok = 0
    for f in files:
        try:
            parts = unpack(f)
        except Exception as e:  # noqa: BLE001 - report and keep going
            print(f"{f.name}: {e}")
            continue
        d = OUT / f.stem
        d.mkdir(parents=True, exist_ok=True)
        for name, blob in parts.items():
            safe = name.replace("/", "_")
            (d / safe).write_bytes(blob)
        filled = fill_missing(d, f.stem, parts)
        names = ", ".join(f"{n} ({len(b)}B)" for n, b in parts.items())
        extra = f" +from-game {', '.join(filled)}" if filled else ""
        print(f"{f.stem}: {names}{extra}")
        ok += 1
    print(f"unpacked {ok}/{len(files)} archives into {OUT}")
    return 0 if ok == len(files) else 1


if __name__ == "__main__":
    raise SystemExit(main())