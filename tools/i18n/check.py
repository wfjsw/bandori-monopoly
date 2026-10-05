#!/usr/bin/env python3
"""Check that the Rust message keys and the client locales agree.

The engine and server never hold display text: every log line, prompt and error
is a message key (crates/game-core/src/msg.rs: `Msg::new("err.poor")`, or a bare
`"err.poor"` where `impl Into<Msg>` converts it), rendered by the web client in
each player's language from `webui/src/i18n/locales/<lang>/game.json`. Card
modules name their own namespace (`key!("name")` -> `cards:<crate>.<name>`),
bundled from `rules/*/locales/*.json` into the client's `cards` namespace.

Two namespaces:
  game -- protocol messages: keys from the Rust code, plus the few the web
          client's transport emits itself (net/api.ts, game/session.ts, i18n/msg.ts).
  ui   -- interface copy of the web client.

This script fails when code references a key no locale carries (it would render
as a raw key) or a locale carries a key nothing references (stale translation).
Run-time key families are matched by prefix (`err.join.*`).

Usage: python tools/i18n/check.py
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LOCALES = ROOT / "webui" / "src" / "i18n" / "locales"
LANGS = ("zh-CN", "en")
# webui files that emit protocol messages (their keys belong to the `game` namespace)
PROTOCOL = {"net/api.ts", "game/session.ts", "i18n/msg.ts"}
DYNAMIC = ("err.join.",)  # built at run time: net::describe
SPECIAL = ("blank", "list")  # message plumbing, not sentences


def load(lang: str, ns: str) -> dict[str, str]:
    p = LOCALES / lang / f"{ns}.json"
    return json.load(open(p, encoding="utf-8")) if p.exists() else {}


def main() -> int:
    bad = 0
    game_all = set(load(LANGS[0], "game")) | set(load(LANGS[1], "game"))
    ui_all = set(load(LANGS[0], "ui")) | set(load(LANGS[1], "ui"))
    # The key shape is taken from the locales themselves, so a new namespace in a
    # locale file is scanned automatically.
    roots = {k.split(".")[0] for k in game_all | ui_all} | set(SPECIAL) | {"err", "log", "ask", "src", "vote", "end", "status", "room"}
    alt = "|".join(sorted((re.escape(r) for r in roots), key=len, reverse=True))
    key_re = re.compile(r'"((?:%s)(?:\.[A-Za-z0-9_]+)+)"' % alt)
    file_name = re.compile(r"\.(json|txt|webp|png|wasm|js|ts|css|md|sh)$")

    engine_used: set[str] = set()
    ui_used: set[str] = set()
    for pat in ("crates/**/*.rs", "rules/**/src/*.rs", "webui/src/**/*.ts", "webui/src/**/*.tsx"):
        for p in ROOT.glob(pat):
            if "target" in p.parts or "locales" in p.parts or "wasm" in p.parts or "tests" in p.parts:
                continue
            rel = p.relative_to(ROOT).as_posix()
            text = p.read_text(encoding="utf-8")
            if p.suffix == ".rs":
                text = re.sub(r"#\[cfg\(test\)\][\s\S]*", "", text)
            keys = {k for k in key_re.findall(text) if not file_name.search(k)}
            keys.update(set(re.findall(r'"(blank|list)"', text)) & set(SPECIAL))
            engine_roots = {"err", "log", "ask", "src", "vote", "end", "status", "blank", "list"}
            from_protocol = rel.removeprefix("webui/src/") in PROTOCOL
            for k in keys:
                to_engine = p.suffix == ".rs" or from_protocol or k.split(".")[0] in engine_roots
                (engine_used if to_engine else ui_used).add(k)

    def report(lang: str, title: str, keys) -> None:
        nonlocal bad
        keys = sorted(keys)
        if keys:
            bad += 1
            print(f"[{lang}] {title} ({len(keys)}):")
            for k in keys:
                print(f"    {k}")

    for lang in LANGS:
        game, ui = load(lang, "game"), load(lang, "ui")
        report(lang, "game.json is missing keys the code emits", (k for k in engine_used if k not in game))
        report(lang, "game.json has keys no code emits", (k for k in game if k not in engine_used and not any(k.startswith(d) for d in DYNAMIC)))
        report(lang, "ui.json is missing keys the code emits", (k for k in ui_used if k not in ui))
        report(lang, "ui.json has keys no code uses", (k for k in ui if k not in ui_used))

    # card modules: key!(...) must exist in that crate's locale files
    for crate in sorted(ROOT.glob("rules/*/src/lib.rs")):
        manifest = crate.parent.parent / "Cargo.toml"
        name = re.search(r'name = "([^"]+)"', manifest.read_text(encoding="utf-8"))
        if not name:
            continue
        for lang in LANGS:
            bundle_path = ROOT / "dist" / "cards" / "locales" / f"{lang}.json"
            bundle = json.load(open(bundle_path, encoding="utf-8")) if bundle_path.exists() else {}
            have = bundle.get(name.group(1), {})
            for k in re.findall(r'key!\("([^"]+)"\)', crate.read_text(encoding="utf-8")):
                if k not in have:
                    bad += 1
                    print(f"[{lang}] card {name.group(1)}: key {k} not in its locales (run tools/build-ruleset.sh)")

    if bad:
        print(f"\n{bad} problem(s).")
        return 1
    print(f"i18n ok: {len(engine_used)} engine keys, {len(ui_used)} client keys, {len(LANGS)} locales in sync.")
    return 0


if __name__ == "__main__":
    sys.exit(main())