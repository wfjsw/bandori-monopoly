#!/usr/bin/env python3
"""Load the ruleset roster (cards / characters / bands) for tools.

The three `data/{cards,characters,bands}.json` files are gone; the roster lives
in the Rust rule code and is exported by the ruleset (ABI v63). Tools that need
JSON rows call these helpers against a dump written by `tools/roster-dump.mjs`
(or `cargo run -p rules-native --example dump_roster -- <dir>`).

Resolution order for the dump directory:

  1. ``ROSTER_DIR`` environment variable
  2. ``<repo>/target/roster-dump/`` (the default ``tools/roster-dump.mjs`` out)

Missing dump -> a clear error naming the command to run. Display-only columns
(``simple``, ``counterReason``) live outside the engine structs and are **not**
in the dump; read ``data/skill_simple.json`` for those.

Usage (isolated mode safe, stdlib only)::

    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
    from roster import load_cards, load_characters, load_bands
"""

from __future__ import annotations

import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DUMP = ROOT / "target" / "roster-dump"
_FILES = {
    "cards": ("cards.json", "cards"),
    "characters": ("characters.json", "characters"),
    "bands": ("bands.json", "bands"),
}


def roster_dir() -> Path:
    """Directory holding the three dump files (see module docstring)."""
    env = os.environ.get("ROSTER_DIR")
    return Path(env) if env else DEFAULT_DUMP


def _load(kind: str) -> list:
    fname, key = _FILES[kind]
    path = roster_dir() / fname
    if not path.is_file():
        raise SystemExit(
            f"roster dump missing: {path}\n"
            f"  run: node tools/roster-dump.mjs\n"
            f"  (or set ROSTER_DIR to a directory written by\n"
            f"   `cargo run -p rules-native --example dump_roster -- <dir>`)"
        )
    doc = json.loads(path.read_text(encoding="utf-8-sig"))
    return doc[key] if isinstance(doc, dict) else doc


def load_cards() -> list:
    return _load("cards")


def load_characters() -> list:
    return _load("characters")


def load_bands() -> list:
    return _load("bands")