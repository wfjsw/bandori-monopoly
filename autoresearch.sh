#!/usr/bin/env bash
# Autoresearch benchmark harness: fixture integrity, display export, face metric,
# and secondary gates. Metrics are data; only fixture/export/metric failures abort.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

sha256sum -c tools/autoresearch/cardlib.sha256 >/dev/null

OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT

echo "METRIC display_build_ok=0"
if node tools/export-display.mjs --out "$OUT/display.json"; then
  echo "METRIC display_build_ok=1"
else
  exit 1
fi

echo "METRIC roster_dump_ok=0"
if node tools/roster-dump.mjs --force; then
  echo "METRIC roster_dump_ok=1"
else
  exit 1
fi

python3 tools/autoresearch/face_metric.py \
  --display "$OUT/display.json" \
  --pairs tools/autoresearch/pairs.json \
  --event-pairs tools/autoresearch/event_pairs.json \
  --persona-pairs tools/autoresearch/persona_pairs.json \
  --group-pairs tools/autoresearch/group_pairs.json \
  --library tools/autoresearch/cardlib \
  --roster target/roster-dump/cards.json

if node --test webui/src/core/cards.test.ts; then
  echo "METRIC cards_test_ok=1"
else
  echo "METRIC cards_test_ok=0"
fi

if python3 tools/rulebook/check-card-text.py --display "$OUT/display.json"; then
  echo "METRIC sheet_check_ok=1"
else
  echo "METRIC sheet_check_ok=0"
fi

if env -u CARGO_TARGET_DIR cargo check -q -p rules-native; then
  echo "METRIC cargo_check_ok=1"
else
  echo "METRIC cargo_check_ok=0"
fi

exit 0
