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
# Built at run time rather than named as a string literal in source:
#   err.join.       net::describe
#   anim.chainKind. / anim.chainSrc.  Board.tsx ChainOrigin
#   log.counter.    trigger_why::counter_name
#   log.kind.       trigger_why::kind_name (`log.kind.{wire spelling}`, incl. `r:bandori:NN`)
#   tileQuote.      Ring.tsx `tr(\`tileQuote.${q.kind}\`)`; kinds are `TileQuoteKind`
#   board.state.    i18n/stateLabel.ts `tr(\`board.state.${key}\`)`
DYNAMIC = (
    "err.join.",
    "anim.chainKind.",
    "anim.chainSrc.",
    "log.counter.",
    "log.kind.",
    "tileQuote.",
    "board.state.",
    # skill.* scratch state names: `stateKeyOf` returns the wire key itself and
    # the markers list looks it up as a `game` key (`webui/src/i18n/stateKey.ts`).
    "skill.",
)
SPECIAL = ("blank", "list")  # message plumbing, not sentences
# Tile-mark kinds: `TileMark::kind` **is** the i18n key (`trigger_why::mark_kind_name`
# -> `Msg::new(kind)`), looked up at render time by fmtMsg, never referenced as a
# message key in source. A colon is part of the key (`mark:cp`), not a namespace.
# Without this list the "unused" sweep deletes every one of them and every mark
# on the board renders as a raw key. Card-declared kinds (`key!("anon_tokyo_mark")`)
# ride the `cards` namespace and are checked against the bundle below; rulebook
# display-name kinds (多惠兔子, 抹茶芭菲, ...) fall back to their literal.
RUNTIME_KEYS = (
    "ember",
    "mark:cp",
    "mark:designate",
    "noTarget",
    "skillBlock",
    "skill.uikaIdol.pot",
)
# Exact keys the unused sweep must keep. Two kinds:
#   * `game.json` -- older archived engine bundles still replay their log
#     through today's locales («Replays backward compatible»); deleting a key
#     an old bundle emits would render a raw key in the replay.
#   * `ui.json` -- retired purchase-UI copy (`deed.fundByMortgage` /
#     `deed.poor`) that no live `tr()` names; kept so a saved quote from
#     before the END-step buttons dropped still has a phrase.
RETAINED_KEYS = {
    # game.json -- replay / test-fixture phrases no current `Msg::new` emits
    "err.bad_act",
    "err.build_not_on_own",
    "err.no_money",
    "err.roll_stay",
    "log.look",
    "log.lookTile",
    "log.outcome.hand_limit",
    "log.outcome.state",
    "log.played",
    "log.why.exile",
    "log.x",
    "src.raise_funds",
    "status.exiled",
    "status.stunned",
    # ui.json -- retired purchase-UI copy
    "deed.fundByMortgage",
    "deed.poor",
}


def strip_doc_comments(text: str) -> str:
    """Drop rustdoc. `key!(…)` / message keys in `///`, `//!`, `#[doc = …]` are
    prose (card-sdk builder examples name `yolo_pay` / `when`), not emissions."""
    text = re.sub(r"^\s*///.*$", "", text, flags=re.M)
    text = re.sub(r"^\s*//!.*$", "", text, flags=re.M)
    text = re.sub(r"^\s*#\[doc\b.*$", "", text, flags=re.M)
    return text


def strip_probes(text: str) -> str:
    """Drop substring probes (`…contains("ask.build")`) -- they match a prompt
    title shape, not a message emission."""
    return re.sub(r'\.(?:contains|starts_with|ends_with)\(\s*"[^"]*"\s*\)', "", text)


# `ctx::slot` / `ctx::set_slot` / `ctx::inc_slot` take a player-state key.
# (The `state::{get,set,…}` wrappers are *not* in here on purpose: those
# `skill.*` scratch names double as markers-list display keys via
# `webui/src/i18n/stateKey.ts` and do need a `game.json` phrase.)
SLOT_API = re.compile(r"\b(?:ctx::)?(?:slot|set_slot|inc_slot)\b")


def slot_only_literals(text: str) -> set[str]:
    """String literals that name a `ctx::slot` key, not a message key.

    A `const NAME: &str = "…"` whose NAME is only ever a slot/set_slot/inc_slot
    argument (card latches like `card.tomoriInnerShout.pulled`) is state-dict
    spelling -- the checker must not demand a locale phrase for it."""
    out: set[str] = set()
    for m in re.finditer(r'const\s+(\w+)\s*:\s*&str\s*=\s*"([^"]+)"', text):
        name, lit = m.group(1), m.group(2)
        uses = list(re.finditer(rf"\b{name}\b", text))
        # Every mention beyond the declaration must sit in a slot API call.
        ok = len(uses) > 1
        for r in uses:
            if r.start() == m.start() + len("const "):
                continue  # the declaration
            lo, hi = max(0, r.start() - 80), min(len(text), r.end() + 80)
            if not SLOT_API.search(text[lo:hi]):
                ok = False
                break
        if ok:
            out.add(lit)
    # `"dotted.name"` passed straight in as the key argument.
    for m in SLOT_API.finditer(text):
        tail = text[m.end() : m.end() + 120]
        for lit in re.findall(r'"([^"]+)"', tail.split(")")[0]):
            out.add(lit)
    return out


def load(lang: str, ns: str) -> dict[str, str]:
    p = LOCALES / lang / f"{ns}.json"
    return json.load(open(p, encoding="utf-8")) if p.exists() else {}


def main() -> int:
    bad = 0
    game_all = set(load(LANGS[0], "game")) | set(load(LANGS[1], "game"))
    ui_all = set(load(LANGS[0], "ui")) | set(load(LANGS[1], "ui"))
    # Roots that live only in `ui.json` are client copy even when a `.rs` string
    # names them (`On::texts` / `deed.note.*` ride the manifest as i18n
    # keys). Everything else a `.rs` file emits is a protocol / engine key.
    ui_only_roots = {k.split(".")[0] for k in ui_all} - {k.split(".")[0] for k in game_all}
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
            if p.name.endswith((".test.ts", ".test.tsx")):
                continue  # unit tests name keys as fixtures, not as live references
            rel = p.relative_to(ROOT).as_posix()
            text = p.read_text(encoding="utf-8")
            if p.suffix == ".rs":
                text = re.sub(r"#\[cfg\(test\)\][\s\S]*", "", text)
                text = strip_doc_comments(text)
                text = strip_probes(text)
            keys = {k for k in key_re.findall(text) if not file_name.search(k)}
            if p.suffix == ".rs":
                keys -= slot_only_literals(text)
            keys.update(set(re.findall(r'"(blank|list)"', text)) & set(SPECIAL))
            engine_roots = {"err", "log", "ask", "src", "vote", "end", "status", "blank", "list"}
            from_protocol = rel.removeprefix("webui/src/") in PROTOCOL
            for k in keys:
                root = k.split(".")[0]
                # A key whose root exists only in ui.json is client copy even
                # when a .rs text declaration names it (tile deed texts).
                if root in ui_only_roots:
                    ui_used.add(k)
                    continue
                to_engine = p.suffix == ".rs" or from_protocol or root in engine_roots
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
        report(lang, "game.json is missing run-time mark kinds", (k for k in RUNTIME_KEYS if k not in game))
        report(lang, "game.json has keys no code emits", (k for k in game if k not in engine_used and k not in RUNTIME_KEYS and k not in RETAINED_KEYS and not any(k.startswith(d) for d in DYNAMIC)))
        report(lang, "ui.json is missing keys the code emits", (k for k in ui_used if k not in ui))
        report(lang, "ui.json has keys no code uses", (k for k in ui if k not in ui_used and k not in RETAINED_KEYS and not any(k.startswith(d) for d in DYNAMIC)))

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
            crate_text = strip_doc_comments(crate.read_text(encoding="utf-8"))
            for k in re.findall(r'key!\("([^"]+)"\)', crate_text):
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