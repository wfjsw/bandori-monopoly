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
usable character / band skill; always declare an offered [反击], picking a
random card rather than the skip option; prompts pick uniformly among the
non-default options, a random target for tile prompts (never "none" while a
target exists), a random valid subset for mortgage / pick; buy, build and
accept forced purchases whenever they keep the reserve; redeem what it can
afford; bid at auctions with random raises; random votes; a random free
character and a random legal deck. Money still gets a floor so the seat does
not die instantly: every voluntary spend must leave `CHAOS_RESERVE = 1 000`
(buy, build, redeem, auction bids, optional paid prompt choices). Card plays
and counteracts are unrestricted -- the view carries no visible card cost to
check against the reserve. End turn only when nothing else is legal.

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
member is driven by the engine, and it carries the same two policies as a
`BotMentality` on the seat (`standard` / `chaos`, see
[ENGINE.md](ENGINE.md#bot-ai-mentalities)). `POST /api/rooms/{id}/bots` takes
`mentality` on `op: "add"` (default `standard`); it is stored on the room
member, shown as a tag in the lobby, and copied onto the seat when the match
starts. Solo's setup screen offers the same choice (one for all bots, with a
per-bot override).

The two are meant to play alike: a chaos bot and a chaos-托管 human follow the
same policy over the same options, differing only in what they can see.

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
