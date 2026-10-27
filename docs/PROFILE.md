# Profile server

`crates/profile-server` is the user-facing half of a split deployment. It owns
the player's account and profile document, the directory of game servers the
player can connect to, and an upload area for game data (rule builds, tables,
ruleset archives). The game server ([SERVER.md](SERVER.md)) owns rooms and
matches.

**It is not an identity provider.** There is no password, no OAuth and no
external account: a player types a display name and gets an account. A real IdP
(Authentik, Zitadel, ...) can be placed in front later; the session ticket is
the seam it would plug into.

## Running

```sh
cargo run -p profile-server --release -- \
  --port 8081 --data data/profile --static webui/dist --gamedata data --secret <hex>
# env: PORT, BM_HOST, BM_PROFILE_DATA, BM_STATIC, BM_GAME_DATA, BM_PROFILE_SECRET
```

Binds `127.0.0.1` by default; pass `--host ::` / `--host 0.0.0.0` to expose it.
`--static` serves the web client (the SPA this server hosts is the entry point
where a player picks a game server). `--gamedata` (default `data`) is served
read-only at `/data/*` so the SPA's first paint — the menu, before any game
server is chosen — has its tables and roster; matches never use it. `--secret`
signs this service's own session tickets; without it a per-process secret is
generated and printed, which logs everyone out on restart.

## Relationship to game servers

The two services share **only the player's display name**:

1. the client signs in here by name and keeps this server's ticket for profile
   calls;
2. on a game server the client posts the same name to `POST /api/session` and
   gets a token local to that server;
3. profile-server tickets are never forwarded to game servers.

So a game server needs no secret, no callback and no shared store, and a single
game server deployment ignores this service entirely. The consequence is that
the name is unverified — anyone can take a known name on any game server.

## API

All responses are JSON; errors are `{error, reason?}`.

| Method | Path | Body | Response |
|---|---|---|---|
| GET    | `/api/health` | | `{service, version, uptime}` |
| POST   | `/api/session` | `{player, persona?, cnId?}` | `{token, player, account}` + `bm_profile` cookie |
| GET    | `/api/session` | | `{token, player, account}` |
| POST   | `/api/logout` | | `{ok}` |
| GET    | `/api/profile` | | `{profile}` (opaque client document) |
| PUT    | `/api/profile` | `{profile}` | `{ok}` (1 MiB cap) |
| GET    | `/api/servers` | | `GameServer[]` |
| POST   | `/api/servers` | `{name, url, note?}` | `GameServer` |
| DELETE | `/api/servers/{id}` | | `{ok}` |
| GET    | `/api/uploads` | | `Upload[]` |
| POST   | `/api/uploads?name=&kind=` | raw bytes | `Upload` (auth, 64 MiB cap) |
| DELETE | `/api/uploads/{id}` | | `{ok}` (owner only) |
| GET    | `/uploads/{id}/{file}` | | the bytes (public, immutable) |

`GameServer` is `{id, name, url, note}`; `url` is trimmed of a trailing `/` and
must be `http(s)://`. `Upload` is `{id, name, kind, size, sha256, owner, created,
url, download}`; the stored name is reduced to a safe path component, and the
declared `kind` (e.g. `ruleset`) is only a label. Uploads are stored under
`<data>/uploads/<id>/`; the SHA-256 lets an installer check what it fetched.

Auth for profile calls is `Authorization: Bearer <ticket>` (or the cookie).
Every account sees the whole server directory — the directory is shared, not
per player.

## Deployment shape

The web client is served by the profile server and points its API calls at the
profile server, its game traffic at a game server picked from the directory (or
entered by hand). Game-server CORS is permissive because the app is cross-origin
from it and `EventSource` cannot set headers. Solo play runs entirely in the
browser and works with no server at all; a custom WASM ruleset can be uploaded
here for sharing but is also applied locally.
