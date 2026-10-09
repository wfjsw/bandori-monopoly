//! A warm pool of `rules-worker` processes.
//!
//! Workers are long-running and hold no match state between requests -- every
//! request carries the whole match -- so any worker can serve any room, there is
//! nothing to synchronise, and N processes give N-way parallelism. The pool
//! keeps them warm, hands each call to one, and restarts one that dies or
//! overruns its deadline.
//!
//! [`Pool::call`] is **blocking**. On the async runtime that means going through
//! `tokio::task::spawn_blocking` (see `state.rs`: critical sections are short and
//! never await). The deadline is what keeps a wedged worker from holding a room
//! lock forever -- it is killed and replaced instead.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use game_core::data::GameData;
use game_core::engine::CardRules;
use game_core::msg::Msg;
use game_core::net::{NetMessage, RoomMember};
use game_core::scoring::ScoreWeights;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// How long one round-trip may take before the worker is treated as wedged.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

/// One worker process plus the reader thread that owns its stdout.
struct Worker {
    child: Child,
    stdin: ChildStdin,
    /// Lines from the reader thread. The sender drops on exit, which is how a
    /// dead worker is noticed (`RecvTimeoutError::Disconnected`).
    rx: Receiver<String>,
}

impl Worker {
    fn spawn(
        exe: &std::path::Path,
        data: &std::path::Path,
        rules: &std::path::Path,
    ) -> Result<Self, String> {
        let mut child = Command::new(exe)
            .arg("--data")
            .arg(data)
            .arg("--rules")
            .arg(rules)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn {}: {e}", exe.display()))?;
        let stdin = child.stdin.take().ok_or("worker has no stdin")?;
        let stdout = child.stdout.take().ok_or("worker has no stdout")?;
        let (tx, rx) = mpsc::channel();
        // The reader thread lives as long as the worker; it exits on EOF, which
        // drops `tx` and closes the channel.
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self { child, stdin, rx })
    }

    /// Write one request, read one response. Errors mean the worker is unusable.
    fn roundtrip(&mut self, req: &Value, timeout: Duration) -> Result<Value, String> {
        let line = serde_json::to_string(req).map_err(|e| e.to_string())?;
        writeln!(self.stdin, "{line}").map_err(|e| format!("write: {e}"))?;
        self.stdin.flush().map_err(|e| format!("flush: {e}"))?;
        let resp = self.rx.recv_timeout(timeout).map_err(|e| match e {
            RecvTimeoutError::Timeout => format!("worker did not answer within {timeout:?}"),
            RecvTimeoutError::Disconnected => "worker exited".to_string(),
        })?;
        serde_json::from_str(&resp).map_err(|e| format!("bad worker response: {e}"))
    }

    fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One mutating round-trip: the new blob, the op's own value, the turn-boundary
/// checkpoint it crossed (if any), and whether the match has just ended.
#[derive(Debug, Clone)]
pub struct Out<T> {
    pub state: String,
    pub value: T,
    /// `cp` from the worker: `{round, turn, hash}` at a turn boundary.
    pub cp: Option<Cp>,
    pub ended: bool,
}

impl Out<()> {
    fn unit(v: Value) -> Result<Self, String> {
        let (state, cp, ended) = split(&v)?;
        Ok(Self {
            state,
            value: (),
            cp,
            ended,
        })
    }
}

/// The worker's turn-boundary checkpoint. [`crate::store::Cp`] is the same
/// thing once the server has filled in the input count and tick total.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cp {
    pub round: i32,
    pub turn: i32,
    pub hash: String,
}

fn split(v: &Value) -> Result<(String, Option<Cp>, bool), String> {
    let state = v
        .get("state")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "worker returned no state".to_string())?;
    let cp = match v.get("cp") {
        None | Some(Value::Null) => None,
        Some(c) => Some(serde_json::from_value(c.clone()).map_err(|e| format!("cp: {e}"))?),
    };
    let ended = v.get("ended").and_then(Value::as_bool).unwrap_or(false);
    Ok((state, cp, ended))
}

/// N warm workers, shared across rooms.
pub struct Pool {
    workers: Vec<Mutex<Option<Worker>>>,
    next: AtomicUsize,
    exe: PathBuf,
    data: PathBuf,
    rules: PathBuf,
    timeout: Duration,
    /// When set, requests run in this process through the same protocol the
    /// worker binary speaks. That is the tests, and a fallback when no worker
    /// binary is installed -- not the production path.
    local: Option<rules_worker::Ctx>,
    /// The engine stamp, cached after the first [`Pool::info`]. It names the
    /// worker binary's identity and is fixed for the life of this process, so
    /// the commit-reveal slot can bind it at room creation without a
    /// round-trip per room (`docs/FAIRNESS.md` §1.1).
    stamp: OnceLock<game_core::record::EngineStamp>,
}

impl Pool {
    /// Start `n` workers. `exe` is the `rules-worker` binary; `data`/`rules` are
    /// forwarded to it (they are the immutable game tables it loads once).
    pub fn start(n: usize, exe: PathBuf, data: PathBuf, rules: PathBuf) -> Arc<Self> {
        let n = n.max(1);
        let pool = Arc::new(Self {
            workers: (0..n).map(|_| Mutex::new(None)).collect(),
            next: AtomicUsize::new(0),
            exe,
            data,
            rules,
            timeout: DEFAULT_TIMEOUT,
            local: None,
            stamp: OnceLock::new(),
        });
        for slot in &pool.workers {
            *slot.lock().unwrap() = Worker::spawn(&pool.exe, &pool.data, &pool.rules).ok();
        }
        let live = pool
            .workers
            .iter()
            .filter(|w| w.lock().unwrap().is_some())
            .count();
        eprintln!("rules pool: {live}/{n} workers from {}", pool.exe.display());
        if live == 0 {
            eprintln!("WARNING: no rule workers started -- matches cannot run");
        }
        pool
    }

    /// The `rules-worker` binary next to this executable, or `BM_WORKER`.
    pub fn default_exe() -> PathBuf {
        if let Ok(p) = std::env::var("BM_WORKER") {
            return PathBuf::from(p);
        }
        let mut p = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        p.pop();
        p.push(if cfg!(windows) {
            "rules-worker.exe"
        } else {
            "rules-worker"
        });
        p
    }

    /// The engine in this process -- no child workers. Tests and the fallback
    /// for a missing `rules-worker` binary use this; production uses [`Pool::start`].
    pub fn in_process(data: Arc<GameData>, rules: Arc<dyn CardRules>) -> Arc<Self> {
        Arc::new(Self {
            workers: vec![],
            next: AtomicUsize::new(0),
            exe: PathBuf::new(),
            data: PathBuf::new(),
            rules: PathBuf::new(),
            timeout: DEFAULT_TIMEOUT,
            local: Some(rules_worker::Ctx::new(data, rules)),
            stamp: OnceLock::new(),
        })
    }

    /// One round-trip. Blocking; see the module docs for why that is deliberate.
    pub fn call(&self, req: Value) -> Result<Value, String> {
        if let Some(ctx) = &self.local {
            let v = rules_worker::handle(ctx, req);
            return if v.get("ok").and_then(Value::as_bool) == Some(false) {
                Err(v
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("worker error")
                    .to_string())
            } else {
                Ok(v)
            };
        }
        let i = self.next.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        let mut slot = self.workers[i].lock().unwrap();
        let out = match slot.as_mut() {
            Some(w) => w.roundtrip(&req, self.timeout),
            None => Err("worker not running".to_string()),
        };
        match out {
            Ok(v) => {
                if v.get("ok").and_then(Value::as_bool) == Some(false) {
                    Err(v
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("worker error")
                        .to_string())
                } else {
                    Ok(v)
                }
            }
            Err(e) => {
                // Dead or wedged: throw it away and bring up a fresh one. The
                // caller retries; the match blob it holds is still valid.
                if let Some(w) = slot.as_mut() {
                    w.kill();
                }
                *slot = Worker::spawn(&self.exe, &self.data, &self.rules).ok();
                Err(e)
            }
        }
    }

    // -------------------------------------------------- typed operations
    //
    // Each is one request. `state` is `Match::save`; the `String` in the Ok is
    // the new blob the caller must store. Mutating ops come back as an [`Out`],
    // which also carries the turn-boundary checkpoint and the `ended` bit the
    // record log needs (`docs/REPLAY.md` §4).

    /// The engine's identity: the [`EngineStamp`] a record should be sealed
    /// with. From the worker's `info` op, so a worker binary and the in-process
    /// fallback report the same thing. Cached after the first call -- the
    /// stamp is the worker binary's identity and cannot change under a running
    /// pool.
    pub fn info(&self) -> Result<game_core::record::EngineStamp, String> {
        if let Some(s) = self.stamp.get() {
            return Ok(s.clone());
        }
        let v = self.call(json!({"op": "info"}))?;
        let s = v
            .get("stamp")
            .cloned()
            .ok_or_else(|| "worker returned no stamp".to_string())?;
        let s: game_core::record::EngineStamp =
            serde_json::from_value(s).map_err(|e| format!("stamp: {e}"))?;
        let _ = self.stamp.set(s.clone());
        Ok(s)
    }

    /// Build a match from the derived 256-bit seed (`docs/FAIRNESS.md`). The
    /// worker keys its ChaCha12 stream from `seed256`; the legacy `seed`
    /// field is no longer sent (a worker that only understands it is older
    /// than this scheme and must not run a fair match).
    pub fn new_match(
        &self,
        members: &[RoomMember],
        seed256: [u8; 32],
        mode: i32,
        weights: &ScoreWeights,
    ) -> Result<String, String> {
        let hex: String = seed256.iter().map(|b| format!("{b:02x}")).collect();
        let v = self.call(json!({"op": "new", "members": members, "seed256": hex, "mode": mode, "weights": weights}))?;
        v.get("state")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no state".to_string())
    }

    /// `Ok` on a rejected command too: the state is returned either way (a
    /// rejected command may still have moved the match). `value` is the `Msg`
    /// that rejected it, or `None`.
    pub fn act(
        &self,
        state: &str,
        member: i32,
        cmd: &NetMessage,
    ) -> Result<Out<Option<Msg>>, String> {
        let v = self.call(json!({"op": "act", "state": state, "member": member, "cmd": cmd}))?;
        let (state, cp, ended) = split(&v)?;
        let err = match v.get("error") {
            None | Some(Value::Null) => None,
            Some(e) => Some(serde_json::from_value(e.clone()).map_err(|e| e.to_string())?),
        };
        Ok(Out {
            state,
            value: err,
            cp,
            ended,
        })
    }

    /// `value` is whether the match changed (`"changed"` in the reply).
    pub fn tick(&self, state: &str, dt: f32) -> Result<Out<bool>, String> {
        let v = self.call(json!({"op": "tick", "state": state, "dt": dt}))?;
        let (state, cp, ended) = split(&v)?;
        let changed = v.get("changed").and_then(Value::as_bool).unwrap_or(false);
        Ok(Out {
            state,
            value: changed,
            cp,
            ended,
        })
    }

    pub fn quick_start(&self, state: &str) -> Result<Out<()>, String> {
        let v = self.call(json!({"op": "quick_start", "state": state}))?;
        Out::unit(v)
    }

    pub fn finish(&self, state: &str) -> Result<Out<()>, String> {
        let v = self.call(json!({"op": "finish", "state": state}))?;
        Out::unit(v)
    }

    pub fn member_left(
        &self,
        state: &str,
        member: i32,
        can_return: bool,
    ) -> Result<Out<()>, String> {
        let v = self.call(
            json!({"op": "member_left", "state": state, "member": member, "canReturn": can_return}),
        )?;
        Out::unit(v)
    }

    pub fn member_back(&self, state: &str, member: i32) -> Result<Out<()>, String> {
        let v = self.call(json!({"op": "member_back", "state": state, "member": member}))?;
        Out::unit(v)
    }

    pub fn view(&self, state: &str, member: i32) -> Result<Value, String> {
        let v = self.call(json!({"op": "view", "state": state, "member": member}))?;
        v.get("view")
            .cloned()
            .ok_or_else(|| "worker returned no view".to_string())
    }

    pub fn events(&self, state: &str, since: i32) -> Result<Vec<Value>, String> {
        let v = self.call(json!({"op": "events", "state": state, "since": since}))?;
        Ok(v.get("events")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    pub fn ended(&self, state: &str) -> Result<bool, String> {
        Ok(self
            .call(json!({"op": "ended", "state": state}))?
            .get("ended")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    pub fn changed(&self, state: &str) -> Result<bool, String> {
        Ok(self
            .call(json!({"op": "changed", "state": state}))?
            .get("changed")
            .and_then(Value::as_bool)
            .unwrap_or(false))
    }

    /// Re-run a `.bdrec` on the worker: `(final_hash, diverged)`. `record` is
    /// the sealed file's bytes -- a zstd frame today, plain JSON from an older
    /// store -- and the worker decodes any framing. `final_hash` is the hash of
    /// the `save()` the replay **produced**, so the caller can compare it
    /// against the blob it kept.
    pub fn replay(&self, record: &[u8]) -> Result<(String, bool), String> {
        let bytes: Vec<Value> = record.iter().map(|&b| Value::from(b)).collect();
        let v = self.call(json!({"op": "replay", "record": bytes}))?;
        let hash = v
            .get("final_hash")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "worker returned no final_hash".to_string())?;
        let diverged = v.get("diverged").and_then(Value::as_bool).unwrap_or(false);
        Ok((hash, diverged))
    }
}
