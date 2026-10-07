# Match records and replay — design (2026-10-07)

Status: P1 (core), P2 (web-glue), P3 (solo UI) and P4 (server + rules-worker)
implemented 2026-10-07; P5 remains. The same day, storage and download moved
from gzip to **zstd** (see "Framing and codec"): one shared codec in
`game-core`, the server seals compressed, and the loader still reads gzip and
plain JSON.
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
  data_sha256, engine, build}`.

  | Case | Policy |
  |---|---|
  | `format` newer than supported | Refuse. |
  | ABI differs, or a card used in the record is missing | Refuse full replay. Offer the log-only view (header plus bundled events). |
  | ruleset, data or save_version differs | Warn, then replay with checkpoint verification. |
  | Everything matches | Play. |

  At the first checkpoint mismatch, auto-pause and show 「Replay diverged at
  round R, turn T」. The user can continue (flagged inaccurate) or switch to the
  log view.
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
pub const RECORD_VERSION: u32 = 1;
pub const STEP: f32 = 0.05;
pub struct RecordFile { magic: String /*"bdrec"*/, header: RecordHeader, body: RecordBody, check: String }
pub struct EngineStamp { format: u32, save_version: u32, abi: u32, ruleset_sha256: String /*"stub"*/, data_sha256: String, engine: String, build: String }
pub struct SeatInfo { member: i32, player: String, bot: bool, mentality: BotMentality, character: String, rank: i32, score: i32 }
pub enum Origin { Solo, Online { room: String } }
pub struct RecordHeader { engine: EngineStamp, mode: MatchMode, step: f32, origin: Origin, created: String,
                          seats: Vec<SeatInfo>, partial: bool, ended: bool, reason: String, rounds: i32, total_ticks: u64 /*str*/ }
pub struct MatchSetup { members: Vec<RoomMember> /*incl. mentality*/, seed: u64 /*str*/, weights: ScoreWeights }
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
  bundled events keep a log view working.
* **Cross-backend wasm float NaN.** Test it.
* **Server restart** loses up to 100 buffered ticks. Set a `gaps` flag and mark
  the record unverifiable.
* **Concurrent work.** Bot mentality must flow through `MatchSetup`.
* **Keyframe memory in wasm.** Cap it by bytes.
* **Privacy.** A full record is intended; it is post-match and
  participants-only.
* **Old solo saves** give partial records.
