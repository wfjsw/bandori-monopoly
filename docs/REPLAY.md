# Match records and replay — design (2026-10-07)

Status: design approved for implementation; phases P1–P5 below. Line numbers
were taken from the 2026-10-07 working tree and drift.

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
  * serde JSON from Rust, gzipped in the browser via `CompressionStream` /
    `DecompressionStream` (no flate2 in the wasm).
  * The server sends `Content-Encoding: gzip` and
    `Content-Disposition: attachment; filename="bdrec-<room>-<yyyymmdd-hhmm>.bdrec"`.
  * Extension `.bdrec`. The loader sniffs the gzip magic `1f 8b` and also
    accepts plain JSON.
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
  * The last 10 solo records go in IndexedDB (`bm.replays`). A record is saved
    automatically once when a solo match ends.
  * Download via a Blob and `a[download]`. Open via
    `<input type=file accept=".bdrec,.json">` or drag and drop.
  * Size: about 6–8 KB gzipped for a typical 160-turn input log, and about
    60–90 KB with events.
* **Performance.** `Match` is not modified; recording is a wrapper, so
  `examples/sim.rs` is unchanged. The per-tick cost is one run-length
  increment. At each turn change: `save()` plus FNV.

## 2. Schema (`crates/game-core/src/record.rs`)

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
    * `record(created)` and `record_with_events(created)`.

    `tick(dt)` is kept but records nothing.
  * **`ReplayMatch`:** `from_record(json, force)`, `header`, `compat`, `step`,
    `next_input`, `index`, `seek`, `turns`, `total_ticks`,
    `view(member; 0 = spectator)`, `events_since`, `take_changed`.
  * `record_header(json)` gives a cheap header parse for the list.
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

## 5. UI (webui)

* **`game/session.ts`:**
  * Solo uses a fixed-step accumulator calling `m.tick_steps(k)`.
  * `persist` saves `rec: m.record_state()` atomically with the save. `resume`
    calls `restore_with_record`.
  * When the match ends, `saveReplay(gzip(m.record(now)))` runs once.
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
* **`game/record.ts`:** gzip / gunzip, `downloadRecord`, `readRecordFile`, and
  an IndexedDB store (list, put with keep = 10, get, delete).
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

* **P1 – core.** `record.rs` and `tests/record.rs`.
* **P2 – glue.** web-glue and regenerated bindings.
* **P3 – solo UI.**
* **P4 – server and rules-worker.**
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
* **`server/tests/http.rs`:** 409 mid-match, 200 after the end, 403 to an
  outsider; left / back are recorded; the `replay` op's hash equals the final
  blob hash.
* **webui tests:** gzip round-trip, file sniffing, IndexedDB keep-N.
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
