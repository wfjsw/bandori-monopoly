# Server & protocol (P5)

`crates/server` — axum + tokio. Commands are HTTP; updates are Server-Sent Events.

```sh
cargo run -p server --release -- --port 8080 --data data --static webui/dist
# env: PORT, BM_DATA, BM_STATIC
# optional advanced-bot decisions (docs/BOT.md B5) -- see "Enabling it in a deploy":
#   --bot-service <path> / BM_BOT_SERVICE, --bot-threads N / BOT_THREADS,
#   --bot-search-threads N / BOT_SEARCH_THREADS
```

Serves `/api/*`, the game data at `/data/*` (e.g. `/data/board.json`), and the web
client from the static directory if it exists. The client routes with the History
API (`/menu`, `/lobby`, `/room/<id>`, `/play/solo`, `/play/<roomId>`), so any page
path that is not a file is answered with `index.html`; missing files still 404.

## Web client

`webui/` — rsbuild + React + TypeScript, one CSS module per component
(`src/styles/base.css` holds only the theme variables and reset). The UI is laid
out on a fixed 1600 × 900 stage scaled to the window, like the original.

```sh
node tools/build-glue.mjs          # wasm rules -> webui/src/wasm/
cd webui && npm run dev           # :5173, proxies /api and /data to :8080
cd webui && npm run build         # dist/, served by --static (prebuild: tools/live2d/build.mjs)
```

`npm run build` compiles the Live2D models first (`prebuild` ->
`tools/live2d/build.mjs`, see [LIVE2D.md](LIVE2D.md)); a model whose assets are
already up to date is skipped.

Localization is documented in [I18N.md](I18N.md): the server sends message
keys (`Msg`), the client renders them per player.

Refresh recovery: a solo match is snapshotted to `localStorage` (`bm.solo`, the
engine's `Match::save`) about once a second and on page hide, and resumed by
`/play/solo`; an online player is re-attached with the tab's session token
(`sessionStorage`) and `GET /api/rooms/{id}/state`, within the presence timeout.

### 托管 / 混沌 (auto-play)

Inside a match -- solo or online -- the TopBar carries an auto mode selector
(`webui/src/ui/AutoToggle.tsx`): **off** (手动) → **托管** (the bot policy) →
**混沌** (the chaos policy) → off. Either auto mode hands the seat to a
browser-side autopilot (`webui/src/game/autopilot.ts`) and locks every input
button: the board's roll / end / skill / build / mortgage / redeem, the hand's
play and discard, the popups' buy / build / mortgage / redeem / skill / settle /
leave, prompt answers, tile-pick clicks, and the ban / pick / deck controls.
Inspecting a deed, the log, the draw pile and the skill text stays available,
and the prompt modal is not opened at all so the board stays watchable. The
selector itself remains clickable to change mode or take the controls back.

The takeover runs **solely in the browser**: the client computes every choice
from what it can see (the match view, its own hand, its prompts) and sends it
through the ordinary `POST /api/rooms/{id}/act` / solo `act` path, exactly as a
button press would. The server's engine is never asked to drive the seat -- no
new act flips `players[i].ai`, and `member_left` is not reused -- so the server
is unchanged and the player's seat is still theirs.

Both policies share one driver; only the decision policy is pluggable
(`policies: { bot, chaos }`).

**托管 (`bot`)** is a port of the bot AI in `crates/game-core/src/engine/ai.rs`
over public data (thresholds kept in sync next to the TS copy): redeem the most
valuable mortgaged deed that still leaves 4 000; play a card with 70% odds while
fewer than two have been played this turn; otherwise roll; after landing, buy
while it leaves 2 000 and build while it leaves 3 500; discard at random over
the hand limit. Prompts prefer the engine's own precomputed AI answer, which the
match frame carries per viewer in `aiAnswer` (`Match::view_extra`) -- only for
the player being asked, never another seat, because an auction ceiling is
hidden information -- and fall back to the prompt's `fallback` when the frame
does not carry one (counteract therefore declines). Ban / pick choose a random
free character; the deck phase submits the character's preset; a live vote is
answered yes so the table is never left hanging.

**混沌 (`chaos`)** is legal but maximally disruptive -- it prefers whatever
makes more things happen. Play every legal card (100%, no cap) and fire every
usable character / band skill; prompts pick uniformly among the non-default
options, a random target for tile prompts (never "none" while a target exists),
a random valid subset for mortgage / pick; buy, build and accept forced
purchases whenever they keep the reserve; redeem what it can afford; bid at
auctions with random raises; random votes; a random free character and a random
legal deck. An offered [反击] is the one gate left: it declares on only
`CHAOS_COUNTER_CHANCE = 0.3` of the offers it gets (a random offered card) and
passes the rest. Money still gets a floor so the seat does not die instantly:
every voluntary spend must leave `CHAOS_RESERVE = 1 000` (buy, build, redeem,
auction bids, optional paid prompt choices). Card plays are unrestricted -- the
view carries no visible card cost to check against the reserve. End turn only
when nothing else is legal.

Pacing matches the engine's bot clock in both modes: one command at a time,
after a 0.4–1.0 s human beat, never while one is in flight. The policy returns
an ordered candidate list; a refused command is logged, marked tried for the
current state, and the next candidate is attempted -- so a speculative try that
bounces moves on instead of looping. Online it always moves well inside the
turn timer.

Limitations: auto-play is client-only, so other players do not see the
`board.afk` tag on the seat. Card play uses the view's `playable` list
(`Match::view_extra`), not a live `rules.ai_play` call -- the two agree today
because `CardRules::ai_play` defaults to `true`, but a card rule that overrides
it would not be consulted by the takeover. Chaos's "no visible card cost" note
above is the same gap.

### Bot mentality (engine bots)

The auto-play takeover above is a *human* seat driven by the browser. A **bot**
member is driven by the engine, and it carries a `BotMentality` on the seat
(`standard` / `chaos` / `advanced`, see
[ENGINE.md](ENGINE.md#bot-ai-mentalities) and [BOT.md](BOT.md)). `POST
/api/rooms/{id}/bots` takes `mentality` on `op: "add"` (default `standard`);
it is stored on the room member, shown as a tag in the lobby, and copied onto
the seat when the match starts. Solo's setup screen offers `standard` /
`chaos` (one for all bots, with a per-bot override); the browser has no
search yet (BOT.md B6).

The first two are meant to play alike: a chaos bot and a chaos-托管 human
follow the same policy over the same options, differing only in what they can
see.

**`advanced`** is the server-side search bot (BOT.md B5, `crates/bot-service`).
It is an **online** option only.

### Enabling it in a deploy

The server **spawns `bot-service` as a child** — one process, its own CPU
budget, the match workers and the 20 Hz tick untouched. Build both bins, then
point the server at the service binary:

```sh
# one-time (release):
cargo build -p bot-service --release
cargo build -p server --release

# run (the server starts the child itself):
./target/release/server --port 18080 --data data --static webui/dist \
  --bot-service ./target/release/bot-service \
  --bot-threads 4 --bot-search-threads 4 \
  --rules dist/cards
```

| flag | env | default | meaning |
|---|---|---|---|
| `--bot-service <path>` | `BM_BOT_SERVICE` | *(none)* | the `bot-service` binary to spawn; absent = advanced bots play as standard (logged once at startup) |
| `--bot-threads N` | `BOT_THREADS` | 4 | request workers **inside** the child (`bot-service --threads`) |
| `--bot-search-threads N` | `BOT_SEARCH_THREADS` | = `--bot-threads` | root-parallel searches **per decision** (`bot-service --search-threads`); 1 is the conservative choice on a shared box |
| `--data` | `BM_DATA` | `data` | forwarded to the child (`bot-service --data`) |
| `--rules` | `BM_RULES` | `dist/cards` | forwarded to the child (`bot-service --rules`) |

`bot-service`'s own flags (for running it standalone, or for A/B) are
documented in `docs/BOT.md` §3.5: `--threads` / `--search-threads` /
`--data` / `--rules` / `--bias-weight` / `--eval-weight` / `--horizon` /
`--legacy` / `--no-reuse` / `--no-ponder`. The server only forwards
`--data` / `--rules` / `--threads` / `--search-threads`; anything else is a
standalone concern.

**Budgets.** Per decision the server sends `budget_ms = min(1000 ms, prompt
time left − 200 ms)`, or 800 ms on the turn surface (no prompt clock). The
outer deadline is `budget + 1.5 s` capped at 3 s
(`server::botsvc::ask_timeout`) — sized so a real-ruleset search iteration
(0.4–1.3 s at 1–4 search threads) that finishes *after* the budget still
lands inside the deadline. Do not shrink the margin below one iteration
overrun: that is exactly what makes the server fall back to the heuristic
routinely (`docs/BOT.md` §5 B7).

**Ponder.** While an advanced seat is idle (another seat's turn or prompt)
the server sends that seat's view as `bot-service`'s `op: "ponder"` —
non-blocking, one in flight per seat, rate-limited to the 200 ms idle probe
cadence, cancelled when the seat's own decision arrives. The service caches
the result by information-set key and a later `decide` for the same key
answers from that cache. `bot-service --no-ponder` turns the whole path off.

* When the service is attached, the server holds the seat (the engine does not
  auto-play it) and asks for one seat's view when that seat must act — an open
  prompt or its turn decision. The answer is applied through the ordinary
  `POST …/act` path.
* On timeout, crash or a bad reply the engine's own heuristic (`aiAnswer`)
  answers that decision and the match keeps moving — a match never stalls on
  the service. The engine's turn clock is the last-resort takeover. A wedged
  child costs at most one probe per seat and never the tick.
* Setup (ban / pick / deck) stays engine-side and runs the standard policy.
* The service sees exactly one seat's view — the same frame the client gets.
  It only proposes answers; the sandboxed engine stays authoritative.

## Auth

`POST /api/session {player}` returns `{token}` and sets an `HttpOnly` cookie
`bm_session`. Send the token as `Authorization: Bearer <token>`; the browser's
`EventSource` (which can't set headers) uses the cookie automatically. `?token=` also
works, for tools.

## Endpoints

| Method | Path | Body | Response |
|---|---|---|---|
| GET  | `/api/health` | | `{game, version, rooms}` |
| POST | `/api/session` | `{player, character?, cnId?}` | `{token, player}` |
| GET  | `/api/session` | | `{token, player, room, member}` |
| GET  | `/api/rooms` | | `RoomInfo[]` |
| POST | `/api/rooms` | `{name, ranked, maxPlayers, password, weights?}` | `{room, you}` |
| POST | `/api/rooms/{id}/join` | `{password, version?}` | `{room, you}` |
| POST | `/api/rooms/{id}/ready` | `{on}` | `RoomInfo` |
| POST | `/api/rooms/{id}/bots` | `{op: "add" \| "remove", member?, mentality?}` | `RoomInfo` (host) |
| POST | `/api/rooms/{id}/weights` | `{money, property, houses}` | `RoomInfo` (host) |
| POST | `/api/rooms/{id}/start` | `{force}` | `RoomInfo` (host) |
| POST | `/api/rooms/{id}/leave` | | `{ok}` |
| GET  | `/api/rooms/{id}/state` | | `{room, you, match}` |
| POST | `/api/rooms/{id}/act` | `NetMessage` | `{ok}` |
| GET  | `/api/rooms/{id}/record` | | the last finished match's `.bdrec` (see below) |
| GET  | `/api/rooms/{id}/stream` | | SSE |

Errors: `{error, reason}`. `error` is the player-facing message from the original
game; `reason` is set for join rejections: `password` (403), `full`, `playing`,
`version` (409).

### Match commands (`/act`)

The body is the original `NetMessage` shape; only the fields the command uses matter.

| `act` | fields | when |
|---|---|---|
| `ban` | `character` (empty = no ban) | Ranked ban phase, your turn |
| `pick` | `character` | pick phase, your turn |
| `deck` | `cards` (10 ids) | deck phase |
| `roll` | | your turn, before moving |
| `buy` / `build` | `value` = tile (optional check) | after moving onto that tile |
| `mortgage` / `redeem` | `value` = tile | your turn (rules as in the original) |
| `play` | `card` | your turn, before moving |
| `discard` | `card` | hand over the limit |
| `end` | | your turn, after moving |
| `answer` | `prompt` = prompt id; `value`, or `cards` for `mortgage` / `pick` prompts; auction: `value` = bid, `-1` = pass | a prompt is waiting for you |
| `vote` | `value` 1 = yes, 0 = no | start or answer the end-match vote |
| `leave` | | forfeit |

## SSE stream

| `event:` | `id:` | `data:` |
|---|---|---|
| `hello` | | `{you, room}` |
| `room` | | `RoomInfo`, whenever it changes |
| `event` | `<matchId>:<eventId>` | `MatchEvent` (one per log line / animation step) |
| `match` | | `{state, hand, handNotes, you, player_id}`, whenever the state changes |
| `dissolve` | | `{reason}` — the room is gone |

* `hand` / `handNotes` are only ever your own; `state.players[*].hand` is just a count.
* Only `event` frames carry ids. On reconnect the browser sends the last one as
  `Last-Event-ID` and the stream resumes right after it. A new match (new `matchId`)
  starts from its first event.
* `state.prompt` describes the prompt in progress (`id`, `kind`, `players`, `answers`
  with `-1` = still waiting, `timeLeft`, auction `bid` / `bidder`).
* Keep-alive comments every 15 s.

## Match records

The server records every online match as an input log beside the match blob
(`docs/REPLAY.md` §4): `Init::Seed` (members, seed, weights), every ordered
public call (`act` with its Ok/Err bit, tick runs, `quick_start`, `finish`,
`member_left`, `member_back`), and a state-hash checkpoint at each turn
boundary. When the match ends the log is sealed into a `RecordFile` -- seats
from the final view, `final_hash` an FNV-1a-64 of the final `Match::save()` --
and stored **zstd-compressed** as `record:{room}:last`; the log is then
cleared. Seal-time compression is enough: the in-progress log stays one JSON
line per entry.

**`GET /api/rooms/{id}/record`** serves that `.bdrec`:

| Case | Answer |
|---|---|
| the room is still playing | `409` `err.record.live` |
| nothing sealed yet | `404` `err.record.none` |
| the caller was not in the match | `403` `err.record.forbidden` |
| otherwise | `200`, the record as `application/zstd` |

Participation is resolved through the room's member → session-token map: the
caller's token names a member id, and that id must be one of the seats the
record was sealed for. A participant who has since left the room (and would
rejoin under a fresh member id) is therefore outside the set -- deliberately
narrow, since the full record reveals every hand and the deck order.

* **The body is the sealed bytes as stored**: a zstd-framed `.bdrec`
  (`Content-Type: application/zstd`), with
  `Content-Disposition: attachment; filename="bdrec-<room>-<yyyymmdd-hhmm>.bdrec"`.
  Deliberately **no** `Content-Encoding: zstd` -- a browser would transparently
  decode that, and inconsistently; the client decompresses in wasm
  (`ReplayMatch::from_record_bytes`), which sniffs the magic and also accepts
  gzip and plain JSON. A record stored before compression is plain JSON and is
  served as-is as `application/json`.
* **Storage.** `StoredRecord` holds the bytes. Redis writes them with a
  binary-safe `SET` under `bm:rec:<room>` (the member list rides a companion
  `bm:rec:<room>:members` key, same TTL) and reads them back with `GET`; the
  in-memory store keeps the bytes in its 64-entry LRU. A value written before
  compression (one JSON envelope) is still read and served.
* **TTL 24 h.** `record:{room}:last` is written with a 24 h expiry (Redis
  `SET EX`; the in-memory store bounds itself to a 64-entry LRU instead of a
  clock). The in-progress log gets the same expiry so a box that dies mid-match
  does not leak a list.
* **`gaps`.** A restart mid-match loses the buffered tick run (up to 100
  ticks). The sealed record then has `header.gaps = true` and is not
  checkpoint-verifiable. Everything else about it still plays.
* **Tick quantization.** The match clock is a fixed-step accumulator:
  `k = floor(acc / 0.05).min(10)` whole quanta per ticker wake-up, each worth
  `0.05 * time_scale` of game time. The record seals that quantum into
  `header.step`, so a replay feeds the engine the identical `tick(dt)` f32s.

## Presence

An open stream counts as connected. A player without a stream for 20 s:

* in a match: the AI takes the player (`members[].away = true`); reopening the stream
  hands it back;
* in the lobby: removed from the room.

## Differences from the original

* No host machine: hosting passes to the next human when the host leaves; the room
  closes when no humans remain.
* No LAN/Steam discovery or direct TCP; the room list comes from `/api/rooms`.
* Protocol version is `9` (the original TCP/Steam protocol was `8`).
