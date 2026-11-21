#!/usr/bin/env bash
# Release to the live site: reset this worktree onto master, build for production,
# seal the built engine into the archive, cut over, and verify. Run it from a
# deploy worktree, never from the checkout a dev server is serving (the build
# writes webui/dist). The deploy is refused unless the live site serves the exact
# engine bundle this run sealed -- a record sealed by it must stay replayable.
#
#   node tools/build-production.mjs      the build steps on their own
#   tools/deploy-production.sh           this script: reset + build + deploy + check
#   tools/deploy-production.sh --prepare-only   reset onto master and stop
#   tools/deploy-production.sh --no-merge   redeploy the tree as it stands
#
# Everything a deploy touches is overridable; the defaults are the bmdev host:
#   LIVE_DIR    /home/monopoly/bm-live     backend: bin/, dist/cards, data/
#   WEBUI_DIR   /home/monopoly/bm-webui    static site root (nginx `root`)
#   SERVICE     monopoly.service           unit of the monopolyd user
#   SITE        bm.piggy.link              Host: header used for the health checks
#   BACKEND     127.0.0.1:8081             backend address for the health checks
#
# The branch this worktree sits on is local, disposable, and never pushed: a
# deploy makes it exactly master plus one archive commit. Nothing is merged into
# master here.
#
# The engine archive is append-only and is written by every deploy, so the reset
# parks the worktree's archive (index + reference records), hard-resets master,
# and unions the parked entries back (tools/merge-engine-archive.mjs). Entries
# are keyed by immutable bundle id, so no merge -- and no merge conflict -- is
# involved. An interrupted run keeps the park in target/scratch/archive-park for
# the rerun.

set -euo pipefail

LIVE_DIR="${LIVE_DIR:-/home/monopoly/bm-live}"
WEBUI_DIR="${WEBUI_DIR:-/home/monopoly/bm-webui}"
SERVICE="${SERVICE:-monopoly.service}"
SITE="${SITE:-bm.piggy.link}"
BACKEND="${BACKEND:-127.0.0.1:8081}"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DO_MERGE=1
PREPARE_ONLY=0
for arg in "$@"; do
  case "$arg" in
    --no-merge) DO_MERGE=0 ;;
    --prepare-only) PREPARE_ONLY=1 ;;
    -h|--help) sed -n '2,29p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
# The deploy branch is disposable and production is always a master commit, so
# this never merges. Park the archive, hard-reset onto master, union the parked
# entries back: no conflict is possible. Master's index is already in the tree
# after the reset and the parked entries are keyed by immutable bundle id.
park="$ROOT/target/scratch/archive-park"
if [ "$DO_MERGE" = 1 ]; then
  log "reset onto master"
  git rev-parse -q --verify master >/dev/null || die "no master branch to deploy from"

  # The reset discards everything not in master; nothing but the archive may
  # ride through it.
  stray="$(git status --porcelain -- . ':(exclude)archive/engine')"
  [ -z "$stray" ] || die "uncommitted changes outside archive/engine would be lost by the reset:
$stray"

  # A deploy interrupted by the old merge-based script can still leave a
  # conflicted merge behind; clear it before touching anything.
  if git rev-parse -q --verify MERGE_HEAD >/dev/null; then
    step "rewinding an interrupted merge"
    git merge --abort
  fi

  # A run that died after parking is the complete record: fold it into the
  # worktree first, then park again, so nothing of this worktree disappears.
  if [ -f "$park/index.json" ]; then
    step "recovering entries parked by an interrupted run"
    node tools/merge-engine-archive.mjs --into archive/engine/index.json \
      --from "$park/index.json" --refs-from "$park/refs"
  fi
  mkdir -p "$park/refs"
  cp archive/engine/index.json "$park/index.json"
  # Reference records, tracked or not: the reset drops whatever master lacks.
  if [ -d archive/engine/refs ]; then cp -a archive/engine/refs/. "$park/refs/"; fi
  step "parked $(node -e 'process.stdout.write(String(JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).bundles.length))' "$park/index.json") archive entries"

  git reset --hard master
  git log --oneline -1 | sed 's/^/   /'

  log "restore the parked archive entries"
  node tools/merge-engine-archive.mjs --into archive/engine/index.json \
    --from "$park/index.json" --refs-from "$park/refs"
  rm -rf "$park"
else
  log "reset onto master (skipped: --no-merge)"
  git log --oneline -1 | sed 's/^/   /'
fi

if [ "$PREPARE_ONLY" = 1 ]; then
  step "prepare only: stopping before the build"
  exit 0
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

# ---------------------------------------------------------------- 2b. archive
# The archive is part of the cutover, not a build side effect: seal the exact
# engine this run built and hold the deploy if its bundle is not in the store.
# (`build-production.mjs` already archives; running it here again is cheap and
# makes `--skip-server` / a hand-run build just as safe.)
log "archive the deployed engine"
[ -f webui/src/wasm/engine_id.json ] || die "no engine identity; build-glue did not run"
built_glue="$(node -e 'const j=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8"));process.stdout.write(j.glueSha256||"")' webui/src/wasm/engine_id.json)"
[ -n "$built_glue" ] || die "engine_id.json carries no glue sha"

node tools/archive-engine.mjs
node tools/archive-engine.mjs --check

# `archive-engine.mjs` makes the entry it just sealed `current`; key the rest
# off that, and prove it really is the engine this run built.
bundle="$(node -e '
  const j = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  process.stdout.write(j.current || "");
' archive/engine/index.json)"
[ -n "$bundle" ] || die "the archive has no current bundle after sealing"
entry_glue="$(node -e '
  const j = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  const e = (j.bundles || []).find((b) => b.id === process.argv[2]);
  process.stdout.write(e ? (e.glueSha256 || "") : "");
' archive/engine/index.json "$bundle")"
[ "$entry_glue" = "$built_glue" ] || die "archive current $bundle is not the built engine ${built_glue:0:12}…"
[ -d "data/engine-archive/$bundle" ] || die "archive entry $bundle has no stored bytes"
# archive-engine.mjs never overwrites an existing dist file; the index may have
# just gained aliases, so take the fresh copy explicitly.
cp archive/engine/index.json webui/dist/assets/engine/index.json
step "engine bundle ${bundle:0:12}… sealed"

# ---------------------------------------------------------------- 3. cut over
log "deploy to $LIVE_DIR and $WEBUI_DIR"

step "stop $SERVICE"
sudo monopoly-systemctl stop "$SERVICE"

step "backend binaries + ruleset + data"
cp target/release/server target/release/bot-service target/release/rules-worker "$LIVE_DIR/bin/"
# The pool's workers read the glue identity from `engine_id.json` beside their
# own binary. The live working directory has no source tree, so without this
# copy every online record seals an empty bundle: replays fall back to stamp
# matching and "download replay, include engine" has nothing to embed.
cp webui/src/wasm/engine_id.json "$LIVE_DIR/bin/engine_id.json"
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

# A worker that cannot name the deployed engine seals online records nobody can
# embed an engine for (and fairness verification refuses them). Spawn one the
# way the pool does and read its stamp.
step "live rules-worker names the deployed engine bundle"
worker_bundle="$(cd "$LIVE_DIR" && printf '{"id":1,"op":"info"}\n' | ./bin/rules-worker --data data --rules dist/cards 2>/dev/null | node -e '
  let s = "";
  process.stdin.on("data", (d) => (s += d)).on("end", () => {
    try { process.stdout.write(JSON.parse(s).stamp.bundle || ""); } catch { process.stdout.write(""); }
  });
')"
printf '   %-32s %s\n' "worker stamp bundle" "${worker_bundle:0:12}…"
[ "$worker_bundle" = "$bundle" ] || die "the live worker seals bundle '${worker_bundle}', not $bundle"

fail=0
for path in / /menu /api/health /data/index.json /assets/engine/index.json; do
  code=$(curl -s -o /dev/null -w '%{http_code}' -H "Host: $SITE" "http://127.0.0.1$path")
  printf '   %-24s %s\n' "$path" "$code"
  [ "$code" = 200 ] || fail=1
done
[ "$fail" = 0 ] || die "an endpoint did not answer 200 through $SITE"

# The deployment must be able to replay the records its own engine sealed.
step "engine bundle ${bundle:0:12}… is live"
[ -d "$LIVE_DIR/data/engine-archive/$bundle" ] || die "live store lacks $bundle"
live_current="$(node -e '
  try {
    const j = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    process.stdout.write(j.current || "");
  } catch { process.stdout.write(""); }
' "$WEBUI_DIR/dist/assets/engine/index.json")"
[ "$live_current" = "$bundle" ] || die "live engine index current is ${live_current:0:12}…, not $bundle"
for f in bundle.json glue.js glue_bg.wasm; do
  code=$(curl -s -o /dev/null -w '%{http_code}' -H "Host: $SITE" "http://127.0.0.1/assets/engine/$bundle/$f")
  printf '   %-32s %s\n' "/assets/engine/${bundle:0:8}…/$f" "$code"
  [ "$code" = 200 ] || fail=1
done
[ "$fail" = 0 ] || die "the live deployment cannot serve engine bundle $bundle"

step "direct backend (should answer the API only)"
code=$(curl -s -o /dev/null -w '%{http_code}' "http://$BACKEND/api/health")
printf '   %-24s %s\n' /api/health "$code"
[ "$code" = 200 ] || die "the backend is not answering on $BACKEND"

if command -v valkey-cli >/dev/null 2>&1 && valkey-cli ping >/dev/null 2>&1; then
  step "valkey: $(valkey-cli dbsize) key(s)"
fi

# ---------------------------------------------------------------- 4b. record
# The entry sealed above is what makes this deploy replayable later. Commit it
# (and any entry the merge left uncommitted) so the catalog is not left
# floating in this worktree for the next merge to trip over.
log "record the sealed archive entry"
if [ -n "$(git status --porcelain -- archive/engine)" ]; then
  source_id="$(node -e '
    const j = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    const e = (j.bundles || []).find((b) => b.id === process.argv[2]);
    process.stdout.write(e ? (e.source_id || "") : "");
  ' archive/engine/index.json "$bundle")"
  git add -A -- archive/engine
  git commit -q -m "Archive: seal ${source_id:0:8} (deployed $(git rev-parse --short HEAD))" -- archive/engine
  step "$(git log --oneline -1)"
else
  step "archive unchanged"
fi

# ---------------------------------------------------------------- 5. report
log "done"
printf '   site          http://%s/\n' "$SITE"
printf '   service       %s\n' "$SERVICE"
printf '   backend       %s\n' "$BACKEND"
printf '   commit        %s\n' "$(git log --oneline -1)"

archive_dirty="$(git status --porcelain -- archive)"
if [ -n "$archive_dirty" ]; then
  printf '\n   note: the engine archive still has uncommitted changes:\n'
  printf '%s\n' "$archive_dirty" | sed 's/^/     /'
fi