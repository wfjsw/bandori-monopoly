//! Match records and replay (`docs/REPLAY.md`).
//!
//! A **record** is an input log first: how the match was set up, every ordered
//! public call (`act`, `quick_start`, `finish`, `member_left`, `member_back`)
//! with its Ok/Err bit, and run-length quantized tick runs. Together with the
//! engine's determinism (both RNG streams serialize, no wall clock, no
//! `thread_rng`) that is enough to rebuild every frame of the match.
//!
//! The log is checked as it plays: at every turn boundary the recorder writes a
//! checkpoint (an FNV-1a-64 of [`Match::save`], with the tick run split there so
//! each checkpoint sits between inputs), and the replayer re-derives it and
//! flags divergence at the first mismatch. A final hash closes the body.
//!
//! Nothing here edits `Match`'s internals: [`RecordedMatch`] wraps the public
//! [`Match`] API, and [`Replayer`] drives the same API from a log. The one
//! visibility change the wrapper needed is `engine::SAVE_VERSION`, now public
//! so [`EngineStamp`] can name the save format.

use std::sync::Arc;

#[allow(unused_imports)]
use serde::{Deserialize, Serialize};

use crate::data::GameData;
use crate::engine::{CardRules, Match, SAVE_VERSION};
use crate::msg::Msg;
use crate::net::{NetMessage, RoomMember};
use crate::scoring::ScoreWeights;
use crate::state::{BotMentality, MatchEvent};
use crate::MatchMode;

/// Record file format version (`EngineStamp.format`). Bump on any change to
/// the schema below; a file with a **newer** format is refused outright.
pub const RECORD_VERSION: u32 = 1;

/// The tick quantum. `tick_steps(k)` ticks `dt = k as f32 * STEP`, k in 1..=10,
/// so replay recomputes the identical f32.
pub const STEP: f32 = 0.05;

/// `RecordFile.magic`.
pub const MAGIC: &str = "bdrec";

/// Keyframe store cap: a seek restores the nearest keyframe and re-simulates
/// forward, so the index holds `save()` strings and nothing else. 4 MiB of
/// saves is roughly a hundred frames -- plenty for a scrub bar, small enough
/// for the wasm heap.
pub const MAX_KEYFRAME_BYTES: usize = 4 * 1024 * 1024;

// ---------------------------------------------------------------- u64 as string

/// u64 fields travel as JSON strings: `JSON.parse` would silently round
/// anything above 2^53. Accepts a bare number too, for hand-edited files.
pub mod u64_str {
    use serde::{de, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &u64, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&v.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        struct V;
        impl<'de> de::Visitor<'de> for V {
            type Value = u64;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a u64 as a string")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
                v.parse().map_err(E::custom)
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
                Ok(v)
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<u64, E> {
                u64::try_from(v).map_err(E::custom)
            }
        }
        d.deserialize_any(V)
    }
}

// ---------------------------------------------------------------- hashing

/// FNV-1a-64. Not cryptographic: it only has to notice a drifted frame.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// FNV-1a-64 as 16 lowercase hex digits (the `Checkpoint.hash` / `check` form).
pub fn fnv1a64_hex(bytes: &[u8]) -> String {
    format!("{:016x}", fnv1a64(bytes))
}

/// The checkpoint / final hash of a `Match::save()` blob.
pub fn hash_save(save: &str) -> String {
    fnv1a64_hex(save.as_bytes())
}

// ---------------------------------------------------------------- schema

/// Who built the engine that wrote the record, and what format it speaks.
/// `ruleset_sha256` is `"stub"` for [`crate::engine::StubRules`].
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct EngineStamp {
    /// [`RECORD_VERSION`] of the writer.
    pub format: u32,
    /// [`SAVE_VERSION`] of the writer's `Match::save`.
    pub save_version: u32,
    /// Host/guest card ABI (`rules/card-sdk`'s `ABI_VERSION`). game-core cannot
    /// see that crate, so callers fill it in; 0 means "unknown" and is skipped
    /// by [`compat`].
    pub abi: u32,
    /// Sha256 of the built ruleset, or `"stub"`.
    pub ruleset_sha256: String,
    /// Sha256 of the `DATA_FILES` contents in order.
    pub data_sha256: String,
    /// Engine identity (`"game-core"`).
    pub engine: String,
    /// Build identity (package version, git hash, ...).
    pub build: String,
}

impl EngineStamp {
    /// What game-core knows on its own: the two format versions. The fields
    /// game-core cannot see stay empty ("unknown") and [`compat`] skips them;
    /// callers that know the full stamp (web-glue, the rules worker) should
    /// pass it to [`Replayer::new_with_stamp`] instead.
    pub fn current() -> Self {
        Self {
            format: RECORD_VERSION,
            save_version: SAVE_VERSION,
            ..Self::default()
        }
    }
}

/// One seat at the end of the match (or at export time).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeatInfo {
    pub member: i32,
    pub player: String,
    pub bot: bool,
    pub mentality: BotMentality,
    pub character: String,
    pub rank: i32,
    pub score: i32,
}

impl Default for SeatInfo {
    fn default() -> Self {
        Self {
            member: 0,
            player: String::new(),
            bot: false,
            mentality: BotMentality::Standard,
            character: String::new(),
            rank: 0,
            score: 0,
        }
    }
}

/// Where the record came from.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Origin {
    /// A local solo match: the recorder saw every seat.
    #[default]
    Solo,
    /// A server match. The full record reveals every hand and the deck order,
    /// so it is only ever served after the match ends and only to participants.
    Online { room: String },
}

/// Everything the list UI needs without parsing the body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordHeader {
    pub engine: EngineStamp,
    pub mode: MatchMode,
    pub step: f32,
    pub origin: Origin,
    /// `yyyy-MM-dd HH:mm` -- the client's clock, for display only.
    pub created: String,
    pub seats: Vec<SeatInfo>,
    /// The record starts at a `save()` snapshot, not at match creation.
    pub partial: bool,
    pub ended: bool,
    /// `MatchState.end_reason` at export (`""` while still playing).
    pub reason: String,
    pub rounds: i32,
    #[serde(with = "u64_str")]
    pub total_ticks: u64,
    /// Server-side recording lost buffered ticks (a restart mid-match). The
    /// record is then unverifiable: checkpoints after the gap do not line up
    /// with the inputs that produced them. Set by the server; always `false`
    /// for a record the engine itself wrote.
    #[serde(default)]
    pub gaps: bool,
}

impl Default for RecordHeader {
    fn default() -> Self {
        Self {
            engine: EngineStamp::current(),
            mode: MatchMode::Solo,
            step: STEP,
            origin: Origin::Solo,
            created: String::new(),
            seats: vec![],
            partial: false,
            ended: false,
            reason: String::new(),
            rounds: 0,
            total_ticks: 0,
            gaps: false,
        }
    }
}

/// `MatchHost(members, seed, mode, weights)` -- the start of a seeded record.
/// `members` carries each bot's [`BotMentality`], which is part of the
/// deterministic inputs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MatchSetup {
    pub members: Vec<RoomMember>,
    #[serde(with = "u64_str")]
    pub seed: u64,
    pub weights: ScoreWeights,
}

impl Default for MatchSetup {
    fn default() -> Self {
        Self {
            members: vec![],
            seed: 0,
            weights: ScoreWeights::default(),
        }
    }
}

/// How the recorded match was started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Init {
    /// From a seed: the whole match is reproducible from the log.
    Seed(MatchSetup),
    /// From a `Match::save()` blob: everything before the snapshot is gone
    /// (old solo saves, a mid-match server join).
    Snapshot {
        save: String,
    },
}

/// One recorded call. `#[serde(tag = "t")]` keeps the log greppable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t")]
pub enum Input {
    /// `n` calls of `tick_steps(k)`, run-length merged. Split at checkpoints.
    Ticks { k: u8, n: u32 },
    /// `act(member, msg)`. `ok` is whether it returned `Ok`.
    Act {
        m: i32,
        msg: NetMessage,
        ok: bool,
    },
    QuickStart,
    Finish,
    Left {
        m: i32,
        can_return: bool,
    },
    Back {
        m: i32,
    },
}

/// A turn-boundary state hash. `at` is how many [`Input`]s had been applied
/// (`inputs[..at]`), so a checkpoint always sits between inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub at: u32,
    #[serde(with = "u64_str")]
    pub tick: u64,
    pub round: i32,
    pub turn: i32,
    /// [`hash_save`] of the `Match::save()` at this point.
    pub hash: String,
}

/// The authoritative body of a record. `check` in the file is the FNV-1a-64
/// hex of `serde_json::to_string(&body)`, recomputed after parsing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RecordBody {
    pub init: Init,
    pub inputs: Vec<Input>,
    pub checkpoints: Vec<Checkpoint>,
    /// [`hash_save`] of the final `Match::save()` at export.
    pub final_hash: String,
    /// Optional public event log, generated at export by re-simulating. A
    /// version-robust fallback for the log-only view; not part of the
    /// authoritative input log.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<MatchEvent>>,
}

impl Default for RecordBody {
    fn default() -> Self {
        Self {
            init: Init::Snapshot {
                save: String::new(),
            },
            inputs: vec![],
            checkpoints: vec![],
            final_hash: String::new(),
            events: None,
        }
    }
}

/// A complete `.bdrec` file: this JSON, zstd-framed on disk and on the wire
/// (see [`encode_record_zst`] / [`decode_record`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordFile {
    /// Always [`MAGIC`].
    pub magic: String,
    pub header: RecordHeader,
    pub body: RecordBody,
    /// [`fnv1a64_hex`] of `serde_json::to_string(&body)`.
    pub check: String,
}

/// The `check` field's value for `body` (the exact same string the writer
/// hashed -- field order is the struct declaration order).
pub fn body_check(body: &RecordBody) -> String {
    let s = serde_json::to_string(body).expect("record body serializes");
    fnv1a64_hex(s.as_bytes())
}

// ---------------------------------------------------------------- compat

/// One field of [`EngineStamp`] that differs between the record and the
/// engine that is about to play it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mismatch {
    /// `format` | `save_version` | `abi` | `ruleset_sha256` | `data_sha256` |
    /// `diverged` | `checkpoint@<n>`.
    pub field: String,
    /// What the record was written against (or expected).
    pub want: String,
    /// What this engine has (or derived).
    pub got: String,
    /// `true` = the record cannot be replayed faithfully here (format / ABI).
    /// `false` = replay with checkpoint verification and warn.
    pub fatal: bool,
}

/// Compare two stamps. Empty / zero fields mean "unknown" and are skipped, so
/// a partial stamp (see [`EngineStamp::current`]) only reports what it knows.
/// `engine` and `build` are informational and never reported.
///
/// | Case | `fatal` |
/// |---|---|
/// | `format` differs | true |
/// | `abi` differs | true |
/// | `save_version`, `ruleset_sha256`, `data_sha256` differ | false |
pub fn compat(a: &EngineStamp, b: &EngineStamp) -> Vec<Mismatch> {
    let mut out = Vec::new();
    fn num(out: &mut Vec<Mismatch>, field: &str, x: u32, y: u32, fatal: bool) {
        if x != 0 && y != 0 && x != y {
            out.push(Mismatch {
                field: field.into(),
                want: x.to_string(),
                got: y.to_string(),
                fatal,
            });
        }
    }
    fn text(out: &mut Vec<Mismatch>, field: &str, x: &str, y: &str, fatal: bool) {
        if !x.is_empty() && !y.is_empty() && x != y {
            out.push(Mismatch {
                field: field.into(),
                want: x.into(),
                got: y.into(),
                fatal,
            });
        }
    }
    num(&mut out, "format", a.format, b.format, true);
    num(&mut out, "save_version", a.save_version, b.save_version, false);
    num(&mut out, "abi", a.abi, b.abi, true);
    text(&mut out, "ruleset_sha256", &a.ruleset_sha256, &b.ruleset_sha256, false);
    text(&mut out, "data_sha256", &a.data_sha256, &b.data_sha256, false);
    out
}

// ---------------------------------------------------------------- errors

/// Why a record would not load.
#[derive(Debug, Clone, PartialEq)]
pub enum ReplayError {
    /// Not a record we can read (bad magic, newer `format`).
    Format(String),
    /// Structurally broken or `check` mismatched.
    Corrupt(String),
    /// Stamp mismatch and the caller did not pass `force`. Carries what
    /// [`compat`] found so the UI can offer "play anyway" / "log only".
    Incompatible(Vec<Mismatch>),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ReplayError::Format(s) => write!(f, "record format: {s}"),
            ReplayError::Corrupt(s) => write!(f, "record corrupt: {s}"),
            ReplayError::Incompatible(m) => {
                write!(f, "record incompatible: ")?;
                for (i, x) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{} want {} got {}", x.field, x.want, x.got)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ReplayError {}

// ---------------------------------------------------------------- parsing

/// Parse and validate a `.bdrec` (or its plain-JSON form): magic, format
/// version and the body `check`.
pub fn parse_record(json: &str) -> Result<RecordFile, ReplayError> {
    let f: RecordFile =
        serde_json::from_str(json).map_err(|e| ReplayError::Corrupt(e.to_string()))?;
    if f.magic != MAGIC {
        return Err(ReplayError::Format(format!("magic {:?}", f.magic)));
    }
    if f.header.engine.format > RECORD_VERSION {
        return Err(ReplayError::Format(format!(
            "format {} is newer than supported {}",
            f.header.engine.format, RECORD_VERSION
        )));
    }
    let want = body_check(&f.body);
    if want != f.check {
        return Err(ReplayError::Corrupt(format!("check {} != {}", f.check, want)));
    }
    Ok(f)
}

/// Cheap header-only parse for a replay list (ignores the body and its check).
pub fn parse_header(json: &str) -> Result<RecordHeader, ReplayError> {
    #[derive(Deserialize)]
    struct HeaderOnly {
        header: RecordHeader,
    }
    let h: HeaderOnly =
        serde_json::from_str(json).map_err(|e| ReplayError::Corrupt(e.to_string()))?;
    Ok(h.header)
}

/// Recompute `check` after hand-editing a body (tests do this on purpose).
pub fn seal(file: &mut RecordFile) {
    file.check = body_check(&file.body);
}

// ---------------------------------------------------------------- codec

/// zstd frame magic (`28 B5 2F FD`, little-endian `0xFD2FB528`).
pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// gzip magic (`1F 8B`).
pub const GZIP_MAGIC: [u8; 2] = [0x1F, 0x8B];

/// What framing a `.bdrec` byte string carries. The body is always the same
/// record JSON; only the outer layer differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordEncoding {
    /// A zstd frame -- the current form for storage and download.
    Zstd,
    /// A gzip member -- what the browser wrote before zstd.
    Gzip,
    /// Plain record JSON (UTF-8).
    Json,
}

/// Sniff the framing of a `.bdrec` byte string. Bytes that are neither zstd
/// nor gzip are taken as plain JSON -- the parse is what rejects garbage.
pub fn sniff_record(bytes: &[u8]) -> RecordEncoding {
    if bytes.len() >= 4 && bytes[..4] == ZSTD_MAGIC {
        RecordEncoding::Zstd
    } else if bytes.len() >= 2 && bytes[..2] == GZIP_MAGIC {
        RecordEncoding::Gzip
    } else {
        RecordEncoding::Json
    }
}

/// True when `bytes` starts with the zstd frame magic.
pub fn is_zstd(bytes: &[u8]) -> bool {
    sniff_record(bytes) == RecordEncoding::Zstd
}

/// True when `bytes` starts with the gzip magic.
pub fn is_gzip(bytes: &[u8]) -> bool {
    sniff_record(bytes) == RecordEncoding::Gzip
}

/// Expand any accepted framing to the record JSON bytes.
///
/// Accepts zstd (the current form), gzip (older browser downloads and
/// IndexedDB rows) and plain JSON. One decode path for every caller -- the
/// wasm build included -- so an old recording plays anywhere a new one does.
pub fn expand_record(bytes: &[u8]) -> Result<Vec<u8>, ReplayError> {
    match sniff_record(bytes) {
        RecordEncoding::Zstd => {
            use std::io::Read;
            let mut out = Vec::new();
            let mut dec = ruzstd::decoding::StreamingDecoder::new(bytes)
                .map_err(|e| ReplayError::Corrupt(format!("zstd: {e}")))?;
            dec.read_to_end(&mut out)
                .map_err(|e| ReplayError::Corrupt(format!("zstd: {e}")))?;
            Ok(out)
        }
        RecordEncoding::Gzip => {
            use std::io::Read;
            let mut out = Vec::new();
            flate2::read::GzDecoder::new(bytes)
                .read_to_end(&mut out)
                .map_err(|e| ReplayError::Corrupt(format!("gzip: {e}")))?;
            Ok(out)
        }
        RecordEncoding::Json => Ok(bytes.to_vec()),
    }
}

/// Serialize a record to plain JSON (the form the `check` covers).
pub fn encode_record_json(file: &RecordFile) -> String {
    serde_json::to_string(file).expect("record serializes")
}

/// Serialize a record to a zstd-framed `.bdrec`.
///
/// One format, two encoders, chosen by target (see [`zst_encode`]). The frame
/// is a standard zstd frame either way: any zstd decoder reads it.
pub fn encode_record_zst(file: &RecordFile) -> Vec<u8> {
    zst_encode(encode_record_json(file).as_bytes())
}

/// zstd-compress a byte string into a standard frame.
///
/// * **wasm32** -- [`zst_encode_pure`]: the pure-Rust `structured-zstd`
///   encoder at level 1 (`zstd-sys` would need a C toolchain here). Measured
///   on a real 200-round record JSON: 5.4x -- the same ratio as the
///   reference binding, and ~1.7x what `ruzstd`'s only implemented level
///   managed.
/// * **native** -- the reference `zstd` binding at level 1. Measured on the
///   same JSON: 5.4x, better than levels 3 and 9. The server seals and
///   serves the bytes where the ratio shows up in Redis and in the download.
///
/// Decode stays on `ruzstd` everywhere (see [`expand_record`]): one decoder
/// reads both encoders' frames, and an old recording plays in either build.
pub fn zst_encode(bytes: &[u8]) -> Vec<u8> {
    #[cfg(target_arch = "wasm32")]
    {
        zst_encode_pure(bytes)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        zstd::stream::encode_all(bytes, 1).expect("zstd encodes")
    }
}

/// The browser-side encoder: pure-Rust `structured-zstd` at level 1.
///
/// `structured-zstd` is the maintained continuation of `ruzstd` (same
/// author) with a real level table. Its level 1 matches the reference
/// `zstd` binding's level-1 output on record JSON (same size to within a
/// fraction of a percent; see the size table in `tests/record_codec.rs`),
/// where `ruzstd`'s encoder, which only implements `Fastest`, was ~1.7x
/// worse than gzip. Output is a standard zstd frame either way.
///
/// Always compiled, not just on wasm32, so the tests measure and
/// cross-check the exact bytes the browser writes (`tests/record_codec.rs`).
pub fn zst_encode_pure(bytes: &[u8]) -> Vec<u8> {
    structured_zstd::encoding::compress_slice_to_vec(
        bytes,
        structured_zstd::encoding::CompressionLevel::Fastest,
    )
}

/// Serialize a record to a gzip member (legacy framing; tests and the size
/// comparison use it).
pub fn encode_record_gz(file: &RecordFile) -> Vec<u8> {
    use std::io::Write;
    let mut out = Vec::new();
    let mut enc =
        flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
    enc.write_all(encode_record_json(file).as_bytes())
        .expect("gzip write");
    enc.finish().expect("gzip finish");
    out
}

/// Parse any accepted framing into a [`RecordFile`].
pub fn decode_record(bytes: &[u8]) -> Result<RecordFile, ReplayError> {
    let json = expand_record(bytes)?;
    parse_record(std::str::from_utf8(&json).map_err(|e| ReplayError::Corrupt(e.to_string()))?)
}

/// Cheap header-only parse of any accepted framing (ignores the body check).
pub fn parse_header_bytes(bytes: &[u8]) -> Result<RecordHeader, ReplayError> {
    let json = expand_record(bytes)?;
    parse_header(std::str::from_utf8(&json).map_err(|e| ReplayError::Corrupt(e.to_string()))?)
}

// ---------------------------------------------------------------- recorder

/// The live input log. Serializes on its own so the client can persist it
/// beside `Match::save()` (`record_state` / `restore_with_record`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recorder {
    pub init: Init,
    pub inputs: Vec<Input>,
    pub checkpoints: Vec<Checkpoint>,
    /// Set when sealing a [`RecordFile`]; empty while the match runs.
    pub final_hash: String,
    /// Only ever filled by the export-time re-simulation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<Vec<MatchEvent>>,
    pub origin: Origin,
    pub partial: bool,
    /// Running total of the recorded tick calls (the sum of every `Ticks.n`).
    /// Kept up to date by [`Recorder::push_ticks`] so a checkpoint is O(1);
    /// recomputed by [`Recorder::recount`] after a deserialize.
    #[serde(skip)]
    ticks: u64,
}

impl Default for Recorder {
    fn default() -> Self {
        Self {
            init: Init::Snapshot {
                save: String::new(),
            },
            inputs: vec![],
            checkpoints: vec![],
            final_hash: String::new(),
            events: None,
            origin: Origin::Solo,
            partial: false,
            ticks: 0,
        }
    }
}

impl Recorder {
    pub fn new(init: Init, partial: bool) -> Self {
        Self {
            init,
            partial,
            ..Self::default()
        }
    }

    /// A seeded record: the log alone rebuilds the match.
    pub fn new_seed(setup: MatchSetup) -> Self {
        Self::new(Init::Seed(setup), false)
    }

    /// A snapshot record: everything before `save` is gone.
    pub fn new_snapshot(save: &str) -> Self {
        Self::new(
            Init::Snapshot {
                save: save.to_string(),
            },
            true,
        )
    }

    /// Total tick calls recorded (each `Ticks{k, n}` contributes `n`).
    pub fn total_ticks(&self) -> u64 {
        self.ticks
    }

    /// Recompute [`Recorder::total_ticks`] from `inputs` (after a deserialize).
    pub fn recount(&mut self) {
        self.ticks = self
            .inputs
            .iter()
            .map(|i| match i {
                Input::Ticks { n, .. } => *n as u64,
                _ => 0,
            })
            .sum();
    }

    /// True when the next tick must start a new run: a checkpoint was taken at
    /// the current input count, so runs split at turn boundaries.
    fn run_closed(&self) -> bool {
        self.checkpoints
            .last()
            .is_some_and(|c| c.at as usize == self.inputs.len())
    }

    /// Record one `tick_steps(k)` call; merges into the open run.
    pub fn push_ticks(&mut self, k: u8) {
        self.ticks += 1;
        let closed = self.run_closed();
        if let Some(Input::Ticks { k: k2, n }) = self.inputs.last_mut() {
            if *k2 == k && !closed {
                *n += 1;
                return;
            }
        }
        self.inputs.push(Input::Ticks { k, n: 1 });
    }

    pub fn push_act(&mut self, member: i32, msg: NetMessage, ok: bool) {
        self.inputs.push(Input::Act {
            m: member,
            msg,
            ok,
        });
    }

    pub fn push_quick_start(&mut self) {
        self.inputs.push(Input::QuickStart);
    }

    pub fn push_finish(&mut self) {
        self.inputs.push(Input::Finish);
    }

    pub fn push_left(&mut self, member: i32, can_return: bool) {
        self.inputs.push(Input::Left {
            m: member,
            can_return,
        });
    }

    pub fn push_back(&mut self, member: i32) {
        self.inputs.push(Input::Back { m: member });
    }

    /// Turn boundary: `at` is the number of inputs applied so far.
    pub fn push_checkpoint(&mut self, tick: u64, round: i32, turn: i32, hash: String) {
        self.checkpoints.push(Checkpoint {
            at: self.inputs.len() as u32,
            tick,
            round,
            turn,
            hash,
        });
    }

    pub fn body(&self) -> RecordBody {
        RecordBody {
            init: self.init.clone(),
            inputs: self.inputs.clone(),
            checkpoints: self.checkpoints.clone(),
            final_hash: self.final_hash.clone(),
            events: self.events.clone(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("recorder serializes")
    }
}

fn turn_key(m: &Match) -> (i32, i32) {
    let st = &m.world().st;
    (st.round, st.turn)
}

// ---------------------------------------------------------------- RecordedMatch

/// A [`Match`] plus its input log. Forwards every public entry point to the
/// match and appends to the log; the match itself is untouched.
pub struct RecordedMatch {
    m: Match,
    rec: Recorder,
    /// Last observed `(round, turn)`; a change opens a checkpoint.
    turn_key: (i32, i32),
    /// Once the match has `ended()`, recording stops.
    rec_done: bool,
}

impl RecordedMatch {
    /// `MatchHost(members, seed, mode, weights)`, recorded from the start.
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        setup: MatchSetup,
        mode: MatchMode,
    ) -> Self {
        let rec = Recorder::new_seed(setup.clone());
        let m = Match::new(
            data,
            rules,
            &setup.members,
            setup.seed,
            mode,
            setup.weights,
        );
        let turn_key = turn_key(&m);
        Self {
            m,
            rec,
            turn_key,
            rec_done: false,
        }
    }

    /// Wrap a match that is already running: the record starts at the current
    /// `save()` and is marked `partial`.
    pub fn from_snapshot(m: Match) -> Self {
        let save = m.save();
        let rec = Recorder::new_snapshot(&save);
        let turn_key = turn_key(&m);
        let rec_done = m.ended();
        Self {
            m,
            rec,
            turn_key,
            rec_done,
        }
    }

    /// Rebuild from a `Match::save()` blob plus the recorder JSON written by
    /// [`RecordedMatch::recorder_json`] (solo persist / refresh recovery).
    pub fn restore(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        save: &str,
        rec_json: &str,
    ) -> Result<Self, Msg> {
        let m = Match::restore(data, rules, save)?;
        let mut rec: Recorder = serde_json::from_str(rec_json)
            .map_err(|e| Msg::new("err.save_corrupt").text("detail", e.to_string()))?;
        rec.recount();
        let turn_key = turn_key(&m);
        let rec_done = m.ended();
        Ok(Self {
            m,
            rec,
            turn_key,
            rec_done,
        })
    }

    // ------------------------------------------------------------ recording

    /// One tick quantum: `dt = k as f32 * STEP`, k in 1..=10. `k == 0` is a
    /// no-op (and is not recorded).
    pub fn tick_steps(&mut self, k: u8) {
        if k == 0 {
            return;
        }
        self.m.tick(k as f32 * STEP);
        if self.rec_done {
            return;
        }
        self.rec.push_ticks(k);
        self.after_input();
    }

    pub fn act(&mut self, member: i32, msg: &NetMessage) -> Result<(), Msg> {
        let r = self.m.act(member, msg);
        if !self.rec_done {
            self.rec.push_act(member, msg.clone(), r.is_ok());
            self.after_input();
        }
        r
    }

    pub fn quick_start(&mut self) {
        self.m.quick_start();
        if self.rec_done {
            return;
        }
        self.rec.push_quick_start();
        self.after_input();
    }

    pub fn finish(&mut self) {
        self.m.finish();
        if self.rec_done {
            return;
        }
        self.rec.push_finish();
        self.after_input();
    }

    pub fn member_left(&mut self, member: i32, can_return: bool) {
        self.m.member_left(member, can_return);
        if self.rec_done {
            return;
        }
        self.rec.push_left(member, can_return);
        self.after_input();
    }

    pub fn member_back(&mut self, member: i32) {
        self.m.member_back(member);
        if self.rec_done {
            return;
        }
        self.rec.push_back(member);
        self.after_input();
    }

    /// Turn boundary? Then close the open tick run and write a checkpoint.
    fn after_input(&mut self) {
        let key = turn_key(&self.m);
        if key != self.turn_key {
            self.turn_key = key;
            let hash = hash_save(&self.m.save());
            let tick = self.rec.total_ticks();
            self.rec.push_checkpoint(tick, key.0, key.1, hash);
        }
        if self.m.ended() {
            self.rec_done = true;
        }
    }

    // ------------------------------------------------------------ access

    pub fn inner(&self) -> &Match {
        &self.m
    }

    pub fn inner_mut(&mut self) -> &mut Match {
        &mut self.m
    }

    pub fn recorder(&self) -> &Recorder {
        &self.rec
    }

    /// The recorder as JSON, to be stored beside `save()`.
    pub fn recorder_json(&self) -> String {
        self.rec.to_json()
    }

    // ------------------------------------------------------------ export

    /// Seal the record: header from the live state, `final_hash` from the
    /// current `save()`. `created` is a display timestamp (`yyyy-MM-dd HH:mm`).
    pub fn export(&self, stamp: EngineStamp, created: &str) -> RecordFile {
        let st = self.m.state();
        let seats = st
            .players
            .iter()
            .map(|p| SeatInfo {
                member: p.member,
                player: p.player.clone(),
                bot: p.bot,
                mentality: p.mentality,
                character: p.character.clone(),
                rank: p.rank,
                score: p.score,
            })
            .collect();
        let body = RecordBody {
            init: self.rec.init.clone(),
            inputs: self.rec.inputs.clone(),
            checkpoints: self.rec.checkpoints.clone(),
            final_hash: hash_save(&self.m.save()),
            events: self.rec.events.clone(),
        };
        let header = RecordHeader {
            engine: stamp,
            mode: MatchMode::from_i32(st.mode).unwrap_or_default(),
            step: STEP,
            origin: self.rec.origin.clone(),
            created: created.to_string(),
            seats,
            partial: self.rec.partial,
            ended: self.m.ended(),
            reason: st.end_reason.clone(),
            rounds: st.round,
            total_ticks: self.rec.total_ticks(),
            gaps: false,
        };
        let check = body_check(&body);
        RecordFile {
            magic: MAGIC.to_string(),
            header,
            body,
            check,
        }
    }

    /// [`RecordedMatch::export`], plus the public event log bundled into the
    /// body. The log is re-derived by re-simulating the record (the engine
    /// only keeps a 400-event tail), so it also works for a snapshot record.
    pub fn export_with_events(
        &self,
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        stamp: EngineStamp,
        created: &str,
    ) -> Result<RecordFile, ReplayError> {
        let mut file = self.export(stamp, created);
        let mut rp = Replayer::new(data, rules, &file, true)?;
        let mut last = rp.status().tick;
        let mut guard = 0u32;
        while !rp.status().ended {
            let st = rp.step_ticks(1);
            if st.tick == last {
                break; // no progress
            }
            last = st.tick;
            guard += 1;
            if guard > 5_000_000 {
                return Err(ReplayError::Corrupt(
                    "export_with_events did not terminate".into(),
                ));
            }
        }
        file.body.events = Some(rp.events().to_vec());
        file.check = body_check(&file.body);
        Ok(file)
    }
}

// ---------------------------------------------------------------- replayer

/// One scrub-bar / prev-turn / next-turn mark, from the checkpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnMark {
    pub round: i32,
    pub turn: i32,
    #[serde(with = "u64_str")]
    pub tick: u64,
}

/// Where the replayer is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    #[serde(with = "u64_str")]
    pub tick: u64,
    /// Nothing left to play (log exhausted, or the match ended early).
    pub ended: bool,
    /// A checkpoint hash mismatched (or an act's Ok/Err bit differed). Sticky:
    /// the UI pauses and flags the rest of the replay as inaccurate.
    pub diverged: bool,
}

/// How far the background index pass has got. `budget` on [`Replayer::index`]
/// bounds one call's work; keyframes stop at [`MAX_KEYFRAME_BYTES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexStatus {
    pub done: bool,
    /// Inputs consumed by the index pass so far.
    pub inputs: u32,
    /// Inputs in the record.
    pub total: u32,
    pub keyframes: u32,
    #[serde(with = "u64_str")]
    pub bytes: u64,
}

#[derive(Clone)]
struct Keyframe {
    tick: u64,
    /// Accumulated game time at `tick` (`sum k * step`).
    time: f32,
    /// How many inputs had been applied (`inputs[..input]`).
    input: usize,
    /// Last event id produced up to here, so a seek can drop later events.
    last_event_id: i32,
    save: String,
}

/// Plays a [`RecordFile`] back through the public [`Match`] API.
pub struct Replayer {
    data: Arc<GameData>,
    rules: Arc<dyn CardRules>,
    header: RecordHeader,
    body: RecordBody,
    step: f32,
    m: Match,
    /// Index of the next input to apply.
    cursor: usize,
    /// Ticks already applied from `inputs[cursor]` when it is a `Ticks` run.
    sub: u32,
    tick: u64,
    /// Accumulated game time (`sum k * step`), for keyframe spacing.
    time: f32,
    last_event_id: i32,
    events: Vec<MatchEvent>,
    diverged: bool,
    first_mismatch: Option<Mismatch>,
    last_act: Option<Result<(), Msg>>,
    /// Background keyframe pass (built lazily by [`Replayer::index`]).
    idx: Option<Box<Replayer>>,
    keyframes: Vec<Keyframe>,
    keyframe_bytes: usize,
    /// Keyframe capture (only on the index pass).
    capture: bool,
    /// Turn boundaries since the last keyframe.
    turns_since_kf: u32,
    last_kf_time: f32,
}

impl Replayer {
    /// Load a record and start at tick 0. `force` plays through a stamp
    /// mismatch; without it any [`compat`] result refuses the replay.
    pub fn new(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        rec: &RecordFile,
        force: bool,
    ) -> Result<Self, ReplayError> {
        Self::new_with_stamp(data, rules, rec, &EngineStamp::current(), force)
    }

    /// [`Replayer::new`], but compared against a **full** current stamp (abi,
    /// ruleset and data hashes included). web-glue and the rules worker use
    /// this; `new` only knows the two format versions.
    pub fn new_with_stamp(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        rec: &RecordFile,
        stamp: &EngineStamp,
        force: bool,
    ) -> Result<Self, ReplayError> {
        if rec.magic != MAGIC {
            return Err(ReplayError::Format(format!("magic {:?}", rec.magic)));
        }
        if rec.header.engine.format > RECORD_VERSION {
            return Err(ReplayError::Format(format!(
                "format {} is newer than supported {}",
                rec.header.engine.format, RECORD_VERSION
            )));
        }
        let want = body_check(&rec.body);
        if want != rec.check {
            return Err(ReplayError::Corrupt(format!(
                "check {} != {}",
                rec.check, want
            )));
        }
        let mis = compat(stamp, &rec.header.engine);
        if !mis.is_empty() && !force {
            return Err(ReplayError::Incompatible(mis));
        }
        Self::from_body(data, rules, rec.header.clone(), rec.body.clone())
    }

    /// Build a replayer from an already-validated body (tests, internal use).
    pub fn from_body(
        data: Arc<GameData>,
        rules: Arc<dyn CardRules>,
        header: RecordHeader,
        body: RecordBody,
    ) -> Result<Self, ReplayError> {
        let m = match &body.init {
            Init::Seed(setup) => Match::new(
                data.clone(),
                rules.clone(),
                &setup.members,
                setup.seed,
                header.mode,
                setup.weights,
            ),
            Init::Snapshot { save } => Match::restore(data.clone(), rules.clone(), save)
                .map_err(|e| ReplayError::Corrupt(e.to_string()))?,
        };
        let step = if header.step.is_finite() && header.step > 0.0 {
            header.step
        } else {
            STEP
        };
        let mut rp = Self {
            data,
            rules,
            header,
            body,
            step,
            m,
            cursor: 0,
            sub: 0,
            tick: 0,
            time: 0.0,
            last_event_id: 0,
            events: vec![],
            diverged: false,
            first_mismatch: None,
            last_act: None,
            idx: None,
            keyframes: vec![],
            keyframe_bytes: 0,
            capture: false,
            turns_since_kf: 0,
            last_kf_time: 0.0,
        };
        rp.drain_events();
        Ok(rp)
    }

    // ------------------------------------------------------------ stepping

    /// Advance at most `n` tick quanta. Non-tick inputs that sit at the
    /// current tick position are applied along the way (the log is ordered),
    /// without consuming budget.
    pub fn step_ticks(&mut self, n: u32) -> Status {
        for _ in 0..n {
            if self.ended() {
                break;
            }
            self.apply_pending_inputs();
            if self.ended() {
                break;
            }
            if !self.apply_one_tick() {
                break;
            }
        }
        self.status()
    }

    /// Apply exactly one log entry (a whole `Ticks` run, or one call).
    pub fn next_input(&mut self) -> Status {
        if self.ended() {
            return self.status();
        }
        match self.body.inputs.get(self.cursor) {
            Some(Input::Ticks { .. }) => {
                let mut guard = 0u32;
                while self.apply_one_tick() {
                    guard += 1;
                    if guard > 1_000_000 {
                        break;
                    }
                    // apply_one_tick advances `cursor` when the run ends.
                    if self.sub == 0 {
                        break;
                    }
                }
            }
            Some(_) => {
                self.apply_non_tick();
            }
            None => {}
        }
        self.status()
    }

    /// Every non-tick input sitting at the current position.
    fn apply_pending_inputs(&mut self) {
        while let Some(input) = self.body.inputs.get(self.cursor) {
            if matches!(input, Input::Ticks { .. }) {
                break;
            }
            self.apply_non_tick();
            if self.ended() {
                break;
            }
        }
    }

    fn apply_non_tick(&mut self) {
        let Some(input) = self.body.inputs.get(self.cursor).cloned() else {
            return;
        };
        match input {
            Input::Act { m, msg, ok } => {
                let r = self.m.act(m, &msg);
                if r.is_ok() != ok {
                    self.flag(format!(
                        "act {:?} at input {} was {}, replay got {}",
                        msg.act,
                        self.cursor,
                        if ok { "Ok" } else { "Err" },
                        if r.is_ok() { "Ok" } else { "Err" },
                    ));
                }
                self.last_act = Some(r);
            }
            Input::QuickStart => self.m.quick_start(),
            Input::Finish => self.m.finish(),
            Input::Left { m, can_return } => self.m.member_left(m, can_return),
            Input::Back { m } => self.m.member_back(m),
            Input::Ticks { .. } => unreachable!("tick run handled by apply_one_tick"),
        }
        self.cursor += 1;
        self.drain_events();
        self.check_checkpoint();
        self.maybe_keyframe();
    }

    /// One `tick_steps(k)` of the run at the cursor. False when there is none.
    fn apply_one_tick(&mut self) -> bool {
        let (k, n) = match self.body.inputs.get(self.cursor) {
            Some(Input::Ticks { k, n }) => (*k, *n),
            _ => return false,
        };
        self.m.tick(k as f32 * self.step);
        self.tick += 1;
        self.time += k as f32 * self.step;
        self.sub += 1;
        self.drain_events();
        if self.sub >= n {
            self.cursor += 1;
            self.sub = 0;
            self.check_checkpoint();
            self.maybe_keyframe();
        }
        true
    }

    /// Verify every checkpoint at the current input count.
    fn check_checkpoint(&mut self) {
        let at = self.cursor as u32;
        let due: Vec<Checkpoint> = self
            .body
            .checkpoints
            .iter()
            .filter(|c| c.at == at)
            .cloned()
            .collect();
        for cp in due {
            let h = hash_save(&self.m.save());
            if h != cp.hash {
                self.flag(format!(
                    "checkpoint at {} (round {}, turn {}, tick {}) hash {} != {}",
                    cp.at, cp.round, cp.turn, cp.tick, cp.hash, h
                ));
                if self.first_mismatch.is_none() {
                    self.first_mismatch = Some(Mismatch {
                        field: format!("checkpoint@{}", cp.at),
                        want: cp.hash.clone(),
                        got: h,
                        fatal: false,
                    });
                }
            }
        }
    }

    fn flag(&mut self, what: String) {
        if !self.diverged {
            self.diverged = true;
            if self.first_mismatch.is_none() {
                self.first_mismatch = Some(Mismatch {
                    field: "diverged".into(),
                    want: what,
                    got: String::new(),
                    fatal: false,
                });
            }
        }
    }

    /// Pull new events off the engine's 400-tail into the collected stream.
    fn drain_events(&mut self) {
        let new = self.m.events_since(self.last_event_id);
        if let Some(e) = new.last() {
            self.last_event_id = e.id;
            self.events.extend(new);
        }
    }

    /// Index pass only: keep a `save()` about every 4 turn boundaries or 20 s
    /// of game time, up to [`MAX_KEYFRAME_BYTES`].
    fn maybe_keyframe(&mut self) {
        if !self.capture {
            return;
        }
        let is_turn = self
            .body
            .checkpoints
            .iter()
            .any(|c| c.at as usize == self.cursor);
        if is_turn {
            self.turns_since_kf += 1;
        }
        let due = self.keyframes.is_empty()
            || self.turns_since_kf >= 4
            || self.time - self.last_kf_time >= 20.0;
        if !due || self.keyframe_bytes >= MAX_KEYFRAME_BYTES {
            return;
        }
        let save = self.m.save();
        self.keyframe_bytes += save.len();
        self.turns_since_kf = 0;
        self.last_kf_time = self.time;
        self.keyframes.push(Keyframe {
            tick: self.tick,
            time: self.time,
            input: self.cursor,
            last_event_id: self.last_event_id,
            save,
        });
    }

    // ------------------------------------------------------------ indexing

    /// Advance the background keyframe pass by at most `budget` inputs.
    /// Keyframes are `save()` blobs about every 4 turn boundaries or 20 s of
    /// game time, capped at [`MAX_KEYFRAME_BYTES`].
    pub fn index(&mut self, budget: usize) -> IndexStatus {
        if self.idx.is_none() {
            let mut fresh = match Self::from_body(
                self.data.clone(),
                self.rules.clone(),
                self.header.clone(),
                self.body.clone(),
            ) {
                Ok(rp) => rp,
                Err(_) => {
                    return IndexStatus {
                        done: true,
                        inputs: 0,
                        total: self.body.inputs.len() as u32,
                        keyframes: self.keyframes.len() as u32,
                        bytes: self.keyframe_bytes as u64,
                    };
                }
            };
            fresh.capture = true;
            fresh.maybe_keyframe();
            self.idx = Some(Box::new(fresh));
        }
        let total = self.body.inputs.len();
        let mut used = 0usize;
        {
            let idx = self.idx.as_mut().expect("just built");
            while used < budget && idx.cursor < total {
                let before = idx.cursor;
                idx.next_input();
                if idx.cursor == before {
                    break; // no progress
                }
                used += 1;
                if idx.ended() {
                    break;
                }
            }
            self.keyframes = idx.keyframes.clone();
            self.keyframe_bytes = idx.keyframe_bytes;
        }
        let done = self
            .idx
            .as_ref()
            .is_some_and(|i| i.cursor >= total || i.ended());
        IndexStatus {
            done,
            inputs: self.idx.as_ref().map_or(0, |i| i.cursor as u32),
            total: total as u32,
            keyframes: self.keyframes.len() as u32,
            bytes: self.keyframe_bytes as u64,
        }
    }

    // ------------------------------------------------------------ queries

    pub fn status(&self) -> Status {
        Status {
            tick: self.tick,
            ended: self.ended(),
            diverged: self.diverged,
        }
    }

    /// Nothing left to play: the log is consumed or the match ended early.
    pub fn ended(&self) -> bool {
        self.cursor >= self.body.inputs.len() || self.m.ended()
    }

    pub fn total_ticks(&self) -> u64 {
        self.body
            .inputs
            .iter()
            .map(|i| match i {
                Input::Ticks { n, .. } => *n as u64,
                _ => 0,
            })
            .sum()
    }

    /// Turn-boundary marks for the scrub bar (from the checkpoints).
    pub fn turns(&self) -> Vec<TurnMark> {
        self.body
            .checkpoints
            .iter()
            .map(|c| TurnMark {
                round: c.round,
                turn: c.turn,
                tick: c.tick,
            })
            .collect()
    }

    /// Jump to `tick` (clamped to [`Replayer::total_ticks`]): restore the
    /// nearest keyframe and re-simulate forward. Event history before the
    /// restored keyframe is dropped when it is not already collected.
    pub fn seek(&mut self, tick: u64) -> Result<Status, ReplayError> {
        let target = tick.min(self.total_ticks());
        let best = self
            .keyframes
            .iter()
            .filter(|k| k.tick <= target)
            .max_by_key(|k| k.tick)
            .map(|k| (k.tick, k.time, k.input, k.last_event_id, k.save.clone()));
        // Rebuild from the init (cheap for a seed, one restore for a snapshot).
        let mut rp = Self::from_body(
            self.data.clone(),
            self.rules.clone(),
            self.header.clone(),
            self.body.clone(),
        )?;
        if let Some((k_tick, k_time, k_input, k_last_event, k_save)) = best {
            let m = Match::restore(self.data.clone(), self.rules.clone(), &k_save)
                .map_err(|e| ReplayError::Corrupt(e.to_string()))?;
            rp.m = m;
            rp.cursor = k_input;
            rp.sub = 0;
            rp.tick = k_tick;
            rp.time = k_time;
            rp.last_event_id = k_last_event;
            // Carry the event prefix over only when this session has already
            // collected it (a plain forward play). Otherwise start the log at
            // the keyframe and leave the gap to the caller.
            let have_prefix = k_last_event == 0
                || self.events.iter().any(|e| e.id == k_last_event);
            if have_prefix {
                rp.events = self
                    .events
                    .iter()
                    .take_while(|e| e.id <= k_last_event)
                    .cloned()
                    .collect();
                rp.last_event_id = rp.events.last().map_or(k_last_event, |e| e.id);
            } else {
                rp.events.clear();
            }
            rp.drain_events();
        }
        while rp.tick < target && !rp.ended() {
            rp.apply_pending_inputs();
            if rp.ended() {
                break;
            }
            if !rp.apply_one_tick() {
                break;
            }
        }
        // Divergence is sticky across seeks; the keyframe index is kept.
        rp.diverged = self.diverged;
        rp.first_mismatch = self.first_mismatch.clone();
        rp.last_act = self.last_act.clone();
        rp.idx = self.idx.take();
        rp.keyframes = std::mem::take(&mut self.keyframes);
        rp.keyframe_bytes = self.keyframe_bytes;
        rp.capture = self.capture;
        rp.turns_since_kf = self.turns_since_kf;
        rp.last_kf_time = self.last_kf_time;
        *self = rp;
        Ok(self.status())
    }

    pub fn match_ref(&self) -> &Match {
        &self.m
    }

    pub fn match_mut(&mut self) -> &mut Match {
        &mut self.m
    }

    pub fn header(&self) -> &RecordHeader {
        &self.header
    }

    pub fn body(&self) -> &RecordBody {
        &self.body
    }

    /// The full event stream collected so far (the engine only keeps a tail).
    pub fn events(&self) -> &[MatchEvent] {
        &self.events
    }

    pub fn events_since(&self, last_id: i32) -> Vec<MatchEvent> {
        self.events
            .iter()
            .filter(|e| e.id > last_id)
            .cloned()
            .collect()
    }

    /// The outcome of the most recent [`Input::Act`], for tests and the
    /// divergence banner.
    pub fn last_act(&self) -> Option<&Result<(), Msg>> {
        self.last_act.as_ref()
    }

    /// The first recorded divergence (checkpoint hash or act outcome).
    pub fn first_mismatch(&self) -> Option<&Mismatch> {
        self.first_mismatch.as_ref()
    }

    /// Undo the sticky divergence flag (tests; the UI leaves it set).
    pub fn clear_divergence(&mut self) {
        self.diverged = false;
        self.first_mismatch = None;
    }
}