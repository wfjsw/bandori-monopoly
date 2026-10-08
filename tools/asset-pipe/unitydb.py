"""Manual reader for the BandoriDatabase ScriptableObject.

Player builds strip MonoBehaviour type trees, so UnityPy cannot read the custom
fields. The layout below follows the field declarations in the decompiled
`BandoriMonopoly/BandoriDatabase.cs` (declaration order == serialization order).
`read_database` asserts that parsing ends exactly at the end of the object, so a
layout mismatch fails loudly instead of producing garbage.
"""

from __future__ import annotations

import os
import struct
from dataclasses import dataclass, field


@dataclass
class PPtr:
    file_id: int
    path_id: int

    @property
    def null(self) -> bool:
        return self.file_id == 0 and self.path_id == 0


@dataclass
class Database:
    text_assets: dict[str, PPtr]
    rules_version: int
    character_art: list[dict] = field(default_factory=list)  # {id, stand, kv, sd, sdHappy, namePlate}
    band_art: list[dict] = field(default_factory=list)       # {band, logo}
    room_art: list[dict] = field(default_factory=list)       # {band, card}
    card_art: list[dict] = field(default_factory=list)       # {id, art}
    fx: dict[str, PPtr] = field(default_factory=dict)        # tapRing, tapLight, tapStar, tapSparkle


class _Reader:
    def __init__(self, data: bytes, endian: str):
        self.d, self.o, self.e = data, 0, endian

    def i32(self) -> int:
        v = struct.unpack_from(self.e + "i", self.d, self.o)[0]
        self.o += 4
        return v

    def i64(self) -> int:
        v = struct.unpack_from(self.e + "q", self.d, self.o)[0]
        self.o += 8
        return v

    def u8(self) -> int:
        v = self.d[self.o]
        self.o += 1
        return v

    def align(self) -> None:
        self.o = (self.o + 3) & ~3

    def string(self) -> str:
        n = self.i32()
        if n < 0 or self.o + n > len(self.d):
            raise ValueError(f"bad string length {n} at {self.o - 4}")
        s = self.d[self.o:self.o + n].decode("utf-8")
        self.o += n
        self.align()
        return s

    def pptr(self) -> PPtr:
        return PPtr(self.i32(), self.i64())

    def array(self, item):
        n = self.i32()
        if n < 0 or n > 100_000:
            raise ValueError(f"bad array length {n} at {self.o - 4}")
        return [item() for _ in range(n)]


TEXT_FIELDS = ["boardJson", "charactersJson", "cardsJson", "bandsJson", "rulesText", "homeLinesJson",
               "voiceLinesJson", "eventsJson", "schoolsJson", "songCardsJson", "matchRulesJson"]
CHAR_SPRITES = ["stand", "kv", "sd", "sdHappy", "namePlate"]
FX_FIELDS = ["tapRing", "tapLight", "tapStar", "tapSparkle"]


# Newer player builds insert additional text assets and an sdJoy sprite.
# Keep the original public field lists and legacy result shape unchanged.
EXTENDED_TEXT_FIELDS = [*TEXT_FIELDS[:7], "cardLinesJson", "emotesJson", *TEXT_FIELDS[7:], "skillSimpleJson"]
EXTENDED_CHAR_SPRITES = [*CHAR_SPRITES[:4], "sdJoy", *CHAR_SPRITES[4:]]


def read_database(raw: bytes, big_endian: bool = False) -> tuple[str, Database]:
    for texts, sprites in ((TEXT_FIELDS, CHAR_SPRITES), (EXTENDED_TEXT_FIELDS, EXTENDED_CHAR_SPRITES)):
        try:
            return _read_database(raw, big_endian, texts, sprites)
        except (ValueError, IndexError, struct.error):
            continue
    raise ValueError(f"BandoriDatabase layout mismatch: no supported layout matches {len(raw)} bytes")


def _read_database(raw: bytes, big_endian: bool, text_fields: list[str], char_sprites: list[str]) -> tuple[str, Database]:
    r = _Reader(raw, ">" if big_endian else "<")
    # MonoBehaviour base
    r.pptr()          # m_GameObject
    r.u8(); r.align() # m_Enabled
    r.pptr()          # m_Script
    name = r.string() # m_Name
    texts = {f: r.pptr() for f in text_fields}
    db = Database(text_assets=texts, rules_version=r.i32())
    db.character_art = r.array(lambda: {"id": r.string(), **{k: r.pptr() for k in char_sprites}})
    db.band_art = r.array(lambda: {"band": r.string(), "logo": r.pptr()})
    db.room_art = r.array(lambda: {"band": r.string(), "card": r.pptr()})
    db.card_art = r.array(lambda: {"id": r.string(), "art": r.pptr()})
    db.fx = {k: r.pptr() for k in FX_FIELDS}
    if r.o != len(raw):
        raise ValueError(f"BandoriDatabase layout mismatch: parsed {r.o} of {len(raw)} bytes")
    return name, db


class Resolver:
    """Resolve PPtrs across every serialized file in a loaded UnityPy environment."""

    def __init__(self, env):
        self.by_key = {}
        for obj in env.objects:
            self.by_key[(os.path.basename(obj.assets_file.name).lower(), obj.path_id)] = obj

    def resolve(self, owner, p: PPtr):
        if p.null:
            return None
        if p.file_id == 0:
            fname = owner.assets_file.name
        else:
            fname = owner.assets_file.externals[p.file_id - 1].path
        return self.by_key.get((os.path.basename(fname).lower(), p.path_id))
