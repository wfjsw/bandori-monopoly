# Commit-reveal fairness (2026-10-09)

How match entropy is generated so that **no party can bias the dice** — not
the server, not a client, not somebody reading the wire — and how anybody can
check that after the fact. The scheme is the Mahjong Soul wall-hash idea
(commit first, mix player entropy in, reveal after), adapted to this engine.

The recipes live in `game-core::fair` (Rust), used by the server, the browser
engine and the tests alike. The webui calls the same code through the glue
(`fair_canon_settings` / `fair_commit` / `fair_derive_seed` / `verify_fair`).

## 1. The scheme

### 1.1 Commit (at room creation, before anyone can contribute)

When a room is **created**, the server draws two secret 256-bit values from
the OS (`getrandom`):

* `seed` — the server's entropy, the root of everything secret;
* `salt` — exists only so the commitment is hiding as well as binding
  (a low-entropy input alone would let a third party brute-force the
  commitment).

It publishes one hash and nothing else:

```text
commit = SHA-256(
  "bd-fair-commit-v2\n"
  "<seed hex>\n"
  "<salt hex>\n"
  "<engine bundle id>\n"          -- EngineStamp.bundle (docs/REPLAY.md §9)
  "<ruleset sha256 hex>\n")       -- EngineStamp.ruleset_sha256
```

`commit` is shown in the room from that moment (copyable) and sealed into the
record header. `seed` and `salt` do not leave the server: they are not in
`RoomInfo`, not in SSE frames, not in `MatchState` / seat views, not in
anything the bot service receives (which is exactly the seat view —
`docs/BOT.md` B5). They live in server-side memory, in the server-side room
record (so a restart picks the same slot back up) and in the server-side
record log head (so a restart can still seal the reveal), and cross the
network only inside the sealed record, only after the match, only to
participants (`docs/SERVER.md` "Match records").

**Why room creation and not match start.** The commitment must be fixed
*before* it sees any nonce, otherwise the server could grind its `seed`
against the nonces it already holds. Nonces arrive as players enter the room,
i.e. before the match — so the only safe moment to draw is when the room is
born. A nonce a client posts before it has seen the commit is fine for exactly
this reason: the server is already committed, and the client is not.

**What the commit binds** — and what it deliberately does not:

| piece | in the commit? | why |
|---|---|---|
| `seed` + `salt` | yes | the openings |
| engine bundle id + ruleset sha | yes | fixed per server process, known at creation |
| room settings (mode, tick step, score weights) | **no** | they can still change before the start (host edits weights, picks mode) |
| participant list | **no** | members join, leave, get kicked between creation and start |
| the nonce list | **no** | the nonces do not exist yet at creation |

The two "no" rows are what the **record header** is for: the settings are
carried in `fair.settings`, the seats in the record's own `MatchSetup`, and
the nonce list in `fair.nonces` — and the verifier checks all three against
the record body (§2). Together with `derived_seed` that pins exactly the game
that was played. The external anchor is still the commit string you copied
from the room: rewriting *everything* in a record consistently is
indistinguishable from a real record (same trust model as Mahjong Soul's wall
hash), so **copy the commitment when the room appears** if you care.

### 1.2 Player entropy (collected on room entry)

Every human client contributes a 256-bit nonce
(`crypto.getRandomValues(new Uint8Array(32))`, hex, `POST
/api/rooms/{id}/nonce`) **as it enters the room** (create / join / rejoin),
and again whenever the room shows a *new* commitment (the slot rolled after a
match, §1.5). Bots contribute none.

* **A missing nonce is simply absent.** There is no window and no wait: the
  match starts the instant the host says so. A client that never posts (an old
  client, a failed request, a spectator) costs nobody anything — it is just
  not in the list.
* **Only the members who sit down count.** At the start the server takes the
  nonces of exactly the members in the match, sorted by member id, and drops
  the rest (someone who left, someone who was kicked, a seat that was never
  filled). A leaver's nonce is dropped the moment they leave.
* **Last write wins, locked at the start.** A member that re-enters may post
  again and replace its earlier nonce; the list is frozen when the match is
  created and a later post is refused.
* **Order is irrelevant** (the derivation sorts), and the server does not
  reveal anyone's nonce before the game ends — nor does it ever show one
  client another's. It also does not matter much *whether* others' nonces
  were visible before you submit, and this is worth spelling out: the match
  seed mixes the server's secret `seed`, which was **committed before any
  nonce existed**. Seeing other nonces does not help you predict the derived
  seed (you would have to invert SHA-256), and it does not let you grind a
  nonce to a favorable stream for the same reason. What nonces buy is exactly
  this: even a server that picked its `seed` adversarially cannot know the
  match stream, because it does not know the player nonces at commit time and
  cannot change its `seed` afterwards.

The match seed is:

```text
match_seed = SHA-256(
  "bd-fair-seed-v1\n"
  "<seed hex>\n"
  "<member id>\n<nonce hex>\n"   -- one line pair per nonce, ascending member id
  ...)
```

Empty list is legal (all bots, or nobody contributed): the seed is then just
`SHA-256("bd-fair-seed-v1\n<seed hex>\n")`.

The engine keys **two** ChaCha12 streams from it (`rng.rs`):

* the game stream, keyed by `match_seed` itself;
* the live-cosmetics stream, keyed by
  `SHA-256("bd-fair-live-v1\n<match_seed hex>\n")` — never shared.

`MatchState.match_id` is **not** seed bits: it is
`SHA-256("bd-fair-id-v1\n<match_seed hex>\n")` truncated to 31 bits with the
low bit set, so a public view leaks none of the stream. (The legacy u64 path
still derives the id from the seed, as it always did — a known, documented
leak of 31 bits of a 64-bit seed, which is why the new path exists.)

### 1.3 Canonical room settings (bound by the header)

The gameplay-affecting room settings, encoded as:

```text
bd-fair-settings-v1\n
<mode as i32>\n
<step f32 bit pattern as decimal>\n
<ScoreWeights JSON, serde declaration order>\n
<id>,<player>,<bot 0|1>,<mentality as i32>\n   -- one line per seat, seat order
```

`step` is the tick quantum (`0.05 * time_scale`, `docs/REPLAY.md` §1); its
**bit pattern** is hashed, never a decimal string, so float formatting can
never disagree across platforms. Seats are the roster as the match was built
(after the advanced→standard bot rewrite the server applies when no bot
service is attached).

Under v2 this string is **not** part of the commitment (it is not known when
the commitment is drawn). It is sealed into the record header
(`fair.settings`) and the verifier requires it to equal what the record's own
setup re-encodes — which also pins the participant list, since the seats are
part of the string. **Not covered** (they cannot change a die roll): room
name, theme, password, max players, display cosmetics. Seating identity
beyond the four fields above (avatar, ready flag, …) is not covered by the
string either; it is covered by the record body's own integrity.

### 1.4 Reveal (after the match)

The sealed `.bdrec` header carries, next to the `commit` it showed from the
room's creation:

```json
"fair": {
  "v": 2,
  "commit": "<hex64>",
  "seed":   "<hex64>",
  "salt":   "<hex64>",
  "nonces": [{"member": 1, "nonce": "<hex64>"}, ...],   // ascending member id
  "settings": "bd-fair-settings-v1\n..."
}
```

Nonces are public after the game (they are in the record everyone can keep).
The field is additive and optional: a record without it predates the scheme
(`RECORD_VERSION` v1), and `verify` reports "no fairness material" rather
than pretending. A v2 record is refused by a v1 engine, so an old archived
bundle never mis-replays a new record (`docs/REPLAY.md` §9).

`fair.v` is the **scheme version** — which recipe produced the openings. The
verifier checks each record against *its own* recorded version, so old
records keep verifying:

* `v: 1` (2026-10-08): the commitment was drawn at match start and folded the
  canonical settings in
  (`SHA-256("bd-fair-commit-v1\n" ‖ seed ‖ salt ‖ bundle ‖ ruleset ‖ settings)`).
* `v: 2` (this document): the commitment is drawn at room creation over the
  openings and the engine identity only.

The openings live in the `RecordHeader` JSON, i.e. inside the plain record
frame. A **portable** `.bdrec` (`docs/REPLAY.md` §10) appends its engine as a
trailing skippable frame and never touches that header, so a portable file
carries the same fairness material and verifies exactly like a plain one.

### 1.5 Rematch / next slot

A commitment covers exactly one match. When a match ends, the room rolls a
fresh slot — new `seed`, new `salt`, new `commit`, empty nonce list — in the
same breath as `playing = false`, and shows the new commitment at once. The
clients in the room post fresh nonces against it automatically (no
user-visible wait), and the next start works exactly like the first.

## 2. Verify

The replay viewer has a **Verify** action (local-only, like the rest of
replay — records are never uploaded). It runs **in the engine bundle that
plays the record** (the archived one, if this build is not it) and reports
pass/fail per step:

| step | what it proves |
|---|---|
| `settings` | the stored canonical settings string is what the record's own setup re-encodes — and since the seats are in that string, this also pins the participant set |
| `nonces` | the nonce list is clean (32 bytes each, no duplicates) and every one of them belongs to a member who sat in this match |
| `commit` | the commitment shown from the room's creation opens: `SHA-256(seed, salt, bundle, ruleset)` (v2) / plus `settings` (v1) |
| `bundle` / `ruleset` | the record names its engine bundle and ruleset (what the loader routed on) |
| `derived_seed` | `SHA-256(seed, sorted nonces)` equals `MatchSetup.seed256` |
| `initial_rng` | the match's two RNG streams are keyed by exactly that derived seed (and its live derivation) |
| `replay` | the whole input log re-runs and every checkpoint holds |

A **pass** means: the openings the record reveals are the ones that were
committed at room creation, and the game that was recorded is exactly the game
those openings produce with that settings string and that nonce list. A
hand-edited record fails the step that covers the edit.

### What a pass does *not* prove

The record file is not a trusted third-party witness. Somebody who rewrites
the whole file — openings *and* `commit` *and* the input log — can produce a
self-consistent forgery. The external anchor is the `commit` you copied from
the room (or saw in a `room` frame at any point): if that string is not the
one the record re-opens, the record is not that match. So: **copy the
commitment when the room appears** if you care. This is the same trust model
as Mahjong Soul's wall hash.

## 3. Threat model

| Adversary | Goal | Why it fails |
|---|---|---|
| **Malicious server** | pick a `seed` that makes a chosen player win | the `seed` is committed when the room is created, before any nonce exists, so the server cannot adapt it to player entropy; and the derived seed mixes nonces it cannot predict at commit time. Grinding `seed` candidates before committing buys nothing it can evaluate. |
| **Malicious server** | adapt the roster / settings *after* seeing the nonces to land on a favorable derived seed | the derived seed is determined by the seated roster and their nonces; a server that kicks members or rewrites settings to chase a better stream changes the game everyone sees (and the header checks make the record show exactly what ran). This is a visible, social attack — the anonymity of a nonce is what the scheme protects, not the host's right to kick people. |
| **Malicious server** | refuse to reveal / abort matches that go badly | possible — abort/denial is not prevented by any commitment scheme. The record then simply never appears; the commitment alone shows the match was never opened. Out of scope. |
| **Malicious client** | bias its own dice | the stream is keyed by the server's secret seed, which the client never sees before the end. The client's own nonce only adds uncertainty it cannot control the direction of (see §1.2). |
| **Malicious client** | grind its nonce against the others | the derived seed is a SHA-256 of the server seed it does not know; no nonce choice is better than any other. Nonce visibility before submitting would not change this. |
| **Anyone on the wire** (SSE, REST, bot service) | learn the seed mid-match and predict rolls | seed/salt never enter a client-facing payload — covered by a test that scans the create / join replies, the room views **before the start**, the start reply, `RoomState`, SSE frames and seat views for the openings (`server/tests/http.rs`: `the_openings_never_reach_a_client_payload`). The match view carries no RNG state at all (`MatchState` has no seed field). |
| **A bot** | read the true RNG state and look ahead | the bot service receives exactly the seat view a human gets (`docs/BOT.md` B5), which has no RNG field; its search forks re-seed their own RNG from the decision seed (`bot-core/src/determinize.rs` — "RNG: never in the view, fresh seed per fork"). The decision seed is hashed from public decision identity, never the match RNG. |
| **Record tampering** | edit a record after the fact | any single-field edit of the openings fails the step that covers it (tests in `game-core/tests/fair.rs`); editing the input log breaks `replay`. Rewriting *everything* consistently is indistinguishable from a real record — see "What a pass does not prove". |
| **RNG state reconstruction** | read a save / snapshot and predict the future stream | the ChaCha key is the derived seed; a `Match::save` blob *does* contain the RNG state and would leak the stream to whoever holds it — which is why a save blob is never sent to a client or to the bot service (server-side `CrossState` only). Note the live save/resume blob in the browser (solo) is local by definition. |

## 4. RNG

`game_core::rng::Rng` keeps its API (`d` / `below` / `shuffle` / `loaded` test
seam) and now holds one of two streams, tagged by its JSON shape:

* `"c": {"key": "<hex64>", "word_pos": n}` — **ChaCha12**
  (`rand_chacha::ChaCha12Rng`), keyed by a 256-bit seed. Every new match
  (online and solo). ChaCha12 over ChaCha20: same construction, 12 rounds;
  no distinguishing attack is known below 7, and this is a game entropy
  source, not a bulk cipher — the reduced round count is measurably cheaper
  on wasm32 where both the sim and the browser draw heavily. The portable
  `rand_chacha` implementation is deterministic across platforms
  (pure Rust, no SIMD-dependent output).
* `"s": [u64; 4]` — the **legacy xoshiro256\*\*** state, for saves and
  records written before the switch. `Match::new(u64)` still keys this
  stream, so an old `Init::Seed` record replays byte-identically
  (`game-core/tests/fair.rs`:
  `legacy_u64_records_still_replay_on_the_legacy_stream`) and an old solo
  save in `localStorage` resumes on the same stream. New matches must use
  `Match::new_seeded(Seed256)`. Throwaway search forks (`bot-core`'s
  determinizer) also keep the legacy constructor — their stream is theirs
  alone and pinned by test.

**Save version:** `engine::SAVE_VERSION` stays at 5. The `Saved` document's
shape is unchanged; only `Rng`'s encoding grew a second, self-describing
form that the old one still parses. An old solo save therefore resumes
across this deploy with no migration and no refusal (`docs/REPLAY.md` §9
archived engines handle old *records*; this handles old *saves*).

## 5. Solo

Solo runs the same recipe locally (the browser draws `seed`, `salt` and the
human's nonce with `crypto.getRandomValues`, derives, and seals the openings
into the exported record), so a solo record verifies exactly like an online
one. The UI **does not show the commitment for solo** — there is no
adversary, so a commitment proves nothing; the uniformity is for the code
path and the record format, not for trust. The engine-side smoke test
`webui/src/game/soloMatch.test.ts` plays a whole seeded solo match through
the built wasm glue (`quick_start` + `tick_steps` to completion +
`record_zst`), so a wasm-side clock read or any other trap in that path
fails a gate instead of a player's game.

## 6. Operational notes

* **Server restart** is invisible to the scheme: the room's slot (seed, salt,
  commitment, already-collected nonces) is persisted with the room record,
  server-side, exactly like the record log head. A restart mid-match still
  seals the reveal from the log head; a restart in the lobby keeps the same
  commitment and the same nonces. A room record written before the slot was
  persisted simply gets a fresh slot on restore, before anybody can post a
  nonce against it.
* **The room record and the record log head are server-side storage**, not
  client-facing payloads: they are the only places the openings live between
  the room's creation and the reveal, and they are what a restart needs to
  keep going / seal. Clients can download the sealed record only after the
  match, only if they sat in it.
* **Re-entering** (a page refresh, an SSE reconnect) posts a fresh nonce for
  the current slot; the last write wins and the list is frozen at the start.
  Mid-match re-entry posts nothing — the slot is already consumed — and the
  next slot gets a nonce the moment it appears.
* Recipe version tags (`bd-fair-*-v1` / `-v2`) are inside every hash.
  Changing a recipe means a new `fair.v` and new tags; old openings then
  fail verification loudly instead of quietly.