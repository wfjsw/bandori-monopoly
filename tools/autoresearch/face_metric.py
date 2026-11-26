#!/usr/bin/env python3
"""Autoresearch face metric: compare display cards against the vendored library.

Reads only the ``## Cards`` sections of ``--library/*.md``; a body is the
following non-empty lines (trailing whitespace removed) up to the next heading.
``events.md`` is skipped (event faces are a follow-up; pairs cover roster faces).
Both sides are normalized: an optional ``[衍生]`` derived marker, an optional
leading ``[character]``, and a leading/trailing balanced ``（...）`` annotation
are stripped before comparison.

Untitled faces pair through upstream's literal ``unnamed`` marker: an empty
local title, or an internal ``#`` id, matches it. A prefix-only local face
(``HHW:（育美）``) may also front a titled library entry
(``[北泽育美]We Can☆Hurray!``); the local short name must be a suffix of the
pair's ``[full name]`` annotation. Any other empty-vs-nonempty title is drift.
``unpaired_library`` counts library entries whose normalized key no pair uses.

Every metric is one stdout line ``METRIC <name>=<value>``; up to ten mismatched
paired ids follow as ``ASI synced=0 id=<id>``. A completed comparison always
exits 0; malformed input or unreadable infrastructure exits 2.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

KNOWN_LOCAL_ONLY = {"R:[衍生] 压"}
MAX_ASI = 10
DERIVED_PREFIX = "[衍生]"
CARD_HEADING = re.compile(r"##\s+Cards\s*")
ENTRY_HEADING = re.compile(r"###\s+(.*\S)\s*")
SUBHEADING = re.compile(r"####")
HEADING = re.compile(r"#{1,6}\s")
# Event faces live in events.md's own `## Cards` section; pairs.json covers the
# roster only, so event entries would masquerade as unpaired library rows.
EXCLUDED_FILES = {"events.md"}


class MetricError(Exception):
    """Parse or infrastructure failure: exit 2, never a metric value."""


def fail(message: str) -> "MetricError":
    return MetricError(message)


def strip_leading_paren(text: str) -> str:
    """Drop one balanced leading ``(...)`` / ``（...）`` annotation."""
    if not text or text[0] not in "(（":
        return text
    depth = 0
    for i, ch in enumerate(text):
        if ch in "(（":
            depth += 1
        elif ch in ")）":
            depth -= 1
            if depth == 0:
                return text[i + 1 :]
    return text


def strip_trailing_paren(text: str) -> str:
    """Drop one balanced trailing ``(...)`` / ``（...）`` annotation."""
    if not text or text[-1] not in ")）":
        return text
    depth = 0
    for i in range(len(text) - 1, -1, -1):
        ch = text[i]
        if ch in ")）":
            depth += 1
        elif ch in "(（":
            depth -= 1
            if depth == 0:
                return text[:i]
    return text


def leading_paren_text(text: str) -> str | None:
    """Inner text of one balanced leading ``(...)`` / ``（...）`` group."""
    if not text or text[0] not in "(（":
        return None
    depth = 0
    for i, ch in enumerate(text):
        if ch in "(（":
            depth += 1
        elif ch in ")）":
            depth -= 1
            if depth == 0:
                return text[1:i]
    return None


def leading_bracket_text(text: str) -> str | None:
    """Inner text of a leading ``[character]`` annotation."""
    match = re.match(r"^\[([^\]]*)\]", text.strip())
    return match.group(1) if match else None


def title_matches(
    local_id: str,
    local_norm: str,
    pair_title: str,
    pair_norm: str,
    pair_derived: bool,
    local_derived: bool,
) -> bool:
    """Whether a local card title names the paired library title.

    ``unnamed`` is upstream's literal marker for untitled faces. A prefix-only
    local id (its ``（character）`` annotation normalizes away to ``""``) or an
    internal ``#`` id pairs with it. A prefix-only id can also front a titled
    card (``HHW:（育美）`` -> ``[北泽育美]We Can☆Hurray!``); there the local
    short name must be a suffix of the pair's ``[full name]`` annotation.
    """
    if local_derived != pair_derived:
        return False
    if pair_norm == "unnamed" and (local_norm == "" or local_norm.startswith("#")):
        return True
    if local_norm == "":
        short = leading_paren_text(local_id.split(":", 1)[-1])
        full = leading_bracket_text(pair_title)
        return bool(short and full and full.endswith(short))
    return local_norm == pair_norm


def normalize_title(title: str) -> tuple[bool, str]:
    """(derived flag, comparison title)."""
    text = title.strip()
    derived = text.startswith(DERIVED_PREFIX)
    if derived:
        text = text[len(DERIVED_PREFIX) :]
    text = re.sub(r"^\[[^\]]*\]", "", text)  # leading [character name]
    text = strip_leading_paren(text)
    text = strip_trailing_paren(text)
    return derived, text.strip()


def normalize_local_id(local_id: str) -> tuple[bool, str]:
    """(derived flag, comparison title) for a ``<PREFIX>:<title>`` display id."""
    text = local_id.split(":", 1)[1] if ":" in local_id else local_id
    return normalize_title(text)


def normalize_text(text: str) -> str:
    return re.sub(r"\s+", "", text)


def parse_library(library: Path) -> tuple[list[str], dict[tuple[bool, str], list[str]]]:
    """Parse ``*.md`` into ordered raw titles and a normalized-title body index."""
    if not library.is_dir():
        raise fail(f"library directory not found: {library}")
    files = sorted(
        path for path in library.glob("*.md") if path.name not in EXCLUDED_FILES
    )
    if not files:
        raise fail(f"no *.md files under {library}")

    titles: list[str] = []
    index: dict[tuple[bool, str], list[str]] = {}

    for path in files:
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError as exc:
            raise fail(f"cannot read {path}: {exc}") from exc

        seen: set[str] = set()
        in_cards = False
        current: str | None = None
        body: list[str] = []

        def close() -> None:
            nonlocal current, body
            if current is None:
                return
            derived, norm = normalize_title(current)
            # `unnamed` is upstream's placeholder for untitled cards and may
            # legitimately repeat (Sumimi has two, both pair targets); every
            # other repeated title makes the body index ambiguous.
            if current in seen and current != "unnamed":
                raise fail(f"duplicate card title in {path.name}: {current!r}")
            seen.add(current)
            titles.append(current)
            index.setdefault((derived, norm), []).append("\n".join(body))
            current = None
            body = []

        for line in lines:
            if not in_cards:
                if line.rstrip() == "## Cards":
                    in_cards = True
                continue
            if CARD_HEADING.fullmatch(line.rstrip()):
                continue
            if line.startswith("### "):
                close()
                match = ENTRY_HEADING.fullmatch(line.rstrip())
                if not match:
                    raise fail(f"empty card title in {path.name}")
                current = match.group(1)
                continue
            if SUBHEADING.match(line):
                close()
                continue
            if HEADING.match(line):  # next level-2 section ends ## Cards
                close()
                in_cards = False
                continue
            if current is not None and line.strip():
                body.append(line.rstrip())
        close()

    return titles, index


def load_json(path: Path, label: str) -> dict:
    if not path.is_file():
        raise fail(f"{label} not found: {path}")
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise fail(f"cannot load {label} {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise fail(f"{label} must be a JSON object: {path}")
    return data


def pair_is_synced(pairs, cards, index, local_id: str) -> bool:
    """Both title and body must line up for one paired id."""
    derived, norm = normalize_local_id(local_id)
    t_derived, t_norm = normalize_title(pairs[local_id])
    bodies = index.get((t_derived, t_norm))
    if local_id not in cards or not bodies:
        return False
    title_ok = title_matches(
        local_id, norm, pairs[local_id], t_norm, t_derived, derived
    )
    card = cards[local_id]
    text = card.get("text", "") if isinstance(card, dict) else ""
    text_ok = any(normalize_text(body) == normalize_text(text) for body in bodies)
    return title_ok and text_ok


def find_mismatches(
    pairs: dict[str, str],
    cards: dict,
    index: dict[tuple[bool, str], list[str]],
) -> list[str]:
    return [
        local_id
        for local_id in sorted(pairs)
        if local_id in cards and not pair_is_synced(pairs, cards, index, local_id)
    ]


def compute() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--display", required=True, type=Path)
    parser.add_argument("--pairs", required=True, type=Path)
    parser.add_argument("--library", required=True, type=Path)
    args = parser.parse_args()

    display = load_json(args.display, "display")
    pairs_raw = load_json(args.pairs, "pairs")

    cards = display.get("cards")
    if not isinstance(cards, dict):
        raise fail(f"display has no object 'cards' map: {args.display}")

    pairs: dict[str, str] = {}
    for local_id, title in pairs_raw.items():
        if not isinstance(title, str):
            raise fail(f"pair {local_id!r} does not map to a title")
        pairs[local_id] = title

    titles, index = parse_library(args.library)

    moved_groups = {
        value.get("shortName")
        for value in display.get("groups", {}).values()
        if isinstance(value, dict) and value.get("shortName")
    }
    moved_groups.add("通用")
    moved_groups.update(local_id.split(":", 1)[0] for local_id in pairs if ":" in local_id)

    paired = 0
    title_match = 0
    text_match = 0
    synced = 0
    missing_local = 0

    for local_id, title in pairs.items():
        derived, norm = normalize_local_id(local_id)
        t_derived, t_norm = normalize_title(title)
        bodies = index.get((t_derived, t_norm))

        if local_id not in cards:
            missing_local += 1
            continue
        paired += 1

        title_ok = bool(bodies) and title_matches(
            local_id, norm, title, t_norm, t_derived, derived
        )
        card = cards[local_id]
        text = card.get("text", "") if isinstance(card, dict) else ""
        text_ok = bool(bodies) and any(
            normalize_text(body) == normalize_text(text) for body in bodies
        )
        title_match += int(title_ok)
        text_match += int(text_ok)
        synced += int(title_ok and text_ok)

    unpaired_local = sorted(
        local_id
        for local_id in cards
        if local_id.split(":", 1)[0] in moved_groups
        and local_id not in pairs
        and local_id not in KNOWN_LOCAL_ONLY
    )
    used_keys = {normalize_title(title) for title in pairs.values()}
    unpaired_library = sum(
        1 for title in titles if normalize_title(title) not in used_keys
    )

    has_tags = any(
        isinstance(card, dict) and card.get("tags") for card in cards.values()
    )
    tags_match = 0
    if has_tags:
        label = re.compile(r"^\s*\[([^\]]+)\][：:]?\s*$")
        for local_id, title in pairs.items():
            if local_id not in cards:
                continue
            card = cards[local_id]
            display_tags = set(card.get("tags") or []) if isinstance(card, dict) else set()
            derived, norm = normalize_title(title)
            bodies = index.get((derived, norm)) or []
            library_tags: set[str] = set()
            for body in bodies:
                for line in body.splitlines():
                    match = label.match(line)
                    if match:
                        library_tags.add(match.group(1))
            if display_tags and display_tags == library_tags:
                tags_match += 1

    print(f"METRIC face_paired={paired}")
    print(f"METRIC face_title_match={title_match}")
    print(f"METRIC face_text_match={text_match}")
    print(f"METRIC face_synced={synced}")
    print(f"METRIC unpaired_local={len(unpaired_local)}")
    print(f"METRIC unpaired_library={unpaired_library}")
    print(f"METRIC cards_present={len(cards)}")
    if has_tags:
        print(f"METRIC tags_match={tags_match}")
    print(f"METRIC missing_local={missing_local}")

    for local_id in find_mismatches(pairs, cards, index)[:MAX_ASI]:
        print(f"ASI synced=0 id={local_id}")
    return 0


def main() -> int:
    try:
        return compute()
    except MetricError as exc:
        print(f"face_metric: {exc}", file=sys.stderr)
        return 2
    except SystemExit:
        raise
    except Exception as exc:  # infra surprise: still a parse-class failure
        print(f"face_metric: unexpected error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
