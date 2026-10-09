# Match records and replay — design (2026-10-07)

Status: P1 (core), P2 (web-glue), P3 (solo UI) and P4 (server + rules-worker)
implemented 2026-10-07; P5 remains. The same day, storage and download moved
from gzip to **zstd** (see "Framing and codec"): one shared codec in
`game-core`, the server seals compressed, and the loader still reads gzip and
plain JSON. Later that day the **engine archive** landed (§9): a record is
replayed by the engine bundle that wrote it, so later engine / rules / data
changes cannot make old records diverge.
Line numbers were taken from the 2026-10-07 working tree and drift.

**Implemented:** `crates/game-core/src/record.rs` (schema, `RecordedMatch`,
`Recorder`, `Replayer`, `compat`, the zstd codec), `crates/game-core/tests/record.rs`
(§7 core matrix) and `tests/record_codec.rs`, the additive web-glue surface (`engine_stamp`,
`SoloMatch.tick_steps` / `record_state` / `restore_with_record` / `record` /
`record_with_events` / `record_zst` / `record_with_events_zst`,
`ReplayMatch` / `from_record_bytes`, `record_header` / `record_header_bytes`)
with regenerated `webui/src/wasm/glue.{js,d.ts}`, the solo UI
(`webui/src/game/{record,replay}.ts`, `webui/src/scenes/replay/*`), and the
server / rules-worker path (`info` / `cp` / `replay` worker ops, the quantized
ticker, `MatchHandle`'s record log, `CrossState::record_*`,
`GET /api/rooms/{id}/record`). See `docs/ENGINE.md`
§Records for the API as built, `docs/SERVER.md` §Match records for the
endpoint policy, and the deviations below.

## 0. Findings that drive the design

* **The engine is deterministic** given:
  * the start: either `seed` + members + mode + weights, or a `save()` snapshot;
  * the ordered public calls: `act`, `quick_start`, `finish`, `member_left`
    and `member_back`;
  * the exact `tick(dt)` f32 sequence.
* **Randomness and serialization:**
  * Both RNG streams (`World.rng` and `Match.live_rng`) are serialized.
  * Bot answer delays draw from `live_rng`.
  * Chaos bots use the match RNG.
  * HashMaps are lookup-only.
  * There is no wall clock and no `thread_rng`.
  * `save()` JSON is byte-stable, because `Msg` args and props are BTreeMaps.
* **The non-determinism is all in the drivers:**
  * browser dt jitter: `performance.now()`, capped at 0.5;
  * server dt jitter: `Instant` × `time_scale`;
  * human-act interleaving: serialized on the server by `MatchHandle.gate`;
  * JS `Math.random` in the autopilot, which only produces inputs.
* **dt matters.** Auction bids and bot-answer ordering against human bids
  depend on which tick crosses `ai_at`. A record must therefore reproduce the
  exact dt sequence.
* **`world.recent` is a 400-event tail**, so a full event log must be
  collected outside the engine.
* **The server is stateless.** The whole `Match::save` blob round-trips through
  the rules-worker per op at 20 Hz. The record log must live beside the blob,
  not inside it.
* **The events op is not per-viewer filtered.** The event log is public; only
  `hand`, `draw` and `aiAnswer` are private.

## 1. Decisions

* **Local files play only in the browser** (user rule, 2026-10-07). A record
  opened from disk, or kept in IndexedDB, is decoded and replayed entirely by
  the web client's wasm engine (`ReplayMatch`). It is never uploaded to the
  server, and the server never hosts, stores or replays a client's file.
  * The server's only record route is the read-only
    `GET /api/rooms/{id}/record` (download of a match it played itself).
  * There is deliberately no upload / POST route.
  * The worker `replay` op is internal (tests / admin), with no HTTP exposure.
* **Format.** A hybrid, input-log first. The authoritative body is:
  * an `Init`;
  * ordered `Input`s;
  * run-length tick runs;
  * per-turn state-hash checkpoints;
  * a final hash.

  It is optionally bundled with the public event log, generated at export by
  re-simulating, as a version-robust fallback. Perspective (view-frame)
  records are deferred.
* **Time is quantized.**
  * Both drivers tick in whole quanta: `dt = k as f32 * step`, where
    `step = 0.05 * time_scale` and k is 1..=10.
  * The record stores `Ticks{k, n}` runs, and replay recomputes the identical
    f32.
  * Solo uses a fixed-step accumulator. The server keeps an accumulator in
    `spawn_ticker`.
* **Solo** records locally: the full record, all seats.
* **Online:**
  * The server records the full input log.
  * `GET /api/rooms/{id}/record` serves the last finished match's record:
    * 409 `err.record.live` while playing;
    * 403 to anyone who wasn't a participant;
    * 404 when there is none.
  * The full record reveals every hand and the deck order, so it is only
    available after the match ends, and only to participants.
* **Serialization:**
  * The body is serde JSON; the **file is that JSON in a zstd frame** (`.bdrec`).
    Compression runs in Rust -- the browser has no reliable zstd -- through one
    shared codec in `game-core` (`encode_record_zst` / `decode_record`). See
    "Format and codec" below.
  * The server serves the sealed bytes as-is:
    `Content-Type: application/zstd` and
    `Content-Disposition: attachment; filename="bdrec-<room>-<yyyymmdd-hhmm>.bdrec"`.
    No `Content-Encoding: zstd` -- browsers would transparently decode that,
    and inconsistently.
  * Extension `.bdrec`. The loader sniffs the zstd magic `28 B5 2F FD`, the
    gzip magic `1f 8b` (older browser downloads and IndexedDB rows) and plain
    JSON.
  * `check` = FNV-1a-64 hex of `serde_json::to_string(&body)`, recomputed after
    parsing.
  * u64 values are serialized as strings.
* **Replay UI.**
  * `ReplaySession extends GameSession` (`kind: "replay"`) feeds the unchanged
    Board and Animator.
  * Input is locked via a shared read-only flag. `recorded = true`, so a replay
    gives no profile rewards.
  * Controls: play/pause, 1×/2×/4×, previous/next turn, a scrub bar with turn
    marks, a perspective dropdown (any seat or spectator), skip-idle, and a
    divergence banner.
  * Seek: a background index pass keeps `save()` keyframes about every 4 turns,
    or every 20 s of game time. A seek restores the nearest keyframe and
    re-simulates forward. Board remounts with `key={epoch}`.
  * Entry points: home menu 「回放 / Replays」 (list, open file, drag and drop);
    the post-match Results 「下载回放」 / 「观看回放」; and the Room scene's
    "download last replay".
* **Version stamp.** `EngineStamp{format, save_version, abi, ruleset_sha256,
  data_sha256, engine, build, bundle}`.

  | Case | Policy |
  |---|---|
  | `format` newer than supported | Refuse. |
  | ABI differs, or a card used in the record is missing | Refuse full replay. Offer the log-only view (header plus bundled events). |
  | The record's engine **bundle** is not the running build | Load that bundle from the engine archive and play there (`§9`). Mismatch is routing information, not a warning. |
  | The record's bundle is missing from the archive | Refuse, naming the bundle id. Never silently re-simulate with a different engine. |
  | Everything matches | Play on the running engine. |

  At the first checkpoint mismatch, auto-pause and show 「Replay diverged at
  round R, turn T」. The user can continue (flagged inaccurate) or switch to the
  log view. With the matching bundle loaded, a checkpoint mismatch means the
  record itself is damaged -- not "a different build".
* **Storage.**
  * The last 10 solo records go in IndexedDB (`bm.replays`), as the zstd
    bytes. A record is saved automatically once when a solo match ends. Older
    rows, which are gzip, still list and play -- the engine decodes both.
  * Download via a Blob and `a[download]`; the file is the same zstd bytes.
    Open via `<input type=file accept=".bdrec,.json">` or drag and drop --
    zstd, gzip and plain JSON all load.
  * Size (a real 200-round bot game, `tests/record_codec.rs`):

    | Body | raw JSON | zstd (native, what the server seals) | zstd (wasm / structured-zstd) | gzip |
    |---|---|---|---|---|
    | input log | 77 769 B | 14 403 B (5.4x) | 14 307 B (5.4x) | 17 659 B (4.4x) |
    | + events | 879 015 B | 65 132 B (13.5x) | 65 042 B (13.5x) | 68 074 B (12.9x) |
* **Performance.** `Match` is not modified; recording is a wrapper, so
  `examples/sim.rs` is unchanged. The per-tick cost is one run-length
  increment. At each turn change: `save()` plus FNV.

## 2. Schema (`crates/game-core/src/record.rs`)

### Framing and codec

A `.bdrec` is a **zstd-framed `RecordFile` JSON**. The codec lives in
`game-core::record` and is shared by every caller (browser, server, worker):

* `encode_record_zst(&RecordFile) -> Vec<u8>` -- what storage and download
  write.
* `decode_record(&[u8]) -> Result<RecordFile, ReplayError>` and
  `parse_header_bytes(&[u8])` -- what every reader uses. Both **sniff the
  magic** first (`sniff_record`):

  | Magic | Framing | Where it came from |
  |---|---|---|
  | `28 B5 2F FD` | zstd | the current form, every writer |
  | `1F 8B` | gzip | the browser's old `CompressionStream("gzip")` |
  | anything else | plain JSON | hand-edited files, records stored before compression |

  All three play everywhere: the same decode path runs in wasm and natively.

* **Encoders.** zstd, two implementations, chosen by target:
  * **wasm32** -- `structured-zstd` at `CompressionLevel::Fastest` (its
    level 1; `zst_encode_pure`). Pure Rust, so no C toolchain in the wasm
    build (`zstd-sys` would need one). It is the maintained continuation of
    `ruzstd` by the same author, with a real level table where `ruzstd`'s
    encoder only implements `Fastest` -- which measured **worse than gzip**
    on record JSON (3.1x vs gzip's 4.4x on the input log). `structured-zstd`
    lands on the reference binding's ratio instead (table above).
  * **native** -- the reference `zstd` binding at level 1 (better ratio than
    levels 3 and 9 on record JSON). The server seals and serves the bytes
    where the ratio shows up in Redis and in the download.
  Both emit standard zstd frames; `zstd_frames_are_standard` cross-checks
  each direction against the reference binding -- including the browser
  encoder's bytes, so wasm and native agree on the wire format.
* **Decode** is `ruzstd` everywhere (plus `flate2` / miniz_oxide for gzip).
  One decoder reads both encoders' frames and older recordings.
* `check` still covers the **JSON body**, not the frame: recomputing it is a
  parse away, and the frame is transport only.

### Schema

```rust
pub const RECORD_VERSION: u32 = 2;   // v2: + MatchSetup.seed256, RecordHeader.fair (docs/FAIRNESS.md)
pub const STEP: f32 = 0.05;
pub struct RecordFile { magic: String /*"bdrec"*/, header: RecordHeader, body: RecordBody, check: String }
pub struct EngineStamp { format: u32, save_version: u32, abi: u32, ruleset_sha256: String /*"stub"*/, data_sha256: String, engine: String, build: String, bundle: String }
pub struct SeatInfo { member: i32, player: String, bot: bool, mentality: BotMentality, character: String, rank: i32, score: i32 }
pub enum Origin { Solo, Online { room: String } }
pub struct RecordHeader { engine: EngineStamp, mode: MatchMode, step: f32, origin: Origin, created: String,
                          seats: Vec<SeatInfo>, partial: bool, ended: bool, reason: String, rounds: i32, total_ticks: u64 /*str*/,
                          #[serde(default)] fair: Option<fair::Fairness> /*docs/FAIRNESS.md, additive*/ }
pub struct MatchSetup { members: Vec<RoomMember> /*incl. mentality*/, seed: u64 /*str*/ /*legacy xoshiro*/, weights: ScoreWeights,
                        #[serde(default)] seed256: Option<rng::Seed256> /*hex; the ChaCha key every new match uses*/ }
pub enum Init { Seed(MatchSetup), Snapshot { save: String } }
#[serde(tag="t")] pub enum Input { Ticks{k:u8,n:u32}, Act{m:i32,msg:NetMessage,ok:bool}, QuickStart, Finish, Left{m:i32,can_return:bool}, Back{m:i32} }
pub struct Checkpoint { at: u32, tick: u64, round: i32, turn: i32, hash: String }
pub struct RecordBody { init: Init, inputs: Vec<Input>, checkpoints: Vec<Checkpoint>, final_hash: String, #[serde(default)] events: Option<Vec<MatchEvent>> }
```

* Rejected acts are recorded too (`ok=false`), and replay asserts the same
  Ok/Err.
* A tick run is split at turn boundaries, so every checkpoint sits between
  inputs.
* Recording stops once the match has `ended()`.

## 3. Engine hooks (no edits to `engine/mod.rs`)

* **`RecordedMatch { m: Match, log: Recorder }`** forwards to the public
  `Match` entry points: `new`, `restore`, `save`, `tick`, `act`, `finish`,
  `quick_start`, `member_left`, `member_back` and `events_since`. The turn key
  is read via `world()`. Its methods:
  * `new(data, rules, setup, mode)`;
  * `from_snapshot(m)` (`partial`);
  * `tick_steps(k)`, `act`, `quick_start`, `finish`, `member_left`,
    `member_back`;
  * `inner()`, `recorder_json()`, `restore(data, rules, save, rec_json)` and
    `export(stamp, created)`.
* **`Replayer`**:
  * `new(data, rules, &RecordFile, force) -> Result<_, ReplayError{Format, Corrupt, Incompatible(Vec<Mismatch>)}>`;
  * `step_ticks(n) -> Status{tick, ended, diverged}`;
  * `next_input()`, `index(budget)`, `seek(tick)`, `turns()`, `total_ticks()`,
    `match_ref()`;
  * `compat(&a, &b)`.

## 4. Glue, rules-worker, server

* **web-glue** (additive):
  * **Stamp inputs:** `load_data` computes `data_sha256` (sha256 of the
    `DATA_FILES` contents in order). `ruleset_build` captures `set.sha256()`.
    `engine_stamp()` exposes both.
  * **`SoloMatch`** holds a `RecordedMatch`. New methods:
    * `tick_steps(k)`;
    * `record_state()`;
    * `restore_with_record(save, rec)`;
    * `record(created)` / `record_with_events(created)` (JSON) and
      `record_zst(created)` / `record_with_events_zst(created)` (the zstd
      `.bdrec` that storage and download write).

    `tick(dt)` is kept but records nothing.
  * **`ReplayMatch`:** `from_record(json, force)` (the constructor) and
    `from_record_bytes(bytes, force)` (zstd / gzip / JSON), plus `header`,
    `compat`, `step`, `next_input`, `index`, `seek`, `turns`, `total_ticks`,
    `view(member; 0 = spectator)`, `events_since`, `take_changed`.
  * `record_header(json)` and `record_header_bytes(bytes)` give a cheap header
    parse for the list.
* **rules-worker:**
  * an `info` op returning the `EngineStamp`;
  * mutating ops add `"cp":{round,turn,hash}` when the turn key changed;
  * a `replay` op `{record}` → `{final_hash, diverged}`.
* **Server:**
  * **`spawn_ticker`:** an accumulator. `k = floor(acc/0.05).min(10)`; skip
    when k is 0; call `tick_all(k as f32 * step)` and pass k through.
  * **`MatchHandle::run`:** after a successful put, append the input (act with
    `ok`, tick runs buffered, quick_start, finish, member_left, member_back)
    and the `cp` checkpoints. Flush before other inputs, every 100 ticks, and
    at the end. Failed worker round-trips are not recorded.
  * **`start`:** `Init::Seed` plus the stamp from `pool.info()`.
  * **On end:** finalize the `RecordFile` and store it as `record:{room}:last`
    with a 24 h TTL, together with the participant member ids.
  * **Store:** `CrossState` gains `record_log_append/get/del` and
    `record_put/get`, in both Dummy (an LRU of 64) and Redis.
  * **`api.rs`:** `GET /api/rooms/{id}/record`, returning 409 / 404 / 403 as
    above.

### As built (2026-10-07, P4)

Everything above is in place, with these departures from the letter of the
design — all of them forced by something the design already says:

* **`header.step` is `0.05 * time_scale`, not a constant `0.05`.** The server
  ticks `k * step` (see the accumulator), so a record written on a scaled
  clock has to name the quantum it used or a replay would feed the engine
  different `tick(dt)` f32s and diverge. Solo is unscaled, so its `step` is
  still `STEP`. `Replayer` has always read `header.step`.
* **`RecordHeader.gaps`** (new, `#[serde(default)]`) marks a record whose
  buffered tick run was lost to a restart. §1's "mark the record
  unverifiable" needed somewhere to put the flag.
* **The record is sealed zstd-compressed, and served as `application/zstd`
  with no `Content-Encoding`.** §1 asked for `Content-Encoding: gzip`; the
  shipping form is zstd-in-the-body instead (see "Framing and codec"), because
  a `Content-Encoding` is something a browser would transparently -- and
  inconsistently -- undo. `Content-Disposition` is set as specified. The
  in-progress log stays plain JSON lines; only the sealed blob is compressed.
  A record stored before compression is served as-is as `application/json`.
* **Participants are member ids resolved through the room's live token map.**
  A participant who left the room (and would rejoin under a fresh member id)
  is outside that set — deliberately narrow, since the record reveals every
  hand.
* **`CardRules::ruleset_sha256()`** (a defaulted trait method) is how the
  worker fills `EngineStamp.ruleset_sha256`; `StubRules` reports `"stub"`.
  `Ctx::load` hashes the `DATA_FILES` contents for `data_sha256`, the same
  recipe web-glue uses. The in-process `Ctx::new` (tests, and the fallback
  when no worker binary is installed) leaves `data_sha256` empty, which
  `compat` reads as "unknown" and skips.
* **The in-memory store has no clock.** `record:{room}:last` is an LRU of 64
  there instead of a 24 h TTL; Redis does `SET EX 86400`.

## 5. UI (webui)

* **`game/session.ts`:**
  * Solo uses a fixed-step accumulator calling `m.tick_steps(k)`.
  * `persist` saves `rec: m.record_state()` atomically with the save. `resume`
    calls `restore_with_record`.
  * When the match ends, `saveReplay(m.record_zst(now))` runs once (the zstd
    bytes go to IndexedDB and to the Results buttons).
* **Shared lock:**
  * `GameSession.readOnly`.
  * `core/hooks.ts` `useAutoplay` = `isAuto(mode) || !!s?.readOnly`.
  * The 托管 toggle is hidden when read-only.
* **`ReplaySession`:**
  * a 50 ms timer, `step(k * speed)`, and skip-idle;
  * `seek`, `nextTurn` and `prevTurn` bump `epoch`;
  * `setPerspective(member)`;
  * `act()` returns `err.replay`;
  * `index()` runs in setTimeout chunks.
* **`game/record.ts`:** framing sniffing (`zstd` / `gzip` / `json`),
  `downloadRecord`, `readRecordFile` (raw bytes), the legacy gzip helpers, and
  an IndexedDB store (list, put with keep = 10, get, delete). The engine does
  the compressing and decompressing.
* **Router:** `/replay` (list) and `/replay/view`. Scenes:
  `scenes/replay/{Replays,ReplayPlayer,ReplayBar}.tsx`, plus the compat dialog.
* **Elsewhere:**
  * `Menu.tsx` 「回放」;
  * `Results.tsx` download and watch buttons;
  * `Room.tsx` "download last replay";
  * `net/api.ts` `record(id)`;
  * `Animator.speed`;
  * replay branches on `sess.kind` in Board, Side and Results.

## 6. Phases

* **P1 – core.** `record.rs` and `tests/record.rs`. **done 2026-10-07.**
* **P2 – glue.** web-glue and regenerated bindings. **done 2026-10-07.**
* **P3 – solo UI.** **done 2026-10-07.**
* **P4 – server and rules-worker.** **done 2026-10-07.**
* **P5 – polish:**
  * speed, skip-idle, the perspective switcher;
  * events bundle and the log-only fallback;
  * docs.

## 7. Tests

* **`crates/game-core/tests/record.rs`:**
  * Round-trip: standard, chaos and mixed bots; 2–6 players; plus a scripted
    human with random legal and rejected acts at random tick offsets, and
    random k. Replaying must give a byte-equal `save()`, every checkpoint
    matched, and an equal event stream.
  * Seeds 0..64, plus a 1000-seed `#[ignore]` variant.
  * Partial `Init::Snapshot` records.
  * Corruption gives `Corrupt`; an unknown format gives `Format`.
  * Stamp mismatch: `compat` reports it, strict mode refuses, `force` plays. A
    tampered act is caught as divergence at the expected checkpoint.
  * Seek equals a linear replay.
  * Rejected acts replay to the same Err.
  * u64 and f32 values are bit-exact.
* **`crates/game-rules/tests/record_wasm.rs`:** a real-ruleset round-trip,
  cross-backend if possible.
* **`crates/game-core/tests/record_codec.rs`:** encode / decode round trip
  through all three framings; magic sniffing; the reference `zstd` binding
  must read what the browser encoder (`zst_encode_pure`) and the native
  encoder write, and vice versa; corrupt input is `Corrupt`;
  and the size comparison on a real 200-round bot game (raw JSON vs zstd vs
  gzip, input log and events-bundled).
* **`server/tests/http.rs`:** 409 mid-match, 200 after the end, 403 to an
  outsider; left / back are recorded; the response is `application/zstd` with
  the zstd magic and no `Content-Encoding`; the `replay` op (bytes and JSON
  forms) ends on the final blob hash.
* **webui tests:** framing sniffing, gzip round-trip (legacy), a zstd record
  through the store, `decodeRecord`'s refusal of zstd, IndexedDB keep-N.
* **Performance:** the sim is unchanged, and `RecordedMatch` overhead stays
  under 2%.

## 8. Risks

* **Silent logic drift.** Caught by checkpoints and the divergence UI. The
  bundled events keep a log view working. The engine archive (§9) removes the
  usual cause -- replaying with a build the record never saw.
* **Cross-backend wasm float NaN.** Test it.
* **Server restart** loses up to 100 buffered ticks. Set a `gaps` flag and mark
  the record unverifiable.
* **Concurrent work.** Bot mentality must flow through `MatchSetup`.
* **Keyframe memory in wasm.** Cap it by bytes.
* **Privacy.** A full record is intended; it is post-match and
  participants-only.
* **Old solo saves** give partial records.

## 9. Engine archive: versioned bundles (2026-10-07)

Every record produced from now on must replay **exactly**, no matter how the
engine, card rules (ABI / ruleset), game data or save format change later.
Re-simulation stays the replay method -- it is exact and checkpoint-verified --
but a record is always played back by **the engine build that wrote it**, not
by whatever engine is current. That build is kept in a versioned **engine
archive**.

### 9.1 Bundle identity

A **bundle** is one replay engine plus the inputs its stamp hashed:

* the glue (`webui/src/wasm/glue.js` + `glue_bg.wasm`);
* the ruleset index and its content-addressed modules;
* the game tables (`DATA_FILES`);
* the record format / save format versions and the card ABI.

The id is a sha256 over those identities -- one recipe, in
`game_core::record::bundle_id` and its JS twin `tools/engine-bundle.mjs`:

```text
bundle = sha256_hex("bdre-bundle-v1\n"
                    "<glue_sha256>\n<ruleset_sha256>\n<data_sha256>\n"
                    "<format>\n<save_version>\n<abi>\n")
```

`glue_sha256` is sha256 over the glue's `glue.js` bytes followed by its
`glue_bg.wasm` bytes. It is computed by `tools/build-glue.mjs` and written to
`webui/src/wasm/engine_id.json`; the webui hands it to `set_glue_sha` at boot,
and `engine_stamp()` then carries the derived `bundle` in every record it
seals. An empty `glue_sha256` yields `""` -- an unknown bundle, never a wrong
one. `tools/test-replay-archive.mjs` pins the JS recipe and the engine's
`bundle_id` against a real glue build; `crates/game-core/tests/bundle_id.rs`
pins a known vector.

The same recipe applies to the **server**: `rules-worker` fills `bundle` from
the deployed glue identity (`BD_GLUE_SHA`, or the `glueSha256` in
`BD_ENGINE_ID`, default `webui/src/wasm/engine_id.json`) plus its own ruleset
and data hashes, so a server-made record and a browser-made record of the same
deploy seal the **same** stamp and resolve to the same bundle.

### 9.2 Archive layout

`tools/archive-engine.mjs` freezes the build being deployed and copies it into
the served site. The **index** is versioned in git; the **bytes** are not (the
user does not want wasm in git). Two identities per bundle, deliberately
separate:

* **`id`** (and the `bundle` field of every `EngineStamp`) is the **byte**
  identity -- sha256 over the glue js+wasm, the ruleset module hashes, the
  data hashes and format/save/abi (`tools/engine-bundle.mjs`). This is what a
  record stamps, so it must stay stable. It also changes when something that
  does not affect game behaviour changes: rustc version, wasm-bindgen version,
  whether `rust-src` is installed, absolute paths in panic strings.
* **`source_id`** is the **behaviour** identity -- sha256 over the inputs that
  change how a record replays, and nothing else (`tools/engine-source.mjs`).
  Two builds with the same `source_id` are the same engine.

`source_id` covers (git tree/blob object ids when the tree is clean, sha256 of
the same file set's contents when it is dirty):

```text
crates/game-core, crates/game-rules, crates/web-glue, crates/rules-cond,
third_party/cel-rust,                       (the whole trees)
rules/**,                                   (card-sdk, cards, skills, tiles,
                                             events, fixtures, both locks)
Cargo.lock, rules/Cargo.lock,               (dependency resolution)
Cargo.toml, rules/Cargo.toml,               (the build graph: members, the cel patch)
the DATA_FILES the bundle hashes,           (the game tables)
format / save_version / abi                 (the record schema)
```

and excludes, on purpose: rustc / cargo / wasm-bindgen versions, build flags
and recipes (`tools/*.mjs`), paths, timestamps, webui, docs, bots. The
advanced-bot worker bundle (`crates/bot-glue` /
`webui/public/assets/engine/bot-glue/`, `docs/BOT.md` B6) is excluded too: a
record stores the bot's **answers** as inputs and replays never run the bot,
so the archive does not need those bytes.

Layout:

```text
archive/engine/
  index.json                  VERSIONED -- bundle id -> stamp + provenance
  refs/<bundle>.bdrec         VERSIONED -- short seeded match per bundle,
                              the behaviour check every rebuild must pass
data/engine-archive/          persistent store, outside git (backup this!)
  modules/<sha256>.wasm       content-addressed card modules, shared
  <bundle>/…                  the engine + the tables it hashed
data/engine-cache/            tools/rebuild-engine.mjs output (outside git)
webui/dist/assets/engine/     served copy of index + store + cache
  replay-worker.js            the frozen-API driver (webui/public/... copy)
```

Only what replay needs is kept: no audio, no live2d, no UI. Size on the
2026-10-07 build: **5.9 MiB bundle + 0.4 MiB shared modules ≈ 6.2 MiB**. The
modules pool is content-addressed -- filenames are their sha256 -- so bundles
share it. Each reference record is a few kB of input log (two bots, seed 7).

What is **not** in the archive: the site's own workers. The live solo match
runs in a module worker the page bundles beside itself
(`webui/src/game/soloWorker.ts`, rsbuild's worker chunk) and loads **this
build's** glue -- the same `webui/src/wasm/glue*` bytes the page imports, same
`engine_id.json`, so a record it seals names this bundle exactly. The
advanced-bot worker bundle is excluded for the same reason as before (§9.2):
replays never run a bot. Archived bundles are still driven only by
`replay-worker.js`; an old record is unaffected by where the live match runs.

Each index entry records how the bytes can be reproduced:

| Field | Meaning |
|---|---|
| `source_id` | behaviour-level identity (see above) |
| `ref` / `ref_sha256` | the reference record every rebuild must replay clean |
| `commit` | full sha the build ran on; null when unknown |
| `dirty` | the input paths differed from `commit` at seal time |
| `rebuild` | `tools/rebuild-engine.mjs <id>` may regenerate this bundle |
| `toolchain` | `rustc` / `cargo` / `wasm-bindgen` versions (informational) |
| `files` | `{relpath: sha256}` -- what `--check` verifies |
| `aliases` | byte-id -> serve-id, when a rebuild's bytes hashed differently |
| `rebuilt` | set by a verified rebuild: its byte id, source_id, ref, toolchain |

`rebuild: false` means the source cannot be regenerated (sealed from a dirty
tree or a snapshot). Keep those in `data/engine-archive/` and back that
directory up.

**Never hardlink a build output into the store.** `webui/src/wasm/glue_bg.wasm`
is rewritten in place by every `tools/build-glue.mjs` run; on NTFS that write
goes through a hardlink and destroys the archived bytes. This happened to
bundle `e4e20956…` on 2026-10-08 (its `glue_bg.wasm` is unrecoverable from
this machine; the entry is flagged `damaged` and `--check` refuses the deploy
until the bytes are restored from a `--backup`). `tools/archive-engine.mjs`'s
`copyOrLink` now hardlinks only between frozen trees (store <-> dist) and
copies everything else.

Usage:

```bash
node tools/archive-engine.mjs                       # this repo's build
node tools/archive-engine.mjs --from <deployed-dir> # a snapshot (glue/ + data/ + dist/assets/rules/)
node tools/archive-engine.mjs --check               # fail unless store + refs cover every entry
node tools/archive-engine.mjs --backup <dir>        # copy the store aside (never uploads)
node tools/rebuild-engine.mjs <commit|bundle-id>    # regenerate + behaviour-verify
```

`--no-dist` skips the site copy; `--not-current` archives without moving the
`current` pointer. `--check` is the deploy gate: a bundle that is neither in
the store nor rebuildable, or whose reference record is missing / hash-broken,
fails the run.

### 9.3 Stable replay API (frozen)

Every archived bundle exposes the same driver surface -- **v1** is exactly the
`ReplayMatch` / `record_header*` methods web-glue has shipped since P2
(`replay_api_version()` returns 1 and is optional on bundles that predate it):

```text
record_header_bytes(bytes) -> RecordHeader JSON
ReplayMatch.from_record_bytes(bytes, force) -> handle
  header() / compat() / status() / step(n) / seek(tick) / index(budget)
  turns() / total_ticks() / view(seat|0) / events_since(id) / take_changed()
  ended() / next_input() / free()
```

The UI may only drive engines through `webui/src/game/replayEngine.ts`
(`ReplayHandle`), which speaks this set to either the page's wasm or
`webui/public/assets/engine/replay-worker.js` (a module worker that loads an
archived bundle's glue as its own wasm instance and posts view JSON back --
the worker plumbing `docs/BOT.md` §3.6 wants, reused there later).

**MatchState JSON compatibility.** A frame from an older bundle must render in
the current UI. The rule: *`MatchState` changes are additive* -- new fields get
defaults and old frames keep playing. Anything else needs a TS-side migration
keyed by bundle id. `normalizeMatchView` in `replayEngine.ts` fills the fields
that have grown since (prompt prices, per-player keyed state, tile marks,
movement plan, ...) so a v1 frame reaches the Board whole.

### 9.4 Loader policy (replaces "warn, then diverge")

`resolveBundle` (`webui/src/game/engineBundle.ts`) picks the engine:

| Case | Policy |
|---|---|
| `format` newer than supported | Refuse. |
| ABI differs | Refuse full replay. |
| record `bundle` == the running build's bundle | Play on the page's engine (no worker). |
| record `bundle` is in `index.json` | Boot that bundle in the worker and play there. |
| record `bundle` is missing | Refuse, naming the id and the path `/assets/engine/<id>/`. |
| record has no `bundle` (written before §9) | Match the stamp's identity fields (`format`, `save_version`, `abi`, `ruleset_sha256`, `data_sha256`) against the archive and the running engine; prefer the running engine when it matches, else the archived build that does. |
| no match at all | Refuse: written by a build that was never archived, not reproducible exactly. |

A stamp mismatch is therefore **routing information** ("load the other
bundle"), not a "continue anyway?" prompt. The old warn-then-diverge path is
gone; the divergence banner remains for genuine checkpoint mismatches on the
matching engine (a damaged record).

### 9.5 Records made before this change

The first archived bundle is the build deployed 2026-10-07, frozen from
`target/scratch/deployed-20261007/` as bundle
`3a5ac017eec465fb0cb5fc0c8476ac8ef47b3fa926425dd777520c14937c41cf`. Records
made that day have no `bundle` field and resolve to it through the stamp-field
fallback (they were written by that exact build). Records from **earlier**
builds have no archived engine and cannot be reproduced exactly -- the loader
says so instead of approximating. Do not attempt git archaeology to rebuild
them.

Both bundles sealed on 2026-10-07 (`3a5ac017…` and `e4e20956…`) came from
**uncommitted** trees and are therefore `rebuild: false`. Their bytes live in
`data/engine-archive/`; copy that directory, do not try to regenerate them.

### 9.5.1 Rebuilding from source (option C)

A bundle whose index entry has `rebuild: true` is regenerated from its
`commit` with

```bash
node tools/rebuild-engine.mjs <bundle-id>     # or a commit
node tools/rebuild-engine.mjs --verify        # every rebuildable entry
```

The tool makes a temporary `git worktree` at the commit **in `os.tmpdir()`**
(outside the repo, so no parent `rust-toolchain.toml` / `.cargo/config.toml`
can leak into the build), runs **that commit's own** `tools/build-glue.mjs` and
`tools/build-ruleset.mjs`, assembles the bundle, and decides whether the result
is the same engine -- by **source identity and behaviour**, not by bytes:

1. **`source_id` must match the seal.** The behaviour-relevant inputs (the
   trees in §9.2) are the same. Compiler / wasm-bindgen version, `rust-src`
   presence and paths are not part of this and may differ freely. Toolchain
   differences are a **warning**, never a failure.
2. **The stored reference record must replay clean.** `archive/engine/refs/<id>.bdrec`
   is a short seeded match (two bots, seed 7) sealed beside the index at
   archive time. The rebuild loads it through the rebuilt glue with `force`
   (the rebuilt stamp names its own bundle id) and steps it to the end: every
   per-turn checkpoint hash must match, `diverged` must stay false. A
   checkpoint mismatch means the rebuilt engine *behaves* differently and the
   rebuild is rejected.

On a byte-id mismatch (the usual case across rustc installs) the rebuilt bytes
are kept under their own byte-id in `data/engine-cache/`, also installed at the
seal's id so a plain `/assets/engine/<seal>/` fetch works, and the index entry
gains

```json
"aliases":  { "<seal-id>": "<rebuilt-byte-id>", "<rebuilt-byte-id>": "<rebuilt-byte-id>" },
"rebuilt":  { "byte_id": "<rebuilt-byte-id>", "source_id": "…", "verified": true,
              "ref": "refs/<seal-id>.bdrec", "toolchain": { … } }
```

Records stamped with the seal's id still resolve: `webui/src/game/engineBundle.ts`
walks `aliases` (and `rebuilt.byte_id`) when looking a record's bundle up, and
serves the directory the alias names.

#### What the byte-level id is for

The byte id (`tools/engine-bundle.mjs`) is what a record stamps, so it stays
the stable handle a record can name. It is **not** a behaviour claim. The
2026-10-08 spike measured exactly what moves it without moving behaviour:

* **Directory / path length / non-ASCII**: no effect. Two HEAD worktrees at
  `D:\tmp\bdetA` (12 chars) and `D:\tmp\bdetB-longer-directory-name-20261008`
  (40 chars) produced **byte-identical** glue, rules modules and ruleset
  index. Workspace crates already get relative `file!()` paths
  (`crates\game-core\src\record.rs`), so the checkout's absolute path --
  including this repo's `大富翁` -- never reaches the bytes. The rules profile
  is `strip = true` + `panic = "abort"` + `codegen-units = 1`, so card modules
  carry no path strings at all. Generators sort their inputs.
* **Machine / `CARGO_HOME` / sysroot**: panic `Location` strings from registry
  crates and std name those absolute paths. `tools/build-glue.mjs` and
  `build-ruleset.mjs` set `CARGO_INCREMENTAL=0` and `--remap-path-prefix`
  (via `CARGO_ENCODED_RUSTFLAGS`, so the repo's spaces and non-ASCII path
  survive) to rewrite them to `/.cargo/…` and `/rustc/sysroot/…`. Verified:
  a remapped glue carries **zero** occurrences of the user name, the home
  directory or the repo path.
* **rustc install, including `rust-src` presence**: the same rustc version can
  be two rustup installs. `stable` with `rust-src` records std paths as
  `C:\Users\<you>\.rustup\toolchains\<name>\lib/rustlib/src/rust\library\…`;
  an install without it records `/rustc/<commit>/library/…`. Different bytes,
  same behaviour. The spike hit this the hard way -- see below.

**Do not add a `rust-toolchain.toml` to this repo.** It is not harmless here:
`stable` and a pinned `1.96.1` are different rustup installs with different
components, and a pin silently changes every build under the repo (including a
rebuild worktree placed under `target/`, because rustup walks up and finds the
file). That is what the `os.tmpdir()` worktree is for. The index records
`rustc -V` / `wasm-bindgen -V` as informational metadata only.

### 9.6 Deploy steps

1. `node tools/build-glue.mjs` (part of `npm run build`) -- writes
   `webui/src/wasm/engine_id.json` with the glue sha **and** the commit /
   dirty flag / toolchain of the bytes just built.
2. `npm run build` -- the UI, with `webui/public/assets/engine/replay-worker.js`.
3. **`node tools/archive-engine.mjs`** -- archive the build, then copy the
   store into `webui/dist/assets/engine/`. **Run this before the new dist
   goes live.** The run ends with `--check` semantics: every previously indexed
   bundle must still be servable (in the store with matching hashes, or
   `rebuild: true`) or the deploy fails. Never delete old bundle directories
   from `data/engine-archive/` -- for `rebuild: false` entries they are the
   only copy. (The site copy under `webui/dist` is disposable -- the next
   archive run restores it from the store.)
4. `node tools/archive-engine.mjs --check` -- optional repeat of the gate.
5. `node tools/archive-engine.mjs --backup <dir>` -- copy the store to your
   backup target. This command never uploads anything.
6. Server: no extra step beyond the same repo build; `rules-worker` picks the
   glue identity up from `webui/src/wasm/engine_id.json` (or `BD_GLUE_SHA` /
   `BD_ENGINE_ID`).

### 9.7 Tests

* `node --test tools/test-replay-archive.mjs` -- JS recipe == engine
  `bundle_id`; `archive-engine.mjs` produces a loadable bundle whose frozen
  engine reports exactly the index's stamp; a record sealed by build A plays
  clean on archived A while the current engine is a trivially different build
  B (a one-event data override), B refuses it, and B under `force` diverges at
  the first checkpoint.
* `node --test webui/src/game/ruleset.test.ts` -- the **conds gate**
  (`docs/GUARDS.md` §8.2): the real built ruleset (modules + the
  `conds-*.bin` precompiled guard conditions) loads into the real browser glue
  through the same `loadRulesetInto` sequence the page uses, with zero load
  errors; a card with a `pre` evaluates; a card with a `pre` and no blob fails
  loudly (`BadPre`) rather than treating the condition as true. Needs
  `tools/build-ruleset.mjs` + `tools/build-glue.mjs` first. Without this, the
  silent failure mode is a solo match running on `StubRules` while a record
  sealed elsewhere names a ruleset the player never actually played.
* `node --test webui/src/game/engineBundle.test.ts` -- the routing table.
* `cargo test -p game-core --test bundle_id` -- the recipe known vector and
  the serde default for pre-bundle records.
* `cargo test -p game-rules --test pre_conditions` -- the shipped `conds-*.bin`
  is exactly the host compile of every `pre` (`shipped_precompiled_blobs_match_host_compile`),
  and a mismatched / stale blob is a build error.
* `crates/game-core/tests/record.rs` / `record_codec.rs` stay green.

## 10. Portable records: the engine inside the file (2026-10-08)

§9 replays a record with the bundle that wrote it, fetched from the engine
archive. That is exact, but it needs the archive to hold those bytes -- a
deployment that never ran `archive-engine.mjs` for that build cannot play the
record, and nothing works offline. A **portable** `.bdrec` closes that gap
*optionally*: it carries the exact engine bundle inside the file, so the
record is self-contained.

Standing rules, unchanged: a portable file is still only ever opened and
replayed **in the browser**, never uploaded to or hosted by the server; the
plain `.bdrec` format does not change; every existing record keeps replaying
exactly as before. Exporting the portable form is opt-in (「包含引擎（可离线回放）」
/ "Include engine (play offline)", **default off**) on every download dialog.

### 10.1 Format

An additive, versioned extension. The file is three parts back to back:

```text
[0, R)          the plain `.bdrec` -- ONE zstd frame of RecordFile JSON,
                byte-identical to what `record_zst` writes today
[R, R+S)        the portable section: one **zstd skippable frame**
                (magic 184D2A50) wrapping the engine-bundle container
[R+S, R+S+32)   the trailer locating the section
```

* **Trailer (32 bytes):** `"BDRECEND"` | `u8 version=1` | `u8 kind=1` (engine
  bundle) | `u16 flags=0` | `u64le section_off` | `u64le section_len` |
  `u32le crc32` of the first 28 bytes.
* **Skippable frame:** `50 2A 4D 18` | `u32le payload_len` | payload. zstd
  decoders must skip it, so `zstd -d file.bdrec` prints the record JSON and
  nothing else.
* **Container (the payload):** `"BDRECENG"` | `u8 version=1` |
  `u8 compression=1` (zstd) | `u16 reserved` | `u32le manifest_len` |
  manifest (UTF-8 JSON) | blob (one zstd frame over `concat(file bytes)`).

The **manifest** is the `{path, sha256, len}` list the extension is built on:

```json
{ "kind": "bdrec-portable", "version": 1,
  "bundle": "<EngineStamp.bundle>", "glueSha256": "<sha256 of glue.js || glue_bg.wasm>",
  "api": 1, "compression": "zstd",
  "blobLen": 1732274, "blobSha256": "…",
  "files": [ { "path": "glue.js",          "sha256": "…", "len": 50762 },
             { "path": "glue_bg.wasm",     "sha256": "…", "len": 6703874 },
             { "path": "data/board.json",  "sha256": "…", "len": 15194 },
             { "path": "rules/index.json", "sha256": "…", "len": 65538 },
             { "path": "rules/conds-….bin", "sha256": "…", "len": 10276 },
             { "path": "modules/<sha>.wasm", "sha256": "…", "len": 393598 } ] }
```

Every file the replay worker loads rides in it: `glue.js` + `glue_bg.wasm`,
the `data/` tables, `rules/index.json`, the precompiled `conds-*.bin`, the
content-addressed `modules/*.wasm`, and `bundle.json`. Paths are
bundle-relative, slash-separated, and rejected if they contain `..`, a
leading slash, a backslash or a drive letter.

**Why a trailing section is safe.** Every reader's `expand_record` stops at the
end of the record frame and ignores what follows -- verified against every
archived bundle's glue, and pinned by
`trailing_portable_section_is_ignored` in `crates/game-core/tests/record_codec.rs`
and `record_header_bytes reads a portable file exactly like the plain one` in
`tools/test-portable.mjs`. So:

* a plain reader sees a plain record (it never looks at the tail);
* `record_header_bytes` on a portable file returns exactly the plain header;
* `zstd -d` skips the skippable frame;
* the plain `.bdrec` format is unchanged -- a portable file is a plain record
  with something appended, not a new record encoding.

A trailer that is present but malformed is an **error**, never a silent
fallback to "plain": the user asked for that engine and must know it did not
load.

**Bounds** (decompression-bomb and hostile-manifest limits, enforced before
any allocation grows past them):

| Cap | Value |
|---|---|
| whole portable section | 24 MiB |
| decompressed blob | 32 MiB |
| compressed blob | 24 MiB |
| manifest | 256 KiB |
| file count | 256 |
| one path | 200 bytes |
| one file | 24 MiB |

### 10.2 Loading

`webui/src/game/portable.ts` (the format, pure and node-safe) +
`webui/src/game/portableEngine.ts` (the policy). Order:

| Case | Policy |
|---|---|
| plain record | unchanged (§9.4) |
| record names this page's bundle | play on the page's engine |
| portable, and the hosted archive holds that bundle with the **same bytes** (the index's `files` map matches the manifest) | play the hosted bundle in the worker (§9) |
| portable, otherwise | run the **embedded** engine |
| embedded loader not on the allow-list | refuse, naming the sha and the path it would live at |

"Prefer the hosted bundle" is a byte-identity claim, not a guess: the
manifest's per-file hashes must agree with `index.json`'s. When they do not,
or the bundle is not deployed at all, the embedded engine runs -- that is the
whole point of carrying it. The user is told: 「正在使用回放文件内嵌的引擎」 /
"Playing on the engine embedded in this record (bundle …)".

**BOM note (`data_sha256`).** Three of the game tables (`cards.json`,
`characters.json`, `skill_simple.json`) begin with a UTF-8 BOM, and a
byte-preserving decode keeps it (`as_bytes()` puts `EF BB BF` back) while
`TextDecoder`'s default and `Response.text()` strip it. The stamp recipe is
unified on **hash without the BOM**: `load_data`, `Ctx::load` and
`tools/archive-engine.mjs` all drop a leading `U+FEFF` before hashing, so a
record sealed by `tools/`, the server or a page carries the same
`data_sha256` for the same tables. The embedded loader still decodes the
tables both ways and feeds the one whose pre-recipe hash is the record's own
`data_sha256`, so the bytes the engine parses match what the file was sealed
against -- including records sealed before the recipe was unified (those
carry the BOM-inclusive stamp; `compat` names `data_sha256` and `force`
opens them). (The same dual decode is what the hosted worker's `keepBom`
fetch does; a mismatch there falls back to the embedded engine.)

Sizes (the 2026-10-08 archive bundle `ae7ede19…`, 18 files, measured by
`tools/bdrec-portable.mjs pack`):

| Form | Size |
|---|---|
| plain `.bdrec` (ref record) | 1,115 B |
| plain `.bdrec` (a real match, +events) | ~65 KiB |
| engine bundle, uncompressed | 7,533,971 B (7.18 MiB) |
| engine bundle, zstd blob | 1,728,620 B (1.65 MiB) — 4.4× |
| portable `.bdrec` (ref + engine) | 1,732,334 B (1.65 MiB) |

### 10.3 Export

`tools/bdrec-portable.mjs pack|unpack|verify` (node, `node:zlib` zstd) and the
browser's download dialog (Results / replay list / room) build the same bytes
through the same `portable.ts`. The browser packs with the engine's own codec
(`zst_compress` / `zst_decompress` in web-glue -- the same frames `record_zst`
writes; node's libzstd and the engine's frames are cross-checked in both test
suites).

The export option is **default off**. With it on, the page collects the bundle
from the hosted archive (verified against `index.json`), or reuses a portable
record's own embedded copy on re-export, and reports the plain vs portable
sizes before the download.

```bash
node tools/bdrec-portable.mjs pack   in.bdrec [--out f] [--bundle id] [--store dir]
node tools/bdrec-portable.mjs verify in.bdrec      # hashes, bounds, loader allow-list
node tools/bdrec-portable.mjs unpack in.bdrec [--out-dir d]
```

`--store` defaults to `data/engine-archive/` (read-only: the tool never writes
into the store). The archive's reference records pack as a test.

### 10.4 Security model (the file is untrusted input)

**The embedded `glue.js` is never executed.** wasm-bindgen glue is tied to its
wasm's import/export interface, so we need *a* matching loader -- but not the
one in the file. Instead:

1. sha256 the embedded `glue.js` and `glue_bg.wasm`; check
   `sha256(glue.js || glue_bg.wasm) == manifest.glueSha256`;
2. check `manifest.glueSha256` is on the deployment's **loader allow-list** --
   the set of `glueSha256`s in `archive/engine/index.json` (plus the running
   build's own glue identity). Not on the list: refuse, with the path
   `assets/engine/loaders/<sha>/glue.js` a deployment can add. Never run it;
3. fetch **our** copy of that loader (`assets/engine/loaders/<sha>/glue.js`,
   deployed by `tools/archive-engine.mjs`) and check its sha256 equals the
   embedded `glue.js`'s -- we import our bytes, the file's copy is discarded;
4. instantiate the embedded **wasm** with that loader (`init({module_or_path:
   wasmBytes})`), and feed it the embedded tables -- all hash-verified first.

The wasm runs in a dedicated module worker (`replay-worker.js`, no DOM) with
the embedded boot (`init-embedded`). After boot the worker replaces `fetch`,
`XMLHttpRequest`, `WebSocket`, `EventSource`, `importScripts` and `caches`
with throwing stubs; the worker is never given a session token.

**Wasm import audit.** The thing that decides what an untrusted wasm can do is
the import object the glue hands it. Audited across the shipped glue builds
(`webui/src/wasm/glue.js` and the archived bundles in `data/engine-archive/`):
the wasm imports exactly three functions, all of them `./glue_bg.js` shims --

| Import | What it can do |
|---|---|
| `__wbg_Error_*` | construct a JS `Error` from a wasm string |
| `__wbg___wbindgen_throw_*` | throw that error |
| `__wbindgen_init_externref_table` | seed the externref table with `undefined` / `null` / `true` / `false` |

No `fetch`, no `eval` / `Function`, no DOM, no storage, no `postMessage`, no
timers, no crypto. `TextEncoder` / `TextDecoder` and `WebAssembly.instantiate`
are used by the glue's *own* JS (string marshalling and boot), not exposed to
the wasm. `ruleset_pre_eval` is a card-guard entry point with that name -- it
is not `eval`. The glue's `__wbg_load` does call `fetch` when given a URL; the
embedded path only ever passes bytes, and the worker's lockdown means even a
future glue that asked for a URL would throw.

Everything else the engine touches goes through the frozen replay API v1
(§9.3): JSON in, JSON out, over the worker's postMessage protocol.

**Manifest verification is total.** `verifyEmbedded` checks the blob's sha256,
decompresses under the 32 MiB cap, checks the decompressed length against the
manifest, and checks every file's length and sha256 before anything is handed
to an engine. Paths are validated against a strict allow-pattern. A mismatch
is an error, never a warning.

### 10.5 Tools and tests

* `tools/bdrec-portable.mjs` -- pack / unpack / verify (shared format code
  with the browser: `webui/src/game/portable.ts`).
* `node --test webui/src/game/portable.test.ts` -- the format: round-trip,
  tampered blob / file / manifest hashes, unsafe paths, oversized section,
  corrupt trailer, unknown loader, manifest shape.
* `node --test tools/test-portable.mjs` -- end to end against the real
  archive: every reference record packs through the CLI and replays
  **identically** as plain and as portable (byte-equal end state) on the
  bundle that wrote it; `record_header_bytes` agrees; the engine and node
  zstd codecs read each other's frames; the decompression cap holds; tampered
  hashes, an unknown loader and an oversized section are refused.
* `cargo test -p game-core --test record_codec` -- `zst_decode`'s cap, and
  that a portable tail is ignored by `parse_header_bytes` / `decode_record`.
* Browser gate: a portable reference record opened in a build whose
  `assets/engine/<bundle>/` is **absent** (rename it away) plays on the
  embedded engine and shows 「正在使用回放文件内嵌的引擎」.
* `python tools/i18n/check.py` -- en + zh-CN, no new findings.

## 11. Commit-reveal fairness (2026-10-08)

Every new match (online **and** solo) seeds its RNG from a 256-bit derived
key that nobody can bias: the server commits to a secret seed before play,
each human mixes in a nonce, and the sealed record reveals the openings
afterwards. The scheme, the exact hash recipes, the verify action in the
replay viewer and the threat model are in **`docs/FAIRNESS.md`**. What it
means here:

* `RecordHeader.fair` (additive, optional) carries the commitment shown at
  start plus the openings revealed at seal time. A record without it simply
  predates the scheme.
* `MatchSetup.seed256` (additive, optional) is the derived key the match ran
  on. When it is absent the record's `seed: u64` keys the **legacy
  xoshiro256\*\*** stream (`game_core::rng::Rng`) and replays exactly as it
  always did -- both forms coexist in one `Rng`, tagged by their JSON shape
  (`"s"` vs `"c"`), so an old save resumes and an old record rebuilds
  byte-identically on this engine as well as on its archived bundle.
* `RECORD_VERSION` is 2. A v2 record is refused by a v1 engine (so an old
  bundle never mis-replays one); a v1 record loads here, with the new fields
  defaulting to absent. `compat` only reports `format` when the record is
  **newer** than the engine -- an older format is the normal case.
* The Verify action in the replay viewer runs on the engine bundle that
  plays the record (§9) and checks the commitment opening, the derived seed,
  the initial RNG state and the full replay (`docs/FAIRNESS.md` §2).
* The fairness material sits in the `RecordHeader` JSON, inside the plain
  record frame. A portable file (§10) appends its engine after that frame and
  leaves the header alone, so the openings ride along unchanged and verify
  the same way.
