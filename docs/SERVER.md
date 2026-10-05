# Server & protocol (P5)

`crates/server` — axum + tokio. Commands are HTTP; updates are Server-Sent Events.

```sh
cargo run -p server --release -- --port 8080 --data data --static webui/dist
# env: PORT, BM_DATA, BM_STATIC
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
`/play/solo`; an online seat is re-attached with the tab's session token
(`sessionStorage`) and `GET /api/rooms/{id}/state`, within the presence timeout.

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
| POST | `/api/rooms/{id}/bots` | `{op: "add" \| "remove", member?}` | `RoomInfo` (host) |
| POST | `/api/rooms/{id}/weights` | `{money, property, houses}` | `RoomInfo` (host) |
| POST | `/api/rooms/{id}/start` | `{force}` | `RoomInfo` (host) |
| POST | `/api/rooms/{id}/leave` | | `{ok}` |
| GET  | `/api/rooms/{id}/state` | | `{room, you, match}` |
| POST | `/api/rooms/{id}/act` | `NetMessage` | `{ok}` |
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
| `match` | | `{state, hand, handNotes, you, seat}`, whenever the state changes |
| `dissolve` | | `{reason}` — the room is gone |

* `hand` / `handNotes` are only ever your own; `state.seats[*].hand` is just a count.
* Only `event` frames carry ids. On reconnect the browser sends the last one as
  `Last-Event-ID` and the stream resumes right after it. A new match (new `matchId`)
  starts from its first event.
* `state.prompt` describes the prompt in progress (`id`, `kind`, `seats`, `answers`
  with `-1` = still waiting, `timeLeft`, auction `bid` / `bidder`).
* Keep-alive comments every 15 s.

## Presence

An open stream counts as connected. A player without a stream for 20 s:

* in a match: the AI takes the seat (`members[].away = true`); reopening the stream
  hands it back;
* in the lobby: removed from the room.

## Differences from the original

* No host machine: hosting passes to the next human when the host leaves; the room
  closes when no humans remain.
* No LAN/Steam discovery or direct TCP; the room list comes from `/api/rooms`.
* Protocol version is `9` (the original TCP/Steam protocol was `8`).
