#!/usr/bin/env bash
# Release to the live site: pull master into this worktree, build for production,
# cut over, and verify. Run it from a deploy worktree, never from the checkout a
# dev server is serving (the build writes webui/dist).
#
#   node tools/build-production.mjs      the build steps on their own
#   tools/deploy-production.sh           this script: merge + build + deploy + check
#   tools/deploy-production.sh --no-merge   redeploy the tree as it stands
#
# Everything a deploy touches is overridable; the defaults are the bmdev host:
#   LIVE_DIR    /home/monopoly/bm-live     backend: bin/, dist/cards, data/
#   WEBUI_DIR   /home/monopoly/bm-webui    static site root (nginx `root`)
#   SERVICE     monopoly.service           unit of the monopolyd user
#   SITE        bm.piggy.link              Host: header used for the health checks
#   BACKEND     127.0.0.1:8081             backend address for the health checks
#
# The branch this worktree sits on is local and is never pushed. Master is only
# read (merged in); nothing is merged into master here.

set -euo pipefail

LIVE_DIR="${LIVE_DIR:-/home/monopoly/bm-live}"
WEBUI_DIR="${WEBUI_DIR:-/home/monopoly/bm-webui}"
SERVICE="${SERVICE:-monopoly.service}"
SITE="${SITE:-bm.piggy.link}"
BACKEND="${BACKEND:-127.0.0.1:8081}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DO_MERGE=1
for arg in "$@"; do
  case "$arg" in
    --no-merge) DO_MERGE=0 ;;
    -h|--help) sed -n '2,18p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "deploy-production: unknown argument: $arg" >&2; exit 2 ;;
  esac
done

log()  { printf '\n== %s\n' "$*"; }
step() { printf '   %s\n' "$*"; }
die()  { printf '\ndeploy-production: FAILED: %s\n' "$*" >&2; exit 1; }

# refuse to build into the primary checkout -- it is what the dev server serves
common="$(git rev-parse --git-common-dir)"
here="$(git rev-parse --git-dir)"
if [ "$(cd "$common" && pwd)" = "$(cd "$here" && pwd)" ]; then
  die "this is the primary checkout; run from a deploy worktree instead"
fi

# ---------------------------------------------------------------- 1. master
if [ "$DO_MERGE" = 1 ]; then
  log "merge master"
  if ! git merge master --no-edit; then
    die "merge of master failed -- resolve it in $ROOT and rerun"
  fi
  git log --oneline -1 | sed 's/^/   /'
else
  log "merge master (skipped: --no-merge)"
  git log --oneline -1 | sed 's/^/   /'
fi

# ---------------------------------------------------------------- 2. build
log "build for production"
node tools/build-production.mjs

[ -f webui/dist/index.html ] || die "build did not produce webui/dist/index.html"
[ -f dist/cards/index.json ] || die "build did not produce dist/cards/index.json"
ls dist/cards/*.wasm >/dev/null 2>&1 || die "no card wasm under dist/cards"
for b in target/release/server target/release/bot-service target/release/rules-worker; do
  [ -x "$b" ] || die "build did not produce $b"
done
step "bundles in dist: $(ls webui/dist/assets/engine | wc -l)"

# ---------------------------------------------------------------- 3. cut over
log "deploy to $LIVE_DIR and $WEBUI_DIR"

step "stop $SERVICE"
sudo monopoly-systemctl stop "$SERVICE"

step "backend binaries + ruleset + data"
cp target/release/server target/release/bot-service target/release/rules-worker "$LIVE_DIR/bin/"
rm -rf "$LIVE_DIR/dist/cards"
cp -a dist/cards "$LIVE_DIR/dist/"
# append-only on purpose: engine-archive/ keeps every bundle a record may need
cp -a data/. "$LIVE_DIR/data/"

step "webui (static root for nginx)"
rsync -a --delete webui/dist/ "$WEBUI_DIR/dist/"
setfacl -R -m u:www-data:rX "$WEBUI_DIR/dist"
setfacl -R -d -m u:www-data:rX "$WEBUI_DIR/dist"
chmod -R g+rwX "$WEBUI_DIR" "$LIVE_DIR"

step "start $SERVICE"
sudo monopoly-systemctl start "$SERVICE"

# ---------------------------------------------------------------- 4. verify
log "verify"
for _ in $(seq 1 20); do
  active="$(sudo monopoly-systemctl is-active "$SERVICE" || true)"
  [ "$active" = active ] && break
  sleep 1
done
[ "$active" = active ] || die "$SERVICE did not come up (state: ${active:-unknown}); see: sudo monopoly-systemctl logs $SERVICE"
step "$SERVICE is active"

# the server binds only after it has loaded the ruleset and started its
# rules-worker pool, so wait for the endpoint rather than the process
up=""
for _ in $(seq 1 60); do
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://$BACKEND/api/health" || echo 000)
  if [ "$code" = 200 ]; then up=1; break; fi
  sleep 1
done
[ -n "$up" ] || die "the backend on $BACKEND never answered 200; see: sudo monopoly-systemctl logs $SERVICE"
step "backend is answering on $BACKEND"

fail=0
for path in / /menu /api/health /data/index.json /assets/engine/index.json; do
  code=$(curl -s -o /dev/null -w '%{http_code}' -H "Host: $SITE" "http://127.0.0.1$path")
  printf '   %-24s %s\n' "$path" "$code"
  [ "$code" = 200 ] || fail=1
done
[ "$fail" = 0 ] || die "an endpoint did not answer 200 through $SITE"

step "direct backend (should answer the API only)"
code=$(curl -s -o /dev/null -w '%{http_code}' "http://$BACKEND/api/health")
printf '   %-24s %s\n' /api/health "$code"
[ "$code" = 200 ] || die "the backend is not answering on $BACKEND"

if command -v valkey-cli >/dev/null 2>&1 && valkey-cli ping >/dev/null 2>&1; then
  step "valkey: $(valkey-cli dbsize) key(s)"
fi

# ---------------------------------------------------------------- 5. report
log "done"
printf '   site          http://%s/\n' "$SITE"
printf '   service       %s\n' "$SERVICE"
printf '   backend       %s\n' "$BACKEND"
printf '   commit        %s\n' "$(git log --oneline -1)"

archive_dirty="$(git status --porcelain -- archive)"
if [ -n "$archive_dirty" ]; then
  printf '\n   note: the engine archive changed under this worktree and is not committed:\n'
  printf '%s\n' "$archive_dirty" | sed 's/^/     /'
  printf '   These record the bundles sealed here. Master does not list them yet.\n'
fi